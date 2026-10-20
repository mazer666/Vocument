//! Energy and thermal adaptation of the background work (`POWER-001`, `BIGMOBILE-001`).
//!
//! A phone that is almost empty or that gets hot must not run the same
//! background work as a phone that is plugged in and cool. The decision itself
//! belongs into the shared core, because macOS and Android have to behave the
//! same way and because a decision that only lives in the interface cannot be
//! tested there.
//!
//! The platform shell **reports** the state of the device; the core **decides**
//! what may run. The split keeps the policy free of platform APIs and makes it
//! verifiable on the build machine.
//!
//! # The rules
//!
//! | Situation | Effect |
//! |---|---|
//! | Critical thermal status | only interactive work, one job at a time |
//! | Severe thermal status | no background work, at most two jobs at a time |
//! | Moderate thermal status | no background work, the normal worker count |
//! | Battery below 15 % and not charging | no background work |
//! | Power saving mode | no background work, the worker count is halved |
//! | Normal | the configured limits apply |
//!
//! Background work is never *cancelled* by the policy: a suspended job stays
//! queued and runs later. Only work that the user is waiting for is affected
//! by the parallelism limit.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// Thermal status of the device, mirroring the Android `THERMAL_STATUS_*` levels
/// (`PowerManager.getCurrentThermalStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThermalStatus {
    /// The device is within its normal temperature range.
    None,
    /// The device is slightly above the normal range.
    Light,
    /// The device is noticeably warm; background work should stop.
    Moderate,
    /// The device is hot; only work that the user waits for may run.
    Severe,
    /// The device is critically hot; the application should reduce everything.
    Critical,
    /// The platform did not report a value.
    Unknown,
}

impl ThermalStatus {
    /// Parses the platform value. The numbers follow the Android level, so that
    /// the shell can pass the value through unchanged.
    pub fn from_platform_level(level: i32) -> Self {
        match level {
            0 => Self::None,
            1 => Self::Light,
            2 => Self::Moderate,
            3 => Self::Severe,
            4 => Self::Critical,
            _ => Self::Unknown,
        }
    }

    /// Returns the platform level of this status.
    pub fn platform_level(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Light => 1,
            Self::Moderate => 2,
            Self::Severe => 3,
            Self::Critical => 4,
            Self::Unknown => -1,
        }
    }
}

/// The energy state of the device, as reported by the platform shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    /// Remaining battery charge in percent, `0..=100`.
    pub battery_percent: u8,
    /// Whether the device is currently charging.
    pub is_charging: bool,
    /// The thermal status of the device.
    pub thermal_status: ThermalStatus,
    /// Whether the platform power saving mode is active.
    pub power_save_mode: bool,
}

impl PowerState {
    /// A state in which nothing restricts the background work.
    pub fn unrestricted() -> Self {
        Self {
            battery_percent: 100,
            is_charging: false,
            thermal_status: ThermalStatus::None,
            power_save_mode: false,
        }
    }

    /// A state that describes a hot device with a nearly empty battery.
    pub fn critical() -> Self {
        Self {
            battery_percent: 5,
            is_charging: false,
            thermal_status: ThermalStatus::Critical,
            power_save_mode: true,
        }
    }
}

/// The limits that the power policy derives from a [`PowerState`].
///
/// The limits are applied by the worker pool. They are deliberately coarse:
/// the point is to protect the device, not to squeeze out the last percent of
/// performance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerLimits {
    /// How many jobs may run at the same time.
    pub max_concurrent_jobs: usize,
    /// Whether background work may run at all.
    pub allow_background_jobs: bool,
    /// Whether the work should yield to an interactive request.
    pub yield_background_on_interaction: bool,
    /// A short description of the active restriction, for the user interface.
    pub reason: &'static str,
}

impl PowerLimits {
    fn unrestricted(configured_workers: usize) -> Self {
        Self {
            max_concurrent_jobs: configured_workers.max(1),
            allow_background_jobs: true,
            yield_background_on_interaction: true,
            reason: "no restriction",
        }
    }
}

/// Battery level below which background work is suspended when the device is
/// not charging.
pub const LOW_BATTERY_PERCENT: u8 = 15;

/// Derives the limits of a power state.
///
/// `configured_workers` is the limit from the central configuration, so that the
/// power policy can only *reduce* the amount of parallel work and never
/// increase it beyond what the project has configured.
pub fn derive_limits(configured_workers: usize, state: PowerState) -> PowerLimits {
    let workers = configured_workers.max(1);

    // A critically hot device only gets the work that the user waits for, and
    // even that only one job at a time.
    if state.thermal_status == ThermalStatus::Critical {
        return PowerLimits {
            max_concurrent_jobs: 1,
            allow_background_jobs: false,
            yield_background_on_interaction: true,
            reason: "critical thermal status",
        };
    }

    // A hot device stops the background work and slows the rest down.
    if state.thermal_status >= ThermalStatus::Severe {
        return PowerLimits {
            max_concurrent_jobs: workers.min(2),
            allow_background_jobs: false,
            yield_background_on_interaction: true,
            reason: "severe thermal status",
        };
    }

    if state.thermal_status == ThermalStatus::Moderate {
        return PowerLimits {
            max_concurrent_jobs: workers,
            allow_background_jobs: false,
            yield_background_on_interaction: true,
            reason: "moderate thermal status",
        };
    }

    // A nearly empty battery that is not charging stops the background work.
    if !state.is_charging && state.battery_percent < LOW_BATTERY_PERCENT {
        return PowerLimits {
            max_concurrent_jobs: workers,
            allow_background_jobs: false,
            yield_background_on_interaction: true,
            reason: "low battery",
        };
    }

    // The power saving mode of the platform halves the parallelism.
    if state.power_save_mode {
        return PowerLimits {
            max_concurrent_jobs: (workers / 2).max(1),
            allow_background_jobs: false,
            yield_background_on_interaction: true,
            reason: "power saving mode",
        };
    }

    PowerLimits::unrestricted(workers)
}

/// The current power state, shared between the platform shell and the pool.
///
/// The state is stored as atomics so that the worker loop can read it on every
/// job without taking a lock.
#[derive(Debug)]
pub struct PowerMonitor {
    battery_percent: AtomicU8,
    is_charging: AtomicBool,
    thermal_status: AtomicU8,
    power_save_mode: AtomicBool,
    configured_workers: usize,
    limits: Mutex<PowerLimits>,
}

impl PowerMonitor {
    /// Creates a monitor for a pool with the given number of workers.
    pub fn new(configured_workers: usize) -> Arc<Self> {
        let state = PowerState::unrestricted();
        Arc::new(Self {
            battery_percent: AtomicU8::new(state.battery_percent),
            is_charging: AtomicBool::new(state.is_charging),
            thermal_status: AtomicU8::new(state.thermal_status as u8),
            power_save_mode: AtomicBool::new(state.power_save_mode),
            configured_workers,
            limits: Mutex::new(PowerLimits::unrestricted(configured_workers)),
        })
    }

    /// Reports a new power state and recalculates the limits.
    pub fn report(&self, state: PowerState) -> PowerLimits {
        self.battery_percent
            .store(state.battery_percent.min(100), Ordering::Release);
        self.is_charging.store(state.is_charging, Ordering::Release);
        self.thermal_status
            .store(state.thermal_status as u8, Ordering::Release);
        self.power_save_mode
            .store(state.power_save_mode, Ordering::Release);
        let limits = derive_limits(self.configured_workers, state);
        if let Ok(mut guard) = self.limits.lock() {
            *guard = limits;
        }
        limits
    }

    /// Returns the currently active limits.
    pub fn limits(&self) -> PowerLimits {
        match self.limits.lock() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    /// Returns the state that was reported last.
    pub fn state(&self) -> PowerState {
        PowerState {
            battery_percent: self.battery_percent.load(Ordering::Acquire),
            is_charging: self.is_charging.load(Ordering::Acquire),
            thermal_status: thermal_from_u8(self.thermal_status.load(Ordering::Acquire)),
            power_save_mode: self.power_save_mode.load(Ordering::Acquire),
        }
    }
}

fn thermal_from_u8(value: u8) -> ThermalStatus {
    match value {
        0 => ThermalStatus::None,
        1 => ThermalStatus::Light,
        2 => ThermalStatus::Moderate,
        3 => ThermalStatus::Severe,
        4 => ThermalStatus::Critical,
        _ => ThermalStatus::Unknown,
    }
}

#[cfg(test)]
#[path = "power/power_tests.rs"]
mod power_tests;

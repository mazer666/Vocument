//! UniFFI surface of the energy and thermal adaptation (`POWER-001`).
//!
//! The platform shell **reports** the state of the device, the shared core
//! **decides** what it means. That split keeps the decision testable on the
//! build machine and guarantees that macOS and Android behave the same way.
//!
//! The shell reports the state when it changes, for example when the device is
//! plugged in or when it gets hot. The limits that result from the report are
//! returned, so that the interface can show why the background work is
//! currently reduced.

use vocument_core::jobs::power::{PowerLimits, PowerState, ThermalStatus};

use crate::DocumentCoreError;
use crate::job_dto::job_executor;

/// The energy and thermal state of the device, as reported by the platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiPowerState {
    /// Remaining battery charge in percent, `0..=100`.
    pub battery_percent: u8,
    /// Whether the device is currently charging.
    pub is_charging: bool,
    /// The thermal level of the platform, `0..=4` (Android `THERMAL_STATUS_*`).
    pub thermal_level: i32,
    /// Whether the platform power saving mode is active.
    pub power_save_mode: bool,
}

/// The limits that the current power state implies.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPowerLimits {
    /// How many jobs may run at the same time.
    pub max_concurrent_jobs: u32,
    /// Whether background work may run at all.
    pub allow_background_jobs: bool,
    /// Whether the background work has to yield to an interactive request.
    pub yield_background_on_interaction: bool,
    /// A short description of the active restriction, for the user interface.
    pub reason: String,
}

impl From<PowerLimits> for FfiPowerLimits {
    fn from(limits: PowerLimits) -> Self {
        Self {
            max_concurrent_jobs: limits.max_concurrent_jobs as u32,
            allow_background_jobs: limits.allow_background_jobs,
            yield_background_on_interaction: limits.yield_background_on_interaction,
            reason: limits.reason.to_owned(),
        }
    }
}

/// Reports the energy and thermal state of the device (`POWER-001`).
///
/// The platform shell calls this when the state changes. The background work is
/// suspended while the device is hot or almost empty; the work that the user
/// is waiting for continues, with a reduced parallelism on a hot device.
#[uniffi::export]
pub fn document_core_report_power_state(
    state: FfiPowerState,
) -> Result<FfiPowerLimits, DocumentCoreError> {
    let internal = PowerState {
        battery_percent: state.battery_percent.min(100),
        is_charging: state.is_charging,
        thermal_status: ThermalStatus::from_platform_level(state.thermal_level),
        power_save_mode: state.power_save_mode,
    };
    let limits = job_executor()?.report_power_state(internal);
    Ok(limits.into())
}

/// Returns the limits that the current power state implies.
#[uniffi::export]
pub fn document_core_power_limits() -> Result<FfiPowerLimits, DocumentCoreError> {
    let limits = job_executor()?.power_limits();
    Ok(limits.into())
}

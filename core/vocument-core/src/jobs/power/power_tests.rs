//! Tests of the energy and thermal adaptation (`POWER-001`, `BIGMOBILE-001`).
//!
//! The tests live in their own file so that the implementation stays within
//! the size limits of `CODE-001`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::*;
use crate::config::JobBudgets;
use crate::jobs::executor::JobExecutor;
use crate::jobs::scheduler::{JobKind, JobOutcome};

fn budgets(workers: usize) -> JobBudgets {
    JobBudgets {
        max_concurrent_render_workers: workers,
        max_concurrent_background_ocr_workers: 1,
        yield_background_on_interaction: true,
        max_concurrent_workers: workers,
        max_queued_jobs: 32,
        starvation_promotion_ms: 50,
        idle_shutdown_timeout_ms: 50,
        shutdown_grace_ms: 200,
    }
}

#[test]
fn an_unrestricted_device_keeps_the_configured_limits() {
    let limits = derive_limits(4, PowerState::unrestricted());
    assert_eq!(limits.max_concurrent_jobs, 4);
    assert!(limits.allow_background_jobs);
}

#[test]
fn a_critically_hot_device_runs_only_one_interactive_job() {
    let state = PowerState {
        battery_percent: 80,
        is_charging: true,
        thermal_status: ThermalStatus::Critical,
        power_save_mode: false,
    };
    let limits = derive_limits(4, state);
    assert_eq!(limits.max_concurrent_jobs, 1);
    assert!(!limits.allow_background_jobs);
    assert_eq!(limits.reason, "critical thermal status");
}

#[test]
fn a_hot_device_stops_the_background_work_and_slows_the_rest_down() {
    let state = PowerState {
        battery_percent: 80,
        is_charging: true,
        thermal_status: ThermalStatus::Severe,
        power_save_mode: false,
    };
    let limits = derive_limits(4, state);
    assert_eq!(limits.max_concurrent_jobs, 2);
    assert!(!limits.allow_background_jobs);
}

#[test]
fn a_warm_device_stops_the_background_work_but_keeps_the_parallelism() {
    let state = PowerState {
        battery_percent: 80,
        is_charging: true,
        thermal_status: ThermalStatus::Moderate,
        power_save_mode: false,
    };
    let limits = derive_limits(4, state);
    assert_eq!(limits.max_concurrent_jobs, 4);
    assert!(!limits.allow_background_jobs);
}

#[test]
fn an_empty_battery_stops_the_background_work_while_it_is_not_charging() {
    let state = PowerState {
        battery_percent: 9,
        is_charging: false,
        thermal_status: ThermalStatus::None,
        power_save_mode: false,
    };
    let limits = derive_limits(4, state);
    assert!(!limits.allow_background_jobs);
    assert_eq!(limits.reason, "low battery");
}

#[test]
fn an_empty_battery_that_is_charging_keeps_the_background_work() {
    let state = PowerState {
        battery_percent: 9,
        is_charging: true,
        thermal_status: ThermalStatus::None,
        power_save_mode: false,
    };
    let limits = derive_limits(4, state);
    assert!(limits.allow_background_jobs);
}

#[test]
fn the_power_saving_mode_halves_the_parallelism() {
    let state = PowerState {
        battery_percent: 90,
        is_charging: false,
        thermal_status: ThermalStatus::None,
        power_save_mode: true,
    };
    let limits = derive_limits(4, state);
    assert_eq!(limits.max_concurrent_jobs, 2);
    assert!(!limits.allow_background_jobs);
    assert_eq!(limits.reason, "power saving mode");
}

#[test]
fn the_policy_never_increases_the_configured_parallelism() {
    let states = [
        PowerState::unrestricted(),
        PowerState::critical(),
        PowerState {
            battery_percent: 50,
            is_charging: false,
            thermal_status: ThermalStatus::Light,
            power_save_mode: false,
        },
    ];
    for state in states {
        let limits = derive_limits(3, state);
        assert!(
            limits.max_concurrent_jobs <= 3,
            "the policy must not exceed the configured worker count"
        );
    }
}

#[test]
fn the_thermal_levels_survive_the_round_trip_through_the_platform_value() {
    for status in [
        ThermalStatus::None,
        ThermalStatus::Light,
        ThermalStatus::Moderate,
        ThermalStatus::Severe,
        ThermalStatus::Critical,
    ] {
        assert_eq!(
            ThermalStatus::from_platform_level(status.platform_level()),
            status
        );
    }
    assert_eq!(
        ThermalStatus::from_platform_level(99),
        ThermalStatus::Unknown
    );
}

#[test]
fn the_monitor_reports_the_state_and_the_limits() {
    let monitor = PowerMonitor::new(4);
    assert!(monitor.limits().allow_background_jobs);

    let limits = monitor.report(PowerState::critical());
    assert_eq!(limits.max_concurrent_jobs, 1);
    assert!(!limits.allow_background_jobs);
    assert_eq!(monitor.state().thermal_status, ThermalStatus::Critical);
    assert_eq!(monitor.state().battery_percent, 5);
}

/// Releases a blocked job, also when the test fails, so that the pool can
/// always be shut down.
struct Release {
    flag: Arc<AtomicBool>,
}

impl Drop for Release {
    fn drop(&mut self) {
        self.flag.store(true, Ordering::SeqCst);
    }
}

#[test]
fn a_background_job_waits_while_the_device_is_hot_and_runs_afterwards() {
    let executor = JobExecutor::start(budgets(2)).expect("executor must start");
    let release = Arc::new(AtomicBool::new(false));
    let guard = Release {
        flag: Arc::clone(&release),
    };
    let ran = Arc::new(AtomicBool::new(false));
    let ran_flag = Arc::clone(&ran);

    // The device is hot: the background work must not start.
    let hot = PowerState {
        battery_percent: 80,
        is_charging: true,
        thermal_status: ThermalStatus::Severe,
        power_save_mode: false,
    };
    executor.report_power_state(hot);

    let release_flag = Arc::clone(&release);
    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderThumbnail,
            false,
            Arc::new(move |_, _| {
                ran_flag.store(true, Ordering::SeqCst);
                while !release_flag.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit");

    std::thread::sleep(Duration::from_millis(150));
    assert!(
        !ran.load(Ordering::SeqCst),
        "the background job must not run while the device is hot"
    );
    assert_eq!(
        executor.running_count(),
        0,
        "no job may be running while the background work is suspended"
    );

    // The device cools down: the suspended job starts.
    executor.report_power_state(PowerState::unrestricted());
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ran.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        ran.load(Ordering::SeqCst),
        "the suspended job must run as soon as the device allows it again"
    );

    drop(guard);
    release.store(true, Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn a_critically_hot_device_runs_the_interactive_work_but_not_the_background_work() {
    let executor = JobExecutor::start(budgets(4)).expect("executor must start");
    let release = Arc::new(AtomicBool::new(false));
    let guard = Release {
        flag: Arc::clone(&release),
    };
    let interactive_ran = Arc::new(AtomicBool::new(false));
    let interactive_flag = Arc::clone(&interactive_ran);
    let background_ran = Arc::new(AtomicBool::new(false));
    let background_flag = Arc::clone(&background_ran);

    executor.report_power_state(PowerState::critical());

    let interactive_release = Arc::clone(&release);
    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderVisibleTile,
            false,
            Arc::new(move |_, _| {
                interactive_flag.store(true, Ordering::SeqCst);
                while !interactive_release.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                JobOutcome::Completed
            }),
        )
        .expect("submit interactive");
    executor
        .submit(
            "sess_1".to_owned(),
            1,
            JobKind::RenderThumbnail,
            false,
            Arc::new(move |_, _| {
                background_flag.store(true, Ordering::SeqCst);
                JobOutcome::Completed
            }),
        )
        .expect("submit background");

    let deadline = Instant::now() + Duration::from_secs(5);
    while !interactive_ran.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        interactive_ran.load(Ordering::SeqCst),
        "the work the user waits for must still run on a hot device"
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !background_ran.load(Ordering::SeqCst),
        "the background work must stay suspended on a hot device"
    );

    drop(guard);
    release.store(true, Ordering::SeqCst);
    executor.shutdown();
}

#[test]
fn the_parallelism_drops_to_one_on_a_critically_hot_device() {
    let executor = JobExecutor::start(budgets(4)).expect("executor must start");
    let release = Arc::new(AtomicBool::new(false));
    let guard = Release {
        flag: Arc::clone(&release),
    };
    let running = Arc::new(AtomicU64::new(0));
    let peak = Arc::new(AtomicU64::new(0));
    let release_flag = Arc::clone(&release);
    let running_flag = Arc::clone(&running);
    let peak_flag = Arc::clone(&peak);

    executor.report_power_state(PowerState::critical());

    for _ in 0..4 {
        let release_flag = Arc::clone(&release_flag);
        let running_flag = Arc::clone(&running_flag);
        let peak_flag = Arc::clone(&peak_flag);
        executor
            .submit(
                "sess_1".to_owned(),
                1,
                JobKind::RenderVisibleTile,
                false,
                Arc::new(move |_, _| {
                    let now = running_flag.fetch_add(1, Ordering::SeqCst) + 1;
                    peak_flag.fetch_max(now, Ordering::SeqCst);
                    while !release_flag.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    running_flag.fetch_sub(1, Ordering::SeqCst);
                    JobOutcome::Completed
                }),
            )
            .expect("submit");
    }

    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(
        peak.load(Ordering::SeqCst),
        1,
        "a critically hot device may only run one job at a time"
    );

    drop(guard);
    release.store(true, Ordering::SeqCst);
    executor.shutdown();
}

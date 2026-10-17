//! Vocument shared Rust core (`ARCHITECTURE.md` §4).
//!
//! Owns document/business state, configuration models, transactions, jobs,
//! recovery contracts, and broker boundaries without any native UI dependencies.

pub mod config;

use config::{AppDefaultsConfig, ConfigError, PerformanceBudgetsConfig, SecurityPolicyConfig};

/// Version of the Vocument baseline and core library.
pub const BASELINE_VERSION: &str = "0.1";

/// Immutable runtime snapshot of the core bootstrap state, exposed across FFI in M0.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreBootstrapSnapshot {
    pub baseline_version: String,
    pub milestone: String,
    pub strict_offline_default: bool,
    pub telemetry_enabled: bool,
    pub target_fps: u32,
}

/// Validates embedded/central configuration and returns the M0.0 bootstrap snapshot.
pub fn bootstrap_core_snapshot() -> Result<CoreBootstrapSnapshot, ConfigError> {
    let defaults = AppDefaultsConfig::load_embedded()?;
    let budgets = PerformanceBudgetsConfig::load_embedded()?;
    let security = SecurityPolicyConfig::load_embedded()?;

    let _ = defaults;

    Ok(CoreBootstrapSnapshot {
        baseline_version: BASELINE_VERSION.to_owned(),
        milestone: "M0".to_owned(),
        strict_offline_default: security.network.default_policy == "deny_all",
        telemetry_enabled: security.privacy_and_logging.enable_telemetry,
        target_fps: budgets.interaction.target_scroll_zoom_fps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_snapshot_respects_security_and_performance_invariants() {
        let snapshot = bootstrap_core_snapshot().expect("embedded config must parse");
        assert_eq!(snapshot.baseline_version, "0.1");
        assert_eq!(snapshot.milestone, "M0");
        assert!(snapshot.strict_offline_default);
        assert!(!snapshot.telemetry_enabled);
        assert_eq!(snapshot.target_fps, 60);
    }
}

//! Vocument shared Rust core (`ARCHITECTURE.md` §4).
//!
//! Owns document/business state, configuration models, transactions, jobs,
//! recovery contracts, and broker boundaries without any native UI dependencies.

pub mod annotations;
pub mod annotations_store;
pub mod build_info;
pub mod config;
pub mod document_inspection;
pub mod file_broker;
pub mod identity;
pub mod jobs;
pub mod journal;
pub mod navigation;
pub mod page_geometry;
pub mod pdf_engine;
pub mod recovery;
pub mod recovery_actions;
pub mod redaction;
pub mod redaction_report;
pub mod redaction_rules;
pub mod render;
pub mod rendered_page;
pub mod save;
pub mod search_index;
pub mod session;
pub mod session_mutations;
pub mod session_stamps;
pub mod text_selection;
pub mod thumbnail_cache;
pub mod tile_cache;
pub mod transaction_execution;
pub mod transactions;
pub mod workbench;
pub mod workbench_stamps;

use config::{AppDefaultsConfig, ConfigError, PerformanceBudgetsConfig, SecurityPolicyConfig};

/// Version of the Vocument baseline and core library.
pub const BASELINE_VERSION: &str = "0.1";

/// Machine-readable project state, embedded at build time so that the runtime
/// milestone can never drift away from `project/project.yaml` (`PROJECT_MANAGEMENT.md` §1).
const EMBEDDED_PROJECT_STATE: &str = include_str!("../../../project/project.yaml");

/// Immutable runtime snapshot of the core bootstrap state, exposed across FFI in M0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreBootstrapSnapshot {
    pub baseline_version: String,
    pub milestone: String,
    pub strict_offline_default: bool,
    pub telemetry_enabled: bool,
    pub target_fps: u32,
    pub default_german_variant: String,
    pub built_in_ocr_languages: Vec<String>,
}

/// Extracts the current milestone identifier from the embedded project state.
///
/// `project/project.yaml` is the single source of truth for project status
/// (`PROJECT_MANAGEMENT.md` §1). The value is parsed directly instead of being
/// duplicated as a Rust constant so that `README.md`, `ROADMAP.md` and the
/// runtime handshake can never report a stale milestone.
fn current_milestone_from_project_state() -> Result<String, ConfigError> {
    for line in EMBEDDED_PROJECT_STATE.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("current_milestone:") {
            let milestone = value.trim().trim_matches('"').trim_matches('\'');
            if milestone.is_empty() {
                return Err(ConfigError::Invalid {
                    details: "project.yaml current_milestone must not be empty".to_owned(),
                });
            }
            return Ok(milestone.to_owned());
        }
    }
    Err(ConfigError::Invalid {
        details: "project.yaml is missing current_milestone".to_owned(),
    })
}

/// Validates embedded/central configuration and returns the M0 bootstrap snapshot.
pub fn bootstrap_core_snapshot() -> Result<CoreBootstrapSnapshot, ConfigError> {
    let defaults = AppDefaultsConfig::load_embedded()?;
    let budgets = PerformanceBudgetsConfig::load_embedded()?;
    let security = SecurityPolicyConfig::load_embedded()?;
    let milestone = current_milestone_from_project_state()?;

    Ok(CoreBootstrapSnapshot {
        baseline_version: BASELINE_VERSION.to_owned(),
        milestone,
        strict_offline_default: security.network.default_policy == "deny_all",
        telemetry_enabled: security.privacy_and_logging.enable_telemetry,
        target_fps: budgets.interaction.target_scroll_zoom_fps,
        default_german_variant: defaults.internationalisation.default_german_variant,
        built_in_ocr_languages: defaults.ocr.built_in_languages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_snapshot_respects_security_and_performance_invariants() {
        let snapshot = bootstrap_core_snapshot().expect("embedded config must parse");
        assert_eq!(snapshot.baseline_version, "0.1");
        // The milestone is derived from `project/project.yaml`, the single source
        // of truth. M2 is completed, M3 has started with the Page Workbench.
        assert_eq!(snapshot.milestone, "M3");
        assert!(snapshot.strict_offline_default);
        assert!(!snapshot.telemetry_enabled);
        assert_eq!(snapshot.target_fps, 60);
        assert_eq!(snapshot.default_german_variant, "de-AT");
        assert_eq!(snapshot.built_in_ocr_languages, vec!["deu", "eng"]);
    }

    #[test]
    fn milestone_is_derived_from_the_central_project_state() {
        // `project/project.yaml` is the single source of truth for project status
        // (`PROJECT_MANAGEMENT.md` §1). The runtime milestone must never drift away
        // from it, so the value is read from the embedded project state instead of
        // being duplicated as a constant.
        let embedded_milestone = current_milestone_from_project_state()
            .expect("project state must declare a current milestone");
        assert!(
            !embedded_milestone.is_empty(),
            "current_milestone must not be empty"
        );

        let snapshot = bootstrap_core_snapshot().expect("embedded config must parse");
        assert_eq!(snapshot.milestone, embedded_milestone);

        // The embedded project state must be the real file, not a stale copy.
        assert!(
            EMBEDDED_PROJECT_STATE.contains("current_milestone:"),
            "embedded project state must contain a current_milestone entry"
        );
    }
}

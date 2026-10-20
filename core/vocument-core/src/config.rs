//! Typed configuration models synchronized with `/config/*.toml` (`ADR-0002`).

use serde::Deserialize;
use thiserror::Error;

const EMBEDDED_APP_DEFAULTS: &str = include_str!("../../../config/app-defaults.toml");
const EMBEDDED_PERFORMANCE_BUDGETS: &str = include_str!("../../../config/performance-budgets.toml");
const EMBEDDED_SECURITY_POLICY: &str = include_str!("../../../config/security-policy.toml");

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse configuration TOML: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("security policy invariant violated: {0}")]
    SecurityInvariantViolation(&'static str),
    #[error("invalid central configuration: {details}")]
    Invalid { details: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppDefaultsConfig {
    pub interface: InterfaceDefaults,
    pub viewer: ViewerDefaults,
    pub search: SearchDefaults,
    pub clipboard: ClipboardDefaults,
    pub ocr: OcrDefaults,
    pub editing: EditingDefaults,
    pub recovery: RecoveryDefaults,
    pub internationalisation: InternationalisationDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InterfaceDefaults {
    pub complexity_profile: String,
    pub density: String,
    pub theme: String,
    pub status_feedback: String,
    pub desktop_layout: String,
    pub mobile_navigation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ViewerDefaults {
    pub macos_default_mode: String,
    pub mobile_default_mode: String,
    pub show_physical_page_index: bool,
    pub use_logical_page_labels: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchDefaults {
    pub default_mode: String,
    pub progressive_indexing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ClipboardDefaults {
    pub default_copy_mode: String,
    pub auto_clear_clipboard: bool,
    pub auto_clear_delay_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OcrDefaults {
    pub built_in_languages: Vec<String>,
    pub default_output_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EditingDefaults {
    pub pdf_text_edit_default: String,
    pub incremental_save_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RecoveryDefaults {
    pub autosave_interval_seconds: u64,
    pub checkpoint_transaction_interval: u32,
    pub retention_days: u32,
    pub max_recovery_storage_mb: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InternationalisationDefaults {
    pub supported_locales: Vec<String>,
    pub default_german_variant: String,
}

impl AppDefaultsConfig {
    pub fn load_embedded() -> Result<Self, ConfigError> {
        Ok(toml::from_str(EMBEDDED_APP_DEFAULTS)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PerformanceBudgetsConfig {
    pub startup: StartupBudgets,
    pub interaction: InteractionBudgets,
    pub rendering_cache: RenderingCacheBudgets,
    pub jobs: JobBudgets,
    pub code_limits: CodeLimitsBudgets,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CodeLimitsBudgets {
    pub target_max_lines_per_file: usize,
    pub hard_max_lines_per_file: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StartupBudgets {
    pub app_start_target_ms: u64,
    pub app_start_max_ms: u64,
    pub first_page_render_target_ms: u64,
    pub first_page_render_max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InteractionBudgets {
    pub target_scroll_zoom_fps: u32,
    pub indexed_search_first_result_target_ms: u64,
    pub indexed_search_first_result_max_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RenderingCacheBudgets {
    pub macos_tile_cache_max_mb: u64,
    pub android_tile_cache_max_mb: u64,
    pub thumbnail_cache_max_mb: u64,
    pub nearby_page_prefetch_radius: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JobBudgets {
    pub max_concurrent_render_workers: usize,
    pub max_concurrent_background_ocr_workers: usize,
    pub yield_background_on_interaction: bool,
    pub max_concurrent_workers: usize,
    pub max_queued_jobs: usize,
    pub starvation_promotion_ms: u64,
    pub idle_shutdown_timeout_ms: u64,
    /// Longest time the pool waits for a worker to end before it lets it go.
    ///
    /// A worker that is stuck inside an engine call cannot be stopped from the
    /// outside. The pool therefore does not wait for it forever: after the grace
    /// period the worker is detached and the shutdown continues. Without this
    /// limit, dropping the executor — and with it the application — could block
    /// indefinitely.
    pub shutdown_grace_ms: u64,
}

impl PerformanceBudgetsConfig {
    pub fn load_embedded() -> Result<Self, ConfigError> {
        Ok(toml::from_str(EMBEDDED_PERFORMANCE_BUDGETS)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SecurityPolicyConfig {
    pub network: NetworkPolicyConfig,
    pub documents: DocumentSecurityConfig,
    pub package_limits: PackageLimitsConfig,
    pub privacy_and_logging: PrivacyLoggingConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NetworkPolicyConfig {
    pub default_policy: String,
    pub allow_automatic_external_resources: bool,
    pub allow_silent_cloud_ai_fallback: bool,
    pub allow_silent_link_reputation_check: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DocumentSecurityConfig {
    pub treat_all_inputs_as_untrusted: bool,
    pub allow_automatic_pdf_javascript: bool,
    pub allow_safe_form_calculations: bool,
    pub allow_automatic_attachment_open: bool,
    pub allow_automatic_multimedia_exec: bool,
    pub require_reopen_redaction_verification: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PackageLimitsConfig {
    pub max_pdoc_archive_size_mb: u64,
    pub max_zip_compression_ratio: u32,
    pub max_zip_entries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PrivacyLoggingConfig {
    pub enable_telemetry: bool,
    pub enable_automatic_crash_upload: bool,
    pub redact_document_content_in_logs: bool,
    pub use_opaque_recovery_container_ids: bool,
}

impl SecurityPolicyConfig {
    pub fn load_embedded() -> Result<Self, ConfigError> {
        let policy: Self = toml::from_str(EMBEDDED_SECURITY_POLICY)?;
        policy.validate_invariants()?;
        Ok(policy)
    }

    pub fn validate_invariants(&self) -> Result<(), ConfigError> {
        if self.network.default_policy != "deny_all" {
            return Err(ConfigError::SecurityInvariantViolation(
                "NET-001 requires network.default_policy = 'deny_all'",
            ));
        }
        if self.privacy_and_logging.enable_telemetry {
            return Err(ConfigError::SecurityInvariantViolation(
                "LOG-001 forbids automatic telemetry",
            ));
        }
        if self.privacy_and_logging.enable_automatic_crash_upload {
            return Err(ConfigError::SecurityInvariantViolation(
                "LOG-002 forbids automatic crash upload",
            ));
        }
        if !self.documents.treat_all_inputs_as_untrusted {
            return Err(ConfigError::SecurityInvariantViolation(
                "SECARCH-001 requires treat_all_inputs_as_untrusted = true",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn central_app_defaults_parse_and_match_requirements() {
        let defaults = AppDefaultsConfig::load_embedded().expect("valid app-defaults.toml");
        assert_eq!(defaults.ocr.built_in_languages, vec!["deu", "eng"]);
        assert_eq!(
            defaults.internationalisation.default_german_variant,
            "de-AT"
        );
        assert_eq!(defaults.search.default_mode, "exact");
    }

    #[test]
    fn central_performance_budgets_parse_and_match_quality_targets() {
        let budgets =
            PerformanceBudgetsConfig::load_embedded().expect("valid performance-budgets.toml");
        assert_eq!(budgets.interaction.target_scroll_zoom_fps, 60);
        assert!(budgets.startup.first_page_render_target_ms <= 500);
        assert!(budgets.jobs.yield_background_on_interaction);
        assert_eq!(budgets.code_limits.target_max_lines_per_file, 600);
        assert_eq!(budgets.code_limits.hard_max_lines_per_file, 900);
    }

    #[test]
    fn central_security_policy_enforces_mandatory_invariants() {
        let policy = SecurityPolicyConfig::load_embedded().expect("valid security-policy.toml");
        assert_eq!(policy.network.default_policy, "deny_all");
        assert!(!policy.network.allow_silent_cloud_ai_fallback);
        assert!(!policy.documents.allow_automatic_pdf_javascript);
        assert!(policy.documents.require_reopen_redaction_verification);
    }
}

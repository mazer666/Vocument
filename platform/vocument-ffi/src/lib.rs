//! Thin UniFFI transport boundary (`DocumentCore` API) between `vocument-core` and native UI (`ARCHITECTURE.md` §5).
//!
//! Exposes only opaque handles and immutable DTO snapshots. Never exposes raw engine pointers.

use thiserror::Error;
use vocument_core::{CoreBootstrapSnapshot, bootstrap_core_snapshot};

uniffi::setup_scaffolding!("vocument_ffi");

/// Error type crossing the UniFFI boundary from `DocumentCore`.
#[derive(Debug, Error, uniffi::Error, PartialEq, Eq)]
pub enum DocumentCoreError {
    #[error("configuration error: {details}")]
    ConfigurationError { details: String },
}

/// Immutable DTO snapshot returned across the UniFFI bridge in M0.0 to verify end-to-end linkage
/// and central configuration enforcement from both Swift (macOS) and Kotlin (Android).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DocumentCoreHandshake {
    pub baseline_version: String,
    pub milestone: String,
    pub strict_offline_default: bool,
    pub telemetry_enabled: bool,
    pub target_fps: u32,
    pub default_german_variant: String,
    pub built_in_ocr_languages: Vec<String>,
}

impl From<CoreBootstrapSnapshot> for DocumentCoreHandshake {
    fn from(snapshot: CoreBootstrapSnapshot) -> Self {
        Self {
            baseline_version: snapshot.baseline_version,
            milestone: snapshot.milestone,
            strict_offline_default: snapshot.strict_offline_default,
            telemetry_enabled: snapshot.telemetry_enabled,
            target_fps: snapshot.target_fps,
            default_german_variant: snapshot.default_german_variant,
            built_in_ocr_languages: snapshot.built_in_ocr_languages,
        }
    }
}

/// Performs the M0.0 `DocumentCore` bootstrap handshake for Swift and Kotlin callers.
#[uniffi::export]
pub fn document_core_handshake() -> Result<DocumentCoreHandshake, DocumentCoreError> {
    bootstrap_core_snapshot()
        .map(DocumentCoreHandshake::from)
        .map_err(|err| DocumentCoreError::ConfigurationError {
            details: err.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_handshake_returns_verified_core_snapshot() {
        let handshake = document_core_handshake().expect("handshake must succeed");
        assert_eq!(handshake.baseline_version, "0.1");
        assert_eq!(handshake.milestone, "M0");
        assert!(handshake.strict_offline_default);
        assert!(!handshake.telemetry_enabled);
        assert_eq!(handshake.target_fps, 60);
        assert_eq!(handshake.default_german_variant, "de-AT");
        assert_eq!(handshake.built_in_ocr_languages, vec!["deu", "eng"]);
    }
}

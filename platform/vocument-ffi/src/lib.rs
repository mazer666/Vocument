//! Thin transport boundary (`DocumentCore` API) between `vocument-core` and native UI (`ARCHITECTURE.md` §5).
//!
//! Exposes only opaque handles and immutable DTO snapshots. Never exposes raw engine pointers.

use vocument_core::{CoreBootstrapSnapshot, bootstrap_core_snapshot};

/// Immutable DTO snapshot returned across the native bridge in M0.0 to verify end-to-end linkage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentCoreHandshake {
    pub baseline_version: String,
    pub milestone: String,
    pub strict_offline_default: bool,
    pub telemetry_enabled: bool,
    pub target_fps: u32,
}

impl From<CoreBootstrapSnapshot> for DocumentCoreHandshake {
    fn from(snapshot: CoreBootstrapSnapshot) -> Self {
        Self {
            baseline_version: snapshot.baseline_version,
            milestone: snapshot.milestone,
            strict_offline_default: snapshot.strict_offline_default,
            telemetry_enabled: snapshot.telemetry_enabled,
            target_fps: snapshot.target_fps,
        }
    }
}

/// Performs the M0.0 `DocumentCore` bootstrap handshake for Swift and Kotlin callers.
pub fn document_core_handshake() -> Result<DocumentCoreHandshake, String> {
    bootstrap_core_snapshot()
        .map(DocumentCoreHandshake::from)
        .map_err(|err| err.to_string())
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
    }
}

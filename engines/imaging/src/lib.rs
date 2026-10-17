//! Shared Rust `ImagingEngine` for scan cleanup, perspective correction, and deskew (`ARCHITECTURE.md` §21).

/// Supported scan cleanup profile modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanCleanupProfile {
    DocumentAuto,
    PreservePhoto,
}

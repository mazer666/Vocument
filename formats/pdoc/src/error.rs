//! Error types for `.pdoc` package container read, write, and validation operations (`ADR-0020`, `SOURCE-001`).

use thiserror::Error;
use vocument_flow::error::FlowError;

/// Typed errors encountered during `.pdoc` container serialization and deserialization.
#[derive(Debug, Error)]
pub enum PdocError {
    #[error("I/O error during .pdoc operation: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("FlowDocument semantic model error: {0}")]
    Flow(#[from] FlowError),

    #[error("Zip-slip path traversal attempt detected for entry '{path}': {reason}")]
    ZipSlipDetected { path: String, reason: String },

    #[error("Decompression bomb detected: {details}")]
    DecompressionBomb { details: String },

    #[error("Archive exceeds maximum allowed entry count: found {count}, max allowed is {max}")]
    TooManyEntries { count: usize, max: usize },

    #[error(
        "Archive uncompressed size exceeds limit: {bytes} bytes (max allowed: {max_bytes} bytes)"
    )]
    ArchiveTooLarge { bytes: u64, max_bytes: u64 },

    #[error("Required container entry '{entry_name}' is missing from .pdoc archive")]
    MissingRequiredEntry { entry_name: String },

    #[error(
        "Asset integrity check failed for '{asset_path}': expected SHA-256 '{expected_sha256}', computed '{actual_sha256}'"
    )]
    AssetCorrupted {
        asset_path: String,
        expected_sha256: String,
        actual_sha256: String,
    },

    #[error("Invalid asset path '{path}': {reason}")]
    InvalidAssetPath { path: String, reason: String },

    #[error(
        "Unsupported .pdoc format version '{version}' (minimum supported: '{minimum_supported}')"
    )]
    UnsupportedFormatVersion {
        version: String,
        minimum_supported: String,
    },

    #[error("Corrupt .pdoc archive container: {details}")]
    CorruptContainer { details: String },
}

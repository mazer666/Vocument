//! Security validation for `.pdoc` archive extraction, zip-slip defense, and decompression bomb prevention (`ADR-0020`, `SECARCH-001`).

use std::path::{Component, Path, PathBuf};
use vocument_core::config::PackageLimitsConfig;

use crate::error::PdocError;

/// Validates that an archive entry name represents a safe, relative path without traversal attempts.
pub fn validate_entry_path(raw_path: &str) -> Result<PathBuf, PdocError> {
    if raw_path.contains('\0') {
        return Err(PdocError::ZipSlipDetected {
            path: raw_path.to_string(),
            reason: "entry path contains null byte".to_string(),
        });
    }

    if raw_path.starts_with('/') || raw_path.starts_with('\\') {
        return Err(PdocError::ZipSlipDetected {
            path: raw_path.to_string(),
            reason: "entry path has absolute root prefix".to_string(),
        });
    }

    // Windows drive letter check (e.g., "C:...")
    if raw_path.len() >= 2
        && raw_path.as_bytes()[1] == b':'
        && raw_path.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err(PdocError::ZipSlipDetected {
            path: raw_path.to_string(),
            reason: "entry path contains drive letter prefix".to_string(),
        });
    }

    let path = Path::new(raw_path);
    let mut normalized = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                return Err(PdocError::ZipSlipDetected {
                    path: raw_path.to_string(),
                    reason: "entry path contains root or drive component".to_string(),
                });
            }
            Component::ParentDir => {
                return Err(PdocError::ZipSlipDetected {
                    path: raw_path.to_string(),
                    reason: "entry path contains forbidden '..' parent directory traversal"
                        .to_string(),
                });
            }
            Component::CurDir => {
                // Ignore '.' component
            }
            Component::Normal(part) => {
                normalized.push(part);
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(PdocError::ZipSlipDetected {
            path: raw_path.to_string(),
            reason: "entry path normalizes to empty path".to_string(),
        });
    }

    Ok(normalized)
}

/// Evaluates archive safety invariants against central security policy limits (`SECARCH-001`).
pub fn validate_entry_limits(
    entry_index: usize,
    entry_name: &str,
    compressed_size: u64,
    uncompressed_size: u64,
    accumulated_uncompressed_bytes: &mut u64,
    limits: &PackageLimitsConfig,
) -> Result<(), PdocError> {
    if entry_index >= limits.max_zip_entries {
        return Err(PdocError::TooManyEntries {
            count: entry_index + 1,
            max: limits.max_zip_entries,
        });
    }

    let max_bytes = limits.max_pdoc_archive_size_mb.saturating_mul(1024 * 1024);
    *accumulated_uncompressed_bytes =
        accumulated_uncompressed_bytes.saturating_add(uncompressed_size);
    if *accumulated_uncompressed_bytes > max_bytes {
        return Err(PdocError::ArchiveTooLarge {
            bytes: *accumulated_uncompressed_bytes,
            max_bytes,
        });
    }

    // Decompression ratio check: evaluate non-trivial streams
    if compressed_size > 1024 {
        let ratio = uncompressed_size / compressed_size;
        if ratio > limits.max_zip_compression_ratio as u64 {
            return Err(PdocError::DecompressionBomb {
                details: format!(
                    "entry '{}' compression ratio {} exceeds maximum policy threshold {}",
                    entry_name, ratio, limits.max_zip_compression_ratio
                ),
            });
        }
    }

    Ok(())
}

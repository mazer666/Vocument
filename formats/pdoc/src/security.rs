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

/// Normalizes an archive entry path to a canonical, case-folded string for collision detection.
///
/// Prevents ambiguous entries in zip archives where attackers include multiple files
/// differing only by case, redundant `./`, or forward/backward slashes.
pub fn normalize_for_collision_check(raw_path: &str) -> String {
    let clean = raw_path.replace('\\', "/").trim_matches('/').to_string();

    let parts: Vec<&str> = clean
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();

    parts.join("/").to_ascii_lowercase()
}

/// Validates that an archive entry uses an approved compression algorithm.
///
/// Under AGENTS.md and ADR-0020, only standard Stored and Deflated methods are allowed
/// to ensure deterministic cross-platform decoding and prevent complex algorithmic attacks.
pub fn validate_compression_method(
    entry_name: &str,
    method: zip::CompressionMethod,
) -> Result<(), PdocError> {
    match method {
        zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated => Ok(()),
        other => Err(PdocError::UnsupportedCompressionMethod {
            entry: entry_name.to_string(),
            method: format!("{:?}", other),
        }),
    }
}

/// Reads uncompressed bytes from an archive entry stream with strict streaming bounds.
///
/// Defends against malicious archives with forged local headers claiming small uncompressed
/// sizes while streaming out massive decompression bombs.
pub fn read_entry_bounded<R: std::io::Read>(
    mut reader: R,
    entry_name: &str,
    compressed_size: u64,
    declared_uncompressed_size: u64,
    remaining_archive_quota: u64,
    limits: &PackageLimitsConfig,
) -> Result<Vec<u8>, PdocError> {
    let max_entry_bytes = remaining_archive_quota;
    let mut buffer = Vec::with_capacity(declared_uncompressed_size.min(1024 * 1024) as usize);
    let mut chunk = [0u8; 8192];
    let mut total_read: u64 = 0;

    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        total_read = total_read.saturating_add(n as u64);
        if total_read > max_entry_bytes {
            return Err(PdocError::ArchiveTooLarge {
                bytes: total_read,
                max_bytes: limits.max_pdoc_archive_size_mb.saturating_mul(1024 * 1024),
            });
        }

        // Dynamic streaming decompression bomb ratio check
        if compressed_size > 1024 {
            let ratio = total_read / compressed_size;
            if ratio > limits.max_zip_compression_ratio as u64 {
                return Err(PdocError::DecompressionBomb {
                    details: format!(
                        "entry '{}' streaming decompression ratio {} exceeds maximum policy threshold {}",
                        entry_name, ratio, limits.max_zip_compression_ratio
                    ),
                });
            }
        }

        buffer.extend_from_slice(&chunk[..n]);
    }

    Ok(buffer)
}

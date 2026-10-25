//! Embedded binary asset registry, content-addressing, and cryptographic SHA-256 validation (`ADR-0020`, `SOURCE-002`).

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

use crate::error::PdocError;
use crate::manifest::PdocAssetEntry;
use crate::security::validate_entry_path;

/// In-memory asset storage for a `.pdoc` container package.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PdocAssetStore {
    assets: BTreeMap<String, Vec<u8>>,
}

impl PdocAssetStore {
    pub fn new() -> Self {
        Self {
            assets: BTreeMap::new(),
        }
    }

    /// Computes the hexadecimal SHA-256 digest of raw byte content.
    pub fn compute_sha256(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Adds an asset with explicit relative container path (e.g. `assets/logo.png`).
    pub fn insert_with_path(
        &mut self,
        relative_path: &str,
        data: Vec<u8>,
        mime_type: Option<&str>,
    ) -> Result<PdocAssetEntry, PdocError> {
        let normalized = validate_entry_path(relative_path)?;
        let path_str = normalized.to_string_lossy().to_string();

        if !path_str.starts_with("assets/") && !path_str.starts_with("assets\\") {
            return Err(PdocError::InvalidAssetPath {
                path: path_str,
                reason: "asset entry must reside inside the 'assets/' subdirectory".to_string(),
            });
        }

        let byte_size = data.len() as u64;
        let sha256 = Self::compute_sha256(&data);

        self.assets.insert(path_str.clone(), data);

        Ok(PdocAssetEntry {
            path: path_str,
            byte_size,
            sha256,
            mime_type: mime_type.map(str::to_string),
            extra: BTreeMap::new(),
        })
    }

    /// Adds an asset using content-addressable storage (`assets/<sha256>.<extension>`).
    pub fn insert_content_addressed(
        &mut self,
        extension: &str,
        data: Vec<u8>,
        mime_type: Option<&str>,
    ) -> Result<PdocAssetEntry, PdocError> {
        let ext = extension.trim_start_matches('.');
        let sha256 = Self::compute_sha256(&data);
        let path_str = if ext.is_empty() {
            format!("assets/{sha256}")
        } else {
            format!("assets/{sha256}.{ext}")
        };

        let byte_size = data.len() as u64;
        self.assets.insert(path_str.clone(), data);

        Ok(PdocAssetEntry {
            path: path_str,
            byte_size,
            sha256,
            mime_type: mime_type.map(str::to_string),
            extra: BTreeMap::new(),
        })
    }

    /// Retrieves asset bytes by container path.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.assets.get(path).map(|v| v.as_slice())
    }

    /// Verifies that all registered assets match their declared SHA-256 digests.
    pub fn verify_integrity(
        &self,
        entries: &BTreeMap<String, PdocAssetEntry>,
    ) -> Result<(), PdocError> {
        for (path, entry) in entries {
            let data = self
                .assets
                .get(path)
                .ok_or_else(|| PdocError::MissingRequiredEntry {
                    entry_name: path.clone(),
                })?;

            if data.len() as u64 != entry.byte_size {
                return Err(PdocError::AssetCorrupted {
                    asset_path: path.clone(),
                    expected_sha256: format!("{} bytes", entry.byte_size),
                    actual_sha256: format!("{} bytes", data.len()),
                });
            }

            let actual_sha256 = Self::compute_sha256(data);
            if actual_sha256 != entry.sha256 {
                return Err(PdocError::AssetCorrupted {
                    asset_path: path.clone(),
                    expected_sha256: entry.sha256.clone(),
                    actual_sha256,
                });
            }
        }
        Ok(())
    }

    /// Returns the number of assets stored.
    pub fn len(&self) -> usize {
        self.assets.len()
    }

    /// Returns true if no assets are stored.
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    /// Iterates over all stored assets.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.assets.iter()
    }
}

//! Package manifest specification and schema versioning (`manifest.json`, `ADR-0020`, `SOURCE-001`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Standard `.pdoc` specification format version.
pub const CURRENT_FORMAT_VERSION: &str = "1.0";

/// Metadata record for an embedded binary asset stored in the `.pdoc` package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdocAssetEntry {
    /// Relative path within container (e.g. `assets/image_001.png`).
    pub path: String,

    /// Size of the uncompressed asset in bytes.
    pub byte_size: u64,

    /// Cryptographic SHA-256 hex digest of the raw asset bytes.
    pub sha256: String,

    /// Optional MIME type (e.g. `image/png`, `font/ttf`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Unknown / future asset attributes preserved across roundtrips.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Root package manifest (`manifest.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdocManifest {
    /// Specification version of the container format (e.g. "1.0").
    pub format_version: String,

    /// Stable unique document identifier.
    pub document_id: String,

    /// Application/tool name and build revision generating this package.
    pub generator: String,

    /// ISO-8601 creation timestamp.
    pub created_at: String,

    /// ISO-8601 last modification timestamp.
    pub modified_at: String,

    /// Embedded assets registry keyed by relative container path.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub assets: BTreeMap<String, PdocAssetEntry>,

    /// Unknown / forward-compatible top-level fields preserved across roundtrips.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl PdocManifest {
    /// Constructs a standard manifest for a document identifier with current versioning.
    pub fn new(document_id: impl Into<String>, generator: impl Into<String>) -> Self {
        let now = chrono_or_fallback_timestamp();
        Self {
            format_version: CURRENT_FORMAT_VERSION.to_string(),
            document_id: document_id.into(),
            generator: generator.into(),
            created_at: now.clone(),
            modified_at: now,
            assets: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }

    /// Records an asset entry in the manifest.
    pub fn register_asset(&mut self, entry: PdocAssetEntry) {
        self.assets.insert(entry.path.clone(), entry);
    }

    /// Validates manifest schema constraints and version compatibility.
    pub fn validate(&self) -> Result<(), crate::error::PdocError> {
        if self.document_id.trim().is_empty() {
            return Err(crate::error::PdocError::CorruptContainer {
                details: "manifest document_id cannot be empty".to_string(),
            });
        }

        // Check format version compatibility: major version must match CURRENT_FORMAT_VERSION major
        let current_major = CURRENT_FORMAT_VERSION
            .split('.')
            .next()
            .unwrap_or(CURRENT_FORMAT_VERSION);
        let manifest_major = self
            .format_version
            .split('.')
            .next()
            .unwrap_or(&self.format_version);

        if manifest_major != current_major {
            return Err(crate::error::PdocError::IncompatibleMajorVersion {
                version: self.format_version.clone(),
                supported_major: current_major.to_string(),
            });
        }

        for (path, asset) in &self.assets {
            if !path.starts_with("assets/") && !path.starts_with("assets\\") {
                return Err(crate::error::PdocError::InvalidAssetPath {
                    path: path.clone(),
                    reason: "manifest asset entry must reside in 'assets/' directory".to_string(),
                });
            }
            if asset.sha256.len() != 64 || !asset.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(crate::error::PdocError::CorruptContainer {
                    details: format!(
                        "asset '{path}' has invalid SHA-256 hex digest: '{}'",
                        asset.sha256
                    ),
                });
            }
        }

        Ok(())
    }
}

fn chrono_or_fallback_timestamp() -> String {
    // Generate deterministic ISO timestamp or RFC-3339 without mandatory heavy external time crate
    if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let secs = duration.as_secs();
        format!("unix:{}", secs)
    } else {
        "1970-01-01T00:00:00Z".to_string()
    }
}

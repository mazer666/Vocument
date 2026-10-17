//! Controlled `FileBroker` boundary (`ARCHITECTURE.md` §15, `WHERE_TO_CHANGE.md`).
//!
//! Native platform code (macOS App Sandbox / `NSDocument`, Android Storage Access Framework)
//! owns OS file permissions and supplies scoped descriptors/bytes through `FileBroker`.
//! Raw unrestricted filesystem paths are not the public cross-platform API.

use crate::identity::FileIdentity;
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Origin mechanism through which the user granted access to a file (`FILEINT-001`, `ARCHITECTURE.md` §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAccessOrigin {
    /// macOS Finder / Open With / `NSDocument` security-scoped file URL.
    MacOsScopedUrl,
    /// Android Storage Access Framework (`content://`) persisted or transient URI.
    AndroidSafUri,
    /// Headless test or internal fixture buffer.
    InMemoryBuffer,
}

/// Platform-provided file descriptor + content buffer passed across `FileBroker`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokeredFileRead {
    pub source_token: String,
    pub display_name: String,
    pub access_origin: FileAccessOrigin,
    pub os_file_id: Option<String>,
    pub modified_unix_ms: Option<i64>,
    pub bytes: Vec<u8>,
}

impl BrokeredFileRead {
    /// Computes the `FileIdentity` for this brokered file payload.
    pub fn compute_identity(&self) -> FileIdentity {
        FileIdentity::compute(
            &self.source_token,
            &self.display_name,
            self.os_file_id.clone(),
            self.modified_unix_ms,
            &self.bytes,
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FileBrokerError {
    #[error("permission denied by platform FileBroker for '{source_token}'")]
    PermissionDenied { source_token: String },
    #[error("file not found or inaccessible: '{source_token}' ({details})")]
    NotFound {
        source_token: String,
        details: String,
    },
    #[error("empty or invalid file payload from '{source_token}'")]
    EmptyPayload { source_token: String },
    #[error("I/O error while reading '{source_token}': {details}")]
    IoError {
        source_token: String,
        details: String,
    },
}

/// Controlled file-access boundary (`ARCHITECTURE.md` §15).
pub trait FileBroker: Send + Sync {
    /// Reads a user-approved file identified by `source_token`.
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError>;
}

/// Scoped local `FileBroker` implementation used by macOS scoped file URLs and headless tests.
#[derive(Debug, Default, Clone)]
pub struct ScopedLocalFileBroker;

impl ScopedLocalFileBroker {
    pub fn new() -> Self {
        Self
    }

    /// Reads a local path that was explicitly granted by the user via Finder / Open With (`MacOsScopedUrl`).
    pub fn read_macos_scoped_path(path: &Path) -> Result<BrokeredFileRead, FileBrokerError> {
        let source_token = path.to_string_lossy().into_owned();
        let metadata = std::fs::metadata(path).map_err(|err| FileBrokerError::NotFound {
            source_token: source_token.clone(),
            details: err.to_string(),
        })?;
        let bytes = std::fs::read(path).map_err(|err| FileBrokerError::IoError {
            source_token: source_token.clone(),
            details: err.to_string(),
        })?;
        if bytes.is_empty() {
            return Err(FileBrokerError::EmptyPayload { source_token });
        }

        let display_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| source_token.clone());

        let modified_unix_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| i64::try_from(d.as_millis()).ok());

        #[cfg(unix)]
        let os_file_id = {
            use std::os::unix::fs::MetadataExt;
            Some(format!("{}:{}", metadata.dev(), metadata.ino()))
        };
        #[cfg(not(unix))]
        let os_file_id = None;

        Ok(BrokeredFileRead {
            source_token,
            display_name,
            access_origin: FileAccessOrigin::MacOsScopedUrl,
            os_file_id,
            modified_unix_ms,
            bytes,
        })
    }
}

impl FileBroker for ScopedLocalFileBroker {
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError> {
        let normalized = source_token.strip_prefix("file://").unwrap_or(source_token);
        Self::read_macos_scoped_path(Path::new(normalized))
    }
}

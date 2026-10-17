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
    #[error("staging write failed for '{target_token}': {details}")]
    StagingFailed {
        target_token: String,
        details: String,
    },
    #[error("commit staged write failed for '{target_token}': {details}")]
    CommitFailed {
        target_token: String,
        details: String,
    },
    #[error("I/O error while reading '{source_token}': {details}")]
    IoError {
        source_token: String,
        details: String,
    },
}

/// Platform-provided staging handle for safe atomic writes (`ARCHITECTURE.md` §16, `SAVEATOMIC-001`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokeredStagedWrite {
    pub staging_token: String,
    pub target_token: String,
    pub staging_path: std::path::PathBuf,
    pub target_path: std::path::PathBuf,
    pub bytes_written: u64,
}

/// Controlled file-access boundary (`ARCHITECTURE.md` §15).
pub trait FileBroker: Send + Sync + std::fmt::Debug {
    /// Reads a user-approved file identified by `source_token`.
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError>;

    /// Inspects the current on-disk `FileIdentity` for `source_token`, or `Ok(None)` if it does not exist.
    fn inspect_file_identity(
        &self,
        source_token: &str,
    ) -> Result<Option<FileIdentity>, FileBrokerError>;

    /// Stages bytes safely into temporary output without touching `target_token`.
    fn stage_write(
        &self,
        target_token: &str,
        bytes: &[u8],
    ) -> Result<BrokeredStagedWrite, FileBrokerError>;

    /// Commits a previously staged write by atomically replacing `target_token` with the staged output.
    fn commit_staged_write(
        &self,
        staged: BrokeredStagedWrite,
    ) -> Result<FileIdentity, FileBrokerError>;

    /// Discards a staged write, removing any temporary staged artifacts.
    fn discard_staged_write(&self, staged: BrokeredStagedWrite) -> Result<(), FileBrokerError>;
}

/// Scoped local `FileBroker` implementation used by macOS scoped file URLs and headless tests.
#[derive(Debug, Default, Clone)]
pub struct ScopedLocalFileBroker;

static STAGING_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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

    fn inspect_file_identity(
        &self,
        source_token: &str,
    ) -> Result<Option<FileIdentity>, FileBrokerError> {
        let normalized = source_token.strip_prefix("file://").unwrap_or(source_token);
        let path = Path::new(normalized);
        if !path.exists() {
            return Ok(None);
        }
        let read = Self::read_macos_scoped_path(path)?;
        Ok(Some(read.compute_identity()))
    }

    fn stage_write(
        &self,
        target_token: &str,
        bytes: &[u8],
    ) -> Result<BrokeredStagedWrite, FileBrokerError> {
        use std::io::Write as _;
        let normalized = target_token.strip_prefix("file://").unwrap_or(target_token);
        let target_path = std::path::PathBuf::from(normalized);
        let parent_dir = target_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        if !parent_dir.exists() {
            std::fs::create_dir_all(parent_dir).map_err(|err| FileBrokerError::StagingFailed {
                target_token: target_token.to_owned(),
                details: format!("failed to create parent directory: {err}"),
            })?;
        }

        let counter = STAGING_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        let stem = target_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".to_owned());
        let staging_name = format!(".{stem}.vocument_staging_{pid}_{counter}.tmp");
        let staging_path = parent_dir.join(staging_name);

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging_path)
            .map_err(|err| FileBrokerError::StagingFailed {
                target_token: target_token.to_owned(),
                details: format!("failed to open staging file '{staging_path:?}': {err}"),
            })?;

        file.write_all(bytes)
            .map_err(|err| FileBrokerError::StagingFailed {
                target_token: target_token.to_owned(),
                details: format!("failed to write staged bytes: {err}"),
            })?;

        file.sync_all()
            .map_err(|err| FileBrokerError::StagingFailed {
                target_token: target_token.to_owned(),
                details: format!("fsync failed on staged file: {err}"),
            })?;

        Ok(BrokeredStagedWrite {
            staging_token: staging_path.to_string_lossy().into_owned(),
            target_token: target_token.to_owned(),
            staging_path,
            target_path,
            bytes_written: bytes.len() as u64,
        })
    }

    fn commit_staged_write(
        &self,
        staged: BrokeredStagedWrite,
    ) -> Result<FileIdentity, FileBrokerError> {
        std::fs::rename(&staged.staging_path, &staged.target_path).map_err(|err| {
            FileBrokerError::CommitFailed {
                target_token: staged.target_token.clone(),
                details: format!(
                    "atomic rename from '{:?}' to '{:?}' failed: {err}",
                    staged.staging_path, staged.target_path
                ),
            }
        })?;

        let read = Self::read_macos_scoped_path(&staged.target_path)?;
        Ok(read.compute_identity())
    }

    fn discard_staged_write(&self, staged: BrokeredStagedWrite) -> Result<(), FileBrokerError> {
        if staged.staging_path.exists() {
            let _ = std::fs::remove_file(&staged.staging_path);
        }
        Ok(())
    }
}

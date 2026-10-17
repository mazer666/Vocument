//! File identity, fingerprinting, and external-change comparison (`ARCHITECTURE.md` §14, `HASH-001`, `FILELOCK-002`).
//!
//! Never relies on path alone. Uses OS file identity/inode (when available), byte size,
//! modification timestamp, SHA-256 content digest, and source descriptor metadata to distinguish:
//! - identical unmodified file
//! - same file moved to a new path/URI
//! - probable copy of the same source at another location
//! - externally modified file at the same path/URI
//! - different file replacing the same path/URI

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable internal document identifier derived from content + identity (`ARCHITECTURE.md` §6, §13).
/// Opaque string (`doc_<first-16-hex-of-sha256>`) that never leaks plaintext document filenames.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DocumentId(String);

impl DocumentId {
    pub fn from_sha256_hex(sha256_hex: &str) -> Self {
        let prefix: String = sha256_hex.chars().take(16).collect();
        Self(format!("doc_{prefix}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable internal session identifier assigned per opened `DocumentSession` (`ARCHITECTURE.md` §6).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new(counter: u64, document_id: &DocumentId) -> Self {
        Self(format!("sess_{counter:04}_{}", document_id.as_str()))
    }

    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Multi-signal file identity (`ARCHITECTURE.md` §14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileIdentity {
    /// Platform-controlled source URI or scoped path token (`FileBroker`).
    pub source_token: String,
    /// Human-readable display name (e.g. `"contract.pdf"`).
    pub display_name: String,
    /// Optional OS-level file identifier (e.g. POSIX `dev:ino` or SAF document ID).
    pub os_file_id: Option<String>,
    /// Exact size in bytes.
    pub size_bytes: u64,
    /// Last modification timestamp in Unix epoch milliseconds, if provided by the OS/SAF.
    pub modified_unix_ms: Option<i64>,
    /// Lowercase hex SHA-256 digest of the file bytes (`HASH-001`).
    pub sha256_hex: String,
}

/// Relationship between a previously recorded `FileIdentity` and a newly observed `FileIdentity` (`ARCHITECTURE.md` §14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileIdentityComparison {
    /// Same path/token and identical content digest.
    Unchanged,
    /// Different path/token, same `os_file_id` (inode/document ID) and identical content digest.
    SameFileMoved,
    /// Different path/token (and different or unknown `os_file_id`), but identical content digest.
    ProbableCopyOfSameSource,
    /// Same `os_file_id` (or same path/token with matching OS identity), but content digest changed.
    ModifiedExternally,
    /// Same path/token, but replaced by a different `os_file_id` and different content digest.
    DifferentFileReplacingSamePath,
    /// Unrelated files (different path, different OS ID, different content).
    Unrelated,
}

impl FileIdentity {
    /// Computes a `FileIdentity` from raw bytes and platform descriptor metadata.
    pub fn compute(
        source_token: impl Into<String>,
        display_name: impl Into<String>,
        os_file_id: Option<String>,
        modified_unix_ms: Option<i64>,
        bytes: &[u8],
    ) -> Self {
        let sha256_hex = compute_sha256_hex(bytes);
        Self {
            source_token: source_token.into(),
            display_name: display_name.into(),
            os_file_id,
            size_bytes: bytes.len() as u64,
            modified_unix_ms,
            sha256_hex,
        }
    }

    /// Derives the stable internal `DocumentId` for this file identity.
    pub fn stable_document_id(&self) -> DocumentId {
        DocumentId::from_sha256_hex(&self.sha256_hex)
    }

    /// Compares this baseline `FileIdentity` against a newly observed `FileIdentity` (`ARCHITECTURE.md` §14).
    pub fn compare_with(&self, observed: &Self) -> FileIdentityComparison {
        let same_content =
            self.sha256_hex == observed.sha256_hex && self.size_bytes == observed.size_bytes;
        let same_source_token = self.source_token == observed.source_token;
        let same_os_id = match (&self.os_file_id, &observed.os_file_id) {
            (Some(a), Some(b)) => Some(a == b),
            _ => None,
        };

        if same_content {
            if same_source_token {
                return FileIdentityComparison::Unchanged;
            }
            if same_os_id == Some(true) {
                return FileIdentityComparison::SameFileMoved;
            }
            return FileIdentityComparison::ProbableCopyOfSameSource;
        }

        // Content differs:
        if same_source_token {
            if same_os_id == Some(false) {
                return FileIdentityComparison::DifferentFileReplacingSamePath;
            }
            return FileIdentityComparison::ModifiedExternally;
        }

        if same_os_id == Some(true) {
            return FileIdentityComparison::ModifiedExternally;
        }

        FileIdentityComparison::Unrelated
    }
}

/// Computes lowercase hexadecimal SHA-256 digest (`HASH-001`).
pub fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_moved_copied_modified_and_replaced_files() {
        let original = FileIdentity::compute(
            "file:///Users/test/a.pdf",
            "a.pdf",
            Some("dev1:ino100".to_owned()),
            Some(1_700_000_000_000),
            b"%PDF-1.7 original",
        );

        let unchanged = original.clone();
        assert_eq!(
            original.compare_with(&unchanged),
            FileIdentityComparison::Unchanged
        );

        let moved = FileIdentity::compute(
            "file:///Users/test/archive/a_renamed.pdf",
            "a_renamed.pdf",
            Some("dev1:ino100".to_owned()),
            Some(1_700_000_000_000),
            b"%PDF-1.7 original",
        );
        assert_eq!(
            original.compare_with(&moved),
            FileIdentityComparison::SameFileMoved
        );

        let copied = FileIdentity::compute(
            "file:///Users/test/copy.pdf",
            "copy.pdf",
            Some("dev1:ino200".to_owned()),
            Some(1_700_000_001_000),
            b"%PDF-1.7 original",
        );
        assert_eq!(
            original.compare_with(&copied),
            FileIdentityComparison::ProbableCopyOfSameSource
        );

        let modified_in_place = FileIdentity::compute(
            "file:///Users/test/a.pdf",
            "a.pdf",
            Some("dev1:ino100".to_owned()),
            Some(1_700_000_005_000),
            b"%PDF-1.7 edited in place",
        );
        assert_eq!(
            original.compare_with(&modified_in_place),
            FileIdentityComparison::ModifiedExternally
        );

        let replaced_at_same_path = FileIdentity::compute(
            "file:///Users/test/a.pdf",
            "a.pdf",
            Some("dev1:ino999".to_owned()),
            Some(1_700_000_009_000),
            b"%PDF-1.7 completely different file",
        );
        assert_eq!(
            original.compare_with(&replaced_at_same_path),
            FileIdentityComparison::DifferentFileReplacingSamePath
        );
    }
}

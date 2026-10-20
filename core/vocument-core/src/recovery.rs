//! Per-document crash recovery subsystem (`ROADMAP.md` M0.6, `ARCHITECTURE.md` §§11–12, `REQUIREMENTS.md` §34).
//!
//! Implements:
//! - Per-document recovery container with `manifest.json`, `journal.json`, and `checkpoints/`
//! - Crash detection via process PID and `is_clean_shutdown` marker
//! - Recovery actions: Restore, Preview, Discard, and Open as New (`RECOVERY-001` Preset)
//! - Gate invariant: **Original file on disk remains completely intact until explicit user save.**

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::file_broker::FileBroker;
use crate::identity::{FileIdentity, FileIdentityComparison, compute_sha256_hex};
use crate::save::generate_collision_free_path;
use crate::session::{DocumentSessionSnapshot, SessionError, SessionRegistry};
use crate::transactions::{PersistentTransactionJournal, TransactionError};

/// Schema version for recovery manifests and containers.
pub const RECOVERY_SCHEMA_VERSION: u32 = 1;

/// Default name of the recovery storage directory inside App Support / temp storage.
pub const RECOVERY_DIR_NAME: &str = "vocument_recovery";

/// Error type for the recovery subsystem.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RecoveryError {
    #[error("I/O error in recovery container: {details}")]
    Io { details: String },
    #[error("JSON serialization error in recovery container: {details}")]
    Json { details: String },
    #[error("Recovery container integrity mismatch for document '{document_id}': {details}")]
    IntegrityMismatch {
        document_id: String,
        details: String,
    },
    #[error("No recovery container found for document '{document_id}'")]
    NotFound { document_id: String },
    #[error("Transaction error during recovery replay: {details}")]
    Transaction { details: String },
    #[error("Session registry error during recovery: {details}")]
    Session { details: String },
    #[error("Recovery restore for document '{document_id}' produced no session snapshot")]
    MissingSessionSnapshot { document_id: String },
}

impl From<std::io::Error> for RecoveryError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            details: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for RecoveryError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json {
            details: err.to_string(),
        }
    }
}

impl From<TransactionError> for RecoveryError {
    fn from(err: TransactionError) -> Self {
        Self::Transaction {
            details: err.to_string(),
        }
    }
}

/// Recovery action selected by the user when opening a document with crash recovery (`RECOVERY-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Restores the recovered transaction journal into the active session without touching the disk file.
    Restore,
    /// Opens an isolated read-only preview of the recovered document state.
    Preview,
    /// Discards the recovery container and opens the pristine on-disk document.
    Discard,
    /// Opens the recovered state as a new document with a collision-free file path, leaving original intact.
    OpenAsNew,
}

/// Outcome of executing a `RecoveryAction`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryActionResult {
    pub document_id: String,
    pub action: RecoveryAction,
    pub session_snapshot: Option<DocumentSessionSnapshot>,
    pub target_token: Option<String>,
    pub message: String,
}

/// Information describing a discovered recoverable document session (`ARCHITECTURE.md` §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableDocumentInfo {
    pub document_id: String,
    pub source_token: String,
    pub display_name: String,
    pub uncommitted_revisions: u64,
    pub last_updated_unix_ms: u64,
    pub has_checkpoint: bool,
    pub has_journal: bool,
    pub original_file_exists: bool,
    pub original_modified_externally: bool,
}

/// Versioned metadata manifest for a single document recovery container (`ARCHITECTURE.md` §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryManifest {
    pub schema_version: u32,
    pub document_id: String,
    pub source_token: String,
    pub display_name: String,
    pub original_file_identity: FileIdentity,
    pub active_pid: u32,
    pub is_clean_shutdown: bool,
    pub has_unsaved_changes: bool,
    pub uncommitted_revisions: u64,
    pub last_saved_revision: u64,
    pub created_unix_ms: u64,
    pub updated_unix_ms: u64,
    pub last_checkpoint_file: Option<String>,
    pub last_checkpoint_sha256: Option<String>,
    pub manifest_checksum_sha256: String,
}

impl RecoveryManifest {
    #[allow(clippy::too_many_arguments)]
    pub fn compute_checksum(
        schema_version: u32,
        document_id: &str,
        source_token: &str,
        active_pid: u32,
        is_clean_shutdown: bool,
        has_unsaved_changes: bool,
        uncommitted_revisions: u64,
        updated_unix_ms: u64,
        last_checkpoint_sha256: Option<&str>,
    ) -> String {
        let material = format!(
            "v={schema_version}|doc={document_id}|src={source_token}|pid={active_pid}|clean={is_clean_shutdown}|unsaved={has_unsaved_changes}|rev={uncommitted_revisions}|t={updated_unix_ms}|chk={}",
            last_checkpoint_sha256.unwrap_or("none")
        );
        compute_sha256_hex(material.as_bytes())
    }

    pub fn verify_checksum(&self) -> bool {
        let expected = Self::compute_checksum(
            self.schema_version,
            &self.document_id,
            &self.source_token,
            self.active_pid,
            self.is_clean_shutdown,
            self.has_unsaved_changes,
            self.uncommitted_revisions,
            self.updated_unix_ms,
            self.last_checkpoint_sha256.as_deref(),
        );
        expected == self.manifest_checksum_sha256
    }
}

/// Manages a single document's isolated recovery container directory (`ARCHITECTURE.md` §12).
#[derive(Debug, Clone)]
pub struct RecoveryContainer {
    container_dir: PathBuf,
    document_id: String,
}

impl RecoveryContainer {
    pub fn new(root_dir: &Path, document_id: &str) -> Self {
        Self {
            container_dir: root_dir.join(document_id),
            document_id: document_id.to_owned(),
        }
    }

    pub fn container_dir(&self) -> &Path {
        &self.container_dir
    }

    pub fn exists(&self) -> bool {
        self.container_dir.exists()
    }

    fn manifest_path(&self) -> PathBuf {
        self.container_dir.join("manifest.json")
    }

    fn journal_path(&self) -> PathBuf {
        self.container_dir.join("journal.json")
    }

    /// Creates the container directory with restrictive permissions.
    ///
    /// The container holds the name and the path of the document of the user.
    /// It must not be readable for anyone else on the machine.
    fn ensure_container_dir(&self) -> Result<(), RecoveryError> {
        fs::create_dir_all(&self.container_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&self.container_dir)?.permissions();
            if permissions.mode() & 0o077 != 0 {
                permissions.set_mode(0o700);
                fs::set_permissions(&self.container_dir, permissions)?;
            }
        }
        Ok(())
    }

    /// Creates a file that only the owner may read.
    ///
    /// The recovery container holds the name and the path of the document of the
    /// user. The files must not be readable for anyone else on the machine.
    fn create_private_file(path: &Path) -> std::io::Result<File> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(path)
    }

    fn checkpoints_dir(&self) -> PathBuf {
        self.container_dir.join("checkpoints")
    }

    /// Atomically writes the recovery manifest to disk via sibling staging and rename.
    pub fn write_manifest(&self, manifest: &RecoveryManifest) -> Result<(), RecoveryError> {
        self.ensure_container_dir()?;
        let json = serde_json::to_string_pretty(manifest)?;
        let temp_path = self.container_dir.join(format!(
            ".tmp_manifest_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let mut file = Self::create_private_file(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        fs::rename(temp_path, self.manifest_path())?;
        Ok(())
    }

    /// Reads and integrity-verifies the recovery manifest from disk.
    pub fn read_manifest(&self) -> Result<Option<RecoveryManifest>, RecoveryError> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(None);
        }
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let manifest: RecoveryManifest = serde_json::from_str(&content)?;
        if !manifest.verify_checksum() {
            return Err(RecoveryError::IntegrityMismatch {
                document_id: self.document_id.clone(),
                details: "manifest checksum mismatch".to_owned(),
            });
        }
        Ok(Some(manifest))
    }

    /// Atomically writes the transaction journal to disk.
    pub fn write_journal(
        &self,
        journal: &PersistentTransactionJournal,
    ) -> Result<(), RecoveryError> {
        self.ensure_container_dir()?;
        let json = journal.to_json_pretty()?;
        let temp_path = self.container_dir.join(format!(
            ".tmp_journal_{}_{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let mut file = Self::create_private_file(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        fs::rename(temp_path, self.journal_path())?;
        Ok(())
    }

    /// Reads and integrity-verifies the persistent transaction journal from disk.
    pub fn read_journal(&self) -> Result<Option<PersistentTransactionJournal>, RecoveryError> {
        let path = self.journal_path();
        if !path.exists() {
            return Ok(None);
        }
        let mut file = File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        let journal = PersistentTransactionJournal::from_json_verified(&content)?;
        Ok(Some(journal))
    }

    /// Atomically writes a periodic full document checkpoint PDF to `checkpoints/`.
    pub fn write_checkpoint(
        &self,
        revision: u64,
        pdf_bytes: &[u8],
    ) -> Result<(String, String), RecoveryError> {
        let dir = self.checkpoints_dir();
        fs::create_dir_all(&dir)?;
        let filename = format!("checkpoint_rev_{revision}.pdf");
        let dest = dir.join(&filename);
        let temp = dir.join(format!(".tmp_{filename}"));

        let mut file = File::create(&temp)?;
        file.write_all(pdf_bytes)?;
        file.sync_all()?;
        fs::rename(temp, dest)?;

        let hash = compute_sha256_hex(pdf_bytes);
        Ok((filename, hash))
    }

    /// Reads raw checkpoint bytes from disk.
    pub fn read_checkpoint(&self, filename: &str) -> Result<Vec<u8>, RecoveryError> {
        let path = self.checkpoints_dir().join(filename);
        let mut file = File::open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Marks the container as a clean shutdown (e.g. after clean document close or save).
    pub fn mark_clean_shutdown(&self) -> Result<(), RecoveryError> {
        if let Some(mut manifest) = self.read_manifest()? {
            manifest.is_clean_shutdown = true;
            manifest.has_unsaved_changes = false;
            manifest.updated_unix_ms = current_unix_ms();
            manifest.manifest_checksum_sha256 = RecoveryManifest::compute_checksum(
                manifest.schema_version,
                &manifest.document_id,
                &manifest.source_token,
                manifest.active_pid,
                manifest.is_clean_shutdown,
                manifest.has_unsaved_changes,
                manifest.uncommitted_revisions,
                manifest.updated_unix_ms,
                manifest.last_checkpoint_sha256.as_deref(),
            );
            self.write_manifest(&manifest)?;
        }
        Ok(())
    }

    /// Deletes the checkpoints of this document.
    ///
    /// A checkpoint holds the bytes of an earlier state of the document. After
    /// a redaction those bytes still contain the removed content, so the
    /// checkpoints have to go with the redaction (`REDACT-001`).
    pub fn purge_checkpoints(&self) -> Result<(), RecoveryError> {
        let directory = self.checkpoints_dir();
        if directory.exists() {
            fs::remove_dir_all(&directory)?;
        }
        // The manifest points at the last checkpoint. It must not point at a
        // file that no longer exists.
        if let Some(mut manifest) = self.read_manifest()? {
            let has_checkpoint = manifest.last_checkpoint_file.is_some()
                || manifest.last_checkpoint_sha256.is_some();
            if has_checkpoint {
                manifest.last_checkpoint_file = None;
                manifest.last_checkpoint_sha256 = None;
                self.write_manifest(&manifest)?;
            }
        }
        Ok(())
    }

    /// Removes this document's recovery container from disk.
    pub fn purge(&self) -> Result<(), RecoveryError> {
        if self.container_dir.exists() {
            fs::remove_dir_all(&self.container_dir)?;
        }
        Ok(())
    }
}

/// Central store managing per-document recovery containers (`ARCHITECTURE.md` §12).
#[derive(Debug)]
pub struct RecoveryStore {
    root_dir: PathBuf,
}

impl RecoveryStore {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn default_app_store() -> Self {
        let dir = std::env::temp_dir().join(RECOVERY_DIR_NAME);
        Self::new(dir)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn container_for(&self, document_id: impl AsRef<str>) -> RecoveryContainer {
        RecoveryContainer::new(&self.root_dir, document_id.as_ref())
    }

    /// Scans the recovery store and returns information on all unrecovered crashed sessions.
    pub fn scan_recoverable_documents(
        &self,
        broker: &dyn FileBroker,
    ) -> Vec<RecoverableDocumentInfo> {
        let mut results = Vec::new();
        let entries = match fs::read_dir(&self.root_dir) {
            Ok(e) => e,
            Err(_) => return results,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let doc_id = match path.file_name().and_then(|n| n.to_str()) {
                Some(id) => id,
                None => continue,
            };
            let container = self.container_for(doc_id);
            let manifest = match container.read_manifest() {
                Ok(Some(m)) if !m.is_clean_shutdown && m.has_unsaved_changes => m,
                _ => continue,
            };
            let external_identity = broker
                .inspect_file_identity(&manifest.source_token)
                .ok()
                .flatten();
            let exists = external_identity.is_some();
            let ext_modified = match external_identity {
                Some(ext) => {
                    let cmp = manifest.original_file_identity.compare_with(&ext);
                    matches!(
                        cmp,
                        FileIdentityComparison::ModifiedExternally
                            | FileIdentityComparison::DifferentFileReplacingSamePath
                    )
                }
                None => false,
            };
            results.push(RecoverableDocumentInfo {
                document_id: manifest.document_id,
                source_token: manifest.source_token,
                display_name: manifest.display_name,
                uncommitted_revisions: manifest.uncommitted_revisions,
                last_updated_unix_ms: manifest.updated_unix_ms,
                has_checkpoint: manifest.last_checkpoint_file.is_some(),
                has_journal: container.journal_path().exists(),
                original_file_exists: exists,
                original_modified_externally: ext_modified,
            });
        }
        results
    }

    /// Inspects whether a specific `source_token` has a pending crash recovery container.
    pub fn check_recovery_for_source_token(
        &self,
        source_token: &str,
        broker: &dyn FileBroker,
    ) -> Option<RecoverableDocumentInfo> {
        let all = self.scan_recoverable_documents(broker);
        all.into_iter().find(|d| d.source_token == source_token)
    }
}

/// Helper function to return current time in milliseconds since UNIX epoch.
pub fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Recovery execution methods on `SessionRegistry` (`ROADMAP.md` M0.6).
impl SessionRegistry {
    /// Dispatches a `RecoveryAction` (`Restore`, `Preview`, `Discard`, `OpenAsNew`) on `document_id`.
    pub fn execute_recovery_action(
        &self,
        document_id: &str,
        action: RecoveryAction,
    ) -> Result<RecoveryActionResult, SessionError> {
        let container = self.recovery_store.container_for(document_id);
        let manifest = container
            .read_manifest()
            .map_err(SessionError::Recovery)?
            .ok_or_else(|| {
                SessionError::Recovery(RecoveryError::NotFound {
                    document_id: document_id.to_owned(),
                })
            })?;

        match action {
            RecoveryAction::Restore => self.execute_recovery_restore(&container, manifest),
            RecoveryAction::Preview => self.execute_recovery_preview(&container, manifest),
            RecoveryAction::Discard => {
                container.purge().map_err(SessionError::Recovery)?;
                Ok(RecoveryActionResult {
                    document_id: document_id.to_owned(),
                    action: RecoveryAction::Discard,
                    session_snapshot: None,
                    target_token: None,
                    message: "Recovery container discarded successfully.".to_owned(),
                })
            }
            RecoveryAction::OpenAsNew => self.execute_recovery_open_as_new(&container, manifest),
        }
    }

    fn execute_recovery_restore(
        &self,
        container: &RecoveryContainer,
        manifest: RecoveryManifest,
    ) -> Result<RecoveryActionResult, SessionError> {
        let journal = container
            .read_journal()
            .map_err(SessionError::Recovery)?
            .ok_or_else(|| {
                SessionError::Recovery(RecoveryError::NotFound {
                    document_id: manifest.document_id.clone(),
                })
            })?;

        // Read baseline file via broker without modifying it:
        let brokered = self.file_broker.read_file(&manifest.source_token)?;
        let snap = self.open_brokered_file(brokered)?;

        // Replay transactions from the recovery journal onto the live session:
        let restored_snap = {
            let mut guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard.get_mut(snap.session_id.as_str()).ok_or_else(|| {
                SessionError::SessionNotFound {
                    session_id: snap.session_id.to_string(),
                }
            })?;
            let actor = session.actor();
            session
                .transaction_manager
                .restore_and_replay_journal(&actor, &journal)?;
            session.sync_from_transaction_manager();
            session.snapshot()
        };

        Ok(RecoveryActionResult {
            document_id: manifest.document_id,
            action: RecoveryAction::Restore,
            session_snapshot: Some(restored_snap),
            target_token: Some(manifest.source_token),
            message: "Document session restored from crash recovery journal. Original file on disk remains unchanged until save.".to_owned(),
        })
    }

    fn execute_recovery_preview(
        &self,
        container: &RecoveryContainer,
        manifest: RecoveryManifest,
    ) -> Result<RecoveryActionResult, SessionError> {
        let journal = container
            .read_journal()
            .map_err(SessionError::Recovery)?
            .ok_or_else(|| {
                SessionError::Recovery(RecoveryError::NotFound {
                    document_id: manifest.document_id.clone(),
                })
            })?;

        // Open read-only preview session:
        let brokered = self.file_broker.read_file(&manifest.source_token)?;
        let snap = self.open_brokered_file(brokered)?;

        let preview_snap = {
            let mut guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard.get_mut(snap.session_id.as_str()).ok_or_else(|| {
                SessionError::SessionNotFound {
                    session_id: snap.session_id.to_string(),
                }
            })?;
            let actor = session.actor();
            session
                .transaction_manager
                .restore_and_replay_journal(&actor, &journal)?;
            session.sync_from_transaction_manager();
            session.snapshot()
        };

        Ok(RecoveryActionResult {
            document_id: manifest.document_id,
            action: RecoveryAction::Preview,
            session_snapshot: Some(preview_snap),
            target_token: Some(manifest.source_token),
            message: "Preview session created. Recovery container and on-disk file are unchanged."
                .to_owned(),
        })
    }

    fn execute_recovery_open_as_new(
        &self,
        container: &RecoveryContainer,
        manifest: RecoveryManifest,
    ) -> Result<RecoveryActionResult, SessionError> {
        // First restore state into session:
        let restored = self.execute_recovery_restore(container, manifest.clone())?;
        let restored_snap = restored.session_snapshot.ok_or_else(|| {
            SessionError::Recovery(RecoveryError::MissingSessionSnapshot {
                document_id: manifest.document_id.clone(),
            })
        })?;

        // Generate collision-free path for recovered file:
        let normalized = manifest
            .source_token
            .strip_prefix("file://")
            .unwrap_or(&manifest.source_token);
        let new_path = generate_collision_free_path(Path::new(normalized), "Recovered");
        let new_token = new_path.to_string_lossy().into_owned();

        // Safe save as new document:
        let save_res = self.save_session_as(restored_snap.session_id.as_str(), &new_token)?;
        let final_snap = match save_res {
            crate::save::SaveResult::Success(succ) => {
                self.session_snapshot(succ.session_id.as_str())?
            }
            crate::save::SaveResult::Conflict(_) => {
                return Err(SessionError::Recovery(RecoveryError::Session {
                    details: "conflict saving recovered copy as new document".to_owned(),
                }));
            }
        };

        // Purge recovery container of original document:
        let _ = container.purge();

        Ok(RecoveryActionResult {
            document_id: manifest.document_id,
            action: RecoveryAction::OpenAsNew,
            session_snapshot: Some(final_snap),
            target_token: Some(new_token),
            message: "Recovered state saved as new document. Original file and recovery container cleaned up.".to_owned(),
        })
    }

    /// Internal hook called after every transaction/mutation to persist recovery journal and manifest.
    pub(crate) fn record_recovery_mutation(&self, session_id: &str) -> Result<(), SessionError> {
        let (doc_id, src_token, disp_name, file_id, journal, rev, bytes_opt) = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let s = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            let journal = s.transaction_manager.export_persistent_journal();
            let bytes_opt = if s.revision % 10 == 0 {
                // Periodic checkpoint every 10 revisions
                s.actor().current_pdf_bytes().ok()
            } else {
                None
            };
            (
                s.document_id.clone(),
                s.file_identity.source_token.clone(),
                s.file_identity.display_name.clone(),
                s.file_identity.clone(),
                journal,
                s.revision,
                bytes_opt,
            )
        };

        let container = self.recovery_store.container_for(doc_id.as_str());
        container
            .write_journal(&journal)
            .map_err(SessionError::Recovery)?;

        let mut chk_file = None;
        let mut chk_sha = None;
        if let Some((f, h)) =
            bytes_opt.and_then(|bytes| container.write_checkpoint(rev, &bytes).ok())
        {
            chk_file = Some(f);
            chk_sha = Some(h);
        }

        let now = current_unix_ms();
        let manifest = RecoveryManifest {
            schema_version: RECOVERY_SCHEMA_VERSION,
            document_id: doc_id.to_string(),
            source_token: src_token.clone(),
            display_name: disp_name,
            original_file_identity: file_id,
            active_pid: std::process::id(),
            is_clean_shutdown: false,
            has_unsaved_changes: true,
            uncommitted_revisions: rev,
            last_saved_revision: journal.saved_revision,
            created_unix_ms: now,
            updated_unix_ms: now,
            last_checkpoint_file: chk_file,
            last_checkpoint_sha256: chk_sha.clone(),
            manifest_checksum_sha256: RecoveryManifest::compute_checksum(
                RECOVERY_SCHEMA_VERSION,
                doc_id.as_str(),
                &src_token,
                std::process::id(),
                false,
                true,
                rev,
                now,
                chk_sha.as_deref(),
            ),
        };
        container
            .write_manifest(&manifest)
            .map_err(SessionError::Recovery)?;
        Ok(())
    }
}

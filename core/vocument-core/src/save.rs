//! Safe Save architecture (`ARCHITECTURE.md` §16, `ROADMAP.md` M0.5).
//!
//! Enforces:
//! 1. Atomic save path with temporary staging and flush/sync (`SAVEATOMIC-001`).
//! 2. Post-save validation via `PdfEngine` before replacing original file (`SAVE-001`).
//! 3. External change detection via multi-signal `FileIdentity` comparison (`FILELOCK-001`, `FILELOCK-002`).
//! 4. Non-silent conflict UI contract and collision-free safe copy option (`FILELOCK-003`, `FILELOCK-004`).
//! 5. **M0.5 Gate: Never silently overwrite externally changed content.**

use crate::file_broker::{FileBroker, FileBrokerError};
use crate::identity::{FileIdentity, FileIdentityComparison, SessionId};
use crate::pdf_engine::{PdfEngine, PdfEngineError};
use crate::session::{DocumentSession, SessionError, SessionRegistry};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SaveError {
    #[error(transparent)]
    FileBroker(#[from] FileBrokerError),
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
    #[error("post-save validation failed: {details}")]
    PostSaveValidationFailed { details: String },
    #[error("external modification conflict: {details}")]
    ExternalConflict {
        conflict: Box<SaveConflict>,
        details: String,
    },
    #[error("invalid save operation: {details}")]
    InvalidOperation { details: String },
}

/// Conflict contract generated when an on-disk file was modified externally (`FILELOCK-003`, `FILELOCK-004`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveConflict {
    pub session_id: SessionId,
    pub target_token: String,
    pub baseline_identity: FileIdentity,
    pub observed_external_identity: FileIdentity,
    pub comparison: FileIdentityComparison,
    pub suggested_safe_copy_path: String,
}

/// Resolution choices offered to the user in the conflict UI contract (`ARCHITECTURE.md` §16, `FILELOCK-004`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveConflictResolution {
    /// Saves local dirty state to a collision-free safe copy path, leaving external file untouched.
    SaveLocalAsCollisionFreeCopy,
    /// Explicitly overwrites external file after user confirmation.
    OverwriteExternalExplicitly,
    /// Discards local dirty edits and reloads external version from disk.
    ReloadExternalVersion,
    /// Cancels save and keeps local dirty edits in session.
    Cancel,
}

/// Snapshot describing a successfully completed safe save (`ROADMAP.md` M0.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveSuccessSnapshot {
    pub session_id: SessionId,
    pub target_token: String,
    pub new_file_identity: FileIdentity,
    pub saved_revision: u64,
    pub bytes_written: u64,
}

/// Outcome of a safe save operation: either successful save or an external modification conflict (`ROADMAP.md` M0.5 Gate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveResult {
    Success(SaveSuccessSnapshot),
    Conflict(SaveConflict),
}

/// Result of checking whether the file on disk has changed externally (`FILELOCK-002`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalChangeCheckResult {
    pub session_id: SessionId,
    pub source_token: String,
    pub has_external_changes: bool,
    pub comparison: FileIdentityComparison,
    pub observed_identity: Option<FileIdentity>,
}

/// Generates a guaranteed collision-free path (`ARCHITECTURE.md` §16, `FILELOCK-004`).
///
/// Example: `report.pdf` with suffix `"Safe Copy"` produces:
/// - `report (Safe Copy).pdf` if available, or
/// - `report (Safe Copy 1).pdf`, `report (Safe Copy 2).pdf`, etc.
pub fn generate_collision_free_path(base_path: &Path, suffix_label: &str) -> PathBuf {
    let parent = base_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let stem = base_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".to_owned());

    let ext = base_path
        .extension()
        .map(|e| e.to_string_lossy().into_owned());

    let candidate_name = match &ext {
        Some(e) => format!("{stem} ({suffix_label}).{e}"),
        None => format!("{stem} ({suffix_label})"),
    };
    let candidate_path = parent.join(&candidate_name);
    if !candidate_path.exists() {
        return candidate_path;
    }

    let mut counter: u32 = 1;
    loop {
        let name = match &ext {
            Some(e) => format!("{stem} ({suffix_label} {counter}).{e}"),
            None => format!("{stem} ({suffix_label} {counter})"),
        };
        let p = parent.join(name);
        if !p.exists() {
            return p;
        }
        counter += 1;
    }
}

/// Save coordinator orchestrating transactional save, staging, post-save validation,
/// and external conflict detection (`ARCHITECTURE.md` §16).
#[derive(Debug)]
pub struct SaveCoordinator;

impl SaveCoordinator {
    /// Inspects whether the on-disk file for `session` was modified externally (`FILELOCK-001`, `FILELOCK-002`).
    pub fn check_external_changes(
        session: &DocumentSession,
        broker: &dyn FileBroker,
    ) -> Result<ExternalChangeCheckResult, SessionError> {
        let source_token = &session.file_identity.source_token;
        let observed = broker.inspect_file_identity(source_token)?;
        let (has_external_changes, comparison) = match &observed {
            None => (false, FileIdentityComparison::Unchanged),
            Some(obs) => {
                let cmp = session.compare_external_identity(obs);
                let conflict = matches!(
                    cmp,
                    FileIdentityComparison::ModifiedExternally
                        | FileIdentityComparison::DifferentFileReplacingSamePath
                );
                (conflict, cmp)
            }
        };

        Ok(ExternalChangeCheckResult {
            session_id: session.session_id.clone(),
            source_token: source_token.clone(),
            has_external_changes,
            comparison,
            observed_identity: observed,
        })
    }

    /// Performs post-save validation of staged bytes (`ARCHITECTURE.md` §16 step 3, `SAVE-001`).
    pub fn validate_staged_bytes(
        engine: &dyn PdfEngine,
        bytes: &[u8],
        expected_page_count: u32,
    ) -> Result<(), SaveError> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(SaveError::PostSaveValidationFailed {
                details: "staged output missing %PDF- header".to_owned(),
            });
        }

        let doc =
            engine
                .open_pdf_bytes(bytes)
                .map_err(|err| SaveError::PostSaveValidationFailed {
                    details: format!("engine failed to open staged PDF: {err}"),
                })?;

        if doc.summary().page_count != expected_page_count {
            return Err(SaveError::PostSaveValidationFailed {
                details: format!(
                    "page count mismatch: expected {expected_page_count}, found {}",
                    doc.summary().page_count
                ),
            });
        }

        if expected_page_count > 0 {
            doc.page_geometry(0)
                .map_err(|err| SaveError::PostSaveValidationFailed {
                    details: format!("failed to read page 0 geometry: {err}"),
                })?;
            if expected_page_count > 1 {
                doc.page_geometry(expected_page_count - 1).map_err(|err| {
                    SaveError::PostSaveValidationFailed {
                        details: format!("failed to read last page geometry: {err}"),
                    }
                })?;
            }
        }

        Ok(())
    }

    /// Executes transactional save:
    /// 1. Fetch current PDF bytes from actor.
    /// 2. Stage write to temporary file (`FileBroker::stage_write`).
    /// 3. Validate staged PDF (`PdfEngine`).
    /// 4. Verify external file identity. If modified externally and overwrite not explicit, abort! (M0.5 Gate).
    /// 5. Atomically replace target file (`FileBroker::commit_staged_write`).
    /// 6. Mark revision saved and update session identity.
    pub fn execute_safe_save(
        session: &mut DocumentSession,
        broker: &dyn FileBroker,
        engine: &dyn PdfEngine,
        target_token: &str,
        allow_overwrite_external: bool,
    ) -> Result<SaveResult, SessionError> {
        let bytes = session.actor.current_pdf_bytes()?;
        let expected_page_count = session.pdf_summary.page_count;

        // Stage output safely:
        let staged = broker.stage_write(target_token, &bytes)?;

        // Step 3: Validate staged bytes before touching original target:
        if let Err(err) = Self::validate_staged_bytes(engine, &bytes, expected_page_count) {
            let _ = broker.discard_staged_write(staged);
            return Err(SessionError::Save(err));
        }

        // Step 4: External conflict check (M0.5 Gate):
        let external_identity = if allow_overwrite_external {
            None
        } else {
            broker.inspect_file_identity(target_token)?
        };
        if let Some(observed_external) = external_identity {
            let cmp = session.file_identity.compare_with(&observed_external);
            if matches!(
                cmp,
                FileIdentityComparison::ModifiedExternally
                    | FileIdentityComparison::DifferentFileReplacingSamePath
            ) {
                // M0.5 Gate: Never silently overwrite externally changed content!
                let _ = broker.discard_staged_write(staged);

                let normalized = target_token.strip_prefix("file://").unwrap_or(target_token);
                let safe_copy =
                    generate_collision_free_path(Path::new(normalized), "Conflicted Copy");
                let conflict = SaveConflict {
                    session_id: session.session_id.clone(),
                    target_token: target_token.to_owned(),
                    baseline_identity: session.file_identity.clone(),
                    observed_external_identity: observed_external,
                    comparison: cmp,
                    suggested_safe_copy_path: safe_copy.to_string_lossy().into_owned(),
                };
                return Ok(SaveResult::Conflict(conflict));
            }
        }

        // Step 5: Atomically replace target file:
        let new_file_identity = broker.commit_staged_write(staged)?;
        let bytes_written = new_file_identity.size_bytes;

        // Step 6: Synchronize session state:
        session.mark_as_saved(new_file_identity.clone());

        Ok(SaveResult::Success(SaveSuccessSnapshot {
            session_id: session.session_id.clone(),
            target_token: target_token.to_owned(),
            new_file_identity,
            saved_revision: session.revision,
            bytes_written,
        }))
    }
}

impl SessionRegistry {
    /// Safe save of active `session_id` back to its original file (`ROADMAP.md` M0.5, `SAVE-001`).
    pub fn save_session(&self, session_id: &str) -> Result<SaveResult, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;

        let doc_id = session.document_id.clone();
        let target_token = session.file_identity.source_token.clone();
        let outcome = SaveCoordinator::execute_safe_save(
            session,
            self.file_broker.as_ref(),
            self.pdf_engine.as_ref(),
            &target_token,
            false,
        )?;
        if matches!(outcome, SaveResult::Success(_)) {
            let _ = self
                .recovery_store
                .container_for(&doc_id)
                .mark_clean_shutdown();
        }
        Ok(outcome)
    }

    /// 'Save As' of active `session_id` to `target_token` (`SAVE-002`).
    pub fn save_session_as(
        &self,
        session_id: &str,
        target_token: &str,
    ) -> Result<SaveResult, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;

        let doc_id = session.document_id.clone();
        let outcome = SaveCoordinator::execute_safe_save(
            session,
            self.file_broker.as_ref(),
            self.pdf_engine.as_ref(),
            target_token,
            false,
        )?;
        if matches!(outcome, SaveResult::Success(_)) {
            let _ = self
                .recovery_store
                .container_for(&doc_id)
                .mark_clean_shutdown();
        }
        Ok(outcome)
    }

    /// Saves a collision-free safe copy of `session_id`, leaving original file untouched (`ARCHITECTURE.md` §16, `FILELOCK-004`).
    pub fn save_session_collision_free_copy(
        &self,
        session_id: &str,
    ) -> Result<SaveSuccessSnapshot, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;

        let source = &session.file_identity.source_token;
        let normalized = source.strip_prefix("file://").unwrap_or(source);
        let safe_copy_path = generate_collision_free_path(Path::new(normalized), "Safe Copy");
        let safe_copy_token = safe_copy_path.to_string_lossy().into_owned();

        let outcome = SaveCoordinator::execute_safe_save(
            session,
            self.file_broker.as_ref(),
            self.pdf_engine.as_ref(),
            &safe_copy_token,
            true,
        )?;

        match outcome {
            SaveResult::Success(snapshot) => Ok(snapshot),
            SaveResult::Conflict(_) => Err(SessionError::Save(SaveError::InvalidOperation {
                details: "unexpected conflict during safe copy".to_owned(),
            })),
        }
    }

    /// Proactively checks whether on-disk file for `session_id` was modified externally (`FILELOCK-002`).
    pub fn check_external_changes(
        &self,
        session_id: &str,
    ) -> Result<ExternalChangeCheckResult, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;

        SaveCoordinator::check_external_changes(session, self.file_broker.as_ref())
    }

    /// Resolves an external save conflict according to `resolution` (`ARCHITECTURE.md` §16, `FILELOCK-004`).
    pub fn resolve_save_conflict(
        &self,
        session_id: &str,
        resolution: SaveConflictResolution,
    ) -> Result<Option<SaveSuccessSnapshot>, SessionError> {
        match resolution {
            SaveConflictResolution::SaveLocalAsCollisionFreeCopy => {
                let snapshot = self.save_session_collision_free_copy(session_id)?;
                Ok(Some(snapshot))
            }
            SaveConflictResolution::OverwriteExternalExplicitly => {
                let mut guard = self
                    .sessions
                    .lock()
                    .map_err(|_| SessionError::RegistryPoisoned)?;
                let session =
                    guard
                        .get_mut(session_id)
                        .ok_or_else(|| SessionError::SessionNotFound {
                            session_id: session_id.to_owned(),
                        })?;

                let target_token = session.file_identity.source_token.clone();
                let outcome = SaveCoordinator::execute_safe_save(
                    session,
                    self.file_broker.as_ref(),
                    self.pdf_engine.as_ref(),
                    &target_token,
                    true,
                )?;
                match outcome {
                    SaveResult::Success(snapshot) => Ok(Some(snapshot)),
                    SaveResult::Conflict(_) => {
                        Err(SessionError::Save(SaveError::InvalidOperation {
                            details: "overwrite failed unexpectedly".to_owned(),
                        }))
                    }
                }
            }
            SaveConflictResolution::ReloadExternalVersion => {
                let mut guard = self
                    .sessions
                    .lock()
                    .map_err(|_| SessionError::RegistryPoisoned)?;
                let session =
                    guard
                        .get_mut(session_id)
                        .ok_or_else(|| SessionError::SessionNotFound {
                            session_id: session_id.to_owned(),
                        })?;

                let read = self
                    .file_broker
                    .read_file(&session.file_identity.source_token)?;
                let engine_doc = self.pdf_engine.open_pdf_bytes(&read.bytes)?;
                let pdf_summary = engine_doc.summary().clone();
                session.actor = std::sync::Arc::new(crate::render::DocumentActor::new(engine_doc));
                session.pdf_summary = pdf_summary;
                session.file_identity = read.compute_identity();
                session.transaction_manager = crate::transactions::TransactionManager::new(
                    session.document_id.clone(),
                    &read.bytes,
                );
                session.sync_from_transaction_manager();
                Ok(None)
            }
            SaveConflictResolution::Cancel => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_free_path_increments_sequence() {
        let temp_dir =
            std::env::temp_dir().join(format!("vocument_test_coll_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let base = temp_dir.join("contract.pdf");
        let _ = std::fs::write(&base, b"base");

        let copy1 = generate_collision_free_path(&base, "Safe Copy");
        assert_eq!(
            copy1
                .file_name()
                .expect("filename")
                .to_str()
                .expect("utf-8"),
            "contract (Safe Copy).pdf"
        );
        let _ = std::fs::write(&copy1, b"c1");

        let copy2 = generate_collision_free_path(&base, "Safe Copy");
        assert_eq!(
            copy2
                .file_name()
                .expect("filename")
                .to_str()
                .expect("utf-8"),
            "contract (Safe Copy 1).pdf"
        );
        let _ = std::fs::write(&copy2, b"c2");

        let copy3 = generate_collision_free_path(&base, "Safe Copy");
        assert_eq!(
            copy3
                .file_name()
                .expect("filename")
                .to_str()
                .expect("utf-8"),
            "contract (Safe Copy 2).pdf"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

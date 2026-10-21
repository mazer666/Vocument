//! The recovery actions of a document (`ROADMAP.md` M0.6, `ARCHITECTURE.md` §12).
//!
//! The actions of the crash recovery — restore, preview, discard and open as
//! new — belong to the session, but they are not part of the container of the
//! recovery store. They live in their own module, so that the container does
//! not grow beyond the size that is readable.

use std::path::Path;

use crate::recovery::{
    RECOVERY_SCHEMA_VERSION, RecoveryAction, RecoveryActionResult, RecoveryContainer,
    RecoveryError, RecoveryManifest, current_unix_ms,
};
use crate::save::generate_collision_free_path;
use crate::session::{SessionError, SessionRegistry};
use crate::transactions::PersistentTransactionJournal;

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

    /// The note that a recovery action has to carry when the journal holds a
    /// redaction.
    ///
    /// A redaction is not replayed after a crash: it is irreversible and the
    /// journal cannot say whether it was ever verified. The document therefore
    /// stands at the state before the redaction — and the user has to know
    /// that, because the text that was to be removed is still in the document.
    fn redaction_not_replayed_note(journal: &PersistentTransactionJournal) -> Option<&'static str> {
        let dropped = journal
            .committed_records
            .iter()
            .any(|record| record.forward_command.is_redaction());
        if dropped {
            Some(
                "the last redaction was not replayed: a redaction cannot be verified from the journal, and the document stands at the state before it",
            )
        } else {
            None
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
            message: match Self::redaction_not_replayed_note(&journal) {
                Some(note) => format!(
                    "Document session restored from crash recovery journal, but {note}. \
                     Original file on disk remains unchanged until save."
                ),
                None => "Document session restored from crash recovery journal. Original file on disk remains unchanged until save.".to_owned(),
            },
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
            message: match Self::redaction_not_replayed_note(&journal) {
                Some(note) => format!(
                    "Preview session created, but {note}. \
                     Recovery container and on-disk file are unchanged."
                ),
                None => {
                    "Preview session created. Recovery container and on-disk file are unchanged."
                        .to_owned()
                }
            },
        })
    }

    fn execute_recovery_open_as_new(
        &self,
        container: &RecoveryContainer,
        manifest: RecoveryManifest,
    ) -> Result<RecoveryActionResult, SessionError> {
        // The journal is read a second time on purpose: the note about a
        // redaction that was not replayed has to reach the user here as well.
        // The file that is written here still carries the text that was to be
        // removed, and that has to be said, not only noted in the restore.
        let journal = container
            .read_journal()
            .map_err(SessionError::Recovery)?
            .ok_or_else(|| {
                SessionError::Recovery(RecoveryError::NotFound {
                    document_id: manifest.document_id.clone(),
                })
            })?;
        let note = Self::redaction_not_replayed_note(&journal);

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
            message: match note {
                Some(note) => format!(
                    "Recovered state saved as new document, but {note}. \
                     The new file therefore still carries the text that was to be removed. \
                     Original file and recovery container cleaned up."
                ),
                None => "Recovered state saved as new document. Original file and recovery container cleaned up."
                    .to_owned(),
            },
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

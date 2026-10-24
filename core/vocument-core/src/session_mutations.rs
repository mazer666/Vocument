//! M0.4 mutation, Undo/Redo, and persistent transaction journal operations on `SessionRegistry` (`ROADMAP.md` M0.4, `ARCHITECTURE.md` §11).
//!
//! Gate (`ROADMAP.md` M0.4): **Mutation never bypasses `TransactionManager`.**

use crate::pdf_engine::{
    PageBoxUpdate, PdfHighlightAnnotation, PdfPageBoxes, PdfRectMilliPt, PdfSanitizeOptions,
};
use crate::session::{SessionError, SessionRegistry};
use crate::transactions::{
    PersistentTransactionJournal, TransactionCommand, TransactionError, TransactionHistorySnapshot,
    TransactionRecord,
};

impl SessionRegistry {
    /// Queries the current rotation (`0`, `90`, `180`, or `270` degrees) of `page_index` (`ROTATE-001`).
    pub fn page_rotation_degrees(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<u32, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.page_rotation_degrees(page_index)?)
    }

    /// Queries all standard PDF `/Highlight` annotations on `page_index` (`ANNOT-001`, `INTEROP-002`).
    pub fn page_highlight_annotations(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.page_highlight_annotations(page_index)?)
    }

    /// Executes a semantic `TransactionCommand` exclusively through `TransactionManager` (`ROADMAP.md` M0.4 Gate).
    pub fn execute_transaction(
        &self,
        session_id: &str,
        command: TransactionCommand,
    ) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session
            .transaction_manager
            .execute_command(&actor, command)?;
        session.sync_from_transaction_manager();
        drop(guard);
        // The state of the transaction is secured in the recovery store. If
        // that fails, the state of the document is not protected against a
        // crash any more. The transaction itself has succeeded — the error
        // belongs to the recovery store and must not turn a change that
        // happened into a failure. It is reported and recorded as degraded recovery.
        if let Err(error) = self.record_recovery_mutation(session_id) {
            self.set_session_recovery_degraded(session_id, true);
            eprintln!(
                "vocument: the state of the document could not be secured for the crash recovery: {error}"
            );
        } else {
            self.set_session_recovery_degraded(session_id, false);
        }
        Ok(record)
    }

    fn set_session_recovery_degraded(&self, session_id: &str, degraded: bool) {
        if let Ok(mut guard) = self.sessions.lock()
            && let Some(session) = guard.get_mut(session_id)
        {
            session.recovery_degraded = degraded;
        }
    }

    /// Rotates one or more `0`-based pages by `delta_degrees` (`+90`, `-90`, `180`, `-180`, `270`) via `TransactionManager` (`ROTATE-001`).
    pub fn rotate_pages(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        delta_degrees: i32,
    ) -> Result<TransactionRecord, SessionError> {
        self.rotate_pages_with_options(
            session_id,
            page_indices,
            delta_degrees,
            crate::workbench::PageWorkbenchOptions::default(),
        )
    }

    /// Rotates one or more `0`-based pages with explicit options through `TransactionManager` (`M3`).
    pub fn rotate_pages_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        delta_degrees: i32,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        for &idx in &page_indices {
            if idx >= snap.pdf_summary.page_count {
                return Err(crate::pdf_engine::PdfEngineError::PageOutOfBounds {
                    page_index: idx,
                    page_count: snap.pdf_summary.page_count,
                }
                .into());
            }
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::RotatePages {
                page_indices,
                delta_degrees,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Adds a standard PDF `/Highlight` annotation via `TransactionManager` (`ANNOT-001`, `INTEROP-002`).
    pub fn add_highlight_annotation(
        &self,
        session_id: &str,
        annotation: PdfHighlightAnnotation,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::AddHighlightAnnotation { annotation },
        )
    }

    /// Removes the content of the given regions of a page for good (`REDACT-001`).
    ///
    /// The redaction cannot be undone. The checkpoints of the recovery store
    /// still hold the bytes of the earlier states of the document, and those
    /// bytes still contain the removed content — they are deleted with the
    /// redaction.
    ///
    /// The checkpoints are **not** deleted here: the removal can still fail
    /// its verification, and then the document is put back. The checkpoints
    /// are deleted after the verification has succeeded.
    pub fn apply_redaction(
        &self,
        session_id: &str,
        page_index: u32,
        regions: Vec<PdfRectMilliPt>,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RedactPageRegions {
                page_index,
                regions,
            },
        )
    }

    /// Writes the current text of a page into the search index.
    ///
    /// After a redaction the index has to be updated, otherwise the search
    /// would still find the text that was removed from the document
    /// (`REDACT-001`).
    pub(crate) fn reindex_page(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<(), SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        let structured = actor.extract_page_text(page_index)?;

        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let _ = session.search_index.index_structured_page(structured)?;
        Ok(())
    }

    /// Removes a standard PDF `/Highlight` annotation by `annotation_id` via `TransactionManager` (`ANNOT-001`).
    pub fn remove_highlight_annotation(
        &self,
        session_id: &str,
        page_index: u32,
        annotation_id: String,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(
            session_id,
            TransactionCommand::RemoveHighlightAnnotation {
                page_index,
                annotation_id,
                restored_annotation: None,
            },
        )
    }

    /// Undoes the most recent semantic transaction on `session_id` (`UNDO-001`, `UNDO-004`).
    pub fn undo(&self, session_id: &str) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session.transaction_manager.undo(&actor)?;
        session.sync_from_transaction_manager();
        let pages_to_reindex = match &record.forward_command {
            TransactionCommand::ApplyHardCrop { page_indices, .. }
            | TransactionCommand::RotatePages { page_indices, .. }
            | TransactionCommand::DeskewPages { page_indices, .. } => page_indices.clone(),
            _ => Vec::new(),
        };
        drop(guard);
        for p in pages_to_reindex {
            let _ = self.reindex_page(session_id, p);
        }
        // The state of the transaction is secured in the recovery store. If
        // that fails, the state of the document is not protected against a
        // crash any more. The transaction itself has succeeded — the error
        // belongs to the recovery store and must not turn a change that
        // happened into a failure. It is reported and recorded as degraded recovery.
        if let Err(error) = self.record_recovery_mutation(session_id) {
            self.set_session_recovery_degraded(session_id, true);
            eprintln!(
                "vocument: the state of the document could not be secured for the crash recovery: {error}"
            );
        } else {
            self.set_session_recovery_degraded(session_id, false);
        }
        Ok(record)
    }

    /// Redoes the most recently undone semantic transaction on `session_id` (`UNDO-001`, `UNDO-004`).
    pub fn redo(&self, session_id: &str) -> Result<TransactionRecord, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let record = session.transaction_manager.redo(&actor)?;
        session.sync_from_transaction_manager();
        let pages_to_reindex = match &record.forward_command {
            TransactionCommand::ApplyHardCrop { page_indices, .. }
            | TransactionCommand::RotatePages { page_indices, .. }
            | TransactionCommand::DeskewPages { page_indices, .. } => page_indices.clone(),
            _ => Vec::new(),
        };
        drop(guard);
        for p in pages_to_reindex {
            let _ = self.reindex_page(session_id, p);
        }
        // The state of the transaction is secured in the recovery store. If
        // that fails, the state of the document is not protected against a
        // crash any more. The transaction itself has succeeded — the error
        // belongs to the recovery store and must not turn a change that
        // happened into a failure. It is reported and recorded as degraded recovery.
        if let Err(error) = self.record_recovery_mutation(session_id) {
            self.set_session_recovery_degraded(session_id, true);
            eprintln!(
                "vocument: the state of the document could not be secured for the crash recovery: {error}"
            );
        } else {
            self.set_session_recovery_degraded(session_id, false);
        }
        Ok(record)
    }

    /// Queries the live Undo/Redo history snapshot of `session_id` (`ROADMAP.md` M0.4).
    pub fn transaction_history(
        &self,
        session_id: &str,
    ) -> Result<TransactionHistorySnapshot, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok(session.transaction_manager.history_snapshot())
    }

    /// Exports the session's `PersistentTransactionJournal` as checksum-verified pretty JSON (`ARCHITECTURE.md` §§11–12, `UNDO-001`).
    pub fn export_persistent_transaction_journal_json(
        &self,
        session_id: &str,
    ) -> Result<String, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let journal = session.transaction_manager.export_persistent_journal();
        Ok(journal.to_json_pretty()?)
    }

    /// Verifies and replays a `PersistentTransactionJournal` JSON payload onto `session_id` (`UNDO-001`).
    pub fn restore_persistent_transaction_journal_json(
        &self,
        session_id: &str,
        journal_json: &str,
    ) -> Result<TransactionHistorySnapshot, SessionError> {
        let journal = PersistentTransactionJournal::from_json_verified(journal_json)?;
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let snapshot = session
            .transaction_manager
            .restore_and_replay_journal(&actor, &journal)?;
        session.sync_from_transaction_manager();
        Ok(snapshot)
    }

    /// Sanitizes the document of `session_id` by stripping metadata, attachments,
    /// actions, annotations, or form fields through `TransactionManager` (`SAN-001`).
    pub fn sanitize_document(
        &self,
        session_id: &str,
        options: PdfSanitizeOptions,
    ) -> Result<TransactionRecord, SessionError> {
        self.execute_transaction(session_id, TransactionCommand::SanitizeDocument { options })
    }

    /// Audits structural invariants and digital signatures before workbench page mutations (`ROADMAP.md` M3, `ADR-0019`).
    pub fn workbench_preflight(
        &self,
        session_id: &str,
    ) -> Result<crate::workbench::PageWorkbenchPreflightReport, SessionError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let report = actor.workbench_preflight()?;
        Ok(report)
    }

    /// Reorders the pages of `session_id` according to `order` through `TransactionManager` (`ROADMAP.md` M3).
    pub fn reorder_pages(
        &self,
        session_id: &str,
        order: Vec<u32>,
    ) -> Result<TransactionRecord, SessionError> {
        self.reorder_pages_with_options(
            session_id,
            order,
            crate::workbench::PageWorkbenchOptions::default(),
        )
    }

    /// Reorders the pages of `session_id` with explicit options through `TransactionManager` (`ROADMAP.md` M3).
    pub fn reorder_pages_with_options(
        &self,
        session_id: &str,
        order: Vec<u32>,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        crate::workbench::validate_permutation(&order, snap.pdf_summary.page_count)?;
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::ReorderPages {
                order,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Duplicates `page_index` of `session_id` through `TransactionManager` (`ROADMAP.md` M3).
    pub fn duplicate_page(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<TransactionRecord, SessionError> {
        self.duplicate_page_with_options(
            session_id,
            page_index,
            crate::workbench::PageWorkbenchOptions::default(),
        )
    }

    /// Duplicates `page_index` with explicit options through `TransactionManager` (`ROADMAP.md` M3).
    pub fn duplicate_page_with_options(
        &self,
        session_id: &str,
        page_index: u32,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        if page_index >= snap.pdf_summary.page_count {
            return Err(crate::pdf_engine::PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: snap.pdf_summary.page_count,
            }
            .into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::DuplicatePage {
                page_index,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Deletes `page_indices` from `session_id` through `TransactionManager` (`ROADMAP.md` M3).
    pub fn delete_pages(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
    ) -> Result<TransactionRecord, SessionError> {
        self.delete_pages_with_options(
            session_id,
            page_indices,
            crate::workbench::PageWorkbenchOptions::default(),
        )
    }

    /// Deletes `page_indices` with explicit options through `TransactionManager` (`ROADMAP.md` M3).
    pub fn delete_pages_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        crate::workbench::validate_delete_selection(&page_indices, snap.pdf_summary.page_count)?;
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::DeletePages {
                page_indices,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Extracts `page_indices` from `session_id` into a new standalone PDF buffer (`ROADMAP.md` M3).
    pub fn extract_pages(
        &self,
        session_id: &str,
        page_indices: &[u32],
    ) -> Result<Vec<u8>, SessionError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let bytes = actor.extract_pages(page_indices)?;
        Ok(bytes)
    }

    /// Inserts pages from an external PDF document at `insert_at` index (`M3.2`).
    pub fn insert_pages_from_document_with_options(
        &self,
        session_id: &str,
        source_pdf: Vec<u8>,
        source_page_indices: Vec<u32>,
        insert_at: u32,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        if insert_at > snap.pdf_summary.page_count {
            return Err(crate::pdf_engine::PdfEngineError::PageOutOfBounds {
                page_index: insert_at,
                page_count: snap.pdf_summary.page_count,
            }
            .into());
        }
        if source_page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::InsertPagesFromDocument {
                source_pdf,
                source_page_indices,
                insert_at,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Merges multiple external PDF documents into the active session (`M3.2`).
    pub fn merge_documents_with_options(
        &self,
        session_id: &str,
        source_pdfs: Vec<Vec<u8>>,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if source_pdfs.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::MergeDocuments {
                source_pdfs,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Splits the session document into standalone PDF byte vectors for each page range (`M3.2`).
    pub fn split_document(
        &self,
        session_id: &str,
        split_ranges: Vec<Vec<u32>>,
    ) -> Result<Vec<Vec<u8>>, SessionError> {
        let snap = self.session_snapshot(session_id)?;
        for range in &split_ranges {
            if range.is_empty() {
                return Err(TransactionError::EmptyPageSelection.into());
            }
            for &idx in range {
                if idx >= snap.pdf_summary.page_count {
                    return Err(crate::pdf_engine::PdfEngineError::PageOutOfBounds {
                        page_index: idx,
                        page_count: snap.pdf_summary.page_count,
                    }
                    .into());
                }
            }
        }
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        let actor = session.actor();
        let split_docs = actor.split_document(&split_ranges)?;
        Ok(split_docs)
    }

    /// Queries the 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
    pub fn get_page_boxes(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<PdfPageBoxes, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.get_page_boxes(page_index)?)
    }

    /// Detects document skew angle on `page_index` in degrees (`ROTATE-003`).
    pub fn detect_deskew_angle(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<f32, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.detect_deskew_angle(page_index)?)
    }

    /// Detects content bounding box on `page_index` with a safety margin (`CROP-003`).
    pub fn detect_content_margins(
        &self,
        session_id: &str,
        page_index: u32,
        safety_margin_milli_pt: i32,
    ) -> Result<PdfRectMilliPt, SessionError> {
        let actor = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            session.actor()
        };
        Ok(actor.detect_content_margins(page_index, safety_margin_milli_pt)?)
    }

    /// Sets or updates page boundary boxes with explicit preflight options (`CROP-002`).
    pub fn set_page_boxes_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        boxes: PageBoxUpdate,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::SetPageBoxes {
                page_indices,
                boxes,
                previous_boxes: Vec::new(),
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Applies a destructive hard crop to selected pages (`CROP-001`).
    pub fn apply_hard_crop_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        crop_rect: PdfRectMilliPt,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        let record = self.execute_transaction(
            session_id,
            TransactionCommand::ApplyHardCrop {
                page_indices: page_indices.clone(),
                crop_rect,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )?;
        for p in page_indices {
            let _ = self.reindex_page(session_id, p);
        }
        Ok(record)
    }

    /// Applies a non-destructive display crop by adjusting /CropBox (`CROP-001`).
    pub fn apply_display_crop_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        crop_rect: PdfRectMilliPt,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        self.set_page_boxes_with_options(
            session_id,
            page_indices,
            PageBoxUpdate {
                media_box: None,
                crop_box: Some(crop_rect),
                bleed_box: None,
                trim_box: None,
                art_box: None,
            },
            options,
        )
    }

    /// Applies fine deskew rotation via transformation matrix (`ROTATE-003`).
    pub fn deskew_pages_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        angle_milli_degrees: i32,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::DeskewPages {
                page_indices,
                angle_milli_degrees,
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }

    /// Automatically crops pages by detecting content margins and setting CropBox (`CROP-003`).
    pub fn auto_crop_pages_with_options(
        &self,
        session_id: &str,
        page_indices: Vec<u32>,
        safety_margin_milli_pt: i32,
        options: crate::workbench::PageWorkbenchOptions,
    ) -> Result<TransactionRecord, SessionError> {
        if page_indices.is_empty() {
            return Err(TransactionError::EmptyPageSelection.into());
        }
        let preflight = self.workbench_preflight(session_id)?;
        if preflight.is_signed && !options.allow_signature_invalidation {
            return Err(
                crate::workbench::WorkbenchError::OperationBlockedBySignature {
                    signature_count: preflight.signature_count,
                }
                .into(),
            );
        }
        self.execute_transaction(
            session_id,
            TransactionCommand::AutoCropPages {
                page_indices,
                safety_margin_milli_pt,
                previous_boxes: Vec::new(),
                allow_signature_invalidation: options.allow_signature_invalidation,
            },
        )
    }
}

#[cfg(test)]
#[path = "session_mutations_tests.rs"]
mod session_mutations_tests;

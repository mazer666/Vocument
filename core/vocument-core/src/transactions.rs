//! `TransactionManager`, semantic mutation commands, Undo/Redo stack, and persistent transaction journal (`ARCHITECTURE.md` §§6, 11–12, `ROADMAP.md` M0.4).
//!
//! Gate (`ROADMAP.md` M0.4): **Mutation never bypasses `TransactionManager`.**
//! Every document mutation executes through `TransactionManager::execute_command`, `undo`, or `redo`.

use crate::identity::{DocumentId, compute_sha256_hex as sha256_hex};
use crate::pdf_engine::{
    PdfCommentAnnotation, PdfEngineDocument, PdfEngineError, PdfHighlightAnnotation,
    PdfRectMilliPt, PdfSanitizeOptions,
};
use crate::render::{DocumentActor, RenderPipelineError};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Current schema version for `PersistentTransactionJournal` (`ARCHITECTURE.md` §12).
pub const TRANSACTION_JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransactionError {
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
    #[error(transparent)]
    Workbench(#[from] crate::workbench::WorkbenchError),
    #[error(transparent)]
    RenderPipeline(#[from] RenderPipelineError),
    #[error("no pages specified for RotatePages command")]
    EmptyPageSelection,
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
    #[error("cannot invert RemoveHighlightAnnotation without captured annotation snapshot")]
    MissingRemovedAnnotationSnapshot,
    #[error("persistent transaction journal serialization failed: {details}")]
    SerializationFailure { details: String },
    #[error("persistent transaction journal integrity error: {details}")]
    IntegrityMismatch { details: String },
    #[error("the operation cannot be undone: {description}")]
    Irreversible { description: String },
}

/// Semantic mutation commands (`ARCHITECTURE.md` §11, `UNDO-004`, `ROTATE-001`, `ANNOT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionCommand {
    /// Rotates one or more `0`-based pages by `delta_degrees` (`+90`, `-90`, `180`, `-180`, `270`).
    RotatePages {
        page_indices: Vec<u32>,
        delta_degrees: i32,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// Adds a standard PDF `/Highlight` annotation (`ANNOT-001`, `INTEROP-002`).
    AddHighlightAnnotation { annotation: PdfHighlightAnnotation },
    /// Removes a standard PDF `/Highlight` annotation by `annotation_id` (`ANNOT-001`).
    RemoveHighlightAnnotation {
        page_index: u32,
        annotation_id: String,
        /// Captured snapshot of the removed annotation so Undo can deterministically restore it.
        restored_annotation: Option<PdfHighlightAnnotation>,
    },
    /// Adds a comment of the reviewer (`ANNOT-001`).
    ///
    /// A comment is a standard PDF `/Text` annotation with a popup. The
    /// command carries the text of the note: the note belongs to the
    /// document, and the journal of the recovery store belongs to the user
    /// alone.
    AddCommentAnnotation { annotation: PdfCommentAnnotation },
    /// Removes a comment of the reviewer by `annotation_id` (`ANNOT-001`).
    RemoveCommentAnnotation {
        page_index: u32,
        annotation_id: String,
        /// Captured snapshot of the removed note so Undo can restore it.
        restored_annotation: Option<PdfCommentAnnotation>,
    },
    /// Removes the content of regions of a page for good (`REDACT-001`).
    ///
    /// The command has no inverse: the removed content is gone from the
    /// document and cannot be restored.
    ///
    /// The command carries no text. It is written into the persistent journal
    /// of the recovery store, and the journal must not contain the content
    /// that was just removed.
    RedactPageRegions {
        page_index: u32,
        regions: Vec<PdfRectMilliPt>,
    },
    /// Sanitizes the document by stripping requested categories of metadata,
    /// attachments, actions, annotations, or form fields (`SAN-001`).
    SanitizeDocument { options: PdfSanitizeOptions },
    /// Reorders pages according to the specified complete permutation (`ROADMAP.md` M3).
    ReorderPages {
        order: Vec<u32>,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// Duplicates a single page, placing the new copy directly following it (`ROADMAP.md` M3).
    DuplicatePage {
        page_index: u32,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// Deletes one or more pages from the document (`ROADMAP.md` M3).
    DeletePages {
        page_indices: Vec<u32>,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// Restores previously deleted document bytes (`ROADMAP.md` M3).
    RestorePages { previous_bytes: Vec<u8> },
    /// Inserts pages from another PDF document at the given target index (`ROADMAP.md` M3.2).
    InsertPagesFromDocument {
        source_pdf: Vec<u8>,
        source_page_indices: Vec<u32>,
        insert_at: u32,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// Merges multiple PDF documents into the current document (`ROADMAP.md` M3.2).
    MergeDocuments {
        source_pdfs: Vec<Vec<u8>>,
        #[serde(default)]
        allow_signature_invalidation: bool,
    },
    /// The inverse of a command that cannot be undone (`REDACT-001`, `SAN-001`).
    Irreversible { description: String },
}

impl TransactionCommand {
    /// Human-readable user-facing description (`UNDO-004`: human-level operations, not engine microsteps).
    pub fn user_description(&self) -> String {
        match self {
            Self::RotatePages {
                page_indices,
                delta_degrees,
                ..
            } => {
                let sign = if *delta_degrees >= 0 { "+" } else { "" };
                if page_indices.len() == 1 {
                    format!(
                        "Rotate page #{} by {}{}°",
                        page_indices[0] + 1,
                        sign,
                        delta_degrees
                    )
                } else {
                    format!(
                        "Rotate {} pages by {}{}°",
                        page_indices.len(),
                        sign,
                        delta_degrees
                    )
                }
            }
            Self::AddHighlightAnnotation { annotation } => {
                format!(
                    "Add highlight annotation on page #{}",
                    annotation.page_index + 1
                )
            }
            Self::RemoveHighlightAnnotation { page_index, .. } => {
                format!("Remove highlight annotation on page #{}", page_index + 1)
            }
            Self::AddCommentAnnotation { annotation } => {
                format!("Add comment on page #{}", annotation.page_index + 1)
            }
            Self::RemoveCommentAnnotation { page_index, .. } => {
                format!("Remove comment on page #{}", page_index + 1)
            }
            Self::RedactPageRegions {
                page_index,
                regions,
            } => {
                format!(
                    "Remove the content of {} region(s) on page #{}",
                    regions.len(),
                    page_index + 1
                )
            }
            Self::SanitizeDocument { .. } => "Sanitize document".to_owned(),
            Self::ReorderPages { order, .. } => format!("Reorder {} pages", order.len()),
            Self::DuplicatePage { page_index, .. } => {
                format!("Duplicate page #{}", page_index + 1)
            }
            Self::DeletePages { page_indices, .. } => {
                format!("Delete {} page(s)", page_indices.len())
            }
            Self::RestorePages { .. } => "Restore deleted page(s)".to_owned(),
            Self::InsertPagesFromDocument {
                source_page_indices,
                insert_at,
                ..
            } => {
                format!(
                    "Insert {} page(s) at position #{}",
                    source_page_indices.len(),
                    insert_at + 1
                )
            }
            Self::MergeDocuments { source_pdfs, .. } => {
                format!("Merge {} document(s)", source_pdfs.len())
            }
            Self::Irreversible { description } => description.clone(),
        }
    }

    /// Answers whether the command is a redaction or sanitization.
    ///
    /// An irreversible command carries content removal that cannot be undone.
    pub fn is_redaction(&self) -> bool {
        matches!(
            self,
            Self::RedactPageRegions { .. } | Self::SanitizeDocument { .. }
        )
    }

    pub fn affected_pages(&self) -> Vec<u32> {
        match self {
            Self::RotatePages { page_indices, .. } => page_indices.clone(),
            Self::AddHighlightAnnotation { annotation } => vec![annotation.page_index],
            Self::RemoveHighlightAnnotation { page_index, .. } => vec![*page_index],
            Self::AddCommentAnnotation { annotation } => vec![annotation.page_index],
            Self::RemoveCommentAnnotation { page_index, .. } => vec![*page_index],
            Self::RedactPageRegions { page_index, .. } => vec![*page_index],
            Self::SanitizeDocument { .. } => Vec::new(),
            Self::ReorderPages { order, .. } => (0..order.len() as u32).collect(),
            Self::DuplicatePage { page_index, .. } => vec![*page_index, *page_index + 1],
            Self::DeletePages { page_indices, .. } => page_indices.clone(),
            Self::RestorePages { .. } => Vec::new(),
            Self::InsertPagesFromDocument {
                source_page_indices,
                insert_at,
                ..
            } => (*insert_at..(*insert_at + source_page_indices.len() as u32)).collect(),
            Self::MergeDocuments { .. } => Vec::new(),
            Self::Irreversible { .. } => Vec::new(),
        }
    }

    pub fn affected_objects(&self) -> Vec<String> {
        match self {
            Self::RotatePages { page_indices, .. } => page_indices
                .iter()
                .map(|idx| format!("page:{idx}"))
                .collect(),
            Self::AddHighlightAnnotation { annotation } => {
                vec![format!("annot:{}", annotation.annotation_id)]
            }
            Self::RemoveHighlightAnnotation { annotation_id, .. } => {
                vec![format!("annot:{annotation_id}")]
            }
            Self::AddCommentAnnotation { annotation } => {
                vec![format!("comment:{}", annotation.annotation_id)]
            }
            Self::RemoveCommentAnnotation { annotation_id, .. } => {
                vec![format!("comment:{annotation_id}")]
            }
            Self::RedactPageRegions { page_index, .. } => {
                vec![format!("page:{page_index}:redaction")]
            }
            Self::SanitizeDocument { .. } => {
                vec!["document:sanitize".to_owned()]
            }
            Self::ReorderPages { .. } => vec!["document:reorder_pages".to_owned()],
            Self::DuplicatePage { page_index, .. } => {
                vec![format!("page:{page_index}:duplicate")]
            }
            Self::DeletePages { page_indices, .. } => page_indices
                .iter()
                .map(|idx| format!("page:{idx}:delete"))
                .collect(),
            Self::RestorePages { .. } => vec!["document:restore_pages".to_owned()],
            Self::InsertPagesFromDocument { insert_at, .. } => {
                vec![format!("document:insert_pages_at:{insert_at}")]
            }
            Self::MergeDocuments { .. } => vec!["document:merge".to_owned()],
            Self::Irreversible { .. } => Vec::new(),
        }
    }
}

/// Immutable, serializable transaction record (`ARCHITECTURE.md` §11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub transaction_id: String,
    pub sequence_number: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub timestamp_unix_ms: u64,
    pub description: String,
    pub affected_pages: Vec<u32>,
    pub affected_objects: Vec<String>,
    pub forward_command: TransactionCommand,
    pub inverse_command: TransactionCommand,
    pub before_pdf_sha256: String,
    pub after_pdf_sha256: String,
    pub record_checksum_sha256: String,
}

impl TransactionRecord {
    pub fn compute_checksum(&self) -> String {
        let payload = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            self.transaction_id,
            self.sequence_number,
            self.revision_before,
            self.revision_after,
            self.timestamp_unix_ms,
            self.description,
            self.before_pdf_sha256,
            self.after_pdf_sha256
        );
        sha256_hex(payload.as_bytes())
    }

    pub fn verify_checksum(&self) -> bool {
        self.compute_checksum() == self.record_checksum_sha256
    }
}

pub use crate::journal::PersistentTransactionJournal;

/// Live Undo/Redo state snapshot exposed to native UI across UniFFI (`ROADMAP.md` M0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionHistorySnapshot {
    pub current_revision: u64,
    pub saved_revision: u64,
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_description: Option<String>,
    pub redo_description: Option<String>,
    pub undo_count: u32,
    pub redo_count: u32,
    pub committed_records: Vec<TransactionRecord>,
    pub redo_records: Vec<TransactionRecord>,
}

/// Sole authority for mutating a `DocumentSession` (`ARCHITECTURE.md` §§6, 11, `ROADMAP.md` M0.4 Gate).
#[derive(Debug)]
pub struct TransactionManager {
    document_id: DocumentId,
    base_pdf_sha256: String,
    current_revision: u64,
    saved_revision: u64,
    next_sequence: u64,
    undo_stack: Vec<TransactionRecord>,
    redo_stack: Vec<TransactionRecord>,
}

impl TransactionManager {
    pub fn new(document_id: DocumentId, initial_pdf_bytes: &[u8]) -> Self {
        Self {
            document_id,
            base_pdf_sha256: sha256_hex(initial_pdf_bytes),
            current_revision: 1,
            saved_revision: 1,
            next_sequence: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn current_revision(&self) -> u64 {
        self.current_revision
    }

    pub fn is_dirty(&self) -> bool {
        self.current_revision != self.saved_revision
    }

    /// Marks the current document revision as cleanly saved and updates the baseline identity (`ARCHITECTURE.md` §16, `ROADMAP.md` M0.5).
    pub fn mark_as_saved(&mut self, document_id: DocumentId, base_pdf_sha256: String) {
        self.document_id = document_id;
        self.base_pdf_sha256 = base_pdf_sha256;
        self.saved_revision = self.current_revision;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn history_snapshot(&self) -> TransactionHistorySnapshot {
        TransactionHistorySnapshot {
            current_revision: self.current_revision,
            saved_revision: self.saved_revision,
            is_dirty: self.is_dirty(),
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
            undo_description: self.undo_stack.last().map(|r| r.description.clone()),
            redo_description: self.redo_stack.last().map(|r| r.description.clone()),
            undo_count: self.undo_stack.len() as u32,
            redo_count: self.redo_stack.len() as u32,
            committed_records: self.undo_stack.clone(),
            redo_records: self.redo_stack.clone(),
        }
    }

    /// Executes a semantic `TransactionCommand` against `actor`, records the forward and inverse commands,
    /// clears the redo stack, increments the document revision, and returns the committed `TransactionRecord`.
    pub fn execute_command(
        &mut self,
        actor: &DocumentActor,
        command: TransactionCommand,
    ) -> Result<TransactionRecord, TransactionError> {
        let before_bytes = actor.current_pdf_bytes()?;
        let before_pdf_sha256 = sha256_hex(&before_bytes);

        let (normalized_forward, inverse_command, after_bytes) =
            Self::apply_command_on_actor(actor, command)?;
        let after_pdf_sha256 = sha256_hex(&after_bytes);

        let seq = self.next_sequence;
        self.next_sequence += 1;
        let revision_before = self.current_revision;
        let revision_after = revision_before + 1;
        self.current_revision = revision_after;

        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(1_760_000_000_000);

        let short_hash = &after_pdf_sha256[..8.min(after_pdf_sha256.len())];
        let transaction_id = format!("tx_{seq:04}_{short_hash}");
        let description = normalized_forward.user_description();
        let affected_pages = normalized_forward.affected_pages();
        let affected_objects = normalized_forward.affected_objects();

        let mut record = TransactionRecord {
            transaction_id,
            sequence_number: seq,
            revision_before,
            revision_after,
            timestamp_unix_ms,
            description,
            affected_pages,
            affected_objects,
            forward_command: normalized_forward,
            inverse_command,
            before_pdf_sha256,
            after_pdf_sha256,
            record_checksum_sha256: String::new(),
        };
        record.record_checksum_sha256 = record.compute_checksum();

        self.undo_stack.push(record.clone());
        self.redo_stack.clear();
        Ok(record)
    }

    /// Undoes the most recent transaction on the undo stack, pushing it onto the redo stack (`UNDO-001`, `UNDO-004`).
    pub fn undo(&mut self, actor: &DocumentActor) -> Result<TransactionRecord, TransactionError> {
        let record = self
            .undo_stack
            .pop()
            .ok_or(TransactionError::NothingToUndo)?;

        if matches!(
            record.forward_command,
            TransactionCommand::RedactPageRegions { .. }
        ) {
            // The removed content is gone from the document. Undoing it would
            // be a lie, so the command stays on the stack and the refusal is
            // reported.
            self.undo_stack.push(record);
            return Err(TransactionError::Irreversible {
                description:
                    "A redaction cannot be undone: the removed content is gone from the document."
                        .to_owned(),
            });
        }

        let (_, _, _) = Self::apply_command_on_actor(actor, record.inverse_command.clone())?;

        // When undo stack becomes empty and we were originally at saved_revision = 1, return to saved_revision
        // so `is_dirty` accurately reflects clean state when all mutations are undone (`UNDO-002`).
        if self.undo_stack.is_empty() && self.saved_revision == 1 {
            self.current_revision = 1;
        } else {
            self.current_revision += 1;
        }

        self.redo_stack.push(record.clone());
        Ok(record)
    }

    /// Redoes the most recently undone transaction from the redo stack (`UNDO-001`, `UNDO-004`).
    pub fn redo(&mut self, actor: &DocumentActor) -> Result<TransactionRecord, TransactionError> {
        let record = self
            .redo_stack
            .pop()
            .ok_or(TransactionError::NothingToRedo)?;

        let (_, _, _) = Self::apply_command_on_actor(actor, record.forward_command.clone())?;
        self.current_revision += 1;
        self.undo_stack.push(record.clone());
        Ok(record)
    }

    /// Removes the most recent transaction from the undo stack (`REDACT-002`).
    ///
    /// A redaction that cannot be verified is rolled back: the document is put
    /// back as it was. The attempt must not leave a transaction behind — the
    /// journal of the recovery store would otherwise replay a redaction that
    /// never happened.
    ///
    /// The revision goes back as well: the bytes of the document are the old
    /// ones, and a document that is marked as changed although it is the
    /// saved one would make the user save a document that nobody touched.
    pub fn discard_last_transaction(&mut self) -> Option<TransactionRecord> {
        let record = self.undo_stack.pop();
        if let Some(record) = &record {
            self.redo_stack.clear();
            self.current_revision = record.revision_before;
        }
        record
    }

    /// Exports a versioned, checksum-verified `PersistentTransactionJournal` (`ARCHITECTURE.md` §§11–12).
    pub fn export_persistent_journal(&self) -> PersistentTransactionJournal {
        let journal_checksum_sha256 = PersistentTransactionJournal::compute_journal_checksum(
            TRANSACTION_JOURNAL_SCHEMA_VERSION,
            self.document_id.as_str(),
            &self.base_pdf_sha256,
            self.current_revision,
            self.saved_revision,
            &self.undo_stack,
            &self.redo_stack,
        );
        PersistentTransactionJournal {
            schema_version: TRANSACTION_JOURNAL_SCHEMA_VERSION,
            document_id: self.document_id.as_str().to_owned(),
            base_pdf_sha256: self.base_pdf_sha256.clone(),
            current_revision: self.current_revision,
            saved_revision: self.saved_revision,
            committed_records: self.undo_stack.clone(),
            redo_records: self.redo_stack.clone(),
            journal_checksum_sha256,
        }
    }

    /// Verifies and replays a `PersistentTransactionJournal` onto a freshly opened base document (`UNDO-001`).
    pub fn restore_and_replay_journal(
        &mut self,
        actor: &DocumentActor,
        journal: &PersistentTransactionJournal,
    ) -> Result<TransactionHistorySnapshot, TransactionError> {
        journal.verify_integrity()?;
        if journal.document_id != self.document_id.as_str() {
            return Err(TransactionError::IntegrityMismatch {
                details: format!(
                    "journal document_id '{}' does not match session document_id '{}'",
                    journal.document_id,
                    self.document_id.as_str()
                ),
            });
        }
        let current_bytes = actor.current_pdf_bytes()?;
        let current_sha = sha256_hex(&current_bytes);
        if current_sha != journal.base_pdf_sha256 {
            return Err(TransactionError::IntegrityMismatch {
                details: "base PDF SHA-256 does not match persistent transaction journal"
                    .to_owned(),
            });
        }

        // A redaction is not replayed. It is irreversible, and the journal
        // cannot say whether it was ever verified: the process may have died
        // between the removal and the verification. Replaying it would put
        // content into the document without a proof. The document is restored
        // to the state before the redaction, and the caller has to say so.
        for rec in &journal.committed_records {
            if matches!(
                rec.forward_command,
                TransactionCommand::RedactPageRegions { .. }
            ) {
                continue;
            }
            let _ = Self::apply_command_on_actor(actor, rec.forward_command.clone())?;
        }

        self.undo_stack = journal
            .committed_records
            .iter()
            .filter(|record| {
                !matches!(
                    record.forward_command,
                    TransactionCommand::RedactPageRegions { .. }
                )
            })
            .cloned()
            .collect();
        self.redo_stack = journal.redo_records.clone();
        // The revision has to describe the state of the document, not the state
        // of the journal. A redaction was not replayed, so the document stands
        // one step back: every redaction in the journal has consumed a
        // revision, and that revision belongs to a state the document does not
        // have any more. Without the correction the document would claim a
        // revision it never reached, and the next transaction would be
        // numbered as if it were the second one.
        let dropped_redactions = journal
            .committed_records
            .iter()
            .filter(|record| {
                matches!(
                    record.forward_command,
                    TransactionCommand::RedactPageRegions { .. }
                )
            })
            .count() as u64;
        self.current_revision = journal.current_revision.saturating_sub(dropped_redactions);
        self.saved_revision = journal.saved_revision;
        self.next_sequence = self
            .undo_stack
            .iter()
            .chain(self.redo_stack.iter())
            .map(|r| r.sequence_number)
            .max()
            .unwrap_or(0)
            + 1;

        Ok(self.history_snapshot())
    }

    fn apply_command_on_actor(
        actor: &DocumentActor,
        command: TransactionCommand,
    ) -> Result<(TransactionCommand, TransactionCommand, Vec<u8>), TransactionError> {
        match command {
            TransactionCommand::RotatePages {
                page_indices,
                delta_degrees,
                allow_signature_invalidation,
            } => {
                if page_indices.is_empty() {
                    return Err(TransactionError::EmptyPageSelection);
                }
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| {
                        doc.rotate_pages_ext(
                            &page_indices,
                            delta_degrees,
                            allow_signature_invalidation,
                        )
                    },
                )?;
                let forward = TransactionCommand::RotatePages {
                    page_indices: page_indices.clone(),
                    delta_degrees,
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::RotatePages {
                    page_indices,
                    delta_degrees: -delta_degrees,
                    allow_signature_invalidation: true,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::AddHighlightAnnotation { annotation } => {
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| doc.add_highlight_annotation(&annotation),
                )?;
                let inverse = TransactionCommand::RemoveHighlightAnnotation {
                    page_index: annotation.page_index,
                    annotation_id: annotation.annotation_id.clone(),
                    restored_annotation: Some(annotation.clone()),
                };
                let forward = TransactionCommand::AddHighlightAnnotation { annotation };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::AddCommentAnnotation { annotation } => {
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| doc.add_comment_annotation(&annotation),
                )?;
                let inverse = TransactionCommand::RemoveCommentAnnotation {
                    page_index: annotation.page_index,
                    annotation_id: annotation.annotation_id.clone(),
                    restored_annotation: Some(annotation.clone()),
                };
                let forward = TransactionCommand::AddCommentAnnotation { annotation };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::RemoveCommentAnnotation {
                page_index,
                annotation_id,
                ..
            } => {
                let (removed_annot, updated_bytes) = actor
                    .apply_engine_mutation_for_transaction_manager(
                        |doc: &mut dyn PdfEngineDocument| {
                            doc.remove_comment_annotation(page_index, &annotation_id)
                        },
                    )?;
                let forward = TransactionCommand::RemoveCommentAnnotation {
                    page_index,
                    annotation_id,
                    restored_annotation: Some(removed_annot.clone()),
                };
                let inverse = TransactionCommand::AddCommentAnnotation {
                    annotation: removed_annot,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::RedactPageRegions {
                page_index,
                regions,
            } => {
                if regions.is_empty() {
                    return Err(TransactionError::EmptyPageSelection);
                }
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| doc.redact_page_regions(page_index, &regions),
                )?;
                let forward = TransactionCommand::RedactPageRegions {
                    page_index,
                    regions: regions.clone(),
                };
                // The removed content is gone from the document. It cannot be
                // restored, so the inverse of the command is the refusal to
                // undo it.
                let inverse = TransactionCommand::Irreversible {
                    description: format!(
                        "The content of page #{} cannot be restored",
                        page_index + 1
                    ),
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::SanitizeDocument { options } => {
                let (_report, updated_bytes) = actor
                    .apply_engine_mutation_for_transaction_manager(
                        |doc: &mut dyn PdfEngineDocument| doc.sanitize_document(&options),
                    )?;
                let forward = TransactionCommand::SanitizeDocument { options };
                let inverse = TransactionCommand::Irreversible {
                    description: "Sanitized document content cannot be restored".to_owned(),
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::Irreversible { description } => {
                Err(TransactionError::Irreversible {
                    description: description.clone(),
                })
            }
            TransactionCommand::RemoveHighlightAnnotation {
                page_index,
                annotation_id,
                ..
            } => {
                let (removed_annot, updated_bytes) = actor
                    .apply_engine_mutation_for_transaction_manager(
                        |doc: &mut dyn PdfEngineDocument| {
                            doc.remove_highlight_annotation(page_index, &annotation_id)
                        },
                    )?;
                let forward = TransactionCommand::RemoveHighlightAnnotation {
                    page_index,
                    annotation_id,
                    restored_annotation: Some(removed_annot.clone()),
                };
                let inverse = TransactionCommand::AddHighlightAnnotation {
                    annotation: removed_annot,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::ReorderPages {
                order,
                allow_signature_invalidation,
            } => {
                let inverse_order = crate::workbench::compute_inverse_permutation(&order)?;
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.reorder_pages_ext(&order, allow_signature_invalidation)
                })?;
                let forward = TransactionCommand::ReorderPages {
                    order: order.clone(),
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::ReorderPages {
                    order: inverse_order,
                    allow_signature_invalidation: true,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::DuplicatePage {
                page_index,
                allow_signature_invalidation,
            } => {
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.duplicate_page_ext(page_index, allow_signature_invalidation)
                })?;
                let forward = TransactionCommand::DuplicatePage {
                    page_index,
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::DeletePages {
                    page_indices: vec![page_index + 1],
                    allow_signature_invalidation: true,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::DeletePages {
                page_indices,
                allow_signature_invalidation,
            } => {
                let before_bytes = actor.current_pdf_bytes()?;
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.delete_pages_ext(&page_indices, allow_signature_invalidation)
                })?;
                let forward = TransactionCommand::DeletePages {
                    page_indices,
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::RestorePages {
                    previous_bytes: before_bytes,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::RestorePages { previous_bytes } => {
                let before_bytes = actor.current_pdf_bytes()?;
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.replace_pdf_bytes(previous_bytes.clone())?;
                    Ok(previous_bytes.clone())
                })?;
                let forward = TransactionCommand::RestorePages { previous_bytes };
                let inverse = TransactionCommand::RestorePages {
                    previous_bytes: before_bytes,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::InsertPagesFromDocument {
                source_pdf,
                source_page_indices,
                insert_at,
                allow_signature_invalidation,
            } => {
                let before_bytes = actor.current_pdf_bytes()?;
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.insert_pages_ext(
                        &source_pdf,
                        &source_page_indices,
                        insert_at,
                        allow_signature_invalidation,
                    )
                })?;
                let forward = TransactionCommand::InsertPagesFromDocument {
                    source_pdf,
                    source_page_indices,
                    insert_at,
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::RestorePages {
                    previous_bytes: before_bytes,
                };
                Ok((forward, inverse, updated_bytes))
            }
            TransactionCommand::MergeDocuments {
                source_pdfs,
                allow_signature_invalidation,
            } => {
                let before_bytes = actor.current_pdf_bytes()?;
                let doc_slices: Vec<&[u8]> = source_pdfs.iter().map(|b| b.as_slice()).collect();
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(|doc| {
                    doc.merge_documents_ext(&doc_slices, allow_signature_invalidation)
                })?;
                let forward = TransactionCommand::MergeDocuments {
                    source_pdfs,
                    allow_signature_invalidation,
                };
                let inverse = TransactionCommand::RestorePages {
                    previous_bytes: before_bytes,
                };
                Ok((forward, inverse, updated_bytes))
            }
        }
    }
}

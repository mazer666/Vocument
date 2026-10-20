//! `TransactionManager`, semantic mutation commands, Undo/Redo stack, and persistent transaction journal (`ARCHITECTURE.md` §§6, 11–12, `ROADMAP.md` M0.4).
//!
//! Gate (`ROADMAP.md` M0.4): **Mutation never bypasses `TransactionManager`.**
//! Every document mutation executes through `TransactionManager::execute_command`, `undo`, or `redo`.

use crate::identity::{DocumentId, compute_sha256_hex as sha256_hex};
use crate::pdf_engine::{
    PdfEngineDocument, PdfEngineError, PdfHighlightAnnotation, PdfRectMilliPt,
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
    /// The inverse of a command that cannot be undone (`REDACT-001`).
    Irreversible { description: String },
}

impl TransactionCommand {
    /// Human-readable user-facing description (`UNDO-004`: human-level operations, not engine microsteps).
    pub fn user_description(&self) -> String {
        match self {
            Self::RotatePages {
                page_indices,
                delta_degrees,
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
            Self::Irreversible { description } => description.clone(),
        }
    }

    pub fn affected_pages(&self) -> Vec<u32> {
        match self {
            Self::RotatePages { page_indices, .. } => page_indices.clone(),
            Self::AddHighlightAnnotation { annotation } => vec![annotation.page_index],
            Self::RemoveHighlightAnnotation { page_index, .. } => vec![*page_index],
            Self::RedactPageRegions { page_index, .. } => vec![*page_index],
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
            Self::RedactPageRegions { page_index, .. } => {
                vec![format!("redaction:page:{page_index}")]
            }
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

/// Persistent transaction journal representation (`ARCHITECTURE.md` §§11–12, `UNDO-001`, `ROADMAP.md` M0.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistentTransactionJournal {
    pub schema_version: u32,
    pub document_id: String,
    pub base_pdf_sha256: String,
    pub current_revision: u64,
    pub saved_revision: u64,
    pub committed_records: Vec<TransactionRecord>,
    pub redo_records: Vec<TransactionRecord>,
    pub journal_checksum_sha256: String,
}

impl PersistentTransactionJournal {
    pub fn compute_journal_checksum(
        schema_version: u32,
        document_id: &str,
        base_pdf_sha256: &str,
        current_revision: u64,
        saved_revision: u64,
        committed_records: &[TransactionRecord],
        redo_records: &[TransactionRecord],
    ) -> String {
        let mut material = format!(
            "v={schema_version}|doc={document_id}|base={base_pdf_sha256}|cur={current_revision}|saved={saved_revision}|committed="
        );
        for rec in committed_records {
            material.push_str(&rec.record_checksum_sha256);
            material.push(',');
        }
        material.push_str("|redo=");
        for rec in redo_records {
            material.push_str(&rec.record_checksum_sha256);
            material.push(',');
        }
        sha256_hex(material.as_bytes())
    }

    /// Verifies schema version, every individual record checksum, and the overall journal checksum (`ARCHITECTURE.md` §12).
    pub fn verify_integrity(&self) -> Result<(), TransactionError> {
        if self.schema_version != TRANSACTION_JOURNAL_SCHEMA_VERSION {
            return Err(TransactionError::IntegrityMismatch {
                details: format!(
                    "unsupported journal schema_version {} (expected {})",
                    self.schema_version, TRANSACTION_JOURNAL_SCHEMA_VERSION
                ),
            });
        }
        for rec in self
            .committed_records
            .iter()
            .chain(self.redo_records.iter())
        {
            if !rec.verify_checksum() {
                return Err(TransactionError::IntegrityMismatch {
                    details: format!(
                        "record checksum mismatch on transaction '{}'",
                        rec.transaction_id
                    ),
                });
            }
        }
        let expected = Self::compute_journal_checksum(
            self.schema_version,
            &self.document_id,
            &self.base_pdf_sha256,
            self.current_revision,
            self.saved_revision,
            &self.committed_records,
            &self.redo_records,
        );
        if expected != self.journal_checksum_sha256 {
            return Err(TransactionError::IntegrityMismatch {
                details: "overall journal_checksum_sha256 mismatch".to_owned(),
            });
        }
        Ok(())
    }

    /// Serializes this persistent transaction journal to pretty-printed JSON (`ARCHITECTURE.md` §12).
    pub fn to_json_pretty(&self) -> Result<String, TransactionError> {
        serde_json::to_string_pretty(self).map_err(|err| TransactionError::SerializationFailure {
            details: err.to_string(),
        })
    }

    /// Parses and integrity-verifies a `PersistentTransactionJournal` from JSON.
    pub fn from_json_verified(json: &str) -> Result<Self, TransactionError> {
        let parsed: Self =
            serde_json::from_str(json).map_err(|err| TransactionError::SerializationFailure {
                details: err.to_string(),
            })?;
        parsed.verify_integrity()?;
        Ok(parsed)
    }
}

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

        for rec in &journal.committed_records {
            let _ = Self::apply_command_on_actor(actor, rec.forward_command.clone())?;
        }

        self.undo_stack = journal.committed_records.clone();
        self.redo_stack = journal.redo_records.clone();
        self.current_revision = journal.current_revision;
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
            } => {
                if page_indices.is_empty() {
                    return Err(TransactionError::EmptyPageSelection);
                }
                let updated_bytes = actor.apply_engine_mutation_for_transaction_manager(
                    |doc: &mut dyn PdfEngineDocument| {
                        doc.rotate_pages(&page_indices, delta_degrees)
                    },
                )?;
                let forward = TransactionCommand::RotatePages {
                    page_indices: page_indices.clone(),
                    delta_degrees,
                };
                let inverse = TransactionCommand::RotatePages {
                    page_indices,
                    delta_degrees: -delta_degrees,
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
        }
    }
}

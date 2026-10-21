//! Proof that the crash recovery does not replay a redaction (`REDACT-002`).
//!
//! A redaction is irreversible, and the journal cannot say whether it was
//! ever verified: the process may have died between the removal and the
//! verification. Replaying it would put content into the document without a
//! proof — the document would carry a redaction that nobody checked.
//!
//! The recovery therefore restores the state **before** the redaction and
//! says so.

use vocument_core::identity::compute_sha256_hex;
use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngine, PdfEngineDocument, PdfEngineError,
    PdfHighlightAnnotation, PdfPageGeometry, PdfPageRaster, PdfPageStructuredText, PdfRectMilliPt,
};
use vocument_core::render::DocumentActor;
use vocument_core::transactions::{
    PersistentTransactionJournal, TRANSACTION_JOURNAL_SCHEMA_VERSION, TransactionCommand,
    TransactionManager, TransactionRecord,
};

/// The bytes of the document.
fn sample_pdf() -> Vec<u8> {
    b"%PDF-1.7\nsample".to_vec()
}

/// An engine that refuses the redaction — the way an engine that cannot
/// remove anything would.
#[derive(Debug)]
struct PlainEngine;

impl PdfEngine for PlainEngine {
    fn engine_identifier(&self) -> &str {
        "mock-plain"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        Ok(Box::new(PlainDocument {
            bytes: bytes.to_vec(),
        }))
    }
}

#[derive(Debug)]
struct PlainDocument {
    bytes: Vec<u8>,
}

impl PdfEngineDocument for PlainDocument {
    fn summary(&self) -> &PdfDocumentSummary {
        static SUMMARY: std::sync::OnceLock<PdfDocumentSummary> = std::sync::OnceLock::new();
        SUMMARY.get_or_init(|| PdfDocumentSummary {
            page_count: 1,
            format_version: Some("1.7".to_owned()),
            title: None,
            author: None,
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            modification_date: None,
            needs_password: false,
            engine_identifier: "mock-plain".to_owned(),
        })
    }

    fn current_pdf_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
        Ok(PdfPageGeometry {
            page_index,
            width_milli_pt: 200_000,
            height_milli_pt: 200_000,
        })
    }

    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        Ok(PdfPageStructuredText {
            page_index,
            embedded_page_label: None,
            words: Vec::new(),
            lines: Vec::new(),
        })
    }

    fn create_display_list(
        &self,
        _page_index: u32,
    ) -> Result<Box<dyn vocument_core::pdf_engine::PdfEngineDisplayList>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the mock has no display list".to_owned(),
        })
    }

    fn render_page_gray(
        &self,
        _page_index: u32,
        _scale_milli: u32,
    ) -> Result<PdfPageRaster, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the mock has no rendering".to_owned(),
        })
    }

    fn page_rotation_degrees(&self, _page_index: u32) -> Result<u32, PdfEngineError> {
        Ok(0)
    }

    fn page_highlight_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
        Ok(Vec::new())
    }

    /// The comments of the page (`ANNOT-001`). The test engine does not carry
    /// comments — it is a stand-in for the real engine, and it says so
    /// instead of pretending that it succeeded.
    fn page_comment_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Writes a comment (`ANNOT-001`).
    fn add_comment_annotation(
        &mut self,
        _annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Removes a comment (`ANNOT-001`).
    fn remove_comment_annotation(
        &mut self,
        _page_index: u32,
        _annotation_id: &str,
    ) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
        Ok(None)
    }

    fn rotate_pages(
        &mut self,
        _page_indices: &[u32],
        _delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Ok(self.bytes.clone())
    }

    fn add_highlight_annotation(
        &mut self,
        _annotation: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Ok(self.bytes.clone())
    }

    fn remove_highlight_annotation(
        &mut self,
        _page_index: u32,
        _annotation_id: &str,
    ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the mock has no annotations".to_owned(),
        })
    }
}

/// A journal whose last transaction is a redaction.
fn journal_with_a_redaction(document_id: &str, base_sha256: &str) -> PersistentTransactionJournal {
    let mut record = TransactionRecord {
        transaction_id: "tx-redaction".to_owned(),
        sequence_number: 1,
        revision_before: 1,
        revision_after: 2,
        timestamp_unix_ms: 1_700_000_000_000,
        description: "Remove the content of 1 region(s) on page #1".to_owned(),
        affected_pages: vec![0],
        affected_objects: vec!["redaction:page:0".to_owned()],
        forward_command: TransactionCommand::RedactPageRegions {
            page_index: 0,
            regions: vec![PdfRectMilliPt {
                x0_milli_pt: 10,
                y0_milli_pt: 10,
                x1_milli_pt: 50,
                y1_milli_pt: 50,
            }],
        },
        inverse_command: TransactionCommand::Irreversible {
            description: "A redaction cannot be undone.".to_owned(),
        },
        before_pdf_sha256: base_sha256.to_owned(),
        after_pdf_sha256: base_sha256.to_owned(),
        record_checksum_sha256: String::new(),
    };
    record.record_checksum_sha256 = record.compute_checksum();

    let records = vec![record];
    let journal_checksum_sha256 = PersistentTransactionJournal::compute_journal_checksum(
        TRANSACTION_JOURNAL_SCHEMA_VERSION,
        document_id,
        base_sha256,
        2,
        1,
        &records,
        &[],
    );
    PersistentTransactionJournal {
        schema_version: TRANSACTION_JOURNAL_SCHEMA_VERSION,
        document_id: document_id.to_owned(),
        base_pdf_sha256: base_sha256.to_owned(),
        current_revision: 2,
        saved_revision: 1,
        committed_records: records,
        redo_records: Vec::new(),
        journal_checksum_sha256,
    }
}

/// The recovery does not replay a redaction: the document stands at the state
/// before it, and the redaction is not in the undo stack.
#[test]
fn the_recovery_does_not_replay_a_redaction() {
    let bytes = sample_pdf();
    let document = PlainEngine
        .open_pdf_bytes(&bytes)
        .expect("the document must be openable");
    let actor = DocumentActor::new(document);
    let base_sha256 = compute_sha256_hex(&bytes);
    let document_id = vocument_core::identity::DocumentId::from_sha256_hex(&base_sha256);
    let mut manager = TransactionManager::new(document_id.clone(), &bytes);
    let journal = journal_with_a_redaction(document_id.as_str(), &base_sha256);

    // The restore of the journal must not fail: the redaction is not applied.
    // An engine that refuses the redaction would report an error if the
    // recovery tried to apply it.
    let history = manager
        .restore_and_replay_journal(&actor, &journal)
        .expect("the recovery must not replay the redaction");

    // The redaction is not in the undo stack: it was not applied.
    assert!(
        !history
            .committed_records
            .iter()
            .any(|entry| entry.forward_command.is_redaction()),
        "the redaction must not be in the undo stack: {:?}",
        history.committed_records
    );

    // The document still holds its bytes.
    assert_eq!(
        actor
            .current_pdf_bytes()
            .expect("the bytes must be readable"),
        bytes.as_slice(),
        "the document must stand at the state before the redaction"
    );
}

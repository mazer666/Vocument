//! Proof that a redaction that cannot be verified leaves the document alone
//! (`REDACT-002`, `REDACT-005`).
//!
//! Two things have to hold at the same time:
//!
//! 1. A redaction that **succeeds** deletes the checkpoints of the recovery
//!    store — they hold the bytes of the earlier states of the document, and
//!    the removed content is still in them.
//! 2. A redaction that **fails** its verification changes nothing: the bytes
//!    of the document are the bytes it had before, and the checkpoints are
//!    still there. The user must not hold a changed document while believing
//!    that nothing happened.
//!
//! The engine is a mock that behaves like an engine whose removal leaves the
//! text behind — the case that the real engine produces for a Type 3 font.

use std::sync::Arc;

use vocument_core::file_broker::{BrokeredFileRead, FileAccessOrigin, ScopedLocalFileBroker};
use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument,
    PdfEngineError, PdfHighlightAnnotation, PdfPageGeometry, PdfPageRaster, PdfPageStructuredText,
    PdfRectMilliPt, PdfTextOccurrence, PdfTextWord,
};
use vocument_core::recovery::RecoveryStore;
use vocument_core::redaction::RedactionRegion;
use vocument_core::rendered_page::RenderedPageReader;
use vocument_core::session::SessionRegistry;

/// The word on the page.
const SECRET: &str = "VERTRAULICH";

/// The region that covers the word.
fn region() -> RedactionRegion {
    RedactionRegion::new(10_000, 10_000, 90_000, 30_000)
}

/// An engine whose removal leaves the text behind.
///
/// The bytes of the document change — the removal did happen — but the text
/// is still in the document, which is exactly what the engine of a Type 3
/// font does.
#[derive(Debug)]
struct LeakingEngine {
    /// True if the engine reports the surviving text.
    leaks: bool,
}

impl PdfEngine for LeakingEngine {
    fn engine_identifier(&self) -> &str {
        "mock-leaking"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        Ok(Box::new(LeakingDocument {
            bytes: bytes.to_vec(),
            leaks: self.leaks,
            redacted: false,
        }))
    }
}

#[derive(Debug)]
struct LeakingDocument {
    bytes: Vec<u8>,
    leaks: bool,
    /// True after the removal: the text layer of the page is empty then.
    redacted: bool,
}

impl PdfEngineDocument for LeakingDocument {
    fn summary(&self) -> &PdfDocumentSummary {
        // The summary is taken once when the session is opened; a static
        // value is enough for the test.
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
            engine_identifier: "mock-leaking".to_owned(),
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
        let words = if self.redacted {
            Vec::new()
        } else {
            vec![PdfTextWord {
                text: SECRET.to_owned(),
                bbox: PdfRectMilliPt {
                    x0_milli_pt: 20_000,
                    y0_milli_pt: 20_000,
                    x1_milli_pt: 80_000,
                    y1_milli_pt: 25_000,
                },
            }]
        };
        Ok(PdfPageStructuredText {
            page_index,
            embedded_page_label: None,
            words,
            lines: Vec::new(),
        })
    }

    fn create_display_list(
        &self,
        _page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
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

    fn redact_page_regions(
        &mut self,
        _page_index: u32,
        _regions: &[PdfRectMilliPt],
    ) -> Result<Vec<u8>, PdfEngineError> {
        // The removal happens: the bytes of the document change, and the text
        // layer of the page is empty afterwards.
        let mut updated = self.bytes.clone();
        updated.extend_from_slice(b"\nredacted");
        self.bytes = updated.clone();
        self.redacted = true;
        Ok(updated)
    }

    fn find_word_occurrences(
        &self,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, PdfEngineError> {
        if !self.leaks {
            return Ok(Vec::new());
        }
        Ok(words
            .iter()
            .map(|word| PdfTextOccurrence {
                kind: "content".to_owned(),
                page_index: 0,
                location: word.clone(),
            })
            .collect())
    }

    fn replace_pdf_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PdfEngineError> {
        self.bytes = bytes;
        Ok(())
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

/// A reader that reads no words: the picture of the page carries no text
/// that the redaction would have to remove.
#[derive(Debug)]
struct SilentReader;

impl RenderedPageReader for SilentReader {
    fn read_rendered_words(
        &self,
        _pdf_bytes: &[u8],
        _page_index: u32,
    ) -> Result<Vec<PdfTextWord>, PdfEngineError> {
        Ok(Vec::new())
    }
}

/// Opens a session in a registry with its own recovery store.
fn open_session(leaks: bool, root: &std::path::Path) -> (SessionRegistry, String) {
    let registry = SessionRegistry::with_broker_and_recovery(
        Box::new(LeakingEngine { leaks }),
        Arc::new(ScopedLocalFileBroker::new()),
        Arc::new(RecoveryStore::new(root.to_path_buf())),
    )
    .with_rendered_page_reader(Arc::new(SilentReader));
    let snapshot = registry
        .open_brokered_file(BrokeredFileRead {
            source_token: "test:redaction".to_owned(),
            display_name: "redaction.pdf".to_owned(),
            access_origin: FileAccessOrigin::InMemoryBuffer,
            os_file_id: None,
            modified_unix_ms: None,
            bytes: b"%PDF-1.7\nbase".to_vec(),
        })
        .expect("the session must be openable");
    (registry, snapshot.session_id.as_str().to_owned())
}

fn temp_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("vocument_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("the temporary directory must be creatable");
    root
}

/// The number of checkpoints of the document.
fn checkpoint_count(registry: &SessionRegistry, session_id: &str) -> usize {
    let document_id = registry
        .session_snapshot(session_id)
        .expect("the snapshot must be readable")
        .document_id;
    let container = registry
        .recovery_store()
        .container_for(document_id.as_str());
    // The checkpoints of the container live in its own directory.
    let checkpoints = container.container_dir().join("checkpoints");
    std::fs::read_dir(checkpoints)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_name().to_string_lossy().ends_with(".pdf"))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn a_successful_redaction_deletes_the_checkpoints_of_the_recovery_store() {
    let root = temp_root("redaction_success");
    let (registry, session_id) = open_session(false, &root);

    // A checkpoint of the state before the redaction.
    let container = registry.recovery_store().container_for(
        registry
            .session_snapshot(&session_id)
            .expect("the snapshot must be readable")
            .document_id
            .as_str(),
    );
    container
        .write_checkpoint(1, b"%PDF-1.7\nbase")
        .expect("the checkpoint must be writable");
    assert_eq!(
        checkpoint_count(&registry, &session_id),
        1,
        "the checkpoint must be there before the redaction"
    );

    let report = registry
        .redact_page_regions(&session_id, 0, &[region()])
        .expect("the redaction must succeed");
    assert!(report.is_verified(), "the redaction must be verified");

    assert_eq!(
        checkpoint_count(&registry, &session_id),
        0,
        "the checkpoints must be deleted with the redaction: they still hold the removed content"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_redaction_that_cannot_be_verified_leaves_the_document_and_the_checkpoints_alone() {
    let root = temp_root("redaction_failure");
    let (registry, session_id) = open_session(true, &root);

    let container = registry.recovery_store().container_for(
        registry
            .session_snapshot(&session_id)
            .expect("the snapshot must be readable")
            .document_id
            .as_str(),
    );
    container
        .write_checkpoint(1, b"%PDF-1.7\nbase")
        .expect("the checkpoint must be writable");

    let result = registry.redact_page_regions(&session_id, 0, &[region()]);
    assert!(
        result.is_err(),
        "a redaction whose content survives must not be reported as successful"
    );

    // The document is the document it was before the attempt.
    let bytes = registry
        .export_session_bytes(&session_id)
        .expect("the bytes must be readable");
    assert_eq!(
        bytes,
        b"%PDF-1.7\nbase".to_vec(),
        "the document must be unchanged after a failed redaction"
    );

    // The checkpoint that holds the old bytes is still there — the removal
    // never happened, so nothing has to be deleted.
    assert_eq!(
        checkpoint_count(&registry, &session_id),
        1,
        "the checkpoints must survive a failed redaction"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A redaction that fails its verification puts the document back — and the
/// session back: the revision and the dirty flag have to be the ones of the
/// state before the attempt. A session that claims the revision of a
/// redaction that never happened would make the next save write a document
/// that the user never saw.
#[test]
fn a_redaction_that_fails_puts_the_revision_and_the_dirty_flag_back() {
    let root = temp_root("redaction_state");
    let (registry, session_id) = open_session(true, &root);

    let before = registry
        .session_snapshot(&session_id)
        .expect("the snapshot must be readable");
    assert_eq!(before.revision, 1, "the document starts at revision 1");
    assert!(!before.is_dirty, "the document starts without changes");

    let result = registry.redact_page_regions(&session_id, 0, &[region()]);
    assert!(
        result.is_err(),
        "the redaction must fail: the text survives the removal"
    );

    let after = registry
        .session_snapshot(&session_id)
        .expect("the snapshot must be readable");
    assert_eq!(
        after.revision, before.revision,
        "the revision must be the one of the state before the attempt"
    );
    assert!(
        !after.is_dirty,
        "the session must not claim unsaved changes after a rolled back redaction"
    );
    let _ = std::fs::remove_dir_all(&root);
}

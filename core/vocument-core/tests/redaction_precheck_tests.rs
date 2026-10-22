//! Proof that the **pre-check** of the redaction refuses before the document
//! is touched (`REDACT-001`).
//!
//! The redaction removes what the user marked and then proves that the text
//! is gone. The proof needs a **reference**: the list of the places that
//! carried the text before the removal. A place that the engine could not
//! examine makes that reference incomplete — and an incomplete reference is a
//! proof that proves nothing.
//!
//! The refusal therefore happens **before** the document is changed. A stream
//! that cannot be read is such a place: it is a place that could not be
//! examined, not a place that carries the text. The engine names it as
//! `uninspected`, and the pre-check refuses the redaction — without a
//! removal, without a rollback, without a change of a single byte.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// The number of the removals that were attempted.
type RemovalCounter = Arc<AtomicUsize>;

/// An engine whose document reports a place that it could **not** examine.
///
/// The place is the stream of an attached file whose content cannot be read:
/// the engine cannot look into it, and it says so.
#[derive(Debug)]
struct UnreadableStreamEngine {
    removals: RemovalCounter,
}

impl PdfEngine for UnreadableStreamEngine {
    fn engine_identifier(&self) -> &str {
        "mock-unreadable-stream"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        Ok(Box::new(UnreadableStreamDocument {
            bytes: bytes.to_vec(),
            removals: Arc::clone(&self.removals),
        }))
    }
}

#[derive(Debug)]
struct UnreadableStreamDocument {
    bytes: Vec<u8>,
    removals: RemovalCounter,
}

impl PdfEngineDocument for UnreadableStreamDocument {
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
            engine_identifier: "mock-unreadable-stream".to_owned(),
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
            words: vec![PdfTextWord {
                text: SECRET.to_owned(),
                bbox: PdfRectMilliPt {
                    x0_milli_pt: 20_000,
                    y0_milli_pt: 20_000,
                    x1_milli_pt: 80_000,
                    y1_milli_pt: 25_000,
                },
            }],
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
        self.removals.fetch_add(1, Ordering::SeqCst);
        let mut updated = self.bytes.clone();
        updated.extend_from_slice(b"\nredacted");
        self.bytes = updated.clone();
        Ok(updated)
    }

    /// The places of the word: the page carries the text, and the stream of
    /// the attached file **could not be examined**.
    fn find_word_occurrences(
        &self,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, PdfEngineError> {
        let mut places: Vec<PdfTextOccurrence> = words
            .iter()
            .map(|word| PdfTextOccurrence {
                kind: "content".to_owned(),
                page_index: 0,
                location: word.clone(),
            })
            .collect();
        places.push(PdfTextOccurrence {
            kind: "uninspected".to_owned(),
            page_index: -1,
            location: "the content of the stream could not be read completely".to_owned(),
        });
        Ok(places)
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

    fn page_comment_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    fn add_comment_annotation(
        &mut self,
        _annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

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

/// A reader that reads no words.
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

/// A place that the engine could not examine stops the redaction **before**
/// the document is touched.
///
/// The refusal is not a late one: the removal is not attempted at all, so
/// the document does not have to be put back. The bytes of the file are the
/// same before and after the refusal.
#[test]
fn a_place_that_could_not_be_examined_stops_the_redaction_before_the_document_is_touched() {
    let root = std::env::temp_dir().join(format!("vocument-precheck-{}", std::process::id()));
    let removals: RemovalCounter = Arc::new(AtomicUsize::new(0));
    let registry = SessionRegistry::with_broker_and_recovery(
        Box::new(UnreadableStreamEngine {
            removals: Arc::clone(&removals),
        }),
        Arc::new(ScopedLocalFileBroker::new()),
        Arc::new(RecoveryStore::new(root.clone())),
    )
    .with_rendered_page_reader(Arc::new(SilentReader));
    let bytes = format!("%PDF-1.7\n{SECRET} on the page").into_bytes();
    let snapshot = registry
        .open_brokered_file(BrokeredFileRead {
            source_token: "test:precheck".to_owned(),
            display_name: "precheck.pdf".to_owned(),
            access_origin: FileAccessOrigin::InMemoryBuffer,
            os_file_id: None,
            modified_unix_ms: None,
            bytes,
        })
        .expect("the session must be openable");
    let session_id = snapshot.session_id.as_str().to_owned();

    let error = registry
        .redact_page_regions(&session_id, 0, &[region()])
        .expect_err("a place that could not be examined stops the redaction");
    let details = error.to_string();
    assert!(
        details.contains("could not all be examined"),
        "the error names the reason: {details}"
    );
    assert!(
        details.contains("the content of the stream could not be read completely"),
        "the error names the place: {details}"
    );
    assert_eq!(
        removals.load(Ordering::SeqCst),
        0,
        "the document must not be touched before the refusal"
    );

    let _ = std::fs::remove_dir_all(&root);
}

//! Proof that a redaction names the places where the same word still stands
//! (`REDACT-001`).
//!
//! The redaction removes what the user marked. A word that occurs somewhere
//! else in the document is **not** part of the redaction — the user marked a
//! region of one page, not the word in the whole document. But the user has
//! to know about it:
//!
//! 1. The redaction is not refused because of it, otherwise no document with
//!    a word that occurs twice could be redacted at all.
//! 2. The report names the places, so that nobody believes that a word is
//!    gone from the document when it is still in it.
//! 3. A word that stands in the bytes of the file **without** a known place —
//!    an old version of the file, an unreachable object — still stops the
//!    redaction. That is the difference between a remainder and a word that
//!    belongs to the document.

use std::sync::Arc;

use vocument_core::file_broker::{BrokeredFileRead, FileAccessOrigin, ScopedLocalFileBroker};
use vocument_core::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError,
    PdfHighlightAnnotation, PdfPageGeometry, PdfPageRaster, PdfPageStructuredText, PdfRectMilliPt,
    PdfTextOccurrence, PdfTextWord,
};
use vocument_core::recovery::RecoveryStore;
use vocument_core::redaction::{RedactionCheck, RedactionRegion};
use vocument_core::rendered_page::RenderedPageReader;
use vocument_core::session::SessionRegistry;

/// The word on the page.
const SECRET: &str = "VERTRAULICH";

/// The region that covers the word.
fn region() -> RedactionRegion {
    RedactionRegion::new(10_000, 10_000, 90_000, 30_000)
}

/// An engine with two pages that both carry the word.
///
/// The word is removed from the first page only. It stays on the second one,
/// and it stays in the bytes of the file — the mock behaves like a real
/// document in which the word occurs twice.
#[derive(Debug)]
struct TwiceEngine {
    /// False if the engine cannot report the places of the word.
    reports_places: bool,
}

impl PdfEngine for TwiceEngine {
    fn engine_identifier(&self) -> &str {
        "mock-twice"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        Ok(Box::new(TwiceDocument {
            bytes: bytes.to_vec(),
            redacted: false,
            reports_places: self.reports_places,
        }))
    }
}

#[derive(Debug)]
struct TwiceDocument {
    bytes: Vec<u8>,
    /// True after the removal of the first page.
    redacted: bool,
    /// False if the engine cannot report the places of the word.
    reports_places: bool,
}

impl PdfEngineDocument for TwiceDocument {
    fn summary(&self) -> &PdfDocumentSummary {
        static SUMMARY: std::sync::OnceLock<PdfDocumentSummary> = std::sync::OnceLock::new();
        SUMMARY.get_or_init(|| PdfDocumentSummary {
            page_count: 2,
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
            engine_identifier: "mock-twice".to_owned(),
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
        let gone = self.redacted && page_index == 0;
        let words = if gone {
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
        if !self.reports_places {
            return Ok(Vec::new());
        }
        // After the removal the word is only on the second page. Before it, it
        // is on both.
        let pages: Vec<i32> = if self.redacted { vec![1] } else { vec![0, 1] };
        Ok(words
            .iter()
            .flat_map(|word| {
                pages.iter().map(move |page| PdfTextOccurrence {
                    kind: "content".to_owned(),
                    page_index: *page,
                    location: word.clone(),
                })
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

/// Opens a session in a registry with its own recovery store.
fn open_session(root: &std::path::Path) -> (SessionRegistry, String) {
    open_session_with(root, true)
}

/// Opens a session whose engine knows (or does not know) the places.
fn open_session_with(root: &std::path::Path, reports_places: bool) -> (SessionRegistry, String) {
    let registry = SessionRegistry::with_broker_and_recovery(
        Box::new(TwiceEngine { reports_places }),
        Arc::new(ScopedLocalFileBroker::new()),
        Arc::new(RecoveryStore::new(root.to_path_buf())),
    )
    .with_rendered_page_reader(Arc::new(SilentReader));
    let bytes = format!("%PDF-1.7\n{SECRET} on the first page and on the second one").into_bytes();
    let snapshot = registry
        .open_brokered_file(BrokeredFileRead {
            source_token: "test:twice".to_owned(),
            display_name: "twice.pdf".to_owned(),
            access_origin: FileAccessOrigin::InMemoryBuffer,
            os_file_id: None,
            modified_unix_ms: None,
            bytes,
        })
        .expect("the session must be openable");
    (registry, snapshot.session_id.as_str().to_owned())
}

/// The same word on another page does not stop the redaction, but it is
/// named in the report.
#[test]
fn the_same_word_on_another_page_is_named_in_the_report() {
    let root = std::env::temp_dir().join(format!("vocument-twice-{}", std::process::id()));
    let (registry, session_id) = open_session(&root);

    let report = registry
        .redact_page_regions(&session_id, 0, &[region()])
        .expect("the redaction succeeds");

    // The word is still in the document, on the second page.
    assert_eq!(
        report.remaining_occurrences.len(),
        1,
        "the place of the word is named: {:?}",
        report.remaining_occurrences
    );
    assert_eq!(report.remaining_occurrences[0].page_index, 1);
    assert_eq!(report.remaining_occurrences[0].kind, "content");

    // The word stands in the bytes of the file, but in a place the engine
    // knows. That is not a failure.
    assert_eq!(
        report.file_bytes_clean,
        RedactionCheck::Passed,
        "a known place is reported, not refused"
    );

    // The marked page is clean in every respect.
    assert_eq!(report.text_layer_clean, RedactionCheck::Passed);
    assert_eq!(report.search_index_clean, RedactionCheck::Passed);
    assert_eq!(report.rendered_page_clean, RedactionCheck::Passed);
    assert!(report.is_verified());

    let _ = std::fs::remove_dir_all(&root);
}

/// A remainder without a place stops the redaction — the word stands in the
/// bytes of the file and the engine does not know where.
///
/// That is the situation of a word in an old version of the file or in an
/// object that nothing points to any more. The engine cannot explain the
/// remainder, so the check over the bytes fails and the redaction is not
/// verified.
#[test]
fn a_remainder_without_a_place_still_stops_the_redaction() {
    let root = std::env::temp_dir().join(format!("vocument-remainder-{}", std::process::id()));
    let (registry, session_id) = open_session_with(&root, false);

    // The redaction is refused: the word stands in the bytes of the file and
    // nobody knows where. The caller gets an error, not a report that says
    // "done".
    let error = registry
        .redact_page_regions(&session_id, 0, &[region()])
        .expect_err("a remainder without a place stops the redaction");
    let details = error.to_string();
    assert!(
        details.contains("file bytes"),
        "the error names the check that failed: {details}"
    );

    // The caller gets no report: there is nothing to present as a successful
    // redaction. That the document itself is unchanged is proved in
    // `redaction_rollback_tests.rs`.

    let _ = std::fs::remove_dir_all(&root);
}

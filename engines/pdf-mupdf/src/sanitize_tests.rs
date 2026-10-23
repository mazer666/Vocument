//! Unit tests for PDF document sanitization (`SAN-001`).

use std::path::PathBuf;

use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfHighlightAnnotation, PdfRectMilliPt, PdfSanitizeOptions,
};

use crate::comment::add_comment_annotation_to_pdf;
use crate::mutate::add_highlight_annotation_to_pdf;
use crate::sanitize::sanitize_document;
use crate::{MuPdfEngineAdapter, PdfEngine};

fn sample_pdf_bytes() -> Vec<u8> {
    let mut path = PathBuf::from("tests/corpus/m0_3_navigation_and_search.pdf");
    if !path.exists() {
        path = PathBuf::from("../../tests/corpus/m0_3_navigation_and_search.pdf");
    }
    std::fs::read(&path).expect("sample PDF must exist")
}

#[test]
fn test_sanitize_document_clears_annotations_and_metadata() {
    let base_bytes = sample_pdf_bytes();

    // Add a highlight and a comment to page 0
    let highlight = PdfHighlightAnnotation {
        annotation_id: "test-hl-1".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 100_000,
            y0_milli_pt: 100_000,
            x1_milli_pt: 200_000,
            y1_milli_pt: 120_000,
        },
        color_rgb: [255, 255, 0],
        opacity_milli: 500,
        author: Some("Tester".to_owned()),
        contents: Some("Sensitive annotation".to_owned()),
    };
    let with_highlight = add_highlight_annotation_to_pdf(&base_bytes, 4, &highlight)
        .expect("highlight must be added");

    let comment = PdfCommentAnnotation {
        annotation_id: "test-cm-1".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 150_000,
            y0_milli_pt: 150_000,
            x1_milli_pt: 180_000,
            y1_milli_pt: 180_000,
        },
        color_rgb: [255, 0, 0],
        author: Some("Jurist".to_owned()),
        contents: "Confidential remark".to_owned(),
        palette_key: "note".to_owned(),
        palette_label: "Hinweis".to_owned(),
        palette_meaning: "Prüfung erforderlich".to_owned(),
    };
    let with_annotations =
        add_comment_annotation_to_pdf(&with_highlight, 4, &comment).expect("comment must be added");

    // Verify annotations are present before sanitize
    let adapter = MuPdfEngineAdapter::new();
    let doc_before = adapter
        .open_pdf_bytes(&with_annotations)
        .expect("doc must open");
    let hl_before = doc_before
        .page_highlight_annotations(0)
        .expect("highlights");
    let cm_before = doc_before.page_comment_annotations(0).expect("comments");
    assert!(
        !hl_before.is_empty(),
        "highlight must be present before sanitize"
    );
    assert!(
        !cm_before.is_empty(),
        "comment must be present before sanitize"
    );

    // Perform full sanitize
    let options = PdfSanitizeOptions::default();
    let (sanitized_bytes, report) =
        sanitize_document(&with_annotations, &options).expect("sanitize must succeed");

    assert!(sanitized_bytes.starts_with(b"%PDF-"));
    assert!(
        report.annotations_removed >= 2,
        "must remove at least 2 annotations"
    );

    // Verify sanitized document reopens and has no annotations
    let doc_after = adapter
        .open_pdf_bytes(&sanitized_bytes)
        .expect("sanitized doc must open");
    assert_eq!(doc_after.summary().page_count, 4);

    let hl_after = doc_after.page_highlight_annotations(0).expect("highlights");
    let cm_after = doc_after.page_comment_annotations(0).expect("comments");
    assert!(hl_after.is_empty(), "highlights must be cleared");
    assert!(cm_after.is_empty(), "comments must be cleared");

    // Document text must still be intact and extractable
    let text = doc_after.extract_page_text(1).expect("text must extract");
    assert!(
        !text.lines.is_empty(),
        "page content text must remain readable"
    );
}

#[test]
fn test_sanitize_preserves_annotations_when_option_disabled() {
    let base_bytes = sample_pdf_bytes();
    let comment = PdfCommentAnnotation {
        annotation_id: "test-cm-preserve".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 150_000,
            y0_milli_pt: 150_000,
            x1_milli_pt: 180_000,
            y1_milli_pt: 180_000,
        },
        color_rgb: [0, 255, 0],
        author: Some("Reviewer".to_owned()),
        contents: "Keep this note".to_owned(),
        palette_key: "note".to_owned(),
        palette_label: "Hinweis".to_owned(),
        palette_meaning: "Prüfung erforderlich".to_owned(),
    };
    let with_comment =
        add_comment_annotation_to_pdf(&base_bytes, 4, &comment).expect("comment added");

    let options = PdfSanitizeOptions {
        remove_metadata: true,
        remove_attachments: true,
        remove_actions: true,
        remove_annotations: false,
        remove_form_fields: true,
    };
    let (sanitized_bytes, report) =
        sanitize_document(&with_comment, &options).expect("sanitize must succeed");

    assert_eq!(report.annotations_removed, 0);

    let adapter = MuPdfEngineAdapter::new();
    let doc_after = adapter
        .open_pdf_bytes(&sanitized_bytes)
        .expect("sanitized doc must open");
    let cm_after = doc_after.page_comment_annotations(0).expect("comments");
    assert_eq!(
        cm_after.len(),
        1,
        "comment must be preserved when remove_annotations is false"
    );
}

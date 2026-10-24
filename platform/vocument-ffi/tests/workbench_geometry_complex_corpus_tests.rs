//! Integration tests for M3.3 Page Geometry, Hard Crop (permanent content removal),
//! Display Crop, Deskew, and Journal Privacy on complex PDFs (`ROADMAP.md` M3.3).

use vocument_ffi::mutation_dto::{
    document_core_export_transaction_journal_json, document_core_redo, document_core_undo,
};
use vocument_ffi::navigation_search_dto::FfiPdfRectMilliPt;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::{
    FfiPageWorkbenchOptions, document_core_apply_display_crop_with_options,
    document_core_apply_hard_crop_with_options, document_core_auto_crop_pages_with_options,
    document_core_deskew_pages_with_options, document_core_detect_content_margins,
    document_core_detect_deskew_angle, document_core_get_page_boxes,
};
use vocument_ffi::{
    document_core_index_search_batch, document_core_open_brokered_file, document_core_search_exact,
};

mod common;

const NAV_SEARCH_PDF: &[u8] =
    include_bytes!("../../../tests/corpus/m0_3_navigation_and_search.pdf");

fn open_test_session(pdf_bytes: &[u8], name: &str) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("workbench://{name}"),
        display_name: name.to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some(format!("geom-wb-{name}")),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: pdf_bytes.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

#[test]
fn test_complex_pdf_display_crop_vs_hard_crop_permanent_removal() {
    let session_id = open_test_session(NAV_SEARCH_PDF, "nav_search.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // Index page 0 for FTS5 search
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");

    // 1. Initial search: header text "Overview" at y=750pt and body text "architecture" at y=722pt
    let search_header_init = document_core_search_exact(session_id.clone(), "Overview".to_owned())
        .expect("search header init");
    assert!(
        !search_header_init.hits.is_empty(),
        "header should exist initially"
    );

    let search_body_init =
        document_core_search_exact(session_id.clone(), "architecture".to_owned())
            .expect("search body init");
    assert!(
        !search_body_init.hits.is_empty(),
        "body should exist initially"
    );

    // 2. Crop box excluding header (y0 = 50pt, y1 = 735pt; header is at 750pt)
    // Page is 595 x 842 pt -> [50_000, 50_000, 550_000, 735_000] milli-pt
    let crop_rect = FfiPdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 50_000,
        x1_milli_pt: 550_000,
        y1_milli_pt: 735_000,
    };

    // Apply Display Crop (non-destructive)
    let display_tx = document_core_apply_display_crop_with_options(
        session_id.clone(),
        vec![0],
        crop_rect,
        options,
    )
    .expect("apply display crop");
    assert!(display_tx.description.contains("Set page boundaries"));

    // MediaBox remains unchanged (full page 595 x 842 pt)
    let display_boxes = document_core_get_page_boxes(session_id.clone(), 0).expect("get boxes");
    assert_eq!(display_boxes.media_box.x1_milli_pt, 595_000);
    assert_eq!(display_boxes.media_box.y1_milli_pt, 842_000);

    // In non-destructive display crop, the header text is STILL present and searchable in the stream
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_header_display =
        document_core_search_exact(session_id.clone(), "Overview".to_owned())
            .expect("search header after display crop");
    assert!(
        !search_header_display.hits.is_empty(),
        "display crop must NOT remove text from the underlying stream"
    );

    // Undo display crop to return to baseline
    document_core_undo(session_id.clone()).expect("undo display crop");

    // 3. Apply Hard Crop (permanent destructive excision of out-of-bounds margins)
    let hard_tx =
        document_core_apply_hard_crop_with_options(session_id.clone(), vec![0], crop_rect, options)
            .expect("apply hard crop");
    assert!(hard_tx.description.contains("Apply hard crop"));

    // MediaBox is clamped to cropped dimensions: width = 500_000, height = 685_000 milli-pt
    let hard_boxes = document_core_get_page_boxes(session_id.clone(), 0).expect("get boxes");
    assert_eq!(hard_boxes.media_box.x0_milli_pt, 0);
    assert_eq!(hard_boxes.media_box.y0_milli_pt, 0);
    assert_eq!(hard_boxes.media_box.x1_milli_pt, 500_000);
    assert_eq!(hard_boxes.media_box.y1_milli_pt, 685_000);

    // 4. Header text "Overview" was permanently excised by margin redaction from the content stream
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_header_hard = document_core_search_exact(session_id.clone(), "Overview".to_owned())
        .expect("search header after hard crop");
    assert!(
        search_header_hard.hits.is_empty(),
        "hard crop must permanently erase out-of-bounds margin text from the document"
    );

    // 5. Surviving body text inside the crop box remains intact and searchable
    let search_body_hard =
        document_core_search_exact(session_id.clone(), "architecture".to_owned())
            .expect("search body after hard crop");
    assert!(
        !search_body_hard.hits.is_empty(),
        "text inside the crop boundary must remain intact and searchable"
    );

    // 6. Undo restores the excised content cleanly
    document_core_undo(session_id.clone()).expect("undo hard crop");
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_header_undone =
        document_core_search_exact(session_id.clone(), "Overview".to_owned())
            .expect("search header after undo");
    assert!(
        !search_header_undone.hits.is_empty(),
        "undo must restore original excised content"
    );

    // 7. Redo re-excises it
    document_core_redo(session_id.clone()).expect("redo hard crop");
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_header_redone =
        document_core_search_exact(session_id.clone(), "Overview".to_owned())
            .expect("search header after redo");
    assert!(
        search_header_redone.hits.is_empty(),
        "redo must excise out-of-bounds text again"
    );
}

#[test]
fn test_complex_pdf_deskew_and_search_alignment() {
    let session_id = open_test_session(NAV_SEARCH_PDF, "nav_search_deskew.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // Detect skew angle on clean document
    let detected_angle =
        document_core_detect_deskew_angle(session_id.clone(), 0).expect("detect deskew angle");
    assert!(
        detected_angle.abs() < 10.0,
        "clean document should have reasonable skew: {detected_angle}"
    );

    // Apply deskew rotation of +1.5° (1500 milli-degrees)
    let deskew_tx =
        document_core_deskew_pages_with_options(session_id.clone(), vec![0], 1500, options)
            .expect("deskew pages");
    assert!(deskew_tx.description.contains("Deskew"));

    // Document remains searchable and valid after deskew rotation
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_hits = document_core_search_exact(session_id.clone(), "architecture".to_owned())
        .expect("search after deskew");
    assert!(
        !search_hits.hits.is_empty(),
        "search must resolve words after deskew rotation"
    );

    // Undo deskew rotation
    document_core_undo(session_id.clone()).expect("undo deskew");
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search batch");
    let search_hits_undo =
        document_core_search_exact(session_id.clone(), "architecture".to_owned())
            .expect("search after undo deskew");
    assert!(!search_hits_undo.hits.is_empty());
}

#[test]
fn test_complex_pdf_auto_crop_margins() {
    let session_id = open_test_session(NAV_SEARCH_PDF, "nav_search_autocrop.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let initial_boxes = document_core_get_page_boxes(session_id.clone(), 0).expect("initial boxes");
    let detected =
        document_core_detect_content_margins(session_id.clone(), 0, 0).expect("detect margins");

    assert!(detected.x0_milli_pt > 0);
    assert!(detected.y0_milli_pt > 0);

    // Apply auto-crop with 5mm safety margin (14,170 milli-pt)
    let auto_tx =
        document_core_auto_crop_pages_with_options(session_id.clone(), vec![0], 14_170, options)
            .expect("auto crop");
    assert!(auto_tx.description.contains("Auto-crop"));

    let cropped_boxes = document_core_get_page_boxes(session_id.clone(), 0).expect("cropped boxes");
    assert!(cropped_boxes.crop_box.x0_milli_pt >= initial_boxes.crop_box.x0_milli_pt);

    // Undo auto-crop
    document_core_undo(session_id.clone()).expect("undo auto-crop");
    let reverted_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("reverted boxes");
    assert_eq!(
        reverted_boxes.crop_box.x0_milli_pt,
        initial_boxes.crop_box.x0_milli_pt
    );
}

#[test]
fn test_persistent_transaction_journal_strips_sensitive_pdf_byte_dumps() {
    let session_id = open_test_session(NAV_SEARCH_PDF, "nav_search_privacy.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let crop_rect = FfiPdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 50_000,
        x1_milli_pt: 500_000,
        y1_milli_pt: 700_000,
    };
    document_core_apply_hard_crop_with_options(session_id.clone(), vec![0], crop_rect, options)
        .expect("apply hard crop");

    let journal_json =
        document_core_export_transaction_journal_json(session_id).expect("export journal JSON");

    // The journal JSON must be valid, verified, and free of sensitive raw PDF byte dumps
    assert!(journal_json.contains("\"schema_version\": 1"));
    assert!(journal_json.contains("\"Apply hard crop to page #1\""));
    assert!(
        !journal_json.contains("\"previous_bytes\""),
        "journal JSON must not leak raw PDF byte buffers"
    );
}

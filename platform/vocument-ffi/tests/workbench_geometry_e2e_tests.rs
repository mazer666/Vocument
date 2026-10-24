//! End-to-end integration tests for Page Geometry, Crop, and Deskew operations across the FFI boundary (`ROADMAP.md` M3.3).

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::navigation_search_dto::FfiPdfRectMilliPt;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::{
    FfiPageBoxUpdate, FfiPageWorkbenchOptions, document_core_apply_display_crop_with_options,
    document_core_apply_hard_crop_with_options, document_core_auto_crop_pages_with_options,
    document_core_deskew_pages_with_options, document_core_detect_content_margins,
    document_core_detect_deskew_angle, document_core_get_page_boxes,
    document_core_set_page_boxes_with_options,
};
use vocument_ffi::{document_core_get_session_snapshot, document_core_open_brokered_file};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");
const NAV_SEARCH_PDF: &[u8] =
    include_bytes!("../../../tests/corpus/m0_3_navigation_and_search.pdf");

fn open_test_session(pdf_bytes: &[u8]) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://geom_sample.pdf".to_owned(),
        display_name: "geom_sample.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("geom-wb-e2e".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: pdf_bytes.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

#[test]
fn test_ffi_get_and_set_page_boxes_undo_redo() {
    let session_id = open_test_session(TWO_PAGE_PDF);

    // 1. Query initial boxes
    let initial_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("get initial page 0 boxes");
    assert_eq!(initial_boxes.media_box.x0_milli_pt, 0);
    assert_eq!(initial_boxes.media_box.y0_milli_pt, 0);
    assert_eq!(initial_boxes.rotation_degrees, 0);

    // 2. Set CropBox (non-destructive display crop)
    let update = FfiPageBoxUpdate {
        media_box: None,
        crop_box: Some(FfiPdfRectMilliPt {
            x0_milli_pt: 50_000,
            y0_milli_pt: 50_000,
            x1_milli_pt: 500_000,
            y1_milli_pt: 700_000,
        }),
        bleed_box: None,
        trim_box: None,
        art_box: None,
    };

    let tx = document_core_set_page_boxes_with_options(
        session_id.clone(),
        vec![0],
        update,
        FfiPageWorkbenchOptions::default(),
    )
    .expect("set page boxes succeeds");
    assert_eq!(tx.description, "Set page boundaries for page #1");

    let snap = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert!(snap.is_dirty);
    assert_eq!(snap.revision, 2);

    // 3. Query back
    let updated_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("get updated page 0 boxes");
    assert_eq!(updated_boxes.crop_box.x0_milli_pt, 50_000);
    assert_eq!(updated_boxes.crop_box.y0_milli_pt, 50_000);
    // MediaBox remains untouched
    assert_eq!(updated_boxes.media_box, initial_boxes.media_box);

    // 4. Undo restores initial state
    let undo_hist = document_core_undo(session_id.clone()).expect("undo succeeds");
    assert!(undo_hist.can_redo);

    let undo_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("get undo page 0 boxes");
    assert_eq!(undo_boxes.crop_box, initial_boxes.crop_box);

    // 5. Redo re-applies
    document_core_redo(session_id.clone()).expect("redo succeeds");
    let redo_boxes = document_core_get_page_boxes(session_id, 0).expect("get redo page 0 boxes");
    assert_eq!(redo_boxes.crop_box.x0_milli_pt, 50_000);
}

#[test]
fn test_ffi_display_crop_vs_hard_crop() {
    let session_id = open_test_session(TWO_PAGE_PDF);

    let crop_rect = FfiPdfRectMilliPt {
        x0_milli_pt: 40_000,
        y0_milli_pt: 40_000,
        x1_milli_pt: 450_000,
        y1_milli_pt: 650_000,
    };

    // 1. Display crop
    let tx_disp = document_core_apply_display_crop_with_options(
        session_id.clone(),
        vec![0],
        crop_rect,
        FfiPageWorkbenchOptions::default(),
    )
    .expect("apply display crop succeeds");
    assert_eq!(tx_disp.description, "Set page boundaries for page #1");

    let disp_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("get display crop boxes");
    assert_eq!(disp_boxes.crop_box.x0_milli_pt, 40_000);
    assert_eq!(disp_boxes.media_box.x0_milli_pt, 0); // MediaBox intact

    // Undo display crop
    document_core_undo(session_id.clone()).expect("undo display crop");

    // 2. Hard crop (destructive)
    let tx_hard = document_core_apply_hard_crop_with_options(
        session_id.clone(),
        vec![0],
        crop_rect,
        FfiPageWorkbenchOptions::default(),
    )
    .expect("apply hard crop succeeds");
    assert_eq!(tx_hard.description, "Apply hard crop to page #1");

    let hard_boxes =
        document_core_get_page_boxes(session_id.clone(), 0).expect("get hard crop boxes");
    assert_eq!(hard_boxes.crop_box.x0_milli_pt, 0);
    assert_eq!(hard_boxes.crop_box.x1_milli_pt, 410_000);
    assert_eq!(hard_boxes.media_box.x0_milli_pt, 0);
    assert_eq!(hard_boxes.media_box.x1_milli_pt, 410_000);

    // Undo hard crop restores original MediaBox (595 pt width) and content stream
    document_core_undo(session_id.clone()).expect("undo hard crop");
    let restored_boxes = document_core_get_page_boxes(session_id, 0).expect("get restored boxes");
    assert_eq!(restored_boxes.media_box.x0_milli_pt, 0);
    assert_eq!(restored_boxes.media_box.x1_milli_pt, 595_000);
}

#[test]
fn test_ffi_deskew_pages_and_undo_redo() {
    let session_id = open_test_session(TWO_PAGE_PDF);

    // 1. Detect skew angle on clean document
    let angle =
        document_core_detect_deskew_angle(session_id.clone(), 0).expect("detect deskew angle");
    assert!(angle.abs() < 1.0);

    // 2. Apply deskew rotation (+2.5° = 2500 milli-degrees)
    let tx = document_core_deskew_pages_with_options(
        session_id.clone(),
        vec![0],
        2500,
        FfiPageWorkbenchOptions::default(),
    )
    .expect("deskew pages succeeds");
    assert_eq!(tx.description, "Deskew page #1 by +2.5°");

    let snap = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert!(snap.is_dirty);
    assert_eq!(snap.revision, 2);

    // 3. Undo restores previous bytes
    let undo_hist = document_core_undo(session_id.clone()).expect("undo deskew");
    assert!(undo_hist.can_redo);

    let snap_restored = document_core_get_session_snapshot(session_id).expect("snapshot");
    assert_eq!(snap_restored.revision, 1);
    assert!(!snap_restored.is_dirty);
}

#[test]
fn test_ffi_auto_crop_pages_with_options() {
    let session_id = open_test_session(NAV_SEARCH_PDF);

    // 1. Detect content margins
    let margins = document_core_detect_content_margins(session_id.clone(), 0, 15_000)
        .expect("detect content margins");
    assert!(margins.x0_milli_pt < margins.x1_milli_pt);
    assert!(margins.y0_milli_pt < margins.y1_milli_pt);

    // 2. Apply auto crop
    let tx = document_core_auto_crop_pages_with_options(
        session_id.clone(),
        vec![0],
        15_000,
        FfiPageWorkbenchOptions::default(),
    )
    .expect("auto crop succeeds");
    assert_eq!(tx.description, "Auto-crop page #1");

    let boxes = document_core_get_page_boxes(session_id.clone(), 0).expect("get page boxes");
    assert_eq!(boxes.crop_box, margins);

    // 3. Undo
    document_core_undo(session_id.clone()).expect("undo auto crop");
    let restored = document_core_get_page_boxes(session_id, 0).expect("restored boxes");
    assert_ne!(restored.crop_box, margins);
}

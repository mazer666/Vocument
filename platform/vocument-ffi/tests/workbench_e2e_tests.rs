//! End-to-end integration tests for Page Workbench operations across the FFI boundary (`ROADMAP.md` M3).
//!
//! Verifies reordering, duplicating, deleting, and extracting pages, along with
//! reversible Undo/Redo transactions, bounds checks, and PDF structural integrity.

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::{
    document_core_delete_pages, document_core_duplicate_page, document_core_extract_pages,
    document_core_reorder_pages,
};
use vocument_ffi::{document_core_get_session_snapshot, document_core_open_brokered_file};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

fn open_test_session() -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://sample.pdf".to_owned(),
        display_name: "sample.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("wb-e2e-1".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: TWO_PAGE_PDF.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

#[test]
fn test_ffi_workbench_reorder_pages_and_undo_redo() {
    let session_id = open_test_session();
    let snap = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap.pdf_summary.page_count, 2);

    // 1. Reorder: swap [0, 1] to [1, 0]
    let tx = document_core_reorder_pages(session_id.clone(), vec![1, 0])
        .expect("reorder pages succeeds");
    assert_eq!(tx.description, "Reorder 2 pages");

    let snap_after = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_after.pdf_summary.page_count, 2);
    assert_eq!(snap_after.revision, 2);
    assert!(snap_after.is_dirty);

    // 2. Undo reorder -> returns to initial sequence
    let undo_hist = document_core_undo(session_id.clone()).expect("undo succeeds");
    assert_eq!(
        undo_hist.redo_description.as_deref(),
        Some("Reorder 2 pages")
    );
    assert!(undo_hist.can_redo);

    let snap_undo = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_undo.revision, 1);
    assert!(!snap_undo.is_dirty);
    assert_eq!(snap_undo.pdf_summary.page_count, 2);

    // 3. Redo reorder -> re-applies reorder
    let redo_hist = document_core_redo(session_id.clone()).expect("redo succeeds");
    assert_eq!(
        redo_hist.undo_description.as_deref(),
        Some("Reorder 2 pages")
    );

    // 4. Invalid permutation is rejected
    let invalid_res = document_core_reorder_pages(session_id.clone(), vec![0, 0]);
    assert!(invalid_res.is_err());
}

#[test]
fn test_ffi_workbench_duplicate_page_and_undo_redo() {
    let session_id = open_test_session();

    // 1. Duplicate page 0 -> page count becomes 3
    let tx = document_core_duplicate_page(session_id.clone(), 0).expect("duplicate succeeds");
    assert_eq!(tx.description, "Duplicate page #1");

    let snap_after = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_after.pdf_summary.page_count, 3);

    // 2. Undo duplication -> page count drops back to 2
    document_core_undo(session_id.clone()).expect("undo succeeds");
    let snap_undo = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_undo.pdf_summary.page_count, 2);

    // 3. Redo duplication -> page count returns to 3
    document_core_redo(session_id.clone()).expect("redo succeeds");
    let snap_redo = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_redo.pdf_summary.page_count, 3);
}

#[test]
fn test_ffi_workbench_delete_pages_and_undo_redo() {
    let session_id = open_test_session();

    // 1. Delete page 1 -> page count becomes 1
    let tx = document_core_delete_pages(session_id.clone(), vec![1]).expect("delete succeeds");
    assert_eq!(tx.description, "Delete 1 page(s)");

    let snap_after = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_after.pdf_summary.page_count, 1);

    // 2. Attempting to delete the last page must fail
    let err = document_core_delete_pages(session_id.clone(), vec![0]);
    assert!(err.is_err());

    // 3. Undo deletion -> restores 2 pages
    document_core_undo(session_id.clone()).expect("undo succeeds");
    let snap_undo = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_undo.pdf_summary.page_count, 2);

    // 4. Redo deletion -> page count drops to 1 again
    document_core_redo(session_id.clone()).expect("redo succeeds");
    let snap_redo = document_core_get_session_snapshot(session_id.clone()).expect("snapshot");
    assert_eq!(snap_redo.pdf_summary.page_count, 1);
}

#[test]
fn test_ffi_workbench_extract_pages() {
    let session_id = open_test_session();

    // Extract page 0
    let extracted =
        document_core_extract_pages(session_id.clone(), vec![0]).expect("extract succeeds");
    assert!(extracted.starts_with(b"%PDF-"));

    // Open extracted bytes as a new brokered session
    let extracted_descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://extracted.pdf".to_owned(),
        display_name: "extracted.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("wb-e2e-extracted".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: extracted,
    };
    let extracted_snap =
        document_core_open_brokered_file(extracted_descriptor).expect("open extracted document");
    assert_eq!(extracted_snap.pdf_summary.page_count, 1);

    // Verify source document remains completely intact (2 pages, revision 1, not dirty)
    let src_snap = document_core_get_session_snapshot(session_id).expect("snapshot");
    assert_eq!(src_snap.pdf_summary.page_count, 2);
    assert_eq!(src_snap.revision, 1);
    assert!(!src_snap.is_dirty);
}

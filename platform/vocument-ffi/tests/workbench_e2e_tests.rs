//! End-to-end integration tests for Page Workbench operations across the FFI boundary (`ROADMAP.md` M3).
//!
//! Verifies reordering, duplicating, deleting, and extracting pages, along with
//! reversible Undo/Redo transactions, bounds checks, and PDF structural integrity.

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::{
    FfiPageWorkbenchOptions, document_core_delete_pages, document_core_delete_pages_with_options,
    document_core_duplicate_page, document_core_duplicate_page_with_options,
    document_core_extract_pages, document_core_reorder_pages,
    document_core_reorder_pages_with_options, document_core_workbench_preflight,
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

fn build_signed_test_pdf() -> Vec<u8> {
    const CONTENTS_LEN: usize = 40;
    let contents = "BT /F1 12 Tf 72 740 Td (Signed Invoice) Tj ET\n";
    let build = |byte_range: &str| -> Vec<u8> {
        let objects: Vec<String> = vec![
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R] >> >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{contents}endstream", contents.len()),
            "<< /Type /Annot /Subtype /Widget /FT /Sig /T (SignatureField) /V 7 0 R /Rect [72 100 320 140] /P 3 0 R >>".to_owned(),
            format!("<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /Name (Dr. Weber) /M (D:20260131120000Z) /Reason (Approval) /ByteRange {byte_range} /Contents <{}> >>", "0".repeat(CONTENTS_LEN)),
        ];
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.7\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref_offset = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for offset in &offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    };

    let probe = build("[0000000000 0000000000 0000000000 0000000000]");
    let marker = b"/Contents <";
    let contents_start = probe
        .windows(marker.len())
        .position(|w| w == marker)
        .expect("marker")
        + marker.len();
    let file_size = probe.len();
    let byte_range = format!(
        "[{:010} {:010} {:010} {:010}]",
        0,
        contents_start,
        contents_start + CONTENTS_LEN,
        file_size - (contents_start + CONTENTS_LEN)
    );
    build(&byte_range)
}

#[test]
fn test_ffi_workbench_preflight_and_fail_closed_digital_signature_protection() {
    let signed_bytes = build_signed_test_pdf();
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://signed.pdf".to_owned(),
        display_name: "signed.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("wb-e2e-signed-1".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: signed_bytes,
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open signed file");
    let session_id = snap.session_id;

    // 1. Mandatory Preflight Check
    let preflight =
        document_core_workbench_preflight(session_id.clone()).expect("preflight report");
    assert!(preflight.is_signed);
    assert_eq!(preflight.signature_count, 1);
    assert_eq!(preflight.signature_field_names, vec!["SignatureField"]);
    assert!(!preflight.warnings.is_empty());
    assert!(preflight.warnings[0].contains("digital signature"));

    // 2. Default mutations fail closed
    let reorder_err = document_core_reorder_pages(session_id.clone(), vec![0]);
    assert!(reorder_err.is_err());

    let dup_err = document_core_duplicate_page(session_id.clone(), 0);
    assert!(dup_err.is_err());

    let del_err = document_core_delete_pages(session_id.clone(), vec![0]);
    assert!(del_err.is_err());

    // Document remains unmutated at revision 1
    let snap_after = document_core_get_session_snapshot(session_id.clone()).expect("snap");
    assert_eq!(snap_after.revision, 1);
    assert!(!snap_after.is_dirty);

    // 3. Mutation with explicit signature invalidation succeeds
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: true,
    };
    let dup_tx = document_core_duplicate_page_with_options(session_id.clone(), 0, options)
        .expect("duplicate with signature invalidation succeeds");
    assert_eq!(dup_tx.description, "Duplicate page #1");

    let snap_dup = document_core_get_session_snapshot(session_id.clone()).expect("snap");
    assert_eq!(snap_dup.pdf_summary.page_count, 2);
    assert_eq!(snap_dup.revision, 2);
    assert!(snap_dup.is_dirty);

    // Reorder with options succeeds
    let reorder_tx =
        document_core_reorder_pages_with_options(session_id.clone(), vec![1, 0], options)
            .expect("reorder with options succeeds");
    assert_eq!(reorder_tx.description, "Reorder 2 pages");

    // Delete with options succeeds
    let del_tx = document_core_delete_pages_with_options(session_id.clone(), vec![1], options)
        .expect("delete with options succeeds");
    assert_eq!(del_tx.description, "Delete 1 page(s)");

    // 4. Undo / Redo support across all mutations
    // Undo #1: undo Delete (restores 2 pages)
    document_core_undo(session_id.clone()).expect("undo delete");
    let snap_u1 = document_core_get_session_snapshot(session_id.clone()).expect("snap");
    assert_eq!(snap_u1.pdf_summary.page_count, 2);
    assert!(snap_u1.is_dirty);

    // Undo #2: undo Reorder (page count remains 2)
    document_core_undo(session_id.clone()).expect("undo reorder");
    let snap_u2 = document_core_get_session_snapshot(session_id.clone()).expect("snap");
    assert_eq!(snap_u2.pdf_summary.page_count, 2);
    assert!(snap_u2.is_dirty);

    // Undo #3: undo Duplicate (restores initial 1 page, rev 1, not dirty)
    document_core_undo(session_id.clone()).expect("undo duplicate");
    let snap_u3 = document_core_get_session_snapshot(session_id.clone()).expect("snap");
    assert_eq!(snap_u3.pdf_summary.page_count, 1);
    assert_eq!(snap_u3.revision, 1);
    assert!(!snap_u3.is_dirty);

    // Redo #1: re-applies duplicate (2 pages)
    document_core_redo(session_id.clone()).expect("redo duplicate");
    let snap_r1 = document_core_get_session_snapshot(session_id).expect("snap");
    assert_eq!(snap_r1.pdf_summary.page_count, 2);
    assert!(snap_r1.is_dirty);
}

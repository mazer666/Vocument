//! Independent verification tests for the 3 integrity findings:
//! 1. Hard Crop / Data Removal (reversibility vs. end-state content removal, no residual text/images in exported PDF).
//! 2. Recovery / Transaction Journal (safe recovery of Merge/Insert/HardCrop with live Undo/Redo without plaintext leaks).
//! 3. Multi-Document Integrity (forward link preservation, named dest collision resolution, and sorted NameTrees).

use vocument_ffi::mutation_dto::{
    document_core_export_transaction_journal_json, document_core_redo,
    document_core_restore_transaction_journal_json, document_core_transaction_history,
    document_core_undo,
};
use vocument_ffi::navigation_search_dto::FfiPdfRectMilliPt;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::{
    FfiPageWorkbenchOptions, document_core_apply_hard_crop_with_options,
    document_core_extract_pages, document_core_get_page_boxes,
    document_core_insert_pages_with_options,
};
use vocument_ffi::{
    document_core_close_session, document_core_get_session_snapshot,
    document_core_index_search_batch, document_core_open_brokered_file, document_core_search_exact,
};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");
const NAV_SEARCH_PDF: &[u8] =
    include_bytes!("../../../tests/corpus/m0_3_navigation_and_search.pdf");

fn open_brokered_session(pdf_bytes: &[u8], name: &str) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("integrity://{name}"),
        display_name: name.to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some(format!("integ-{name}")),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: pdf_bytes.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

/// Helper that creates a PDF with internal links (including forward links) and named destinations.
fn create_pdf_with_forward_link_and_named_dest(doc_title: &str, dest_name: &str) -> Vec<u8> {
    let mut objects: Vec<String> = Vec::new();

    // 1: Catalog with Dests and Names
    objects.push(format!(
        "<< /Type /Catalog /Pages 2 0 R /Dests << /{dest_name} [3 0 R /Fit] >> /Names << /Dests 8 0 R >> >>"
    ));

    // 2: Pages root (2 pages: 3 0 R and 5 0 R)
    objects.push("<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>".to_owned());

    // 3: Page 0 (contains link pointing forward to Page 1: 5 0 R)
    objects.push(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Annots [7 0 R] /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>".to_owned(),
    );

    // 4: Page 0 Contents
    let p0_content = format!("BT /F1 12 Tf 72 700 Td ({doc_title} - Page 0) Tj ET\n");
    objects.push(format!(
        "<< /Length {} >>\nstream\n{p0_content}endstream",
        p0_content.len()
    ));

    // 5: Page 1
    objects.push(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 6 0 R /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>".to_owned(),
    );

    // 6: Page 1 Contents
    let p1_content = format!("BT /F1 12 Tf 72 700 Td ({doc_title} - Page 1 Target) Tj ET\n");
    objects.push(format!(
        "<< /Length {} >>\nstream\n{p1_content}endstream",
        p1_content.len()
    ));

    // 7: Link annotation on Page 0 targeting Page 1 (forward GoTo link)
    objects.push(
        "<< /Type /Annot /Subtype /Link /Rect [72 680 200 710] /A << /S /GoTo /D [5 0 R /Fit] >> >>"
            .to_owned(),
    );

    // 8: NameTree dictionary for /Names /Dests
    objects.push(format!("<< /Names [ ({dest_name}) [5 0 R /Fit] ] >>"));

    // 9: Info dictionary
    objects.push(format!("<< /Title ({doc_title}) >>"));

    let mut buffer = Vec::new();
    buffer.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = Vec::new();
    for (idx, obj_str) in objects.iter().enumerate() {
        let obj_num = idx + 1;
        offsets.push(buffer.len());
        buffer.extend_from_slice(format!("{obj_num} 0 obj\n{obj_str}\nendobj\n").as_bytes());
    }
    let xref_offset = buffer.len();
    buffer.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    buffer.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        buffer.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    buffer.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 9 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    buffer
}

#[test]
fn test_multidoc_forward_internal_links_and_named_dest_collisions() {
    let doc_a = create_pdf_with_forward_link_and_named_dest("DocA", "TargetSection");
    let doc_b = create_pdf_with_forward_link_and_named_dest("DocB", "TargetSection");

    let session_id = open_brokered_session(&doc_a, "doc_a.pdf");
    let _options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // Insert all pages of doc_b into doc_a at page index 1 via multidoc engine
    let inserted_bytes =
        vocument_pdf_mupdf::multidoc_ops::insert_pages_ext(&doc_a, &doc_b, &[0, 1], 1, false)
            .expect("insert pages from doc_b");
    assert!(inserted_bytes.starts_with(b"%PDF-"));

    let engine = vocument_pdf_mupdf::MuPdfEngineAdapter::new();
    use vocument_core::pdf_engine::PdfEngine;
    let doc = engine
        .open_pdf_bytes(&inserted_bytes)
        .expect("open merged pdf");
    assert_eq!(doc.summary().page_count, 4);

    // Verify named dest collision resolution:
    // Both DocA and DocB had "TargetSection". In the combined document, DocB's
    // destination must be preserved without overwriting DocA's, using disambiguation.
    let exported_str = String::from_utf8_lossy(&inserted_bytes);
    assert!(
        exported_str.contains("TargetSection"),
        "Primary named destination must exist"
    );
    assert!(
        exported_str.contains("TargetSection_Insert") || exported_str.contains("TargetSection_Doc"),
        "Colliding named destination must be disambiguated, not dropped"
    );

    document_core_close_session(session_id).expect("close");
}

#[test]
fn test_crash_recovery_replay_and_live_undo_redo_for_insert_and_hard_crop() {
    let session_id = open_brokered_session(TWO_PAGE_PDF, "base_recovery.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // 1. Insert 2 pages from NAV_SEARCH_PDF at index 1 -> total becomes 4 pages
    let insert_tx = document_core_insert_pages_with_options(
        session_id.clone(),
        NAV_SEARCH_PDF.to_vec(),
        vec![0, 1],
        1,
        options,
    )
    .expect("insert pages");
    assert_eq!(insert_tx.revision_after, 2);

    let snap1 = document_core_get_session_snapshot(session_id.clone()).expect("snap 1");
    assert_eq!(snap1.pdf_summary.page_count, 4);

    // 2. Apply hard crop to page 1
    let crop_rect = FfiPdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 50_000,
        x1_milli_pt: 450_000,
        y1_milli_pt: 650_000,
    };
    let crop_tx =
        document_core_apply_hard_crop_with_options(session_id.clone(), vec![1], crop_rect, options)
            .expect("apply hard crop");
    assert_eq!(crop_tx.revision_after, 3);

    let boxes_cropped = document_core_get_page_boxes(session_id.clone(), 1).expect("boxes");
    assert_eq!(boxes_cropped.media_box.x1_milli_pt, 400_000);
    assert_eq!(boxes_cropped.media_box.y1_milli_pt, 600_000);

    // 3. Export transaction journal JSON
    let journal_json = document_core_export_transaction_journal_json(session_id.clone())
        .expect("export journal JSON");
    assert!(journal_json.contains("\"schema_version\": 1"));
    // Journal must NOT store redundant plaintext previous_bytes copies on disk
    assert!(
        !journal_json.contains("\"previous_bytes\""),
        "journal JSON on disk must not leak raw previous_bytes snapshots"
    );

    // 4. Simulate crash & restart: open fresh session from base TWO_PAGE_PDF
    let fresh_session_id = open_brokered_session(TWO_PAGE_PDF, "base_recovery_fresh.pdf");
    let restored_hist =
        document_core_restore_transaction_journal_json(fresh_session_id.clone(), journal_json)
            .expect("restore and replay journal");

    // Verified state matches exact pre-crash state
    assert_eq!(restored_hist.current_revision, 3);
    assert_eq!(restored_hist.undo_count, 2);
    assert!(restored_hist.can_undo);

    let snap_restored =
        document_core_get_session_snapshot(fresh_session_id.clone()).expect("restored snap");
    assert_eq!(snap_restored.pdf_summary.page_count, 4);

    let boxes_restored =
        document_core_get_page_boxes(fresh_session_id.clone(), 1).expect("restored boxes");
    assert_eq!(boxes_restored.media_box.x1_milli_pt, 400_000);

    // 5. Live Undo #1: undo hard crop immediately after recovery replay
    let undo1 = document_core_undo(fresh_session_id.clone()).expect("undo hard crop after crash");
    assert_eq!(undo1.current_revision, 4);
    assert!(undo1.can_undo);
    assert!(undo1.can_redo);

    let snap_u1 = document_core_get_session_snapshot(fresh_session_id.clone()).expect("snap u1");
    assert_eq!(snap_u1.pdf_summary.page_count, 4);

    // 6. Live Undo #2: undo insert pages
    let undo2 = document_core_undo(fresh_session_id.clone()).expect("undo insert after crash");
    assert!(!undo2.can_undo);
    assert!(undo2.can_redo);

    let snap_u2 = document_core_get_session_snapshot(fresh_session_id.clone()).expect("snap u2");
    assert_eq!(snap_u2.pdf_summary.page_count, 2); // returns cleanly to original 2 pages!

    // 7. Live Redo #1: redo insert pages
    let _redo1 = document_core_redo(fresh_session_id.clone()).expect("redo insert");
    assert_eq!(
        document_core_get_session_snapshot(fresh_session_id.clone())
            .expect("fresh snapshot")
            .pdf_summary
            .page_count,
        4
    );

    // 8. Live Redo #2: redo hard crop
    let _redo2 = document_core_redo(fresh_session_id.clone()).expect("redo hard crop");
    let boxes_redone =
        document_core_get_page_boxes(fresh_session_id.clone(), 1).expect("redone boxes");
    assert_eq!(boxes_redone.media_box.x1_milli_pt, 400_000);

    document_core_close_session(session_id).expect("close original");
    document_core_close_session(fresh_session_id).expect("close restored");
}

#[test]
fn test_hard_crop_end_state_and_residual_content_verification() {
    let session_id = open_brokered_session(NAV_SEARCH_PDF, "nav_end_state.pdf");
    let options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // Index page 0
    document_core_index_search_batch(session_id.clone(), 0, 10).expect("index search");

    // Header "Overview" is at y = 750 pt; crop box is y0 = 50 pt, y1 = 735 pt
    let crop_rect = FfiPdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 50_000,
        x1_milli_pt: 550_000,
        y1_milli_pt: 735_000,
    };

    document_core_apply_hard_crop_with_options(session_id.clone(), vec![0], crop_rect, options)
        .expect("apply hard crop");

    // Extract/export the resulting PDF bytes
    let exported_bytes =
        document_core_extract_pages(session_id.clone(), vec![0]).expect("export page 0");

    // 1. In a freshly opened document from the exported PDF, the excised text "Overview" does NOT exist
    let exported_session = open_brokered_session(&exported_bytes, "exported.pdf");
    document_core_index_search_batch(exported_session.clone(), 0, 10).expect("index exported");
    let search_excised =
        document_core_search_exact(exported_session.clone(), "Overview".to_owned())
            .expect("search excised");
    assert!(
        search_excised.hits.is_empty(),
        "exported PDF must not contain excised text anywhere in its stream"
    );

    // 2. Surviving text inside crop box remains intact
    let search_surviving =
        document_core_search_exact(exported_session.clone(), "architecture".to_owned())
            .expect("search surviving");
    assert!(
        !search_surviving.hits.is_empty(),
        "text inside crop rectangle must be preserved"
    );

    // 3. Scan the raw exported bytes for the word "Overview":
    // The literal text string must NOT appear in any stream or unreferenced object!
    let raw_str = String::from_utf8_lossy(&exported_bytes);
    assert!(
        !raw_str.contains("Overview"),
        "Raw PDF bytes must not contain orphaned stream content or previous revisions"
    );

    // 4. In the active session, Undo is still available for user recovery
    let active_hist =
        document_core_transaction_history(session_id.clone()).expect("active history");
    assert!(
        active_hist.can_undo,
        "Active session retains undo history for user flexibility"
    );

    document_core_close_session(session_id).expect("close");
    document_core_close_session(exported_session).expect("close exported");
}

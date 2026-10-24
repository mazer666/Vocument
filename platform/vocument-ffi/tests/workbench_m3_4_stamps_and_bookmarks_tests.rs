//! Integration tests for M3.4 Bates Numbering, Headers/Footers, Bookmarks, and Stamps (`ROADMAP.md` M3.4).

use vocument_ffi::mutation_dto::{
    document_core_export_transaction_journal_json, document_core_redo,
    document_core_restore_transaction_journal_json, document_core_undo,
};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::stamp_dto::{
    FfiBatesNumberingOptions, FfiBatesPosition, FfiBookmarkItem, FfiHeaderFooterOptions,
    FfiStampOrWatermarkOptions, document_core_apply_bates_numbering_with_options,
    document_core_apply_header_footer_with_options,
    document_core_apply_stamp_or_watermark_with_options, document_core_get_bookmarks,
    document_core_set_bookmarks_with_options,
};
use vocument_ffi::workbench_dto::{FfiPageWorkbenchOptions, document_core_extract_pages};
use vocument_ffi::{
    DocumentCoreError, document_core_close_session, document_core_index_search_batch,
    document_core_open_brokered_file, document_core_search_exact,
};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

fn open_test_session() -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://m34_sample.pdf".to_owned(),
        display_name: "m34_sample.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("wb-m34-1".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: TWO_PAGE_PDF.to_vec(),
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

fn page_contains_text(session_id: &str, page_index: u32, expected_text: &str) -> bool {
    let exported = document_core_extract_pages(session_id.to_owned(), vec![page_index])
        .expect("extract page bytes");
    let raw = String::from_utf8_lossy(&exported);
    if raw.contains(expected_text) {
        return true;
    }
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("test://page_{page_index}.pdf"),
        display_name: format!("page_{page_index}.pdf"),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: None,
        modified_unix_ms: None,
        bytes: exported,
    };
    if let Ok(snap) = document_core_open_brokered_file(descriptor) {
        let _ = document_core_index_search_batch(snap.session_id.clone(), 0, 10);
        if let Ok(res) = document_core_search_exact(snap.session_id, expected_text.to_owned()) {
            return !res.hits.is_empty();
        }
    }
    false
}

#[test]
fn test_m3_4_bates_numbering_formatting_and_undo_redo() {
    let session_id = open_test_session();

    let options = FfiBatesNumberingOptions {
        prefix: "CASE-".to_owned(),
        start_number: 10,
        digit_padding: 5,
        suffix: "-EX".to_owned(),
        position: FfiBatesPosition::TopRight,
        font_size_pt: 12,
        margin_pt: 36,
    };
    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // 1. Apply Bates numbering to pages [0, 1]
    let tx = document_core_apply_bates_numbering_with_options(
        session_id.clone(),
        vec![0, 1],
        options,
        wb_options,
    )
    .expect("apply bates numbering succeeds");

    assert_eq!(tx.description, "Apply Bates numbering (CASE-10) to 2 pages");

    // Verify text on page 0 and 1
    assert!(
        page_contains_text(&session_id, 0, "CASE-00010-EX"),
        "Page 0 must contain CASE-00010-EX"
    );
    assert!(
        page_contains_text(&session_id, 1, "CASE-00011-EX"),
        "Page 1 must contain CASE-00011-EX"
    );

    // 2. Undo -> Bates numbers removed
    let undo = document_core_undo(session_id.clone()).expect("undo");
    assert!(undo.can_redo);

    assert!(
        !page_contains_text(&session_id, 0, "CASE-00010-EX"),
        "Page 0 must NOT contain Bates after undo"
    );

    // 3. Redo -> Bates numbers restored
    let redo = document_core_redo(session_id.clone()).expect("redo");
    assert!(redo.can_undo);

    assert!(
        page_contains_text(&session_id, 0, "CASE-00010-EX"),
        "Page 0 must contain Bates after redo"
    );
}

#[test]
fn test_m3_4_header_footer_macro_expansion_and_undo_redo() {
    let session_id = open_test_session();

    let options = FfiHeaderFooterOptions {
        header_left: Some("CONFIDENTIAL".to_owned()),
        header_center: Some("{filename}".to_owned()),
        header_right: Some("{date}".to_owned()),
        footer_left: None,
        footer_center: Some("Page {page} of {total}".to_owned()),
        footer_right: None,
        font_size_pt: 10,
        margin_pt: 30,
    };
    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let tx = document_core_apply_header_footer_with_options(
        session_id.clone(),
        vec![0, 1],
        options,
        Some("ProjectReport".to_owned()),
        Some("2026-10-24".to_owned()),
        wb_options,
    )
    .expect("apply header footer succeeds");

    assert_eq!(tx.description, "Apply headers and footers to 2 pages");

    assert!(
        page_contains_text(&session_id, 0, "CONFIDENTIAL"),
        "Page 0 must contain header_left text"
    );
    assert!(
        page_contains_text(&session_id, 0, "ProjectReport"),
        "Page 0 must contain expanded {{filename}}"
    );
    assert!(
        page_contains_text(&session_id, 0, "2026-10-24"),
        "Page 0 must contain expanded {{date}}"
    );

    // Undo -> removed
    let undo = document_core_undo(session_id.clone()).expect("undo");
    assert!(undo.can_redo);

    assert!(
        !page_contains_text(&session_id, 0, "CONFIDENTIAL"),
        "Page 0 must NOT contain header after undo"
    );
}

#[test]
fn test_m3_4_hierarchical_bookmarks_read_write_and_undo_redo() {
    let session_id = open_test_session();

    // 1. Initial bookmarks
    let initial_bookmarks =
        document_core_get_bookmarks(session_id.clone()).expect("get initial bookmarks");

    // 2. Set hierarchical outline
    let new_bookmarks = vec![
        FfiBookmarkItem {
            title: "Chapter 1: Overview".to_owned(),
            target_page: 0,
            children: vec![
                FfiBookmarkItem {
                    title: "Section 1.1: Background".to_owned(),
                    target_page: 0,
                    children: Vec::new(),
                },
                FfiBookmarkItem {
                    title: "Section 1.2: Scope".to_owned(),
                    target_page: 0,
                    children: Vec::new(),
                },
            ],
        },
        FfiBookmarkItem {
            title: "Chapter 2: Architecture".to_owned(),
            target_page: 1,
            children: Vec::new(),
        },
    ];

    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let tx = document_core_set_bookmarks_with_options(
        session_id.clone(),
        new_bookmarks.clone(),
        wb_options,
    )
    .expect("set bookmarks succeeds");

    assert_eq!(tx.description, "Update bookmark hierarchy (2 bookmarks)");

    // 3. Read back and verify exact hierarchy
    let read_back = document_core_get_bookmarks(session_id.clone()).expect("read back bookmarks");
    assert_eq!(read_back.len(), 2);
    assert_eq!(read_back[0].title, "Chapter 1: Overview");
    assert_eq!(read_back[0].target_page, 0);
    assert_eq!(read_back[0].children.len(), 2);
    assert_eq!(read_back[0].children[0].title, "Section 1.1: Background");
    assert_eq!(read_back[0].children[1].title, "Section 1.2: Scope");
    assert_eq!(read_back[1].title, "Chapter 2: Architecture");
    assert_eq!(read_back[1].target_page, 1);

    // 4. Undo -> restores initial bookmarks
    let undo = document_core_undo(session_id.clone()).expect("undo");
    assert!(undo.can_redo);

    let restored_bookmarks =
        document_core_get_bookmarks(session_id.clone()).expect("get bookmarks after undo");
    assert_eq!(restored_bookmarks.len(), initial_bookmarks.len());

    // 5. Redo -> re-applies outline
    let redo = document_core_redo(session_id.clone()).expect("redo");
    assert!(redo.can_undo);

    let redo_bookmarks =
        document_core_get_bookmarks(session_id.clone()).expect("get bookmarks after redo");
    assert_eq!(redo_bookmarks.len(), 2);
    assert_eq!(redo_bookmarks[0].title, "Chapter 1: Overview");
}

#[test]
fn test_m3_4_rubber_stamps_and_watermarks_presets_and_undo_redo() {
    let session_id = open_test_session();

    // 1. Apply "APPROVED" stamp (diagonal foreground)
    let stamp_options = FfiStampOrWatermarkOptions {
        text: "APPROVED".to_owned(),
        opacity_milli: 800,
        rotation_degrees: -45,
        r: 30,
        g: 160,
        b: 50,
        is_background: false,
        rect: None,
    };
    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let tx = document_core_apply_stamp_or_watermark_with_options(
        session_id.clone(),
        vec![0],
        stamp_options,
        wb_options,
    )
    .expect("apply stamp succeeds");

    assert_eq!(tx.description, "Apply stamp \"APPROVED\" to 1 pages");

    assert!(
        page_contains_text(&session_id, 0, "APPROVED"),
        "Page 0 must contain 'APPROVED' stamp"
    );

    // 2. Undo -> stamp removed
    let undo = document_core_undo(session_id.clone()).expect("undo");
    assert!(undo.can_redo);

    assert!(
        !page_contains_text(&session_id, 0, "APPROVED"),
        "Page 0 must NOT contain 'APPROVED' after undo"
    );

    // 3. Redo -> stamp restored
    let redo = document_core_redo(session_id.clone()).expect("redo");
    assert!(redo.can_undo);

    assert!(
        page_contains_text(&session_id, 0, "APPROVED"),
        "Page 0 must contain 'APPROVED' after redo"
    );
}

#[test]
fn test_m3_4_unicode_and_special_characters_stamps_and_headers() {
    let session_id = open_test_session();

    // 1. Apply watermark with German umlauts, French accents, and symbols
    let stamp_options = FfiStampOrWatermarkOptions {
        text: "ENTWURF – Vertraulich & Geprüft © 2026 (€)".to_owned(),
        opacity_milli: 650,
        rotation_degrees: -30,
        r: 200,
        g: 40,
        b: 40,
        is_background: false,
        rect: None,
    };
    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let tx = document_core_apply_stamp_or_watermark_with_options(
        session_id.clone(),
        vec![0],
        stamp_options,
        wb_options,
    )
    .expect("apply unicode stamp");
    assert_eq!(tx.revision_after, tx.revision_before + 1);

    // 2. Apply Header & Footer with special characters
    let hf_options = FfiHeaderFooterOptions {
        header_left: Some("Dossier d'activité #42 — Équipe A".to_owned()),
        header_center: Some("Seite {page}/{total}".to_owned()),
        header_right: Some("München / Zürich".to_owned()),
        footer_left: Some("CONFIDENTIEL • STRICTEMENT INTERNE".to_owned()),
        footer_center: Some("{filename}".to_owned()),
        footer_right: Some("Stand: {date}".to_owned()),
        font_size_pt: 8,
        margin_pt: 28,
    };

    let tx_hf = document_core_apply_header_footer_with_options(
        session_id.clone(),
        vec![0, 1],
        hf_options,
        Some("Vocument-Projektübersicht".to_owned()),
        Some("2026-10-25".to_owned()),
        wb_options,
    )
    .expect("apply header footer with special characters");
    assert_eq!(tx_hf.revision_after, tx_hf.revision_before + 1);

    // 3. Verify extracted page contents render without corruption or crash
    let exported = document_core_extract_pages(session_id.clone(), vec![0])
        .expect("extract page with unicode content");
    assert!(!exported.is_empty());

    // 4. Undo and redo both operations
    let undo1 = document_core_undo(session_id.clone()).expect("undo header footer");
    assert!(undo1.can_undo);
    let undo2 = document_core_undo(session_id.clone()).expect("undo watermark");
    assert!(!undo2.can_undo);
    assert!(undo2.can_redo);

    let redo1 = document_core_redo(session_id.clone()).expect("redo watermark");
    assert!(redo1.can_undo);
    let redo2 = document_core_redo(session_id.clone()).expect("redo header footer");
    assert!(redo2.can_undo);
}

/// Helper that builds a minimal cryptographically signed PDF.
fn create_signed_test_pdf() -> Vec<u8> {
    let contents = "BT /F1 12 Tf 72 740 Td (Signed Document Invariant) Tj ET\n";
    let placeholder = "[0000000000 0000000000 0000000000 0000000000]";
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 6 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        "<< /Type /Annot /Subtype /Widget /FT /Sig /T (SignatureField) /V 7 0 R /Rect [72 100 320 140] /P 3 0 R >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{contents}endstream", contents.len()),
        format!("<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /Name (Auditor) /ByteRange {placeholder} /Contents <{:0>40}> >>", "A"),
    ];

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n%\x80\x81\x82\x83\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, obj).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn test_m3_4_digital_signature_protection_fail_closed() {
    let signed_bytes = create_signed_test_pdf();
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: "workbench://signed_test.pdf".to_owned(),
        display_name: "signed_test.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some("wb-sig-test".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: signed_bytes,
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open signed file");
    let session_id = snap.session_id;

    let wb_options_blocked = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // 1. Bates numbering must fail-closed
    let bates_res = document_core_apply_bates_numbering_with_options(
        session_id.clone(),
        vec![0],
        FfiBatesNumberingOptions {
            prefix: "SIG-".to_owned(),
            start_number: 1,
            digit_padding: 6,
            suffix: String::new(),
            position: FfiBatesPosition::BottomRight,
            font_size_pt: 10,
            margin_pt: 36,
        },
        wb_options_blocked,
    );
    assert!(
        matches!(
            bates_res,
            Err(DocumentCoreError::OperationBlockedBySignature { .. })
        ),
        "Bates on signed document must fail-closed with OperationBlockedBySignature"
    );

    // 2. Header / Footer must fail-closed
    let hf_res = document_core_apply_header_footer_with_options(
        session_id.clone(),
        vec![0],
        FfiHeaderFooterOptions {
            header_left: Some("Header".to_owned()),
            header_center: None,
            header_right: None,
            footer_left: None,
            footer_center: None,
            footer_right: None,
            font_size_pt: 10,
            margin_pt: 36,
        },
        None,
        None,
        wb_options_blocked,
    );
    assert!(
        matches!(
            hf_res,
            Err(DocumentCoreError::OperationBlockedBySignature { .. })
        ),
        "Header/Footer on signed document must fail-closed with OperationBlockedBySignature"
    );

    // 3. Set Bookmarks must fail-closed
    let bm_res = document_core_set_bookmarks_with_options(
        session_id.clone(),
        vec![FfiBookmarkItem {
            title: "Signed Outline".to_owned(),
            target_page: 0,
            children: Vec::new(),
        }],
        wb_options_blocked,
    );
    assert!(
        matches!(
            bm_res,
            Err(DocumentCoreError::OperationBlockedBySignature { .. })
        ),
        "Bookmarks on signed document must fail-closed with OperationBlockedBySignature"
    );

    // 4. Stamps must fail-closed
    let stamp_res = document_core_apply_stamp_or_watermark_with_options(
        session_id.clone(),
        vec![0],
        FfiStampOrWatermarkOptions {
            text: "FORBIDDEN".to_owned(),
            opacity_milli: 500,
            rotation_degrees: 0,
            r: 255,
            g: 0,
            b: 0,
            is_background: false,
            rect: None,
        },
        wb_options_blocked,
    );
    assert!(
        matches!(
            stamp_res,
            Err(DocumentCoreError::OperationBlockedBySignature { .. })
        ),
        "Stamps on signed document must fail-closed with OperationBlockedBySignature"
    );

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_4_repeated_crash_recovery_cycle() {
    let session_id_1 = open_test_session();

    // Operation 1: Apply Bates Numbering
    let wb_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };
    let _ = document_core_apply_bates_numbering_with_options(
        session_id_1.clone(),
        vec![0, 1],
        FfiBatesNumberingOptions {
            prefix: "CRASH-".to_owned(),
            start_number: 100,
            digit_padding: 4,
            suffix: String::new(),
            position: FfiBatesPosition::BottomRight,
            font_size_pt: 10,
            margin_pt: 36,
        },
        wb_options,
    )
    .expect("apply bates");

    let journal_1 = document_core_export_transaction_journal_json(session_id_1.clone())
        .expect("export journal 1");
    let _ = document_core_close_session(session_id_1);

    // Simulate Recovery 1
    let session_id_2 = open_test_session();
    let history_2 = document_core_restore_transaction_journal_json(session_id_2.clone(), journal_1)
        .expect("restore journal 1");
    assert!(history_2.can_undo);

    // Undo after Crash 1
    let undo_after_crash = document_core_undo(session_id_2.clone()).expect("undo after crash 1");
    assert!(undo_after_crash.can_redo);

    // Mutate after Crash 1: Apply Stamp
    let _ = document_core_apply_stamp_or_watermark_with_options(
        session_id_2.clone(),
        vec![0],
        FfiStampOrWatermarkOptions {
            text: "RECOVERED".to_owned(),
            opacity_milli: 500,
            rotation_degrees: 45,
            r: 0,
            g: 100,
            b: 200,
            is_background: true,
            rect: None,
        },
        wb_options,
    )
    .expect("apply stamp");

    let journal_2 = document_core_export_transaction_journal_json(session_id_2.clone())
        .expect("export journal 2");
    let _ = document_core_close_session(session_id_2);

    // Simulate Recovery 2
    let session_id_3 = open_test_session();
    let history_3 = document_core_restore_transaction_journal_json(session_id_3.clone(), journal_2)
        .expect("restore journal 2");
    assert!(history_3.can_undo);

    assert!(page_contains_text(&session_id_3, 0, "RECOVERED"));

    // Undo stamp
    let undo_stamp = document_core_undo(session_id_3.clone()).expect("undo stamp");
    assert!(undo_stamp.can_redo);
    assert!(!page_contains_text(&session_id_3, 0, "RECOVERED"));

    // Redo stamp
    let redo_stamp = document_core_redo(session_id_3.clone()).expect("redo stamp");
    assert!(redo_stamp.can_undo);
    assert!(page_contains_text(&session_id_3, 0, "RECOVERED"));

    // Simulate Crash 3: Export journal and restore once more
    let journal_3 = document_core_export_transaction_journal_json(session_id_3.clone())
        .expect("export journal 3");
    let _ = document_core_close_session(session_id_3);

    let session_id_4 = open_test_session();
    let history_4 = document_core_restore_transaction_journal_json(session_id_4.clone(), journal_3)
        .expect("restore journal 3");
    assert!(history_4.can_undo);
    assert!(page_contains_text(&session_id_4, 0, "RECOVERED"));

    let _ = document_core_close_session(session_id_4);
}

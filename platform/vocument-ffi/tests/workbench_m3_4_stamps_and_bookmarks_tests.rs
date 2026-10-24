//! Integration tests for M3.4 Bates Numbering, Headers/Footers, Bookmarks, and Stamps (`ROADMAP.md` M3.4).

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
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
    document_core_index_search_batch, document_core_open_brokered_file, document_core_search_exact,
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

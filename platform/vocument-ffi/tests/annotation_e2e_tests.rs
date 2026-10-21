//! End-to-end proof that the comment of the reviewer works across the whole
//! chain (`ANNOT-001`).
//!
//! A note is only worth something if it reaches the document, survives the
//! save, can be read back, and can be taken back. The tests walk the chain
//! across the FFI boundary: a document is opened, a note is written, the
//! document is saved, the file is opened again — and the note is still there.

use vocument_ffi::annotation_dto::{
    FfiCommentAnnotation, FfiJuristPaletteEntry, document_core_add_comment_annotation,
    document_core_default_jurist_palette, document_core_jurist_palette,
    document_core_page_comment_annotations, document_core_remove_comment_annotation,
    document_core_set_jurist_palette,
};
use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::save_dto::document_core_export_session_bytes;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::{document_core_get_session_snapshot, document_core_open_brokered_file};

/// Builds a document with one page and a line of text.
fn document_with_one_page() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (Vertrag) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
    ];

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::new();
    for object in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(object);
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
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

/// Opens the document in a session.
fn open_session(pdf_bytes: Vec<u8>, token: &str) -> String {
    let snapshot = document_core_open_brokered_file(FfiBrokeredFileDescriptor {
        source_token: format!("test:{token}"),
        display_name: "comment.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::InMemoryBuffer,
        os_file_id: None,
        modified_unix_ms: None,
        bytes: pdf_bytes,
    })
    .expect("the document must be openable");
    snapshot.session_id
}

/// The note that the tests write.
fn note(annotation_id: &str, key: &str, contents: &str) -> FfiCommentAnnotation {
    FfiCommentAnnotation {
        annotation_id: annotation_id.to_owned(),
        page_index: 0,
        rect: vocument_ffi::navigation_search_dto::FfiPdfRectMilliPt {
            x0_milli_pt: 20_000,
            y0_milli_pt: 20_000,
            x1_milli_pt: 34_000,
            y1_milli_pt: 34_000,
        },
        palette_key: key.to_owned(),
        color_r: 0,
        color_g: 0,
        color_b: 0,
        contents: contents.to_owned(),
        author: Some("Dr. Klar".to_owned()),
    }
}

#[test]
fn a_note_reaches_the_document_and_is_read_back_with_its_meaning() {
    let session_id = open_session(document_with_one_page(), "comment-write");

    // The rail is available to the caller: it does not have to know the
    // colours, it chooses the key.
    let rail = document_core_jurist_palette();
    assert!(
        rail.len() >= 5,
        "the rail of the profession carries more than one colour: {}",
        rail.len()
    );
    let objection = rail
        .iter()
        .find(|entry| entry.key == "objection")
        .expect("the rail knows the objection");

    let history = document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_001", "objection", "Die Frist ist zu kurz."),
    )
    .expect("the note is written");
    assert!(history.is_dirty, "the document is changed");
    assert!(history.can_undo, "the note can be taken back");

    let comments = document_core_page_comment_annotations(session_id.clone(), 0)
        .expect("the notes of the page are readable");
    assert_eq!(comments.len(), 1, "the note stands on the page");
    assert_eq!(comments[0].annotation_id, "comment_001");
    assert_eq!(comments[0].contents, "Die Frist ist zu kurz.");
    assert_eq!(comments[0].author.as_deref(), Some("Dr. Klar"));
    // The colour comes from the rail: the caller did not set it.
    assert_eq!(comments[0].palette_key, "objection");
    assert_eq!(comments[0].color_r, objection.color_r);
    assert_eq!(comments[0].color_g, objection.color_g);
    assert_eq!(comments[0].color_b, objection.color_b);
}

#[test]
fn a_note_survives_the_save_and_is_found_in_the_file_again() {
    let session_id = open_session(document_with_one_page(), "comment-save");
    document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_002", "note", "Bitte rückmelden."),
    )
    .expect("the note is written");

    let bytes =
        document_core_export_session_bytes(session_id.clone()).expect("the bytes of the document");
    // The identifier of the note stands in the file as a plain string. The
    // text of the note is written in the encoding of the document — the
    // umlaut becomes an escape sequence — and is proven by the round trip
    // below: a note can only be read back from the file.
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("comment_002"), "the note stands in the file");

    // The file is opened again: the note stands in the document, not only in
    // the memory of the process.
    let reopened = open_session(bytes, "comment-reopen");
    let comments = document_core_page_comment_annotations(reopened.clone(), 0)
        .expect("the notes of the page are readable");
    assert_eq!(comments.len(), 1, "the note survived the save");
    assert_eq!(comments[0].contents, "Bitte rückmelden.");
    assert_eq!(comments[0].palette_key, "note");
}

#[test]
fn a_note_can_be_taken_back_and_brought_again() {
    let session_id = open_session(document_with_one_page(), "comment-undo");
    document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_003", "question", "Wer haftet?"),
    )
    .expect("the note is written");
    assert_eq!(
        document_core_page_comment_annotations(session_id.clone(), 0)
            .expect("the notes")
            .len(),
        1
    );

    let undone = document_core_undo(session_id.clone()).expect("the note is taken back");
    assert!(undone.can_redo, "the note can be brought again");
    assert!(
        document_core_page_comment_annotations(session_id.clone(), 0)
            .expect("the notes")
            .is_empty(),
        "the note is gone"
    );

    let redone = document_core_redo(session_id.clone()).expect("the note is brought again");
    assert!(!redone.can_redo, "the note cannot be brought again");
    let comments = document_core_page_comment_annotations(session_id.clone(), 0)
        .expect("the notes of the page are readable");
    assert_eq!(comments.len(), 1, "the note is there again");
    assert_eq!(comments[0].contents, "Wer haftet?");
}

#[test]
fn a_removed_note_is_gone_from_the_page() {
    let session_id = open_session(document_with_one_page(), "comment-remove");
    document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_004", "agreed", "Einverstanden."),
    )
    .expect("the note is written");

    document_core_remove_comment_annotation(session_id.clone(), 0, "comment_004".to_owned())
        .expect("the note is removed");
    assert!(
        document_core_page_comment_annotations(session_id.clone(), 0)
            .expect("the notes")
            .is_empty(),
        "the note is gone"
    );
}

#[test]
fn a_note_with_a_key_that_the_rail_does_not_carry_is_refused_and_the_document_stays() {
    let session_id = open_session(document_with_one_page(), "comment-refused");
    let before = document_core_export_session_bytes(session_id.clone()).expect("the bytes");

    let error = document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_005", "rosa", "Einwand"),
    )
    .expect_err("a note without a place on the rail is refused");
    assert!(
        error.to_string().contains("rosa"),
        "the error names the key: {error}"
    );

    let after = document_core_export_session_bytes(session_id.clone()).expect("the bytes");
    assert_eq!(before, after, "the document is untouched");
    assert!(
        !document_core_get_session_snapshot(session_id.clone())
            .expect("the state of the session")
            .is_dirty,
        "the document is not changed"
    );
}

#[test]
fn the_rail_of_the_user_may_replace_the_default() {
    let session_id = open_session(document_with_one_page(), "comment-rail");
    let before = document_core_jurist_palette();

    document_core_set_jurist_palette(vec![FfiJuristPaletteEntry {
        key: "house".to_owned(),
        label: "Hausfarbe".to_owned(),
        meaning: "Die Farbe des Hauses.".to_owned(),
        color_r: 12,
        color_g: 34,
        color_b: 56,
    }])
    .expect("the rail is set");

    let rail = document_core_jurist_palette();
    assert_eq!(rail.len(), 1, "the rail of the user has one entry");
    assert_eq!(rail[0].key, "house");
    assert_ne!(rail.len(), before.len(), "the rail was replaced");

    document_core_add_comment_annotation(session_id.clone(), note("comment_006", "house", "Haus"))
        .expect("the note is written");
    let comments = document_core_page_comment_annotations(session_id.clone(), 0)
        .expect("the notes of the page are readable");
    assert_eq!(
        comments[0].color_r, 12,
        "the colour comes from the rail of the user"
    );
    assert_eq!(comments[0].palette_key, "house");
}

/// The key of the rail travels with the note. A note stays interpretable
/// even when the rail has changed in the meantime: the note says which
/// meaning it was written with, not only which colour it has.
#[test]
fn the_key_of_the_rail_travels_with_the_note() {
    let session_id = open_session(document_with_one_page(), "comment-key");
    document_core_add_comment_annotation(
        session_id.clone(),
        note("comment_007", "objection", "Einwand"),
    )
    .expect("the note is written");

    // The rail of the user is replaced: the colour of the note is not in it
    // any more. The note keeps its key — the meaning does not depend on the
    // rail of the moment.
    document_core_set_jurist_palette(vec![FfiJuristPaletteEntry {
        key: "house".to_owned(),
        label: "Hausfarbe".to_owned(),
        meaning: "Die Farbe des Hauses.".to_owned(),
        color_r: 12u8,
        color_g: 34u8,
        color_b: 56u8,
    }])
    .expect("the rail is set");

    let comments = document_core_page_comment_annotations(session_id.clone(), 0)
        .expect("the notes of the page are readable");
    assert_eq!(comments.len(), 1, "the note is still there");
    assert_eq!(
        comments[0].palette_key, "objection",
        "the note keeps the key it was written with: {:?}",
        comments[0]
    );
    assert_eq!(comments[0].color_r, 235, "the note keeps its colour");

    // The rail of the user is put back, so that the other tests see the
    // standard rail.
    document_core_set_jurist_palette(document_core_default_jurist_palette())
        .expect("the standard rail is set");
}

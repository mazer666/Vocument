//! Tests of the comment of the reviewer (`ANNOT-001`).
//!
//! A comment is a standard PDF `/Text` annotation with a popup. The engine
//! has to write it into the document, find it again, and remove it again —
//! and the text of the note has to survive the way through, because the
//! note is the work of the reviewer.

use vocument_core::pdf_engine::{PdfCommentAnnotation, PdfEngine, PdfRectMilliPt};

use crate::MuPdfEngineAdapter;

/// Builds a document with one page and a line of text.
fn document_with_one_page() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (Vertrag) Tj ET\n";
    // Die Objektnummern laufen ohne Lücke: eine Lücke im Kreuzverzeichnis
    // lässt MuPDF das Dokument reparieren, und die Reparatur verliert die
    // Anmerkung, die gerade geschrieben wurde.
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

/// The comment that the tests write.
fn comment(annotation_id: &str, contents: &str) -> PdfCommentAnnotation {
    PdfCommentAnnotation {
        annotation_id: annotation_id.to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 20_000,
            y0_milli_pt: 20_000,
            x1_milli_pt: 34_000,
            y1_milli_pt: 34_000,
        },
        palette_key: "objection".to_owned(),
        palette_label: "Einwand".to_owned(),
        palette_meaning: "Der Prüfer widerspricht.".to_owned(),
        color_rgb: [235, 90, 80],
        contents: contents.to_owned(),
        author: Some("Dr. Klar".to_owned()),
    }
}

#[test]
fn a_comment_is_written_into_the_document_and_found_again() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");

    let written = engine
        .add_comment_annotation(&comment("comment_001", "Die Frist ist zu kurz."))
        .expect("the comment is written");
    assert!(!written.is_empty(), "the document carries the note");

    let found = engine
        .page_comment_annotations(0)
        .expect("the comments of the page are read");
    assert_eq!(found.len(), 1, "the note stands on the page");
    assert_eq!(found[0].annotation_id, "comment_001");
    assert_eq!(found[0].contents, "Die Frist ist zu kurz.");
    assert_eq!(found[0].author.as_deref(), Some("Dr. Klar"));
    assert_eq!(found[0].color_rgb, [235, 90, 80]);
    assert_eq!(found[0].page_index, 0);
}

/// The name of the rail and its meaning travel with the note.
///
/// A note that carries only a key says nothing to a reader who does not have
/// the rail of the author — and the rail of the user may be replaced years
/// later. The note therefore carries the name and the meaning of the rail
/// that the note was written with.
#[test]
fn the_note_carries_the_name_and_the_meaning_of_the_rail() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");

    engine
        .add_comment_annotation(&comment("comment_003", "Einwand."))
        .expect("the comment is written");

    let found = engine
        .page_comment_annotations(0)
        .expect("the comments of the page are read");
    assert_eq!(found.len(), 1, "the note stands on the page");
    assert_eq!(found[0].palette_key, "objection", "the key travels as well");
    assert_eq!(
        found[0].palette_label, "Einwand",
        "the name of the rail travels with the note"
    );
    assert_eq!(
        found[0].palette_meaning, "Der Prüfer widerspricht.",
        "the meaning of the rail travels with the note"
    );
}

#[test]
fn the_note_survives_the_save_of_the_document() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");
    engine
        .add_comment_annotation(&comment("comment_002", "Bitte prüfen."))
        .expect("the comment is written");

    // Das Dokument wird neu geöffnet: die Notiz steht im Dokument, nicht nur
    // im Speicher des Prozesses.
    let reopened = adapter
        .open_pdf_bytes(engine.current_pdf_bytes())
        .expect("the document opens again");
    let found = reopened
        .page_comment_annotations(0)
        .expect("the comments of the page are read");
    assert_eq!(found.len(), 1, "the note survived the save");
    assert_eq!(found[0].contents, "Bitte prüfen.");
    assert_eq!(found[0].annotation_id, "comment_002");
}

#[test]
fn a_removed_comment_is_gone_and_hands_its_text_back() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");
    engine
        .add_comment_annotation(&comment("comment_003", "Zurückgegeben."))
        .expect("the comment is written");

    let (removed, _) = engine
        .remove_comment_annotation(0, "comment_003")
        .expect("the comment is removed");
    assert_eq!(removed.contents, "Zurückgegeben.");
    assert_eq!(removed.annotation_id, "comment_003");

    let found = engine
        .page_comment_annotations(0)
        .expect("the comments of the page are read");
    assert!(found.is_empty(), "the note is gone from the page");
}

#[test]
fn a_note_that_carries_no_text_is_refused() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");
    let before = engine.current_pdf_bytes().to_vec();

    let error = engine
        .add_comment_annotation(&comment("comment_004", "   "))
        .expect_err("a note without a text is refused");
    assert!(
        error.to_string().to_lowercase().contains("text"),
        "the error names the missing text: {error}"
    );
    assert_eq!(
        engine.current_pdf_bytes(),
        before.as_slice(),
        "the document is untouched"
    );
}

#[test]
fn a_comment_on_a_page_that_does_not_exist_is_refused() {
    let adapter = MuPdfEngineAdapter::new();
    let mut engine = adapter
        .open_pdf_bytes(&document_with_one_page())
        .expect("the engine opens the document");
    let mut on_missing_page = comment("comment_005", "Zu viel.");
    on_missing_page.page_index = 7;

    let error = engine
        .add_comment_annotation(&on_missing_page)
        .expect_err("a note on a missing page is refused");
    assert!(
        error.to_string().contains('7'),
        "the error names the page: {error}"
    );
}

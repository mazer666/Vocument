//! Tests of the redaction on pages that are turned (`REDACT-001`).
//!
//! A page can carry a `/Rotate` entry: the content is written as usual, but
//! the page is shown turned by 90, 180 or 270 degrees. The boxes of the text
//! are then not in the order "upper left" and "lower right" any more — on a
//! page that is turned by 90 degrees the upper corner of a character lies on
//! the right. A box that is built from those two corners alone comes out
//! inverted, and an inverted box matches nothing when a region is redacted:
//! the text survives the redaction while the user believes it was removed.
//!
//! The tests therefore turn the page and check that the word boxes are
//! normalised and that the redaction really removes the word.

use vocument_core::pdf_engine::{PdfEngine, PdfRectMilliPt};

/// The word that is written on the page and that has to disappear.
const SECRET: &str = "VERTRAULICH";

/// Builds a page with a single line of text and the given `/Rotate` entry.
fn document_with_rotation(rotate: i32) -> Vec<u8> {
    let content =
        format!("BT /F1 18 Tf 20 100 Td ({SECRET}) Tj ET\nBT /F1 18 Tf 20 60 Td (Anhang) Tj ET\n");
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Rotate {rotate} /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n"
        )
        .into_bytes(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj =
                format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
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

/// The box of the word with the given text.
fn box_of<'a>(
    words: &'a [vocument_core::pdf_engine::PdfTextWord],
    text: &str,
) -> &'a PdfRectMilliPt {
    words
        .iter()
        .find(|word| word.text == text)
        .map(|word| &word.bbox)
        .unwrap_or_else(|| panic!("the page must carry the word {text}"))
}

#[test]
fn the_word_boxes_of_a_turned_page_are_normalised() {
    for rotate in [0, 90, 180, 270] {
        let bytes = document_with_rotation(rotate);
        let engine = crate::MuPdfEngineAdapter::new();
        let document = engine
            .open_pdf_bytes(&bytes)
            .expect("the turned page must be openable");
        let text = document
            .extract_page_text(0)
            .expect("the turned page must have a text layer");

        assert!(
            text.words.iter().any(|word| word.text == SECRET),
            "the word must be found on a page that is turned by {rotate} degrees"
        );
        for word in &text.words {
            assert!(
                word.bbox.x0_milli_pt <= word.bbox.x1_milli_pt,
                "the box of {word:?} must not be inverted in x on a page that is turned by {rotate} degrees"
            );
            assert!(
                word.bbox.y0_milli_pt <= word.bbox.y1_milli_pt,
                "the box of {word:?} must not be inverted in y on a page that is turned by {rotate} degrees"
            );
        }
    }
}

#[test]
fn a_redaction_removes_the_word_on_a_turned_page() {
    for rotate in [0, 90, 180, 270] {
        let bytes = document_with_rotation(rotate);
        let engine = crate::MuPdfEngineAdapter::new();
        let mut document = engine
            .open_pdf_bytes(&bytes)
            .expect("the turned page must be openable");

        let before = document
            .extract_page_text(0)
            .expect("the turned page must have a text layer");
        let region = *box_of(&before.words, SECRET);

        let redacted = document
            .redact_page_regions(0, &[region])
            .unwrap_or_else(|error| {
                panic!("the redaction must succeed on a page that is turned by {rotate} degrees: {error:?}")
            });

        // The word is gone from the text layer of the redacted document.
        let redacted_document = engine
            .open_pdf_bytes(&redacted)
            .expect("the redacted document must be openable");
        let after = redacted_document
            .extract_page_text(0)
            .expect("the redacted page must have a text layer");
        assert!(
            !after.words.iter().any(|word| word.text == SECRET),
            "the word must be gone on a page that is turned by {rotate} degrees, but the page still carries {:?}",
            after
                .words
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>()
        );

        // The other line of the page is untouched.
        assert!(
            after.words.iter().any(|word| word.text == "Anhang"),
            "the other text of the page must survive on a page that is turned by {rotate} degrees"
        );
    }
}

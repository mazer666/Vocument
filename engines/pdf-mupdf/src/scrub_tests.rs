//! Tests of the removal of a redacted word from the places of a document that
//! do not lie on the page (`REDACT-002`).
//!
//! The redaction of a page removes the content of the page. The same text can
//! survive in the text of an annotation, in the value of a form field and in
//! the metadata. A redaction that reports success while the text is still in
//! the document is a lie — the tests below make sure that the text is removed
//! and that a place that cannot be cleaned is reported.

use vocument_core::pdf_engine::PdfEngine;

use crate::scrub::{find_word_occurrences, scrub_words};

/// The word that is written in the annotation and in the form field.
const SECRET: &str = "GEHEIM";

/// Builds a page with a text line, an annotation and a form field — all three
/// carry the secret.
fn document_with_annotation_and_field() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 6 0 R >> >> /Contents 7 0 R /Annots [4 0 R 5 0 R] >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents (GEHEIM) >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /Annot /Subtype /Widget /FT /Tx /T (note) /Rect [60 60 160 80] /V (GEHEIM) /DA (/F1 0 Tf 0 g) >>\nendobj\n".to_vec(),
        b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj =
                format!("7 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
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

#[test]
fn the_text_of_an_annotation_and_of_a_form_field_is_found() {
    let bytes = document_with_annotation_and_field();
    let occurrences =
        find_word_occurrences(&bytes, &[SECRET.to_owned()]).expect("the document must be readable");
    let kinds: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.kind.as_str())
        .collect();
    assert!(
        kinds.contains(&"annotation"),
        "the text of the annotation must be found, found: {kinds:?}"
    );
    assert!(
        kinds.contains(&"form"),
        "the value of the form field must be found, found: {kinds:?}"
    );
}

#[test]
fn the_text_of_an_annotation_and_of_a_form_field_is_emptied() {
    let bytes = document_with_annotation_and_field();
    let (scrubbed, report) =
        scrub_words(&bytes, &[SECRET.to_owned()]).expect("the document must be scrubable");
    assert_eq!(
        report.scrubbed_annotations, 1,
        "the text of the annotation must be emptied"
    );
    assert_eq!(
        report.scrubbed_form_values, 1,
        "the value of the form field must be emptied"
    );

    let occurrences = find_word_occurrences(&scrubbed, &[SECRET.to_owned()])
        .expect("the scrubbed document must be readable");
    assert!(
        occurrences.is_empty(),
        "the secret must not be found any more, found: {occurrences:?}"
    );

    // The rest of the document survived: the text of the page is still there.
    let engine = crate::MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&scrubbed)
        .expect("the scrubbed document must be openable");
    let text = document
        .extract_page_text(0)
        .expect("the text of the page must be extractable");
    assert!(
        text.words.iter().any(|word| word.text == "VERTRAULICH"),
        "the text of the page must survive the scrubbing"
    );
}

#[test]
fn a_word_that_is_not_in_the_document_is_not_reported() {
    let bytes = document_with_annotation_and_field();
    let occurrences = find_word_occurrences(&bytes, &["NICHTDA".to_owned()])
        .expect("the document must be readable");
    assert!(
        occurrences.is_empty(),
        "a word that is not in the document must not be reported, found: {occurrences:?}"
    );
}

#[test]
fn a_word_that_is_hex_encoded_is_found() {
    // The text of an annotation can be stored as the hex string of the UTF-16
    // bytes. A comparison of the plain text alone does not see it.
    let hex = "<FEFF00470045004800450049004D>";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Annots [4 0 R] >>\nendobj\n".to_vec(),
        format!("4 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents {hex} >>\nendobj\n").into_bytes(),
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

    let occurrences =
        find_word_occurrences(&pdf, &[SECRET.to_owned()]).expect("the document must be readable");
    assert!(
        occurrences.iter().any(|entry| entry.kind == "annotation"),
        "the hex encoded text of the annotation must be found, found: {occurrences:?}"
    );
}

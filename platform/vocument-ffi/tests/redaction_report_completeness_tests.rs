//! End-to-end proof that the report of the places is complete (`REDACT-002`).
//!
//! A redaction is verified by looking at **every** place of the document
//! that could still carry the text. The list of those places must be
//! complete: a list that stops in the middle does not say that the places
//! behind it were examined — and a redaction that is verified on the strength
//! of a list that was cut off is a redaction that promises more than it
//! knows.
//!
//! The sixth round of the red team measured exactly that: a document with 201
//! pages that carry the secret and one place that the walk could not reach
//! was reported as "verified", because the limit of 200 places pushed the
//! note about the unreadable place out of the report.
//!
//! The test walks the whole chain across the FFI boundary: a document is
//! opened, a redaction is attempted, the refusal is checked and the bytes of
//! the document are compared before and after.

use vocument_ffi::redaction_dto::{FfiRedactionRegion, document_core_redact_page_regions};
use vocument_ffi::save_dto::document_core_export_session_bytes;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::{document_core_open_brokered_file, document_core_page_words};
mod common;

/// The word on the page.
const SECRET: &str = "GEHEIM";

/// The number of pages that carry the word. It is one more than the number
/// of places that the report of the sixth round could hold — the document
/// has to be *longer* than the limit, otherwise the test proves nothing.
const PAGES: usize = 201;

/// The depth of the chain of dictionaries that the walk cannot reach. The
/// walk stops at a depth of 48.
const DEPTH: usize = 60;

/// Builds a document with many pages that carry the secret and one place
/// that the walk cannot reach.
fn document_with_many_places() -> Vec<u8> {
    // The numbering of the objects: 1 the catalogue, 2 the tree of the pages,
    // 3 the font, 4..(4 + DEPTH) the chain, then two objects per page.
    let chain_start = 4usize;
    let chain_end = chain_start + DEPTH;
    let first_page = chain_end;

    let content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\n");
    let kids: String = (0..PAGES)
        .map(|index| format!("{} 0 R ", first_page + index * 2))
        .collect();

    let mut objects: Vec<Vec<u8>> = vec![
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra {chain_start} 0 R >>\nendobj\n")
            .into_bytes(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {PAGES} >>\nendobj\n").into_bytes(),
        b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ];
    for index in 0..DEPTH {
        let number = chain_start + index;
        if index == DEPTH - 1 {
            objects
                .push(format!("{number} 0 obj\n<< /Secret ({SECRET}) >>\nendobj\n").into_bytes());
        } else {
            objects.push(
                format!("{number} 0 obj\n<< /Next {} 0 R >>\nendobj\n", number + 1).into_bytes(),
            );
        }
    }
    for index in 0..PAGES {
        let page_number = first_page + index * 2;
        let content_number = page_number + 1;
        objects.push(
            format!(
                "{page_number} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 3 0 R >> >> /Contents {content_number} 0 R >>\nendobj\n"
            )
            .into_bytes(),
        );
        let mut object = format!(
            "{content_number} 0 obj\n<< /Length {} >>\nstream\n",
            content.len()
        )
        .into_bytes();
        object.extend_from_slice(content.as_bytes());
        object.extend_from_slice(b"\nendstream\nendobj\n");
        objects.push(object);
    }

    let mut pdf: Vec<u8> = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len());
    for object in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(object);
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// Opens the document in a session.
fn open_session(pdf_bytes: Vec<u8>) -> String {
    let snapshot = document_core_open_brokered_file(FfiBrokeredFileDescriptor {
        source_token: "test:redaction-report-completeness".to_owned(),
        display_name: "redaction-report-completeness.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::InMemoryBuffer,
        os_file_id: None,
        modified_unix_ms: None,
        bytes: pdf_bytes,
    })
    .expect("the document must be openable");
    snapshot.session_id
}

/// A document that carries the word on more pages than the report could hold
/// is **not** verified when a place of it could not be examined.
///
/// The redaction has to be refused: the place that the walk could not reach
/// is named, and the document stays as it was. A redaction that is verified
/// on the strength of a list that was cut off would promise that a place was
/// examined that nobody ever looked into.
#[test]
fn a_place_that_the_walk_could_not_reach_is_named_even_in_a_long_document() {
    common::use_a_temporary_data_directory();
    let bytes = document_with_many_places();
    let session_id = open_session(bytes.clone());

    let words =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    let word = words
        .iter()
        .find(|word| word.text.eq_ignore_ascii_case(SECRET))
        .expect("the word must be in the text of the page");
    let region = FfiRedactionRegion {
        x0_milli_pt: word.rect.x0_milli_pt,
        y0_milli_pt: word.rect.y0_milli_pt,
        x1_milli_pt: word.rect.x1_milli_pt,
        y1_milli_pt: word.rect.y1_milli_pt,
    };

    let outcome = document_core_redact_page_regions(session_id.clone(), 0, vec![region]);
    let error = match outcome {
        Ok(report) => panic!("the redaction must not be verified: {report:?}"),
        Err(error) => error,
    };
    let details = format!("{error:?}");
    assert!(
        details.contains("could not be read completely")
            || details.contains("could not be looked into")
            || details.contains("not be looked into"),
        "the refusal has to name the place that could not be examined: {details}"
    );

    // A refused redaction leaves the document alone.
    let exported = document_core_export_session_bytes(session_id.clone()).expect("the export");
    assert_eq!(
        exported, bytes,
        "a refused redaction must not change the document"
    );

    vocument_ffi::document_core_close_session(session_id).expect("the session must close");
}

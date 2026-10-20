//! End-to-end proof that a redaction that cannot be verified leaves the
//! document alone (`REDACT-002`).
//!
//! A redaction is only worth something if it works. When it does not work, the
//! user must not hold a changed document while believing that nothing
//! happened: the removal has to be rolled back, and the failure has to be
//! reported.
//!
//! The tests walk the whole chain across the FFI boundary: a document is
//! opened, a redaction is attempted, and the bytes of the document are
//! compared before and after.

use vocument_ffi::redaction_dto::{FfiRedactionRegion, document_core_redact_page_regions};
use vocument_ffi::save_dto::document_core_export_session_bytes;
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::{document_core_open_brokered_file, document_core_page_words};

/// The word on the page. It is long enough that the redaction has to find it,
/// and short enough that a part of it can stay behind.
const SECRET: &str = "VERTRAULICH";

/// Builds a page with a single line of text.
fn document_with_secret() -> Vec<u8> {
    let content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\n");
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_vec(),
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

/// Opens the document in a session.
fn open_session(pdf_bytes: Vec<u8>) -> String {
    let snapshot = document_core_open_brokered_file(FfiBrokeredFileDescriptor {
        source_token: "test:redaction-hardening".to_owned(),
        display_name: "redaction-hardening.pdf".to_owned(),
        access_origin: FfiFileAccessOrigin::InMemoryBuffer,
        os_file_id: None,
        modified_unix_ms: None,
        bytes: pdf_bytes,
    })
    .expect("the document must be openable");
    snapshot.session_id
}

#[test]
fn a_redaction_that_only_covers_a_part_of_a_word_is_not_reported_as_successful() {
    let session_id = open_session(document_with_secret());

    let words =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    let word = words
        .iter()
        .find(|word| word.text.eq_ignore_ascii_case(SECRET))
        .expect("the word must be in the text of the page");

    // The region covers the first half of the word. The removal works
    // character by character, so the rest of the word stays in the document —
    // and a fragment is still content.
    let region = FfiRedactionRegion {
        x0_milli_pt: word.rect.x0_milli_pt,
        y0_milli_pt: word.rect.y0_milli_pt,
        x1_milli_pt: (word.rect.x0_milli_pt + word.rect.x1_milli_pt) / 2,
        y1_milli_pt: word.rect.y1_milli_pt,
    };

    let result = document_core_redact_page_regions(session_id.clone(), 0, vec![region]);
    assert!(
        result.is_err(),
        "a redaction that leaves a fragment of the word must not be reported as successful"
    );
}

#[test]
fn a_redaction_that_cannot_be_verified_leaves_the_document_unchanged() {
    let session_id = open_session(document_with_secret());
    let before = document_core_export_session_bytes(session_id.clone())
        .expect("the bytes of the document must be readable");

    let words =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    let word = words
        .iter()
        .find(|word| word.text.eq_ignore_ascii_case(SECRET))
        .expect("the word must be in the text of the page");
    let region = FfiRedactionRegion {
        x0_milli_pt: word.rect.x0_milli_pt,
        y0_milli_pt: word.rect.y0_milli_pt,
        x1_milli_pt: (word.rect.x0_milli_pt + word.rect.x1_milli_pt) / 2,
        y1_milli_pt: word.rect.y1_milli_pt,
    };

    let result = document_core_redact_page_regions(session_id.clone(), 0, vec![region]);
    assert!(
        result.is_err(),
        "the redaction cannot be verified and must fail"
    );

    let after = document_core_export_session_bytes(session_id.clone())
        .expect("the bytes of the document must be readable");
    assert_eq!(
        before, after,
        "a redaction that could not be verified must leave the document unchanged"
    );
}

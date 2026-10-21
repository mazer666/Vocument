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

/// Builds a document whose pages each carry a single line of text.
fn document_with_secret() -> Vec<u8> {
    document_with_secret_on_pages(1)
}

/// Builds a document with the given number of pages. Every page carries the
/// word — the same word on every page.
fn document_with_secret_on_pages(page_count: usize) -> Vec<u8> {
    let content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\n");
    // The objects: 1 the catalog, 2 the pages, 3.. the pages, then the font
    // and one content stream per page.
    let font_object = 3 + page_count as u32;
    let first_content_object = font_object + 1;
    let kids: String = (0..page_count)
        .map(|index| format!("{} 0 R ", 3 + index as u32))
        .collect();
    let mut objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {page_count} >>\nendobj\n")
            .into_bytes(),
    ];
    for index in 0..page_count as u32 {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 {font_object} 0 R >> >> /Contents {} 0 R >>\nendobj\n",
                3 + index,
                first_content_object + index
            )
            .into_bytes(),
        );
    }
    objects.push(
        format!(
            "{font_object} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n"
        )
        .into_bytes(),
    );
    for index in 0..page_count as u32 {
        let mut obj = format!(
            "{} 0 obj\n<< /Length {} >>\nstream\n",
            first_content_object + index,
            content.len()
        )
        .into_bytes();
        obj.extend_from_slice(content.as_bytes());
        obj.extend_from_slice(b"\nendstream\nendobj\n");
        objects.push(obj);
    }

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
fn a_region_that_only_covers_a_part_of_a_word_removes_the_whole_word() {
    let session_id = open_session(document_with_secret());

    let words =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    let word = words
        .iter()
        .find(|word| word.text.eq_ignore_ascii_case(SECRET))
        .expect("the word must be in the text of the page");

    // The region covers the first half of the word. The removal works
    // character by character, so the region grows to the whole word: a
    // fragment of a secret is still a secret.
    let region = FfiRedactionRegion {
        x0_milli_pt: word.rect.x0_milli_pt,
        y0_milli_pt: word.rect.y0_milli_pt,
        x1_milli_pt: (word.rect.x0_milli_pt + word.rect.x1_milli_pt) / 2,
        y1_milli_pt: word.rect.y1_milli_pt,
    };

    let report = document_core_redact_page_regions(session_id.clone(), 0, vec![region])
        .expect("the redaction must succeed");
    assert!(
        report.verified,
        "the whole word has to be gone, not a fragment of it"
    );

    // The word is not in the document any more — not in its text layer, not
    // in the bytes of the file.
    let after =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    assert!(
        !after
            .iter()
            .any(|word| word.text.eq_ignore_ascii_case(SECRET)),
        "the word must not be in the text any more"
    );
    let bytes = document_core_export_session_bytes(session_id.clone())
        .expect("the bytes of the document must be readable");
    assert!(
        !String::from_utf8_lossy(&bytes)
            .to_lowercase()
            .contains(SECRET.to_lowercase().as_str()),
        "the word must not be in the bytes of the file any more"
    );
}

#[test]
fn the_same_word_on_another_page_is_named_in_the_report() {
    let session_id = open_session(document_with_secret_on_pages(2));

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

    let report = document_core_redact_page_regions(session_id, 0, vec![region])
        .expect("the redaction of the marked page must succeed");

    // The word is still in the document, on the second page. The report says
    // so, so that nobody believes the word is gone.
    assert_eq!(
        report.remaining_occurrences.len(),
        1,
        "the place of the word is named: {:?}",
        report.remaining_occurrences
    );
    assert_eq!(report.remaining_occurrences[0].page_index, 1);
    assert!(report.verified, "the marked page is clean in every respect");
}

#[test]
fn a_region_without_content_is_refused_and_the_document_stays_unchanged() {
    let session_id = open_session(document_with_secret());
    let before = document_core_export_session_bytes(session_id.clone())
        .expect("the bytes of the document must be readable");

    // A region that touches no word at all. The redaction would remove
    // nothing, and a redaction that removes nothing must not be reported as
    // a success: the user would believe that something was removed.
    let region = FfiRedactionRegion {
        x0_milli_pt: 150_000,
        y0_milli_pt: 150_000,
        x1_milli_pt: 190_000,
        y1_milli_pt: 190_000,
    };

    let result = document_core_redact_page_regions(session_id.clone(), 0, vec![region]);
    assert!(
        result.is_err(),
        "a redaction that removes nothing must be refused"
    );

    let after = document_core_export_session_bytes(session_id.clone())
        .expect("the bytes of the document must be readable");
    assert_eq!(
        before, after,
        "a redaction that could not be verified must leave the document unchanged"
    );
}

/// Builds a document that carries the word twice on the same page — once at
/// the place the user marks, once somewhere else on the page.
fn document_with_the_word_twice_on_one_page() -> Vec<u8> {
    let content = format!(
        "BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\nBT /F1 18 Tf 20 60 Td ({SECRET}) Tj ET\n"
    );
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

/// The same word twice on the same page: the redaction removes the place the
/// user marked and reports the other place. A redaction that would refuse
/// every word that occurs twice would be useless — a name, a word that
/// matters, occurs in every document more than once.
#[test]
fn the_same_word_twice_on_the_same_page_is_redacted_and_the_other_place_is_named() {
    let session_id = open_session(document_with_the_word_twice_on_one_page());

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

    let report = document_core_redact_page_regions(session_id.clone(), 0, vec![region])
        .expect("the redaction of the marked place must succeed");
    assert!(
        report.verified,
        "the marked place is clean in every respect: {report:?}"
    );

    // The marked place is empty, the other place still carries the word — and
    // the report says so.
    let after =
        document_core_page_words(session_id.clone(), 0).expect("the words must be readable");
    assert_eq!(
        after
            .iter()
            .filter(|word| word.text.eq_ignore_ascii_case(SECRET))
            .count(),
        1,
        "the marked place is empty, the other place is not"
    );
    assert!(
        !report.remaining_occurrences.is_empty(),
        "the place of the word that is still in the document is named: {report:?}"
    );
}

/// Builds a document with an attached ZIP archive. The archive holds a
/// compressed file — the content of the archive cannot be read without
/// unpacking it.
fn document_with_a_zip_attachment() -> Vec<u8> {
    let content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\n");
    // A ZIP archive with one stored member: the header is enough for the
    // redaction to recognise the container.
    let archive: Vec<u8> = vec![
        0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut archive_object =
        format!("8 0 obj\n<< /Length {} >>\nstream\n", archive.len()).into_bytes();
    archive_object.extend_from_slice(&archive);
    archive_object.extend_from_slice(b"\nendstream\nendobj\n");

    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Names 6 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"6 0 obj\n<< /EmbeddedFiles 7 0 R >>\nendobj\n".to_vec(),
        b"7 0 obj\n<< /Names [(Anhang) 9 0 R] >>\nendobj\n".to_vec(),
        archive_object,
        b"9 0 obj\n<< /Type /Filespec /F (Anhang.zip) /EF << /F 8 0 R >> >>\nendobj\n".to_vec(),
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

/// An attached archive cannot be read without unpacking it. A redaction that
/// cannot prove that the text is gone from **every** place of the document
/// is not a secure redaction: it is refused, the document stays as it was,
/// and the place that could not be read is named.
///
/// A tool that reported "verified" here would promise what it cannot prove:
/// the secret can still stand in the archive.
#[test]
fn an_attached_archive_makes_the_redaction_refuse_and_names_the_place() {
    let session_id = open_session(document_with_a_zip_attachment());
    let before = document_core_export_session_bytes(session_id.clone()).expect("the bytes");

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

    let failure = document_core_redact_page_regions(session_id.clone(), 0, vec![region])
        .expect_err("the redaction must be refused when a place could not be read");
    let message = failure.to_string();
    assert!(
        message.contains("could not be read") || message.contains("could not be looked into"),
        "the error names the place that could not be read: {message}"
    );
    assert!(
        message.contains("EmbeddedFiles") || message.contains("attachment"),
        "the error names the place of the document: {message}"
    );

    let after = document_core_export_session_bytes(session_id).expect("the bytes");
    assert_eq!(before, after, "the document is unchanged");
}

/// Builds a document whose form XObject carries a second copy of the secret.
///
/// The copy stands in the content stream of an object that only the
/// dictionary of the form XObject refers to. The fourth round of the red
/// team showed that the walk did not descend into the dictionary of a stream
/// — the copy was never seen, and the redaction reported a success while the
/// word was still in the file.
fn document_with_a_copy_in_a_form_xobject() -> Vec<u8> {
    // The page draws the form, the form draws the inner form, and the inner
    // form carries the word a second time. The redaction of MuPDF reaches
    // into the content of the page — but not into the content of a form
    // object: the copy stays in the file.
    let page_content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\nq /Fm0 Do Q\n");
    let outer_form = "q /Fm1 Do Q\n";
    let inner_form = format!("BT /F1 18 Tf 20 100 Td ({SECRET}) Tj ET\n");
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> /XObject << /Fm0 6 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", page_content.len()).into_bytes();
            obj.extend_from_slice(page_content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        {
            let mut obj = format!("6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /XObject << /Fm1 7 0 R >> >> /Length {} >>\nstream\n", outer_form.len()).into_bytes();
            obj.extend_from_slice(outer_form.as_bytes());
            obj.extend_from_slice(b"endstream\nendobj\n");
            obj
        },
        {
            let mut obj = format!("7 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Length {} >>\nstream\n", inner_form.len()).into_bytes();
            obj.extend_from_slice(inner_form.as_bytes());
            obj.extend_from_slice(b"endstream\nendobj\n");
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

/// A copy of the secret in a form XObject makes the redaction refuse.
///
/// The content of a form XObject is a content stream, not a string: the
/// scrub cannot empty it, and MuPDF's redaction does not descend into a
/// form XObject. The copy therefore stays in the document — and the
/// redaction has to say so instead of reporting a success.
///
/// This is the proof across the whole chain: the fourth round of the red
/// team measured `verified = true` while the secret stood in the file.
#[test]
fn a_copy_in_a_form_xobject_makes_the_redaction_refuse_and_names_the_place() {
    let session_id = open_session(document_with_a_copy_in_a_form_xobject());
    let before = document_core_export_session_bytes(session_id.clone()).expect("the bytes");

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

    let failure = document_core_redact_page_regions(session_id.clone(), 0, vec![region])
        .expect_err("the redaction must be refused while the copy is in the document");
    let message = failure.to_string();
    assert!(
        message.contains("could not be verified")
            || message.contains("could not be removed")
            || message.contains("could not be read")
            || message.contains("redaction failed"),
        "the error has to say that the redaction did not work: {message}"
    );
    assert!(
        message.contains("Fm1") || message.contains("XObject"),
        "the error has to name the place where the copy stands: {message}"
    );

    // The document is untouched: the bytes before and after are the same.
    let after = document_core_export_session_bytes(session_id).expect("the bytes");
    assert_eq!(before, after, "the document has to be unchanged");
}

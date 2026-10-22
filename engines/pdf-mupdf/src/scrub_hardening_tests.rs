//! The hardening of the verification of a redaction (`REDACT-002`).
//!
//! The tests of this file prove three things:
//!
//! 1. a place of the document that the walk could **not** examine is named —
//!    the limit of the depth of the walk and an object that cannot be read
//!    must not pass for a place that was examined and found clean;
//! 2. the keys of an encrypted document are **examined** (the check has to
//!    see them) and **never emptied** (emptying a key destroys the document);
//! 3. the text of a content stream is **read**: a word that the writer split
//!    over several show operations is found — also as UTF-16 — and a secret
//!    that is encoded is found and removed.

use crate::scrub::{find_word_occurrences, find_word_occurrences_with_note_budget, scrub_words};

/// The word that stands in the places of the document.
const SECRET: &str = "GEHEIM";

/// Builds a page with an annotation that carries the secret and a **security
/// dictionary** whose keys carry the secret as well.
///
/// The dictionary is not the `/Encrypt` of the trailer — the trailer of this
/// document has none, so the document stays readable. It hangs in the
/// catalogue, where the sweep over the cross-reference table reaches it like
/// every other object of the file.
fn document_with_security_dictionary() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 6 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [7 0 R] >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        // The encryption dictionary: the two keys of the standard security
        // handler and the filter `/Standard`.
        b"6 0 obj\n<< /Filter /Standard /V 1 /R 2 /O (GEHEIM) /U (GEHEIM) /P -4 >>\nendobj\n".to_vec(),
        b"7 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents (GEHEIM) >>\nendobj\n".to_vec(),
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

/// The keys of the document are not the text of the user.
///
/// The fifth round of the red team measured a redaction that emptied the
/// `/U` of an encrypted document and broke it: the walk protected the key
/// `/Encrypt` of the trailer, but the specification asks for that key to be
/// an **indirect reference** — and then the dictionary is an object of the
/// file, which the sweep over the cross-reference table reaches by its
/// number. The dictionary is recognised by its content now and is never
/// descended into.
#[test]
fn the_keys_of_the_security_dictionary_are_not_emptied() {
    let bytes = document_with_security_dictionary();
    let (scrubbed, report) =
        scrub_words(&bytes, &[SECRET.to_owned()]).expect("the document must be scrubable");

    // The annotation that really carries the text of the user is cleaned …
    assert_eq!(
        report.scrubbed_annotations, 1,
        "the text of the annotation must be emptied"
    );
    // … and the keys of the document are not. The writer of MuPDF writes the
    // keys without a space, so the text is counted and not matched literally.
    let text = String::from_utf8_lossy(&scrubbed);
    assert_eq!(
        text.matches("GEHEIM").count(),
        2,
        "the two keys of the security dictionary must not be emptied: {text}"
    );
    assert!(
        text.contains("/O"),
        "the key /O of the security dictionary must survive: {text}"
    );
    assert!(
        text.contains("/U"),
        "the key /U of the security dictionary must survive: {text}"
    );
}

/// The keys of the document **are** a place that carries the text — for the
/// check, not for the cleaning.
///
/// The keys of an encrypted document are not the text of the user: they are
/// bytes that the writer computed, and they are never emptied. But a key
/// that happens to spell the text of the user stands in the document, and the
/// check has to see it: a check that cannot see it would report a success
/// while the text is in the file. The redaction refuses such a document and
/// names the place.
#[test]
fn the_security_dictionary_is_a_place_that_carries_the_text() {
    let bytes = document_with_security_dictionary();
    let occurrences =
        find_word_occurrences(&bytes, &[SECRET.to_owned()]).expect("the document must be readable");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.contains("/Extra")),
        "the security dictionary must be reported as a place: {places:?}"
    );
    assert!(
        places.iter().any(|place| place.ends_with("/Contents")),
        "the text of the annotation must be found, found: {places:?}"
    );
}

/// Builds a document whose secret lies **deeper** than the walk reaches.
///
/// The objects 7..67 form a chain of dictionaries, each of them pointing to
/// the next one. The last object of the chain carries the secret. The walk
/// stops at a depth of 48 — the secret is below that depth.
fn document_with_a_deep_chain() -> Vec<u8> {
    let depth = 60;
    let mut objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra 7 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>\nendobj\n".to_vec(),
        {
            let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
            let mut obj = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ];
    for index in 0..depth {
        let number = 7 + index;
        if number == 7 + depth - 1 {
            objects.push(format!("{number} 0 obj\n<< /Secret (GEHEIM) >>\nendobj\n").into_bytes());
        } else {
            objects.push(
                format!("{number} 0 obj\n<< /Next {} 0 R >>\nendobj\n", number + 1).into_bytes(),
            );
        }
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

/// A place that the walk could not examine is named.
///
/// A walk that stops somewhere without saying so is a walk that cannot be
/// trusted: the redaction would report a success while a place of the
/// document was never looked into. The limit of the depth is therefore
/// written down.
#[test]
fn a_place_that_the_walk_could_not_examine_is_named() {
    let bytes = document_with_a_deep_chain();
    let occurrences =
        find_word_occurrences(&bytes, &[SECRET.to_owned()]).expect("the document must be readable");
    let uninspected: Vec<&str> = occurrences
        .iter()
        .filter(|entry| entry.kind == "uninspected")
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        !uninspected.is_empty(),
        "a place that could not be looked into must be named: {occurrences:?}"
    );
    assert!(
        uninspected
            .iter()
            .any(|place| place.contains("the limit of the depth")),
        "the reason must be named: {uninspected:?}"
    );
}

/// Builds a page whose **form** carries the secret in two pieces.
///
/// The writer of a document may split a word over several show operations:
/// `(GE) (HEIM) Tj` puts "GEHEIM" on the page, but the bytes of the stream
/// do not carry the word in one piece. A comparison of bytes is blind for it.
fn document_with_split_text_in_a_form() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let form = "BT /F1 18 Tf 20 150 Td (GE) (HEIM) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> /XObject << /Fm0 6 0 R >> >> /Contents 4 0 R >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Length {} >>\nstream\n", form.len()).into_bytes();
            obj.extend_from_slice(form.as_bytes());
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

/// A word that is split over several show operations is found.
///
/// The fifth round of the red team measured that the search for the places
/// compares bytes: a secret that the writer split into `(GE) (HEIM)` was not
/// found, and the verification of the redaction accepted the document. The
/// text of the stream is read now — the pieces are joined, as long as no
/// operator moved the writing position.
#[test]
fn a_word_that_is_split_over_several_show_operations_is_found() {
    let bytes = document_with_split_text_in_a_form();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.contains("Fm0")),
        "the split text of the form must be found, found: {places:?}"
    );
}

/// Two words that stand next to each other are not one word.
///
/// The text of a page is read by joining the pieces of a run. Two words that
/// the writer wrote one after the other belong to two runs, because a
/// positioning operator stands between them.
#[test]
fn two_words_that_stand_next_to_each_other_are_not_one_word() {
    let bytes = document_with_split_text_in_a_form();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIMVERTRAULICH".to_owned()])
        .expect("the document must be readable");
    assert!(
        occurrences.is_empty(),
        "the two words must not be joined: {occurrences:?}"
    );
}

/// Builds a document whose form carries the secret in two pieces, both of
/// them as UTF-16 in the hex form.
///
/// The comparison of the text of a run knows the encodings of a PDF: plain
/// text, UTF-16 in either byte order and the hex string of the UTF-16 bytes.
fn document_with_a_split_utf16_form() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    // The form carries the secret in two pieces, both of them as UTF-16 in
    // the hex form: "GE" and "HEIM".
    let form = "BT /F1 18 Tf 20 150 Td <00470045> <004800450049004D> Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> /XObject << /Fm0 6 0 R >> >> /Contents 4 0 R >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Length {} >>\nstream\n", form.len()).into_bytes();
            obj.extend_from_slice(form.as_bytes());
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

#[test]
fn a_word_that_is_split_and_stored_as_utf16_is_found() {
    let bytes = document_with_a_split_utf16_form();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.contains("Fm0")),
        "the split text of the form must be found, found: {places:?}"
    );
}

/// Builds a document whose annotation carries the secret **encoded**.
///
/// "GEHEIM" as base64 is "R0VIRUlN". A reader who decodes it reads the
/// secret, and the bytes of the document do not carry the word in one piece.
fn document_with_an_encoded_secret() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R /Annots [6 0 R] >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        b"6 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents (R0VIRUlN) >>\nendobj\n".to_vec(),
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

/// A secret that is **encoded** is found and removed.
///
/// "GEHEIM" as base64 is "R0VIRUlN". The bytes of the document do not carry
/// the word in one piece, and a comparison of bytes therefore does not see
/// it — a reader who decodes the text reads the secret all the same.
#[test]
fn a_secret_that_is_encoded_is_found_and_emptied() {
    let bytes = document_with_an_encoded_secret();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.ends_with("/Contents")),
        "the encoded text of the annotation must be found, found: {places:?}"
    );

    let (scrubbed, report) =
        scrub_words(&bytes, &["GEHEIM".to_owned()]).expect("the document must be scrubable");
    assert_eq!(
        report.scrubbed_annotations, 1,
        "the encoded text of the annotation must be emptied"
    );
    let text = String::from_utf8_lossy(&scrubbed);
    assert!(
        !text.contains("R0VIRUlN"),
        "the encoded secret must not survive the cleaning: {text}"
    );
}

/// Builds a document with many pages that carry the secret and one place
/// that the walk cannot reach.
///
/// The places of the report are written in the order of the walk: the
/// content of the pages comes first, the places that could not be examined
/// come **last**. A report that is cut off before the end therefore loses
/// exactly the note that says that a place was never looked into.
fn document_with_many_places() -> Vec<u8> {
    const PAGES: usize = 201;
    const DEPTH: usize = 60;

    // The numbering of the objects: 1 the catalogue, 2 the tree of the pages,
    // 3 the font, 4..(4 + DEPTH) the chain, then two objects per page.
    let chain_start = 4usize;
    let chain_end = chain_start + DEPTH;
    let first_page = chain_end;

    let mut objects: Vec<Vec<u8>> = Vec::new();
    objects.push(
        format!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Extra {chain_start} 0 R >>\nendobj\n")
            .into_bytes(),
    );
    let kids: Vec<String> = (0..PAGES)
        .map(|index| format!("{} 0 R", first_page + index * 2))
        .collect();
    objects.push(
        format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {PAGES} >>\nendobj\n",
            kids.join(" ")
        )
        .into_bytes(),
    );
    objects.push(
        b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    );

    for index in 0..DEPTH {
        let number = chain_start + index;
        if index == DEPTH - 1 {
            objects.push(format!("{number} 0 obj\n<< /Secret (GEHEIM) >>\nendobj\n").into_bytes());
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
        let content = "BT /F1 18 Tf 20 150 Td (GEHEIM) Tj ET\n";
        let mut object = format!(
            "{content_number} 0 obj\n<< /Length {} >>\nstream\n",
            content.len()
        )
        .into_bytes();
        object.extend_from_slice(content.as_bytes());
        object.extend_from_slice(b"\nendstream\nendobj\n");
        objects.push(object);
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

/// A report that is cut off does not say that the places behind it were
/// examined.
///
/// The sixth round of the red team measured the defect: a document with 201
/// pages that carry the secret and one place that the walk could not reach
/// was reported as "verified" — the note about the unreadable place was
/// written last and was pushed out of the report by the limit of 200
/// places. The report must be complete, or the caller has to know that it
/// is not.
#[test]
fn the_report_of_the_places_is_not_cut_off() {
    let bytes = document_with_many_places();

    let occurrences =
        find_word_occurrences(&bytes, &[SECRET.to_owned()]).expect("the document must be readable");

    let content = occurrences
        .iter()
        .filter(|place| place.kind == "content")
        .count();
    assert!(
        content >= 201,
        "the report must name every page that carries the secret, found {content}"
    );

    let uninspected: Vec<&str> = occurrences
        .iter()
        .filter(|place| place.kind == "uninspected")
        .map(|place| place.location.as_str())
        .collect();
    assert!(
        !uninspected.is_empty(),
        "the place that the walk could not examine must be named — a report that is cut off \
         before the end does not say that a place was examined"
    );
}

/// A place that cannot be written down is not lost silently.
///
/// The walk writes down every place it could not examine. When the memory of
/// the machine is exhausted — or when the budget of the notes is too small —
/// the note does not fit. The walk then has to say that it could not examine
/// everything: a place that was never named must not pass for a place that
/// was examined and found clean.
///
/// The budget is the way to reproduce the case of the exhausted memory: it
/// can only make the walk stricter, never laxer.
#[test]
fn a_place_that_cannot_be_written_down_is_not_lost_silently() {
    let bytes = document_with_a_deep_chain();

    // A budget of one byte: no note fits.
    let occurrences = find_word_occurrences_with_note_budget(&bytes, &[SECRET.to_owned()], 1)
        .expect("the document must be readable");

    let uninspected: Vec<&str> = occurrences
        .iter()
        .filter(|place| place.kind == "uninspected")
        .map(|place| place.location.as_str())
        .collect();
    assert!(
        uninspected
            .iter()
            .any(|place| place.contains("could not write down")),
        "the walk must say that it could not write down every place: {uninspected:?}"
    );
}

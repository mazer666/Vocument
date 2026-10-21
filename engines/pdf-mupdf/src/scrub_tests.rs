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
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.ends_with("/Contents")),
        "the text of the annotation must be found, found: {places:?}"
    );
    assert!(
        places.iter().any(|place| place.ends_with("/V")),
        "the value of the form field must be found, found: {places:?}"
    );
    assert!(
        occurrences.iter().all(|entry| entry.kind == "hidden"),
        "every place that is not the content of a page is hidden text: {occurrences:?}"
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
            objects.push(
                format!("{number} 0 obj\n<< /Secret (GEHEIM) >>\nendobj\n").into_bytes(),
            );
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

/// Builds a page whose annotation carries the secret in its alternative text
/// and whose page carries an XMP packet with the secret.
fn document_with_alternative_text_and_page_metadata() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let xmp = "<?xpacket begin='' id='W5M0MpCehiHzreSzNTczkc9d'?><x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF><rdf:Description xmlns:dc='http://purl.org/dc/elements/1.1/'><dc:title><rdf:Alt><rdf:li xml:lang='x-default'>GEHEIM</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end='w'?>";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 6 0 R /Annots [4 0 R] /Metadata 7 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents (visible) /Alt (GEHEIM) >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj =
                format!("6 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        {
            let mut obj =
                format!("7 0 obj\n<< /Length {} >>\nstream\n", xmp.len()).into_bytes();
            obj.extend_from_slice(xmp.as_bytes());
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

/// The alternative text of an annotation and the XMP packet of a page carry
/// the secret — both places have to be found and both have to be cleaned.
#[test]
fn the_alternative_text_of_an_annotation_and_the_xmp_packet_of_a_page_are_found() {
    let engine = crate::MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&document_with_alternative_text_and_page_metadata())
        .expect("the document must be openable");

    let occurrences = document
        .find_word_occurrences(&[SECRET.to_owned()])
        .expect("the search must work");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();
    assert!(
        places.iter().any(|place| place.ends_with("/Alt")),
        "the alternative text of the annotation must be found: {places:?}"
    );
    assert!(
        places.iter().any(|place| place.ends_with("/Metadata")),
        "the XMP packet of the page must be found: {places:?}"
    );
}

#[test]
fn the_alternative_text_of_an_annotation_and_the_xmp_packet_of_a_page_are_emptied() {
    let (scrubbed, report) = scrub_words(
        &document_with_alternative_text_and_page_metadata(),
        &[SECRET.to_owned()],
    )
    .expect("the cleaning must work");
    assert!(
        report.scrubbed_metadata_fields > 0,
        "the XMP packet of the page must be cleaned: {report:?}"
    );

    let engine = crate::MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&scrubbed)
        .expect("the scrubbed document must be openable");
    let occurrences = document
        .find_word_occurrences(&[SECRET.to_owned()])
        .expect("the search must work");
    assert!(
        occurrences.is_empty(),
        "the secret must not stand in the document any more: {occurrences:?}"
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
        occurrences
            .iter()
            .any(|entry| entry.location.ends_with("/Contents")),
        "the hex encoded text of the annotation must be found, found: {occurrences:?}"
    );
}

/// Builds a document whose secret hides in the places that a list of known
/// places does not reach: in the popup of an annotation, in the structure
/// tree, in the name tree, in the optional content and in the private data
/// of a page.
fn document_with_hidden_places() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let secret_hex = "<FEFF00470045004800450049004D>";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 7 0 R /Names 10 0 R /OCProperties 12 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 6 0 R >> >> /Contents 5 0 R /Annots [8 0 R] /PieceInfo 13 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Annot /Subtype /Popup /Rect [20 20 40 40] /Contents (GEHEIM) >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        b"7 0 obj\n<< /Type /StructTreeRoot /K [9 0 R] >>\nendobj\n".to_vec(),
        b"8 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 60 40 80] /Contents (Notiz) /Popup 4 0 R >>\nendobj\n".to_vec(),
        format!("9 0 obj\n<< /Type /StructElem /S /Document /ActualText {secret_hex} >>\nendobj\n").into_bytes(),
        b"10 0 obj\n<< /Dests 11 0 R >>\nendobj\n".to_vec(),
        format!("11 0 obj\n<< /Names [{secret_hex}] >>\nendobj\n").into_bytes(),
        b"12 0 obj\n<< /OCGs [14 0 R] /D << /Order [14 0 R] /ON [14 0 R] >> >>\nendobj\n".to_vec(),
        b"13 0 obj\n<< /Private << /Data (GEHEIM) >> >>\nendobj\n".to_vec(),
        format!("14 0 obj\n<< /Type /OCG /Name {secret_hex} >>\nendobj\n").into_bytes(),
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

/// The places that a list of known places does not reach have to be found:
/// the popup of an annotation, the structure tree, the name tree, the
/// optional content and the private data of a page.
#[test]
fn the_places_that_a_list_of_known_places_does_not_reach_are_found() {
    let bytes = document_with_hidden_places();
    let occurrences =
        find_word_occurrences(&bytes, &[SECRET.to_owned()]).expect("the document must be readable");
    let places: Vec<&str> = occurrences
        .iter()
        .map(|entry| entry.location.as_str())
        .collect();

    for expected in ["/Popup/Contents", "/ActualText", "/Names", "/Name", "/Data"] {
        assert!(
            places.iter().any(|place| place.contains(expected)),
            "the place {expected} must be found, found: {places:?}"
        );
    }
}

/// The same places have to be cleaned: a secret that stays in the document
/// after the redaction is a secret that is not removed.
#[test]
fn the_places_that_a_list_of_known_places_does_not_reach_are_emptied() {
    let bytes = document_with_hidden_places();
    let (scrubbed, _report) =
        scrub_words(&bytes, &[SECRET.to_owned()]).expect("the document must be scrubable");

    let occurrences = find_word_occurrences(&scrubbed, &[SECRET.to_owned()])
        .expect("the scrubbed document must be readable");
    assert!(
        occurrences.is_empty(),
        "the secret must not stand in the document any more: {occurrences:?}"
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

/// Builds a document that carries an object which nothing refers to any
/// more: an older version of a note that the writer left in the file.
///
/// The object stands in the cross-reference table, but no page, no catalog
/// and no annotation refers to it. A secret that stands there is still in the
/// file — and the user who removed it believes that it is gone.
fn document_with_an_orphaned_object() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
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
        // Das verwaiste Objekt: es steht im Kreuzverzeichnis, aber kein
        // Objekt des Dokuments verweist auf es.
        b"6 0 obj\n<< /Type /Annot /Subtype /Text /Rect [20 20 40 40] /Contents (GEHEIM) >>\nendobj\n".to_vec(),
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

/// A secret that stands in an object which nothing refers to any more is
/// still in the file. The search has to find it.
#[test]
fn a_secret_in_an_object_that_nothing_refers_to_is_found() {
    let bytes = document_with_an_orphaned_object();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    assert!(
        !occurrences.is_empty(),
        "the secret in the orphaned object must be found"
    );
    assert!(
        occurrences
            .iter()
            .any(|entry| entry.location.contains("/Object[6]")),
        "the place of the secret must be named: {occurrences:?}"
    );
}

/// And it has to be cleaned: a secret that stays in the file after the
/// redaction is a secret that is not removed.
#[test]
fn a_secret_in_an_object_that_nothing_refers_to_is_emptied() {
    let bytes = document_with_an_orphaned_object();
    let (scrubbed, _report) =
        scrub_words(&bytes, &["GEHEIM".to_owned()]).expect("the document must be scrubable");

    let occurrences = find_word_occurrences(&scrubbed, &["GEHEIM".to_owned()])
        .expect("the scrubbed document must be readable");
    assert!(
        occurrences.is_empty(),
        "the secret must not stand in the file any more: {occurrences:?}"
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

/// Builds a document whose form XObject carries a second copy of the word.
///
/// The copy stands in the content stream of an object that only the
/// dictionary of the form XObject refers to. The fourth round of the red
/// team showed that the walk did not descend into the dictionary of a
/// stream — so the copy was never seen, and the redaction reported success
/// while the word was still in the file.
fn document_with_a_copy_in_a_form_xobject() -> Vec<u8> {
    let page_content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
    let form_content = "BT /F1 18 Tf 20 100 Td (GEHEIM) Tj ET\n";
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
        // The form XObject draws the inner form, which carries the copy. The
        // inner form stands in the **dictionary** of the outer one — the
        // place that the walk did not reach before.
        b"6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Resources << /XObject << /Fm1 7 0 R >> >> /Length 20 >>\nstream\nq /Fm1 Do Q\nendstream\nendobj\n".to_vec(),
        {
            let mut obj = format!("7 0 obj\n<< /Length {} >>\nstream\n", form_content.len()).into_bytes();
            obj.extend_from_slice(form_content.as_bytes());
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

/// A copy of the word in the content of a form XObject has to be found. It
/// is not drawn on any page, so neither the text of the page nor the search
/// over the pages sees it.
#[test]
fn a_copy_of_the_word_in_a_form_xobject_is_found() {
    let bytes = document_with_a_copy_in_a_form_xobject();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    assert!(
        !occurrences.is_empty(),
        "the copy in the form XObject must be found"
    );
}

/// The copy has to be **named**, even when it cannot be removed.
///
/// The content of a form XObject is a content stream, not a string: the
/// scrub cannot empty it, and MuPDF's redaction does not descend into a
/// form XObject. A copy that cannot be removed must therefore make the
/// redaction **refuse** — a redaction that reports a success while the word
/// is still in the file is the worst possible answer.
#[test]
fn a_copy_of_the_word_in_a_form_xobject_is_named_and_not_hidden() {
    let bytes = document_with_a_copy_in_a_form_xobject();
    let (_scrubbed, _report) =
        scrub_words(&bytes, &["GEHEIM".to_owned()]).expect("the document must be scrubable");

    // The copy is still in the document — and the search says so.
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    assert!(
        !occurrences.is_empty(),
        "the copy must still be found — the redaction must not claim that it is gone"
    );
    assert!(
        occurrences
            .iter()
            .any(|entry| entry.location.contains("Fm1")),
        "the place of the copy must be named: {occurrences:?}"
    );
}

/// Builds a document whose identifier of the file carries the word.
///
/// The identifier stands in the trailer. It is not an object of the
/// cross-reference table, so a walk that only looks at the objects of the
/// table never sees it — and the word stays in the file.
fn document_with_the_word_in_the_file_identifier() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 150 Td (VERTRAULICH) Tj ET\n";
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
    // The identifier of the file: the word stands in it as a hex string, so
    // that the search over the plain bytes of the file does not see it.
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /ID [(<FEFF00470045004800450049004D>)<00112233445566778899AABBCCDDEEFF>] >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

/// The identifier of the file is a place of the document, and it has to be
/// seen: the word that stands in it is still in the file.
#[test]
fn the_word_in_the_identifier_of_the_file_is_found() {
    let bytes = document_with_the_word_in_the_file_identifier();
    let occurrences = find_word_occurrences(&bytes, &["GEHEIM".to_owned()])
        .expect("the document must be readable");
    assert!(
        !occurrences.is_empty(),
        "the word in the identifier of the file must be found"
    );
    assert!(
        occurrences
            .iter()
            .any(|entry| entry.location.contains("/ID") || entry.location.contains("/Trailer")),
        "the place of the word must be named: {occurrences:?}"
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
    let occurrences =
        find_word_occurrences(&bytes, &["GEHEIM".to_owned()]).expect("the document must be readable");
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
    let occurrences =
        find_word_occurrences(&bytes, &["GEHEIM".to_owned()]).expect("the document must be readable");
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

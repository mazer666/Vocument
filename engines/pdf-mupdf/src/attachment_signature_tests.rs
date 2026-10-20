//! Tests of the inspection of the embedded files and the signatures of a
//! document (`ATTACH-001`, `SIGN-001`).
//!
//! The fixture is a real PDF with an embedded file and a signed form. The
//! signature covers the whole file in one case and only a part of it in the
//! other — that is the difference between a signature that is worth
//! something and one that is not.

use crate::MuPdfEngineAdapter;
use vocument_core::pdf_engine::PdfEngine;

/// The length of the placeholder of the signature contents.
const CONTENTS_LEN: usize = 40;

/// Builds a PDF with an embedded file and a signature field.
///
/// `changed_after_signing` decides whether the signature covers the whole
/// document or only the beginning of it. A signature that covers only a part
/// of the document says nothing about the rest — that is the case a reviewer
/// has to see.
fn pdf_with_attachment_and_signature(changed_after_signing: bool) -> Vec<u8> {
    let contents = "BT /F1 12 Tf 72 740 Td (Vocument Review) Tj ET\n";
    let attachment = "MZ this is not a real program";
    let placeholder = "[0000000000 0000000000 0000000000 0000000000]";

    let build = |byte_range: &str| -> Vec<u8> {
        let objects: Vec<String> = vec![
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [6 0 R] >> \
/Names << /EmbeddedFiles << /Names [(anhang.exe) 8 0 R] >> >> >>"
                .to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>"
                .to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
            format!(
                "<< /Length {} >>\nstream\n{contents}endstream",
                contents.len()
            ),
            // 6: the signature field (also the widget of the form)
            "<< /Type /Annot /Subtype /Widget /FT /Sig /T (Unterschrift) /V 7 0 R \
/Rect [72 100 320 140] /P 3 0 R >>"
                .to_owned(),
            // 7: the signature itself
            format!(
                "<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached \
/Name (Dr. Weber) /M (D:20260131120000Z) /Reason (Freigabe) /Location (Berlin) \
/ByteRange {byte_range} /Contents <{}> >>",
                "0".repeat(CONTENTS_LEN)
            ),
            // 8: the file specification of the attachment
            "<< /Type /Filespec /F (anhang.exe) /UF (anhang.exe) \
/EF << /F 9 0 R >> /Desc (Anhang des Schreibens) >>"
                .to_owned(),
            // 9: the content of the attachment
            format!(
                "<< /Type /EmbeddedFile /Subtype /application#2Foctet-stream /Length {} \
/Params << /Size {} >> >>\nstream\n{attachment}endstream",
                attachment.len(),
                attachment.len()
            ),
        ];

        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.7\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref_offset = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for offset in &offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    };

    // The first build with the placeholder determines the offsets; the second
    // build carries the real byte range. Both have the same length, so the
    // offsets stay valid.
    let probe = build(placeholder);
    let marker = b"/Contents <";
    let contents_start = probe
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("the fixture must carry the contents of the signature")
        + marker.len();
    let file_size = probe.len();

    let byte_range = if changed_after_signing {
        // The signature covers only the beginning of the document: the rest was
        // added afterwards.
        format!(
            "[{:010} {:010} {:010} {:010}]",
            0,
            contents_start,
            contents_start + CONTENTS_LEN,
            100
        )
    } else {
        // The signature covers the whole document, apart from the placeholder
        // of its own contents.
        format!(
            "[{:010} {:010} {:010} {:010}]",
            0,
            contents_start,
            contents_start + CONTENTS_LEN,
            file_size - (contents_start + CONTENTS_LEN)
        )
    };
    build(&byte_range)
}

#[test]
fn the_embedded_file_of_the_document_is_reported() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_attachment_and_signature(false))
        .expect("the fixture must be openable");
    let attachments = document
        .document_attachments()
        .expect("the inspection must succeed");

    assert_eq!(
        attachments.len(),
        1,
        "unexpected attachments: {attachments:?}"
    );
    let attachment = &attachments[0];
    assert_eq!(attachment.file_name, "anhang.exe");
    assert!(
        attachment.is_embedded,
        "the content of the file is in the document"
    );
    assert!(
        attachment.checksum_is_valid,
        "there is no checksum to contradict"
    );
    assert_eq!(
        attachment.size_bytes,
        "MZ this is not a real program".len() as i64,
        "the size of the file"
    );
}

#[test]
fn the_signature_of_the_document_is_reported_with_its_details() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_attachment_and_signature(false))
        .expect("the fixture must be openable");
    let signatures = document
        .document_signatures()
        .expect("the inspection must succeed");

    assert_eq!(signatures.len(), 1, "unexpected signatures: {signatures:?}");
    let signature = &signatures[0];
    assert_eq!(signature.field_name, "Unterschrift");
    assert!(signature.is_signed, "the field carries a signature");
    assert_eq!(signature.signer, "Dr. Weber");
    assert_eq!(signature.reason, "Freigabe");
    assert_eq!(signature.location, "Berlin");
    assert_eq!(signature.signed_at, "2026-01-31T12:00:00Z");
}

#[test]
fn a_document_that_was_changed_after_the_signature_is_reported() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_attachment_and_signature(true))
        .expect("the fixture must be openable");
    let signatures = document
        .document_signatures()
        .expect("the inspection must succeed");

    assert_eq!(signatures.len(), 1, "unexpected signatures: {signatures:?}");
    let signature = &signatures[0];
    assert!(signature.is_signed, "the field carries a signature");
    assert!(
        signature.changed_since_signing,
        "the signature covers only the beginning of the document"
    );
    assert!(
        signature.is_risky,
        "a signature that covers nothing is risky"
    );
}

#[test]
fn a_document_that_was_not_changed_after_the_signature_is_reported_as_unchanged() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_attachment_and_signature(false))
        .expect("the fixture must be openable");
    let signatures = document
        .document_signatures()
        .expect("the inspection must succeed");

    let signature = &signatures[0];
    assert!(
        !signature.changed_since_signing,
        "the signature covers the whole document"
    );
    assert!(!signature.is_risky, "an intact signature is not risky");
}

#[test]
fn a_document_without_attachments_and_signatures_is_reported_as_empty() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&crate::tests::sample_two_page_pdf_bytes())
        .expect("the sample must be openable");
    assert!(
        document
            .document_attachments()
            .expect("the inspection must succeed")
            .is_empty()
    );
    assert!(
        document
            .document_signatures()
            .expect("the inspection must succeed")
            .is_empty()
    );
}

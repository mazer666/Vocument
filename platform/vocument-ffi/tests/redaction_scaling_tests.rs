//! The sweep over the objects of a document has to stay **linear**.
//!
//! The fourth round of the red team found that the sweep scanned a linear
//! list for every entry of the cross-reference table, and therefore grew
//! quadratically with the number of objects of the document (R4-5). A
//! document with 32,000 objects needed 0.83 s for a single walk, and a
//! document with 32,000 reachable annotations needed 14.0 s for a single
//! redaction.
//!
//! The test measures the growth: a document that is **fifty times** larger
//! must not take **sixteen times** as long. A quadratic sweep would need
//! 2,500 times as long — the test fails long before that.
//!
//! The measurement runs against the real chain (core → MuPDF → UniFFI), and
//! the secret really stands on the page, so the sweep really is on the path
//! of the redaction.

use std::time::{Duration, Instant};

use vocument_ffi::redaction_dto::{FfiRedactionRegion, document_core_redact_page_regions};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::{document_core_open_brokered_file, document_core_page_words};

/// The word on the page.
const SECRET: &str = "VERTRAULICH";

/// Builds a document with `ballast` additional objects. The objects are
/// unreachable — nothing in the document refers to them — but they stand in
/// the cross-reference table, and the sweep has to look at every one of them.
fn document_with_ballast(ballast: usize) -> Vec<u8> {
    let content = format!("BT /F1 18 Tf 20 150 Td ({SECRET}) Tj ET\n");
    let mut objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>\nendobj\n".to_vec(),
        {
            let mut stream_object =
                format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream_object.extend_from_slice(content.as_bytes());
            stream_object.extend_from_slice(b"\nendstream\nendobj\n");
            stream_object
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ];
    for index in 0..ballast {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /Metadata /Subtype /XML /Ballast {index} >>\nendobj\n",
                6 + index
            )
            .into_bytes(),
        );
    }

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n");
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len());
    for object in &objects {
        offsets.push(out.len());
        // The object already carries its own head — writing a second one
        // would put the number of the object into its body.
        out.extend_from_slice(object);
        out.extend_from_slice(b"\n");
    }
    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
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
}

/// Redacts the secret and returns the duration of the redaction.
fn measure_redaction(ballast: usize) -> Duration {
    let document = document_with_ballast(ballast);
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("test:redaction-scaling-{ballast}"),
        display_name: format!("scaling-{ballast}.pdf"),
        access_origin: FfiFileAccessOrigin::InMemoryBuffer,
        os_file_id: None,
        modified_unix_ms: None,
        bytes: document,
    };
    let snapshot = document_core_open_brokered_file(descriptor).expect("the document must open");
    let session_id = snapshot.session_id.clone();

    let words = document_core_page_words(session_id.clone(), 0).expect("the page must be readable");
    let target = words
        .iter()
        .find(|word| word.text == SECRET)
        .expect("the secret must stand on the page");
    let region = FfiRedactionRegion {
        x0_milli_pt: target.rect.x0_milli_pt,
        y0_milli_pt: target.rect.y0_milli_pt,
        x1_milli_pt: target.rect.x1_milli_pt,
        y1_milli_pt: target.rect.y1_milli_pt,
    };

    let start = Instant::now();
    let report = document_core_redact_page_regions(session_id.clone(), 0, vec![region])
        .expect("the redaction must succeed");
    let duration = start.elapsed();
    assert!(report.verified, "the redaction must be verified");
    vocument_ffi::document_core_close_session(session_id).expect("the session must close");
    duration
}

/// The sweep grows with the number of objects of the document — but it grows
/// **linearly**, not quadratically.
#[test]
fn the_sweep_over_the_objects_grows_linearly_and_not_quadratically() {
    let small = measure_redaction(1_000);
    let large = measure_redaction(50_000);

    // Fifty times the objects may not cost sixteen times the time (that
    // would be the growth of a quadratic sweep) — with a second of slack
    // for the noise of a shared machine.
    let bound = small * 20 + Duration::from_secs(1);
    assert!(
        large < bound,
        "the sweep is not linear: {small:?} for 1,000 objects, {large:?} for 50,000 objects, the bound is {bound:?}"
    );
}

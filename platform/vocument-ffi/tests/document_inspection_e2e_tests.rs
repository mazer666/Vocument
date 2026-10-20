//! End-to-end test of the inspection of the links and actions of a document
//! (`INSPECT-001`).
//!
//! The document is a real PDF with the actions that a reviewer has to see
//! before a click: a link into the document, a link to an address, a link
//! that runs a script, a link that starts a file, a link to a local file and
//! an action that runs when the document is opened.

use std::path::PathBuf;
use vocument_ffi::{
    FfiPdfActionKind, document_core_close_session, document_core_inspect_document,
    document_core_open_macos_scoped_file,
};

fn temp_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("vocument-e2e-{name}"));
    std::fs::create_dir_all(&directory).expect("the temporary directory must be creatable");
    directory
}

/// Builds a PDF with five links and one document action.
fn pdf_with_actions() -> Vec<u8> {
    let content = "BT /F1 12 Tf 72 740 Td (Vocument Review) Tj ET\n";
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /OpenAction 6 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R \
/Annots [7 0 R 8 0 R 9 0 R 10 0 R 11 0 R] >>"
            .to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
        "<< /Type /Action /S /JavaScript /JS (app.alert('open')) >>".to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 700 220 720] /Border [0 0 0] \
/A << /S /GoTo /D (chapter1) >> >>"
            .to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 660 320 680] /Border [0 0 0] \
/A << /S /URI /URI (https://example.com/review) >> >>"
            .to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 620 320 640] /Border [0 0 0] \
/A << /S /JavaScript /JS (app.alert('click')) >> >>"
            .to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 580 320 600] /Border [0 0 0] \
/A << /S /Launch /F (calc.exe) >> >>"
            .to_owned(),
        "<< /Type /Annot /Subtype /Link /Rect [72 540 320 560] /Border [0 0 0] \
/A << /S /URI /URI (file:///etc/passwd) >> >>"
            .to_owned(),
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
}

#[test]
fn the_inspection_of_a_document_shows_where_the_links_lead_and_what_they_do() {
    let path = temp_dir("inspection").join("links.pdf");
    std::fs::write(&path, pdf_with_actions()).expect("the fixture must be writable");

    let snapshot = document_core_open_macos_scoped_file(path.to_string_lossy().into_owned())
        .expect("the document must be openable");
    let session_id = snapshot.session_id.clone();

    let inspection =
        document_core_inspect_document(session_id.clone()).expect("the inspection must succeed");

    assert_eq!(
        inspection.actions.len(),
        6,
        "unexpected report: {inspection:?}"
    );

    // The action that runs when the document is opened: no page, but a script.
    let open_action = &inspection.actions[0];
    assert_eq!(open_action.kind, FfiPdfActionKind::JavaScript);
    assert_eq!(open_action.page_index, None);
    assert!(open_action.is_risky, "a script that runs on open is risky");
    assert!(open_action.target.contains("app.alert"));

    // The link into the document: harmless.
    let internal = &inspection.actions[1];
    assert_eq!(internal.kind, FfiPdfActionKind::InternalDestination);
    assert_eq!(internal.page_index, Some(0));
    assert!(!internal.is_risky, "a link into the document is not risky");
    let rect = internal.rect.expect("a link of a page has a position");
    assert_eq!(rect.x0_milli_pt, 72_000);
    assert_eq!(
        rect.y0_milli_pt, 72_000,
        "the position is in the space of the page"
    );
    assert_eq!(rect.y1_milli_pt, 92_000);

    // The link to an address: it leaves the machine, so it is marked.
    let uri = &inspection.actions[2];
    assert_eq!(uri.kind, FfiPdfActionKind::Uri);
    assert!(uri.target.contains("https://example.com/review"));
    assert!(uri.is_risky, "a link that leaves the machine is marked");

    // The link that runs a script.
    let script = &inspection.actions[3];
    assert_eq!(script.kind, FfiPdfActionKind::JavaScript);
    assert!(script.is_risky, "a script is risky");

    // The link that starts a file.
    let launch = &inspection.actions[4];
    assert_eq!(launch.kind, FfiPdfActionKind::LaunchFile);
    assert!(launch.target.contains("calc.exe"));
    assert!(launch.is_risky, "starting a file is risky");

    // The link to a local file.
    let local_file = &inspection.actions[5];
    assert_eq!(local_file.kind, FfiPdfActionKind::Uri);
    assert!(local_file.target.contains("file:///etc/passwd"));
    assert!(local_file.is_risky, "a local file is marked");

    // The report counts what it found.
    // Risky: the script on open, the address, the script on click, the file
    // that is started and the link to the local file.
    assert_eq!(inspection.risky_count, 5, "the risky findings");
    // Outside the document: the address and the local file.
    assert_eq!(
        inspection.external_count, 2,
        "the findings that leave the document"
    );

    document_core_close_session(session_id).expect("the session must close");
}

//! Tests of the inspection of the links and actions of a document
//! (`INSPECT-001`).
//!
//! The fixture is a real PDF with the actions that a reviewer has to see
//! before a click: a link into the document, a link to an address, a link
//! that runs a script, a link that starts a file, a link to a local file and
//! an action that runs when the document is opened.

use crate::MuPdfEngineAdapter;
use vocument_core::document_inspection::PdfActionKind;
use vocument_core::pdf_engine::PdfEngine;

/// Builds a PDF with five links and one document action.
///
/// The rectangles of the annotations are in the user space of the PDF (origin
/// at the bottom left, y ascending); the page is 612 x 792 points.
fn pdf_with_actions() -> Vec<u8> {
    let content = "BT /F1 12 Tf 72 740 Td (Vocument Review) Tj ET\n";
    let objects: Vec<String> = vec![
        // 1: the catalog with the action that runs when the document is opened
        "<< /Type /Catalog /Pages 2 0 R /OpenAction 6 0 R >>".to_owned(),
        // 2: the page tree
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        // 3: the page with the annotations
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R \
/Annots [7 0 R 8 0 R 9 0 R 10 0 R 11 0 R] >>"
            .to_owned(),
        // 4: the font
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        // 5: the content of the page
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
        // 6: the action that runs when the document is opened
        "<< /Type /Action /S /JavaScript /JS (app.alert('open')) >>".to_owned(),
        // 7: a link into the document
        "<< /Type /Annot /Subtype /Link /Rect [72 700 220 720] /Border [0 0 0] \
/A << /S /GoTo /D (chapter1) >> >>"
            .to_owned(),
        // 8: a link to an address
        "<< /Type /Annot /Subtype /Link /Rect [72 660 320 680] /Border [0 0 0] \
/A << /S /URI /URI (https://example.com/review) >> >>"
            .to_owned(),
        // 9: a link that runs a script
        "<< /Type /Annot /Subtype /Link /Rect [72 620 320 640] /Border [0 0 0] \
/A << /S /JavaScript /JS (app.alert('click')) >> >>"
            .to_owned(),
        // 10: a link that starts a file
        "<< /Type /Annot /Subtype /Link /Rect [72 580 320 600] /Border [0 0 0] \
/A << /S /Launch /F (calc.exe) >> >>"
            .to_owned(),
        // 11: a link to a local file
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
fn the_inspection_reports_every_action_of_the_document() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_actions())
        .expect("the fixture must be openable");
    let actions = document
        .document_actions()
        .expect("the inspection must succeed");

    // The action of the document itself comes first, then the five links.
    assert_eq!(actions.len(), 6, "unexpected actions: {actions:?}");

    let open_action = &actions[0];
    assert_eq!(open_action.kind, PdfActionKind::JavaScript);
    assert_eq!(
        open_action.page_index, None,
        "the document action has no page"
    );
    assert!(
        open_action.target.contains("app.alert"),
        "the script must be reported, got: {}",
        open_action.target
    );

    let kinds: Vec<&PdfActionKind> = actions[1..].iter().map(|action| &action.kind).collect();
    assert_eq!(
        kinds,
        vec![
            &PdfActionKind::InternalDestination,
            &PdfActionKind::Uri,
            &PdfActionKind::JavaScript,
            &PdfActionKind::LaunchFile,
            &PdfActionKind::Uri,
        ]
    );

    assert!(actions[1].target.contains("chapter1"));
    assert!(actions[2].target.contains("https://example.com/review"));
    assert!(actions[3].target.contains("app.alert('click')"));
    assert!(actions[4].target.contains("calc.exe"));
    assert!(actions[5].target.contains("file:///etc/passwd"));
}

#[test]
fn the_position_of_a_link_is_reported_in_the_space_of_the_page() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&pdf_with_actions())
        .expect("the fixture must be openable");
    let actions = document
        .document_actions()
        .expect("the inspection must succeed");

    let link = actions
        .iter()
        .find(|action| action.kind == PdfActionKind::InternalDestination)
        .expect("the link into the document must be reported");
    assert_eq!(link.page_index, Some(0));
    let rect = link.rect.expect("a link of a page has a position");
    // The annotation lies at y 700..720 in the user space (origin bottom
    // left); the page space has its origin at the top left, so the position
    // is 792 - 720 = 72 .. 92.
    assert_eq!(rect.x0_milli_pt, 72_000, "x0 of the link");
    assert_eq!(rect.x1_milli_pt, 220_000, "x1 of the link");
    assert_eq!(rect.y0_milli_pt, 72_000, "y0 of the link");
    assert_eq!(rect.y1_milli_pt, 92_000, "y1 of the link");
}

#[test]
fn a_document_without_actions_is_reported_as_empty() {
    let engine = MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&crate::tests::sample_two_page_pdf_bytes())
        .expect("the sample must be openable");
    let actions = document
        .document_actions()
        .expect("the inspection must succeed");
    assert!(actions.is_empty(), "unexpected actions: {actions:?}");
}

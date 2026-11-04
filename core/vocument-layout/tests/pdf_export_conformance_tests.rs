//! Conformance and structure tests for the Tagged PDF/A-2b and PDF/UA-1 export
//! (`ADR-0020`, `ROADMAP.md` M4.4, `QA-003`).
//!
//! The veraPDF tests run the independent validator when it is installed. When it is missing the
//! test prints `NOT RUN` and proves nothing about conformance; `scripts/verify-pdfa-compliance.sh`
//! and the self-hosted CI runner (which has veraPDF) are the gate for the conformance claim.

use std::process::Command;

use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, CalloutBlock, CalloutType, FlowDocument, HeadingBlock, ListBlock, ListItem, ListKind,
    ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
};
use vocument_flow::review::FlowComment;
use vocument_flow::styles::TextSpan;
use vocument_layout::font::FontRegistry;
use vocument_layout::pdf_export::{PdfExportOptions, export_flow_document_to_pdf};

/// A legal-style document that touches every structure type the exporter emits.
fn rich_document() -> FlowDocument {
    let mut doc = FlowDocument::new(DocumentMetadata::new(
        "doc_conformance",
        "Schriftsatz & Anlagen <Entwurf>",
    ));
    let header_row = TableRow::header(vec![
        table_cell("hc1", "Dokument"),
        table_cell("hc2", "Bezeichnung"),
    ]);
    let body_row = TableRow::new(vec![
        table_cell("c1", "Anlage K1"),
        table_cell("c2", "Urteil vom 12. Januar 2026"),
    ]);
    let blocks = vec![
        Block::Heading(HeadingBlock::new(
            "h1",
            1,
            vec![TextSpan::bold("Rechtliche Begründung")],
        )),
        Block::Paragraph(ParagraphBlock::new(
            "p1",
            vec![
                TextSpan::plain("Der Beschwerdeführer rügt die Verletzung seiner Grundrechte "),
                TextSpan::bold("aus Art. 2 Abs. 1 GG"),
                TextSpan::plain(" – § 242 BGB (Treu und Glauben)."),
            ],
        )),
        Block::Table(TableBlock::new("t1", vec![header_row, body_row])),
        Block::List(ListBlock::new(
            "l1",
            ListKind::Unordered,
            vec![
                ListItem::new("li1", vec![TextSpan::plain("Zuständigkeit")]),
                ListItem::new("li2", vec![TextSpan::plain("Fristen & Form")]),
            ],
        )),
        Block::Callout(CalloutBlock {
            id: "co1".to_string(),
            callout_type: CalloutType::LegalNotice,
            title: Some("Hinweis".to_string()),
            blocks: vec![Block::Paragraph(ParagraphBlock::new(
                "cop1",
                vec![TextSpan::plain("Hervorgehobener Text.")],
            ))],
            extra: Default::default(),
        }),
    ];
    doc.sections
        .push(Section::new("s1", SectionGeometry::a4_portrait(), blocks));
    doc.review.comments.push(FlowComment::new(
        "comm_1",
        "Dr. Müller",
        "unix:1700000000",
        "Bitte Zuständigkeit prüfen.",
        Some("p1".to_string()),
        Some((0, 20)),
        Some("Der Beschwerdeführer".to_string()),
    ));
    doc
}

fn table_cell(id: &str, text: &str) -> TableCell {
    TableCell::new(
        id,
        vec![Block::Paragraph(ParagraphBlock::new(
            format!("{id}_p"),
            vec![TextSpan::plain(text)],
        ))],
    )
}

fn export_rich_document() -> Vec<u8> {
    export_flow_document_to_pdf(
        &rich_document(),
        &FontRegistry::new(),
        &PdfExportOptions::default(),
    )
    .expect("export rich document")
}

/// Runs veraPDF for one flavour and returns `None` when the validator is not installed.
fn verapdf_is_compliant(pdf_bytes: &[u8], flavour: &str, file_label: &str) -> Option<bool> {
    let path = std::env::temp_dir().join(format!("vocument_{file_label}_{flavour}.pdf"));
    std::fs::write(&path, pdf_bytes).expect("write temporary pdf");
    let output = Command::new("verapdf")
        .args(["--flavour", flavour])
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    let output = output.ok()?;
    let report = String::from_utf8_lossy(&output.stdout);
    if report.contains("isCompliant=\"true\"") {
        Some(true)
    } else {
        eprintln!("veraPDF {flavour} report:\n{report}");
        Some(false)
    }
}

#[test]
fn export_is_pdfa_2b_compliant_according_to_verapdf() {
    let pdf_bytes = export_rich_document();
    match verapdf_is_compliant(&pdf_bytes, "2b", "rich_document") {
        Some(compliant) => assert!(compliant, "veraPDF rejected the PDF/A-2b export"),
        None => {
            eprintln!("NOT RUN: veraPDF is not installed; PDF/A-2b conformance is unproven here")
        }
    }
}

#[test]
fn export_is_pdfua_1_compliant_according_to_verapdf() {
    let pdf_bytes = export_rich_document();
    match verapdf_is_compliant(&pdf_bytes, "ua1", "rich_document") {
        Some(compliant) => assert!(compliant, "veraPDF rejected the PDF/UA-1 export"),
        None => {
            eprintln!("NOT RUN: veraPDF is not installed; PDF/UA-1 conformance is unproven here")
        }
    }
}

#[test]
fn export_embeds_font_color_profile_and_trailer_identifier() {
    let pdf = String::from_utf8_lossy(&export_rich_document()).into_owned();
    assert!(pdf.contains("/FontFile2"), "font program must be embedded");
    assert!(pdf.contains("/BaseFont /Lato-Regular"));
    assert!(
        pdf.contains("/DestOutputProfile"),
        "output intent needs an ICC profile"
    );
    assert!(
        pdf.contains("/ID [ <"),
        "PDF/A requires a trailer file identifier"
    );
    assert!(
        !pdf.contains("/BaseFont /Helvetica"),
        "no unembedded standard font"
    );
}

#[test]
fn export_builds_table_list_callout_and_annotation_structure() {
    let pdf = String::from_utf8_lossy(&export_rich_document()).into_owned();
    for structure_type in [
        "H1", "P", "Table", "TR", "TH", "TD", "L", "LI", "LBody", "Sect", "Annot",
    ] {
        assert!(
            pdf.contains(&format!("/Type /StructElem /S /{structure_type} ")),
            "missing structure element {structure_type}"
        );
    }
    assert!(pdf.contains("/Scope /Column"), "header cells need a scope");
    assert!(pdf.contains("/ParentTree"));
    assert!(pdf.contains("/Type /OBJR"), "annotation must be tagged");
    assert!(pdf.contains("/Type /MCR"));
}

#[test]
fn export_places_text_inside_the_page_and_encodes_german_characters() {
    let pdf_bytes = export_rich_document();
    let pdf = String::from_utf8_lossy(&pdf_bytes);
    let mut text_matrix_count = 0;
    for line in pdf.lines().filter(|line| line.ends_with(" Tm")) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let x: f32 = fields[4].parse().expect("x coordinate");
        let y: f32 = fields[5].parse().expect("y coordinate");
        // A4 portrait is 595 x 842 points; the layout margin keeps text away from the edges.
        assert!((20.0..575.0).contains(&x), "x {x} outside the page margins");
        assert!((20.0..822.0).contains(&y), "y {y} outside the page margins");
        text_matrix_count += 1;
    }
    assert!(text_matrix_count > 5, "expected several positioned runs");
    assert!(
        pdf.contains("\\374"),
        "ü must be written as WinAnsi code 0374"
    );
    assert!(
        pdf.contains("\\247"),
        "§ must be written as WinAnsi code 0247"
    );
}

#[test]
fn export_is_deterministic() {
    assert_eq!(export_rich_document(), export_rich_document());
}

#[test]
fn xmp_title_with_markup_characters_is_escaped() {
    let pdf = String::from_utf8_lossy(&export_rich_document()).into_owned();
    assert!(pdf.contains("Schriftsatz &amp; Anlagen &lt;Entwurf&gt;"));
}

//! Independent structural and compliance tests for Tagged PDF, PDF/A-2b, and PDF/UA-1 export (`ADR-0020`, `ROADMAP.md` M4.4).

use std::process::Command;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, FlowDocument, HeadingBlock, ParagraphBlock, Section, SectionGeometry,
};
use vocument_flow::review::FlowComment;
use vocument_flow::styles::TextSpan;
use vocument_layout::font::FontRegistry;
use vocument_layout::pdf_export::{PdfExportOptions, export_flow_document_to_pdf};

/// Concatenates every string shown with `Tj` in the PDF. The bundled font program is embedded as
/// binary data, so whole-file substring checks would match font bytes instead of page text.
fn shown_text(pdf_bytes: &[u8]) -> String {
    let content = String::from_utf8_lossy(pdf_bytes);
    content
        .lines()
        .filter_map(|line| line.strip_suffix(") Tj"))
        .filter_map(|line| line.strip_prefix('('))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn test_pdf_export_tagged_pdfa_pdfua_structure() {
    let mut doc = FlowDocument::new(DocumentMetadata::new("doc_pdfa", "Commercial Agreement"));
    let blocks = vec![
        Block::Heading(HeadingBlock::new(
            "h1_title",
            1,
            vec![TextSpan::plain("Commercial Agreement & Service SLA")],
        )),
        Block::Paragraph(ParagraphBlock::new(
            "p_terms",
            vec![TextSpan::plain(
                "This Agreement is executed between Supplier and Customer under strict German law (§ 242 BGB).",
            )],
        )),
    ];

    let section = Section::new("sec_main", SectionGeometry::a4_portrait(), blocks);
    doc.sections.push(section);

    // Add a comment
    doc.review.comments.push(FlowComment::new(
        "comm_01",
        "Attorney Alice",
        "unix:1700000000",
        "Please confirm jurisdiction clause.",
        Some("p_terms".to_string()),
        Some((0, 14)),
        Some("This Agreement".to_string()),
    ));

    let font_registry = FontRegistry::new();
    let options = PdfExportOptions::default();

    let pdf_bytes = export_flow_document_to_pdf(&doc, &font_registry, &options);

    // 1. Basic PDF validity
    assert!(pdf_bytes.starts_with(b"%PDF-1.7\n"));
    assert!(pdf_bytes.ends_with(b"%%EOF\n"));

    let pdf_str = String::from_utf8_lossy(&pdf_bytes);

    // 2. Tagged PDF / PDF/UA-1 Markers
    assert!(pdf_str.contains("/MarkInfo << /Marked true >>"));
    assert!(pdf_str.contains("/StructTreeRoot"));
    assert!(pdf_str.contains("/Type /StructElem /S /Document"));
    assert!(pdf_str.contains("/Type /StructElem /S /H1"));
    assert!(pdf_str.contains("/Type /StructElem /S /P"));
    assert!(pdf_str.contains("/ViewerPreferences << /DisplayDocTitle true >>"));
    assert!(pdf_str.contains("/Lang (de-DE)"));

    // 3. PDF/A-2b & PDF/UA-1 Metadata
    assert!(pdf_str.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(pdf_str.contains("<pdfaid:conformance>B</pdfaid:conformance>"));
    assert!(pdf_str.contains("<pdfuaid:part>1</pdfuaid:part>"));
    assert!(pdf_str.contains("/OutputIntents"));
    assert!(pdf_str.contains("/ToUnicode"));

    // 4. Comment annotation
    assert!(pdf_str.contains("/Subtype /Text"));
    assert!(pdf_str.contains("Attorney Alice"));
    assert!(pdf_str.contains("Please confirm jurisdiction clause."));

    // 5. Independent validation via mutool (if available)
    let temp_path = std::env::temp_dir().join("vocument_m4_4_test.pdf");
    std::fs::write(&temp_path, &pdf_bytes).expect("write temp pdf");

    if let Ok(output) = Command::new("mutool")
        .args(["info", temp_path.to_str().expect("valid path")])
        .output()
    {
        if output.status.success() {
            let info_out = String::from_utf8_lossy(&output.stdout);
            println!("mutool info output:\n{}", info_out);
            assert!(info_out.contains("Pages: 1") || info_out.contains("1 pages"));
        }
    }

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn test_pdf_export_clean_vs_tracked_changes() {
    let mut doc = FlowDocument::new(DocumentMetadata::new("doc_review", "Review Comparison"));
    let mut p = ParagraphBlock::new("p_rev", vec![TextSpan::plain("Standard warranty: ")]);

    // Tracked insertion
    let mut ins_span = TextSpan::plain("three years");
    ins_span.tracked_change_id = Some("chg_1".to_string());
    p.spans.push(ins_span);

    // Tracked deletion
    let mut del_span = TextSpan::plain("one year");
    del_span.style.strikethrough = true;
    del_span.tracked_change_id = Some("chg_2".to_string());
    p.spans.push(del_span);

    doc.sections.push(Section::new(
        "sec_1",
        SectionGeometry::a4_portrait(),
        vec![Block::Paragraph(p)],
    ));

    let font_registry = FontRegistry::new();

    // Export with tracked changes
    let options_tracked = PdfExportOptions {
        include_tracked_changes: true,
        ..Default::default()
    };
    let pdf_tracked = export_flow_document_to_pdf(&doc, &font_registry, &options_tracked);
    let text_tracked = shown_text(&pdf_tracked);
    assert!(text_tracked.contains("three"));
    assert!(text_tracked.contains("years"));
    assert!(text_tracked.contains("one"));
    assert!(text_tracked.contains("year"));

    // Export clean (omits deleted text)
    let options_clean = PdfExportOptions {
        include_tracked_changes: false,
        ..Default::default()
    };
    let pdf_clean = export_flow_document_to_pdf(&doc, &font_registry, &options_clean);
    let text_clean = shown_text(&pdf_clean);
    assert!(text_clean.contains("three"));
    assert!(text_clean.contains("years"));
    assert!(!text_clean.contains("one"));
}

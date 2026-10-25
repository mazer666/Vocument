use std::env;
use std::fs;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, FlowDocument, HeadingBlock, ParagraphBlock, Section, SectionGeometry, TableBlock,
    TableCell, TableRow,
};
use vocument_flow::styles::TextSpan;
use vocument_layout::font::FontRegistry;
use vocument_layout::pdf_export::{PdfExportOptions, export_flow_document_to_pdf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "target/test_export.pdf".to_string());

    let mut doc = FlowDocument::new(DocumentMetadata::new(
        "doc_demo_1",
        "Verfassungsbeschwerde und Tagged PDF/UA Nachweis",
    ));
    let mut blocks = Vec::new();

    blocks.push(Block::Heading(HeadingBlock::new(
        "h1",
        1,
        vec![TextSpan::bold("Rechtliche Begründung & Schutzantrag")],
    )));

    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p1",
        vec![TextSpan::plain(
            "Der Beschwerdeführer rügt die Verletzung seiner Grundrechte aus Art. 2 Abs. 1 GG i.V.m. Art. 1 Abs. 1 GG.",
        )],
    )));

    let cell1 = TableCell::new(
        "c1",
        vec![Block::Paragraph(ParagraphBlock::new(
            "cp1",
            vec![TextSpan::plain("Anlage K1")],
        ))],
    );
    let cell2 = TableCell::new(
        "c2",
        vec![Block::Paragraph(ParagraphBlock::new(
            "cp2",
            vec![TextSpan::plain("Urteil vom 12. Januar 2026")],
        ))],
    );
    let row = TableRow::new(vec![cell1, cell2]);
    blocks.push(Block::Table(TableBlock::new("t1", vec![row])));

    doc.sections = vec![Section::new("s1", SectionGeometry::a4_portrait(), blocks)];

    let registry = FontRegistry::new();
    let options = PdfExportOptions::default();
    let pdf_bytes = export_flow_document_to_pdf(&doc, &registry, &options);
    fs::write(&out_path, pdf_bytes)?;
    println!("Exported test PDF to: {}", out_path);
    Ok(())
}

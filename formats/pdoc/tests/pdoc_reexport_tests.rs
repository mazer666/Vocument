//! Verification that `vocument-pdoc` seamlessly re-exports and integrates `FlowDocument` (`ADR-0020`, `SOURCE-001`).

use vocument_pdoc::{
    DocumentMetadata, FlowDocument, ParagraphBlock, Section, SectionGeometry, TextSpan,
    to_json_canonical, validate_document,
};

#[test]
fn test_pdoc_reexports_flow_document_and_validates() {
    let meta = DocumentMetadata::new("pdoc-001", "PDOC Authored Container");
    let mut doc = FlowDocument::new(meta);

    let p = ParagraphBlock::new("p1", vec![TextSpan::plain("Authored flow content.")]);
    doc.sections.push(Section::new(
        "sec-1",
        SectionGeometry::a4_portrait(),
        vec![p.into()],
    ));

    validate_document(&doc).expect("validation must succeed");
    let json = to_json_canonical(&doc).expect("canonical json serialization");
    assert!(json.contains("PDOC Authored Container"));
    assert!(json.contains("pdoc-001"));
}

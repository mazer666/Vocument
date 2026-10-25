//! Determinism and bit-identical layout geometry verification (`ADR-0020`, `AUTHOR-004`).

use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, CalloutBlock, CalloutType, FlowDocument, FootnoteDefinition, HeadingBlock,
    ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
};
use vocument_flow::styles::{FontStyle, Stylesheet, TextSpan};
use vocument_layout::font::FontRegistry;
use vocument_layout::layout_document;

fn create_deterministic_test_document() -> FlowDocument {
    let mut doc = FlowDocument::new(DocumentMetadata::new(
        "det_doc",
        "Bit-Identical Layout Test",
    ));
    doc.stylesheet = Stylesheet::standard_legal();

    let fn_block = Block::Paragraph(ParagraphBlock::new(
        "fn_p",
        vec![TextSpan {
            text: "Standard legal footnote reference text.".to_string(),
            style: FontStyle::PLAIN,
            font_size_pt: Some(9.0),
            font_family: Some("Inter".to_string()),
            font_weight: None,
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            tracked_change_id: None,
            extra: Default::default(),
        }],
    ));
    doc.footnotes.insert(
        "fn_det".to_string(),
        FootnoteDefinition::new("fn_det", 1, vec![fn_block]),
    );

    let mut blocks = Vec::new();

    // Heading
    blocks.push(Block::Heading(HeadingBlock::new(
        "h1",
        1,
        vec![TextSpan {
            text: "Deterministic Typesetting Proof".to_string(),
            style: FontStyle::bold(),
            font_size_pt: Some(18.0),
            font_family: Some("Inter".to_string()),
            font_weight: Some(700),
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            tracked_change_id: None,
            extra: Default::default(),
        }],
    )));

    // Paragraph with footnote
    blocks.push(Block::Paragraph(ParagraphBlock::new(
        "p1",
        vec![
            TextSpan {
                text: "This document establishes that identical semantic input and font metrics yield identical fixed-point geometry coordinates across runs.".to_string(),
                style: FontStyle::PLAIN,
                font_size_pt: Some(11.0),
                font_family: Some("Inter".to_string()),
                font_weight: None,
                color: None,
                background_color: None,
                link_url: None,
                footnote_ref: Some("fn_det".to_string()),
                tracked_change_id: None,
                extra: Default::default(),
            },
        ],
    )));

    // Table with 2x2 spans
    let r1 = TableRow::header(vec![
        TableCell::new(
            "th1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth1",
                vec![TextSpan {
                    text: "Metric".to_string(),
                    style: FontStyle::bold(),
                    font_size_pt: Some(10.0),
                    font_family: Some("Inter".to_string()),
                    font_weight: Some(700),
                    color: None,
                    background_color: None,
                    link_url: None,
                    footnote_ref: None,
                    tracked_change_id: None,
                    extra: Default::default(),
                }],
            ))],
        ),
        TableCell::new(
            "th2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pth2",
                vec![TextSpan {
                    text: "Fixed-Point Value".to_string(),
                    style: FontStyle::bold(),
                    font_size_pt: Some(10.0),
                    font_family: Some("Inter".to_string()),
                    font_weight: Some(700),
                    color: None,
                    background_color: None,
                    link_url: None,
                    footnote_ref: None,
                    tracked_change_id: None,
                    extra: Default::default(),
                }],
            ))],
        ),
    ]);
    let r2 = TableRow::new(vec![
        TableCell::new(
            "c1",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pc1",
                vec![TextSpan {
                    text: "Sub-point precision".to_string(),
                    style: FontStyle::PLAIN,
                    font_size_pt: Some(10.0),
                    font_family: Some("Inter".to_string()),
                    font_weight: None,
                    color: None,
                    background_color: None,
                    link_url: None,
                    footnote_ref: None,
                    tracked_change_id: None,
                    extra: Default::default(),
                }],
            ))],
        ),
        TableCell::new(
            "c2",
            vec![Block::Paragraph(ParagraphBlock::new(
                "pc2",
                vec![TextSpan {
                    text: "1/64th pt (26.6)".to_string(),
                    style: FontStyle::PLAIN,
                    font_size_pt: Some(10.0),
                    font_family: Some("Inter".to_string()),
                    font_weight: None,
                    color: None,
                    background_color: None,
                    link_url: None,
                    footnote_ref: None,
                    tracked_change_id: None,
                    extra: Default::default(),
                }],
            ))],
        ),
    ]);
    blocks.push(Block::Table(TableBlock::new("tbl_det", vec![r1, r2])));

    // Callout
    blocks.push(Block::Callout(CalloutBlock {
        id: "callout_det".to_string(),
        callout_type: CalloutType::Tip,
        title: Some("Verification Notice".to_string()),
        blocks: vec![Block::Paragraph(ParagraphBlock::new(
            "p_tip",
            vec![TextSpan {
                text: "All coordinates are deterministically computed using pure integer and 26.6 fixed-point arithmetic.".to_string(),
                style: FontStyle::PLAIN,
                font_size_pt: Some(10.0),
                font_family: Some("Inter".to_string()),
                font_weight: None,
                color: None,
                background_color: None,
                link_url: None,
                footnote_ref: None,
                tracked_change_id: None,
                extra: Default::default(),
            }],
        ))],
        extra: Default::default(),
    }));

    let section = Section::new("sec_det", SectionGeometry::a4_portrait(), blocks);
    doc.sections.push(section);
    doc
}

#[test]
fn test_bit_identical_repeated_layouts() {
    let registry = FontRegistry::new();
    let doc = create_deterministic_test_document();

    // Baseline layout run
    let baseline_layout = layout_document(&doc, &registry);
    let baseline_json = baseline_layout
        .to_json_canonical()
        .expect("canonical serialization");

    // Perform 10 consecutive layout runs
    for run in 1..=10 {
        let current_layout = layout_document(&doc, &registry);
        let current_json = current_layout
            .to_json_canonical()
            .expect("canonical serialization");

        assert_eq!(
            baseline_json, current_json,
            "Run #{} must produce byte-for-byte identical JSON serialization to baseline",
            run
        );
        assert_eq!(
            baseline_layout, current_layout,
            "Run #{} must have identical LayoutDocument structural and coordinate equality",
            run
        );
    }
}

#[test]
fn test_determinism_across_font_families() {
    let registry = FontRegistry::new();
    let families = ["Inter", "Helvetica", "Times", "Courier"];

    for family in &families {
        let mut doc = create_deterministic_test_document();
        for sec in &mut doc.sections {
            for b in &mut sec.blocks {
                if let Block::Paragraph(p) = b {
                    for span in &mut p.spans {
                        span.font_family = Some(family.to_string());
                    }
                }
            }
        }

        let l1 = layout_document(&doc, &registry);
        let l2 = layout_document(&doc, &registry);

        let json1 = l1.to_json_canonical().expect("json1");
        let json2 = l2.to_json_canonical().expect("json2");

        assert_eq!(
            json1, json2,
            "Family {} layout must be 100% deterministic",
            family
        );
    }
}

//! Comprehensive test suite for FlowDocument core model, schema validation, and serialization (`ADR-0020`, `AUTHOR-001`–`005`).

use std::collections::BTreeMap;
use std::time::Instant;

use vocument_flow::{
    Alignment, Author, Block, CalloutBlock, CalloutType, DocumentMetadata, FlowDocument, FlowError,
    FootnoteDefinition, HeadingBlock, ImageBlock, ListBlock, ListItem, ListKind, PageOrientation,
    ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow, TextSpan,
    ThematicBreakBlock, from_json_str, to_json_canonical, to_json_pretty, validate_document,
};

fn create_test_metadata() -> DocumentMetadata {
    let mut meta = DocumentMetadata::new("doc-uuid-12345", "Contractual Legal Brief");
    meta.subtitle = Some("Regarding Intellectual Property Assignment".to_string());
    meta.authors.push(
        Author::new("Dr. Jane Doe")
            .with_email("jane.doe@legal.org")
            .with_affiliation("Institute for Legal Standards"),
    );
    meta.description = Some("Executive legal summary of binding covenants.".to_string());
    meta.language = "de-DE".to_string();
    meta.keywords = vec!["law".to_string(), "covenant".to_string(), "ip".to_string()];
    meta.created_at_utc = "2026-10-25T10:00:00Z".to_string();
    meta.modified_at_utc = "2026-10-25T10:30:00Z".to_string();
    meta.revision = 3;
    meta.custom_attributes
        .insert("jurisdiction".to_string(), "DE".to_string());
    meta
}

fn create_valid_flow_document() -> FlowDocument {
    let metadata = create_test_metadata();
    let mut doc = FlowDocument::new(metadata);

    // Footnotes
    let fn1 = FootnoteDefinition::new(
        "fn-1",
        1,
        vec![Block::Paragraph(ParagraphBlock::new(
            "fn-1-p1",
            vec![TextSpan::plain("See statutory code § 312 BGB.")],
        ))],
    );
    doc.footnotes.insert("fn-1".to_string(), fn1);

    // Section 1
    let geom = SectionGeometry::a4_portrait();
    let mut s1_blocks = Vec::new();

    // 1. Heading 1
    s1_blocks.push(Block::Heading(HeadingBlock::new(
        "h1-intro",
        1,
        vec![TextSpan::plain("1. Introduction & Background")],
    )));

    // 2. Formatted Paragraph with inline styles, color, link, and footnote reference
    let p1_spans = vec![
        TextSpan::plain("This is a binding document governed by "),
        TextSpan::bold("statutory law"),
        TextSpan::plain(" with "),
        TextSpan::italic("special provisions"),
        TextSpan::plain(" regarding "),
        TextSpan::plain("Section 4").with_link("https://standards.org/sec4"),
        TextSpan::plain(" as documented"),
        TextSpan::plain("¹").with_footnote("fn-1"),
        TextSpan::plain("."),
    ];
    let mut p1 = ParagraphBlock::new("p1-content", p1_spans);
    p1.alignment = Some(Alignment::Justified);
    s1_blocks.push(Block::Paragraph(p1));

    // 3. Headings 2 through 6
    for level in 2..=6 {
        s1_blocks.push(Block::Heading(HeadingBlock::new(
            format!("h{level}-sub"),
            level,
            vec![TextSpan::plain(format!("Section Level {level} Title"))],
        )));
    }

    // 4. Nested List
    let nested_list = ListBlock::new(
        "list-nested",
        ListKind::Unordered,
        vec![
            ListItem::new("item-n1", vec![TextSpan::plain("Sub-clause alpha")]),
            ListItem::new("item-n2", vec![TextSpan::plain("Sub-clause beta")]),
        ],
    );
    let mut top_item1 = ListItem::new("item-1", vec![TextSpan::plain("Primary covenant")]);
    top_item1.nested_list = Some(Box::new(nested_list));

    let top_item2 = ListItem::new("item-2", vec![TextSpan::plain("Secondary obligation")]);
    s1_blocks.push(Block::List(ListBlock::new(
        "list-main",
        ListKind::Ordered,
        vec![top_item1, top_item2],
    )));

    // 5. Table with headers, colspans, and formatting
    let table_rows = vec![
        TableRow::header(vec![
            TableCell::new(
                "th-1",
                vec![Block::Paragraph(ParagraphBlock::new(
                    "th-1-p",
                    vec![TextSpan::bold("Obligation ID")],
                ))],
            ),
            TableCell::new(
                "th-2",
                vec![Block::Paragraph(ParagraphBlock::new(
                    "th-2-p",
                    vec![TextSpan::bold("Scope & Description")],
                ))],
            ),
        ]),
        TableRow::new(vec![
            TableCell::new(
                "td-1-1",
                vec![Block::Paragraph(ParagraphBlock::new(
                    "td-1-1-p",
                    vec![TextSpan::plain("COV-001")],
                ))],
            ),
            TableCell::new(
                "td-1-2",
                vec![Block::Paragraph(ParagraphBlock::new(
                    "td-1-2-p",
                    vec![TextSpan::plain("Complete IP assignment without retention.")],
                ))],
            ),
        ]),
    ];
    let mut table = TableBlock::new("table-summary", table_rows);
    table.caption = Some("Table 1: Summary of contractual obligations".to_string());
    table.column_widths_pt = vec![150.0, 300.0];
    s1_blocks.push(Block::Table(table));

    // 6. Image Block
    let img = ImageBlock {
        id: "img-flowchart".to_string(),
        asset_ref: "assets/architecture_diagram.png".to_string(),
        alt_text: "System architectural flow diagram".to_string(),
        caption: Some("Figure 1: High-level overview".to_string()),
        intrinsic_width_pt: 480.0,
        intrinsic_height_pt: 320.0,
        display_width_pt: Some(400.0),
        display_height_pt: Some(266.6),
        extra: BTreeMap::new(),
    };
    s1_blocks.push(Block::Image(img));

    // 7. Callout Block
    let callout = CalloutBlock {
        id: "callout-warning".to_string(),
        callout_type: CalloutType::Warning,
        title: Some("Important Legal Deadline".to_string()),
        blocks: vec![Block::Paragraph(ParagraphBlock::new(
            "callout-p",
            vec![TextSpan::plain(
                "Filing must occur within 14 calendar days.",
            )],
        ))],
        extra: BTreeMap::new(),
    };
    s1_blocks.push(Block::Callout(callout));

    // 8. Thematic Break
    s1_blocks.push(Block::ThematicBreak(ThematicBreakBlock {
        id: "hr-divider".to_string(),
        extra: BTreeMap::new(),
    }));

    let section1 = Section::new("sec-1", geom, s1_blocks);
    doc.sections.push(section1);

    doc
}

#[test]
fn test_construct_comprehensive_flow_document_and_validate() {
    let doc = create_valid_flow_document();
    let validation = validate_document(&doc);
    assert!(
        validation.is_ok(),
        "Document must pass validation: {validation:?}"
    );

    assert_eq!(doc.sections.len(), 1);
    assert_eq!(
        doc.sections[0].geometry.orientation,
        PageOrientation::Portrait
    );
    assert_eq!(doc.sections[0].blocks.len(), 12);
    assert_eq!(doc.footnotes.len(), 1);
}

#[test]
fn test_canonical_json_roundtrip_equality() {
    let doc = create_valid_flow_document();

    let json_canonical = to_json_canonical(&doc).expect("canonical json serialization");
    assert!(!json_canonical.is_empty());

    let json_pretty = to_json_pretty(&doc).expect("pretty json serialization");
    assert!(!json_pretty.is_empty());

    // Deserialize from canonical string
    let parsed_doc = from_json_str(&json_canonical).expect("deserialization from canonical json");
    assert_eq!(doc, parsed_doc);

    // Deserialize from pretty string
    let parsed_pretty = from_json_str(&json_pretty).expect("deserialization from pretty json");
    assert_eq!(doc, parsed_pretty);

    // Second roundtrip: parsed -> json2
    let json_canonical_2 = to_json_canonical(&parsed_doc).expect("second serialization");
    assert_eq!(json_canonical, json_canonical_2);
}

#[test]
fn test_forward_compatibility_and_unknown_fields_preservation() {
    let doc = create_valid_flow_document();
    let canonical_str = to_json_canonical(&doc).expect("canonical json string");
    let mut json_val: serde_json::Value =
        serde_json::from_str(&canonical_str).expect("parse to serde Value");

    // Inject unrecognized unknown fields into document root and metadata
    json_val["future_ai_provenance_signature"] = serde_json::json!({
        "algorithm": "Dilithium5",
        "verified": true
    });
    json_val["metadata"]["custom_tax_id"] = serde_json::json!("DE-998877");

    let modified_json = serde_json::to_string(&json_val).expect("serialize modified json");

    // Deserialize into FlowDocument
    let doc_with_extra: FlowDocument =
        from_json_str(&modified_json).expect("deserialization with unknown fields must succeed");

    // Verify unknown root field is preserved in extra
    assert!(
        doc_with_extra
            .extra
            .contains_key("future_ai_provenance_signature")
    );
    assert_eq!(
        doc_with_extra.extra["future_ai_provenance_signature"]["algorithm"],
        "Dilithium5"
    );

    // Verify unknown metadata field is preserved in metadata.extra
    assert!(doc_with_extra.metadata.extra.contains_key("custom_tax_id"));
    assert_eq!(doc_with_extra.metadata.extra["custom_tax_id"], "DE-998877");

    // Reserialize and verify unknown fields survive intact
    let reserialized_json = to_json_canonical(&doc_with_extra).expect("reserialize extra fields");
    assert!(reserialized_json.contains("future_ai_provenance_signature"));
    assert!(reserialized_json.contains("custom_tax_id"));
}

#[test]
fn test_schema_validation_rejects_empty_document() {
    let mut doc = FlowDocument::new(create_test_metadata());
    doc.sections = Vec::new();

    let err = validate_document(&doc).expect_err("Must reject document with no sections");
    assert_eq!(err, FlowError::EmptyDocument);
}

#[test]
fn test_schema_validation_rejects_empty_section() {
    let mut doc = FlowDocument::new(create_test_metadata());
    doc.sections.push(Section::new(
        "sec-empty",
        SectionGeometry::a4_portrait(),
        Vec::new(),
    ));

    let err = validate_document(&doc).expect_err("Must reject section with no blocks");
    match err {
        FlowError::EmptySection { section_id } => assert_eq!(section_id, "sec-empty"),
        other => panic!("Expected EmptySection, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_invalid_heading_levels() {
    // Level 0
    let mut doc = create_valid_flow_document();
    doc.sections[0].blocks[0] = Block::Heading(HeadingBlock::new(
        "h-invalid-0",
        0,
        vec![TextSpan::plain("Invalid Level 0")],
    ));
    let err0 = validate_document(&doc).expect_err("Must reject level 0");
    match err0 {
        FlowError::InvalidHeadingLevel { block_id, level } => {
            assert_eq!(block_id, "h-invalid-0");
            assert_eq!(level, 0);
        }
        other => panic!("Expected InvalidHeadingLevel, got: {other:?}"),
    }

    // Level 7
    let mut doc2 = create_valid_flow_document();
    doc2.sections[0].blocks[0] = Block::Heading(HeadingBlock::new(
        "h-invalid-7",
        7,
        vec![TextSpan::plain("Invalid Level 7")],
    ));
    let err7 = validate_document(&doc2).expect_err("Must reject level 7");
    match err7 {
        FlowError::InvalidHeadingLevel { block_id, level } => {
            assert_eq!(block_id, "h-invalid-7");
            assert_eq!(level, 7);
        }
        other => panic!("Expected InvalidHeadingLevel, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_empty_paragraph_and_heading() {
    let mut doc = create_valid_flow_document();
    doc.sections[0].blocks[0] = Block::Heading(HeadingBlock::new("h-empty", 1, Vec::new()));
    let err = validate_document(&doc).expect_err("Must reject empty heading");
    match err {
        FlowError::EmptyHeading { block_id } => assert_eq!(block_id, "h-empty"),
        other => panic!("Expected EmptyHeading, got: {other:?}"),
    }

    let mut doc2 = create_valid_flow_document();
    doc2.sections[0].blocks[1] = Block::Paragraph(ParagraphBlock::new("p-empty", Vec::new()));
    let err2 = validate_document(&doc2).expect_err("Must reject empty paragraph");
    match err2 {
        FlowError::EmptyParagraph { block_id } => assert_eq!(block_id, "p-empty"),
        other => panic!("Expected EmptyParagraph, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_mismatched_table_columns() {
    let mut doc = create_valid_flow_document();
    let mismatched_table = TableBlock::new(
        "table-mismatch",
        vec![
            TableRow::header(vec![
                TableCell::new(
                    "th-m1",
                    vec![Block::Paragraph(ParagraphBlock::new(
                        "p-thm1",
                        vec![TextSpan::plain("Col1")],
                    ))],
                ),
                TableCell::new(
                    "th-m2",
                    vec![Block::Paragraph(ParagraphBlock::new(
                        "p-thm2",
                        vec![TextSpan::plain("Col2")],
                    ))],
                ),
            ]),
            TableRow::new(vec![
                // Only 1 column instead of 2
                TableCell::new(
                    "td-m1",
                    vec![Block::Paragraph(ParagraphBlock::new(
                        "p-tdm1",
                        vec![TextSpan::plain("Val1")],
                    ))],
                ),
            ]),
        ],
    );
    doc.sections[0].blocks.push(Block::Table(mismatched_table));

    let err = validate_document(&doc).expect_err("Must reject table column dimension mismatch");
    match err {
        FlowError::InvalidTableDimensions {
            table_id,
            row_index,
            expected_cols,
            actual_cols,
        } => {
            assert_eq!(table_id, "table-mismatch");
            assert_eq!(row_index, 1);
            assert_eq!(expected_cols, 2);
            assert_eq!(actual_cols, 1);
        }
        other => panic!("Expected InvalidTableDimensions, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_duplicate_ids() {
    let mut doc = create_valid_flow_document();
    // Add another block with duplicate id "p1-content"
    doc.sections[0]
        .blocks
        .push(Block::Paragraph(ParagraphBlock::new(
            "p1-content",
            vec![TextSpan::plain("Duplicate ID paragraph")],
        )));

    let err = validate_document(&doc).expect_err("Must reject duplicate ID");
    match err {
        FlowError::DuplicateId { id } => assert_eq!(id, "p1-content"),
        other => panic!("Expected DuplicateId, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_missing_footnote_reference() {
    let mut doc = create_valid_flow_document();
    let dangling_span =
        TextSpan::plain("Text referencing nonexistent footnote").with_footnote("fn-does-not-exist");

    doc.sections[0]
        .blocks
        .push(Block::Paragraph(ParagraphBlock::new(
            "p-dangling",
            vec![dangling_span],
        )));

    let err = validate_document(&doc).expect_err("Must reject dangling footnote reference");
    match err {
        FlowError::InvalidFootnoteReference { block_id, ref_id } => {
            assert_eq!(block_id, "p-dangling");
            assert_eq!(ref_id, "fn-does-not-exist");
        }
        other => panic!("Expected InvalidFootnoteReference, got: {other:?}"),
    }
}

#[test]
fn test_schema_validation_rejects_unsupported_version() {
    let mut doc = create_valid_flow_document();
    doc.schema_version = "99.0".to_string();

    let err = validate_document(&doc).expect_err("Must reject schema version 99.0");
    match err {
        FlowError::UnsupportedSchemaVersion { version, expected } => {
            assert_eq!(version, "99.0");
            assert_eq!(expected, "1.0");
        }
        other => panic!("Expected UnsupportedSchemaVersion, got: {other:?}"),
    }
}

#[test]
fn test_large_document_construction_and_performance_1000_blocks() {
    let t_start = Instant::now();
    let metadata = create_test_metadata();
    let mut doc = FlowDocument::new(metadata);

    let mut blocks = Vec::with_capacity(1000);
    for i in 0..1000 {
        blocks.push(Block::Paragraph(ParagraphBlock::new(
            format!("p-stress-{i}"),
            vec![
                TextSpan::plain(format!("Paragraph #{i}: ")),
                TextSpan::bold("Binding contractual agreement covenant text. "),
                TextSpan::italic("Exercised in full compliance with ISO and legal mandates."),
            ],
        )));
    }

    doc.sections.push(Section::new(
        "sec-stress",
        SectionGeometry::a4_portrait(),
        blocks,
    ));
    let construction_duration = t_start.elapsed().as_secs_f64();
    println!("Constructed 1,000-block FlowDocument in {construction_duration:.4}s");
    assert!(
        construction_duration < 0.5,
        "Construction must be sub-second"
    );

    let t_val = Instant::now();
    validate_document(&doc).expect("stress document must validate cleanly");
    let val_duration = t_val.elapsed().as_secs_f64();
    println!("Validated 1,000-block FlowDocument in {val_duration:.4}s");
    assert!(val_duration < 0.2, "Validation must be sub-second");

    let t_ser = Instant::now();
    let json = to_json_canonical(&doc).expect("serialize 1,000-block document");
    let ser_duration = t_ser.elapsed().as_secs_f64();
    println!(
        "Serialized 1,000-block FlowDocument ({} KB) in {ser_duration:.4}s",
        json.len() / 1024
    );
    assert!(ser_duration < 0.5, "Serialization must be sub-second");
    assert!(json.len() > 100_000);

    let t_deser = Instant::now();
    let parsed = from_json_str(&json).expect("deserialization of 1,000-block document");
    let deser_duration = t_deser.elapsed().as_secs_f64();
    println!("Deserialized 1,000-block FlowDocument in {deser_duration:.4}s");
    assert!(deser_duration < 0.5, "Deserialization must be sub-second");
    assert_eq!(parsed.sections[0].blocks.len(), 1000);
}

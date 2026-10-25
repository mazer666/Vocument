//! Target negative, boundary, and complex structural tests for FlowDocument hardening (`ADR-0020`, `AUTHOR-001`–`005`).

use std::collections::BTreeMap;
use vocument_flow::error::FlowError;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::*;
use vocument_flow::schema::validate_document;
use vocument_flow::styles::*;

fn make_valid_base_doc() -> FlowDocument {
    let mut doc = FlowDocument::new(DocumentMetadata::new(
        "doc-valid-base",
        "Valid Base Document",
    ));
    doc.stylesheet = Stylesheet::standard_legal();
    let sec = Section::new(
        "sec-1",
        SectionGeometry::a4_portrait(),
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-1",
            vec![TextSpan::plain("Hello world")],
        ))],
    );
    doc.sections.push(sec);
    doc
}

#[test]
fn test_complex_valid_table_with_rowspan_and_colspan() {
    let mut doc = make_valid_base_doc();

    // Construct a valid 3x3 table:
    // Row 0: Cell 0 (rowspan=2, colspan=2) -> covers (0,0), (0,1), (1,0), (1,1)
    //        Cell 1 (rowspan=1, colspan=1) -> covers (0,2)
    // Row 1: Cell 2 (rowspan=1, colspan=1) -> covers (1,2)
    // Row 2: Cell 3 (1x1), Cell 4 (1x1), Cell 5 (1x1) -> covers (2,0), (2,1), (2,2)
    let c0 = TableCell::new(
        "c-0",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c0",
            vec![TextSpan::plain("Large 2x2")],
        ))],
    )
    .with_row_span(2)
    .with_col_span(2);

    let c1 = TableCell::new(
        "c-1",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c1",
            vec![TextSpan::plain("Top right")],
        ))],
    );

    let c2 = TableCell::new(
        "c-2",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c2",
            vec![TextSpan::plain("Mid right")],
        ))],
    );

    let c3 = TableCell::new(
        "c-3",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c3",
            vec![TextSpan::plain("Bottom 0")],
        ))],
    );
    let c4 = TableCell::new(
        "c-4",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c4",
            vec![TextSpan::plain("Bottom 1")],
        ))],
    );
    let c5 = TableCell::new(
        "c-5",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-c5",
            vec![TextSpan::plain("Bottom 2")],
        ))],
    );

    let row0 = TableRow::header(vec![c0, c1]);
    let row1 = TableRow::new(vec![c2]);
    let row2 = TableRow::new(vec![c3, c4, c5]);

    let table = TableBlock::new("tbl-valid-span", vec![row0, row1, row2])
        .with_column_widths(vec![100.0, 100.0, 100.0])
        .with_repeat_header_rows(1);

    doc.sections[0].blocks.push(Block::Table(table));
    assert!(
        validate_document(&doc).is_ok(),
        "3x3 table with 2x2 rowspan/colspan must pass validation"
    );
}

#[test]
fn test_nested_table_in_table_cell() {
    let mut doc = make_valid_base_doc();

    let inner_cell_a = TableCell::new(
        "inner-cell-a",
        vec![Block::Paragraph(ParagraphBlock::new(
            "inner-p-a",
            vec![TextSpan::plain("Inner A")],
        ))],
    );
    let inner_cell_b = TableCell::new(
        "inner-cell-b",
        vec![Block::Paragraph(ParagraphBlock::new(
            "inner-p-b",
            vec![TextSpan::plain("Inner B")],
        ))],
    );
    let inner_table = TableBlock::new(
        "inner-table-1",
        vec![TableRow::new(vec![inner_cell_a, inner_cell_b])],
    );

    let outer_cell_1 = TableCell::new(
        "outer-cell-1",
        vec![
            Block::Paragraph(ParagraphBlock::new(
                "p-outer-1",
                vec![TextSpan::plain("Outer cell heading")],
            )),
            Block::Table(inner_table),
        ],
    );
    let outer_cell_2 = TableCell::new(
        "outer-cell-2",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-outer-2",
            vec![TextSpan::plain("Outer cell right")],
        ))],
    );
    let outer_table = TableBlock::new(
        "outer-table-1",
        vec![TableRow::new(vec![outer_cell_1, outer_cell_2])],
    );

    doc.sections[0].blocks.push(Block::Table(outer_table));
    assert!(
        validate_document(&doc).is_ok(),
        "Nested table within table cell must validate cleanly"
    );
}

#[test]
fn test_table_validation_rejects_zero_span() {
    let mut doc = make_valid_base_doc();
    let cell_zero_col = TableCell::new(
        "c-zero-col",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-z1",
            vec![TextSpan::plain("Zero col")],
        ))],
    )
    .with_col_span(0);

    let table = TableBlock::new("tbl-zero-col", vec![TableRow::new(vec![cell_zero_col])]);
    doc.sections[0].blocks.push(Block::Table(table));

    match validate_document(&doc) {
        Err(FlowError::InvalidCellSpan { col_span, .. }) => {
            assert_eq!(col_span, 0);
        }
        other => panic!("Expected InvalidCellSpan, got {:?}", other),
    }
}

#[test]
fn test_table_validation_rejects_row_span_out_of_bounds() {
    let mut doc = make_valid_base_doc();
    let cell_oob = TableCell::new(
        "c-oob",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-oob",
            vec![TextSpan::plain("OOB")],
        ))],
    )
    .with_row_span(3); // Table only has 2 rows!

    let cell_right = TableCell::new(
        "c-right",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-r",
            vec![TextSpan::plain("Right")],
        ))],
    );

    let row0 = TableRow::new(vec![cell_oob, cell_right]);
    let row1 = TableRow::new(vec![TableCell::new(
        "c-r1",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-r1",
            vec![TextSpan::plain("Row 1")],
        ))],
    )]);

    let table = TableBlock::new("tbl-oob", vec![row0, row1]);
    doc.sections[0].blocks.push(Block::Table(table));

    match validate_document(&doc) {
        Err(FlowError::RowSpanOutOfBounds {
            row_span,
            total_rows,
            ..
        }) => {
            assert_eq!(row_span, 3);
            assert_eq!(total_rows, 2);
        }
        other => panic!("Expected RowSpanOutOfBounds, got {:?}", other),
    }
}

#[test]
fn test_table_validation_rejects_overlapping_cells() {
    let mut doc = make_valid_base_doc();

    // Row 0 has Cell 0 with rowspan=2, colspan=1 covering (0,0) and (1,0).
    // Row 1 also defines a Cell 1 that attempts to occupy column 0!
    let c0 = TableCell::new(
        "c-overlap-0",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-ov-0",
            vec![TextSpan::plain("Spans down")],
        ))],
    )
    .with_row_span(2);

    let c_top_right = TableCell::new(
        "c-top-r",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-tr",
            vec![TextSpan::plain("Top right")],
        ))],
    );

    let row0 = TableRow::new(vec![c0, c_top_right]);

    // Row 1 attempts to specify 2 cells, which would cause cell 1 to collide or exceed
    let c_colliding = TableCell::new(
        "c-colliding-1",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-col-1",
            vec![TextSpan::plain("Collides")],
        ))],
    );
    let c_colliding_2 = TableCell::new(
        "c-colliding-2",
        vec![Block::Paragraph(ParagraphBlock::new(
            "p-col-2",
            vec![TextSpan::plain("Excess")],
        ))],
    );
    let row1 = TableRow::new(vec![c_colliding, c_colliding_2]);

    let table = TableBlock::new("tbl-overlap", vec![row0, row1]);
    doc.sections[0].blocks.push(Block::Table(table));

    // Either OverlappingTableCells or InvalidTableDimensions must be returned
    assert!(
        validate_document(&doc).is_err(),
        "Table with conflicting row spans must fail validation"
    );
}

#[test]
fn test_geometric_validation_rejects_invalid_margins() {
    let mut doc = make_valid_base_doc();

    // Set horizontal margins equal to width (595.276pt)
    doc.sections[0].geometry.margin_left_pt = 300.0;
    doc.sections[0].geometry.margin_right_pt = 300.0; // 600pt > 595.276pt

    match validate_document(&doc) {
        Err(FlowError::InvalidGeometry { details, .. }) => {
            assert!(
                details.contains("equal or exceed page width"),
                "Expected margin width error, got: {details}"
            );
        }
        other => panic!("Expected InvalidGeometry, got {:?}", other),
    }
}

#[test]
fn test_geometric_validation_rejects_zero_column_count_or_excessive_gap() {
    let mut doc = make_valid_base_doc();
    doc.sections[0].geometry.column_count = 0;

    match validate_document(&doc) {
        Err(FlowError::InvalidGeometry { details, .. }) => {
            assert!(details.contains("column count must be at least 1"));
        }
        other => panic!(
            "Expected InvalidGeometry for column_count 0, got {:?}",
            other
        ),
    }

    doc.sections[0].geometry.column_count = 2;
    doc.sections[0].geometry.column_gap_pt = 500.0; // Printable width is ~451pt, gap 500 exceeds it!

    match validate_document(&doc) {
        Err(FlowError::InvalidGeometry { details, .. }) => {
            assert!(details.contains("total column gaps"));
        }
        other => panic!(
            "Expected InvalidGeometry for excessive gap, got {:?}",
            other
        ),
    }
}

#[test]
fn test_image_validation_rejects_negative_or_zero_dimensions() {
    let mut doc = make_valid_base_doc();
    let bad_img = ImageBlock {
        id: "img-bad".to_string(),
        asset_ref: "assets/photo.jpg".to_string(),
        alt_text: "A photo".to_string(),
        caption: None,
        intrinsic_width_pt: -100.0,
        intrinsic_height_pt: 200.0,
        display_width_pt: None,
        display_height_pt: None,
        extra: BTreeMap::new(),
    };
    doc.sections[0].blocks.push(Block::Image(bad_img));

    match validate_document(&doc) {
        Err(FlowError::InvalidImageDimensions { details, .. }) => {
            assert!(details.contains("must be finite and > 0"));
        }
        other => panic!("Expected InvalidImageDimensions, got {:?}", other),
    }
}

#[test]
fn test_stylesheet_validation_rejects_unknown_style_references() {
    let mut doc = make_valid_base_doc();
    let p_unknown = ParagraphBlock::new(
        "p-unknown-style",
        vec![TextSpan::plain("Has non-existent style")],
    )
    .with_style("NonExistentStyleId");

    doc.sections[0].blocks.push(Block::Paragraph(p_unknown));

    match validate_document(&doc) {
        Err(FlowError::UnknownStyle {
            style_type,
            style_id,
            ..
        }) => {
            assert_eq!(style_type, "paragraph");
            assert_eq!(style_id, "NonExistentStyleId");
        }
        other => panic!("Expected UnknownStyle, got {:?}", other),
    }
}

#[test]
fn test_stylesheet_validation_rejects_invalid_typography_values() {
    let mut doc = make_valid_base_doc();
    // Corrupt standard style font size to -5.0pt
    if let Some(style) = doc.stylesheet.paragraph_styles.get_mut("Normal") {
        style.font_size_pt = -5.0;
    }

    match validate_document(&doc) {
        Err(FlowError::InvalidTypography { details, .. }) => {
            assert!(details.contains("font_size_pt must be finite and > 0"));
        }
        other => panic!("Expected InvalidTypography, got {:?}", other),
    }
}

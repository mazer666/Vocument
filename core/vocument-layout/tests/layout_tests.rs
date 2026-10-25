//! Unit and component test suites for deterministic layout engine (`ADR-0020`, `AUTHOR-004`).

use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{
    Block, FlowDocument, ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
};
use vocument_flow::styles::{Alignment, FontStyle, TextSpan};
use vocument_layout::font::FontRegistry;
use vocument_layout::incremental::IncrementalLayoutEngine;
use vocument_layout::line_breaker::{LineBreakerConfig, break_paragraph_runs};
use vocument_layout::shaper::shape_span;
use vocument_layout::{PaginatorOptions, layout_document};

fn sample_text_spans(text: &str) -> Vec<TextSpan> {
    vec![TextSpan {
        text: text.to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(11.0),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        tracked_change_id: None,
        original_style: None,
        extra: Default::default(),
    }]
}

#[test]
fn test_font_registry_and_fixed_point_metrics() {
    let registry = FontRegistry::new();
    let regular = registry.resolve("Inter", false, false);
    let bold = registry.resolve("Inter", true, false);

    assert_eq!(regular.family, "Inter");
    assert!(!regular.is_bold);
    assert!(bold.is_bold);

    // Check fixed point width calculation
    let w_m = regular.advance_width_fixed('M', 12.0);
    let w_i = regular.advance_width_fixed('i', 12.0);
    assert!(w_m > w_i, "M must be wider than i");
    assert!(w_m.0 > 0);
    assert!(w_i.0 > 0);
}

#[test]
fn test_greedy_vs_knuth_plass_line_breaking() {
    let registry = FontRegistry::new();
    let font = registry.resolve("Inter", false, false);
    let long_sentence = "The quick brown fox jumps over the lazy dog in the courtroom during the trial proceedings while the judge presides with great dignity.";
    let span = TextSpan {
        text: long_sentence.to_string(),
        style: FontStyle::PLAIN,
        font_size_pt: Some(11.0),
        font_family: Some("Inter".to_string()),
        font_weight: None,
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        tracked_change_id: None,
        original_style: None,
        extra: Default::default(),
    };

    let runs = shape_span(&span, font, 11.0, 1.15);
    assert!(!runs.is_empty());

    let greedy_config = LineBreakerConfig {
        available_width_pt: 180.0,
        alignment: Alignment::Left,
        first_line_indent_pt: 0.0,
        line_spacing_mult: 1.15,
        use_knuth_plass: false,
    };
    let greedy_lines = break_paragraph_runs(&runs, &greedy_config, 0.0);
    assert!(greedy_lines.len() >= 3);

    let kp_config = LineBreakerConfig {
        available_width_pt: 180.0,
        alignment: Alignment::Justified,
        first_line_indent_pt: 0.0,
        line_spacing_mult: 1.15,
        use_knuth_plass: true,
    };
    let kp_lines = break_paragraph_runs(&runs, &kp_config, 0.0);
    assert!(!kp_lines.is_empty());

    // Both break modes must produce non-empty lines with valid bounds
    for line in &greedy_lines {
        assert!(line.bounds.width > 0.0);
        assert!(line.bounds.height > 0.0);
    }
    for line in &kp_lines {
        assert!(line.bounds.width > 0.0);
        assert!(line.bounds.height > 0.0);
    }
}

#[test]
fn test_text_alignments() {
    let registry = FontRegistry::new();
    let font = registry.resolve("Inter", false, false);
    let span = TextSpan {
        text: "Short centered headline".to_string(),
        style: FontStyle::bold(),
        font_size_pt: Some(14.0),
        font_family: Some("Inter".to_string()),
        font_weight: Some(700),
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        tracked_change_id: None,
        original_style: None,
        extra: Default::default(),
    };
    let runs = shape_span(&span, font, 14.0, 1.2);

    let left_cfg = LineBreakerConfig {
        available_width_pt: 400.0,
        alignment: Alignment::Left,
        first_line_indent_pt: 0.0,
        line_spacing_mult: 1.2,
        use_knuth_plass: false,
    };
    let left_line = &break_paragraph_runs(&runs, &left_cfg, 0.0)[0];
    assert_eq!(left_line.bounds.x, 0.0);

    let center_cfg = LineBreakerConfig {
        available_width_pt: 400.0,
        alignment: Alignment::Center,
        first_line_indent_pt: 0.0,
        line_spacing_mult: 1.2,
        use_knuth_plass: false,
    };
    let center_line = &break_paragraph_runs(&runs, &center_cfg, 0.0)[0];
    assert!(
        center_line.bounds.x > 50.0,
        "Center alignment must offset line.x"
    );

    let right_cfg = LineBreakerConfig {
        available_width_pt: 400.0,
        alignment: Alignment::Right,
        first_line_indent_pt: 0.0,
        line_spacing_mult: 1.2,
        use_knuth_plass: false,
    };
    let right_line = &break_paragraph_runs(&runs, &right_cfg, 0.0)[0];
    assert!(
        right_line.bounds.x > center_line.bounds.x,
        "Right alignment must be further right than center"
    );
}

#[test]
fn test_table_multi_page_splitting_and_headers() {
    let registry = FontRegistry::new();
    let mut rows = Vec::new();

    // Row 0: Header
    let h_cell1 = TableCell::new(
        "h1",
        vec![Block::Paragraph(ParagraphBlock::new(
            "hp1",
            sample_text_spans("Item Name"),
        ))],
    );
    let h_cell2 = TableCell::new(
        "h2",
        vec![Block::Paragraph(ParagraphBlock::new(
            "hp2",
            sample_text_spans("Amount EUR"),
        ))],
    );
    rows.push(TableRow::header(vec![h_cell1, h_cell2]));

    // 40 body rows to force multi-page table
    for i in 1..=40 {
        let c1 = TableCell::new(
            format!("c1_{}", i),
            vec![Block::Paragraph(ParagraphBlock::new(
                format!("p1_{}", i),
                sample_text_spans(&format!("Claim item {}", i)),
            ))],
        );
        let c2 = TableCell::new(
            format!("c2_{}", i),
            vec![Block::Paragraph(ParagraphBlock::new(
                format!("p2_{}", i),
                sample_text_spans(&format!("{}.00 €", i * 150)),
            ))],
        );
        rows.push(TableRow::new(vec![c1, c2]));
    }

    let mut table = TableBlock::new("t1", rows);
    table.repeat_header_rows = 1;

    let mut doc = FlowDocument::new(DocumentMetadata::new("doc_table", "Table Test"));
    let section = Section::new(
        "sec1",
        SectionGeometry::a4_portrait(),
        vec![Block::Table(table)],
    );
    doc.sections.push(section);

    let layout = layout_document(&doc, &registry);
    assert!(
        layout.total_pages >= 2,
        "40 rows table must paginate onto at least 2 pages"
    );

    // Both page 1 and page 2 must contain table boxes
    assert!(!layout.pages[0].content_boxes.is_empty());
    assert!(!layout.pages[1].content_boxes.is_empty());
}

#[test]
fn test_orphan_and_widow_control() {
    let registry = FontRegistry::new();
    let mut doc = FlowDocument::new(DocumentMetadata::new("doc_orphan", "Orphan Control Test"));

    // Fill page 1 with paragraphs until only a small sliver of space remains
    let mut blocks = Vec::new();
    for i in 0..35 {
        blocks.push(Block::Paragraph(ParagraphBlock::new(
            format!("p_fill_{}", i),
            sample_text_spans(
                "Standard legal pleading paragraph demonstrating page filling behavior.",
            ),
        )));
    }

    // Now a multi-line paragraph that would normally leave 1 line orphan
    let multi_line_text = "This is a long legal argument consisting of several sentences designed to span multiple lines. ".repeat(6);
    let splittable_para = ParagraphBlock::new("p_split", sample_text_spans(&multi_line_text));
    blocks.push(Block::Paragraph(splittable_para));

    let section = Section::new("sec1", SectionGeometry::a4_portrait(), blocks);
    doc.sections.push(section);

    let layout = layout_document(&doc, &registry);
    assert!(layout.total_pages >= 2);

    // Verify orphan control: if split across pages, page 2 must have at least 2 lines of the split paragraph
    let page2_lines: Vec<_> = layout.pages[1]
        .content_boxes
        .iter()
        .filter(|b| b.id.starts_with("p_split"))
        .collect();
    if !page2_lines.is_empty() {
        assert!(
            page2_lines.len() >= 2,
            "Orphan/widow control must ensure at least 2 lines on continuation page"
        );
    }
}

#[test]
fn test_incremental_layout_sub_16ms() {
    let registry = FontRegistry::new();
    let mut doc = FlowDocument::new(DocumentMetadata::new("doc_inc", "Incremental Test"));

    // Generate a long paragraph (500 words)
    let words = "Whereas the party of the first part hereby agrees to furnish the requisite legal documentation without undue delay and in accordance with section 433 of the civil code. ".repeat(25);
    let para = ParagraphBlock::new("target_p", sample_text_spans(&words));
    let section = Section::new(
        "sec1",
        SectionGeometry::a4_portrait(),
        vec![Block::Paragraph(para.clone())],
    );
    doc.sections.push(section);

    let mut engine = IncrementalLayoutEngine::new();
    let opts = PaginatorOptions::default();

    // Initial full layout
    let t0 = std::time::Instant::now();
    let initial_layout = engine.layout_full(&doc, &registry, &opts);
    let _full_time = t0.elapsed();
    assert_eq!(initial_layout.total_pages, 1);

    // Incremental update: modify single character without changing height
    let mut modified_para = para.clone();
    modified_para.spans[0].text = words.replacen("Whereas", "Therefore", 1);

    let t1 = std::time::Instant::now();
    let updated_layout = engine.update_paragraph(&doc, &modified_para, 0, &registry, &opts);
    let inc_time = t1.elapsed();

    assert_eq!(updated_layout.total_pages, 1);
    assert!(
        inc_time.as_millis() < 16,
        "Incremental typing update must execute in < 16ms, got {:?}",
        inc_time
    );
}

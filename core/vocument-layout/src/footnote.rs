//! Footnote allocation, measurement, and separator layout (`ADR-0020`, `AUTHOR-004`).

use std::collections::BTreeMap;
use vocument_flow::model::{Block, FootnoteDefinition, ParagraphBlock};
use vocument_flow::styles::{ColorRgba, FontStyle, Stylesheet, TextSpan};

use crate::font::FontRegistry;
use crate::geometry::Rect;
use crate::paragraph::{TypesettingContext, layout_paragraph};
use crate::tree::{LayoutBox, LayoutBoxKind};

/// Sized and positioned footnote area for a specific page.
#[derive(Debug, Clone, PartialEq)]
pub struct PageFootnotes {
    pub total_height_pt: f32,
    pub boxes: Vec<LayoutBox>,
}

/// Measures the required height for a list of footnote identifiers.
pub fn measure_footnotes_height(
    footnote_ids: &[String],
    definitions: &BTreeMap<String, FootnoteDefinition>,
    stylesheet: &Stylesheet,
    font_registry: &FontRegistry,
    available_width_pt: f32,
) -> f32 {
    if footnote_ids.is_empty() {
        return 0.0;
    }

    let mut total_h = 12.0; // Rule thickness + margins
    let ctx = TypesettingContext {
        stylesheet,
        font_registry,
        available_width_pt,
        use_knuth_plass: false,
    };

    for id in footnote_ids {
        if let Some(def) = definitions.get(id) {
            let h = measure_single_footnote(def, &ctx);
            total_h += h + 4.0;
        }
    }

    total_h
}

/// Layouts the footnote area at the bottom of a page above the footer.
pub fn layout_page_footnotes(
    footnote_ids: &[String],
    definitions: &BTreeMap<String, FootnoteDefinition>,
    stylesheet: &Stylesheet,
    font_registry: &FontRegistry,
    x_pt: f32,
    y_start_pt: f32,
    available_width_pt: f32,
) -> PageFootnotes {
    if footnote_ids.is_empty() {
        return PageFootnotes {
            total_height_pt: 0.0,
            boxes: Vec::new(),
        };
    }

    let mut boxes = Vec::new();
    let mut current_y = y_start_pt;

    // 1. Separator rule (classic legal / typestyle 2-inch rule)
    let rule_width = 144.0_f32.min(available_width_pt);
    boxes.push(LayoutBox {
        id: "footnote_separator_rule".to_string(),
        bounds: Rect::new(x_pt, current_y, rule_width, 0.5),
        kind: LayoutBoxKind::Rule {
            color: ColorRgba::rgb(160, 160, 160),
            thickness: 0.5,
        },
    });
    current_y += 8.0;

    let ctx = TypesettingContext {
        stylesheet,
        font_registry,
        available_width_pt,
        use_knuth_plass: false,
    };

    // 2. Typeset each footnote sequentially
    for id in footnote_ids {
        if let Some(def) = definitions.get(id) {
            let (fn_boxes, fn_height) = layout_single_footnote(def, x_pt, current_y, &ctx);
            boxes.extend(fn_boxes);
            current_y += fn_height + 4.0;
        }
    }

    PageFootnotes {
        total_height_pt: current_y - y_start_pt,
        boxes,
    }
}

fn measure_single_footnote(def: &FootnoteDefinition, ctx: &TypesettingContext) -> f32 {
    let mut h = 0.0;
    for (idx, block) in def.blocks.iter().enumerate() {
        match block {
            Block::Paragraph(p) => {
                let mut para = p.clone();
                if idx == 0 {
                    prepend_footnote_marker(&mut para, def.number);
                }
                let pos = layout_paragraph(&para, ctx);
                h += pos.total_height_pt;
            }
            _ => {
                h += 14.0;
            }
        }
    }
    h.max(12.0)
}

fn layout_single_footnote(
    def: &FootnoteDefinition,
    x_pt: f32,
    y_pt: f32,
    ctx: &TypesettingContext,
) -> (Vec<LayoutBox>, f32) {
    let mut boxes = Vec::new();
    let mut current_y = y_pt;

    for (idx, block) in def.blocks.iter().enumerate() {
        if let Block::Paragraph(p) = block {
            let mut para = p.clone();
            if para.style_id.is_none() {
                para.style_id = Some("footnote".to_string());
            }
            if idx == 0 {
                prepend_footnote_marker(&mut para, def.number);
            }
            let pos = layout_paragraph(&para, ctx);
            let line_boxes = pos.to_layout_boxes(x_pt, current_y);
            boxes.extend(line_boxes);
            current_y += pos.total_height_pt;
        }
    }

    let height = current_y - y_pt;
    (boxes, height)
}

fn prepend_footnote_marker(para: &mut ParagraphBlock, number: u32) {
    let marker_span = TextSpan {
        text: format!("{}. ", number),
        style: FontStyle::bold(),
        font_size_pt: Some(9.0),
        font_family: Some("Inter".to_string()),
        font_weight: Some(700),
        color: None,
        background_color: None,
        link_url: None,
        footnote_ref: None,
        extra: Default::default(),
    };
    para.spans.insert(0, marker_span);
}

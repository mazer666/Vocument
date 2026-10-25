//! Paragraph and text block typesetting (`ADR-0020`, `AUTHOR-004`).

use std::collections::HashSet;
use vocument_flow::model::{HeadingBlock, ListBlock, ListKind, ParagraphBlock};
use vocument_flow::styles::{Alignment, FontStyle, ParagraphStyle, Stylesheet, TextSpan};

use crate::font::FontRegistry;
use crate::geometry::Rect;
use crate::line_breaker::{LineBreakerConfig, break_paragraph_runs};
use crate::shaper::{ShapedRun, shape_span};
use crate::tree::{LayoutBox, LayoutLine};

/// Typeset representation of a paragraph or block of text with pagination flags.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedParagraph {
    pub block_id: String,
    pub lines: Vec<LayoutLine>,
    pub space_before_pt: f32,
    pub space_after_pt: f32,
    pub keep_with_next: bool,
    pub keep_lines_together: bool,
    pub orphan_control: bool,
    pub total_height_pt: f32,
    pub footnote_refs: Vec<String>,
}

impl PositionedParagraph {
    /// Converts positioned lines into visual layout boxes.
    pub fn to_layout_boxes(&self, x_offset_pt: f32, y_offset_pt: f32) -> Vec<LayoutBox> {
        let mut boxes = Vec::with_capacity(self.lines.len());
        for (idx, line) in self.lines.iter().enumerate() {
            let line_bounds = Rect::new(
                x_offset_pt + line.bounds.x,
                y_offset_pt + line.bounds.y,
                line.bounds.width,
                line.bounds.height,
            );
            let box_id = format!("{}:line_{}", self.block_id, idx);
            let mut positioned_line = line.clone();
            positioned_line.bounds = line_bounds;
            boxes.push(LayoutBox::text_line(box_id, line_bounds, positioned_line));
        }
        boxes
    }
}

/// Helper context for resolving styles and fonts during typesetting.
#[derive(Debug)]
pub struct TypesettingContext<'a> {
    pub stylesheet: &'a Stylesheet,
    pub font_registry: &'a FontRegistry,
    pub available_width_pt: f32,
    pub use_knuth_plass: bool,
}

/// Typesets a `ParagraphBlock` into a `PositionedParagraph`.
pub fn layout_paragraph(para: &ParagraphBlock, ctx: &TypesettingContext) -> PositionedParagraph {
    let style = resolve_paragraph_style(para.style_id.as_deref(), ctx.stylesheet);
    let space_before = para.space_before_pt.unwrap_or(style.space_before_pt);
    let space_after = para.space_after_pt.unwrap_or(style.space_after_pt);
    let alignment = para.alignment.unwrap_or(style.alignment);

    let (runs, footnote_refs) = shape_spans(&para.spans, &style, ctx.font_registry);

    let config = LineBreakerConfig {
        available_width_pt: ctx.available_width_pt,
        alignment,
        first_line_indent_pt: style.first_line_indent_pt,
        line_spacing_mult: style.line_spacing_mult,
        use_knuth_plass: ctx.use_knuth_plass,
    };

    let lines = break_paragraph_runs(&runs, &config, 0.0);
    let total_height = calculate_lines_height(&lines);

    PositionedParagraph {
        block_id: para.id.clone(),
        lines,
        space_before_pt: space_before,
        space_after_pt: space_after,
        keep_with_next: style.keep_with_next,
        keep_lines_together: style.keep_lines_together,
        orphan_control: style.orphan_control,
        total_height_pt: total_height,
        footnote_refs,
    }
}

/// Typesets a `HeadingBlock` into a `PositionedParagraph`.
pub fn layout_heading(heading: &HeadingBlock, ctx: &TypesettingContext) -> PositionedParagraph {
    let heading_style_id = format!("heading{}", heading.level);
    let style_id = heading.style_id.as_deref().unwrap_or(&heading_style_id);
    let style = resolve_paragraph_style(Some(style_id), ctx.stylesheet);

    // If numbered, prepend numbering span
    let mut spans = Vec::new();
    if heading.numbered {
        let prefix = heading
            .number_prefix
            .clone()
            .unwrap_or_else(|| format!("{}. ", heading.level));
        spans.push(TextSpan {
            text: prefix,
            style: FontStyle::bold(),
            font_size_pt: Some(style.font_size_pt),
            font_family: Some(style.font_family.clone()),
            font_weight: Some(700),
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            tracked_change_id: None,
            extra: Default::default(),
        });
    }
    spans.extend(heading.spans.clone());

    let (runs, footnote_refs) = shape_spans(&spans, &style, ctx.font_registry);

    let config = LineBreakerConfig {
        available_width_pt: ctx.available_width_pt,
        alignment: style.alignment,
        first_line_indent_pt: 0.0,
        line_spacing_mult: style.line_spacing_mult,
        use_knuth_plass: ctx.use_knuth_plass,
    };

    let lines = break_paragraph_runs(&runs, &config, 0.0);
    let total_height = calculate_lines_height(&lines);

    // Headings always default to keep_with_next = true unless explicitly overridden
    let keep_with_next = true;

    PositionedParagraph {
        block_id: heading.id.clone(),
        lines,
        space_before_pt: style.space_before_pt,
        space_after_pt: style.space_after_pt,
        keep_with_next,
        keep_lines_together: true, // Don't split headings across pages
        orphan_control: false,
        total_height_pt: total_height,
        footnote_refs,
    }
}

/// Typesets a `ListBlock` into a sequence of positioned paragraphs.
pub fn layout_list(
    list: &ListBlock,
    ctx: &TypesettingContext,
    indent_level: usize,
) -> Vec<PositionedParagraph> {
    let mut result = Vec::new();
    let style = resolve_paragraph_style(list.style_id.as_deref(), ctx.stylesheet);
    let bullet_indent = 18.0 * (indent_level as f32 + 1.0);
    let item_width = (ctx.available_width_pt - bullet_indent).max(36.0);

    for (idx, item) in list.items.iter().enumerate() {
        let marker = match list.kind {
            ListKind::Ordered => format!("{}. ", idx + 1),
            ListKind::Unordered => "• ".to_string(),
            ListKind::Task => {
                if item.checked.unwrap_or(false) {
                    "[x] ".to_string()
                } else {
                    "[ ] ".to_string()
                }
            }
        };

        let mut item_spans = Vec::new();
        item_spans.push(TextSpan {
            text: marker,
            style: FontStyle::PLAIN,
            font_size_pt: Some(style.font_size_pt),
            font_family: Some(style.font_family.clone()),
            font_weight: None,
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            tracked_change_id: None,
            extra: Default::default(),
        });
        item_spans.extend(item.spans.clone());

        let (runs, footnote_refs) = shape_spans(&item_spans, &style, ctx.font_registry);

        let config = LineBreakerConfig {
            available_width_pt: item_width,
            alignment: Alignment::Left,
            first_line_indent_pt: 0.0,
            line_spacing_mult: style.line_spacing_mult,
            use_knuth_plass: ctx.use_knuth_plass,
        };

        let mut lines = break_paragraph_runs(&runs, &config, 0.0);
        // Shift lines by bullet indent
        for line in &mut lines {
            line.bounds.x += bullet_indent;
            for run in &mut line.runs {
                run.bounds.x += bullet_indent;
            }
        }

        let total_height = calculate_lines_height(&lines);

        result.push(PositionedParagraph {
            block_id: item.id.clone(),
            lines,
            space_before_pt: if idx == 0 { style.space_before_pt } else { 2.0 },
            space_after_pt: if idx + 1 == list.items.len() {
                style.space_after_pt
            } else {
                2.0
            },
            keep_with_next: false,
            keep_lines_together: false,
            orphan_control: style.orphan_control,
            total_height_pt: total_height,
            footnote_refs,
        });

        // Handle nested lists recursively
        if let Some(nested) = &item.nested_list {
            let nested_paras = layout_list(nested, ctx, indent_level + 1);
            result.extend(nested_paras);
        }
    }

    result
}

/// Resolves a paragraph style from the stylesheet with fallback to "normal" or default.
pub fn resolve_paragraph_style(style_id: Option<&str>, stylesheet: &Stylesheet) -> ParagraphStyle {
    if let Some(id) = style_id {
        if let Some(style) = stylesheet.paragraph_styles.get(id) {
            return style.clone();
        }
    }

    if let Some(normal) = stylesheet.paragraph_styles.get("normal") {
        return normal.clone();
    }

    ParagraphStyle {
        id: "default".to_string(),
        name: "Default".to_string(),
        font_family: "Inter".to_string(),
        font_size_pt: 11.0,
        line_spacing_mult: 1.15,
        space_before_pt: 0.0,
        space_after_pt: 6.0,
        alignment: Alignment::Left,
        first_line_indent_pt: 0.0,
        keep_with_next: false,
        keep_lines_together: false,
        orphan_control: true,
    }
}

/// Shapes a sequence of spans into shaped runs using the font registry.
pub fn shape_spans(
    spans: &[TextSpan],
    style: &ParagraphStyle,
    font_registry: &FontRegistry,
) -> (Vec<ShapedRun>, Vec<String>) {
    let mut runs = Vec::new();
    let mut footnote_refs = Vec::new();
    let mut seen_footnotes = HashSet::new();

    for span in spans {
        let font_family = span.font_family.as_deref().unwrap_or(&style.font_family);
        let is_bold = span.style.bold || span.font_weight.unwrap_or(400) >= 600;
        let is_italic = span.style.italic;

        let font = font_registry.resolve(font_family, is_bold, is_italic);
        let shaped = shape_span(span, font, style.font_size_pt, style.line_spacing_mult);

        if let Some(fn_id) = &span.footnote_ref {
            if seen_footnotes.insert(fn_id.clone()) {
                footnote_refs.push(fn_id.clone());
            }
        }

        runs.extend(shaped);
    }

    (runs, footnote_refs)
}

fn calculate_lines_height(lines: &[LayoutLine]) -> f32 {
    if lines.is_empty() {
        return 0.0;
    }
    let last = &lines[lines.len() - 1];
    last.bounds.y + last.bounds.height
}

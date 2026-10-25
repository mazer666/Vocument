//! Unicode segmentation, BiDi direction analysis, and text run shaping (`ADR-0020`, `AUTHOR-004`).

use std::sync::Arc;
use vocument_flow::styles::{ColorRgba, FontStyle, TextSpan};

use crate::font::Font;
use crate::geometry::FixedPt;

/// Directionality of a text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

/// An individual positioned glyph in a shaped run.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub char_value: char,
    pub cluster_index: usize,
    pub x_advance: FixedPt,
    pub x_offset: FixedPt,
    pub y_offset: FixedPt,
}

/// A shaped run of contiguous text sharing identical font, style, and direction.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapedRun {
    pub text: String,
    pub font: Arc<Font>,
    pub font_size_pt: f32,
    pub font_style: FontStyle,
    pub color: ColorRgba,
    pub link_url: Option<String>,
    pub footnote_ref: Option<String>,
    pub direction: Direction,
    pub glyphs: Vec<ShapedGlyph>,
    pub total_advance: FixedPt,
    pub ascender_pt: f32,
    pub descender_pt: f32,
    pub line_height_pt: f32,
    pub is_whitespace: bool,
}

impl ShapedRun {
    pub fn advance_pt(&self) -> f32 {
        self.total_advance.to_f32()
    }
}

/// Shapes an inline `TextSpan` into a sequence of atomic word and whitespace `ShapedRun` items.
pub fn shape_span(
    span: &TextSpan,
    font: Arc<Font>,
    default_font_size: f32,
    line_spacing_mult: f32,
) -> Vec<ShapedRun> {
    let font_size = span.font_size_pt.unwrap_or(default_font_size);
    let color = span.color.unwrap_or(ColorRgba::BLACK);
    let text = &span.text;

    if text.is_empty() {
        return Vec::new();
    }

    let mut runs = Vec::new();
    let mut current_segment = String::new();
    let mut in_whitespace = None;

    // Segment text into alternating non-whitespace words and whitespace tokens
    for c in text.chars() {
        let is_ws = c.is_whitespace();
        match in_whitespace {
            Some(ws) if ws == is_ws => {
                current_segment.push(c);
            }
            Some(_) => {
                if !current_segment.is_empty() {
                    let run = create_shaped_run(
                        &current_segment,
                        &font,
                        font_size,
                        span.style,
                        color,
                        span.link_url.clone(),
                        span.footnote_ref.clone(),
                        line_spacing_mult,
                    );
                    runs.push(run);
                    current_segment.clear();
                }
                current_segment.push(c);
                in_whitespace = Some(is_ws);
            }
            None => {
                current_segment.push(c);
                in_whitespace = Some(is_ws);
            }
        }
    }

    if !current_segment.is_empty() {
        let run = create_shaped_run(
            &current_segment,
            &font,
            font_size,
            span.style,
            color,
            span.link_url.clone(),
            span.footnote_ref.clone(),
            line_spacing_mult,
        );
        runs.push(run);
    }

    runs
}

#[allow(clippy::too_many_arguments)]
fn create_shaped_run(
    text: &str,
    font: &Arc<Font>,
    font_size_pt: f32,
    font_style: FontStyle,
    color: ColorRgba,
    link_url: Option<String>,
    footnote_ref: Option<String>,
    line_spacing_mult: f32,
) -> ShapedRun {
    let mut glyphs = Vec::with_capacity(text.len());
    let mut total_advance = FixedPt::ZERO;
    let mut prev_char = None;
    let is_whitespace = text.chars().all(|c| c.is_whitespace());
    let direction = detect_direction(text);

    for (cluster_idx, c) in text.char_indices() {
        let kern = if let Some(p) = prev_char {
            font.kerning_fixed(p, c, font_size_pt)
        } else {
            FixedPt::ZERO
        };

        let adv = font.advance_width_fixed(c, font_size_pt) + kern;
        glyphs.push(ShapedGlyph {
            char_value: c,
            cluster_index: cluster_idx,
            x_advance: adv,
            x_offset: FixedPt::ZERO,
            y_offset: FixedPt::ZERO,
        });
        total_advance += adv;
        prev_char = Some(c);
    }

    let ascender_pt = font.ascender_pt(font_size_pt);
    let descender_pt = font.descender_pt(font_size_pt);
    let line_height_pt = font.line_height_pt(font_size_pt, line_spacing_mult);

    ShapedRun {
        text: text.to_string(),
        font: Arc::clone(font),
        font_size_pt,
        font_style,
        color,
        link_url,
        footnote_ref,
        direction,
        glyphs,
        total_advance,
        ascender_pt,
        descender_pt,
        line_height_pt,
        is_whitespace,
    }
}

/// Lightweight Unicode Bidirectional heuristic detecting RTL text (Arabic, Hebrew, Persian).
fn detect_direction(text: &str) -> Direction {
    for c in text.chars() {
        let u = c as u32;
        // Hebrew: 0x0590..=0x05FF, Arabic: 0x0600..=0x06FF, Arabic Supplement: 0x0750..=0x077F
        if (0x0590..=0x05FF).contains(&u)
            || (0x0600..=0x06FF).contains(&u)
            || (0x0750..=0x077F).contains(&u)
            || (0x08A0..=0x08FF).contains(&u)
        {
            return Direction::RightToLeft;
        }
    }
    Direction::LeftToRight
}

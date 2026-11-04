//! Page content stream generation for the tagged PDF export (`ADR-0020`, `ROADMAP.md` M4.4).
//!
//! Text lines are wrapped in marked-content sequences (`/P << /MCID n >> BDC … EMC`) that the
//! logical structure tree points at. Everything that is not real content — backgrounds, borders,
//! rules, headers and footers — is marked as `/Artifact`, as ISO 14289-1 (7.1) requires.

use std::fmt::Write as FmtWrite;

use crate::tree::{LayoutBox, LayoutBoxKind, LayoutLine, LayoutPage};
use vocument_flow::styles::ColorRgba;

use super::font_embedding::{
    EmbeddedFontMetrics, char_to_win_ansi_code, encode_single_byte_to_literal, encode_utf16be_hex,
    encode_win_ansi_literal, has_non_win_ansi,
};
use super::structure::{StructureRoles, StructureTree};
use super::unicode_fallback::{
    FALLBACK_ARABIC_FONT_RESOURCE_NAME, FALLBACK_UNICODE_FONT_RESOURCE_NAME, FallbackScript,
    UnicodeFallbackRegistry,
};

/// Name under which the embedded font is registered in every page's resource dictionary.
pub(super) const FONT_RESOURCE_NAME: &str = "F1";

/// Largest per-character spacing correction, as a fraction of the font size. The layout engine
/// measures text with calibrated metrics while the PDF embeds the real glyph widths; the
/// correction keeps every run inside the geometry the layout engine decided on, and the cap stops
/// a metrics mismatch from distorting text.
const MAX_CHARACTER_SPACING_FRACTION: f32 = 0.12;

/// Stroke width of synthetic bold text as a fraction of the font size.
const SYNTHETIC_BOLD_STROKE_FRACTION: f32 = 0.03;

/// Horizontal shear of synthetic italic text (tangent of roughly 11 degrees).
const SYNTHETIC_ITALIC_SHEAR: f32 = 0.2;

/// Where a text line sits on the page and which block it belongs to; the comment annotation
/// placement uses it to find the line a comment is anchored to.
#[derive(Debug, Clone)]
pub(super) struct LineAnchor {
    pub(super) block_id: String,
    pub(super) pdf_top_y: f32,
}

/// Where an image sits on a page and which resource identifier names it.
#[derive(Debug, Clone)]
pub(super) struct PageImagePlacement {
    pub(super) resource_name: String,
    pub(super) asset_ref: String,
}

/// Result of rendering one page.
#[derive(Debug, Default)]
pub(super) struct RenderedPage {
    pub(super) content_stream: String,
    pub(super) line_anchors: Vec<LineAnchor>,
    /// Structure element that owns each marked-content identifier, indexed by MCID.
    pub(super) mcid_owners: Vec<usize>,
    /// Placed raster image XObjects on this page.
    pub(super) images: Vec<PageImagePlacement>,
}

struct PageRenderer<'a> {
    page_index: usize,
    page_height: f32,
    roles: &'a StructureRoles,
    tree: &'a mut StructureTree,
    metrics: &'a EmbeddedFontMetrics,
    fallback_registry: &'a mut UnicodeFallbackRegistry,
    rendered: RenderedPage,
}

/// Renders one layout page into a content stream and registers its text in the structure tree.
pub(super) fn render_page(
    page: &LayoutPage,
    page_index: usize,
    roles: &StructureRoles,
    tree: &mut StructureTree,
    metrics: &EmbeddedFontMetrics,
    fallback_registry: &mut UnicodeFallbackRegistry,
) -> RenderedPage {
    let mut renderer = PageRenderer {
        page_index,
        page_height: page.size.height,
        roles,
        tree,
        metrics,
        fallback_registry,
        rendered: RenderedPage::default(),
    };
    renderer.render_boxes(&page.header_boxes, 0.0, 0.0, false);
    renderer.render_boxes(&page.content_boxes, 0.0, 0.0, true);
    renderer.render_boxes(&page.footnote_boxes, 0.0, 0.0, true);
    renderer.render_boxes(&page.footer_boxes, 0.0, 0.0, false);
    renderer.rendered
}

impl PageRenderer<'_> {
    fn render_boxes(&mut self, boxes: &[LayoutBox], origin_x: f32, origin_y: f32, tagged: bool) {
        for layout_box in boxes {
            let x = origin_x + layout_box.bounds.x;
            let y = origin_y + layout_box.bounds.y;
            match &layout_box.kind {
                LayoutBoxKind::TextLine(line) => {
                    self.render_text_line(&layout_box.id, line, x, y, tagged);
                }
                LayoutBoxKind::SolidRect { color } => {
                    self.render_filled_rect(
                        x,
                        y,
                        layout_box.bounds.width,
                        layout_box.bounds.height,
                        *color,
                    );
                }
                LayoutBoxKind::BorderRect { width, color } => {
                    self.render_stroked_rect(
                        x,
                        y,
                        layout_box.bounds.width,
                        layout_box.bounds.height,
                        *width,
                        *color,
                    );
                }
                LayoutBoxKind::Rule { color, thickness } => {
                    self.render_filled_rect(
                        x,
                        y,
                        layout_box.bounds.width,
                        thickness.max(0.25),
                        *color,
                    );
                }
                LayoutBoxKind::Container { children } => {
                    self.render_boxes(children, x, y, tagged);
                }
                LayoutBoxKind::Image { asset_ref } => {
                    self.render_image(
                        &layout_box.id,
                        asset_ref,
                        x,
                        y,
                        layout_box.bounds.width,
                        layout_box.bounds.height,
                        tagged,
                    );
                }
            }
        }
    }

    fn pdf_y(&self, layout_y: f32) -> f32 {
        self.page_height - layout_y
    }

    fn render_filled_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: ColorRgba) {
        let stream = &mut self.rendered.content_stream;
        let _ = writeln!(
            stream,
            "/Artifact BMC\nq\n{} rg\n{:.2} {:.2} {:.2} {:.2} re\nf\nQ\nEMC",
            color_components(color),
            x,
            self.page_height - (y + height),
            width,
            height
        );
    }

    fn render_stroked_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        line_width: f32,
        color: ColorRgba,
    ) {
        let stream = &mut self.rendered.content_stream;
        let _ = writeln!(
            stream,
            "/Artifact BMC\nq\n{} RG\n{:.2} w\n{:.2} {:.2} {:.2} {:.2} re\nS\nQ\nEMC",
            color_components(color),
            line_width,
            x,
            self.page_height - (y + height),
            width,
            height
        );
    }

    fn render_text_line(&mut self, box_id: &str, line: &LayoutLine, x: f32, y: f32, tagged: bool) {
        if line.runs.is_empty() {
            return;
        }

        // `LayoutBox.bounds` is the only position every placement layer (page, table slice,
        // callout container) has shifted; run offsets inside the line are still line-local.
        let line_start_x = line.runs[0].bounds.x;
        let baseline = self.pdf_y(y + line.ascent_pt);

        if tagged {
            let path = self.roles.path_for_box_id(box_id);
            let mcid = u32::try_from(self.rendered.mcid_owners.len()).unwrap_or(u32::MAX);
            let owner = self.tree.add_marked_content(&path, self.page_index, mcid);
            self.rendered.mcid_owners.push(owner);
            let structure_type = self.tree.nodes[owner].structure_type;
            let _ = writeln!(
                self.rendered.content_stream,
                "/{structure_type} << /MCID {mcid} >> BDC"
            );
            let block_id = box_id
                .rsplit_once(":line_")
                .map_or(box_id, |(block_id, _)| block_id);
            self.rendered.line_anchors.push(LineAnchor {
                block_id: block_id.to_string(),
                pdf_top_y: self.pdf_y(y),
            });
        } else {
            self.rendered.content_stream.push_str("/Artifact BMC\n");
        }

        self.rendered.content_stream.push_str("BT\n");
        for run in &line.runs {
            if run.text.is_empty() {
                continue;
            }
            self.render_text_run(run, x + (run.bounds.x - line_start_x), baseline);
        }
        self.rendered.content_stream.push_str("ET\n");

        // Render visual tracked changes and text decorations (underline, strikethrough)
        for run in &line.runs {
            if run.text.is_empty() {
                continue;
            }
            let run_x = x + (run.bounds.x - line_start_x);
            let run_w = run.bounds.width;
            let font_size = run.font_size_pt;
            let stroke_width = (font_size * 0.06).max(0.75);
            let color = color_components(run.color);

            if run.font_style.underline {
                let underline_y = baseline - font_size * 0.12;
                let _ = writeln!(
                    self.rendered.content_stream,
                    "/Artifact BMC\nq\n{color} RG\n{stroke_width:.2} w\n{run_x:.2} {underline_y:.2} m {:.2} {underline_y:.2} l\nS\nQ\nEMC",
                    run_x + run_w
                );
            }
            if run.font_style.strikethrough {
                let strikethrough_y = baseline + font_size * 0.28;
                let _ = writeln!(
                    self.rendered.content_stream,
                    "/Artifact BMC\nq\n{color} RG\n{stroke_width:.2} w\n{run_x:.2} {strikethrough_y:.2} m {:.2} {strikethrough_y:.2} l\nS\nQ\nEMC",
                    run_x + run_w
                );
            }
        }

        self.rendered.content_stream.push_str("EMC\n");
    }

    #[allow(clippy::too_many_arguments)]
    fn render_image(
        &mut self,
        box_id: &str,
        asset_ref: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        tagged: bool,
    ) {
        let image_index = self.rendered.images.len() + 1;
        let resource_name = format!("Im{image_index}");
        self.rendered.images.push(PageImagePlacement {
            resource_name: resource_name.clone(),
            asset_ref: asset_ref.to_string(),
        });

        let y_bottom = self.page_height - (y + height);

        if tagged {
            let path = self.roles.path_for_box_id(box_id);
            let mcid = u32::try_from(self.rendered.mcid_owners.len()).unwrap_or(u32::MAX);
            let owner = self.tree.add_marked_content(&path, self.page_index, mcid);
            self.rendered.mcid_owners.push(owner);
            let structure_type = self.tree.nodes[owner].structure_type;
            let _ = writeln!(
                self.rendered.content_stream,
                "/{structure_type} << /MCID {mcid} >> BDC\nq\n{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/{} Do\nQ\nEMC",
                width, height, x, y_bottom, resource_name
            );
        } else {
            let _ = writeln!(
                self.rendered.content_stream,
                "/Artifact BMC\nq\n{:.2} 0 0 {:.2} {:.2} {:.2} cm\n/{} Do\nQ\nEMC",
                width, height, x, y_bottom, resource_name
            );
        }
    }

    fn render_text_run(&mut self, run: &crate::tree::PositionedRun, x: f32, baseline: f32) {
        let font_size = run.font_size_pt;
        let character_count = run.text.chars().count();
        let natural_width = self.metrics.text_width_pt(&run.text, font_size);
        let character_spacing = if character_count > 0 && run.bounds.width > 0.0 {
            let max_spacing = font_size * MAX_CHARACTER_SPACING_FRACTION;
            ((run.bounds.width - natural_width) / character_count as f32)
                .clamp(-max_spacing, max_spacing)
        } else {
            0.0
        };
        let shear = if run.font_style.italic {
            SYNTHETIC_ITALIC_SHEAR
        } else {
            0.0
        };

        let components = color_components(run.color);
        {
            let stream = &mut self.rendered.content_stream;
            let _ = writeln!(stream, "{components} rg");
            if run.font_style.bold {
                let _ = writeln!(
                    stream,
                    "{components} RG\n{:.3} w\n2 Tr",
                    font_size * SYNTHETIC_BOLD_STROKE_FRACTION
                );
            }
            let _ = writeln!(stream, "/{FONT_RESOURCE_NAME} {font_size:.2} Tf");
            let _ = writeln!(stream, "{character_spacing:.3} Tc");
            let _ = writeln!(stream, "1 0 {shear:.2} 1 {x:.2} {baseline:.2} Tm");
            if has_non_win_ansi(&run.text) {
                let _ = writeln!(
                    stream,
                    "/Span << /ActualText {} >> BDC",
                    encode_utf16be_hex(&run.text)
                );
            } else {
                let _ = writeln!(stream, "({}) Tj", encode_win_ansi_literal(&run.text));
            }
        }
        if has_non_win_ansi(&run.text) {
            self.render_segmented_text(&run.text, font_size);
            self.rendered.content_stream.push_str("EMC\n");
        }
        if run.font_style.bold {
            self.rendered.content_stream.push_str("0 Tr\n");
        }
    }

    fn render_segmented_text(&mut self, text: &str, font_size: f32) {
        let stream = &mut self.rendered.content_stream;
        let mut cur_font = 0u8; // 0: Base F1, 1: Unicode F2, 2: Arabic F3
        let mut cur_win_ansi = String::new();

        for ch in text.chars() {
            if let Some(code) = char_to_win_ansi_code(ch) {
                if cur_font != 0 {
                    let _ = writeln!(stream, "/{FONT_RESOURCE_NAME} {font_size:.2} Tf");
                    cur_font = 0;
                }
                encode_single_byte_to_literal(code, &mut cur_win_ansi);
            } else if let Some(fb) = self.fallback_registry.register_char(ch) {
                if !cur_win_ansi.is_empty() {
                    let _ = writeln!(stream, "({cur_win_ansi}) Tj");
                    cur_win_ansi.clear();
                }
                match fb.script {
                    FallbackScript::Unicode => {
                        if cur_font != 1 {
                            let _ = writeln!(
                                stream,
                                "/{FALLBACK_UNICODE_FONT_RESOURCE_NAME} {font_size:.2} Tf"
                            );
                            cur_font = 1;
                        }
                        let _ = writeln!(stream, "<{:04X}> Tj", fb.gid);
                    }
                    FallbackScript::Arabic => {
                        if cur_font != 2 {
                            let _ = writeln!(
                                stream,
                                "/{FALLBACK_ARABIC_FONT_RESOURCE_NAME} {font_size:.2} Tf"
                            );
                            cur_font = 2;
                        }
                        let _ = writeln!(stream, "<{:04X}> Tj", fb.gid);
                    }
                }
            }
        }

        if !cur_win_ansi.is_empty() {
            let _ = writeln!(stream, "({cur_win_ansi}) Tj");
        }
        if cur_font != 0 {
            let _ = writeln!(stream, "/{FONT_RESOURCE_NAME} {font_size:.2} Tf");
        }
    }
}

/// DeviceRGB components of a colour as three decimals.
fn color_components(color: ColorRgba) -> String {
    format!(
        "{:.3} {:.3} {:.3}",
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0
    )
}

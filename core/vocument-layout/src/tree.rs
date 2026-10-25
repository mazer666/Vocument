//! Platform-independent visual layout tree representation (`ADR-0020`, `AUTHOR-004`).

use serde::{Deserialize, Serialize};
use vocument_flow::styles::{ColorRgba, FontStyle};

use crate::geometry::{Insets, Rect, Size};

/// A fully paginated, positioned document ready for rendering or PDF export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutDocument {
    pub pages: Vec<LayoutPage>,
    pub total_pages: usize,
}

impl LayoutDocument {
    pub fn new(pages: Vec<LayoutPage>) -> Self {
        let total_pages = pages.len();
        Self { pages, total_pages }
    }

    /// Serializes this layout document to deterministic canonical JSON.
    pub fn to_json_canonical(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserializes a layout document from JSON.
    pub fn from_json_str(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// A single rendered page containing positioned layout boxes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutPage {
    pub page_number: usize,
    pub size: Size,
    pub margin_insets: Insets,
    pub header_boxes: Vec<LayoutBox>,
    pub content_boxes: Vec<LayoutBox>,
    pub footer_boxes: Vec<LayoutBox>,
    pub footnote_boxes: Vec<LayoutBox>,
}

impl LayoutPage {
    pub fn new(page_number: usize, size: Size, margin_insets: Insets) -> Self {
        Self {
            page_number,
            size,
            margin_insets,
            header_boxes: Vec::new(),
            content_boxes: Vec::new(),
            footer_boxes: Vec::new(),
            footnote_boxes: Vec::new(),
        }
    }

    /// Returns an iterator over all visual boxes on this page.
    pub fn all_boxes(&self) -> impl Iterator<Item = &LayoutBox> {
        self.header_boxes
            .iter()
            .chain(self.content_boxes.iter())
            .chain(self.footnote_boxes.iter())
            .chain(self.footer_boxes.iter())
    }
}

/// Specific visual content kind of a layout box.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutBoxKind {
    /// A single line of typeset text.
    TextLine(LayoutLine),
    /// A filled solid rectangle (e.g. table cell background or callout box).
    SolidRect { color: ColorRgba },
    /// A stroked border rectangle.
    BorderRect { width: f32, color: ColorRgba },
    /// An embedded raster or vector image.
    Image { asset_ref: String },
    /// A horizontal rule divider or footnote separator line.
    Rule { color: ColorRgba, thickness: f32 },
    /// A composite group containing child layout boxes.
    Container { children: Vec<LayoutBox> },
}

/// An individual positioned visual element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutBox {
    pub id: String,
    pub bounds: Rect,
    pub kind: LayoutBoxKind,
}

impl LayoutBox {
    pub fn text_line(id: impl Into<String>, bounds: Rect, line: LayoutLine) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::TextLine(line),
        }
    }

    pub fn solid_rect(id: impl Into<String>, bounds: Rect, color: ColorRgba) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::SolidRect { color },
        }
    }

    pub fn border_rect(id: impl Into<String>, bounds: Rect, width: f32, color: ColorRgba) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::BorderRect { width, color },
        }
    }

    pub fn image(id: impl Into<String>, bounds: Rect, asset_ref: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::Image {
                asset_ref: asset_ref.into(),
            },
        }
    }

    pub fn rule(id: impl Into<String>, bounds: Rect, color: ColorRgba, thickness: f32) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::Rule { color, thickness },
        }
    }

    pub fn container(id: impl Into<String>, bounds: Rect, children: Vec<LayoutBox>) -> Self {
        Self {
            id: id.into(),
            bounds,
            kind: LayoutBoxKind::Container { children },
        }
    }

    /// Extracts plain text from this layout box.
    pub fn plain_text(&self) -> String {
        match &self.kind {
            LayoutBoxKind::TextLine(l) => l.plain_text(),
            LayoutBoxKind::Container { children } => children
                .iter()
                .map(|c| c.plain_text())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        }
    }
}

/// A typeset text line with baseline position and individual text runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutLine {
    pub bounds: Rect,
    pub runs: Vec<PositionedRun>,
    pub baseline_y: f32,
    pub ascent_pt: f32,
    pub descent_pt: f32,
    pub footnote_refs: Vec<String>,
}

impl LayoutLine {
    pub fn plain_text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// An individual positioned text run within a layout line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionedRun {
    pub text: String,
    pub bounds: Rect,
    pub font_family: String,
    pub font_size_pt: f32,
    pub font_style: FontStyle,
    pub color: ColorRgba,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footnote_ref: Option<String>,
}

//! Core semantic document structures, blocks, sections, and tables (`ADR-0020`, `AUTHOR-001`–`005`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::FlowError;
use crate::metadata::DocumentMetadata;
use crate::styles::{Alignment, ColorRgba, Stylesheet, TextSpan};

/// Page orientation for a section geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum PageOrientation {
    #[default]
    Portrait,
    Landscape,
}

/// Geometric dimensions and margins for a section (`AUTHOR-004`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionGeometry {
    pub width_pt: f32,
    pub height_pt: f32,
    pub margin_top_pt: f32,
    pub margin_bottom_pt: f32,
    pub margin_left_pt: f32,
    pub margin_right_pt: f32,
    #[serde(default)]
    pub orientation: PageOrientation,
    #[serde(default = "default_column_count")]
    pub column_count: u32,
    #[serde(default = "default_column_gap")]
    pub column_gap_pt: f32,
}

fn default_column_count() -> u32 {
    1
}

fn default_column_gap() -> f32 {
    18.0
}

impl SectionGeometry {
    /// Standard ISO A4 portrait geometry (595.276 x 841.89 pt with 72pt margins).
    pub fn a4_portrait() -> Self {
        Self {
            width_pt: 595.276,
            height_pt: 841.890,
            margin_top_pt: 72.0,
            margin_bottom_pt: 72.0,
            margin_left_pt: 72.0,
            margin_right_pt: 72.0,
            orientation: PageOrientation::Portrait,
            column_count: 1,
            column_gap_pt: 18.0,
        }
    }

    /// Standard US Letter portrait geometry (612.0 x 792.0 pt with 72pt margins).
    pub fn letter_portrait() -> Self {
        Self {
            width_pt: 612.0,
            height_pt: 792.0,
            margin_top_pt: 72.0,
            margin_bottom_pt: 72.0,
            margin_left_pt: 72.0,
            margin_right_pt: 72.0,
            orientation: PageOrientation::Portrait,
            column_count: 1,
            column_gap_pt: 18.0,
        }
    }

    /// Validates geometric dimensions, positive printable bounds, and column configuration.
    pub fn validate(&self, section_id: &str) -> Result<(), FlowError> {
        if !self.width_pt.is_finite() || self.width_pt <= 0.0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: format!("page width must be finite and > 0, got {}", self.width_pt),
            });
        }
        if !self.height_pt.is_finite() || self.height_pt <= 0.0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: format!("page height must be finite and > 0, got {}", self.height_pt),
            });
        }
        if !self.margin_top_pt.is_finite()
            || self.margin_top_pt < 0.0
            || !self.margin_bottom_pt.is_finite()
            || self.margin_bottom_pt < 0.0
            || !self.margin_left_pt.is_finite()
            || self.margin_left_pt < 0.0
            || !self.margin_right_pt.is_finite()
            || self.margin_right_pt < 0.0
        {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: "margins must be finite and non-negative".to_string(),
            });
        }
        let printable_width = self.width_pt - (self.margin_left_pt + self.margin_right_pt);
        if printable_width <= 0.0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: format!(
                    "horizontal margins ({:.1}pt + {:.1}pt) equal or exceed page width ({:.1}pt)",
                    self.margin_left_pt, self.margin_right_pt, self.width_pt
                ),
            });
        }
        let printable_height = self.height_pt - (self.margin_top_pt + self.margin_bottom_pt);
        if printable_height <= 0.0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: format!(
                    "vertical margins ({:.1}pt + {:.1}pt) equal or exceed page height ({:.1}pt)",
                    self.margin_top_pt, self.margin_bottom_pt, self.height_pt
                ),
            });
        }
        if self.column_count == 0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: "column count must be at least 1".to_string(),
            });
        }
        if !self.column_gap_pt.is_finite() || self.column_gap_pt < 0.0 {
            return Err(FlowError::InvalidGeometry {
                section_id: section_id.to_string(),
                details: "column gap must be finite and non-negative".to_string(),
            });
        }
        if self.column_count > 1 {
            let total_gaps = (self.column_count - 1) as f32 * self.column_gap_pt;
            if total_gaps >= printable_width {
                return Err(FlowError::InvalidGeometry {
                    section_id: section_id.to_string(),
                    details: format!(
                        "total column gaps ({:.1}pt) equal or exceed printable width ({:.1}pt)",
                        total_gaps, printable_width
                    ),
                });
            }
        }
        Ok(())
    }
}

/// A paragraph block containing formatted inline spans.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphBlock {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    pub spans: Vec<TextSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Alignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_before_pt: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_after_pt: Option<f32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl ParagraphBlock {
    pub fn new(id: impl Into<String>, spans: Vec<TextSpan>) -> Self {
        Self {
            id: id.into(),
            style_id: None,
            spans,
            alignment: None,
            space_before_pt: None,
            space_after_pt: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_style(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }
}

/// A structured heading block (levels 1 through 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadingBlock {
    pub id: String,
    pub level: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    pub spans: Vec<TextSpan>,
    #[serde(default)]
    pub numbered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_prefix: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl HeadingBlock {
    pub fn new(id: impl Into<String>, level: u8, spans: Vec<TextSpan>) -> Self {
        Self {
            id: id.into(),
            level,
            style_id: None,
            spans,
            numbered: false,
            number_prefix: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_style(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }
}

/// List kind specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ListKind {
    #[default]
    Unordered,
    Ordered,
    Task,
}

/// An individual item in a list with optional checkbox state and nested sub-lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListItem {
    pub id: String,
    pub spans: Vec<TextSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nested_list: Option<Box<ListBlock>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl ListItem {
    pub fn new(id: impl Into<String>, spans: Vec<TextSpan>) -> Self {
        Self {
            id: id.into(),
            spans,
            checked: None,
            nested_list: None,
            extra: BTreeMap::new(),
        }
    }
}

/// A list block containing one or more items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListBlock {
    pub id: String,
    pub kind: ListKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    pub items: Vec<ListItem>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl ListBlock {
    pub fn new(id: impl Into<String>, kind: ListKind, items: Vec<ListItem>) -> Self {
        Self {
            id: id.into(),
            kind,
            style_id: None,
            items,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_style(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }
}

/// A cell inside a table row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCell {
    pub id: String,
    pub blocks: Vec<Block>,
    #[serde(default = "default_span_one")]
    pub col_span: u32,
    #[serde(default = "default_span_one")]
    pub row_span: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<ColorRgba>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn default_span_one() -> u32 {
    1
}

impl TableCell {
    pub fn new(id: impl Into<String>, blocks: Vec<Block>) -> Self {
        Self {
            id: id.into(),
            blocks,
            col_span: 1,
            row_span: 1,
            background_color: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_col_span(mut self, col_span: u32) -> Self {
        self.col_span = col_span;
        self
    }

    pub fn with_row_span(mut self, row_span: u32) -> Self {
        self.row_span = row_span;
        self
    }

    pub fn with_background_color(mut self, color: ColorRgba) -> Self {
        self.background_color = Some(color);
        self
    }
}

/// A table row containing table cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    #[serde(default)]
    pub is_header: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl TableRow {
    pub fn new(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            is_header: false,
            extra: BTreeMap::new(),
        }
    }

    pub fn header(cells: Vec<TableCell>) -> Self {
        Self {
            cells,
            is_header: true,
            extra: BTreeMap::new(),
        }
    }
}

/// A tabular data block with optional caption and column widths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableBlock {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    pub rows: Vec<TableRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_widths_pt: Vec<f32>,
    #[serde(default)]
    pub repeat_header_rows: u32,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl TableBlock {
    pub fn new(id: impl Into<String>, rows: Vec<TableRow>) -> Self {
        Self {
            id: id.into(),
            style_id: None,
            caption: None,
            rows,
            column_widths_pt: Vec::new(),
            repeat_header_rows: 0,
            extra: BTreeMap::new(),
        }
    }

    pub fn with_style(mut self, style_id: impl Into<String>) -> Self {
        self.style_id = Some(style_id.into());
        self
    }

    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn with_column_widths(mut self, widths: Vec<f32>) -> Self {
        self.column_widths_pt = widths;
        self
    }

    pub fn with_repeat_header_rows(mut self, rows: u32) -> Self {
        self.repeat_header_rows = rows;
        self
    }
}

/// An embedded image reference block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBlock {
    pub id: String,
    pub asset_ref: String,
    pub alt_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    pub intrinsic_width_pt: f32,
    pub intrinsic_height_pt: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_width_pt: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_height_pt: Option<f32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Visual category for callout blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum CalloutType {
    #[default]
    Note,
    Tip,
    Warning,
    Caution,
    LegalNotice,
}

/// A highlighted callout or advisory panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalloutBlock {
    pub id: String,
    pub callout_type: CalloutType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub blocks: Vec<Block>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Horizontal divider block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThematicBreakBlock {
    pub id: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Unified enumeration over all document block types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Block {
    Paragraph(ParagraphBlock),
    Heading(HeadingBlock),
    List(ListBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Callout(CalloutBlock),
    ThematicBreak(ThematicBreakBlock),
}

impl Block {
    pub fn id(&self) -> &str {
        match self {
            Self::Paragraph(b) => &b.id,
            Self::Heading(b) => &b.id,
            Self::List(b) => &b.id,
            Self::Table(b) => &b.id,
            Self::Image(b) => &b.id,
            Self::Callout(b) => &b.id,
            Self::ThematicBreak(b) => &b.id,
        }
    }
}

/// A document section with geometry, headers/footers, and flow blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub geometry: SectionGeometry,
    pub blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_blocks: Vec<Block>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub footer_blocks: Vec<Block>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Section {
    pub fn new(id: impl Into<String>, geometry: SectionGeometry, blocks: Vec<Block>) -> Self {
        Self {
            id: id.into(),
            geometry,
            blocks,
            header_blocks: Vec::new(),
            footer_blocks: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// Footnote definition anchored at the document level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FootnoteDefinition {
    pub id: String,
    pub number: u32,
    pub blocks: Vec<Block>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl FootnoteDefinition {
    pub fn new(id: impl Into<String>, number: u32, blocks: Vec<Block>) -> Self {
        Self {
            id: id.into(),
            number,
            blocks,
            extra: BTreeMap::new(),
        }
    }
}

/// Root reflowable document container (`ADR-0020`, `AUTHOR-001`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowDocument {
    /// Schema format specification version (e.g. "1.0").
    pub schema_version: String,

    /// Dublin Core and document metadata.
    pub metadata: DocumentMetadata,

    /// Reusable stylesheet catalog.
    pub stylesheet: Stylesheet,

    /// Sequential document sections.
    pub sections: Vec<Section>,

    /// Global footnote definitions keyed by footnote identifier.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub footnotes: BTreeMap<String, FootnoteDefinition>,

    /// Unknown / forward-compatible attributes preserved intact during roundtrips.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl FlowDocument {
    pub fn new(metadata: DocumentMetadata) -> Self {
        Self {
            schema_version: "1.0".to_string(),
            metadata,
            stylesheet: Stylesheet::standard_legal(),
            sections: Vec::new(),
            footnotes: BTreeMap::new(),
            extra: BTreeMap::new(),
        }
    }

    pub fn with_section(mut self, section: Section) -> Self {
        self.sections.push(section);
        self
    }
}

macro_rules! impl_from_block {
    ($($variant:ident($type:ident)),* $(,)?) => {
        $(
            impl From<$type> for Block {
                fn from(b: $type) -> Self {
                    Self::$variant(b)
                }
            }
        )*
    };
}

impl_from_block! {
    Paragraph(ParagraphBlock),
    Heading(HeadingBlock),
    List(ListBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Callout(CalloutBlock),
    ThematicBreak(ThematicBreakBlock),
}

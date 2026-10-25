//! Typography, color models, inline spans, and stylesheet definitions (`ADR-0020`, `WRITE-001`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::FlowError;

/// 32-bit RGBA color representation with serialization helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` hex strings.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::rgba(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Formats as `#RRGGBBAA` string.
    pub fn to_hex_string(&self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }
}

/// Inline font face styling flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub struct FontStyle {
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strikethrough: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub code: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub subscript: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub superscript: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl FontStyle {
    pub const PLAIN: Self = Self {
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        code: false,
        subscript: false,
        superscript: false,
    };

    pub fn bold() -> Self {
        Self {
            bold: true,
            ..Self::PLAIN
        }
    }

    pub fn italic() -> Self {
        Self {
            italic: true,
            ..Self::PLAIN
        }
    }
}

/// An inline text run with granular typography and optional hyperlinks/footnotes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    /// Text content of this span.
    pub text: String,

    /// Typography flags (bold, italic, etc.).
    #[serde(default, skip_serializing_if = "is_plain_font_style")]
    pub style: FontStyle,

    /// Point size override for this span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f32>,

    /// Font family override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,

    /// OpenType font weight (e.g. 400 for regular, 700 for bold).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,

    /// Text foreground color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRgba>,

    /// Text background / highlight color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<ColorRgba>,

    /// Hyperlink destination URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_url: Option<String>,

    /// ID of an associated footnote in the document's footnotes dictionary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footnote_ref: Option<String>,

    /// Unknown / forward-compatible span attributes.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn is_plain_font_style(style: &FontStyle) -> bool {
    *style == FontStyle::PLAIN
}

impl TextSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: FontStyle::PLAIN,
            font_size_pt: None,
            font_family: None,
            font_weight: None,
            color: None,
            background_color: None,
            link_url: None,
            footnote_ref: None,
            extra: BTreeMap::new(),
        }
    }

    pub fn bold(text: impl Into<String>) -> Self {
        Self {
            style: FontStyle::bold(),
            ..Self::plain(text)
        }
    }

    pub fn italic(text: impl Into<String>) -> Self {
        Self {
            style: FontStyle::italic(),
            ..Self::plain(text)
        }
    }

    pub fn with_link(mut self, url: impl Into<String>) -> Self {
        self.link_url = Some(url.into());
        self
    }

    pub fn with_footnote(mut self, footnote_id: impl Into<String>) -> Self {
        self.footnote_ref = Some(footnote_id.into());
        self
    }

    /// Validates inline typographic attributes.
    pub fn validate(&self, context: &str) -> Result<(), FlowError> {
        if let Some(size) = self.font_size_pt
            && (!size.is_finite() || size <= 0.0)
        {
            return Err(FlowError::InvalidTypography {
                context: context.to_string(),
                details: format!("span font_size_pt must be finite and > 0, got {}", size),
            });
        }
        if let Some(weight) = self.font_weight
            && !(1..=1000).contains(&weight)
        {
            return Err(FlowError::InvalidTypography {
                context: context.to_string(),
                details: format!(
                    "span font_weight must be between 1 and 1000, got {}",
                    weight
                ),
            });
        }
        Ok(())
    }
}

/// Paragraph text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}

/// Paragraph typography and spacing style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphStyle {
    pub id: String,
    pub name: String,
    pub font_family: String,
    pub font_size_pt: f32,
    #[serde(default = "default_line_spacing")]
    pub line_spacing_mult: f32,
    #[serde(default)]
    pub space_before_pt: f32,
    #[serde(default)]
    pub space_after_pt: f32,
    #[serde(default)]
    pub alignment: Alignment,
    #[serde(default)]
    pub first_line_indent_pt: f32,
    #[serde(default)]
    pub keep_with_next: bool,
    #[serde(default)]
    pub keep_lines_together: bool,
    #[serde(default = "default_true")]
    pub orphan_control: bool,
}

fn default_line_spacing() -> f32 {
    1.15
}

fn default_true() -> bool {
    true
}

impl ParagraphStyle {
    pub fn validate(&self) -> Result<(), FlowError> {
        if self.id.trim().is_empty() {
            return Err(FlowError::EmptyStyleId {
                style_type: "paragraph".to_string(),
            });
        }
        if !self.font_size_pt.is_finite() || self.font_size_pt <= 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("paragraph style '{}'", self.id),
                details: format!(
                    "font_size_pt must be finite and > 0, got {}",
                    self.font_size_pt
                ),
            });
        }
        if !self.line_spacing_mult.is_finite() || self.line_spacing_mult <= 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("paragraph style '{}'", self.id),
                details: format!(
                    "line_spacing_mult must be finite and > 0, got {}",
                    self.line_spacing_mult
                ),
            });
        }
        if !self.space_before_pt.is_finite() || self.space_before_pt < 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("paragraph style '{}'", self.id),
                details: format!(
                    "space_before_pt must be finite and >= 0, got {}",
                    self.space_before_pt
                ),
            });
        }
        if !self.space_after_pt.is_finite() || self.space_after_pt < 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("paragraph style '{}'", self.id),
                details: format!(
                    "space_after_pt must be finite and >= 0, got {}",
                    self.space_after_pt
                ),
            });
        }
        if !self.first_line_indent_pt.is_finite() {
            return Err(FlowError::InvalidTypography {
                context: format!("paragraph style '{}'", self.id),
                details: "first_line_indent_pt must be finite".to_string(),
            });
        }
        Ok(())
    }
}

/// Reusable character styling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterStyle {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_pt: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(default)]
    pub font_style: FontStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorRgba>,
}

/// Table formatting and border style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableStyle {
    pub id: String,
    pub name: String,
    #[serde(default = "default_border_width")]
    pub border_width_pt: f32,
    #[serde(default = "default_border_color")]
    pub border_color: ColorRgba,
    #[serde(default = "default_cell_padding")]
    pub cell_padding_pt: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_background_color: Option<ColorRgba>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternating_row_color: Option<ColorRgba>,
}

fn default_border_width() -> f32 {
    0.5
}

fn default_border_color() -> ColorRgba {
    ColorRgba::rgb(200, 200, 200)
}

fn default_cell_padding() -> f32 {
    4.0
}

/// List numbering and indentation style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListStyle {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bullet_char: Option<char>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(default = "default_list_indent")]
    pub indent_pt: f32,
}

fn default_list_indent() -> f32 {
    18.0
}

impl CharacterStyle {
    pub fn validate(&self) -> Result<(), FlowError> {
        if self.id.trim().is_empty() {
            return Err(FlowError::EmptyStyleId {
                style_type: "character".to_string(),
            });
        }
        if let Some(size) = self.font_size_pt
            && (!size.is_finite() || size <= 0.0)
        {
            return Err(FlowError::InvalidTypography {
                context: format!("character style '{}'", self.id),
                details: format!("font_size_pt must be finite and > 0, got {}", size),
            });
        }
        if let Some(weight) = self.font_weight
            && !(1..=1000).contains(&weight)
        {
            return Err(FlowError::InvalidTypography {
                context: format!("character style '{}'", self.id),
                details: format!("font_weight must be between 1 and 1000, got {}", weight),
            });
        }
        Ok(())
    }
}

impl TableStyle {
    pub fn validate(&self) -> Result<(), FlowError> {
        if self.id.trim().is_empty() {
            return Err(FlowError::EmptyStyleId {
                style_type: "table".to_string(),
            });
        }
        if !self.border_width_pt.is_finite() || self.border_width_pt < 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("table style '{}'", self.id),
                details: format!(
                    "border_width_pt must be finite and >= 0, got {}",
                    self.border_width_pt
                ),
            });
        }
        if !self.cell_padding_pt.is_finite() || self.cell_padding_pt < 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("table style '{}'", self.id),
                details: format!(
                    "cell_padding_pt must be finite and >= 0, got {}",
                    self.cell_padding_pt
                ),
            });
        }
        Ok(())
    }
}

impl ListStyle {
    pub fn validate(&self) -> Result<(), FlowError> {
        if self.id.trim().is_empty() {
            return Err(FlowError::EmptyStyleId {
                style_type: "list".to_string(),
            });
        }
        if !self.indent_pt.is_finite() || self.indent_pt < 0.0 {
            return Err(FlowError::InvalidTypography {
                context: format!("list style '{}'", self.id),
                details: format!("indent_pt must be finite and >= 0, got {}", self.indent_pt),
            });
        }
        Ok(())
    }
}

/// Document-level style catalog.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Stylesheet {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub paragraph_styles: BTreeMap<String, ParagraphStyle>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub character_styles: BTreeMap<String, CharacterStyle>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub table_styles: BTreeMap<String, TableStyle>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub list_styles: BTreeMap<String, ListStyle>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Stylesheet {
    /// Validates internal consistency and all styles in the catalog.
    pub fn validate(&self) -> Result<(), FlowError> {
        for (key, style) in &self.paragraph_styles {
            if key != &style.id {
                return Err(FlowError::InvalidTypography {
                    context: "stylesheet paragraph_styles".to_string(),
                    details: format!("key '{}' does not match style id '{}'", key, style.id),
                });
            }
            style.validate()?;
        }
        for (key, style) in &self.character_styles {
            if key != &style.id {
                return Err(FlowError::InvalidTypography {
                    context: "stylesheet character_styles".to_string(),
                    details: format!("key '{}' does not match style id '{}'", key, style.id),
                });
            }
            style.validate()?;
        }
        for (key, style) in &self.table_styles {
            if key != &style.id {
                return Err(FlowError::InvalidTypography {
                    context: "stylesheet table_styles".to_string(),
                    details: format!("key '{}' does not match style id '{}'", key, style.id),
                });
            }
            style.validate()?;
        }
        for (key, style) in &self.list_styles {
            if key != &style.id {
                return Err(FlowError::InvalidTypography {
                    context: "stylesheet list_styles".to_string(),
                    details: format!("key '{}' does not match style id '{}'", key, style.id),
                });
            }
            style.validate()?;
        }
        Ok(())
    }

    /// Creates a calibrated standard legal stylesheet.
    pub fn standard_legal() -> Self {
        let mut stylesheet = Self::default();

        stylesheet.paragraph_styles.insert(
            "Normal".to_string(),
            ParagraphStyle {
                id: "Normal".to_string(),
                name: "Normal".to_string(),
                font_family: "Inter".to_string(),
                font_size_pt: 11.0,
                line_spacing_mult: 1.2,
                space_before_pt: 0.0,
                space_after_pt: 6.0,
                alignment: Alignment::Left,
                first_line_indent_pt: 0.0,
                keep_with_next: false,
                keep_lines_together: false,
                orphan_control: true,
            },
        );

        let mk_heading =
            |id: &str, name: &str, size: f32, before: f32, after: f32| ParagraphStyle {
                id: id.to_string(),
                name: name.to_string(),
                font_family: "Inter".to_string(),
                font_size_pt: size,
                line_spacing_mult: 1.15,
                space_before_pt: before,
                space_after_pt: after,
                alignment: Alignment::Left,
                first_line_indent_pt: 0.0,
                keep_with_next: true,
                keep_lines_together: true,
                orphan_control: true,
            };

        stylesheet.paragraph_styles.insert(
            "Heading1".to_string(),
            mk_heading("Heading1", "Heading 1", 20.0, 14.0, 6.0),
        );
        stylesheet.paragraph_styles.insert(
            "Heading2".to_string(),
            mk_heading("Heading2", "Heading 2", 16.0, 12.0, 4.0),
        );

        stylesheet.table_styles.insert(
            "DefaultTable".to_string(),
            TableStyle {
                id: "DefaultTable".to_string(),
                name: "Default Table".to_string(),
                border_width_pt: 0.75,
                border_color: ColorRgba::rgb(180, 180, 180),
                cell_padding_pt: 6.0,
                header_background_color: Some(ColorRgba::rgb(240, 240, 245)),
                alternating_row_color: Some(ColorRgba::rgb(250, 250, 252)),
            },
        );

        stylesheet.list_styles.insert(
            "Bullet".to_string(),
            ListStyle {
                id: "Bullet".to_string(),
                name: "Standard Bullet".to_string(),
                bullet_char: Some('•'),
                number_format: None,
                indent_pt: 18.0,
            },
        );

        stylesheet
    }
}

//! Typography, color models, inline spans, and stylesheet definitions (`ADR-0020`, `WRITE-001`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

        stylesheet.paragraph_styles.insert(
            "Heading1".to_string(),
            ParagraphStyle {
                id: "Heading1".to_string(),
                name: "Heading 1".to_string(),
                font_family: "Inter".to_string(),
                font_size_pt: 20.0,
                line_spacing_mult: 1.1,
                space_before_pt: 14.0,
                space_after_pt: 6.0,
                alignment: Alignment::Left,
                first_line_indent_pt: 0.0,
                keep_with_next: true,
                keep_lines_together: true,
                orphan_control: true,
            },
        );

        stylesheet.paragraph_styles.insert(
            "Heading2".to_string(),
            ParagraphStyle {
                id: "Heading2".to_string(),
                name: "Heading 2".to_string(),
                font_family: "Inter".to_string(),
                font_size_pt: 16.0,
                line_spacing_mult: 1.15,
                space_before_pt: 12.0,
                space_after_pt: 4.0,
                alignment: Alignment::Left,
                first_line_indent_pt: 0.0,
                keep_with_next: true,
                keep_lines_together: true,
                orphan_control: true,
            },
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

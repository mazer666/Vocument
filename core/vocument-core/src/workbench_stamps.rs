//! Types and models for M3.4 Bates Numbering, Headers/Footers, Bookmarks, and Stamps/Watermarks (`ROADMAP.md` M3.4).

use crate::pdf_engine::PdfRectMilliPt;
use serde::{Deserialize, Serialize};

/// Standard page positions for Bates numbering stamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatesPosition {
    TopLeft = 0,
    TopCenter = 1,
    TopRight = 2,
    BottomLeft = 3,
    BottomCenter = 4,
    BottomRight = 5,
}

/// Configuration options for legal Bates numbering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatesNumberingOptions {
    pub prefix: String,
    pub start_number: u32,
    pub digit_padding: u32,
    pub suffix: String,
    pub position: BatesPosition,
    pub font_size_pt: u32,
    pub margin_pt: u32,
}

impl Default for BatesNumberingOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            start_number: 1,
            digit_padding: 6,
            suffix: String::new(),
            position: BatesPosition::BottomRight,
            font_size_pt: 10,
            margin_pt: 36,
        }
    }
}

/// Configuration options for multi-zone headers and footers with dynamic macros.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HeaderFooterOptions {
    pub header_left: Option<String>,
    pub header_center: Option<String>,
    pub header_right: Option<String>,
    pub footer_left: Option<String>,
    pub footer_center: Option<String>,
    pub footer_right: Option<String>,
    pub font_size_pt: u32,
    pub margin_pt: u32,
}

/// Hierarchical bookmark (outline item) node with title, destination page, and recursive children.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub title: String,
    pub target_page: u32,
    pub children: Vec<BookmarkItem>,
}

/// Configuration options for rubber stamps and watermarks (`WATER-001`–`003`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StampOrWatermarkOptions {
    pub text: String,
    pub opacity_milli: u32,
    pub rotation_degrees: i32,
    pub color_rgb: [u8; 3],
    pub is_background: bool,
    pub rect: Option<PdfRectMilliPt>,
}

impl Default for StampOrWatermarkOptions {
    fn default() -> Self {
        Self {
            text: "CONFIDENTIAL".to_owned(),
            opacity_milli: 500,
            rotation_degrees: -45,
            color_rgb: [220, 20, 20],
            is_background: false,
            rect: None,
        }
    }
}

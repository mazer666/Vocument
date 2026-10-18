//! Domain models for OCR results, bounding boxes, text hierarchy, and options (`ARCHITECTURE.md` §20).

use serde::{Deserialize, Serialize};

/// 2D bounding box representing rectangular text extents in either pixel coordinates
/// or normalized coordinates ($[0.0, 1.0]$) relative to document dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BoundingBox {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }

    /// Converts pixel coordinates to normalized $[0.0, 1.0]$ bounds.
    pub fn to_normalized(&self, width: u32, height: u32) -> Self {
        if width == 0 || height == 0 {
            return *self;
        }
        let w = width as f32;
        let h = height as f32;
        Self {
            x0: (self.x0 / w).clamp(0.0, 1.0),
            y0: (self.y0 / h).clamp(0.0, 1.0),
            x1: (self.x1 / w).clamp(0.0, 1.0),
            y1: (self.y1 / h).clamp(0.0, 1.0),
        }
    }

    /// Converts normalized $[0.0, 1.0]$ bounds to integer pixel coordinates.
    pub fn to_pixel(&self, width: u32, height: u32) -> Self {
        let w = width as f32;
        let h = height as f32;
        Self {
            x0: (self.x0 * w).max(0.0),
            y0: (self.y0 * h).max(0.0),
            x1: (self.x1 * w).min(w),
            y1: (self.y1 * h).min(h),
        }
    }

    /// Width of the bounding box.
    pub fn width(&self) -> f32 {
        (self.x1 - self.x0).max(0.0)
    }

    /// Height of the bounding box.
    pub fn height(&self) -> f32 {
        (self.y1 - self.y0).max(0.0)
    }

    /// Area enclosed by the bounding box.
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Validates that bounds are non-inverted.
    pub fn is_valid(&self) -> bool {
        self.x1 > self.x0 && self.y1 > self.y0
    }

    /// Returns whether this bounding box intersects another.
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.x0 < other.x1 && self.x1 > other.x0 && self.y0 < other.y1 && self.y1 > other.y0
    }
}

/// A recognized single character glyph with individual spatial bounding box and confidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrCharacter {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
}

/// A recognized word composed of character glyphs, bounds, and confidence score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrWord {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub characters: Vec<OcrCharacter>,
    pub block_index: u32,
    pub line_index: u32,
    pub word_index: u32,
}

/// A line of words forming a coherent typographical text line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
    pub words: Vec<OcrWord>,
    pub line_index: u32,
}

/// Typographical block type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrBlockType {
    Text,
    Title,
    Heading,
    Header,
    Footer,
}

/// A structural block (paragraph, heading, or title) containing lines of text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrBlock {
    pub block_type: OcrBlockType,
    pub bbox: BoundingBox,
    pub lines: Vec<OcrLine>,
    pub block_index: u32,
}

/// Complete OCR result for a processed scan page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrPageResult {
    pub page_index: u32,
    pub full_text: String,
    pub blocks: Vec<OcrBlock>,
    pub lines: Vec<OcrLine>,
    pub words: Vec<OcrWord>,
    pub mean_confidence: f32,
    pub detected_language: String,
    pub hocr_output: Option<String>,
    pub frame_width: u32,
    pub frame_height: u32,
}

/// Execution options passed to `OcrProvider`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrOptions {
    pub languages: Vec<String>,
    pub tessdata_path: Option<std::path::PathBuf>,
    pub detect_language: bool,
    pub generate_hocr: bool,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            languages: vec!["deu".into(), "eng".into()],
            tessdata_path: None,
            detect_language: true,
            generate_hocr: true,
        }
    }
}

//! Page boundary calibration, crop modes, and deskew parameters (`ROADMAP.md` M3.3, `CROP-001`–`CROP-003`, `ROTATE-001`–`ROTATE-003`).

use crate::pdf_engine::PdfRectMilliPt;
use serde::{Deserialize, Serialize};

/// The 5 standard ISO 32000-1 page boundaries and rotation (`CROP-002`).
/// All box coordinates are in milli-points (1/1000 pt) with top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageBoxes {
    pub media_box: PdfRectMilliPt,
    pub crop_box: PdfRectMilliPt,
    pub bleed_box: PdfRectMilliPt,
    pub trim_box: PdfRectMilliPt,
    pub art_box: PdfRectMilliPt,
    pub rotation_degrees: i32,
}

/// Partial or complete update of page boundary boxes (`CROP-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PageBoxUpdate {
    pub media_box: Option<PdfRectMilliPt>,
    pub crop_box: Option<PdfRectMilliPt>,
    pub bleed_box: Option<PdfRectMilliPt>,
    pub trim_box: Option<PdfRectMilliPt>,
    pub art_box: Option<PdfRectMilliPt>,
}

/// Mode of cropping applied to one or more pages (`CROP-001`, `CROP-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CropMode {
    /// Non-destructive display crop by adjusting /CropBox dictionary entry.
    DisplayCrop,
    /// Destructive hard crop by clamping MediaBox/CropBox, clipping content stream, and pruning out-of-bounds annotations.
    HardCrop,
}

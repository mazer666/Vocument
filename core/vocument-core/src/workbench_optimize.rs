//! Domain models and types for document optimization, image extraction, and print preflight (`ROADMAP.md` M3.5).

use serde::{Deserialize, Serialize};

/// Predefined optimization strategies for document compression (`ROADMAP.md` M3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationPreset {
    /// Compaction into object streams and unreferenced object removal without loss of data.
    Lossless = 0,
    /// Balanced compression with object deduplication and stream deflating.
    Balanced = 1,
    /// Aggressive recompression, deduplication, stream cleaning, and sanitization.
    Maximum = 2,
}

/// Options controlling document optimization and compression (`M3.5`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptimizationOptions {
    pub preset: OptimizationPreset,
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        Self {
            preset: OptimizationPreset::Balanced,
        }
    }
}

/// Outcome summary of a document optimization operation (`M3.5`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub original_size_bytes: usize,
    pub optimized_size_bytes: usize,
    pub bytes_saved: i64,
    pub compression_ratio: f64,
}

/// Metadata description of an embedded raster image discovered in a document (`M3.5`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedImageInfo {
    pub page_index: u32,
    pub image_index: u32,
    pub obj_num: u32,
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub format: String,
    pub colorspace: String,
}

/// Full extracted image payload (`M3.5`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedImageData {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub is_raw_stream: bool,
    pub data: Vec<u8>,
}

/// Restricted structural preflight report for PDF/X (ISO 15930) verification (`ROADMAP.md` M3.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfxPreflightReport {
    pub is_valid: bool,
    pub is_restricted_preflight: bool,
    pub disclaimer: String,
    pub version: String,
    pub has_output_intent: bool,
    pub output_intent_profile: String,
    pub has_trapped_key: bool,
    pub has_bleed_or_trim_box: bool,
    pub is_encrypted: bool,
    pub unembedded_fonts: Vec<String>,
    pub issues: Vec<String>,
}

/// Preflight readiness report for professional printing (`PRINT-003`, `PRINT-004`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintPreflightReport {
    pub page_count: u32,
    pub is_uniform_size: bool,
    pub width_pt: f32,
    pub height_pt: f32,
    pub has_bleed_box: bool,
    pub has_crop_box: bool,
    pub warnings: Vec<String>,
}

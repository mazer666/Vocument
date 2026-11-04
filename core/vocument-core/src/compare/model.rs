//! Domain models and configurations for Document Comparison (`M3.7`, `COMPARE-001`–`COMPARE-004`).

use serde::{Deserialize, Serialize};

use crate::pdf_engine::PdfRectMilliPt;

/// Granularity of text diffing (`COMPARE-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiffGranularity {
    #[default]
    Word,
    Character,
    Line,
}

/// Nature of a detected difference between two document states (`COMPARE-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffOperation {
    Unchanged,
    Insert,
    Delete,
    Replace,
    Move { from_index: usize, to_index: usize },
}

/// Options controlling structural and visual document comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonOptions {
    /// Level of text tokenization (Word, Character, Line).
    pub granularity: DiffGranularity,
    /// When true, ignores whitespace-only differences between tokens.
    pub ignore_whitespace: bool,
    /// When true, performs case-insensitive text matching.
    pub ignore_case: bool,
    /// Color distance threshold for visual pixel delta (0..255, default 16).
    pub visual_pixel_threshold: u8,
    /// Distance in points to cluster adjacent pixel differences into bounding boxes.
    pub visual_cluster_distance_pt: f32,
    /// When true, attempts semantic legal clause / heading alignment (`COMPARE-004`).
    pub align_clauses: bool,
}

impl Default for ComparisonOptions {
    fn default() -> Self {
        Self {
            granularity: DiffGranularity::Word,
            ignore_whitespace: true,
            ignore_case: false,
            visual_pixel_threshold: 16,
            visual_cluster_distance_pt: 10.0,
            align_clauses: true,
        }
    }
}

/// An individual change item detected between Document A and Document B (`COMPARE-001`, `COMPARE-002`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeItem {
    pub id: String,
    pub operation: DiffOperation,
    pub page_a: Option<u32>,
    pub page_b: Option<u32>,
    pub text_a: Option<String>,
    pub text_b: Option<String>,
    pub rect_a: Option<PdfRectMilliPt>,
    pub rect_b: Option<PdfRectMilliPt>,
    pub clause_hint: Option<String>,
}

/// Result of visual bitmap comparison between two pages (`COMPARE-003`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageDiffResult {
    pub page_a: u32,
    pub page_b: u32,
    pub pixel_difference_percentage: f32,
    pub changed_pixel_count: u32,
    pub total_pixel_count: u32,
    pub difference_bounding_boxes: Vec<PdfRectMilliPt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_mask_png: Option<Vec<u8>>,
}

/// Alignment mapping between pages of Document A and Document B (`COMPARE-002`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageAlignment {
    pub page_a: Option<u32>,
    pub page_b: Option<u32>,
    pub similarity_score: f32,
}

/// High-level metrics summarizing differences between two documents (`COMPARE-002`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_changes: u32,
    pub insertions_count: u32,
    pub deletions_count: u32,
    pub replacements_count: u32,
    pub moves_count: u32,
    pub similarity_ratio: f32,
    pub identical_pages_count: u32,
    pub modified_pages_count: u32,
    pub inserted_pages_count: u32,
    pub deleted_pages_count: u32,
}

/// Full exportable comparison report (`COMPARE-001`, `COMPARE-002`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub doc_a_title: String,
    pub doc_b_title: String,
    pub page_count_a: u32,
    pub page_count_b: u32,
    pub summary: ComparisonSummary,
    pub page_alignments: Vec<PageAlignment>,
    pub page_diffs: Vec<PageDiffResult>,
    pub changes: Vec<ChangeItem>,
}

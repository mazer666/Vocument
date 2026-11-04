//! Document Comparison & Diffing Engine (`M3.7`, `COMPARE-001`–`COMPARE-004`).
//!
//! Provides structural text diffing, visual page comparison, global sequence alignment,
//! and multi-format report exports for PDF and flowable documents.

pub mod aligner;
pub mod model;
pub mod report;
pub mod text_diff;
pub mod visual_diff;

use thiserror::Error;

use crate::pdf_engine::{PdfEngineDocument, PdfEngineError, PdfTileRenderRequest};

pub use aligner::align_pages;
pub use model::{
    ChangeItem, ComparisonOptions, ComparisonReport, ComparisonSummary, DiffGranularity,
    DiffOperation, PageAlignment, PageDiffResult,
};
pub use text_diff::{DiffToken, diff_tokens, tokenize};
pub use visual_diff::compare_page_bitmaps;

/// Errors occurring during document comparison operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompareError {
    #[error("PDF engine error during comparison: {0}")]
    Engine(#[from] PdfEngineError),
    #[error("invalid page index {page_index} (document page count: {page_count})")]
    InvalidPageIndex { page_index: u32, page_count: u32 },
    #[error("internal comparison failure: {details}")]
    Failure { details: String },
}

/// Compares two raw text streams and generates a complete `ComparisonReport`.
pub fn compare_text_streams(
    title_a: &str,
    text_a: &str,
    title_b: &str,
    text_b: &str,
    options: &ComparisonOptions,
) -> ComparisonReport {
    let tokens_a = tokenize(text_a, Some(0), options);
    let tokens_b = tokenize(text_b, Some(0), options);
    let changes = diff_tokens(&tokens_a, &tokens_b, options);

    let mut insertions = 0u32;
    let mut deletions = 0u32;
    let mut replacements = 0u32;
    let mut moves = 0u32;

    for item in &changes {
        match item.operation {
            DiffOperation::Insert => insertions += 1,
            DiffOperation::Delete => deletions += 1,
            DiffOperation::Replace => replacements += 1,
            DiffOperation::Move { .. } => moves += 1,
            DiffOperation::Unchanged => {}
        }
    }

    let total_changes = insertions + deletions + replacements + moves / 2;
    let total_tokens = (tokens_a.len() + tokens_b.len()).max(1) as f32;
    let changed_token_count = (insertions + deletions + replacements * 2) as f32;
    let similarity_ratio = (1.0 - (changed_token_count / total_tokens)).clamp(0.0, 1.0);

    let summary = ComparisonSummary {
        total_changes,
        insertions_count: insertions,
        deletions_count: deletions,
        replacements_count: replacements,
        moves_count: moves / 2,
        similarity_ratio,
        identical_pages_count: if total_changes == 0 { 1 } else { 0 },
        modified_pages_count: if total_changes > 0 { 1 } else { 0 },
        inserted_pages_count: 0,
        deleted_pages_count: 0,
    };

    ComparisonReport {
        doc_a_title: title_a.to_string(),
        doc_b_title: title_b.to_string(),
        page_count_a: 1,
        page_count_b: 1,
        summary,
        page_alignments: vec![PageAlignment {
            page_a: Some(0),
            page_b: Some(0),
            similarity_score: similarity_ratio,
        }],
        page_diffs: Vec::new(),
        changes,
    }
}

/// Performs comprehensive structural text and visual page comparison between two PDF documents.
pub fn compare_documents(
    doc_a: &dyn PdfEngineDocument,
    doc_b: &dyn PdfEngineDocument,
    options: &ComparisonOptions,
) -> Result<ComparisonReport, CompareError> {
    let page_count_a = doc_a.summary().page_count;
    let page_count_b = doc_b.summary().page_count;

    let title_a = doc_a
        .summary()
        .title
        .clone()
        .unwrap_or_else(|| "Document A".to_string());
    let title_b = doc_b
        .summary()
        .title
        .clone()
        .unwrap_or_else(|| "Document B".to_string());

    // 1. Extract text and tokens for all pages in both documents
    let mut pages_text_a = Vec::with_capacity(page_count_a as usize);
    let mut tokens_a = Vec::new();
    for p in 0..page_count_a {
        let structured = doc_a.extract_page_text(p)?;
        let mut page_str = String::new();
        for line in &structured.lines {
            page_str.push_str(&line.text);
            page_str.push('\n');
        }
        tokens_a.extend(tokenize(&page_str, Some(p), options));
        pages_text_a.push(page_str);
    }

    let mut pages_text_b = Vec::with_capacity(page_count_b as usize);
    let mut tokens_b = Vec::new();
    for p in 0..page_count_b {
        let structured = doc_b.extract_page_text(p)?;
        let mut page_str = String::new();
        for line in &structured.lines {
            page_str.push_str(&line.text);
            page_str.push('\n');
        }
        tokens_b.extend(tokenize(&page_str, Some(p), options));
        pages_text_b.push(page_str);
    }

    // 2. Global sequence alignment across pages
    let page_alignments = align_pages(&pages_text_a, &pages_text_b);

    // 3. Structural text diffing
    let changes = diff_tokens(&tokens_a, &tokens_b, options);

    // 4. Visual page comparison for aligned pairs
    let mut page_diffs = Vec::new();
    let mut identical_pages = 0u32;
    let mut modified_pages = 0u32;
    let mut inserted_pages = 0u32;
    let mut deleted_pages = 0u32;

    for align in &page_alignments {
        match (align.page_a, align.page_b) {
            (Some(pa), Some(pb)) => {
                let diff_res = compare_pages_visual(doc_a, pa, doc_b, pb, options)?;
                if diff_res.pixel_difference_percentage < 0.05 && align.similarity_score > 0.999 {
                    identical_pages += 1;
                } else {
                    modified_pages += 1;
                }
                page_diffs.push(diff_res);
            }
            (None, Some(_)) => {
                inserted_pages += 1;
            }
            (Some(_), None) => {
                deleted_pages += 1;
            }
            (None, None) => {}
        }
    }

    let mut insertions = 0u32;
    let mut deletions = 0u32;
    let mut replacements = 0u32;
    let mut moves = 0u32;

    for item in &changes {
        match item.operation {
            DiffOperation::Insert => insertions += 1,
            DiffOperation::Delete => deletions += 1,
            DiffOperation::Replace => replacements += 1,
            DiffOperation::Move { .. } => moves += 1,
            DiffOperation::Unchanged => {}
        }
    }

    let total_changes = insertions + deletions + replacements + moves / 2;
    let total_tokens = (tokens_a.len() + tokens_b.len()).max(1) as f32;
    let changed_token_count = (insertions + deletions + replacements * 2) as f32;
    let similarity_ratio = (1.0 - (changed_token_count / total_tokens)).clamp(0.0, 1.0);

    let summary = ComparisonSummary {
        total_changes,
        insertions_count: insertions,
        deletions_count: deletions,
        replacements_count: replacements,
        moves_count: moves / 2,
        similarity_ratio,
        identical_pages_count: identical_pages,
        modified_pages_count: modified_pages,
        inserted_pages_count: inserted_pages,
        deleted_pages_count: deleted_pages,
    };

    Ok(ComparisonReport {
        doc_a_title: title_a,
        doc_b_title: title_b,
        page_count_a,
        page_count_b,
        summary,
        page_alignments,
        page_diffs,
        changes,
    })
}

/// Compares a single page pair visually by rasterizing both pages and computing pixel deltas.
pub fn compare_pages_visual(
    doc_a: &dyn PdfEngineDocument,
    page_a: u32,
    doc_b: &dyn PdfEngineDocument,
    page_b: u32,
    options: &ComparisonOptions,
) -> Result<PageDiffResult, CompareError> {
    let geom_a = doc_a.page_geometry(page_a)?;
    let geom_b = doc_b.page_geometry(page_b)?;

    let pt_w = (geom_a.width_milli_pt as f32 / 1000.0).max(geom_b.width_milli_pt as f32 / 1000.0);
    let pt_h = (geom_a.height_milli_pt as f32 / 1000.0).max(geom_b.height_milli_pt as f32 / 1000.0);

    // Rasterize at 72 DPI for efficient responsive comparison
    let target_w = pt_w.round() as u32;
    let target_h = pt_h.round() as u32;

    let dl_a = doc_a.create_display_list(page_a)?;
    let dl_b = doc_b.create_display_list(page_b)?;

    let req_a = PdfTileRenderRequest {
        page_index: page_a,
        scale_milli: 1000,
        tile_x_px: 0,
        tile_y_px: 0,
        tile_width_px: target_w,
        tile_height_px: target_h,
    };
    let req_b = PdfTileRenderRequest {
        page_index: page_b,
        scale_milli: 1000,
        tile_x_px: 0,
        tile_y_px: 0,
        tile_width_px: target_w,
        tile_height_px: target_h,
    };

    let bmp_a = dl_a.render_tile(req_a)?;
    let bmp_b = dl_b.render_tile(req_b)?;

    let result = compare_page_bitmaps(
        page_a,
        page_b,
        &bmp_a.rgba_bytes,
        bmp_a.width_px,
        bmp_a.height_px,
        &bmp_b.rgba_bytes,
        bmp_b.width_px,
        bmp_b.height_px,
        pt_w,
        pt_h,
        options,
    );

    Ok(result)
}

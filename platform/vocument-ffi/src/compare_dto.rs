//! UniFFI types and exported bridge functions for M3.7 Document Comparison & Diffing (`COMPARE-001`–`004`).
//!
//! Exposes structural text diffing, visual page pixel delta, page sequence alignment,
//! and multi-format reports (JSON, Markdown, offline HTML) across the native boundary.

use vocument_core::compare::{
    ChangeItem, ComparisonOptions, ComparisonReport, ComparisonSummary, DiffGranularity,
    DiffOperation, PageAlignment, PageDiffResult, compare_text_streams,
};
use vocument_core::pdf_engine::PdfRectMilliPt;
use vocument_flow::model::{Block, FlowDocument};

use crate::editor_dto::registry;
use crate::{DocumentCoreError, FfiPdfRectMilliPt, session_registry};

/// Text diff granularity across tokens (`COMPARE-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiDiffGranularity {
    Word,
    Character,
    Line,
}

impl From<DiffGranularity> for FfiDiffGranularity {
    fn from(g: DiffGranularity) -> Self {
        match g {
            DiffGranularity::Word => Self::Word,
            DiffGranularity::Character => Self::Character,
            DiffGranularity::Line => Self::Line,
        }
    }
}

impl From<FfiDiffGranularity> for DiffGranularity {
    fn from(g: FfiDiffGranularity) -> Self {
        match g {
            FfiDiffGranularity::Word => Self::Word,
            FfiDiffGranularity::Character => Self::Character,
            FfiDiffGranularity::Line => Self::Line,
        }
    }
}

/// Operation classification for diff items (`COMPARE-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiDiffOperation {
    Insert,
    Delete,
    Replace,
    Move { from_index: u32, to_index: u32 },
    Unchanged,
}

impl From<DiffOperation> for FfiDiffOperation {
    fn from(op: DiffOperation) -> Self {
        match op {
            DiffOperation::Insert => Self::Insert,
            DiffOperation::Delete => Self::Delete,
            DiffOperation::Replace => Self::Replace,
            DiffOperation::Move {
                from_index,
                to_index,
            } => Self::Move {
                from_index: from_index as u32,
                to_index: to_index as u32,
            },
            DiffOperation::Unchanged => Self::Unchanged,
        }
    }
}

/// Comparison configuration options (`COMPARE-001`–`003`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiComparisonOptions {
    pub granularity: FfiDiffGranularity,
    pub ignore_whitespace: bool,
    pub ignore_case: bool,
    pub visual_pixel_threshold: u8,
    pub visual_cluster_distance_pt: f32,
    pub align_clauses: bool,
}

impl Default for FfiComparisonOptions {
    fn default() -> Self {
        Self {
            granularity: FfiDiffGranularity::Word,
            ignore_whitespace: true,
            ignore_case: false,
            visual_pixel_threshold: 16,
            visual_cluster_distance_pt: 10.0,
            align_clauses: true,
        }
    }
}

impl From<&FfiComparisonOptions> for ComparisonOptions {
    fn from(opts: &FfiComparisonOptions) -> Self {
        Self {
            granularity: opts.granularity.into(),
            ignore_whitespace: opts.ignore_whitespace,
            ignore_case: opts.ignore_case,
            visual_pixel_threshold: opts.visual_pixel_threshold,
            visual_cluster_distance_pt: opts.visual_cluster_distance_pt,
            align_clauses: opts.align_clauses,
        }
    }
}

/// Individual change detected between documents (`COMPARE-001`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiChangeItem {
    pub id: String,
    pub operation: FfiDiffOperation,
    pub page_a: Option<u32>,
    pub page_b: Option<u32>,
    pub text_a: Option<String>,
    pub text_b: Option<String>,
    pub rect_a: Option<FfiPdfRectMilliPt>,
    pub rect_b: Option<FfiPdfRectMilliPt>,
    pub clause_hint: Option<String>,
}

fn map_rect(r: &Option<PdfRectMilliPt>) -> Option<FfiPdfRectMilliPt> {
    r.as_ref().map(|b| FfiPdfRectMilliPt {
        x0_milli_pt: b.x0_milli_pt,
        y0_milli_pt: b.y0_milli_pt,
        x1_milli_pt: b.x1_milli_pt,
        y1_milli_pt: b.y1_milli_pt,
    })
}

impl From<ChangeItem> for FfiChangeItem {
    fn from(item: ChangeItem) -> Self {
        Self {
            id: item.id,
            operation: item.operation.into(),
            page_a: item.page_a,
            page_b: item.page_b,
            text_a: item.text_a,
            text_b: item.text_b,
            rect_a: map_rect(&item.rect_a),
            rect_b: map_rect(&item.rect_b),
            clause_hint: item.clause_hint,
        }
    }
}

/// Visual pixel difference result for a single page pair (`COMPARE-002`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPageDiffResult {
    pub page_a: u32,
    pub page_b: u32,
    pub pixel_difference_percentage: f32,
    pub difference_bounding_boxes: Vec<FfiPdfRectMilliPt>,
}

impl From<PageDiffResult> for FfiPageDiffResult {
    fn from(res: PageDiffResult) -> Self {
        Self {
            page_a: res.page_a,
            page_b: res.page_b,
            pixel_difference_percentage: res.pixel_difference_percentage,
            difference_bounding_boxes: res
                .difference_bounding_boxes
                .into_iter()
                .map(|b| FfiPdfRectMilliPt {
                    x0_milli_pt: b.x0_milli_pt,
                    y0_milli_pt: b.y0_milli_pt,
                    x1_milli_pt: b.x1_milli_pt,
                    y1_milli_pt: b.y1_milli_pt,
                })
                .collect(),
        }
    }
}

/// Global sequence page alignment (`COMPARE-003`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPageAlignment {
    pub page_a: Option<u32>,
    pub page_b: Option<u32>,
    pub similarity_score: f32,
}

impl From<PageAlignment> for FfiPageAlignment {
    fn from(a: PageAlignment) -> Self {
        Self {
            page_a: a.page_a,
            page_b: a.page_b,
            similarity_score: a.similarity_score,
        }
    }
}

/// Summary metrics of comparison session (`COMPARE-004`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiComparisonSummary {
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

impl From<ComparisonSummary> for FfiComparisonSummary {
    fn from(s: ComparisonSummary) -> Self {
        Self {
            total_changes: s.total_changes,
            insertions_count: s.insertions_count,
            deletions_count: s.deletions_count,
            replacements_count: s.replacements_count,
            moves_count: s.moves_count,
            similarity_ratio: s.similarity_ratio,
            identical_pages_count: s.identical_pages_count,
            modified_pages_count: s.modified_pages_count,
            inserted_pages_count: s.inserted_pages_count,
            deleted_pages_count: s.deleted_pages_count,
        }
    }
}

/// Comprehensive comparison report returned to native UI (`COMPARE-004`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiComparisonReport {
    pub doc_a_title: String,
    pub doc_b_title: String,
    pub page_count_a: u32,
    pub page_count_b: u32,
    pub summary: FfiComparisonSummary,
    pub page_alignments: Vec<FfiPageAlignment>,
    pub page_diffs: Vec<FfiPageDiffResult>,
    pub changes: Vec<FfiChangeItem>,
    pub json_report: String,
    pub markdown_report: String,
    pub html_report: String,
}

impl From<ComparisonReport> for FfiComparisonReport {
    fn from(rep: ComparisonReport) -> Self {
        let json_report = rep.to_json().unwrap_or_default();
        let markdown_report = rep.to_markdown();
        let html_report = rep.to_html();

        Self {
            doc_a_title: rep.doc_a_title,
            doc_b_title: rep.doc_b_title,
            page_count_a: rep.page_count_a,
            page_count_b: rep.page_count_b,
            summary: rep.summary.into(),
            page_alignments: rep.page_alignments.into_iter().map(Into::into).collect(),
            page_diffs: rep.page_diffs.into_iter().map(Into::into).collect(),
            changes: rep.changes.into_iter().map(Into::into).collect(),
            json_report,
            markdown_report,
            html_report,
        }
    }
}

/// Compares two active document sessions structurally and visually (`COMPARE-001`–`004`).
#[uniffi::export]
pub fn document_core_compare_documents(
    session_id_a: String,
    session_id_b: String,
    options: FfiComparisonOptions,
) -> Result<FfiComparisonReport, DocumentCoreError> {
    let core_opts = ComparisonOptions::from(&options);
    let report = session_registry().compare_sessions(&session_id_a, &session_id_b, &core_opts)?;
    Ok(report.into())
}

/// Compares two specific pages visually across sessions (`COMPARE-002`).
#[uniffi::export]
pub fn document_core_compare_pages_visual(
    session_id_a: String,
    page_a: u32,
    session_id_b: String,
    page_b: u32,
    options: FfiComparisonOptions,
) -> Result<FfiPageDiffResult, DocumentCoreError> {
    let core_opts = ComparisonOptions::from(&options);
    let diff = session_registry().compare_pages_visual(
        &session_id_a,
        page_a,
        &session_id_b,
        page_b,
        &core_opts,
    )?;
    Ok(diff.into())
}

/// Compares two plain text streams directly (`COMPARE-001`).
#[uniffi::export]
pub fn document_core_compare_text_streams(
    title_a: String,
    text_a: String,
    title_b: String,
    text_b: String,
    options: FfiComparisonOptions,
) -> FfiComparisonReport {
    let core_opts = ComparisonOptions::from(&options);
    let report = compare_text_streams(&title_a, &text_a, &title_b, &text_b, &core_opts);
    report.into()
}

/// Helper extracting linear text from a FlowDocument.
fn extract_flow_document_text(doc: &FlowDocument) -> String {
    let mut out = String::new();
    for section in &doc.sections {
        for block in &section.blocks {
            extract_block_text(block, &mut out);
        }
    }
    out
}

fn extract_block_text(block: &Block, out: &mut String) {
    match block {
        Block::Paragraph(p) => {
            for span in &p.spans {
                out.push_str(&span.text);
            }
            out.push('\n');
        }
        Block::Heading(h) => {
            for span in &h.spans {
                out.push_str(&span.text);
            }
            out.push('\n');
        }
        Block::List(l) => {
            for item in &l.items {
                for span in &item.spans {
                    out.push_str(&span.text);
                }
                out.push('\n');
            }
        }
        Block::Table(t) => {
            for row in &t.rows {
                for cell in &row.cells {
                    for b in &cell.blocks {
                        extract_block_text(b, out);
                    }
                    out.push('\t');
                }
                out.push('\n');
            }
        }
        Block::Callout(c) => {
            if let Some(title) = &c.title {
                out.push_str(title);
                out.push('\n');
            }
            for b in &c.blocks {
                extract_block_text(b, out);
            }
        }
        Block::ThematicBreak(_) | Block::Image(_) => {}
    }
}

/// Compares two active FlowEditor sessions (`COMPARE-001`).
#[uniffi::export]
pub fn document_core_compare_flow_editor_sessions(
    editor_id_a: String,
    editor_id_b: String,
    options: FfiComparisonOptions,
) -> Result<FfiComparisonReport, DocumentCoreError> {
    let reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;

    let session_a = reg
        .get(&editor_id_a)
        .ok_or_else(|| DocumentCoreError::SessionNotFound {
            session_id: editor_id_a.clone(),
        })?;
    let session_b = reg
        .get(&editor_id_b)
        .ok_or_else(|| DocumentCoreError::SessionNotFound {
            session_id: editor_id_b.clone(),
        })?;

    let text_a = extract_flow_document_text(&session_a.state.document);
    let text_b = extract_flow_document_text(&session_b.state.document);

    let title_a = if session_a.state.document.metadata.title.is_empty() {
        editor_id_a
    } else {
        session_a.state.document.metadata.title.clone()
    };
    let title_b = if session_b.state.document.metadata.title.is_empty() {
        editor_id_b
    } else {
        session_b.state.document.metadata.title.clone()
    };

    let core_opts = ComparisonOptions::from(&options);
    let report = compare_text_streams(&title_a, &text_a, &title_b, &text_b, &core_opts);
    Ok(report.into())
}

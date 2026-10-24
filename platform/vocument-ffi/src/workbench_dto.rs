//! UniFFI DTOs and exported functions for Page Workbench operations (`ROADMAP.md` M3).

use crate::navigation_search_dto::FfiPdfRectMilliPt;
use crate::{DocumentCoreError, mutation_dto::FfiTransactionRecord, session_registry};
use vocument_core::page_geometry::{PageBoxUpdate, PdfPageBoxes};

/// Execution options for Page Workbench operations (`M3`, `ADR-0019`).
#[derive(uniffi::Record, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FfiPageWorkbenchOptions {
    /// When false (default), page mutations are strictly rejected if active digital signatures exist.
    /// When true, the mutation proceeds and existing digital signatures are explicitly marked as invalidated.
    pub allow_signature_invalidation: bool,
}

impl From<FfiPageWorkbenchOptions> for vocument_core::workbench::PageWorkbenchOptions {
    fn from(ffi: FfiPageWorkbenchOptions) -> Self {
        Self {
            allow_signature_invalidation: ffi.allow_signature_invalidation,
        }
    }
}

/// The 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct FfiPdfPageBoxes {
    pub media_box: FfiPdfRectMilliPt,
    pub crop_box: FfiPdfRectMilliPt,
    pub bleed_box: FfiPdfRectMilliPt,
    pub trim_box: FfiPdfRectMilliPt,
    pub art_box: FfiPdfRectMilliPt,
    pub rotation_degrees: i32,
}

impl From<PdfPageBoxes> for FfiPdfPageBoxes {
    fn from(b: PdfPageBoxes) -> Self {
        Self {
            media_box: b.media_box.into(),
            crop_box: b.crop_box.into(),
            bleed_box: b.bleed_box.into(),
            trim_box: b.trim_box.into(),
            art_box: b.art_box.into(),
            rotation_degrees: b.rotation_degrees,
        }
    }
}

/// Partial or complete update of page boundary boxes (`CROP-002`).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq, Default)]
pub struct FfiPageBoxUpdate {
    pub media_box: Option<FfiPdfRectMilliPt>,
    pub crop_box: Option<FfiPdfRectMilliPt>,
    pub bleed_box: Option<FfiPdfRectMilliPt>,
    pub trim_box: Option<FfiPdfRectMilliPt>,
    pub art_box: Option<FfiPdfRectMilliPt>,
}

impl From<FfiPageBoxUpdate> for PageBoxUpdate {
    fn from(u: FfiPageBoxUpdate) -> Self {
        Self {
            media_box: u.media_box.map(Into::into),
            crop_box: u.crop_box.map(Into::into),
            bleed_box: u.bleed_box.map(Into::into),
            trim_box: u.trim_box.map(Into::into),
            art_box: u.art_box.map(Into::into),
        }
    }
}

/// Structural preflight report auditing invariants before Page Workbench mutations (`M3.0`, `ADR-0019`).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct FfiPageWorkbenchPreflightReport {
    pub is_signed: bool,
    pub signature_count: u32,
    pub signature_field_names: Vec<String>,
    pub has_acroform: bool,
    pub form_field_count: u32,
    pub has_outlines: bool,
    pub has_page_labels: bool,
    pub has_tagged_pdf: bool,
    pub is_encrypted: bool,
    pub allows_assembly: bool,
    pub warnings: Vec<String>,
}

impl From<vocument_core::workbench::PageWorkbenchPreflightReport>
    for FfiPageWorkbenchPreflightReport
{
    fn from(rep: vocument_core::workbench::PageWorkbenchPreflightReport) -> Self {
        Self {
            is_signed: rep.is_signed,
            signature_count: rep.signature_count as u32,
            signature_field_names: rep.signature_field_names,
            has_acroform: rep.has_acroform,
            form_field_count: rep.form_field_count as u32,
            has_outlines: rep.has_outlines,
            has_page_labels: rep.has_page_labels,
            has_tagged_pdf: rep.has_tagged_pdf,
            is_encrypted: rep.is_encrypted,
            allows_assembly: rep.allows_assembly,
            warnings: rep.warnings,
        }
    }
}

/// Audits structural invariants and digital signatures before Page Workbench operations (`M3.0`).
#[uniffi::export]
pub fn document_core_workbench_preflight(
    session_id: String,
) -> Result<FfiPageWorkbenchPreflightReport, DocumentCoreError> {
    let registry = session_registry();
    let report = registry.workbench_preflight(&session_id)?;
    Ok(report.into())
}

/// Reorders the pages of `session_id` according to `order` (0-indexed permutation) (`M3`).
/// Refuses if active digital signatures exist unless explicit options permit invalidation.
#[uniffi::export]
pub fn document_core_reorder_pages(
    session_id: String,
    order: Vec<u32>,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.reorder_pages(&session_id, order)?;
    Ok(record.into())
}

/// Reorders the pages of `session_id` with explicit signature invalidation options (`M3`).
#[uniffi::export]
pub fn document_core_reorder_pages_with_options(
    session_id: String,
    order: Vec<u32>,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.reorder_pages_with_options(&session_id, order, options.into())?;
    Ok(record.into())
}

/// Duplicates the page at `page_index` (0-indexed) in `session_id` (`M3`).
#[uniffi::export]
pub fn document_core_duplicate_page(
    session_id: String,
    page_index: u32,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.duplicate_page(&session_id, page_index)?;
    Ok(record.into())
}

/// Duplicates the page at `page_index` in `session_id` with explicit signature options (`M3`).
#[uniffi::export]
pub fn document_core_duplicate_page_with_options(
    session_id: String,
    page_index: u32,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.duplicate_page_with_options(&session_id, page_index, options.into())?;
    Ok(record.into())
}

/// Deletes the specified `page_indices` from the document of `session_id` (`M3`).
#[uniffi::export]
pub fn document_core_delete_pages(
    session_id: String,
    page_indices: Vec<u32>,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.delete_pages(&session_id, page_indices)?;
    Ok(record.into())
}

/// Deletes the specified `page_indices` with explicit signature invalidation options (`M3`).
#[uniffi::export]
pub fn document_core_delete_pages_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.delete_pages_with_options(&session_id, page_indices, options.into())?;
    Ok(record.into())
}

/// Extracts selected `page_indices` into a new standalone PDF buffer without modifying the current document (`M3`).
#[uniffi::export]
pub fn document_core_extract_pages(
    session_id: String,
    page_indices: Vec<u32>,
) -> Result<Vec<u8>, DocumentCoreError> {
    let registry = session_registry();
    let bytes = registry.extract_pages(&session_id, &page_indices)?;
    Ok(bytes)
}

/// Rotates the specified `page_indices` with explicit signature invalidation options (`M3`).
#[uniffi::export]
pub fn document_core_rotate_pages_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    delta_degrees: i32,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.rotate_pages_with_options(
        &session_id,
        page_indices,
        delta_degrees,
        options.into(),
    )?;
    Ok(record.into())
}

/// Inserts pages from another PDF document at index `insert_at` (`M3.2`).
#[uniffi::export]
pub fn document_core_insert_pages_with_options(
    session_id: String,
    source_pdf: Vec<u8>,
    source_page_indices: Vec<u32>,
    insert_at: u32,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.insert_pages_from_document_with_options(
        &session_id,
        source_pdf,
        source_page_indices,
        insert_at,
        options.into(),
    )?;
    Ok(record.into())
}

/// Merges external PDF documents into the current document (`M3.2`).
#[uniffi::export]
pub fn document_core_merge_documents_with_options(
    session_id: String,
    source_pdfs: Vec<Vec<u8>>,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.merge_documents_with_options(&session_id, source_pdfs, options.into())?;
    Ok(record.into())
}

/// Splits the current document into standalone PDF byte vectors for each page range (`M3.2`).
#[uniffi::export]
pub fn document_core_split_document(
    session_id: String,
    split_ranges: Vec<Vec<u32>>,
) -> Result<Vec<Vec<u8>>, DocumentCoreError> {
    let registry = session_registry();
    let split_docs = registry.split_document(&session_id, split_ranges)?;
    Ok(split_docs)
}

/// Queries the 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
#[uniffi::export]
pub fn document_core_get_page_boxes(
    session_id: String,
    page_index: u32,
) -> Result<FfiPdfPageBoxes, DocumentCoreError> {
    let registry = session_registry();
    let boxes = registry.get_page_boxes(&session_id, page_index)?;
    Ok(boxes.into())
}

/// Sets or updates page boundary boxes with explicit preflight options (`CROP-002`).
#[uniffi::export]
pub fn document_core_set_page_boxes_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    boxes: FfiPageBoxUpdate,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.set_page_boxes_with_options(
        &session_id,
        page_indices,
        boxes.into(),
        options.into(),
    )?;
    Ok(record.into())
}

/// Applies a destructive hard crop to selected pages (`CROP-001`).
#[uniffi::export]
pub fn document_core_apply_hard_crop_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    crop_rect: FfiPdfRectMilliPt,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.apply_hard_crop_with_options(
        &session_id,
        page_indices,
        crop_rect.into(),
        options.into(),
    )?;
    Ok(record.into())
}

/// Applies a non-destructive display crop by adjusting /CropBox (`CROP-001`).
#[uniffi::export]
pub fn document_core_apply_display_crop_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    crop_rect: FfiPdfRectMilliPt,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.apply_display_crop_with_options(
        &session_id,
        page_indices,
        crop_rect.into(),
        options.into(),
    )?;
    Ok(record.into())
}

/// Detects document skew angle on `page_index` in degrees (`ROTATE-003`).
#[uniffi::export]
pub fn document_core_detect_deskew_angle(
    session_id: String,
    page_index: u32,
) -> Result<f32, DocumentCoreError> {
    let registry = session_registry();
    let angle = registry.detect_deskew_angle(&session_id, page_index)?;
    Ok(angle)
}

/// Applies fine deskew rotation via transformation matrix (`ROTATE-003`).
#[uniffi::export]
pub fn document_core_deskew_pages_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    angle_milli_degrees: i32,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.deskew_pages_with_options(
        &session_id,
        page_indices,
        angle_milli_degrees,
        options.into(),
    )?;
    Ok(record.into())
}

/// Detects content bounding box on `page_index` with a safety margin (`CROP-003`).
#[uniffi::export]
pub fn document_core_detect_content_margins(
    session_id: String,
    page_index: u32,
    safety_margin_milli_pt: i32,
) -> Result<FfiPdfRectMilliPt, DocumentCoreError> {
    let registry = session_registry();
    let rect = registry.detect_content_margins(&session_id, page_index, safety_margin_milli_pt)?;
    Ok(rect.into())
}

/// Automatically crops pages by detecting content margins and setting CropBox (`CROP-003`).
#[uniffi::export]
pub fn document_core_auto_crop_pages_with_options(
    session_id: String,
    page_indices: Vec<u32>,
    safety_margin_milli_pt: i32,
    options: FfiPageWorkbenchOptions,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.auto_crop_pages_with_options(
        &session_id,
        page_indices,
        safety_margin_milli_pt,
        options.into(),
    )?;
    Ok(record.into())
}

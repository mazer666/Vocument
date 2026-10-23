//! UniFFI DTOs and exported functions for Page Workbench operations (`ROADMAP.md` M3).

use crate::{DocumentCoreError, mutation_dto::FfiTransactionRecord, session_registry};

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

//! UniFFI DTOs and exported functions for Page Workbench operations (`ROADMAP.md` M3).

use crate::{DocumentCoreError, mutation_dto::FfiTransactionRecord, session_registry};

/// Reorders the pages of `session_id` according to `order` (0-indexed permutation) (`M3`).
#[uniffi::export]
pub fn document_core_reorder_pages(
    session_id: String,
    order: Vec<u32>,
) -> Result<FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.reorder_pages(&session_id, order)?;
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

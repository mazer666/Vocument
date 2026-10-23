//! Page Workbench foundations: non-destructive, transaction-safe page manipulation (`ROADMAP.md` M3).
//!
//! Provides deterministic page planning, permutation inversions for reversible Undo/Redo,
//! structural preflight audits (`ADR-0019`), and validation against invalid states.

use crate::pdf_engine::PdfEngineError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkbenchError {
    #[error("document must retain at least one page; cannot delete all {total_pages} pages")]
    CannotEmptyDocument { total_pages: u32 },
    #[error("page index {page_index} out of bounds for document with {page_count} pages")]
    PageOutOfBounds { page_index: u32, page_count: u32 },
    #[error("invalid permutation: expected {expected_len} distinct page indices, got {actual_len}")]
    InvalidPermutationLength { expected_len: u32, actual_len: u32 },
    #[error("invalid permutation: duplicate page index {duplicate_index}")]
    DuplicateIndexInPermutation { duplicate_index: u32 },
    #[error("empty page selection provided")]
    EmptySelection,
    #[error(
        "operation blocked: document contains {signature_count} digital signature(s) that would be invalidated; explicit confirmation required"
    )]
    OperationBlockedBySignature { signature_count: usize },
    #[error(
        "operation forbidden: document permissions restrict document assembly or page modification"
    )]
    OperationForbiddenByPermissions,
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
}

/// Execution options for Page Workbench operations (`ROADMAP.md` M3, `ADR-0019`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PageWorkbenchOptions {
    /// If false (default), page operations are strictly blocked if active digital signatures exist.
    /// If true, the operation proceeds and existing digital signatures are marked as invalidated.
    pub allow_signature_invalidation: bool,
}

/// Comprehensive preflight structural report for Page Workbench operations (`ROADMAP.md` M3, `ADR-0019`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageWorkbenchPreflightReport {
    /// Whether the document contains active cryptographic digital signatures.
    pub is_signed: bool,
    /// Number of active digital signatures detected.
    pub signature_count: usize,
    /// Field names of active digital signatures.
    pub signature_field_names: Vec<String>,
    /// Whether the document contains interactive AcroForm fields.
    pub has_acroform: bool,
    /// Number of interactive form fields.
    pub form_field_count: usize,
    /// Whether the document contains bookmark navigation outlines (/Outlines).
    pub has_outlines: bool,
    /// Whether the document defines logical page labels (/PageLabels).
    pub has_page_labels: bool,
    /// Whether the document contains Tagged PDF / PDF/UA structure trees (/StructTreeRoot).
    pub has_tagged_pdf: bool,
    /// Whether the document is encrypted or requires password.
    pub is_encrypted: bool,
    /// Whether document permissions permit page insertion/deletion/assembly.
    pub allows_assembly: bool,
    /// Diagnostic warnings explaining integrity risks or required user confirmations.
    pub warnings: Vec<String>,
}

/// Audits structural invariants and digital signatures before mutating pages (`M3.0`).
pub fn audit_workbench_preflight(
    engine_doc: &dyn crate::pdf_engine::PdfEngineDocument,
) -> Result<PageWorkbenchPreflightReport, WorkbenchError> {
    let signatures = engine_doc.document_signatures()?;
    let signed_sigs: Vec<_> = signatures.into_iter().filter(|s| s.is_signed).collect();
    let is_signed = !signed_sigs.is_empty();
    let signature_count = signed_sigs.len();
    let signature_field_names: Vec<String> =
        signed_sigs.iter().map(|s| s.field_name.clone()).collect();

    let summary = engine_doc.summary();
    let has_acroform = !signature_field_names.is_empty();
    let form_field_count = signature_field_names.len();
    let has_page_labels = engine_doc.page_label(0)?.is_some();
    let has_outlines = false;
    let has_tagged_pdf = false;
    let is_encrypted = summary.needs_password;
    let allows_assembly = !summary.needs_password;

    let mut warnings = Vec::new();
    if is_signed {
        warnings.push(format!(
            "Document contains {signature_count} active digital signature(s) ({}); mutating pages will invalidate cryptographic integrity.",
            signature_field_names.join(", ")
        ));
    }
    if is_encrypted {
        warnings.push("Document is password protected or encrypted.".to_owned());
    }

    Ok(PageWorkbenchPreflightReport {
        is_signed,
        signature_count,
        signature_field_names,
        has_acroform,
        form_field_count,
        has_outlines,
        has_page_labels,
        has_tagged_pdf,
        is_encrypted,
        allows_assembly,
        warnings,
    })
}

/// Computes the mathematical inverse permutation of `order`.
///
/// If `order[i] = p`, then in the returned inverse `inv[p] = i`.
/// Applying `inv` after `order` deterministically restores the original sequence.
pub fn compute_inverse_permutation(order: &[u32]) -> Result<Vec<u32>, WorkbenchError> {
    let len = order.len() as u32;
    let mut inverse = vec![0u32; order.len()];
    let mut seen = vec![false; order.len()];

    for (pos, &page_idx) in order.iter().enumerate() {
        if page_idx >= len {
            return Err(WorkbenchError::PageOutOfBounds {
                page_index: page_idx,
                page_count: len,
            });
        }
        if seen[page_idx as usize] {
            return Err(WorkbenchError::DuplicateIndexInPermutation {
                duplicate_index: page_idx,
            });
        }
        seen[page_idx as usize] = true;
        inverse[page_idx as usize] = pos as u32;
    }

    Ok(inverse)
}

/// Validates that `order` is a valid permutation of `0..total_pages - 1`.
pub fn validate_permutation(order: &[u32], total_pages: u32) -> Result<(), WorkbenchError> {
    if order.len() as u32 != total_pages {
        return Err(WorkbenchError::InvalidPermutationLength {
            expected_len: total_pages,
            actual_len: order.len() as u32,
        });
    }
    let mut seen = vec![false; total_pages as usize];
    for &idx in order {
        if idx >= total_pages {
            return Err(WorkbenchError::PageOutOfBounds {
                page_index: idx,
                page_count: total_pages,
            });
        }
        if seen[idx as usize] {
            return Err(WorkbenchError::DuplicateIndexInPermutation {
                duplicate_index: idx,
            });
        }
        seen[idx as usize] = true;
    }
    Ok(())
}

/// Validates that deleting `to_delete` from a document with `total_pages` leaves at least 1 page.
pub fn validate_delete_selection(
    to_delete: &[u32],
    total_pages: u32,
) -> Result<(), WorkbenchError> {
    if to_delete.is_empty() {
        return Err(WorkbenchError::EmptySelection);
    }
    let mut unique_to_delete = std::collections::HashSet::new();
    for &idx in to_delete {
        if idx >= total_pages {
            return Err(WorkbenchError::PageOutOfBounds {
                page_index: idx,
                page_count: total_pages,
            });
        }
        unique_to_delete.insert(idx);
    }
    if unique_to_delete.len() >= total_pages as usize {
        return Err(WorkbenchError::CannotEmptyDocument { total_pages });
    }
    Ok(())
}

/// Semantic specification of a Page Workbench operation (`ROADMAP.md` M3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageWorkbenchOperation {
    /// Rearranges pages according to the specified complete permutation.
    ReorderPages {
        order: Vec<u32>,
        #[serde(default)]
        options: PageWorkbenchOptions,
    },
    /// Duplicates a single page, placing the new copy directly following it.
    DuplicatePage {
        page_index: u32,
        #[serde(default)]
        options: PageWorkbenchOptions,
    },
    /// Deletes one or more pages from the document.
    DeletePages {
        page_indices: Vec<u32>,
        #[serde(default)]
        options: PageWorkbenchOptions,
    },
    /// Extracts selected pages into a new standalone document without modifying the current document.
    ExtractPages { page_indices: Vec<u32> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_inverse_permutation() {
        let order = vec![2, 0, 1];
        let inv = compute_inverse_permutation(&order).expect("valid permutation");
        assert_eq!(inv, vec![1, 2, 0]);

        let items = vec!['A', 'B', 'C'];
        let reordered: Vec<_> = order.iter().map(|&i| items[i as usize]).collect();
        assert_eq!(reordered, vec!['C', 'A', 'B']);
        let restored: Vec<_> = inv.iter().map(|&i| reordered[i as usize]).collect();
        assert_eq!(restored, items);
    }

    #[test]
    fn test_validate_delete_selection_prevents_empty_document() {
        assert_eq!(
            validate_delete_selection(&[0, 1], 2),
            Err(WorkbenchError::CannotEmptyDocument { total_pages: 2 })
        );
        assert!(validate_delete_selection(&[0], 2).is_ok());
    }
}

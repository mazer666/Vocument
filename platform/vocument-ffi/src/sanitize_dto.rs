//! UniFFI DTOs and exported functions for document sanitization (`SAN-001`).

use crate::{DocumentCoreError, session_registry};
use vocument_core::pdf_engine::PdfSanitizeOptions;

/// Options for document sanitization (`SAN-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfSanitizeOptions {
    pub remove_metadata: bool,
    pub remove_attachments: bool,
    pub remove_actions: bool,
    pub remove_annotations: bool,
    pub remove_form_fields: bool,
}

impl Default for FfiPdfSanitizeOptions {
    fn default() -> Self {
        Self {
            remove_metadata: true,
            remove_attachments: true,
            remove_actions: true,
            remove_annotations: true,
            remove_form_fields: true,
        }
    }
}

impl From<FfiPdfSanitizeOptions> for PdfSanitizeOptions {
    fn from(options: FfiPdfSanitizeOptions) -> Self {
        Self {
            remove_metadata: options.remove_metadata,
            remove_attachments: options.remove_attachments,
            remove_actions: options.remove_actions,
            remove_annotations: options.remove_annotations,
            remove_form_fields: options.remove_form_fields,
        }
    }
}

/// Sanitizes the document of `session_id` through `TransactionManager` (`SAN-001`).
#[uniffi::export]
pub fn document_core_sanitize_document(
    session_id: String,
    options: FfiPdfSanitizeOptions,
) -> Result<crate::mutation_dto::FfiTransactionRecord, DocumentCoreError> {
    let registry = session_registry();
    let record = registry.sanitize_document(&session_id, options.into())?;
    Ok(record.into())
}

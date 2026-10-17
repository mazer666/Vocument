//! Vocument-defined `PdfEngine` interface (`ARCHITECTURE.md` §18, `ROADMAP.md` M0.1).
//!
//! Initial production implementation uses MuPDF (`engines/pdf-mupdf`).
//! No UI layer may reference MuPDF directly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Basic PDF metadata and page summary read when opening a `DocumentSession` (`ROADMAP.md` M0.1, `INSPECT-001`, `META-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfDocumentSummary {
    /// Total number of physical pages in the PDF (`0` or more).
    pub page_count: u32,
    /// PDF format version string reported by the engine (e.g. `"PDF 1.7"`).
    pub format_version: Option<String>,
    /// Document title metadata (`info:Title`), if present.
    pub title: Option<String>,
    /// Document author metadata (`info:Author`), if present.
    pub author: Option<String>,
    /// Document subject metadata (`info:Subject`), if present.
    pub subject: Option<String>,
    /// Document keywords metadata (`info:Keywords`), if present.
    pub keywords: Option<String>,
    /// Document creator application (`info:Creator`), if present.
    pub creator: Option<String>,
    /// PDF producer (`info:Producer`), if present.
    pub producer: Option<String>,
    /// Creation date string (`info:CreationDate`), if present.
    pub creation_date: Option<String>,
    /// Modification date string (`info:ModDate`), if present.
    pub modification_date: Option<String>,
    /// Whether the document requires a password to open (`SEC-001`).
    pub needs_password: bool,
    /// Identifier of the underlying `PdfEngine` implementation (e.g. `"mupdf-1.28.5"`).
    pub engine_identifier: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PdfEngineError {
    #[error("document is encrypted and requires an open password")]
    PasswordRequired,
    #[error("invalid or corrupted PDF document: {details}")]
    InvalidPdf { details: String },
    #[error("PDF engine internal failure: {details}")]
    EngineFailure { details: String },
}

/// Handle to an opened engine-level PDF instance owned inside a `DocumentSession`.
pub trait PdfEngineDocument: Send + Sync + std::fmt::Debug {
    /// Returns the immutable summary (page count + basic metadata) of the opened PDF.
    fn summary(&self) -> &PdfDocumentSummary;
}

/// Vocument-owned PDF engine interface (`ARCHITECTURE.md` §18).
pub trait PdfEngine: Send + Sync + std::fmt::Debug {
    /// Returns the name and version of the concrete PDF engine adapter.
    fn engine_identifier(&self) -> &str;

    /// Opens a PDF document from a brokered byte buffer without exposing raw engine pointers.
    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError>;
}

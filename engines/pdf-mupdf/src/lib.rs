//! MuPDF adapter behind Vocument's `PdfEngine` interface (`ARCHITECTURE.md` §18).
//!
//! No UI layer may depend on this crate directly.

/// Identifier of the PDF engine adapter.
pub const ENGINE_NAME: &str = "mupdf-adapter";

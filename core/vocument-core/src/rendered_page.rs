//! Port for reading the words that are readable on a rendered page
//! (`REDACT-002`).
//!
//! A document can carry the same text twice: as a text layer and as pixels
//! in an image. A redaction that only removes the text layer leaves the
//! picture of the page readable — the case that matters for a scan, where the
//! text layer is invisible and the picture carries the words.
//!
//! The core therefore does not judge the picture itself. It asks a reader
//! for the words that a reader would see on the rendered page, and checks
//! that the removed words are not among them any more. The platform wires the
//! implementation: the PDF engine renders the page, the OCR engine reads it.

use crate::pdf_engine::{PdfEngineError, PdfTextWord};

/// Reads the words that are readable on a rendered page (`REDACT-002`).
///
/// The implementation renders the page and reads the picture with an OCR
/// engine. The returned words carry their boxes in the page space of the
/// engine: milli-points, origin at the top left of the page, y descending —
/// the same space in which the redacted region is given, so that they can be
/// compared with it.
pub trait RenderedPageReader: Send + Sync + std::fmt::Debug {
    /// Renders `page_index` of the document and returns the readable words.
    fn read_rendered_words(
        &self,
        pdf_bytes: &[u8],
        page_index: u32,
    ) -> Result<Vec<PdfTextWord>, PdfEngineError>;
}

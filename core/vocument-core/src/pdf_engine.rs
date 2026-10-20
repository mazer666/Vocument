//! Vocument-defined `PdfEngine` interface (`ARCHITECTURE.md` §§7, 9, 17, 18, `ROADMAP.md` M0.1–M0.3).
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

/// Geometry of a single PDF page in PDF points (`1/72` inch, stored as milli-points for exact integer equality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageGeometry {
    pub page_index: u32,
    /// Width in milli-points (e.g. A4 `595 pt` = `595_000`).
    pub width_milli_pt: u32,
    /// Height in milli-points (e.g. A4 `842 pt` = `842_000`).
    pub height_milli_pt: u32,
}

/// Axis-aligned rectangle on a PDF page in milli-points (`1/1000` pt, origin at top-left of page).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfRectMilliPt {
    pub x0_milli_pt: i32,
    pub y0_milli_pt: i32,
    pub x1_milli_pt: i32,
    pub y1_milli_pt: i32,
}

impl PdfRectMilliPt {
    /// Returns true if this rectangle intersects `other`.
    pub fn intersects(&self, other: &Self) -> bool {
        let a_min_x = self.x0_milli_pt.min(self.x1_milli_pt);
        let a_max_x = self.x0_milli_pt.max(self.x1_milli_pt);
        let a_min_y = self.y0_milli_pt.min(self.y1_milli_pt);
        let a_max_y = self.y0_milli_pt.max(self.y1_milli_pt);

        let b_min_x = other.x0_milli_pt.min(other.x1_milli_pt);
        let b_max_x = other.x0_milli_pt.max(other.x1_milli_pt);
        let b_min_y = other.y0_milli_pt.min(other.y1_milli_pt);
        let b_max_y = other.y0_milli_pt.max(other.y1_milli_pt);

        a_min_x <= b_max_x && a_max_x >= b_min_x && a_min_y <= b_max_y && a_max_y >= b_min_y
    }
}

/// A single extracted text line on a PDF page with its bounding box and paragraph boundary flag (`ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTextLine {
    pub bbox: PdfRectMilliPt,
    pub text: String,
    /// True if this line is the last line of its text block/paragraph.
    pub ends_paragraph: bool,
}

/// Structured text and embedded page label extracted from a single PDF page (`ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageStructuredText {
    pub page_index: u32,
    /// Embedded PDF logical page label (e.g. `"i"`, `"ii"`, `"1"`), if defined in `/PageLabels`.
    pub embedded_page_label: Option<String>,
    pub lines: Vec<PdfTextLine>,
}

/// Request to rasterize a rectangular tile of a page at a given zoom level (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTileRenderRequest {
    pub page_index: u32,
    /// Zoom factor in milli-units (`1000` = 1.0x / 72 DPI, `2000` = 2.0x / 144 DPI).
    pub scale_milli: u32,
    pub tile_x_px: u32,
    pub tile_y_px: u32,
    pub tile_width_px: u32,
    pub tile_height_px: u32,
}

/// Raw RGBA8 bitmap output returned by `PdfEngineDisplayList::render_tile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTileBitmap {
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    pub rgba_bytes: Vec<u8>,
}

/// Standard PDF highlight annotation model (`ROADMAP.md` M0.4, `ANNOT-001`, `INTEROP-002`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfHighlightAnnotation {
    /// Stable annotation identifier stored in the PDF `/NM` key (e.g. `"annot_0001_..."`).
    pub annotation_id: String,
    /// `0`-based physical page index.
    pub page_index: u32,
    /// Bounding rectangle in page milli-points (`1/1000` pt).
    pub rect: PdfRectMilliPt,
    /// RGB highlight color (`[R, G, B]`, `0..=255`; default jurist yellow is `[255, 230, 80]`).
    pub color_rgb: [u8; 3],
    /// Opacity in milli-units (`0..=1000`, where `450` = `0.45`).
    pub opacity_milli: u32,
    /// Optional reviewer/author label (`/T`).
    pub author: Option<String>,
    /// Optional comment/note attached to the highlight (`/Contents`).
    pub contents: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PdfEngineError {
    #[error("document is encrypted and requires an open password")]
    PasswordRequired,
    #[error("invalid or corrupted PDF document: {details}")]
    InvalidPdf { details: String },
    #[error("page index {page_index} is out of bounds (page count: {page_count})")]
    PageOutOfBounds { page_index: u32, page_count: u32 },
    #[error("invalid tile dimensions ({width}x{height})")]
    InvalidTileDimensions { width: u32, height: u32 },
    #[error("invalid rotation angle {degrees} degrees (must be a multiple of 90)")]
    InvalidRotationAngle { degrees: i32 },
    #[error("annotation '{annotation_id}' not found on page {page_index}")]
    AnnotationNotFound {
        page_index: u32,
        annotation_id: String,
    },
    #[error("PDF engine internal failure: {details}")]
    EngineFailure { details: String },
}

/// Immutable display-list representation of a parsed PDF page (`ARCHITECTURE.md` §7, `ROADMAP.md` M0.2).
/// Created exclusively on the serialized `DocumentActor` and safe to replay concurrently on render workers.
pub trait PdfEngineDisplayList: Send + Sync + std::fmt::Debug {
    /// Returns the page geometry recorded when the display list was built.
    fn page_geometry(&self) -> PdfPageGeometry;

    /// Replays the immutable display list to rasterize a specific tile into an RGBA8 bitmap.
    fn render_tile(&self, request: PdfTileRenderRequest) -> Result<PdfTileBitmap, PdfEngineError>;
}

/// Handle to an opened engine-level PDF instance owned inside a `DocumentActor`.
pub trait PdfEngineDocument: Send + Sync + std::fmt::Debug {
    /// Returns the immutable summary (page count + basic metadata) of the opened PDF.
    fn summary(&self) -> &PdfDocumentSummary;

    /// Returns the current in-memory PDF byte buffer reflecting all committed mutations (`ROADMAP.md` M0.4).
    fn current_pdf_bytes(&self) -> &[u8];

    /// Queries the point geometry of `page_index` (`0`-based).
    fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, PdfEngineError>;

    /// Queries the current page rotation (`0`, `90`, `180`, or `270` degrees) of `page_index` (`ROTATE-001`).
    fn page_rotation_degrees(&self, page_index: u32) -> Result<u32, PdfEngineError>;

    /// Queries all standard `/Highlight` annotations on `page_index` (`ANNOT-001`, `INTEROP-002`).
    fn page_highlight_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError>;

    /// Queries the embedded PDF logical page label (from `/PageLabels`) for `page_index` (`0`-based).
    fn page_label(&self, page_index: u32) -> Result<Option<String>, PdfEngineError>;

    /// Extracts structured text lines and bounding boxes for `page_index` (`0`-based) (`ROADMAP.md` M0.3).
    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError>;

    /// Builds an immutable `PdfEngineDisplayList` for `page_index` (`0`-based).
    fn create_display_list(
        &self,
        page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError>;

    /// Rotates the given `0`-based `page_indices` by `delta_degrees` (multiple of 90) in the PDF page tree (`ROTATE-001`, `INTEROP-002`).
    /// Must only be invoked through `TransactionManager` (`ROADMAP.md` M0.4 Gate).
    fn rotate_pages(
        &mut self,
        page_indices: &[u32],
        delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError>;

    /// Adds a standard PDF `/Highlight` annotation to `annotation.page_index` (`ANNOT-001`, `INTEROP-002`).
    /// Must only be invoked through `TransactionManager` (`ROADMAP.md` M0.4 Gate).
    fn add_highlight_annotation(
        &mut self,
        annotation: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError>;

    /// Removes the highlight annotation identified by `annotation_id` from `page_index`, returning the removed snapshot (`ANNOT-001`).
    /// Must only be invoked through `TransactionManager` (`ROADMAP.md` M0.4 Gate).
    fn remove_highlight_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError>;

    /// Removes the content of the given regions of `page_index` from the
    /// document and returns the new bytes of the document (`REDACT-001`).
    ///
    /// The removal has to be final: the text, the images and the line art
    /// under the region must not be recoverable from the returned bytes. A
    /// black rectangle over the content is not a redaction, it only hides it.
    ///
    /// The coordinates are in PDF user space, in milli-points, with the origin
    /// at the lower left of the page — the same space in which
    /// [`PdfTextLine::bbox`] is reported.
    ///
    /// Must only be invoked through `TransactionManager`
    /// (`ROADMAP.md` M2 Gate: redaction extraction/search verification).
    fn redact_page_regions(
        &mut self,
        page_index: u32,
        regions: &[PdfRectMilliPt],
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_index, regions);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support redaction".to_owned(),
        })
    }
}

/// Vocument-owned PDF engine interface (`ARCHITECTURE.md` §18).
pub trait PdfEngine: Send + Sync + std::fmt::Debug {
    /// Returns the name and version of the concrete PDF engine adapter.
    fn engine_identifier(&self) -> &str;

    /// Opens a PDF document from a brokered byte buffer without exposing raw engine pointers.
    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError>;
}

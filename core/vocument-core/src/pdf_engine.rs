//! Vocument-defined `PdfEngine` interface (`ARCHITECTURE.md` §§7, 9, 17, 18, `ROADMAP.md` M0.1–M0.3).
//!
//! Initial production implementation uses MuPDF (`engines/pdf-mupdf`).
//! No UI layer may reference MuPDF directly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::document_inspection::{PdfActionInfo, PdfAttachmentInfo, PdfSignatureInfo};
pub use crate::page_geometry::{CropMode, PageBoxUpdate, PdfPageBoxes};
pub use crate::workbench_optimize::{
    ExtractedImageData, ExtractedImageInfo, OptimizationOptions, OptimizationPreset,
    OptimizationResult, PdfxPreflightReport, PrintPreflightReport,
};
pub use crate::workbench_stamps::{
    BatesNumberingOptions, BatesPosition, BookmarkItem, HeaderFooterOptions,
    StampOrWatermarkOptions,
};

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

/// A word of a text line with its bounding box (`REDACT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTextWord {
    /// The word as it stands in the document, without surrounding spaces.
    pub text: String,
    /// The box of the word in the page space of the engine: milli-points,
    /// origin at the top left of the page, y descending.
    pub bbox: PdfRectMilliPt,
}

/// Structured text and embedded page label extracted from a single PDF page (`ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageStructuredText {
    pub page_index: u32,
    /// Embedded PDF logical page label (e.g. `"i"`, `"ii"`, `"1"`), if defined in `/PageLabels`.
    pub embedded_page_label: Option<String>,
    pub lines: Vec<PdfTextLine>,
    /// The words of the page with their own bounding boxes, in reading order.
    ///
    /// The word boxes are the ground truth of a redaction: only the words
    /// that lie in the redacted region have to disappear from the document
    /// (`REDACT-001`). A line that is only partly redacted keeps its other
    /// words.
    pub words: Vec<PdfTextWord>,
}

/// A place of the document that still carries a redacted word (`REDACT-002`).
///
/// The redaction removes the content of a page. The text can survive in other
/// places: in the text of an annotation, in the value of a form field, in the
/// metadata, in the XMP packet, in the content of an embedded file. The engine
/// reports the places; the core decides what they mean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfTextOccurrence {
    /// The kind of the place: `content`, `annotation`, `form`, `attachment`,
    /// `info` or `xmp`.
    pub kind: String,
    /// The page the place lies on, `-1` for the document itself.
    pub page_index: i32,
    /// A short description of the place.
    pub location: String,
}

/// The result of the removal of the redacted words from the places of a
/// document that do not lie on the page (`REDACT-002`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfScrubReport {
    /// The number of annotations whose text was emptied.
    pub scrubbed_annotations: u32,
    /// The number of form values that were emptied.
    pub scrubbed_form_values: u32,
    /// The number of metadata fields that were cleaned.
    pub scrubbed_metadata_fields: u32,
}

/// Options for document sanitization (`SAN-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfSanitizeOptions {
    pub remove_metadata: bool,
    pub remove_attachments: bool,
    pub remove_actions: bool,
    pub remove_annotations: bool,
    pub remove_form_fields: bool,
}

impl Default for PdfSanitizeOptions {
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

/// The result of sanitizing a document (`SAN-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfSanitizeReport {
    pub metadata_cleared: bool,
    pub xmp_cleared: bool,
    pub attachments_removed: u32,
    pub actions_removed: u32,
    pub annotations_removed: u32,
    pub form_fields_removed: u32,
}

/// The words of the page that lie in the given regions.
///
/// The centre of a word decides, the same way it decides which words a
/// redaction removes. The words are the subject of the verification: after
/// the removal they must not be found in the document any more.
pub fn words_in_region(words: &[PdfTextWord], regions: &[PdfRectMilliPt]) -> Vec<String> {
    words
        .iter()
        .filter(|word| {
            let centre_x = (word.bbox.x0_milli_pt + word.bbox.x1_milli_pt) / 2;
            let centre_y = (word.bbox.y0_milli_pt + word.bbox.y1_milli_pt) / 2;
            regions.iter().any(|region| {
                centre_x >= region.x0_milli_pt
                    && centre_x <= region.x1_milli_pt
                    && centre_y >= region.y0_milli_pt
                    && centre_y <= region.y1_milli_pt
            })
        })
        .map(|word| word.text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect()
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

/// A grayscale raster of a whole page (`REDACT-002`).
///
/// The raster is the picture of the page, exactly as a reader sees it. A
/// document can carry the same text twice — as a text layer and as pixels in
/// an image — so a redaction has to be verified against this picture and not
/// only against the text layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfPageRaster {
    pub width_px: u32,
    pub height_px: u32,
    /// One byte per pixel, row by row, without padding.
    pub gray_bytes: Vec<u8>,
}

/// The comment of the reviewer (`ANNOT-001`).
///
/// A standard PDF `/Text` annotation with a popup: the note stands on the
/// page, the text of the note opens in the popup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfCommentAnnotation {
    /// Stable identifier of the annotation, stored in the PDF `/NM` key.
    pub annotation_id: String,
    /// `0`-based physical page index.
    pub page_index: u32,
    /// The box of the note on the page, in page milli-points.
    pub rect: PdfRectMilliPt,
    /// The key of the palette the note was written with.
    ///
    /// The key is the **stable** identifier: it does not change when the rail
    /// of the user changes.
    pub palette_key: String,
    /// The name of the rail the note was written with (`/VocumentPaletteLabel`).
    ///
    /// The name and the meaning travel with the note, so that the note says
    /// what it means even years later — and even if the rail of the user has
    /// been replaced in the meantime. A note that carries only a key would
    /// be unreadable in a document that is opened with another rail.
    pub palette_label: String,
    /// What the colour of the note meant in the review (`/VocumentPaletteMeaning`).
    pub palette_meaning: String,
    /// The colour of the note, RGB.
    pub color_rgb: [u8; 3],
    /// The text of the note.
    pub contents: String,
    /// The author of the note (`/T`).
    pub author: Option<String>,
}

impl PdfCommentAnnotation {
    /// Answers whether the note carries a text.
    pub fn has_text(&self) -> bool {
        !self.contents.trim().is_empty()
    }
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
    #[error(
        "operation blocked: document contains digital signatures; explicit confirmation required: {details}"
    )]
    OperationBlockedBySignature { details: String },
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

    /// Queries all comments of the reviewer on `page_index` (`ANNOT-001`).
    ///
    /// A comment is a standard PDF `/Text` annotation with a popup. The
    /// engine reports what it finds; the rail of the profession — what a
    /// colour means — lives in the core.
    fn page_comment_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError>;

    /// Writes a comment of the reviewer onto `annotation.page_index` (`ANNOT-001`).
    /// Must only be invoked through `TransactionManager` (`ANNOT-001`).
    fn add_comment_annotation(
        &mut self,
        annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError>;

    /// Removes the comment identified by `annotation_id` from `page_index`, returning the removed snapshot (`ANNOT-001`).
    /// Must only be invoked through `TransactionManager` (`ANNOT-001`).
    fn remove_comment_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError>;

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

    /// Rotates `page_indices` with explicit signature invalidation control (`M3`).
    fn rotate_pages_ext(
        &mut self,
        page_indices: &[u32],
        delta_degrees: i32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = allow_signature_invalidation;
        self.rotate_pages(page_indices, delta_degrees)
    }

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
    /// The coordinates are in the page space of the engine: milli-points,
    /// origin at the top left of the page, y descending — the same space in
    /// which the bounding boxes of the text and of the rendered page are
    /// reported.
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

    /// Replaces the bytes of the document (`REDACT-002`).
    ///
    /// A redaction that cannot be verified is rolled back: the document has to
    /// be the document it was before the attempt. The caller holds the
    /// previous bytes; the engine puts them back.
    fn replace_pdf_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PdfEngineError> {
        let _ = bytes;
        Err(PdfEngineError::EngineFailure {
            details: "this engine cannot restore the previous bytes".to_owned(),
        })
    }

    /// Reports the places of the document that still carry one of the given
    /// words (`REDACT-002`).
    ///
    /// The redaction removes the content of a page. The text can survive in
    /// other places: in the text of an annotation, in the value of a form
    /// field, in the metadata, in the XMP packet, in the content of an
    /// embedded file — and it does not have to be plain text: it can be stored
    /// as UTF-16 or as the hex string of the UTF-16 bytes.
    ///
    /// The engine reports the places, the core decides what they mean. An
    /// engine that cannot report the places has to say so: a redaction whose
    /// verification is missing is not a redaction.
    fn find_word_occurrences(
        &self,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, PdfEngineError> {
        let _ = words;
        Err(PdfEngineError::EngineFailure {
            details: "this engine cannot report the places of a text".to_owned(),
        })
    }

    /// Reports the links and actions of the document (`LINKSEC-001`).
    ///
    /// The report contains the actions of the pages (the links) and the
    /// actions of the document itself, for example the action that runs when
    /// the document is opened. The engine reports the facts; the risk policy
    /// lives in the core.
    fn document_actions(&self) -> Result<Vec<PdfActionInfo>, PdfEngineError> {
        let _ = &self;
        Ok(Vec::new())
    }

    /// Reports the embedded files of the document (`ATTACH-001`).
    ///
    /// The engine reports the facts: the name, the type, the size, whether
    /// the content is really in the document and whether the checksum of the
    /// embedded file matches. The risk policy lives in the core.
    fn document_attachments(&self) -> Result<Vec<PdfAttachmentInfo>, PdfEngineError> {
        let _ = &self;
        Ok(Vec::new())
    }

    /// Reports the signature fields of the document (`SIGREV-001`).
    ///
    /// The decisive finding is whether the document was changed after it was
    /// signed: a signature that no longer covers the content of the document
    /// is worth nothing.
    fn document_signatures(&self) -> Result<Vec<PdfSignatureInfo>, PdfEngineError> {
        let _ = &self;
        Ok(Vec::new())
    }

    /// Renders the whole page into a grayscale raster (`REDACT-002`).
    ///
    /// The scale is in milli-units (`1000` = 1.0x / 72 DPI, `2000` = 144 DPI).
    /// The raster is the picture of the page; it is the proof that a redacted
    /// region no longer carries readable content.
    fn render_page_gray(
        &self,
        page_index: u32,
        scale_milli: u32,
    ) -> Result<PdfPageRaster, PdfEngineError> {
        let _ = (page_index, scale_milli);
        Err(PdfEngineError::EngineFailure {
            details: "this engine cannot render pages".to_owned(),
        })
    }

    /// Sanitizes the document by stripping requested categories of metadata,
    /// attachments, actions, annotations, or form fields (`SAN-001`).
    fn sanitize_document(
        &mut self,
        options: &PdfSanitizeOptions,
    ) -> Result<(PdfSanitizeReport, Vec<u8>), PdfEngineError> {
        let _ = options;
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support document sanitization".to_owned(),
        })
    }

    /// Reorders the pages of the document according to `order` (`M3`).
    /// Must only be invoked through `TransactionManager`.
    fn reorder_pages(&mut self, order: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        self.reorder_pages_ext(order, false)
    }

    /// Reorders the pages with explicit signature invalidation control (`M3`).
    fn reorder_pages_ext(
        &mut self,
        order: &[u32],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (order, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support reordering pages".to_owned(),
        })
    }

    /// Duplicates `page_index` (0-indexed), inserting the copy immediately after it (`M3`).
    /// Must only be invoked through `TransactionManager`.
    fn duplicate_page(&mut self, page_index: u32) -> Result<Vec<u8>, PdfEngineError> {
        self.duplicate_page_ext(page_index, false)
    }

    /// Duplicates `page_index` with explicit signature invalidation control (`M3`).
    fn duplicate_page_ext(
        &mut self,
        page_index: u32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_index, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support duplicating pages".to_owned(),
        })
    }

    /// Deletes the specified `page_indices` from the document (`M3`).
    /// Must only be invoked through `TransactionManager`.
    fn delete_pages(&mut self, page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        self.delete_pages_ext(page_indices, false)
    }

    /// Deletes the specified `page_indices` with explicit signature invalidation control (`M3`).
    fn delete_pages_ext(
        &mut self,
        page_indices: &[u32],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support deleting pages".to_owned(),
        })
    }

    /// Extracts the specified `page_indices` into a new standalone PDF (`M3`).
    fn extract_pages(&self, page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
        let _ = page_indices;
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support extracting pages".to_owned(),
        })
    }

    /// Merges multiple PDF document byte buffers into this document (`M3.2`).
    /// Must only be invoked through `TransactionManager`.
    fn merge_documents_ext(
        &mut self,
        source_documents: &[&[u8]],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (source_documents, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support merging documents".to_owned(),
        })
    }

    /// Inserts pages from an external PDF document into this document at `insert_at` (0-based) (`M3.2`).
    /// Must only be invoked through `TransactionManager`.
    fn insert_pages_ext(
        &mut self,
        source_pdf: &[u8],
        source_page_indices: &[u32],
        insert_at: u32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (
            source_pdf,
            source_page_indices,
            insert_at,
            allow_signature_invalidation,
        );
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support inserting pages from external documents"
                .to_owned(),
        })
    }

    /// Splits the document into multiple standalone PDFs according to the specified page partitions (`M3.2`).
    fn split_document(&self, split_ranges: &[Vec<u32>]) -> Result<Vec<Vec<u8>>, PdfEngineError> {
        let _ = split_ranges;
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support splitting documents".to_owned(),
        })
    }

    /// Queries the 5 standard ISO 32000-1 page boundary boxes and rotation for `page_index` (`CROP-002`).
    fn get_page_boxes(&self, page_index: u32) -> Result<PdfPageBoxes, PdfEngineError> {
        let _ = page_index;
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support querying page boxes".to_owned(),
        })
    }

    /// Sets or updates page boundary boxes on selected pages (`CROP-002`).
    /// Must only be invoked through `TransactionManager`.
    fn set_page_boxes_ext(
        &mut self,
        page_indices: &[u32],
        boxes: &PageBoxUpdate,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, boxes, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support updating page boxes".to_owned(),
        })
    }

    /// Applies a destructive hard crop to selected pages (`CROP-001`).
    /// Clamps MediaBox/CropBox, injects a clipping path around content, and prunes out-of-bounds annotations.
    /// Must only be invoked through `TransactionManager`.
    fn apply_hard_crop_ext(
        &mut self,
        page_indices: &[u32],
        crop_rect: PdfRectMilliPt,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, crop_rect, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support hard cropping".to_owned(),
        })
    }

    /// Detects document skew angle on `page_index` in degrees (`ROTATE-003`).
    fn detect_deskew_angle(&self, page_index: u32) -> Result<f32, PdfEngineError> {
        let _ = page_index;
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support deskew angle detection".to_owned(),
        })
    }

    /// Applies fine deskew rotation to selected pages via transformation matrix `cm` (`ROTATE-003`).
    /// Must only be invoked through `TransactionManager`.
    fn apply_deskew_ext(
        &mut self,
        page_indices: &[u32],
        angle_degrees: f32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, angle_degrees, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support deskew rotation".to_owned(),
        })
    }

    /// Detects content bounding box on `page_index` with a safety margin (`CROP-003`).
    fn detect_content_margins(
        &self,
        page_index: u32,
        safety_margin_milli_pt: i32,
    ) -> Result<PdfRectMilliPt, PdfEngineError> {
        let _ = (page_index, safety_margin_milli_pt);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support detecting content margins".to_owned(),
        })
    }

    /// Automatically crops pages by detecting content margins and applying a non-destructive crop with `safety_margin_milli_pt` (`CROP-003`).
    /// Must only be invoked through `TransactionManager`.
    fn apply_auto_crop_ext(
        &mut self,
        page_indices: &[u32],
        safety_margin_milli_pt: i32,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (
            page_indices,
            safety_margin_milli_pt,
            allow_signature_invalidation,
        );
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support auto-cropping".to_owned(),
        })
    }

    /// Applies Bates numbering to selected pages (`M3.4`).
    fn apply_bates_numbering_ext(
        &mut self,
        page_indices: &[u32],
        options: &BatesNumberingOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, options, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support Bates numbering".to_owned(),
        })
    }

    /// Applies multi-zone headers and footers with dynamic macros (`M3.4`).
    fn apply_header_footer_ext(
        &mut self,
        page_indices: &[u32],
        options: &HeaderFooterOptions,
        doc_title: Option<&str>,
        date_str: Option<&str>,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (
            page_indices,
            options,
            doc_title,
            date_str,
            allow_signature_invalidation,
        );
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support headers and footers".to_owned(),
        })
    }

    /// Retrieves document bookmark (outline) hierarchy (`M3.4`).
    fn get_bookmarks(&self) -> Result<Vec<BookmarkItem>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support reading bookmarks".to_owned(),
        })
    }

    /// Replaces document bookmark (outline) hierarchy (`M3.4`).
    fn set_bookmarks_ext(
        &mut self,
        bookmarks: &[BookmarkItem],
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (bookmarks, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support setting bookmarks".to_owned(),
        })
    }

    /// Applies rubber stamp or watermark overlay to selected pages (`WATER-001`–`003`, `M3.4`).
    fn apply_stamp_or_watermark_ext(
        &mut self,
        page_indices: &[u32],
        options: &StampOrWatermarkOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (page_indices, options, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support stamps or watermarks".to_owned(),
        })
    }

    /// Optimizes and compresses document streams and structures (`ROADMAP.md` M3.5).
    fn optimize_document_ext(
        &mut self,
        options: &OptimizationOptions,
        allow_signature_invalidation: bool,
    ) -> Result<Vec<u8>, PdfEngineError> {
        let _ = (options, allow_signature_invalidation);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support document optimization".to_owned(),
        })
    }

    /// Scans document for embedded raster images (`M3.5`).
    fn get_document_images(&self) -> Result<Vec<ExtractedImageInfo>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support embedded image discovery".to_owned(),
        })
    }

    /// Extracts raster image bytes for a specified page and image index (`M3.5`).
    fn extract_image_bytes(
        &self,
        page_index: u32,
        image_index: u32,
    ) -> Result<ExtractedImageData, PdfEngineError> {
        let _ = (page_index, image_index);
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support image extraction".to_owned(),
        })
    }

    /// Preflight verification for PDF/X (ISO 15930) compliance (`M3.5`).
    fn preflight_pdfx(&self) -> Result<PdfxPreflightReport, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support PDF/X preflight".to_owned(),
        })
    }

    /// Preflight verification for print readiness (`PRINT-003`, `PRINT-004`).
    fn preflight_print(&self) -> Result<PrintPreflightReport, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "this engine does not support print preflight".to_owned(),
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

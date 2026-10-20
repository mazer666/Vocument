//! Reads the words that are readable on a rendered page (`REDACT-002`).
//!
//! The implementation renders the page with the PDF engine and reads the
//! picture with the OCR engine. It is the implementation of the core port
//! `RenderedPageReader`.
//!
//! Why this exists: a document can carry the same text twice — as a text layer
//! and as pixels in an image. For a scan the text layer is invisible and the
//! picture carries the words. A redaction that only removes the text layer
//! leaves the picture readable, and only a reading of the picture can prove
//! that this is not the case.

use std::sync::Arc;

use vocument_core::pdf_engine::{PdfEngine, PdfEngineError, PdfTextWord};
use vocument_core::rendered_page::RenderedPageReader;
use vocument_imaging::{PixelFormat, ScanFrame, ScanFrameMetadata};
use vocument_ocr::{OcrOptions, recognize_scan_frame};

/// The scale of the rendering, in milli-units.
///
/// `2000` = 144 DPI. It is high enough for the OCR to read a word of a
/// normal page and low enough to keep the rendering affordable.
const RENDER_SCALE_MILLI: u32 = 2000;

/// Reads the words of a rendered page with the PDF and the OCR engine.
#[derive(Debug, Default)]
pub struct EngineRenderedPageReader;

impl EngineRenderedPageReader {
    pub fn new() -> Self {
        Self
    }
}

impl RenderedPageReader for EngineRenderedPageReader {
    fn read_rendered_words(
        &self,
        pdf_bytes: &[u8],
        page_index: u32,
    ) -> Result<Vec<PdfTextWord>, PdfEngineError> {
        let engine = vocument_pdf_mupdf::MuPdfEngineAdapter::new();
        let document = engine.open_pdf_bytes(pdf_bytes)?;
        let raster = document.render_page_gray(page_index, RENDER_SCALE_MILLI)?;
        let page_geometry = document.page_geometry(page_index)?;

        let frame = ScanFrame {
            width: raster.width_px,
            height: raster.height_px,
            stride: raster.width_px,
            format: PixelFormat::Gray8,
            orientation_degrees: 0,
            pixels: raster.gray_bytes,
            metadata: ScanFrameMetadata::default(),
        };

        let options = OcrOptions {
            languages: vec!["deu".to_owned(), "eng".to_owned()],
            // The trained data of the application. Without it the OCR would
            // look in the system directory and find nothing.
            tessdata_path: crate::ocr_dto::current_tessdata_path(),
            detect_language: false,
            generate_hocr: false,
        };
        let result = recognize_scan_frame(&frame, &options).map_err(|error| {
            PdfEngineError::EngineFailure {
                details: format!("the rendered page could not be read: {error}"),
            }
        })?;

        // The boxes of the OCR are in pixels of the rendering. The rendering
        // and the page space of the engine have the same origin (top left,
        // y descending), so the boxes are converted without a flip.
        let page_width_milli_pt = page_geometry.width_milli_pt.max(1);
        let page_height_milli_pt = page_geometry.height_milli_pt.max(1);
        let width_px = raster.width_px.max(1);
        let height_px = raster.height_px.max(1);

        let words = result
            .words
            .iter()
            .map(|word| {
                let x0 = i64::from(word.bbox.x0.floor() as i32);
                let x1 = i64::from(word.bbox.x1.ceil() as i32);
                let y0 = i64::from(word.bbox.y0.floor() as i32);
                let y1 = i64::from(word.bbox.y1.ceil() as i32);
                PdfTextWord {
                    text: word.text.trim().to_owned(),
                    bbox: vocument_core::pdf_engine::PdfRectMilliPt {
                        x0_milli_pt: (x0 * i64::from(page_width_milli_pt) / i64::from(width_px))
                            as i32,
                        y0_milli_pt: (y0 * i64::from(page_height_milli_pt) / i64::from(height_px))
                            as i32,
                        x1_milli_pt: (x1 * i64::from(page_width_milli_pt) / i64::from(width_px))
                            as i32,
                        y1_milli_pt: (y1 * i64::from(page_height_milli_pt) / i64::from(height_px))
                            as i32,
                    },
                }
            })
            .collect();
        Ok(words)
    }
}

/// The reader that the platform wires into the session registry.
pub fn default_rendered_page_reader() -> Arc<dyn RenderedPageReader> {
    Arc::new(EngineRenderedPageReader::new())
}

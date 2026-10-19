//! `OcrProvider` interface and initial Tesseract adapter (`ARCHITECTURE.md` §20, `REQUIREMENTS.md` §9).
//!
//! Provides:
//! - Offline optical character recognition via embedded `TesseractEngine`
//! - Typographical hierarchy extraction (blocks, lines, words, character glyphs)
//! - Pixel and normalized bounding boxes with per-word and per-glyph confidence scoring
//! - Multi-language detection (German legal variants `de-AT`/`de-DE` and English `eng`)
//! - Direct integration with `vocument_imaging::enhance_frame` for OCR on clean scans
//! - Realistic test corpus loader and evaluation metrics

pub mod corpus;
pub mod language;
pub mod models;
pub mod provider;
pub mod synthesis;
pub mod tesseract;

pub use corpus::{
    CorpusDocument, CorpusMeta, CorpusVerificationResult, load_ocr_corpus, parse_pgm_to_scan_frame,
    verify_corpus_document_result,
};
pub use language::{LanguageDetectionResult, detect_language};
pub use models::{
    BoundingBox, OcrBlock, OcrBlockType, OcrCharacter, OcrLine, OcrOptions, OcrPageResult, OcrWord,
};
pub use provider::{OcrError, OcrProvider};
pub use synthesis::{
    SearchablePdfDocument, SearchablePdfPageInput, escape_pdf_winansi_string,
    helvetica_glyph_width, synthesize_searchable_pdf, synthesize_single_page_pdf,
};
pub use tesseract::TesseractEngine;
use vocument_imaging::{ScanCleanupProfile, ScanFrame, enhance_frame};

/// Identifier of the initial OCR provider.
pub const DEFAULT_OCR_PROVIDER: &str = "tesseract";

/// Executes optical character recognition over a raw or cleaned `ScanFrame` using the default
/// Tesseract provider.
pub fn recognize_scan_frame(
    frame: &ScanFrame,
    options: &OcrOptions,
) -> Result<OcrPageResult, OcrError> {
    let engine = TesseractEngine::new();
    engine.recognize_frame(frame, options)
}

/// Executes optical character recognition on a scan frame pre-processed through the
/// M1.2 intelligent scan cleanup and enhancement pipeline (`ARCHITECTURE.md` §21).
pub fn recognize_clean_scan_page(
    frame: &ScanFrame,
    profile: ScanCleanupProfile,
    options: &OcrOptions,
) -> Result<OcrPageResult, OcrError> {
    let cleaned = enhance_frame(frame, profile).map_err(|e| OcrError::ImageSetupFailed {
        details: format!("scan cleanup enhancement failed: {}", e),
    })?;
    let engine = TesseractEngine::new();
    engine.recognize_frame(&cleaned, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_provider_metadata() {
        let engine = TesseractEngine::new();
        assert_eq!(engine.provider_id(), "tesseract");
        assert!(engine.supported_languages().contains(&"deu".to_string()));
        assert!(engine.supported_languages().contains(&"eng".to_string()));
    }
}

//! UniFFI transport boundary for Optical Character Recognition (`ARCHITECTURE.md` §20, `ROADMAP.md` M1.3).

use vocument_imaging::{ScanCaptureSessionRegistry, ScanCleanupProfile, ScanFrame};
use vocument_ocr::{
    OcrOptions, detect_language as core_detect_language, recognize_clean_scan_page,
    recognize_scan_frame,
};

use crate::DocumentCoreError;
use crate::scan_dto::{FfiScanCleanupProfile, FfiScanFrame};

/// 2D bounding box representing spatial text extents.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FfiBoundingBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

/// A recognized single character glyph with spatial bounding box and confidence score.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiOcrCharacter {
    pub text: String,
    pub bbox: FfiBoundingBox,
    pub confidence: f32,
}

/// A recognized word composed of character glyphs and spatial bounding box.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiOcrWord {
    pub text: String,
    pub bbox: FfiBoundingBox,
    pub confidence: f32,
    pub characters: Vec<FfiOcrCharacter>,
    pub block_index: u32,
    pub line_index: u32,
    pub word_index: u32,
}

/// A typographical text line composed of words.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiOcrLine {
    pub text: String,
    pub bbox: FfiBoundingBox,
    pub confidence: f32,
    pub words: Vec<FfiOcrWord>,
    pub line_index: u32,
}

/// Complete OCR result snapshot for a processed document scan page.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiOcrPageResult {
    pub page_index: u32,
    pub full_text: String,
    pub words: Vec<FfiOcrWord>,
    pub lines: Vec<FfiOcrLine>,
    pub mean_confidence: f32,
    pub detected_language: String,
    pub hocr_output: Option<String>,
    pub frame_width: u32,
    pub frame_height: u32,
}

fn domain_bbox_to_ffi(b: &vocument_ocr::BoundingBox) -> FfiBoundingBox {
    FfiBoundingBox {
        x0: b.x0,
        y0: b.y0,
        x1: b.x1,
        y1: b.y1,
    }
}

fn domain_result_to_ffi(res: vocument_ocr::OcrPageResult) -> FfiOcrPageResult {
    let words = res
        .words
        .into_iter()
        .map(|w| FfiOcrWord {
            text: w.text,
            bbox: domain_bbox_to_ffi(&w.bbox),
            confidence: w.confidence,
            characters: w
                .characters
                .into_iter()
                .map(|c| FfiOcrCharacter {
                    text: c.text,
                    bbox: domain_bbox_to_ffi(&c.bbox),
                    confidence: c.confidence,
                })
                .collect(),
            block_index: w.block_index,
            line_index: w.line_index,
            word_index: w.word_index,
        })
        .collect();

    let lines = res
        .lines
        .into_iter()
        .map(|l| FfiOcrLine {
            text: l.text,
            bbox: domain_bbox_to_ffi(&l.bbox),
            confidence: l.confidence,
            words: l
                .words
                .into_iter()
                .map(|w| FfiOcrWord {
                    text: w.text,
                    bbox: domain_bbox_to_ffi(&w.bbox),
                    confidence: w.confidence,
                    characters: w
                        .characters
                        .into_iter()
                        .map(|c| FfiOcrCharacter {
                            text: c.text,
                            bbox: domain_bbox_to_ffi(&c.bbox),
                            confidence: c.confidence,
                        })
                        .collect(),
                    block_index: w.block_index,
                    line_index: w.line_index,
                    word_index: w.word_index,
                })
                .collect(),
            line_index: l.line_index,
        })
        .collect();

    FfiOcrPageResult {
        page_index: res.page_index,
        full_text: res.full_text,
        words,
        lines,
        mean_confidence: res.mean_confidence,
        detected_language: res.detected_language,
        hocr_output: res.hocr_output,
        frame_width: res.frame_width,
        frame_height: res.frame_height,
    }
}

/// Executes optical character recognition on a raw input scan frame.
#[uniffi::export]
pub fn document_core_ocr_frame(
    frame: FfiScanFrame,
    languages: Vec<String>,
    detect_language: bool,
) -> Result<FfiOcrPageResult, DocumentCoreError> {
    let engine_frame: ScanFrame = frame.into();
    engine_frame
        .validate()
        .map_err(|e| DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        })?;

    let options = OcrOptions {
        languages,
        tessdata_path: None,
        detect_language,
        generate_hocr: true,
    };

    let result = recognize_scan_frame(&engine_frame, &options).map_err(|e| {
        DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        }
    })?;

    Ok(domain_result_to_ffi(result))
}

/// Executes optical character recognition on a scan frame pre-processed through the
/// M1.2 intelligent scan cleanup and enhancement pipeline.
#[uniffi::export]
pub fn document_core_ocr_clean_scan_frame(
    frame: FfiScanFrame,
    profile: FfiScanCleanupProfile,
    languages: Vec<String>,
    detect_language: bool,
) -> Result<FfiOcrPageResult, DocumentCoreError> {
    let engine_frame: ScanFrame = frame.into();
    engine_frame
        .validate()
        .map_err(|e| DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        })?;
    let engine_profile: ScanCleanupProfile = profile.into();

    let options = OcrOptions {
        languages,
        tessdata_path: None,
        detect_language,
        generate_hocr: true,
    };

    let result =
        recognize_clean_scan_page(&engine_frame, engine_profile, &options).map_err(|e| {
            DocumentCoreError::ImagingFailure {
                details: e.to_string(),
            }
        })?;

    Ok(domain_result_to_ffi(result))
}

/// Executes optical character recognition on a captured scan page within an active session.
#[uniffi::export]
pub fn document_core_ocr_scan_session_page(
    session_id: String,
    page_index: u32,
    languages: Vec<String>,
) -> Result<FfiOcrPageResult, DocumentCoreError> {
    let enhanced_frame = ScanCaptureSessionRegistry::global()
        .get_enhanced_page(&session_id, page_index as usize)
        .map_err(|e| DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        })?;

    let options = OcrOptions {
        languages,
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    let result = recognize_scan_frame(&enhanced_frame, &options).map_err(|e| {
        DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        }
    })?;

    Ok(domain_result_to_ffi(result))
}

/// Automatically detects primary language of a text snippet.
#[uniffi::export]
pub fn document_core_detect_text_language(text: String) -> String {
    core_detect_language(&text).primary_language
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::scan_dto::FfiScanPixelFormat;

    #[test]
    fn test_ffi_language_detection() {
        let deu_text =
            "Vereinbarung über Vertraulichkeit und Geheimhaltung gemäß ABGB.".to_string();
        let lang = document_core_detect_text_language(deu_text);
        assert_eq!(lang, "deu");

        let eng_text =
            "Mutual Non-Disclosure Agreement and Confidentiality Obligations.".to_string();
        let lang2 = document_core_detect_text_language(eng_text);
        assert_eq!(lang2, "eng");
    }

    #[test]
    fn test_ffi_ocr_frame_execution() {
        let w = 80u32;
        let h = 40u32;
        let frame = FfiScanFrame {
            width: w,
            height: h,
            stride: w,
            format: FfiScanPixelFormat::Gray8,
            orientation_degrees: 0,
            pixels: vec![255u8; (w * h) as usize],
            timestamp_ms: 0,
            device_angle_degrees: 0.0,
            brightness_hint: 0.8,
            is_steady: true,
        };

        let result = document_core_ocr_frame(frame, vec!["eng".into()], false)
            .expect("document_core_ocr_frame should succeed");
        assert_eq!(result.frame_width, 80);
        assert_eq!(result.frame_height, 40);
        assert_eq!(result.detected_language, "eng");
    }
}

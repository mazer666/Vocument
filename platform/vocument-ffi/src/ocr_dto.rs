//! UniFFI transport boundary for Optical Character Recognition (`ARCHITECTURE.md` §20, `ROADMAP.md` M1.3).

use vocument_imaging::{ScanCaptureSessionRegistry, ScanCleanupProfile, ScanFrame, enhance_frame};
use vocument_ocr::{
    OcrOptions, OcrPageResult, detect_language as core_detect_language, recognize_clean_scan_page,
    recognize_scan_frame,
};

use crate::scan_dto::{FfiScanCleanupProfile, FfiScanFrame};
use crate::{DocumentCoreError, FfiDocumentSessionSnapshot, session_registry};

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

use std::sync::RwLock;

static CONFIGURED_TESSDATA_PATH: RwLock<Option<String>> = RwLock::new(None);

pub(crate) fn current_tessdata_path() -> Option<std::path::PathBuf> {
    CONFIGURED_TESSDATA_PATH
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .map(std::path::PathBuf::from)
}

/// Explicitly configures the directory containing traineddata files (deu.traineddata, eng.traineddata).
#[uniffi::export]
pub fn document_core_configure_tessdata_path(path: String) {
    if let Ok(mut lock) = CONFIGURED_TESSDATA_PATH.write() {
        *lock = Some(path);
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
        tessdata_path: current_tessdata_path(),
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
        tessdata_path: current_tessdata_path(),
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
        tessdata_path: current_tessdata_path(),
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

fn ffi_result_to_domain(ffi: FfiOcrPageResult) -> OcrPageResult {
    OcrPageResult {
        page_index: ffi.page_index,
        full_text: ffi.full_text,
        blocks: Vec::new(),
        lines: ffi
            .lines
            .into_iter()
            .map(|l| vocument_ocr::OcrLine {
                text: l.text,
                bbox: vocument_ocr::BoundingBox::new(l.bbox.x0, l.bbox.y0, l.bbox.x1, l.bbox.y1),
                confidence: l.confidence,
                words: l
                    .words
                    .into_iter()
                    .map(|w| vocument_ocr::OcrWord {
                        text: w.text,
                        bbox: vocument_ocr::BoundingBox::new(
                            w.bbox.x0, w.bbox.y0, w.bbox.x1, w.bbox.y1,
                        ),
                        confidence: w.confidence,
                        characters: w
                            .characters
                            .into_iter()
                            .map(|c| vocument_ocr::OcrCharacter {
                                text: c.text,
                                bbox: vocument_ocr::BoundingBox::new(
                                    c.bbox.x0, c.bbox.y0, c.bbox.x1, c.bbox.y1,
                                ),
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
            .collect(),
        words: ffi
            .words
            .into_iter()
            .map(|w| vocument_ocr::OcrWord {
                text: w.text,
                bbox: vocument_ocr::BoundingBox::new(w.bbox.x0, w.bbox.y0, w.bbox.x1, w.bbox.y1),
                confidence: w.confidence,
                characters: w
                    .characters
                    .into_iter()
                    .map(|c| vocument_ocr::OcrCharacter {
                        text: c.text,
                        bbox: vocument_ocr::BoundingBox::new(
                            c.bbox.x0, c.bbox.y0, c.bbox.x1, c.bbox.y1,
                        ),
                        confidence: c.confidence,
                    })
                    .collect(),
                block_index: w.block_index,
                line_index: w.line_index,
                word_index: w.word_index,
            })
            .collect(),
        mean_confidence: ffi.mean_confidence,
        detected_language: ffi.detected_language,
        hocr_output: ffi.hocr_output,
        frame_width: ffi.frame_width,
        frame_height: ffi.frame_height,
    }
}

/// Synthesizes a single-page searchable PDF with an invisible OCR text layer matching word bounding boxes.
#[uniffi::export]
pub fn document_core_synthesize_searchable_pdf(
    frame: FfiScanFrame,
    ocr_result: FfiOcrPageResult,
) -> Result<Vec<u8>, DocumentCoreError> {
    let engine_frame: ScanFrame = frame.into();
    let domain_ocr = ffi_result_to_domain(ocr_result);
    vocument_ocr::synthesize_single_page_pdf(engine_frame, domain_ocr).map_err(|e| {
        DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        }
    })
}

/// Synthesizes a multi-page searchable PDF document directly from an active scan capture session.
#[uniffi::export]
pub fn document_core_synthesize_scan_session_pdf(
    session_id: String,
    cleanup_profile: FfiScanCleanupProfile,
    languages: Vec<String>,
) -> Result<Vec<u8>, DocumentCoreError> {
    let registry = ScanCaptureSessionRegistry::global();
    let session = registry.get_session_snapshot(&session_id).map_err(|e| {
        DocumentCoreError::ImagingFailure {
            details: e.to_string(),
        }
    })?;

    if session.pages.is_empty() {
        return Err(DocumentCoreError::ImagingFailure {
            details: "Scan capture session contains 0 pages".into(),
        });
    }

    let engine_profile: ScanCleanupProfile = cleanup_profile.into();
    let options = OcrOptions {
        languages,
        tessdata_path: current_tessdata_path(),
        detect_language: true,
        generate_hocr: true,
    };

    let mut pdf_pages = Vec::with_capacity(session.pages.len());
    for (i, page) in session.pages.iter().enumerate() {
        let frame_to_ocr = if let Ok(enhanced) = registry.get_enhanced_page(&session_id, i) {
            enhanced
        } else if let Some(ref w) = page.warped_frame {
            enhance_frame(w, engine_profile).map_err(|e| DocumentCoreError::ImagingFailure {
                details: e.to_string(),
            })?
        } else {
            return Err(DocumentCoreError::ImagingFailure {
                details: format!("Page {} has no warped or enhanced frame", i),
            });
        };

        let ocr_res = recognize_scan_frame(&frame_to_ocr, &options).map_err(|e| {
            DocumentCoreError::ImagingFailure {
                details: e.to_string(),
            }
        })?;

        pdf_pages.push(vocument_ocr::SearchablePdfPageInput {
            frame: frame_to_ocr,
            ocr_result: ocr_res,
            page_width_pt: None,
            page_height_pt: None,
        });
    }

    let doc = vocument_ocr::SearchablePdfDocument {
        pages: pdf_pages,
        title: Some(format!("Vocument Scanned Session {}", session.session_id)),
        author: None,
        creator: Some("Vocument Core M1.4".into()),
    };

    vocument_ocr::synthesize_searchable_pdf(&doc).map_err(|e| DocumentCoreError::ImagingFailure {
        details: e.to_string(),
    })
}

/// Ingests an active scan capture session directly as an application-managed `DocumentSession` (`ROADMAP.md` M1.4).
#[uniffi::export]
pub fn document_core_ingest_scan_as_document_session(
    scan_session_id: String,
    cleanup_profile: FfiScanCleanupProfile,
    languages: Vec<String>,
    display_name: String,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let pdf_bytes = document_core_synthesize_scan_session_pdf(
        scan_session_id.clone(),
        cleanup_profile,
        languages,
    )?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let brokered = vocument_core::file_broker::BrokeredFileRead {
        source_token: format!("scan://{}", scan_session_id),
        display_name: if display_name.is_empty() {
            format!("{}.pdf", scan_session_id)
        } else {
            display_name
        },
        access_origin: vocument_core::file_broker::FileAccessOrigin::InMemoryBuffer,
        bytes: pdf_bytes,
        modified_unix_ms: Some(now_ms),
        os_file_id: Some(format!("scan_{}", scan_session_id)),
    };

    let snapshot = session_registry()
        .open_brokered_file(brokered)
        .map_err(|e| match e {
            vocument_core::session::SessionError::PdfEngine(pe) => match pe {
                vocument_core::pdf_engine::PdfEngineError::PasswordRequired => {
                    DocumentCoreError::PasswordRequired
                }
                other => DocumentCoreError::InvalidPdf {
                    details: other.to_string(),
                },
            },
            other => DocumentCoreError::FileBrokerFailure {
                details: other.to_string(),
            },
        })?;

    let session_id_str = snapshot.session_id.as_str().to_owned();
    let total_pages = snapshot.pdf_summary.page_count;

    // Trigger immediate progressive SQLite FTS5 search indexing
    let _ = session_registry().index_search_batch(&session_id_str, 0, total_pages);

    Ok(snapshot.into())
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

    #[test]
    fn test_ffi_synthesize_searchable_pdf() {
        let w = 100u32;
        let h = 50u32;
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

        let ocr_result = FfiOcrPageResult {
            page_index: 0,
            full_text: "Vocument Searchable PDF".to_string(),
            words: vec![FfiOcrWord {
                text: "Vocument".to_string(),
                bbox: FfiBoundingBox {
                    x0: 0.1,
                    y0: 0.1,
                    x1: 0.5,
                    y1: 0.4,
                },
                confidence: 98.0,
                characters: Vec::new(),
                block_index: 0,
                line_index: 0,
                word_index: 0,
            }],
            lines: Vec::new(),
            mean_confidence: 98.0,
            detected_language: "eng".to_string(),
            hocr_output: None,
            frame_width: w,
            frame_height: h,
        };

        let pdf = document_core_synthesize_searchable_pdf(frame, ocr_result)
            .expect("document_core_synthesize_searchable_pdf should succeed");
        assert!(pdf.starts_with(b"%PDF-1.7\n"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        assert!(pdf.len() > 500);
    }
}

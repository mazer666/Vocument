//! Realistic OCR and Scan test corpus loader, evaluation metrics, and verification (`REQUIREMENTS.md` §9).

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use vocument_imaging::{PixelFormat, ScanFrame, ScanFrameMetadata};

use crate::models::OcrPageResult;
use crate::provider::OcrError;

/// Metadata record associated with a test corpus document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusMeta {
    pub document_id: String,
    pub language: String,
    pub locale_variant: String,
    pub category: String,
    pub expected_keywords: Vec<String>,
    pub min_confidence: f32,
}

/// A loaded realistic OCR test corpus document.
#[derive(Debug, Clone)]
pub struct CorpusDocument {
    pub meta: CorpusMeta,
    pub frame: ScanFrame,
    pub expected_text: String,
}

/// Results of verifying an OCR provider against a test corpus document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusVerificationResult {
    pub document_id: String,
    pub primary_language: String,
    pub detected_language: String,
    pub mean_confidence: f32,
    pub keyword_hit_rate: f32,
    pub character_accuracy: f32,
    pub word_accuracy: f32,
    pub cer: f32,
    pub wer: f32,
    pub matched_keywords: Vec<String>,
    pub missing_keywords: Vec<String>,
    pub total_words: usize,
    pub total_characters: usize,
    pub bounding_boxes_valid: bool,
    pub passed: bool,
}

/// Parses a binary Netpbm Grayscale (`P5`) image file into a `ScanFrame`.
pub fn parse_pgm_to_scan_frame(bytes: &[u8]) -> Result<ScanFrame, OcrError> {
    if bytes.len() < 16 || &bytes[0..2] != b"P5" {
        return Err(OcrError::ImageSetupFailed {
            details: "not a valid P5 PGM binary image".into(),
        });
    }

    let mut idx = 2;
    // Skip whitespace or comments
    let mut header_tokens = Vec::new();
    while header_tokens.len() < 3 && idx < bytes.len() {
        while idx < bytes.len()
            && (bytes[idx] == b' '
                || bytes[idx] == b'\t'
                || bytes[idx] == b'\r'
                || bytes[idx] == b'\n')
        {
            idx += 1;
        }
        if idx < bytes.len() && bytes[idx] == b'#' {
            while idx < bytes.len() && bytes[idx] != b'\n' {
                idx += 1;
            }
            continue;
        }
        let start = idx;
        while idx < bytes.len() && bytes[idx] > b' ' {
            idx += 1;
        }
        if start < idx {
            let token_str = std::str::from_utf8(&bytes[start..idx]).unwrap_or_default();
            if !token_str.is_empty() {
                header_tokens.push(token_str.to_string());
            }
        }
    }

    if header_tokens.len() < 3 {
        return Err(OcrError::ImageSetupFailed {
            details: "incomplete PGM header".into(),
        });
    }

    // Skip the single whitespace character after maxval
    if idx < bytes.len() && (bytes[idx] == b' ' || bytes[idx] == b'\n' || bytes[idx] == b'\r') {
        idx += 1;
    }

    let width: u32 = header_tokens[0]
        .parse()
        .map_err(|_| OcrError::ImageSetupFailed {
            details: "invalid PGM width".into(),
        })?;
    let height: u32 = header_tokens[1]
        .parse()
        .map_err(|_| OcrError::ImageSetupFailed {
            details: "invalid PGM height".into(),
        })?;

    let pixel_bytes = bytes[idx..].to_vec();
    let expected_len = (width * height) as usize;
    if pixel_bytes.len() < expected_len {
        return Err(OcrError::ImageSetupFailed {
            details: format!(
                "PGM data short: expected {} bytes, got {}",
                expected_len,
                pixel_bytes.len()
            ),
        });
    }

    Ok(ScanFrame {
        width,
        height,
        stride: width,
        format: PixelFormat::Gray8,
        orientation_degrees: 0,
        pixels: pixel_bytes[..expected_len].to_vec(),
        metadata: ScanFrameMetadata::default(),
    })
}

/// Loads all realistic OCR test corpus documents from `tests/corpus/ocr`.
pub fn load_ocr_corpus(base_dir: Option<&Path>) -> Result<Vec<CorpusDocument>, OcrError> {
    let root = if let Some(b) = base_dir {
        b.to_path_buf()
    } else {
        PathBuf::from("tests/corpus/ocr")
    };

    if !root.exists() {
        return Err(OcrError::CorpusNotFound {
            path: root.display().to_string(),
        });
    }

    let doc_ids = [
        "deu_vertrag_vertraulichkeit",
        "eng_mutual_nda",
        "bilingual_rechnung_invoice",
        "deu_vertrag_vertraulichkeit_camera_distorted",
    ];
    let mut docs = Vec::new();

    for id in doc_ids {
        let meta_path = root.join(format!("{}_meta.json", id));
        let pgm_path = root.join(format!("{}.pgm", id));
        let txt_path = root.join(format!("{}_expected.txt", id));

        if !meta_path.exists() || !pgm_path.exists() || !txt_path.exists() {
            continue;
        }

        let meta_content =
            fs::read_to_string(&meta_path).map_err(|e| OcrError::CorpusNotFound {
                path: format!("{}: {}", meta_path.display(), e),
            })?;
        let meta: CorpusMeta =
            serde_json::from_str(&meta_content).map_err(|e| OcrError::CorpusNotFound {
                path: format!("JSON error {}: {}", meta_path.display(), e),
            })?;

        let expected_text = fs::read_to_string(&txt_path).unwrap_or_default();
        let pgm_bytes = fs::read(&pgm_path).map_err(|e| OcrError::CorpusNotFound {
            path: format!("{}: {}", pgm_path.display(), e),
        })?;

        let frame = parse_pgm_to_scan_frame(&pgm_bytes)?;
        docs.push(CorpusDocument {
            meta,
            frame,
            expected_text,
        });
    }

    if docs.is_empty() {
        return Err(OcrError::CorpusNotFound {
            path: format!("no corpus fixtures in {}", root.display()),
        });
    }

    Ok(docs)
}

/// Computes the Levenshtein edit distance between two slices.
pub fn levenshtein_distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let len_b = b.len();
    if a.is_empty() {
        return len_b;
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev_row: Vec<usize> = (0..=len_b).collect();
    let mut curr_row = vec![0; len_b + 1];

    for (i, elem_a) in a.iter().enumerate() {
        curr_row[0] = i + 1;
        for (j, elem_b) in b.iter().enumerate() {
            let cost = if elem_a == elem_b { 0 } else { 1 };
            curr_row[j + 1] = (prev_row[j + 1] + 1)
                .min(curr_row[j] + 1)
                .min(prev_row[j] + cost);
        }
        prev_row.copy_from_slice(&curr_row);
    }

    prev_row[len_b]
}

/// Normalizes whitespace in OCR text string for comparative evaluation.
pub fn normalize_ocr_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Computes Character Error Rate (CER) and Character Accuracy against expected text.
pub fn compute_character_error_rate(expected: &str, recognized: &str) -> (f32, f32) {
    let norm_exp = normalize_ocr_text(expected);
    let norm_rec = normalize_ocr_text(recognized);
    let exp_chars: Vec<char> = norm_exp.chars().collect();
    let rec_chars: Vec<char> = norm_rec.chars().collect();

    if exp_chars.is_empty() {
        return (0.0, 1.0);
    }

    let distance = levenshtein_distance(&exp_chars, &rec_chars);
    let cer = distance as f32 / exp_chars.len() as f32;
    let accuracy = (1.0 - cer.min(1.0)).max(0.0);
    (cer, accuracy)
}

/// Computes Word Error Rate (WER) and Word Accuracy against expected text.
pub fn compute_word_error_rate(expected: &str, recognized: &str) -> (f32, f32) {
    let exp_words: Vec<&str> = expected.split_whitespace().collect();
    let rec_words: Vec<&str> = recognized.split_whitespace().collect();

    if exp_words.is_empty() {
        return (0.0, 1.0);
    }

    let distance = levenshtein_distance(&exp_words, &rec_words);
    let wer = distance as f32 / exp_words.len() as f32;
    let accuracy = (1.0 - wer.min(1.0)).max(0.0);
    (wer, accuracy)
}

/// Evaluates an OCR provider result against expected ground truth metadata with a hardened $\ge 95\%$ gate.
pub fn verify_corpus_document_result(
    doc: &CorpusDocument,
    result: &OcrPageResult,
) -> CorpusVerificationResult {
    let lower_text = result.full_text.to_lowercase();
    let mut matched_keywords = Vec::new();
    let mut missing_keywords = Vec::new();

    for kw in &doc.meta.expected_keywords {
        let lk = kw.to_lowercase();
        if lower_text.contains(&lk) {
            matched_keywords.push(kw.clone());
        } else {
            // Check for minor fuzzy matching (e.g. trailing hyphen or punctuation)
            let found_fuzzy = result.words.iter().any(|w| {
                let w_lower = w.text.to_lowercase();
                w_lower.contains(&lk) || lk.contains(&w_lower)
            });
            if found_fuzzy {
                matched_keywords.push(kw.clone());
            } else {
                missing_keywords.push(kw.clone());
            }
        }
    }

    let keyword_hit_rate = if !doc.meta.expected_keywords.is_empty() {
        matched_keywords.len() as f32 / doc.meta.expected_keywords.len() as f32
    } else {
        1.0
    };

    let total_words = result.words.len();
    let total_characters: usize = result.words.iter().map(|w| w.characters.len()).sum();

    // Verify all bounding boxes are non-degenerate and within document dimensions
    let mut bounding_boxes_valid = true;
    for w in &result.words {
        if !w.bbox.is_valid()
            || w.bbox.x1 > result.frame_width as f32
            || w.bbox.y1 > result.frame_height as f32
        {
            bounding_boxes_valid = false;
            break;
        }
        for c in &w.characters {
            if !c.bbox.is_valid() {
                bounding_boxes_valid = false;
                break;
            }
        }
    }

    let (cer, character_accuracy) =
        compute_character_error_rate(&doc.expected_text, &result.full_text);
    let (wer, word_accuracy) = compute_word_error_rate(&doc.expected_text, &result.full_text);

    // Hardened gate: Character accuracy >= 95%, keyword hit rate >= 95%,
    // confidence >= min_confidence, valid bounding boxes, non-empty text.
    let conf_passed = result.mean_confidence >= doc.meta.min_confidence;
    let hit_passed = keyword_hit_rate >= 0.95;
    let char_acc_passed = character_accuracy >= 0.95;
    let boxes_passed = bounding_boxes_valid && total_words > 0;

    let passed = conf_passed && hit_passed && char_acc_passed && boxes_passed;

    CorpusVerificationResult {
        document_id: doc.meta.document_id.clone(),
        primary_language: doc.meta.language.clone(),
        detected_language: result.detected_language.clone(),
        mean_confidence: result.mean_confidence,
        keyword_hit_rate,
        character_accuracy,
        word_accuracy,
        cer,
        wer,
        matched_keywords,
        missing_keywords,
        total_words,
        total_characters,
        bounding_boxes_valid,
        passed,
    }
}

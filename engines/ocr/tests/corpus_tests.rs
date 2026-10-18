//! Comprehensive realistic OCR corpus integration tests (`REQUIREMENTS.md` §9, `ARCHITECTURE.md` §20).

use std::path::PathBuf;
use vocument_imaging::ScanCleanupProfile;
use vocument_ocr::{
    OcrOptions, load_ocr_corpus, recognize_clean_scan_page, recognize_scan_frame,
    verify_corpus_document_result,
};

fn resolve_corpus_path() -> PathBuf {
    let candidates = [
        PathBuf::from("tests/corpus/ocr"),
        PathBuf::from("../../tests/corpus/ocr"),
    ];
    for c in candidates {
        if c.exists() {
            return c;
        }
    }
    PathBuf::from("tests/corpus/ocr")
}

#[test]
fn test_realistic_corpus_german_legal_document() {
    let corpus_dir = resolve_corpus_path();
    let docs = load_ocr_corpus(Some(&corpus_dir)).expect("Failed to load OCR test corpus");
    let deu_doc = docs
        .iter()
        .find(|d| d.meta.document_id == "deu_vertrag_vertraulichkeit")
        .expect("deu_vertrag_vertraulichkeit not found in corpus");

    let options = OcrOptions {
        languages: vec!["deu".into()],
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    let result = recognize_scan_frame(&deu_doc.frame, &options).expect("OCR execution failed");
    let verification = verify_corpus_document_result(deu_doc, &result);

    assert!(
        verification.passed,
        "Corpus verification failed for German legal document: hit_rate={:.1}%, conf={:.1}%, missing={:?}",
        verification.keyword_hit_rate * 100.0,
        verification.mean_confidence,
        verification.missing_keywords
    );

    assert_eq!(verification.detected_language, "deu");
    assert!(verification.mean_confidence >= 85.0);
    assert!(
        verification.total_words >= 30,
        "Expected at least 30 words, got {}",
        verification.total_words
    );
    assert!(
        verification.total_characters >= 150,
        "Expected at least 150 characters, got {}",
        verification.total_characters
    );
    assert!(verification.bounding_boxes_valid);
    assert!(
        result.hocr_output.is_some(),
        "Expected hOCR output to be present"
    );
}

#[test]
fn test_realistic_corpus_english_mutual_nda() {
    let corpus_dir = resolve_corpus_path();
    let docs = load_ocr_corpus(Some(&corpus_dir)).expect("Failed to load OCR test corpus");
    let eng_doc = docs
        .iter()
        .find(|d| d.meta.document_id == "eng_mutual_nda")
        .expect("eng_mutual_nda not found in corpus");

    let options = OcrOptions {
        languages: vec!["eng".into()],
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    let result = recognize_scan_frame(&eng_doc.frame, &options).expect("OCR execution failed");
    let verification = verify_corpus_document_result(eng_doc, &result);

    assert!(
        verification.passed,
        "Corpus verification failed for English NDA: hit_rate={:.1}%, conf={:.1}%, missing={:?}",
        verification.keyword_hit_rate * 100.0,
        verification.mean_confidence,
        verification.missing_keywords
    );

    assert_eq!(verification.detected_language, "eng");
    assert!(verification.mean_confidence >= 85.0);
    assert!(verification.total_words >= 40);
    assert!(verification.bounding_boxes_valid);
}

#[test]
fn test_realistic_corpus_bilingual_invoice() {
    let corpus_dir = resolve_corpus_path();
    let docs = load_ocr_corpus(Some(&corpus_dir)).expect("Failed to load OCR test corpus");
    let bi_doc = docs
        .iter()
        .find(|d| d.meta.document_id == "bilingual_rechnung_invoice")
        .expect("bilingual_rechnung_invoice not found in corpus");

    let options = OcrOptions {
        languages: vec!["deu".into(), "eng".into()],
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    let result = recognize_scan_frame(&bi_doc.frame, &options).expect("OCR execution failed");
    let verification = verify_corpus_document_result(bi_doc, &result);

    assert!(
        verification.passed,
        "Corpus verification failed for bilingual invoice: hit_rate={:.1}%, conf={:.1}%, missing={:?}",
        verification.keyword_hit_rate * 100.0,
        verification.mean_confidence,
        verification.missing_keywords
    );

    assert!(verification.mean_confidence >= 85.0);
    assert!(verification.total_words >= 25);
    assert!(verification.bounding_boxes_valid);
}

#[test]
fn test_ocr_on_cleaned_scan_frame_pipeline() {
    let corpus_dir = resolve_corpus_path();
    let docs = load_ocr_corpus(Some(&corpus_dir)).expect("Failed to load OCR test corpus");
    let deu_doc = docs
        .iter()
        .find(|d| d.meta.document_id == "deu_vertrag_vertraulichkeit")
        .expect("deu_vertrag_vertraulichkeit not found in corpus");

    let options = OcrOptions {
        languages: vec!["deu".into()],
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    // 1. Run OCR on cleaned page using DocumentAuto profile
    let auto_res =
        recognize_clean_scan_page(&deu_doc.frame, ScanCleanupProfile::DocumentAuto, &options)
            .expect("OCR on DocumentAuto scan failed");

    let auto_verif = verify_corpus_document_result(deu_doc, &auto_res);
    assert!(
        auto_verif.passed,
        "Cleaned DocumentAuto OCR verification failed: hit_rate={:.1}%, conf={:.1}%",
        auto_verif.keyword_hit_rate * 100.0,
        auto_verif.mean_confidence
    );

    // 2. Run OCR on cleaned page using HighContrastBw profile
    let bw_res =
        recognize_clean_scan_page(&deu_doc.frame, ScanCleanupProfile::HighContrastBw, &options)
            .expect("OCR on HighContrastBw scan failed");

    let bw_verif = verify_corpus_document_result(deu_doc, &bw_res);
    assert!(
        bw_verif.passed,
        "Cleaned HighContrastBw OCR verification failed: hit_rate={:.1}%, conf={:.1}%",
        bw_verif.keyword_hit_rate * 100.0,
        bw_verif.mean_confidence
    );
}

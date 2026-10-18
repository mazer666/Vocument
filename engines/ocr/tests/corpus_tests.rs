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
        "Corpus verification failed for bilingual invoice: hit_rate={:.1}%, char_acc={:.1}%, missing={:?}",
        verification.keyword_hit_rate * 100.0,
        verification.character_accuracy * 100.0,
        verification.missing_keywords
    );

    assert!(verification.character_accuracy >= 0.95);
    assert!(verification.keyword_hit_rate >= 0.95);
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
        "Cleaned DocumentAuto OCR verification failed: hit_rate={:.1}%, char_acc={:.1}%",
        auto_verif.keyword_hit_rate * 100.0,
        auto_verif.character_accuracy * 100.0
    );
    assert!(auto_verif.character_accuracy >= 0.95);
    assert!(auto_verif.keyword_hit_rate >= 0.95);

    // 2. Run OCR on cleaned page using HighContrastBw profile
    let bw_res =
        recognize_clean_scan_page(&deu_doc.frame, ScanCleanupProfile::HighContrastBw, &options)
            .expect("OCR on HighContrastBw scan failed");

    let bw_verif = verify_corpus_document_result(deu_doc, &bw_res);
    assert!(
        bw_verif.passed,
        "Cleaned HighContrastBw OCR verification failed: hit_rate={:.1}%, char_acc={:.1}%",
        bw_verif.keyword_hit_rate * 100.0,
        bw_verif.character_accuracy * 100.0
    );
    assert!(bw_verif.character_accuracy >= 0.95);
    assert!(bw_verif.keyword_hit_rate >= 0.95);
}

#[test]
fn test_realistic_camera_distorted_scan_and_cleanup_pipeline() {
    use vocument_imaging::geometry::{Point2D, QuadCorners};
    use vocument_imaging::perspective::warp_perspective;

    let corpus_dir = resolve_corpus_path();
    let docs = load_ocr_corpus(Some(&corpus_dir)).expect("Failed to load OCR test corpus");
    let cam_doc = docs
        .iter()
        .find(|d| d.meta.document_id == "deu_vertrag_vertraulichkeit_camera_distorted")
        .expect("camera distorted fixture not found in corpus");

    let options = OcrOptions {
        languages: vec!["deu".into()],
        tessdata_path: None,
        detect_language: true,
        generate_hocr: true,
    };

    // 1. Verifiably demonstrate that raw OCR directly on the distorted, tilted camera frame fails:
    let raw_res = recognize_scan_frame(&cam_doc.frame, &options).expect("Raw OCR failed");
    let raw_verif = verify_corpus_document_result(cam_doc, &raw_res);
    assert!(
        !raw_verif.passed || raw_verif.character_accuracy < 0.95,
        "Raw camera frame unexpectedly passed without perspective correction or cleanup"
    );

    // 2. Apply M1.1 perspective warp and rectification:
    let corners = QuadCorners {
        top_left: Point2D::new(233.32 / 1500.0, 82.95 / 2050.0),
        top_right: Point2D::new(1465.35 / 1500.0, 223.33 / 2050.0),
        bottom_right: Point2D::new(1266.68 / 1500.0, 1967.05 / 2050.0),
        bottom_left: Point2D::new(34.65 / 1500.0, 1826.67 / 2050.0),
        confidence_milli: 900,
    };

    let rectified_frame = warp_perspective(&cam_doc.frame, &corners, Some(1240.0 / 1755.0))
        .expect("Perspective warp failed on camera frame");

    // 3. Apply M1.2 background illumination whitening and shadow removal:
    let cleaned_res =
        recognize_clean_scan_page(&rectified_frame, ScanCleanupProfile::DocumentAuto, &options)
            .expect("OCR on rectified & cleaned scan failed");

    let cleaned_verif = verify_corpus_document_result(cam_doc, &cleaned_res);
    assert!(
        cleaned_verif.passed,
        "Rectified and cleaned camera scan failed OCR gate: char_acc={:.2}%, hit_rate={:.1}%, conf={:.1}%, missing={:?}",
        cleaned_verif.character_accuracy * 100.0,
        cleaned_verif.keyword_hit_rate * 100.0,
        cleaned_verif.mean_confidence,
        cleaned_verif.missing_keywords
    );

    assert!(
        cleaned_verif.character_accuracy >= 0.95,
        "Expected character accuracy >= 95%, got {:.2}%",
        cleaned_verif.character_accuracy * 100.0
    );
    assert!(
        cleaned_verif.keyword_hit_rate >= 0.95,
        "Expected keyword hit rate >= 95%, got {:.2}%",
        cleaned_verif.keyword_hit_rate * 100.0
    );
    assert!(cleaned_verif.bounding_boxes_valid);
}

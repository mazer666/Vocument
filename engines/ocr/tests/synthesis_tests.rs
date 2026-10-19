//! Integration tests for M1.4 Searchable PDF Synthesis, MuPDF roundtrip, and Session Ingestion (`ROADMAP.md` M1.4).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use vocument_core::file_broker::{BrokeredFileRead, FileAccessOrigin};
use vocument_core::pdf_engine::PdfEngine;
use vocument_core::session::SessionRegistry;
use vocument_imaging::geometry::{Point2D, QuadCorners};
use vocument_imaging::perspective::warp_perspective;
use vocument_imaging::{ScanCleanupProfile, enhance_frame};
use vocument_ocr::corpus::parse_pgm_to_scan_frame;
use vocument_ocr::models::OcrOptions;
use vocument_ocr::provider::OcrProvider;
use vocument_ocr::synthesis::{
    SearchablePdfDocument, SearchablePdfPageInput, synthesize_searchable_pdf,
    synthesize_single_page_pdf,
};
use vocument_ocr::tesseract::TesseractEngine;
use vocument_pdf_mupdf::MuPdfEngineAdapter;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engines directory")
        .parent()
        .expect("repo root directory")
        .to_path_buf()
}

#[test]
fn test_searchable_pdf_synthesis_and_mupdf_roundtrip() {
    let pgm_path =
        repo_root().join("tests/corpus/ocr/deu_vertrag_vertraulichkeit_camera_distorted.pgm");
    let bytes = fs::read(&pgm_path).expect("Failed to read camera-distorted test corpus image");
    let raw_frame = parse_pgm_to_scan_frame(&bytes).expect("Failed to parse PGM to ScanFrame");

    // 1. Perspective Rectification (M1.1)
    let corners = QuadCorners {
        top_left: Point2D::new(233.32 / 1500.0, 82.95 / 2050.0),
        top_right: Point2D::new(1465.35 / 1500.0, 223.33 / 2050.0),
        bottom_right: Point2D::new(1266.68 / 1500.0, 1967.05 / 2050.0),
        bottom_left: Point2D::new(34.65 / 1500.0, 1826.67 / 2050.0),
        confidence_milli: 900,
    };
    let rectified = warp_perspective(&raw_frame, &corners, Some(1240.0 / 1755.0))
        .expect("Perspective warp failed");

    // 2. Scan Enhancement (M1.2)
    let cleaned =
        enhance_frame(&rectified, ScanCleanupProfile::DocumentAuto).expect("Scan cleanup failed");

    // 3. OCR (M1.3)
    let ocr_engine = TesseractEngine::new();
    let options = OcrOptions {
        languages: vec!["deu".into()],
        ..Default::default()
    };
    let ocr_result = ocr_engine
        .recognize_frame(&cleaned, &options)
        .expect("OCR recognition failed");

    assert!(!ocr_result.full_text.is_empty());
    assert!(!ocr_result.words.is_empty());

    // 4. Synthesize Searchable PDF (M1.4)
    let pdf_bytes = synthesize_single_page_pdf(cleaned.clone(), ocr_result.clone())
        .expect("Searchable PDF synthesis failed");

    assert!(pdf_bytes.starts_with(b"%PDF-1.7\n"));
    assert!(pdf_bytes.ends_with(b"%%EOF\n"));

    // 5. Interoperability & External Validation via mutool (if available)
    let mutool_cmd = if Command::new("mutool").arg("--version").output().is_ok() {
        Some("mutool".to_string())
    } else if std::path::Path::new("/opt/homebrew/bin/mutool").exists() {
        Some("/opt/homebrew/bin/mutool".to_string())
    } else {
        None
    };

    if let Some(cmd) = mutool_cmd {
        let temp_pdf =
            std::env::temp_dir().join(format!("vocument_m1_4_test_{}.pdf", std::process::id()));
        fs::write(&temp_pdf, &pdf_bytes).expect("Failed to write temp PDF for mutool");

        let info_out = Command::new(&cmd).arg("info").arg(&temp_pdf).output();
        if let Ok(out) = info_out {
            assert!(
                out.status.success(),
                "mutool info failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let info_str = String::from_utf8_lossy(&out.stdout);
            assert!(
                info_str.contains("Pages: 1")
                    || info_str.contains("1 pages")
                    || info_str.contains("Page 1")
            );
        }

        let draw_out = Command::new(&cmd)
            .args(["draw", "-F", "png", "-o", "/dev/null"])
            .arg(&temp_pdf)
            .output();
        if let Ok(out) = draw_out {
            assert!(
                out.status.success(),
                "mutool draw failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        let _ = fs::remove_file(temp_pdf);
    }

    // 6. MuPDF Engine Inspection & Structured Text Extraction
    let mupdf = MuPdfEngineAdapter::new();
    let engine_doc = mupdf
        .open_pdf_bytes(&pdf_bytes)
        .expect("MuPDF failed to open synthesized searchable PDF");

    let summary = engine_doc.summary();
    assert_eq!(summary.page_count, 1);
    assert!(!summary.needs_password);

    let structured_text = engine_doc
        .extract_page_text(0)
        .expect("MuPDF failed to extract structured text from synthesized searchable PDF");

    let full_text = structured_text
        .lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let lower_text = full_text.to_lowercase();
    assert!(
        lower_text.contains("vereinbarung"),
        "Missing keyword 'vereinbarung' in extracted text"
    );
    assert!(
        lower_text.contains("vertraulichkeit"),
        "Missing keyword 'vertraulichkeit' in extracted text"
    );
    assert!(
        lower_text.contains("geheimhaltung"),
        "Missing keyword 'geheimhaltung' in extracted text"
    );
    assert!(
        lower_text.contains("abgb"),
        "Missing Austrian legal keyword 'abgb' in extracted text"
    );
    assert!(
        lower_text.contains("uwg"),
        "Missing Austrian legal keyword 'uwg' in extracted text"
    );
    assert!(
        lower_text.contains("wien"),
        "Missing location keyword 'wien' in extracted text"
    );

    // 7. Direct Session Ingestion & Immediate Progressive SQLite FTS5 Search
    let session_registry = SessionRegistry::new(Box::new(mupdf));

    let brokered = BrokeredFileRead {
        source_token: "scan://m1_4_synthesized_session.pdf".into(),
        display_name: "m1_4_synthesized_session.pdf".into(),
        access_origin: FileAccessOrigin::InMemoryBuffer,
        bytes: pdf_bytes,
        modified_unix_ms: Some(1729300000000),
        os_file_id: Some("scan_m1_4_001".into()),
    };

    let session_snapshot = session_registry
        .open_brokered_file(brokered)
        .expect("Failed to ingest synthesized searchable PDF into DocumentSession");

    assert_eq!(session_snapshot.pdf_summary.page_count, 1);
    let session_id = session_snapshot.session_id.as_str().to_owned();

    // Immediately index page into SQLite FTS5
    let index_progress = session_registry
        .index_search_batch(&session_id, 0, 10)
        .expect("Failed to index synthesized searchable PDF page in SQLite FTS5");

    assert_eq!(index_progress.indexed_pages, 1);
    assert_eq!(index_progress.total_pages, 1);
    assert!(index_progress.is_complete);

    // Query SQLite FTS5 for Austrian legal keywords and verify hits and snippets
    let search_res_1 = session_registry
        .search_exact(&session_id, "Vertraulichkeit")
        .expect("FTS5 search for 'Vertraulichkeit' failed");
    assert_eq!(search_res_1.hits.len(), 1);
    assert_eq!(search_res_1.hits[0].page_index, 0);
    assert!(
        search_res_1.hits[0]
            .snippet
            .to_lowercase()
            .contains("vertraulichkeit")
    );

    let search_res_2 = session_registry
        .search_exact(&session_id, "ABGB")
        .expect("FTS5 search for 'ABGB' failed");
    assert_eq!(search_res_2.hits.len(), 1);
    assert_eq!(search_res_2.hits[0].page_index, 0);
    assert!(search_res_2.hits[0].snippet.contains("ABGB"));

    let search_res_3 = session_registry
        .search_exact(&session_id, "Wien")
        .expect("FTS5 search for 'Wien' failed");
    assert_eq!(search_res_3.hits.len(), 1);
    assert_eq!(search_res_3.hits[0].page_index, 0);
    assert!(search_res_3.hits[0].snippet.contains("Wien"));
}

#[test]
fn test_multi_page_searchable_pdf_synthesis_and_search() {
    let pgm_path =
        repo_root().join("tests/corpus/ocr/deu_vertrag_vertraulichkeit_camera_distorted.pgm");
    let bytes = fs::read(&pgm_path).expect("Failed to read camera-distorted test corpus image");
    let raw_frame = parse_pgm_to_scan_frame(&bytes).expect("Failed to parse PGM to ScanFrame");

    let corners = QuadCorners {
        top_left: Point2D::new(233.32 / 1500.0, 82.95 / 2050.0),
        top_right: Point2D::new(1465.35 / 1500.0, 223.33 / 2050.0),
        bottom_right: Point2D::new(1266.68 / 1500.0, 1967.05 / 2050.0),
        bottom_left: Point2D::new(34.65 / 1500.0, 1826.67 / 2050.0),
        confidence_milli: 900,
    };
    let rectified = warp_perspective(&raw_frame, &corners, Some(1240.0 / 1755.0))
        .expect("Perspective warp failed");
    let cleaned =
        enhance_frame(&rectified, ScanCleanupProfile::DocumentAuto).expect("Scan cleanup failed");

    let ocr_engine = TesseractEngine::new();
    let options = OcrOptions {
        languages: vec!["deu".into()],
        ..Default::default()
    };
    let ocr_result_1 = ocr_engine
        .recognize_frame(&cleaned, &options)
        .expect("OCR page 1 recognition failed");

    let mut ocr_result_2 = ocr_result_1.clone();
    ocr_result_2.page_index = 1;

    let doc = SearchablePdfDocument {
        pages: vec![
            SearchablePdfPageInput {
                frame: cleaned.clone(),
                ocr_result: ocr_result_1,
                page_width_pt: None,
                page_height_pt: None,
            },
            SearchablePdfPageInput {
                frame: cleaned,
                ocr_result: ocr_result_2,
                page_width_pt: None,
                page_height_pt: None,
            },
        ],
        title: Some("Vocument Multi-Page Contract".into()),
        author: Some("Vocument Scan Capture".into()),
        creator: Some("Vocument Core M1.4".into()),
    };

    let pdf_bytes = synthesize_searchable_pdf(&doc).expect("Multi-page PDF synthesis failed");
    assert!(pdf_bytes.starts_with(b"%PDF-1.7\n"));
    assert!(pdf_bytes.ends_with(b"%%EOF\n"));

    let mupdf = MuPdfEngineAdapter::new();
    let engine_doc = mupdf
        .open_pdf_bytes(&pdf_bytes)
        .expect("MuPDF failed to open multi-page synthesized PDF");

    assert_eq!(engine_doc.summary().page_count, 2);

    let text_p0 = engine_doc.extract_page_text(0).expect("Failed page 0 text");
    let text_p1 = engine_doc.extract_page_text(1).expect("Failed page 1 text");
    assert!(!text_p0.lines.is_empty());
    assert!(!text_p1.lines.is_empty());
}

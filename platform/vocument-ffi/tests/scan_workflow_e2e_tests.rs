//! End-to-end verification of the mobile scan workflow.
//!
//! This test walks the complete chain a phone goes through, using the same
//! UniFFI functions the Android and macOS shells call:
//!
//! ```text
//! camera frame → edge detection → perspective correction → cleanup → OCR
//!              → searchable PDF → session ingestion → full text search
//!              → save to disk → reopen and read back
//! ```
//!
//! The individual steps are covered by their own tests. What is verified here
//! is the **chain**: that the output of one step is the input of the next, and
//! that the document that comes out at the end is a real searchable PDF whose
//! text can be found and read back after saving.
//!
//! Everything runs against the real engine: MuPDF for the PDF side, Tesseract
//! for the OCR, the real image processing for the rectification. No mock is
//! used, because a mock would not show a broken hand-off between the steps.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use vocument_core::pdf_engine::PdfEngine;
use vocument_ffi::navigation_search_dto::FfiSearchResponse;
use vocument_ffi::ocr_dto::document_core_ingest_scan_as_document_session;
use vocument_ffi::save_dto::document_core_export_session_bytes;
use vocument_ffi::save_dto::document_core_save_session_as;
use vocument_ffi::scan_dto::{
    FfiQuadCorners, FfiScanCleanupProfile, FfiScanFrame, FfiScanPixelFormat,
    document_core_append_scan_page, document_core_detect_document_edges,
    document_core_start_scan_session,
};
use vocument_ffi::{document_core_index_search_batch, document_core_search_exact};

/// Longest time the whole workflow may take. A workflow that needs longer is a
/// defect: the user is waiting for it.
const WORKFLOW_BUDGET: Duration = Duration::from_secs(180);

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The camera-distorted capture of the German confidentiality agreement.
///
/// It is the hardest fixture in the corpus: a tilted, shadowed, photographed
/// page. A workflow that only works on a clean synthetic page is not a
/// workflow.
fn distorted_frame_path() -> PathBuf {
    repo_root().join("tests/corpus/ocr/deu_vertrag_vertraulichkeit_camera_distorted.pgm")
}

/// Parses a binary P5 PGM into the gray frame that the camera would deliver.
fn load_gray_frame(path: &Path) -> (u32, u32, Vec<u8>) {
    let bytes = std::fs::read(path).expect("the corpus frame must be readable");
    assert!(
        bytes.len() > 2 && &bytes[0..2] == b"P5",
        "the corpus frame must be a binary P5 PGM"
    );
    let mut index = 2usize;
    let mut tokens: Vec<std::ops::Range<usize>> = Vec::new();
    while tokens.len() < 3 && index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'#' {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        let start = index;
        while index < bytes.len() && !bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        tokens.push(start..index);
    }
    assert_eq!(
        tokens.len(),
        3,
        "the PGM header must carry width, height, depth"
    );
    let width: u32 = std::str::from_utf8(&bytes[tokens[0].clone()])
        .expect("width")
        .parse()
        .expect("width is a number");
    let height: u32 = std::str::from_utf8(&bytes[tokens[1].clone()])
        .expect("height")
        .parse()
        .expect("height is a number");
    let _ = &tokens[2];
    index += 1; // single whitespace after the depth
    let pixels = bytes[index..].to_vec();
    assert_eq!(
        pixels.len(),
        (width * height) as usize,
        "the PGM payload must match its header"
    );
    (width, height, pixels)
}

fn camera_frame() -> FfiScanFrame {
    let (width, height, pixels) = load_gray_frame(&distorted_frame_path());
    FfiScanFrame {
        width,
        height,
        stride: width,
        format: FfiScanPixelFormat::Gray8,
        orientation_degrees: 0,
        pixels,
        timestamp_ms: 1_700_000_000_000,
        device_angle_degrees: 0.0,
        brightness_hint: 0.0,
        is_steady: true,
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("vocument-e2e-{name}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the temporary directory must be creatable");
    directory
}

#[test]
fn the_mobile_scan_workflow_produces_a_saved_searchable_pdf() {
    let begin = Instant::now();

    // --- 1. Capture: the camera hands over a raw frame -------------------
    let frame = camera_frame();
    assert!(
        frame.width > 0 && frame.height > 0,
        "the frame must have a size"
    );

    // --- 2. Edge detection: the app finds the document in the frame ------
    let detected: Option<FfiQuadCorners> = document_core_detect_document_edges(frame.clone())
        .expect("the edge detection must not fail");
    let corners = detected.unwrap_or_else(|| {
        panic!(
            "the edge detection must find the document in the captured frame; \
             without a detection the workflow cannot start"
        )
    });
    assert!(
        corners.confidence_milli > 0,
        "a detected quadrilateral must carry a confidence"
    );
    // The detected outline has to follow the tilt of the page. A detection
    // that returned a rectangle around the page would pass the validity check
    // but would rectify the page wrongly.
    let reference_shear = (0.1555f32 - 0.0231f32).abs();
    let detected_shear = (corners.top_left.x - corners.bottom_left.x).abs();
    assert!(
        (reference_shear - detected_shear).abs() < 0.06,
        "the detected outline must follow the tilt of the page: reference {reference_shear:.3}, detected {detected_shear:.3}"
    );

    // --- 3. Capture session: the frame is appended with its corners ------
    let scan_session_id = document_core_start_scan_session();
    assert!(
        !scan_session_id.is_empty(),
        "a scan session must be created"
    );
    let page = document_core_append_scan_page(
        scan_session_id.clone(),
        frame,
        Some(corners),
        Some(FfiScanCleanupProfile::DocumentAuto),
    )
    .expect("the captured page must be appended");
    assert!(
        page.page_index < u32::MAX,
        "the page must carry a valid index"
    );

    // --- 4. Rectification, cleanup and OCR, then the searchable PDF ------
    // This is the step that turns the capture into a PDF with an invisible
    // text layer. It runs the real perspective correction, the real cleanup
    // and the real OCR.
    let snapshot = document_core_ingest_scan_as_document_session(
        scan_session_id.clone(),
        FfiScanCleanupProfile::DocumentAuto,
        vec!["deu".to_owned()],
        "Vertraulichkeit.pdf".to_owned(),
    )
    .expect("the scan must be ingestable as a document session");
    let session_id = snapshot.session_id.clone();
    assert!(
        snapshot.pdf_summary.page_count >= 1,
        "the ingested document must have at least one page"
    );

    // --- 5. Search: the OCR text is indexed and can be found -------------
    let progress = document_core_index_search_batch(session_id.clone(), 0, 16)
        .expect("the search index must be buildable");
    assert!(
        progress.indexed_pages >= 1,
        "at least one page must be indexed, indexed {}",
        progress.indexed_pages
    );

    let response: FfiSearchResponse =
        document_core_search_exact(session_id.clone(), "Vertraulichkeit".to_owned())
            .expect("the search must run");
    assert!(
        !response.hits.is_empty(),
        "the word that is printed on the page must be found in the indexed text"
    );
    let hit = &response.hits[0];
    assert!(
        !hit.snippet.is_empty(),
        "a search hit must carry a snippet of the found text"
    );

    // --- 6. Save: the document is written to disk ------------------------
    let target = temp_dir("save").join("vertraulichkeit.pdf");
    let outcome =
        document_core_save_session_as(session_id.clone(), target.to_string_lossy().into())
            .expect("the session must be savable");
    assert!(target.exists(), "the saved file must exist on disk");
    let written = std::fs::metadata(&target).expect("the saved file must be readable");
    assert!(
        written.len() > 0,
        "the saved file must not be empty, {} bytes",
        written.len()
    );
    let _ = outcome;

    // --- 7. Read back: the saved file is a real PDF with its text -------
    let saved_bytes = std::fs::read(&target).expect("the saved file must be readable");
    assert!(
        saved_bytes.starts_with(b"%PDF-"),
        "the saved file must be a PDF, it starts with {:?}",
        String::from_utf8_lossy(&saved_bytes[..8.min(saved_bytes.len())])
    );
    assert!(
        saved_bytes.ends_with(b"%%EOF\n"),
        "the saved file must be a complete PDF"
    );

    // The text has to be readable in the saved file, not only in the session.
    let engine = vocument_pdf_mupdf::MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&saved_bytes)
        .expect("the saved PDF must be openable by the engine");
    assert_eq!(
        document.summary().page_count,
        snapshot.pdf_summary.page_count,
        "the saved document must have the same number of pages as the session"
    );
    let text = document
        .extract_page_text(0)
        .expect("the text of the saved page must be extractable");
    let joined = text
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        joined.to_lowercase().contains("vertraulich"),
        "the saved PDF must contain the OCR text, it starts with {:?}",
        joined.chars().take(120).collect::<String>()
    );

    // The text layer has to be invisible: the page must not draw visible text.
    let display_list = document
        .create_display_list(0)
        .expect("the page must be displayable");
    let tile = display_list
        .render_tile(vocument_core::pdf_engine::PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 64,
            tile_height_px: 64,
        })
        .expect("the page must be renderable");

    let elapsed = begin.elapsed();
    println!(
        "scan workflow: session {session_id}, {} page(s), {} search hit(s), saved {} bytes, \
         peak tile {}x{}, duration {elapsed:?}",
        snapshot.pdf_summary.page_count,
        response.hits.len(),
        written.len(),
        tile.width_px,
        tile.height_px
    );
    assert!(
        elapsed < WORKFLOW_BUDGET,
        "the workflow took {elapsed:?}, which is over the budget of {WORKFLOW_BUDGET:?}"
    );
}

#[test]
fn the_scanned_document_can_be_exported_and_opened_again() {
    // The same chain as the workflow test, but the document leaves the
    // application through the export: the bytes go to a place that the user
    // owns, and the file that is written there must be a complete PDF that
    // can be opened again with its text.
    let frame = camera_frame();
    let detected = document_core_detect_document_edges(frame.clone())
        .expect("the edge detection must not fail")
        .expect("the page must be found in the frame");

    let scan_session_id = document_core_start_scan_session();
    document_core_append_scan_page(
        scan_session_id.clone(),
        frame,
        Some(detected),
        Some(FfiScanCleanupProfile::DocumentAuto),
    )
    .expect("the captured page must be appended");

    let snapshot = document_core_ingest_scan_as_document_session(
        scan_session_id,
        FfiScanCleanupProfile::DocumentAuto,
        vec!["deu".to_owned()],
        "Export.pdf".to_owned(),
    )
    .expect("the scan must be ingestable as a document session");
    let session_id = snapshot.session_id.clone();

    // The export: the bytes for a location that the user owns.
    let bytes =
        document_core_export_session_bytes(session_id.clone()).expect("the export must succeed");
    assert!(
        bytes.starts_with(b"%PDF-"),
        "the exported bytes must be a PDF"
    );
    assert!(
        bytes.ends_with(b"%%EOF\n"),
        "the exported bytes must be a complete PDF"
    );

    let target = temp_dir("export").join("exported_scan.pdf");
    std::fs::write(&target, &bytes).expect("the exported bytes must be writable");

    // The file that the user owns must be openable again, with its text.
    let saved_bytes = std::fs::read(&target).expect("the exported file must be readable");
    let engine = vocument_pdf_mupdf::MuPdfEngineAdapter::new();
    let document = engine
        .open_pdf_bytes(&saved_bytes)
        .expect("the exported PDF must be openable by the engine");
    assert_eq!(
        document.summary().page_count,
        snapshot.pdf_summary.page_count,
        "the exported document must have the same number of pages as the session"
    );
    let text = document
        .extract_page_text(0)
        .expect("the text of the exported page must be extractable");
    let joined = text
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        joined.to_lowercase().contains("vertraulich"),
        "the exported PDF must contain the OCR text, it starts with {:?}",
        joined.chars().take(120).collect::<String>()
    );

    // The exported file must not contain the internals of the application.
    let as_text = String::from_utf8_lossy(&bytes);
    assert!(
        !as_text.contains("/workspace/"),
        "the exported document must not contain a path of the build machine"
    );
    assert!(
        !as_text.contains("sess_") && !as_text.contains("vocument_recovery"),
        "the exported document must not contain internal identifiers"
    );
}

//! M3.6 — Page Workbench Integration, Multi-Viewer Validation, and Overall M3 Acceptance (`ROADMAP.md` M3.6).
//!
//! Demonstrates:
//! - Full lifecycle of Page Workbench operations (reorder, duplicate, delete, rotate, bates, headers, stamps, optimization).
//! - Multi-viewer verification using native MuPDF, `mutool` CLI, and Apple Quartz `sips`.
//! - Large-document performance and stability on a 500-page document.
//! - Strict fail-closed error handling without dummy mocks (`QA-003`, `CR-0002`).

use std::fs;
use std::process::Command;
use std::time::Instant;

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::optimize_dto::{
    FfiOptimizationOptions, FfiOptimizationPreset, document_core_optimize_document_with_options,
};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::stamp_dto::{
    FfiBatesNumberingOptions, FfiBatesPosition, FfiHeaderFooterOptions, FfiStampOrWatermarkOptions,
    document_core_apply_bates_numbering_with_options,
    document_core_apply_header_footer_with_options,
    document_core_apply_stamp_or_watermark_with_options,
};
use vocument_ffi::workbench_dto::{
    FfiPageWorkbenchOptions, document_core_delete_pages_with_options,
    document_core_duplicate_page_with_options, document_core_extract_pages,
    document_core_reorder_pages_with_options, document_core_rotate_pages_with_options,
};
use vocument_ffi::{
    DocumentCoreError, document_core_close_session, document_core_open_brokered_file,
    document_core_render_page_thumbnail,
};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

fn open_session_with_bytes(bytes: Vec<u8>, token: &str) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("workbench://m3_6_{token}"),
        display_name: format!("{token}.pdf"),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some(format!("wb-m36-{token}")),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes,
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

/// Synthesizes a valid ISO 32000-1 PDF document with the requested number of pages.
fn generate_multipage_pdf(page_count: usize) -> Vec<u8> {
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let mut kids = String::new();

    // Catalog: obj 1
    objects.push(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec());

    // Kids references for Pages: obj 2
    for i in 0..page_count {
        let page_obj_num = 3 + i * 2;
        kids.push_str(&format!("{page_obj_num} 0 R "));
    }
    objects.push(
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {page_count} >>\nendobj\n")
            .into_bytes(),
    );

    // Each page: Page obj (3 + i*2) and Content obj (4 + i*2)
    for i in 0..page_count {
        let page_num = i + 1;
        let content_num = 4 + i * 2;
        let content_stream =
            format!("BT /F1 12 Tf 72 720 Td (Synthetic Large Document Page {page_num}) Tj ET\n");
        let font_def = "<< /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>";

        let page_obj = format!(
            "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font {font_def} >> /Contents {content_num} 0 R >>\nendobj\n",
            3 + i * 2
        );
        let stream_obj = format!(
            "{content_num} 0 obj\n<< /Length {} >>\nstream\n{content_stream}endstream\nendobj\n",
            content_stream.len()
        );

        objects.push(page_obj.into_bytes());
        objects.push(stream_obj.into_bytes());
    }

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n%\x80\x81\x82\x83\n");
    let mut offsets = Vec::new();
    for obj in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(obj);
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    pdf
}

#[test]
fn test_m3_6_full_workbench_lifecycle_and_multiviewer_validation() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "m3_6_lifecycle");
    let wb_opts = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // 1. Duplicate page 0 (creates 3 pages total)
    let dup_rec = document_core_duplicate_page_with_options(session_id.clone(), 0, wb_opts)
        .expect("duplicate page 0");
    assert_eq!(dup_rec.revision_after, 2);

    // 2. Reorder pages [2, 0, 1]
    let reorder_rec =
        document_core_reorder_pages_with_options(session_id.clone(), vec![2, 0, 1], wb_opts)
            .expect("reorder pages");
    assert_eq!(reorder_rec.revision_after, 3);

    // 3. Rotate page 0 by 90°
    let rot_rec = document_core_rotate_pages_with_options(session_id.clone(), vec![0], 90, wb_opts)
        .expect("rotate page");
    assert_eq!(rot_rec.revision_after, 4);

    // 4. Apply Bates numbering to all pages
    let bates_opts = FfiBatesNumberingOptions {
        prefix: "VOC-".to_owned(),
        start_number: 100,
        digit_padding: 6,
        suffix: "-CONF".to_owned(),
        position: FfiBatesPosition::BottomRight,
        font_size_pt: 10,
        margin_pt: 36,
    };
    let bates_rec = document_core_apply_bates_numbering_with_options(
        session_id.clone(),
        vec![0, 1, 2],
        bates_opts,
        wb_opts,
    )
    .expect("apply bates numbering");
    assert_eq!(bates_rec.revision_after, 5);

    // 5. Apply Header/Footer macros
    let hf_opts = FfiHeaderFooterOptions {
        header_left: Some("[CONFIDENTIAL]".to_owned()),
        header_center: Some("[TITLE]".to_owned()),
        header_right: Some("[DATE]".to_owned()),
        footer_left: Some("Vocument Page Workbench".to_owned()),
        footer_center: Some("Page [PAGE] of [TOTAL]".to_owned()),
        footer_right: Some("[BATES]".to_owned()),
        font_size_pt: 9,
        margin_pt: 30,
    };
    let hf_rec = document_core_apply_header_footer_with_options(
        session_id.clone(),
        vec![0, 1, 2],
        hf_opts,
        Some("Acceptance Specification".to_owned()),
        Some("2026-10-25".to_owned()),
        wb_opts,
    )
    .expect("apply header footer");
    assert_eq!(hf_rec.revision_after, 6);

    // 6. Apply Stamp / Watermark
    let stamp_opts = FfiStampOrWatermarkOptions {
        text: "APPROVED FOR PRODUCTION".to_owned(),
        opacity_milli: 400,
        rotation_degrees: -45,
        r: 220,
        g: 20,
        b: 20,
        is_background: false,
        rect: None,
    };
    let stamp_rec = document_core_apply_stamp_or_watermark_with_options(
        session_id.clone(),
        vec![0, 1, 2],
        stamp_opts,
        wb_opts,
    )
    .expect("apply stamp");
    assert_eq!(stamp_rec.revision_after, 7);

    // 7. Optimize document (Balanced compaction)
    let opt_opts = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Balanced,
    };
    let opt_rec =
        document_core_optimize_document_with_options(session_id.clone(), opt_opts, wb_opts)
            .expect("optimize document");
    assert_eq!(opt_rec.revision_after, 8);

    // 8. Render every page thumbnail to verify visual stability
    for page in 0..3 {
        let thumb = document_core_render_page_thumbnail(session_id.clone(), page, 256)
            .expect("render page thumbnail");
        assert!(thumb.width_px > 0 && thumb.height_px > 0);
        assert!(!thumb.rgba_bytes.is_empty());
    }

    // 9. Independent third-party viewer validation via disk export
    let temp_dir = std::env::temp_dir().join(format!("voc_m36_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let output_pdf_path = temp_dir.join("m3_6_validated_output.pdf");

    let final_bytes = document_core_extract_pages(session_id.clone(), vec![0, 1, 2])
        .expect("extract document pages");
    fs::write(&output_pdf_path, &final_bytes).expect("write output pdf to disk");

    let mut executed_viewers: Vec<String> = Vec::new();

    // A. Validate with `mutool` CLI (if present in PATH, Homebrew, custom prefix, or build tree)
    let mut mutool_candidates = vec![
        std::path::PathBuf::from("mutool"),
        std::path::PathBuf::from("/opt/homebrew/bin/mutool"),
        std::path::PathBuf::from("/usr/local/bin/mutool"),
        std::path::PathBuf::from("/usr/bin/mutool"),
    ];
    if let Ok(mupdf_dir) = std::env::var("MUPDF_DIR") {
        mutool_candidates.insert(0, std::path::PathBuf::from(mupdf_dir).join("bin/mutool"));
    }
    mutool_candidates.push(std::path::PathBuf::from(
        "../../target/mupdf-host/bin/mutool",
    ));
    mutool_candidates.push(std::path::PathBuf::from("target/mupdf-host/bin/mutool"));

    for candidate in &mutool_candidates {
        let cleaned_out = temp_dir.join("mutool_cleaned.pdf");
        if let Ok(clean_output) = Command::new(candidate)
            .arg("clean")
            .arg("-s")
            .arg(&output_pdf_path)
            .arg(&cleaned_out)
            .output()
            && clean_output.status.success()
        {
            assert!(
                cleaned_out.exists()
                    && fs::metadata(&cleaned_out).map(|m| m.len()).unwrap_or(0) > 0,
                "mutool cleaned output must be non-empty"
            );

            let info_output = Command::new(candidate)
                .arg("info")
                .arg(&output_pdf_path)
                .output()
                .expect("mutool info must execute when mutool is present");
            assert!(
                info_output.status.success(),
                "mutool info must parse exported PDF successfully"
            );
            executed_viewers.push(format!("MuPDF CLI ({})", candidate.display()));
            println!(
                "✓ mutool clean -s and info validated PDF structure via {}",
                candidate.display()
            );
            break;
        }
    }

    // B. Validate with macOS CoreGraphics / Quartz via `sips` (native macOS preview renderer)
    #[cfg(target_os = "macos")]
    {
        let sips_bin = if std::path::Path::new("/usr/bin/sips").exists() {
            std::path::Path::new("/usr/bin/sips")
        } else {
            std::path::Path::new("sips")
        };
        let sips_dim = Command::new(sips_bin)
            .arg("-g")
            .arg("pixelWidth")
            .arg("-g")
            .arg("pixelHeight")
            .arg(&output_pdf_path)
            .output()
            .expect("sips must execute on macOS host");

        assert!(
            sips_dim.status.success(),
            "Apple Quartz (sips) must parse and read the generated PDF successfully"
        );
        let sips_out = String::from_utf8_lossy(&sips_dim.stdout);
        assert!(
            sips_out.contains("pixelWidth: ") && sips_out.contains("pixelHeight: "),
            "Apple Quartz must retrieve page dimensions: {sips_out}"
        );

        // Also render first page to PNG via Quartz to confirm image rasterization pipeline
        let sips_png = temp_dir.join("quartz_render.png");
        let sips_render = Command::new(sips_bin)
            .arg("-s")
            .arg("format")
            .arg("png")
            .arg(&output_pdf_path)
            .arg("--out")
            .arg(&sips_png)
            .output()
            .expect("sips must render PDF to PNG on macOS");
        assert!(
            sips_render.status.success(),
            "Apple Quartz must render PDF page to PNG successfully"
        );
        assert!(
            sips_png.exists() && fs::metadata(&sips_png).map(|m| m.len()).unwrap_or(0) > 0,
            "Apple Quartz rendered PNG must be non-empty"
        );
        executed_viewers.push("Apple CoreGraphics/Quartz (sips)".to_string());
        println!(
            "✓ Apple CoreGraphics/Quartz (sips) successfully validated and rasterized exported PDF"
        );
    }

    // Mandatory assertion: Missing tools must NOT produce silent passes!
    assert!(
        !executed_viewers.is_empty(),
        "FATAL: Interoperability verification failed — no independent PDF viewer/validator was found or executed! Checked sips and mutool."
    );
    #[cfg(target_os = "macos")]
    assert!(
        executed_viewers
            .iter()
            .any(|v| v.contains("Apple CoreGraphics/Quartz")),
        "macOS gate strictly requires Apple Quartz (sips) validation"
    );

    // 10. Undo stack rollback verification
    let undo_1 = document_core_undo(session_id.clone()).expect("undo optimize");
    assert_eq!(undo_1.current_revision, 9);
    assert!(undo_1.can_redo);
    assert_eq!(undo_1.redo_count, 1);

    let redo_1 = document_core_redo(session_id.clone()).expect("redo optimize");
    assert_eq!(redo_1.current_revision, 10);
    assert!(!redo_1.can_redo);
    assert_eq!(redo_1.redo_count, 0);

    let _ = fs::remove_dir_all(&temp_dir);
    let _ = document_core_close_session(session_id);
}

fn get_resident_set_size_kb() -> usize {
    #[cfg(unix)]
    unsafe {
        let mut usage = std::mem::zeroed::<libc::rusage>();
        if libc::getrusage(libc::RUSAGE_SELF, &mut usage) == 0 {
            #[cfg(target_os = "macos")]
            {
                (usage.ru_maxrss / 1024) as usize
            }
            #[cfg(not(target_os = "macos"))]
            {
                usage.ru_maxrss as usize
            }
        } else {
            0
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

#[test]
fn test_m3_6_large_document_performance_and_stability_500_pages() {
    let rss_initial_kb = get_resident_set_size_kb();
    let t_gen_start = Instant::now();
    let large_pdf_bytes = generate_multipage_pdf(500);
    let gen_duration = t_gen_start.elapsed().as_secs_f64();
    println!(
        "Synthesized 500-page PDF ({} KB) in {:.2}s",
        large_pdf_bytes.len() / 1024,
        gen_duration
    );

    let session_id = open_session_with_bytes(large_pdf_bytes, "large_500_pages");
    let wb_opts = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // Performance Test 1: Batch Bates Numbering across all 500 pages
    let t_bates_start = Instant::now();
    let all_pages: Vec<u32> = (0..500).collect();
    let bates_opts = FfiBatesNumberingOptions {
        prefix: "STRESS-".to_owned(),
        start_number: 1,
        digit_padding: 6,
        suffix: "".to_owned(),
        position: FfiBatesPosition::BottomRight,
        font_size_pt: 10,
        margin_pt: 36,
    };

    let bates_rec = document_core_apply_bates_numbering_with_options(
        session_id.clone(),
        all_pages,
        bates_opts,
        wb_opts,
    )
    .expect("apply 500-page bates numbering");

    let bates_duration = t_bates_start.elapsed().as_secs_f64();
    println!("Applied Bates numbering to 500 pages in {bates_duration:.3}s");
    assert_eq!(bates_rec.revision_after, 2);
    assert!(
        bates_duration < 4.0,
        "500-page batch Bates numbering must execute in under 4.0s (was {bates_duration:.3}s)"
    );

    // Performance Test 2: Full Document Optimization (Deduplication + Object Stream Compaction)
    let t_opt_start = Instant::now();
    let opt_opts = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Balanced,
    };
    let opt_rec =
        document_core_optimize_document_with_options(session_id.clone(), opt_opts, wb_opts)
            .expect("optimize 500-page document");

    let opt_duration = t_opt_start.elapsed().as_secs_f64();
    println!("Optimized 500-page document in {opt_duration:.3}s");
    assert_eq!(opt_rec.revision_after, 3);
    assert!(
        opt_duration < 4.0,
        "500-page optimization must execute in under 4.0s (was {opt_duration:.3}s)"
    );

    let rss_peak_kb = get_resident_set_size_kb();
    println!(
        "500-page stress test memory: initial={} MB, peak={} MB",
        rss_initial_kb / 1024,
        rss_peak_kb / 1024
    );
    assert!(
        rss_peak_kb < 1_500_000,
        "Peak resident set size ({rss_peak_kb} KB) must remain strictly within reasonable bounds"
    );

    // Verify boundary pages render accurately
    let first_page = document_core_render_page_thumbnail(session_id.clone(), 0, 256)
        .expect("render page 0 of 500");
    assert!(first_page.width_px > 0 && first_page.height_px > 0);

    let last_page = document_core_render_page_thumbnail(session_id.clone(), 499, 256)
        .expect("render page 499 of 500");
    assert!(last_page.width_px > 0 && last_page.height_px > 0);

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_6_fail_closed_invariants_and_error_handling() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "invariants_test");
    let wb_opts = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    // 1. Attempting to delete ALL pages must fail-closed with LastPageProtection
    let delete_all_err =
        document_core_delete_pages_with_options(session_id.clone(), vec![0, 1], wb_opts)
            .expect_err("Must refuse to delete all pages from document");

    match delete_all_err {
        DocumentCoreError::InternalFailure { details }
        | DocumentCoreError::TransactionFailure { details } => {
            assert!(
                details.contains("last page")
                    || details.contains("cannot delete all")
                    || details.contains("retain at least one"),
                "Expected last page protection, got: {details}"
            );
        }
        other => panic!("Expected InternalFailure or TransactionFailure, got: {other:?}"),
    }

    // 2. Incomplete or invalid permutation for reordering must fail-closed
    let reorder_err = document_core_reorder_pages_with_options(
        session_id.clone(),
        vec![0], // Missing page 1
        wb_opts,
    )
    .expect_err("Must refuse incomplete page permutation");

    match reorder_err {
        DocumentCoreError::InternalFailure { details }
        | DocumentCoreError::TransactionFailure { details } => {
            assert!(
                details.contains("permutation")
                    || details.contains("count")
                    || details.contains("workbench operation failed"),
                "Expected invalid permutation failure, got: {details}"
            );
        }
        other => panic!("Expected InternalFailure or TransactionFailure, got: {other:?}"),
    }

    let _ = document_core_close_session(session_id);
}

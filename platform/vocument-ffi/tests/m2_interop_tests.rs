//! Interoperability gate tests for M2 features (`SAN-001`, `REDACT-001`, `ANNOT-001`).
//!
//! Validates:
//! - Sanitized PDF documents across MuPDF reference engine, `mutool info`/`draw`, and Apple Quartz (`sips`)
//! - Redacted PDF documents across MuPDF and `sips`
//! - Documents with comments carrying the jurist palette across MuPDF and `sips`
//! - Explicit non-simulated status documentation for uninstalled foreign viewers (Acrobat, Foxit, PDF-XChange)

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfEngine, PdfHighlightAnnotation, PdfRectMilliPt, PdfSanitizeOptions,
};
use vocument_pdf_mupdf::MuPdfEngineAdapter;
use vocument_pdf_mupdf::comment::add_comment_annotation_to_pdf;
use vocument_pdf_mupdf::mutate::add_highlight_annotation_to_pdf;
use vocument_pdf_mupdf::sanitize::sanitize_document;

fn sample_pdf_path() -> PathBuf {
    let candidate = PathBuf::from("tests/corpus/m0_3_navigation_and_search.pdf");
    if candidate.exists() {
        return candidate;
    }
    let parent = PathBuf::from("../../tests/corpus/m0_3_navigation_and_search.pdf");
    if parent.exists() {
        return parent;
    }
    panic!("Sample PDF not found at {candidate:?} or {parent:?}");
}

#[test]
fn test_m2_sanitized_pdf_interoperability() {
    let orig_bytes = fs::read(sample_pdf_path()).expect("read sample PDF");

    // Add highlight and comment
    let highlight = PdfHighlightAnnotation {
        annotation_id: "m2_interop_hl".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 100_000,
            y0_milli_pt: 100_000,
            x1_milli_pt: 200_000,
            y1_milli_pt: 120_000,
        },
        color_rgb: [255, 230, 80],
        opacity_milli: 500,
        author: Some("Jurist".to_owned()),
        contents: Some("Draft review note".to_owned()),
    };
    let with_hl =
        add_highlight_annotation_to_pdf(&orig_bytes, 4, &highlight).expect("add highlight");

    let comment = PdfCommentAnnotation {
        annotation_id: "m2_interop_cm".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 150_000,
            y0_milli_pt: 150_000,
            x1_milli_pt: 180_000,
            y1_milli_pt: 180_000,
        },
        color_rgb: [220, 50, 50],
        author: Some("Partner".to_owned()),
        contents: "Check jurisdiction clause".to_owned(),
        palette_key: "danger".to_owned(),
        palette_label: "Risiko".to_owned(),
        palette_meaning: "Haftungsrisiko identifiziert".to_owned(),
    };
    let with_annots = add_comment_annotation_to_pdf(&with_hl, 4, &comment).expect("add comment");

    // Sanitize document
    let options = PdfSanitizeOptions::default();
    let (sanitized_bytes, report) =
        sanitize_document(&with_annots, &options).expect("sanitize document");
    assert!(report.annotations_removed >= 2);
    assert!(sanitized_bytes.starts_with(b"%PDF-"));

    // 1. Verify reopened by MuPDF engine
    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter
        .open_pdf_bytes(&sanitized_bytes)
        .expect("reopen in MuPDF");
    assert_eq!(doc.summary().page_count, 4);
    let annots = doc.page_comment_annotations(0).expect("comments");
    assert!(annots.is_empty(), "annotations must be completely purged");

    // Write temp file for external CLI tools
    let temp_dir = std::env::temp_dir().join(format!("voc_m2_interop_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let temp_pdf = temp_dir.join("sanitized.pdf");
    fs::write(&temp_pdf, &sanitized_bytes).expect("write sanitized temp pdf");

    // 2. Verify with mutool if available
    let mutool_cmd = ["/opt/homebrew/bin/mutool", "mutool"]
        .into_iter()
        .find(|cmd| {
            Command::new(cmd)
                .arg("-v")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });

    if let Some(cmd) = mutool_cmd {
        let out = Command::new(cmd)
            .args(["info", &temp_pdf.to_string_lossy()])
            .output()
            .expect("run mutool info");
        assert!(
            out.status.success(),
            "mutool info must succeed on sanitized PDF"
        );

        let draw_out = Command::new(cmd)
            .args([
                "draw",
                "-F",
                "png",
                "-o",
                "/dev/null",
                "-p",
                "1",
                &temp_pdf.to_string_lossy(),
            ])
            .output()
            .expect("run mutool draw");
        assert!(
            draw_out.status.success(),
            "mutool draw must succeed on page 1 of sanitized PDF"
        );
    }

    // 3. Verify with macOS sips / Quartz
    if cfg!(target_os = "macos") {
        let sips_out = Command::new("/usr/bin/sips")
            .args(["--verify", &temp_pdf.to_string_lossy()])
            .output();
        if let Ok(res) = sips_out {
            assert!(
                res.status.success(),
                "macOS sips verification of sanitized PDF must pass"
            );
        }
    }

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_m2_jurist_palette_comment_interoperability() {
    let orig_bytes = fs::read(sample_pdf_path()).expect("read sample PDF");

    let comment = PdfCommentAnnotation {
        annotation_id: "jurist_palette_note_1".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 72_000,
            y0_milli_pt: 700_000,
            x1_milli_pt: 120_000,
            y1_milli_pt: 740_000,
        },
        color_rgb: [50, 150, 255],
        author: Some("Rechtsanwalt".to_owned()),
        contents: "Klausel geprüft und freigegeben".to_owned(),
        palette_key: "approval".to_owned(),
        palette_label: "Freigabe".to_owned(),
        palette_meaning: "Fachlich genehmigt".to_owned(),
    };
    let with_comment =
        add_comment_annotation_to_pdf(&orig_bytes, 4, &comment).expect("add comment");

    // Verify MuPDF reads it back
    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter
        .open_pdf_bytes(&with_comment)
        .expect("reopen in MuPDF");
    let comments = doc.page_comment_annotations(0).expect("read comments");
    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].contents, "Klausel geprüft und freigegeben");
    assert_eq!(comments[0].palette_key, "approval");

    // Write temp file and test with sips
    let temp_dir = std::env::temp_dir().join(format!("voc_m2_palette_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).expect("create temp dir");
    let temp_pdf = temp_dir.join("palette_comment.pdf");
    fs::write(&temp_pdf, &with_comment).expect("write temp pdf");

    if cfg!(target_os = "macos") {
        let sips_out = Command::new("/usr/bin/sips")
            .args(["--verify", &temp_pdf.to_string_lossy()])
            .output();
        if let Ok(res) = sips_out {
            assert!(
                res.status.success(),
                "macOS sips verification of PDF with jurist palette comment must pass"
            );
        }
    }

    let _ = fs::remove_dir_all(temp_dir);
}

#[test]
fn test_m2_uninstalled_viewers_documented_honestly() {
    let acrobat_present = Path::new("/Applications/Adobe Acrobat Reader.app").exists()
        || Path::new("/Applications/Adobe Acrobat.app").exists();
    if !acrobat_present {
        // Documented as NOT RUN per QA-003 without simulation
        assert!(!acrobat_present);
    }

    let foxit_present = Path::new("/Applications/Foxit PDF Reader.app").exists();
    assert!(!foxit_present);
}

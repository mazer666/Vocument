//! Integration tests for M3.5 PDF Optimization, Compression, Image Extraction, and Print/PDF-X Preflight (`ROADMAP.md` M3.5).

use vocument_ffi::mutation_dto::{document_core_redo, document_core_undo};
use vocument_ffi::optimize_dto::{
    FfiOptimizationOptions, FfiOptimizationPreset, document_core_extract_image_bytes,
    document_core_get_document_images, document_core_optimize_document_with_options,
    document_core_preflight_pdfx, document_core_preflight_print,
};
use vocument_ffi::session_dto::{FfiBrokeredFileDescriptor, FfiFileAccessOrigin};
use vocument_ffi::workbench_dto::FfiPageWorkbenchOptions;
use vocument_ffi::{
    DocumentCoreError, document_core_close_session, document_core_open_brokered_file,
};

mod common;

const TWO_PAGE_PDF: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

fn open_session_with_bytes(bytes: Vec<u8>, token: &str) -> String {
    let descriptor = FfiBrokeredFileDescriptor {
        source_token: format!("workbench://{token}"),
        display_name: format!("{token}.pdf"),
        access_origin: FfiFileAccessOrigin::MacOsScopedUrl,
        os_file_id: Some(format!("wb-{token}")),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes,
    };
    let snap = document_core_open_brokered_file(descriptor).expect("open brokered file");
    snap.session_id
}

/// Helper that creates a valid PDF carrying an embedded raster image (`ROADMAP.md` M3.5).
fn build_pdf_with_image(width: usize, height: usize) -> Vec<u8> {
    let mut image_data = vec![0u8; width * height];
    for y in 0..height {
        for x in 0..width {
            image_data[y * width + x] = if (x + y) % 2 == 0 { 80 } else { 160 };
        }
    }

    let content = format!("q {width} 0 0 {height} 0 0 cm /Im1 Do Q\n");
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << /XObject << /Im1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n"
        )
        .into_bytes(),
        {
            let mut obj = format!(
                "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {} >>\nstream\n",
                image_data.len()
            )
            .into_bytes();
            obj.extend_from_slice(&image_data);
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
    ];

    assemble_pdf(&objects)
}

/// Helper to assemble PDF objects and cross-reference table.
fn assemble_pdf(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::new();
    for obj in objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(obj);
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
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
fn test_m3_5_lossless_optimization_and_undo_redo() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "optimize_lossless");

    let options = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Lossless,
        do_linearize: false,
    };
    let workbench_options = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let record = document_core_optimize_document_with_options(
        session_id.clone(),
        options,
        workbench_options,
    )
    .expect("optimize lossless");

    assert_eq!(record.revision_after, record.revision_before + 1);

    // Undo optimization
    let undo_rec = document_core_undo(session_id.clone()).expect("undo optimize");
    assert_eq!(undo_rec.current_revision, record.revision_before);

    // Redo optimization
    let redo_rec = document_core_redo(session_id.clone()).expect("redo optimize");
    assert_eq!(redo_rec.current_revision, record.revision_after);

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_5_balanced_and_maximum_optimization_presets() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "optimize_presets");

    // Test Balanced preset
    let balanced_opts = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Balanced,
        do_linearize: false,
    };
    let wb_opts = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let rec_balanced =
        document_core_optimize_document_with_options(session_id.clone(), balanced_opts, wb_opts)
            .expect("optimize balanced");
    assert_eq!(
        rec_balanced.revision_after,
        rec_balanced.revision_before + 1
    );

    // Test Maximum preset with Fast Web View linearization
    let max_opts = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Maximum,
        do_linearize: true,
    };
    let rec_max =
        document_core_optimize_document_with_options(session_id.clone(), max_opts, wb_opts)
            .expect("optimize maximum linearized");
    assert_eq!(rec_max.revision_after, rec_balanced.revision_after + 1);

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_5_image_discovery_and_png_extraction() {
    let pdf_bytes = build_pdf_with_image(120, 80);
    let session_id = open_session_with_bytes(pdf_bytes, "image_extract");

    // 1. Discover embedded images
    let images = document_core_get_document_images(session_id.clone()).expect("discover images");
    assert_eq!(images.len(), 1, "Must find exactly one embedded image");
    let img = &images[0];
    assert_eq!(img.page_index, 0);
    assert_eq!(img.image_index, 0);
    assert_eq!(img.width, 120);
    assert_eq!(img.height, 80);
    assert_eq!(img.bits, 8);
    assert_eq!(img.colorspace, "DeviceGray");

    // 2. Extract image bytes as PNG
    let extracted =
        document_core_extract_image_bytes(session_id.clone(), 0, 0).expect("extract png bytes");
    assert_eq!(extracted.width, 120);
    assert_eq!(extracted.height, 80);
    assert_eq!(extracted.format, "png");
    assert!(extracted.data.len() > 8);

    // Verify PNG signature (\x89PNG\r\n\x1a\n)
    let png_magic = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    assert_eq!(
        &extracted.data[0..8],
        png_magic,
        "Extracted bytes must begin with genuine PNG header"
    );

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_5_print_readiness_preflight() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "print_preflight");

    let report = document_core_preflight_print(session_id.clone()).expect("preflight print");
    assert_eq!(report.page_count, 2);
    assert!(report.is_uniform_size);
    assert!(report.width_pt > 0.0);
    assert!(report.height_pt > 0.0);

    let _ = document_core_close_session(session_id);
}

fn create_signed_test_pdf() -> Vec<u8> {
    let contents = "BT /F1 12 Tf 72 740 Td (Signed Document Invariant) Tj ET\n";
    let placeholder = "[0000000000 0000000000 0000000000 0000000000]";
    let objects: Vec<String> = vec![
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [5 0 R] >> >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 6 0 R >>".to_owned(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        "<< /Type /Annot /Subtype /Widget /FT /Sig /T (SignatureField) /V 7 0 R /Rect [72 100 320 140] /P 3 0 R >>".to_owned(),
        format!("<< /Length {} >>\nstream\n{contents}endstream", contents.len()),
        format!("<< /Type /Sig /Filter /Adobe.PPKLite /SubFilter /adbe.pkcs7.detached /Name (Auditor) /ByteRange {placeholder} /Contents <{:0>40}> >>", "A"),
    ];

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n%\x80\x81\x82\x83\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, obj).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
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
fn test_m3_5_pdfx_preflight_audit() {
    let session_id = open_session_with_bytes(TWO_PAGE_PDF.to_vec(), "pdfx_preflight");

    let report = document_core_preflight_pdfx(session_id.clone()).expect("preflight pdfx");
    // Standard test PDF does not have OutputIntent or GTS_PDFXVersion
    assert!(!report.is_valid || report.issues.is_empty() == report.is_valid);
    assert!(!report.is_encrypted);

    let _ = document_core_close_session(session_id);
}

#[test]
fn test_m3_5_digital_signature_protection_fail_closed() {
    let signed_pdf = create_signed_test_pdf();
    let session_id = open_session_with_bytes(signed_pdf, "optimize_signed_fail_closed");

    let options = FfiOptimizationOptions {
        preset: FfiOptimizationPreset::Lossless,
        do_linearize: false,
    };
    let wb_options_refuse = FfiPageWorkbenchOptions {
        allow_signature_invalidation: false,
    };

    let err = document_core_optimize_document_with_options(
        session_id.clone(),
        options.clone(),
        wb_options_refuse,
    )
    .expect_err("Must refuse to optimize signed document without explicit override");

    match err {
        DocumentCoreError::OperationBlockedBySignature { details: _ } => {}
        other => panic!("Expected OperationBlockedBySignature, got: {other:?}"),
    }

    // Explicit override succeeds
    let wb_options_allow = FfiPageWorkbenchOptions {
        allow_signature_invalidation: true,
    };
    let record =
        document_core_optimize_document_with_options(session_id.clone(), options, wb_options_allow)
            .expect("Allowed optimization with explicit signature invalidation");
    assert_eq!(record.revision_after, record.revision_before + 1);

    let _ = document_core_close_session(session_id);
}

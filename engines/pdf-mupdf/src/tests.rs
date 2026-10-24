use super::*;
use std::path::PathBuf;

/// Generates a minimal, specification-valid 2-page PDF with exact xref byte offsets and Info metadata.
pub fn sample_two_page_pdf_bytes() -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n");

    let objects: [&[u8]; 5] = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n",
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>\nendobj\n",
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>\nendobj\n",
        b"5 0 obj\n<< /Title (Vocument M0.1 Test Contract) /Author (Vocument QA) /Subject (Architecture Verification) /Creator (Vocument Test Suite) /Producer (MuPDF Bridge Test) >>\nendobj\n",
    ];

    let mut offsets = Vec::with_capacity(objects.len());
    for obj in objects {
        offsets.push(out.len());
        out.extend_from_slice(obj);
    }

    let xref_offset = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn mupdf_adapter_reads_page_count_and_metadata_from_real_pdf() {
    let adapter = MuPdfEngineAdapter::new();
    let bytes = sample_two_page_pdf_bytes();
    let doc = adapter
        .open_pdf_bytes(&bytes)
        .expect("MuPDF must open valid 2-page PDF");
    let summary = doc.summary();

    assert_eq!(summary.page_count, 2);
    assert_eq!(summary.format_version.as_deref(), Some("PDF 1.7"));
    assert_eq!(
        summary.title.as_deref(),
        Some("Vocument M0.1 Test Contract")
    );
    assert_eq!(summary.author.as_deref(), Some("Vocument QA"));
    assert_eq!(
        summary.subject.as_deref(),
        Some("Architecture Verification")
    );
    assert_eq!(summary.creator.as_deref(), Some("Vocument Test Suite"));
    assert_eq!(summary.producer.as_deref(), Some("MuPDF Bridge Test"));
    assert!(!summary.needs_password);
    assert!(summary.engine_identifier.starts_with("mupdf-1."));
}

#[test]
fn mupdf_adapter_creates_display_list_and_renders_rgba8_tiles() {
    let adapter = MuPdfEngineAdapter::new();
    let bytes = sample_two_page_pdf_bytes();
    let doc = adapter.open_pdf_bytes(&bytes).expect("open 2-page PDF");

    let geom = doc.page_geometry(0).expect("page 0 geometry");
    assert_eq!(geom.page_index, 0);
    assert_eq!(geom.width_milli_pt, 595_000);
    assert_eq!(geom.height_milli_pt, 842_000);

    let dl = doc.create_display_list(0).expect("page 0 display list");
    assert_eq!(dl.page_geometry(), geom);

    let tile = dl
        .render_tile(PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 128,
            tile_height_px: 128,
        })
        .expect("render 128x128 tile at 1.0x");

    assert_eq!(tile.width_px, 128);
    assert_eq!(tile.height_px, 128);
    assert_eq!(tile.stride_bytes, 512);
    assert_eq!(tile.rgba_bytes.len(), 128 * 128 * 4);
}

#[test]
fn mupdf_adapter_extracts_embedded_page_labels_and_structured_text() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/m0_3_navigation_and_search.pdf");
    let bytes = std::fs::read(&fixture_path).expect("read M0.3 PDF fixture");
    let adapter = MuPdfEngineAdapter::new();
    let doc = adapter.open_pdf_bytes(&bytes).expect("open M0.3 PDF");

    assert_eq!(doc.summary().page_count, 4);
    assert_eq!(doc.page_label(0).expect("label 0").as_deref(), Some("i"));
    assert_eq!(doc.page_label(1).expect("label 1").as_deref(), Some("ii"));
    assert_eq!(doc.page_label(2).expect("label 2").as_deref(), Some("1"));
    assert_eq!(doc.page_label(3).expect("label 3").as_deref(), Some("2"));

    let page_1_text = doc.extract_page_text(1).expect("extract page 1 text");
    assert_eq!(page_1_text.embedded_page_label.as_deref(), Some("ii"));
    assert!(page_1_text.lines.len() >= 4);
    assert!(page_1_text.lines.iter().any(|l| l.text.contains("trust-")));
}

#[test]
fn mupdf_adapter_rejects_non_pdf_bytes() {
    let adapter = MuPdfEngineAdapter::new();
    let err = adapter
        .open_pdf_bytes(b"not a pdf file")
        .expect_err("non-PDF bytes must fail");
    assert!(matches!(err, PdfEngineError::InvalidPdf { .. }));
}

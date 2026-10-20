//! Tests of the redaction against a document that carries an image
//! (`REDACT-001`).
//!
//! A document can carry the same text twice: as a text layer and as pixels in
//! an image. For a scan the text layer is invisible and the picture carries
//! the words. A redaction that only removes the text layer would leave the
//! picture readable — and a black rectangle over the area would only hide it:
//! whoever removes the rectangle would see the text again.
//!
//! The test therefore uses a document with an **uncompressed** image whose
//! bytes carry a recognisable marker inside the redacted region. After the
//! redaction the marker must be gone from the file — the image data was
//! replaced, not covered — and the rest of the page must still be intact.

use std::path::PathBuf;

use vocument_core::pdf_engine::{PdfEngine, PdfRectMilliPt};

/// The marker that the uncompressed image stream carries inside the redacted
/// region.
///
/// Its absence in the redacted file proves that the image was rewritten.
const IMAGE_MARKER: &[u8] = b"VOCUMENT-IMAGE-PATTERN-8F2A";

/// The image of the fixture: 200 by 100 pixels, drawn in the lower half of
/// the page.
const IMAGE_WIDTH: usize = 200;
const IMAGE_HEIGHT: usize = 100;

/// The region that is redacted: the left part of the image.
///
/// In the page space of the engine (origin top left, y descending) the image
/// covers the rows 100..200 of the page.
const REDACTED_REGION: PdfRectMilliPt = PdfRectMilliPt {
    x0_milli_pt: 10_000,
    y0_milli_pt: 110_000,
    x1_milli_pt: 90_000,
    y1_milli_pt: 190_000,
};

/// Builds a PDF with a page that carries an uncompressed image and a text line.
fn document_with_image() -> Vec<u8> {
    let mut image_data = vec![0u8; IMAGE_WIDTH * IMAGE_HEIGHT];
    for y in 0..IMAGE_HEIGHT {
        for x in 0..IMAGE_WIDTH {
            image_data[y * IMAGE_WIDTH + x] = if (x + y) % 2 == 0 { 90 } else { 140 };
        }
    }
    // The marker lies inside the redacted region: in the image it is at
    // column 20, row 20 (the rows of the image are counted from the top).
    let marker_offset = 20 * IMAGE_WIDTH + 20;
    image_data[marker_offset..marker_offset + IMAGE_MARKER.len()].copy_from_slice(IMAGE_MARKER);

    let content = "q 200 0 0 100 0 0 cm /Im1 Do Q\nBT /F1 12 Tf 20 150 Td (Vertrag ueber die Geheimhaltung) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> /XObject << /Im1 4 0 R >> >> /Contents 6 0 R >>\nendobj\n".to_vec(),
        {
            let mut obj = format!(
                "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceGray /BitsPerComponent 8 /Length {} >>\nstream\n",
                IMAGE_WIDTH,
                IMAGE_HEIGHT,
                image_data.len()
            )
            .into_bytes();
            obj.extend_from_slice(&image_data);
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj =
                format!("6 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
            obj.extend_from_slice(content.as_bytes());
            obj.extend_from_slice(b"\nendstream\nendobj\n");
            obj
        },
    ];

    let mut pdf = Vec::new();
    pdf.extend_from_slice(b"%PDF-1.7\n");
    let mut offsets = Vec::new();
    for object in &objects {
        offsets.push(pdf.len());
        pdf.extend_from_slice(object);
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

fn corpus_pdf() -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/m0_3_navigation_and_search.pdf");
    std::fs::read(&path).expect("the navigation corpus must be present")
}

/// The value of a pixel of the raster.
fn pixel(raster: &vocument_core::pdf_engine::PdfPageRaster, x: u32, y: u32) -> u8 {
    raster.gray_bytes[(y as usize) * (raster.width_px as usize) + (x as usize)]
}

#[test]
fn a_redaction_replaces_the_pixels_of_the_image_instead_of_covering_them() {
    let bytes = document_with_image();
    assert!(
        bytes
            .windows(IMAGE_MARKER.len())
            .any(|window| window == IMAGE_MARKER),
        "the fixture must carry the marker in the raw file"
    );

    let engine = crate::MuPdfEngineAdapter::new();
    let mut document = engine
        .open_pdf_bytes(&bytes)
        .expect("the document with the image must be openable");

    // The picture before the redaction. The image covers the lower half of
    // the page, in the rendering the lower part of the raster.
    let before = document
        .render_page_gray(0, 1000)
        .expect("the page must be renderable");
    // Two points of the image with a different phase of the pattern, so that
    // the comparison is meaningful. The first lies in the redacted region.
    let inside_before = pixel(&before, 21, 179);
    let outside_before = pixel(&before, 180, 181);
    assert_ne!(
        inside_before, outside_before,
        "the pattern of the image must be visible in the rendering"
    );

    let redacted = document
        .redact_page_regions(0, &[REDACTED_REGION])
        .expect("the redaction must succeed");

    // The image data was replaced: the marker of the redacted region is gone
    // from the file. A black rectangle over the area would not achieve this.
    assert!(
        !redacted
            .windows(IMAGE_MARKER.len())
            .any(|window| window == IMAGE_MARKER),
        "the pixels of the image must have been replaced, not covered"
    );

    // The rest of the page survived: the image is still there outside the
    // region, and the text above it is untouched.
    let after = document
        .render_page_gray(0, 1000)
        .expect("the page must be renderable after the redaction");
    assert_eq!(
        outside_before,
        pixel(&after, 180, 181),
        "the part of the image outside the region must be unchanged"
    );
    assert_ne!(
        inside_before,
        pixel(&after, 21, 179),
        "the redacted region must not show the pattern of the image any more"
    );

    let text = document
        .extract_page_text(0)
        .expect("the text of the page must be extractable");
    let joined = text
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        joined.contains("Geheimhaltung"),
        "the text outside the redacted region must survive, it holds: {joined}"
    );
}

#[test]
fn a_redaction_of_a_text_document_keeps_the_other_pages_intact() {
    let bytes = corpus_pdf();
    let engine = crate::MuPdfEngineAdapter::new();
    let mut document = engine
        .open_pdf_bytes(&bytes)
        .expect("the corpus PDF must be openable");

    let structured = document
        .extract_page_text(0)
        .expect("the text of the page must be extractable");
    let line = structured
        .lines
        .iter()
        .find(|line| line.text.contains("Vocument Preface"))
        .expect("the headline must be in the text of the page");
    let region = PdfRectMilliPt {
        x0_milli_pt: line.bbox.x0_milli_pt,
        y0_milli_pt: line.bbox.y0_milli_pt,
        x1_milli_pt: line.bbox.x1_milli_pt,
        y1_milli_pt: line.bbox.y1_milli_pt,
    };
    let redacted = document
        .redact_page_regions(0, &[region])
        .expect("the redaction must succeed");

    let text = document
        .extract_page_text(0)
        .expect("the text of the page must be extractable");
    let joined = text
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        !joined.contains("Vocument Preface"),
        "the redacted text must be gone"
    );

    // The other pages are untouched.
    let other_page = document
        .extract_page_text(1)
        .expect("the text of the second page must be extractable");
    let other_text = other_page
        .lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        other_text.contains("Table of Contents"),
        "the other pages must be untouched, the second page holds: {other_text}"
    );
    assert!(
        !redacted.is_empty(),
        "the redacted document must not be empty"
    );
}

#[test]
fn the_redacted_region_is_empty_in_the_picture_of_the_page() {
    // The redaction does not draw a black box over the removed region: the
    // box would hide content that survived the removal. The picture of the
    // page has to show the truth, so that the verification can read it.
    let bytes = corpus_pdf();
    let engine = crate::MuPdfEngineAdapter::new();
    let mut document = engine
        .open_pdf_bytes(&bytes)
        .expect("the corpus PDF must be openable");

    let structured = document
        .extract_page_text(0)
        .expect("the text of the page must be extractable");
    let line = structured
        .lines
        .iter()
        .find(|line| line.text.contains("Vocument Preface"))
        .expect("the headline must be in the text of the page");

    // The rendering before the redaction: the headline is dark.
    let before = document
        .render_page_gray(0, 1000)
        .expect("the page must be renderable");
    let dark_before = count_dark_pixels(&before, &line.bbox);
    assert!(
        dark_before > 0,
        "the headline must be visible in the picture before the redaction"
    );

    document
        .redact_page_regions(
            0,
            &[PdfRectMilliPt {
                x0_milli_pt: line.bbox.x0_milli_pt,
                y0_milli_pt: line.bbox.y0_milli_pt,
                x1_milli_pt: line.bbox.x1_milli_pt,
                y1_milli_pt: line.bbox.y1_milli_pt,
            }],
        )
        .expect("the redaction must succeed");

    // The rendering after the redaction: the region is empty. The text is
    // gone, and nothing covers it.
    let after = document
        .render_page_gray(0, 1000)
        .expect("the page must be renderable after the redaction");
    let dark_after = count_dark_pixels(&after, &line.bbox);
    assert_eq!(
        dark_after, 0,
        "the redacted region must be empty in the picture of the page, {dark_after} dark pixel(s) are left"
    );
}

/// Counts the dark pixels inside a region of a raster.
///
/// The region is given in the page space of the engine (milli-points, origin
/// top left); the raster is rendered with the same scale of 1.0.
fn count_dark_pixels(
    raster: &vocument_core::pdf_engine::PdfPageRaster,
    region: &PdfRectMilliPt,
) -> usize {
    let x0 = (region.x0_milli_pt / 1000).max(0) as u32;
    let y0 = (region.y0_milli_pt / 1000).max(0) as u32;
    let x1 = ((region.x1_milli_pt / 1000) as u32).min(raster.width_px);
    let y1 = ((region.y1_milli_pt / 1000) as u32).min(raster.height_px);
    let mut dark = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            if pixel(raster, x, y) < 160 {
                dark += 1;
            }
        }
    }
    dark
}

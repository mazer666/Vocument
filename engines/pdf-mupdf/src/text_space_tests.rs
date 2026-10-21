//! The coordinate space of the page (`REDACT-001`, `REDACT-002`).
//!
//! Everything that places something on a page — a redaction region, the box of
//! a word, the box of a note — has to live in **one** space. If the text layer
//! and the rendered picture used different spaces, a region would hit the text
//! layer and miss the picture, and the proof over the picture would pass
//! while the word was still visible.
//!
//! The space is the **device space of the page**: the origin is the **top
//! left** corner, the y axis points **down**, and the unit is the millipoint.
//!
//! This test pins that space down with a measurement instead of a
//! convention: a word is drawn at a known place of the page, and the boxes
//! that the engine reports have to say where it is.
//!
//! * "OBEN" is drawn at `y = 170` of a page that is 200 points high — it is
//!   near the **top** of the page, so its box has to have **small** y values.
//! * "UNTEN" is drawn at `y = 60` — it is further down, so its box has to
//!   have **larger** y values.
//!
//! The rendered tile is measured the same way: the ink of the word at the top
//! of the page has to be in the **first** rows of the tile.

use vocument_core::pdf_engine::{PdfEngine, PdfTileRenderRequest};

/// A page of 200 × 200 points with two words at a known place.
fn document_with_two_words() -> Vec<u8> {
    let content = "BT /F1 18 Tf 20 170 Td (OBEN) Tj ET\nBT /F1 18 Tf 20 60 Td (UNTEN) Tj ET\n";
    let objects: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        {
            let mut obj = format!("5 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes();
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

fn open() -> Box<dyn vocument_core::pdf_engine::PdfEngineDocument> {
    let engine = crate::MuPdfEngineAdapter::new();
    engine
        .open_pdf_bytes(&document_with_two_words())
        .expect("the document must be openable")
}

#[test]
fn the_text_layer_lives_in_the_device_space_of_the_page() {
    let document = open();
    let text = document.extract_page_text(0).expect("the text layer");
    let geometry = document.page_geometry(0).expect("the geometry");
    assert_eq!(geometry.width_milli_pt, 200_000);
    assert_eq!(geometry.height_milli_pt, 200_000);

    let oben = text
        .words
        .iter()
        .find(|word| word.text == "OBEN")
        .expect("the word at the top of the page");
    let unten = text
        .words
        .iter()
        .find(|word| word.text == "UNTEN")
        .expect("the word further down");

    // The origin is the top left corner and the y axis points down: the word
    // that is drawn high on the page has the **smaller** y.
    assert!(
        oben.bbox.y1_milli_pt < unten.bbox.y0_milli_pt,
        "the word at the top of the page must have the smaller y: {oben:?} vs {unten:?}"
    );
    // "OBEN" is drawn 30 points below the top edge of the page (the glyphs
    // reach from 170 to 188 in the space of the PDF, which is measured from
    // the bottom). In the device space that is 12 to 30 points from the top.
    assert!(
        oben.bbox.y0_milli_pt < 60_000,
        "the word at the top of the page must be near the top edge: {oben:?}"
    );
    assert!(
        unten.bbox.y0_milli_pt > 100_000,
        "the word further down must be further down: {unten:?}"
    );
}

#[test]
fn the_rendered_tile_has_the_same_orientation_as_the_text_layer() {
    let document = open();
    let display_list = document.create_display_list(0).expect("the display list");
    let tile = display_list
        .render_tile(PdfTileRenderRequest {
            page_index: 0,
            scale_milli: 1000,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: 200,
            tile_height_px: 200,
        })
        .expect("the tile");
    assert_eq!(tile.width_px, 200);
    assert_eq!(tile.height_px, 200);

    // The rows of the tile that carry ink. The first row of the tile is the
    // **top** of the page — otherwise the ink of the word at the top of the
    // page would be in the last rows.
    let mut first_ink_row: Option<u32> = None;
    let mut last_ink_row: Option<u32> = None;
    for y in 0..tile.height_px {
        let start = (y as usize) * (tile.stride_bytes as usize);
        let row = &tile.rgba_bytes[start..start + (tile.width_px as usize) * 4];
        if row.chunks(4).any(|pixel| pixel[0] < 128) {
            if first_ink_row.is_none() {
                first_ink_row = Some(y);
            }
            last_ink_row = Some(y);
        }
    }
    let first = first_ink_row.expect("the page must carry ink");
    let last = last_ink_row.expect("the page must carry ink");
    assert!(
        first < 60,
        "the ink of the word at the top of the page must be in the first rows: {first}..{last}"
    );
    assert!(
        last > 100,
        "the ink of the word further down must be in the last rows: {first}..{last}"
    );
}

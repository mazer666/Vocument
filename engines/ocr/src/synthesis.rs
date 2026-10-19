//! Searchable PDF synthesis with invisible text layer behind/over clean scan image (`ROADMAP.md` M1.4, `ARCHITECTURE.md` §20).
//!
//! Synthesizes standard, fully compliant PDF/A-friendly documents embedding high-resolution
//! scanned raster images and an invisible text layer (`3 Tr`) matching OCR word bounding boxes.

use std::fmt::Write as FmtWrite;
use vocument_imaging::{PixelFormat, ScanFrame};

use crate::models::{OcrPageResult, OcrWord};
use crate::provider::OcrError;

/// Input for synthesizing a single page of a searchable PDF document.
#[derive(Debug, Clone)]
pub struct SearchablePdfPageInput {
    pub frame: ScanFrame,
    pub ocr_result: OcrPageResult,
    pub page_width_pt: Option<f32>,
    pub page_height_pt: Option<f32>,
}

/// Metadata and multi-page specifications for a searchable PDF document.
#[derive(Debug, Clone, Default)]
pub struct SearchablePdfDocument {
    pub pages: Vec<SearchablePdfPageInput>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub creator: Option<String>,
}

/// Returns the standard Helvetica character advance width in 1/1000 of an em.
pub fn helvetica_glyph_width(c: char) -> u32 {
    match c {
        ' ' | '!' | '.' | ',' | ':' | ';' | '/' => 278,
        '"' => 355,
        '#' | '$' | '?' | '0'..='9' | '§' => 556,
        '%' => 889,
        '&' => 667,
        '\'' => 191,
        '(' | ')' | '-' => 333,
        '*' => 389,
        '+' | '<' | '=' | '>' => 584,
        '@' => 1015,
        'A' | 'B' | 'E' | 'K' | 'P' | 'R' | 'S' | 'V' | 'X' | 'Y' | 'Ä' => 667,
        'C' | 'D' | 'H' | 'N' | 'U' | 'Ü' => 722,
        'F' | 'T' | 'Z' => 611,
        'G' | 'O' | 'Q' | 'Ö' => 778,
        'I' => 278,
        'J' => 500,
        'L' => 556,
        'M' => 833,
        'W' => 944,
        'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'o' | 'p' | 'q' | 'u' | 'v' | 'y' | 'ä' | 'ö' | 'ü'
        | 'ß' => 556,
        'c' | 'k' | 's' | 'x' | 'z' => 500,
        'f' | 't' => 278,
        'i' | 'j' | 'l' => 222,
        'm' => 833,
        'n' => 556,
        'r' => 333,
        'w' => 722,
        _ => 556,
    }
}

/// Converts a UTF-8 string into PDF literal string octal escapes compatible with WinAnsiEncoding.
pub fn escape_pdf_winansi_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for b in text.chars() {
        match b {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii() => {
                out.push(c);
            }
            'ä' => out.push_str("\\344"),
            'ö' => out.push_str("\\366"),
            'ü' => out.push_str("\\374"),
            'Ä' => out.push_str("\\304"),
            'Ö' => out.push_str("\\326"),
            'Ü' => out.push_str("\\334"),
            'ß' => out.push_str("\\337"),
            '§' => out.push_str("\\247"),
            '€' => out.push_str("\\200"),
            _ => {
                // Approximate unmappable non-ASCII as space or omit
                out.push(' ');
            }
        }
    }
    out
}

/// Converts arbitrary ScanFrame pixel buffers into packed RGB8 or Gray8 raster bytes for PDF embedding.
fn extract_image_bytes(frame: &ScanFrame) -> (Vec<u8>, &'static str, u32, u32) {
    let width = frame.width;
    let height = frame.height;
    match frame.format {
        PixelFormat::Gray8 => (frame.pixels.clone(), "/DeviceGray", width, height),
        PixelFormat::Rgb8 => (frame.pixels.clone(), "/DeviceRGB", width, height),
        PixelFormat::Rgba8 => {
            let mut rgb = Vec::with_capacity((width * height * 3) as usize);
            #[allow(clippy::chunks_exact_to_as_chunks)]
            for chunk in frame.pixels.chunks_exact(4) {
                rgb.push(chunk[0]);
                rgb.push(chunk[1]);
                rgb.push(chunk[2]);
            }
            (rgb, "/DeviceRGB", width, height)
        }
        PixelFormat::Nv21 | PixelFormat::Yuv420 => {
            // Convert Y plane to grayscale
            let y_size = (width * height) as usize;
            let gray = if frame.pixels.len() >= y_size {
                frame.pixels[..y_size].to_vec()
            } else {
                frame.pixels.clone()
            };
            (gray, "/DeviceGray", width, height)
        }
    }
}

/// Generates PDF content stream operators for an invisible OCR text layer matching word bounding boxes.
pub fn generate_invisible_text_stream(
    words: &[OcrWord],
    page_width_pt: f32,
    page_height_pt: f32,
) -> String {
    let mut stream = String::with_capacity(words.len() * 80 + 128);
    stream.push_str("BT\n");
    stream.push_str("3 Tr\n"); // Rendering mode 3: Neither fill nor stroke text (invisible)

    for word in words {
        if word.text.trim().is_empty() {
            continue;
        }

        let w_pt = (word.bbox.x1 - word.bbox.x0) * page_width_pt;
        let h_pt = (word.bbox.y1 - word.bbox.y0) * page_height_pt;
        if w_pt <= 0.0 || h_pt <= 0.0 {
            continue;
        }

        let x_pt = word.bbox.x0 * page_width_pt;
        // In PDF (bottom-left origin): baseline is offset slightly from bottom of bounding box
        let y_bottom_pt = (1.0 - word.bbox.y1) * page_height_pt;
        let font_size = h_pt.clamp(4.0, 72.0);
        let y_baseline_pt = y_bottom_pt + (font_size * 0.18);

        // Compute natural width of the string in Helvetica at current font size
        let natural_unscaled_units: u32 = word.text.chars().map(helvetica_glyph_width).sum();
        let natural_width_pt = (natural_unscaled_units as f32 / 1000.0) * font_size;

        let h_scale = if natural_width_pt > 0.0 {
            ((w_pt / natural_width_pt) * 100.0).clamp(20.0, 300.0)
        } else {
            100.0
        };

        let escaped = escape_pdf_winansi_string(&word.text);
        let _ = write!(
            stream,
            "/F1 {:.2} Tf\n{:.2} Tz\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\n",
            font_size, h_scale, x_pt, y_baseline_pt, escaped
        );
    }

    stream.push_str("ET\n");
    stream
}

/// Synthesizes a valid, searchable multi-page PDF document embedding scan raster frames and invisible OCR text.
pub fn synthesize_searchable_pdf(doc: &SearchablePdfDocument) -> Result<Vec<u8>, OcrError> {
    if doc.pages.is_empty() {
        return Err(OcrError::RecognitionFailed {
            details: "cannot synthesize searchable PDF with zero pages".into(),
        });
    }

    let mut pdf = Vec::with_capacity(1024 * 1024);
    let mut offsets = Vec::new();

    // 1. PDF Header
    pdf.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");

    let page_count = doc.pages.len();
    // Object numbering:
    // 1: Catalog
    // 2: Pages
    // 3: Font (F1: /Helvetica)
    // For page i (0-indexed):
    //   page_obj_num = 4 + i * 3
    //   img_obj_num  = 4 + i * 3 + 1
    //   cont_obj_num = 4 + i * 3 + 2

    // 1 0 obj: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // 2 0 obj: Pages
    offsets.push(pdf.len());
    let mut kids = String::new();
    for i in 0..page_count {
        let page_obj_num = 4 + i * 3;
        let _ = write!(kids, "{} 0 R ", page_obj_num);
    }
    let pages_dict = format!(
        "2 0 obj\n<< /Type /Pages /Kids [ {}] /Count {} >>\nendobj\n",
        kids.trim_end(),
        page_count
    );
    pdf.extend_from_slice(pages_dict.as_bytes());

    // 3 0 obj: Font (/F1 /Helvetica)
    offsets.push(pdf.len());
    pdf.extend_from_slice(
        b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );

    for (i, page_input) in doc.pages.iter().enumerate() {
        let page_obj_num = 4 + i * 3;
        let img_obj_num = page_obj_num + 1;
        let cont_obj_num = page_obj_num + 2;

        let (img_bytes, color_space, width_px, height_px) = extract_image_bytes(&page_input.frame);
        let page_w_pt = page_input.page_width_pt.unwrap_or(595.0); // Standard A4 width default
        let page_h_pt = page_input
            .page_height_pt
            .unwrap_or_else(|| page_w_pt * (height_px as f32 / width_px.max(1) as f32));

        // Page Object
        offsets.push(pdf.len());
        let page_obj = format!(
            "{page_obj_num} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 {page_w_pt:.2} {page_h_pt:.2} ] /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 {img_obj_num} 0 R >> >> /Contents {cont_obj_num} 0 R >>\nendobj\n"
        );
        pdf.extend_from_slice(page_obj.as_bytes());

        // Image Object
        offsets.push(pdf.len());
        let img_header = format!(
            "{img_obj_num} 0 obj\n<< /Type /XObject /Subtype /Image /Width {width_px} /Height {height_px} /ColorSpace {color_space} /BitsPerComponent 8 /Length {} >>\nstream\n",
            img_bytes.len()
        );
        pdf.extend_from_slice(img_header.as_bytes());
        pdf.extend_from_slice(&img_bytes);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Content Stream (Draw raster image then invisible text layer over it)
        let mut content = format!("q\n{page_w_pt:.2} 0 0 {page_h_pt:.2} 0 0 cm\n/Im1 Do\nQ\n");
        let text_layer =
            generate_invisible_text_stream(&page_input.ocr_result.words, page_w_pt, page_h_pt);
        content.push_str(&text_layer);

        offsets.push(pdf.len());
        let cont_header = format!(
            "{cont_obj_num} 0 obj\n<< /Length {} >>\nstream\n",
            content.len()
        );
        pdf.extend_from_slice(cont_header.as_bytes());
        pdf.extend_from_slice(content.as_bytes());
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Cross-Reference Table
    let startxref_offset = pdf.len();
    let total_objects = 3 + page_count * 3;
    let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", total_objects + 1);
    for offset in &offsets {
        let _ = writeln!(xref, "{:010} 00000 n ", offset);
    }
    pdf.extend_from_slice(xref.as_bytes());

    // Trailer
    let trailer = format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        total_objects + 1,
        startxref_offset
    );
    pdf.extend_from_slice(trailer.as_bytes());

    Ok(pdf)
}

/// Convenience helper to synthesize a single-page searchable PDF.
pub fn synthesize_single_page_pdf(
    frame: ScanFrame,
    ocr_result: OcrPageResult,
) -> Result<Vec<u8>, OcrError> {
    let doc = SearchablePdfDocument {
        pages: vec![SearchablePdfPageInput {
            frame,
            ocr_result,
            page_width_pt: None,
            page_height_pt: None,
        }],
        title: Some("Vocument Scanned Document".into()),
        author: None,
        creator: Some("Vocument OCR & Searchable PDF Synthesizer".into()),
    };
    synthesize_searchable_pdf(&doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{BoundingBox, OcrWord};

    #[test]
    fn test_helvetica_glyph_widths() {
        assert_eq!(helvetica_glyph_width(' '), 278);
        assert_eq!(helvetica_glyph_width('A'), 667);
        assert_eq!(helvetica_glyph_width('i'), 222);
        assert_eq!(helvetica_glyph_width('§'), 556);
        assert_eq!(helvetica_glyph_width('ä'), 556);
    }

    #[test]
    fn test_escape_pdf_winansi_string() {
        let input = "§ 1 Vertrag über Vertraulichkeit (Wien)";
        let escaped = escape_pdf_winansi_string(input);
        assert!(escaped.contains("\\247"));
        assert!(escaped.contains("\\374"));
        assert!(escaped.contains("\\("));
        assert!(escaped.contains("\\)"));
    }

    #[test]
    fn test_synthesize_minimal_searchable_pdf() {
        let width = 200u32;
        let height = 100u32;
        let pixels = vec![255u8; (width * height) as usize]; // 200x100 white image
        let frame = ScanFrame {
            width,
            height,
            stride: width,
            format: PixelFormat::Gray8,
            orientation_degrees: 0,
            pixels,
            metadata: vocument_imaging::ScanFrameMetadata::default(),
        };

        let word1 = OcrWord {
            text: "VEREINBARUNG".into(),
            bbox: BoundingBox::new(0.1, 0.1, 0.9, 0.3),
            confidence: 95.0,
            characters: Vec::new(),
            block_index: 0,
            line_index: 0,
            word_index: 0,
        };
        let word2 = OcrWord {
            text: "Vertraulichkeit".into(),
            bbox: BoundingBox::new(0.1, 0.4, 0.8, 0.6),
            confidence: 96.0,
            characters: Vec::new(),
            block_index: 0,
            line_index: 1,
            word_index: 0,
        };

        let ocr_result = OcrPageResult {
            page_index: 0,
            full_text: "VEREINBARUNG Vertraulichkeit".into(),
            blocks: Vec::new(),
            lines: Vec::new(),
            words: vec![word1, word2],
            mean_confidence: 95.5,
            detected_language: "deu".into(),
            hocr_output: None,
            frame_width: width,
            frame_height: height,
        };

        let pdf_bytes = synthesize_single_page_pdf(frame, ocr_result)
            .expect("Single-page PDF synthesis should succeed in unit test");
        assert!(pdf_bytes.starts_with(b"%PDF-1.7\n"));
        assert!(pdf_bytes.ends_with(b"%%EOF\n"));
        assert!(pdf_bytes.len() > 1000);
    }
}

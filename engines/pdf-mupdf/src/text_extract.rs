//! Safe Rust wrapper around `mupdf_text_bridge.c` for M0.3 page labels and structured text extraction (`ROADMAP.md` M0.3).

use crate::c_buf_to_optional_string;
use std::os::raw::{c_char, c_int, c_uchar};
use vocument_core::pdf_engine::{
    PdfEngineError, PdfPageStructuredText, PdfRectMilliPt, PdfTextLine,
};

#[repr(C)]
#[derive(Clone)]
struct RawMuPdfTextLine {
    x0_milli_pt: c_int,
    y0_milli_pt: c_int,
    x1_milli_pt: c_int,
    y1_milli_pt: c_int,
    ends_paragraph: c_int,
    text: [c_char; 1024],
}

impl Default for RawMuPdfTextLine {
    fn default() -> Self {
        Self {
            x0_milli_pt: 0,
            y0_milli_pt: 0,
            x1_milli_pt: 0,
            y1_milli_pt: 0,
            ends_paragraph: 0,
            text: [0; 1024],
        }
    }
}

unsafe extern "C" {
    fn vocument_mupdf_query_page_label(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_label_buf: *mut c_char,
        out_label_cap: usize,
    ) -> c_int;

    fn vocument_mupdf_extract_page_text(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_lines: *mut RawMuPdfTextLine,
        max_lines: usize,
        out_line_count: *mut usize,
        out_label_buf: *mut c_char,
        out_label_cap: usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

const MAX_LINES_PER_PAGE: usize = 2048;

pub fn query_embedded_page_label(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
) -> Result<Option<String>, PdfEngineError> {
    if page_index >= page_count {
        return Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        });
    }
    let page_idx_int =
        c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        })?;

    let mut label_buf = [0 as c_char; 128];
    // SAFETY: `pdf_bytes` is a valid buffer and `label_buf` is stack-allocated.
    let status = unsafe {
        vocument_mupdf_query_page_label(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            label_buf.as_mut_ptr(),
            label_buf.len(),
        )
    };

    match status {
        0 => Ok(c_buf_to_optional_string(&label_buf)),
        4 => Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        }),
        _ => Err(PdfEngineError::EngineFailure {
            details: "failed to query PDF page label".to_owned(),
        }),
    }
}

pub fn extract_structured_page_text(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
) -> Result<PdfPageStructuredText, PdfEngineError> {
    if page_index >= page_count {
        return Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        });
    }
    let page_idx_int =
        c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        })?;

    let mut raw_lines = vec![RawMuPdfTextLine::default(); MAX_LINES_PER_PAGE];
    let mut line_count: usize = 0;
    let mut label_buf = [0 as c_char; 128];
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `pdf_bytes` and `raw_lines` are valid buffers with matching capacities.
    let status = unsafe {
        vocument_mupdf_extract_page_text(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            raw_lines.as_mut_ptr(),
            raw_lines.len(),
            &raw mut line_count,
            label_buf.as_mut_ptr(),
            label_buf.len(),
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    match status {
        0 => {
            let capped = line_count.min(raw_lines.len());
            let mut lines = Vec::with_capacity(capped);
            for raw in raw_lines.iter().take(capped) {
                let text = c_buf_to_optional_string(&raw.text).unwrap_or_default();
                lines.push(PdfTextLine {
                    bbox: PdfRectMilliPt {
                        x0_milli_pt: raw.x0_milli_pt,
                        y0_milli_pt: raw.y0_milli_pt,
                        x1_milli_pt: raw.x1_milli_pt,
                        y1_milli_pt: raw.y1_milli_pt,
                    },
                    text,
                    ends_paragraph: raw.ends_paragraph != 0,
                });
            }
            Ok(PdfPageStructuredText {
                page_index,
                embedded_page_label: c_buf_to_optional_string(&label_buf),
                lines,
            })
        }
        4 => Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        }),
        _ => {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "failed to extract structured page text".to_owned());
            Err(PdfEngineError::EngineFailure { details })
        }
    }
}

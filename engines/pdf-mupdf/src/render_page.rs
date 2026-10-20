//! Rendering of a whole page into a grayscale raster (`REDACT-002`).
//!
//! The raster is the picture of the page, exactly as a reader sees it. A
//! document can carry the same text twice — as a text layer and as pixels in
//! an image. A redaction therefore has to be verified against this picture:
//! only the text layer is not enough.

use std::ffi::{c_char, c_int, c_uchar};
use vocument_core::pdf_engine::{PdfEngineError, PdfPageRaster};

unsafe extern "C" {
    /// Releases a buffer that the C bridge allocated.
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_render_page_gray(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        scale_milli: c_int,
        out_pixels: *mut *mut c_uchar,
        out_width: *mut usize,
        out_height: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

/// Renders `page_index` of the document into a grayscale raster.
pub fn render_page_gray(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
    scale_milli: u32,
) -> Result<PdfPageRaster, PdfEngineError> {
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
    let scale_int = c_int::try_from(scale_milli.max(1)).unwrap_or(2000);

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut width: usize = 0;
    let mut height: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `pdf_bytes` is a valid buffer, the output pointers are valid.
    let status = unsafe {
        vocument_mupdf_render_page_gray(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            scale_int,
            &raw mut out_ptr,
            &raw mut width,
            &raw mut height,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 || out_ptr.is_null() || width == 0 || height == 0 {
        let details = crate::c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| format!("MuPDF failed to render the page (status {status})"));
        return Err(PdfEngineError::EngineFailure { details });
    }

    // SAFETY: the C bridge allocated `width * height` bytes and handed the
    // pointer over; the buffer is copied and released here.
    let gray_bytes = unsafe {
        let slice = std::slice::from_raw_parts(out_ptr, width * height);
        let copy = slice.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        copy
    };

    Ok(PdfPageRaster {
        width_px: width as u32,
        height_px: height as u32,
        gray_bytes,
    })
}

//! The comment of the reviewer on a page (`ANNOT-001`).
//!
//! A comment is a standard PDF `/Text` annotation with a popup. The engine
//! writes and reads it; the *meaning* of the colour — the rail of the
//! profession — lives in the core, so that the engine carries no rule of the
//! profession and only reports what it finds.

use std::ffi::{CStr, CString, c_char, c_int, c_uchar};

use vocument_core::pdf_engine::{PdfCommentAnnotation, PdfEngineError, PdfRectMilliPt};

/// One comment, as it travels over the C boundary.
#[repr(C)]
#[derive(Clone, Copy)]
struct RawMuPdfComment {
    page_index: c_int,
    x0_milli_pt: c_int,
    y0_milli_pt: c_int,
    x1_milli_pt: c_int,
    y1_milli_pt: c_int,
    color_r: c_uchar,
    color_g: c_uchar,
    color_b: c_uchar,
    annotation_id: [c_char; 128],
    author: [c_char; 256],
    contents: [c_char; 2048],
}

impl Default for RawMuPdfComment {
    fn default() -> Self {
        Self {
            page_index: 0,
            x0_milli_pt: 0,
            y0_milli_pt: 0,
            x1_milli_pt: 0,
            y1_milli_pt: 0,
            color_r: 255,
            color_g: 230,
            color_b: 80,
            annotation_id: [0; 128],
            author: [0; 256],
            contents: [0; 2048],
        }
    }
}

unsafe extern "C" {
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_query_page_comments(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        out_annots: *mut RawMuPdfComment,
        max_annots: usize,
        out_annot_count: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_add_comment_annot(
        bytes: *const u8,
        len: usize,
        spec: *const RawMuPdfComment,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_remove_comment_annot(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        annotation_id: *const c_char,
        out_removed: *mut RawMuPdfComment,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

/// Reads a C string buffer of the bridge into a Rust string.
fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    if buf.is_empty() || buf[0] == 0 {
        return None;
    }
    // SAFETY: All C buffers are null-terminated.
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let text = c_str.to_string_lossy().trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

/// Copies a Rust string into a C buffer of the bridge.
fn copy_str_to_c_buf(src: &str, dst: &mut [c_char]) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let max_copy = bytes.len().min(dst.len().saturating_sub(1));
    for (i, &b) in bytes.iter().take(max_copy).enumerate() {
        dst[i] = b as c_char;
    }
}

/// Turns the comment of the bridge into the model of the core.
fn raw_to_comment(raw: &RawMuPdfComment, fallback_page_index: u32) -> PdfCommentAnnotation {
    let page_index = u32::try_from(raw.page_index).unwrap_or(fallback_page_index);
    PdfCommentAnnotation {
        annotation_id: c_buf_to_optional_string(&raw.annotation_id)
            .unwrap_or_else(|| format!("comment_page_{page_index}")),
        page_index,
        rect: PdfRectMilliPt {
            x0_milli_pt: raw.x0_milli_pt,
            y0_milli_pt: raw.y0_milli_pt,
            x1_milli_pt: raw.x1_milli_pt,
            y1_milli_pt: raw.y1_milli_pt,
        },
        palette_key: String::new(),
        color_rgb: [raw.color_r, raw.color_g, raw.color_b],
        contents: c_buf_to_optional_string(&raw.contents).unwrap_or_default(),
        author: c_buf_to_optional_string(&raw.author),
    }
}

/// Takes the bytes that the bridge has written.
unsafe fn take_malloc_vec(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        if !ptr.is_null() {
            unsafe { vocument_mupdf_free_bytes(ptr) };
        }
        return Vec::new();
    }
    // SAFETY: the bridge hands out a buffer of `len` bytes that it allocated
    // itself and that has to be released with `vocument_mupdf_free_bytes`.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let out = slice.to_vec();
    unsafe { vocument_mupdf_free_bytes(ptr) };
    out
}

/// Answers whether the page lies in the document.
fn check_page_index(page_index: u32, page_count: u32) -> Result<c_int, PdfEngineError> {
    if page_index >= page_count {
        return Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        });
    }
    c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
        page_index,
        page_count,
    })
}

/// Lists the comments of a page.
pub fn query_page_comments(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
    let page_idx_int = check_page_index(page_index, page_count)?;

    const MAX_ANNOTS: usize = 512;
    let mut raw_annots = vec![RawMuPdfComment::default(); MAX_ANNOTS];
    let mut count: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `raw_annots` has capacity `MAX_ANNOTS`.
    let status = unsafe {
        vocument_mupdf_query_page_comments(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            raw_annots.as_mut_ptr(),
            MAX_ANNOTS,
            &mut count,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| "MuPDF failed to query the comments of the page".to_owned());
        return Err(PdfEngineError::EngineFailure { details });
    }

    let returned = count.min(MAX_ANNOTS);
    Ok(raw_annots
        .iter()
        .take(returned)
        .map(|r| raw_to_comment(r, page_index))
        .collect())
}

/// Writes a comment onto a page.
pub fn add_comment_annotation_to_pdf(
    pdf_bytes: &[u8],
    page_count: u32,
    annotation: &PdfCommentAnnotation,
) -> Result<Vec<u8>, PdfEngineError> {
    let page_idx_int = check_page_index(annotation.page_index, page_count)?;

    let mut raw = RawMuPdfComment {
        page_index: page_idx_int,
        x0_milli_pt: annotation.rect.x0_milli_pt,
        y0_milli_pt: annotation.rect.y0_milli_pt,
        x1_milli_pt: annotation.rect.x1_milli_pt,
        y1_milli_pt: annotation.rect.y1_milli_pt,
        color_r: annotation.color_rgb[0],
        color_g: annotation.color_rgb[1],
        color_b: annotation.color_rgb[2],
        ..RawMuPdfComment::default()
    };
    copy_str_to_c_buf(&annotation.annotation_id, &mut raw.annotation_id);
    if let Some(author) = &annotation.author {
        copy_str_to_c_buf(author, &mut raw.author);
    }
    copy_str_to_c_buf(&annotation.contents, &mut raw.contents);

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: the pointers refer to live buffers of a matching length.
    let status = unsafe {
        vocument_mupdf_add_comment_annot(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            &raw,
            &mut out_ptr,
            &mut out_len,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        // SAFETY: the bridge returns a buffer that has to be released.
        let written = unsafe { take_malloc_vec(out_ptr, out_len) };
        let _ = written;
        let details = c_buf_to_optional_string(&err_buf).unwrap_or_else(|| {
            if status == 5 {
                "the comment carries no text".to_owned()
            } else {
                "MuPDF failed to write the comment".to_owned()
            }
        });
        return Err(PdfEngineError::EngineFailure { details });
    }

    // SAFETY: the bridge returns a buffer that has to be released.
    Ok(unsafe { take_malloc_vec(out_ptr, out_len) })
}

/// Removes a comment from a page and hands the removed note back.
pub fn remove_comment_annotation_from_pdf(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
    annotation_id: &str,
) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
    let page_idx_int = check_page_index(page_index, page_count)?;

    let id = CString::new(annotation_id).map_err(|_| PdfEngineError::EngineFailure {
        details: "the identifier of the comment carries a NUL byte".to_owned(),
    })?;

    let mut removed = RawMuPdfComment::default();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: the pointers refer to live buffers of a matching length.
    let status = unsafe {
        vocument_mupdf_remove_comment_annot(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            id.as_ptr(),
            &mut removed,
            &mut out_ptr,
            &mut out_len,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        // SAFETY: the bridge returns a buffer that has to be released.
        let written = unsafe { take_malloc_vec(out_ptr, out_len) };
        let _ = written;
        let details = c_buf_to_optional_string(&err_buf).unwrap_or_else(|| {
            if status == 6 {
                "the document has no comment with this identifier".to_owned()
            } else {
                "MuPDF failed to remove the comment".to_owned()
            }
        });
        return Err(PdfEngineError::EngineFailure { details });
    }

    // SAFETY: the bridge returns a buffer that has to be released.
    let bytes = unsafe { take_malloc_vec(out_ptr, out_len) };
    Ok((raw_to_comment(&removed, page_index), bytes))
}

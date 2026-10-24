//! Rust bindings for MuPDF Bates Numbering, Headers/Footers, Bookmarks, and Stamps (`ROADMAP.md` M3.4).

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use vocument_core::pdf_engine::{
    BatesNumberingOptions, BookmarkItem, HeaderFooterOptions, PdfEngineError,
    StampOrWatermarkOptions,
};

#[repr(C)]
#[allow(dead_code)]
enum VocumentBatesPosition {
    TopLeft = 0,
    TopCenter = 1,
    TopRight = 2,
    BottomLeft = 3,
    BottomCenter = 4,
    BottomRight = 5,
}

unsafe extern "C" {
    fn vocument_mupdf_apply_bates_numbering(
        pdf_bytes: *const u8,
        pdf_len: usize,
        page_indices: *const i32,
        page_indices_len: usize,
        prefix: *const c_char,
        start_number: i32,
        digit_padding: i32,
        suffix: *const c_char,
        position: i32,
        font_size_pt: f32,
        margin_pt: f32,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_apply_header_footer(
        pdf_bytes: *const u8,
        pdf_len: usize,
        page_indices: *const i32,
        page_indices_len: usize,
        header_left: *const c_char,
        header_center: *const c_char,
        header_right: *const c_char,
        footer_left: *const c_char,
        footer_center: *const c_char,
        footer_right: *const c_char,
        doc_title: *const c_char,
        date_str: *const c_char,
        font_size_pt: f32,
        margin_pt: f32,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_get_bookmarks_json(
        pdf_bytes: *const u8,
        pdf_len: usize,
        out_json_str: *mut *mut c_char,
    ) -> i32;

    fn vocument_mupdf_set_bookmarks_json(
        pdf_bytes: *const u8,
        pdf_len: usize,
        bookmarks_json: *const c_char,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_apply_stamp_or_watermark(
        pdf_bytes: *const u8,
        pdf_len: usize,
        page_indices: *const i32,
        page_indices_len: usize,
        stamp_text: *const c_char,
        opacity: f32,
        rotation_degrees: f32,
        r: u8,
        g: u8,
        b: u8,
        is_background: i32,
        rect_x0: f32,
        rect_y0: f32,
        rect_x1: f32,
        rect_y1: f32,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_free_bytes(bytes: *mut u8);
    fn vocument_mupdf_free_string(s: *mut c_char);
}

fn check_status(status: i32, op_name: &str) -> Result<(), PdfEngineError> {
    match status {
        0 => Ok(()),
        1 => Err(PdfEngineError::InvalidPdf {
            details: format!("{op_name}: invalid arguments or corrupted PDF structure"),
        }),
        2 => Err(PdfEngineError::InvalidPdf {
            details: format!("{op_name}: failed to open or parse PDF bytes"),
        }),
        3 => Err(PdfEngineError::EngineFailure {
            details: format!("{op_name}: out of memory in MuPDF context"),
        }),
        4 => Err(PdfEngineError::OperationBlockedBySignature {
            details: format!("{op_name} blocked: document contains digital signatures"),
        }),
        _ => Err(PdfEngineError::EngineFailure {
            details: format!("{op_name} failed with code {status}"),
        }),
    }
}

pub fn apply_bates_numbering_ext(
    pdf_bytes: &[u8],
    page_indices: &[u32],
    options: &BatesNumberingOptions,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    let c_indices: Vec<i32> = page_indices.iter().map(|&i| i as i32).collect();
    let c_prefix = CString::new(options.prefix.as_str()).unwrap_or_default();
    let c_suffix = CString::new(options.suffix.as_str()).unwrap_or_default();

    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_apply_bates_numbering(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            c_prefix.as_ptr(),
            options.start_number as i32,
            options.digit_padding as i32,
            c_suffix.as_ptr(),
            options.position as i32,
            options.font_size_pt as f32,
            options.margin_pt as f32,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_bytes,
            &mut out_len,
        )
    };

    check_status(status, "apply_bates_numbering")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF returned null buffer for Bates numbering".to_owned(),
        });
    }

    let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_bytes) };
    Ok(vec)
}

pub fn apply_header_footer_ext(
    pdf_bytes: &[u8],
    page_indices: &[u32],
    options: &HeaderFooterOptions,
    doc_title: Option<&str>,
    date_str: Option<&str>,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    let c_indices: Vec<i32> = page_indices.iter().map(|&i| i as i32).collect();

    let c_hl = options
        .header_left
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_hc = options
        .header_center
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_hr = options
        .header_right
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_fl = options
        .footer_left
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_fc = options
        .footer_center
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_fr = options
        .footer_right
        .as_deref()
        .map(|s| CString::new(s).unwrap_or_default());
    let c_title = doc_title.map(|s| CString::new(s).unwrap_or_default());
    let c_date = date_str.map(|s| CString::new(s).unwrap_or_default());

    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_apply_header_footer(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            c_hl.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_hc.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_hr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_fl.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_fc.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_fr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_title.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            c_date.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
            options.font_size_pt as f32,
            options.margin_pt as f32,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_bytes,
            &mut out_len,
        )
    };

    check_status(status, "apply_header_footer")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF returned null buffer for headers and footers".to_owned(),
        });
    }

    let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_bytes) };
    Ok(vec)
}

pub fn get_bookmarks(pdf_bytes: &[u8]) -> Result<Vec<BookmarkItem>, PdfEngineError> {
    let mut out_str: *mut c_char = std::ptr::null_mut();
    let status = unsafe {
        vocument_mupdf_get_bookmarks_json(pdf_bytes.as_ptr(), pdf_bytes.len(), &mut out_str)
    };

    check_status(status, "get_bookmarks")?;

    if out_str.is_null() {
        return Ok(Vec::new());
    }

    let json_str = unsafe { CStr::from_ptr(out_str).to_string_lossy().into_owned() };
    unsafe { vocument_mupdf_free_string(out_str) };

    let items: Vec<BookmarkItem> =
        serde_json::from_str(&json_str).map_err(|e| PdfEngineError::EngineFailure {
            details: format!("Failed to deserialize outline JSON from MuPDF: {e}"),
        })?;

    Ok(items)
}

pub fn set_bookmarks_ext(
    pdf_bytes: &[u8],
    bookmarks: &[BookmarkItem],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    let json_str = serde_json::to_string(bookmarks).map_err(|e| PdfEngineError::EngineFailure {
        details: format!("Failed to serialize outline JSON for MuPDF: {e}"),
    })?;
    let c_json = CString::new(json_str).unwrap_or_default();

    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_set_bookmarks_json(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_json.as_ptr(),
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_bytes,
            &mut out_len,
        )
    };

    check_status(status, "set_bookmarks")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF returned null buffer for set_bookmarks".to_owned(),
        });
    }

    let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_bytes) };
    Ok(vec)
}

pub fn apply_stamp_or_watermark_ext(
    pdf_bytes: &[u8],
    page_indices: &[u32],
    options: &StampOrWatermarkOptions,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    let c_indices: Vec<i32> = page_indices.iter().map(|&i| i as i32).collect();
    let c_text = CString::new(options.text.as_str()).unwrap_or_default();

    let (x0, y0, x1, y1) = if let Some(r) = options.rect {
        (
            r.x0_milli_pt as f32 / 1000.0,
            r.y0_milli_pt as f32 / 1000.0,
            r.x1_milli_pt as f32 / 1000.0,
            r.y1_milli_pt as f32 / 1000.0,
        )
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    let opacity = options.opacity_milli as f32 / 1000.0;
    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_apply_stamp_or_watermark(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            c_text.as_ptr(),
            opacity,
            options.rotation_degrees as f32,
            options.color_rgb[0],
            options.color_rgb[1],
            options.color_rgb[2],
            if options.is_background { 1 } else { 0 },
            x0,
            y0,
            x1,
            y1,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_bytes,
            &mut out_len,
        )
    };

    check_status(status, "apply_stamp_or_watermark")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF returned null buffer for stamp or watermark".to_owned(),
        });
    }

    let vec = unsafe { std::slice::from_raw_parts(out_bytes, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_bytes) };
    Ok(vec)
}

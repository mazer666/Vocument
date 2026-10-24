use std::ffi::c_int;
use std::slice;
use vocument_core::pdf_engine::{PdfEngineError, PdfRectMilliPt};

unsafe extern "C" {
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_get_page_boxes(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        out_media_box: *mut f32,
        out_crop_box: *mut f32,
        out_bleed_box: *mut f32,
        out_trim_box: *mut f32,
        out_art_box: *mut f32,
        out_rotation: *mut c_int,
    ) -> c_int;

    fn vocument_mupdf_set_page_boxes_ext(
        bytes: *const u8,
        len: usize,
        page_indices: *const c_int,
        num_pages: usize,
        media_box: *const f32,
        crop_box: *const f32,
        bleed_box: *const f32,
        trim_box: *const f32,
        art_box: *const f32,
        allow_signature_invalidation: c_int,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> c_int;

    fn vocument_mupdf_apply_hard_crop_ext(
        bytes: *const u8,
        len: usize,
        page_indices: *const c_int,
        num_pages: usize,
        crop_rect: *const f32,
        allow_signature_invalidation: c_int,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> c_int;

    fn vocument_mupdf_detect_deskew_angle(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        out_angle_degrees: *mut f32,
    ) -> c_int;

    fn vocument_mupdf_apply_deskew_ext(
        bytes: *const u8,
        len: usize,
        page_indices: *const c_int,
        num_pages: usize,
        angle_degrees: f32,
        allow_signature_invalidation: c_int,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> c_int;

    fn vocument_mupdf_detect_content_margins(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        safety_margin_pt: f32,
        out_crop_rect: *mut f32,
    ) -> c_int;
}

/// Represents the 5 standard ISO 32000-1 page boundaries (in PDF points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageBoxSet {
    pub media_box: [f32; 4],
    pub crop_box: [f32; 4],
    pub bleed_box: [f32; 4],
    pub trim_box: [f32; 4],
    pub art_box: [f32; 4],
    pub rotation_degrees: i32,
}

fn map_bridge_status(status: i32, op_name: &str) -> PdfEngineError {
    match status {
        4 => PdfEngineError::OperationBlockedBySignature {
            details: format!(
                "{op_name} blocked by preflight check: document contains active digital signatures"
            ),
        },
        1 => PdfEngineError::InvalidPdf {
            details: format!("{op_name}: invalid arguments or page index out of bounds"),
        },
        _ => PdfEngineError::EngineFailure {
            details: format!("{op_name}: MuPDF engine failure with status code {status}"),
        },
    }
}

/// Queries all 5 page boundary boxes and rotation for `page_index`.
pub fn get_page_boxes(pdf_bytes: &[u8], page_index: usize) -> Result<PageBoxSet, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "PDF buffer is empty".to_owned(),
        });
    }

    let mut media = [0.0f32; 4];
    let mut crop = [0.0f32; 4];
    let mut bleed = [0.0f32; 4];
    let mut trim = [0.0f32; 4];
    let mut art = [0.0f32; 4];
    let mut rot: c_int = 0;

    let c_idx = c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
        page_index: page_index as u32,
        page_count: 0,
    })?;

    let status = unsafe {
        vocument_mupdf_get_page_boxes(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_idx,
            media.as_mut_ptr(),
            crop.as_mut_ptr(),
            bleed.as_mut_ptr(),
            trim.as_mut_ptr(),
            art.as_mut_ptr(),
            &mut rot,
        )
    };

    if status != 0 {
        return Err(map_bridge_status(status, "get_page_boxes"));
    }

    Ok(PageBoxSet {
        media_box: media,
        crop_box: crop,
        bleed_box: bleed,
        trim_box: trim,
        art_box: art,
        rotation_degrees: rot as i32,
    })
}

/// Modifies page boxes (MediaBox, CropBox, BleedBox, TrimBox, ArtBox) on selected pages.
#[allow(clippy::too_many_arguments)]
pub fn set_page_boxes_ext(
    pdf_bytes: &[u8],
    page_indices: &[usize],
    media_box: Option<[f32; 4]>,
    crop_box: Option<[f32; 4]>,
    bleed_box: Option<[f32; 4]>,
    trim_box: Option<[f32; 4]>,
    art_box: Option<[f32; 4]>,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() || page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "Empty buffer or page indices".to_owned(),
        });
    }

    let c_indices: Vec<c_int> = page_indices
        .iter()
        .map(|&idx| {
            c_int::try_from(idx).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index: idx as u32,
                page_count: 0,
            })
        })
        .collect::<Result<Vec<c_int>, _>>()?;

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let m_ptr = media_box
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    let c_ptr = crop_box
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    let b_ptr = bleed_box
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    let t_ptr = trim_box
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());
    let a_ptr = art_box
        .as_ref()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null());

    let status = unsafe {
        vocument_mupdf_set_page_boxes_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            m_ptr,
            c_ptr,
            b_ptr,
            t_ptr,
            a_ptr,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 {
        return Err(map_bridge_status(status, "set_page_boxes"));
    }

    if out_ptr.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "set_page_boxes produced empty buffer".to_owned(),
        });
    }

    let result = unsafe {
        let s = slice::from_raw_parts(out_ptr, out_len);
        let v = s.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        v
    };

    Ok(result)
}

/// Applies a non-destructive display crop by setting the `/CropBox` on target pages.
pub fn apply_display_crop_ext(
    pdf_bytes: &[u8],
    page_indices: &[usize],
    crop_rect: [f32; 4],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    set_page_boxes_ext(
        pdf_bytes,
        page_indices,
        None,
        Some(crop_rect),
        None,
        None,
        None,
        allow_signature_invalidation,
    )
}

/// Applies a destructive hard crop: sets MediaBox/CropBox and injects a clipping path.
pub fn apply_hard_crop_ext(
    pdf_bytes: &[u8],
    page_indices: &[usize],
    crop_rect: [f32; 4],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() || page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "Empty buffer or page indices".to_owned(),
        });
    }

    let c_indices: Vec<c_int> = page_indices
        .iter()
        .map(|&idx| {
            c_int::try_from(idx).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index: idx as u32,
                page_count: 0,
            })
        })
        .collect::<Result<Vec<c_int>, _>>()?;

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_apply_hard_crop_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            crop_rect.as_ptr(),
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 {
        return Err(map_bridge_status(status, "apply_hard_crop"));
    }

    if out_ptr.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "apply_hard_crop produced empty buffer".to_owned(),
        });
    }

    let result = unsafe {
        let s = slice::from_raw_parts(out_ptr, out_len);
        let v = s.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        v
    };

    Ok(result)
}

/// Detects document skew angle on `page_index` (degrees, accurate to 0.1°).
pub fn detect_deskew_angle(pdf_bytes: &[u8], page_index: usize) -> Result<f32, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "PDF buffer is empty".to_owned(),
        });
    }

    let c_idx = c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
        page_index: page_index as u32,
        page_count: 0,
    })?;

    let mut angle: f32 = 0.0;
    let status = unsafe {
        vocument_mupdf_detect_deskew_angle(pdf_bytes.as_ptr(), pdf_bytes.len(), c_idx, &mut angle)
    };

    if status != 0 {
        return Err(map_bridge_status(status, "detect_deskew_angle"));
    }

    Ok(angle)
}

/// Applies fine deskew rotation to the specified pages using a vector transformation matrix.
pub fn apply_deskew_ext(
    pdf_bytes: &[u8],
    page_indices: &[usize],
    angle_degrees: f32,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() || page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "Empty buffer or page indices".to_owned(),
        });
    }

    let c_indices: Vec<c_int> = page_indices
        .iter()
        .map(|&idx| {
            c_int::try_from(idx).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index: idx as u32,
                page_count: 0,
            })
        })
        .collect::<Result<Vec<c_int>, _>>()?;

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_apply_deskew_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            angle_degrees,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 {
        return Err(map_bridge_status(status, "apply_deskew"));
    }

    if out_ptr.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "apply_deskew produced empty buffer".to_owned(),
        });
    }

    let result = unsafe {
        let s = slice::from_raw_parts(out_ptr, out_len);
        let v = s.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        v
    };

    Ok(result)
}

/// Detects content bounding box on a page and returns a trimmed crop rectangle.
pub fn detect_content_margins(
    pdf_bytes: &[u8],
    page_index: usize,
    safety_margin_pt: f32,
) -> Result<[f32; 4], PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "PDF buffer is empty".to_owned(),
        });
    }

    let c_idx = c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
        page_index: page_index as u32,
        page_count: 0,
    })?;

    let mut crop_rect = [0.0f32; 4];
    let status = unsafe {
        vocument_mupdf_detect_content_margins(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_idx,
            safety_margin_pt,
            crop_rect.as_mut_ptr(),
        )
    };

    if status != 0 {
        return Err(map_bridge_status(status, "detect_content_margins"));
    }

    Ok(crop_rect)
}

/// Automatically detects content margins and applies a non-destructive crop with `safety_margin_pt`.
pub fn apply_auto_crop_ext(
    pdf_bytes: &[u8],
    page_indices: &[usize],
    safety_margin_pt: f32,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "No pages specified for auto-crop".to_owned(),
        });
    }

    let mut current_bytes = pdf_bytes.to_vec();
    for &page_index in page_indices {
        let crop_rect = detect_content_margins(&current_bytes, page_index, safety_margin_pt)?;
        current_bytes = apply_display_crop_ext(
            &current_bytes,
            &[page_index],
            crop_rect,
            allow_signature_invalidation,
        )?;
    }

    Ok(current_bytes)
}

/// Converts `PdfRectMilliPt` (origin at top-left, 1/1000 pt) to PDF points `[x0, y0, x1, y1]` (origin at bottom-left).
pub fn milli_pt_rect_to_pdf_points(rect: PdfRectMilliPt, media_box: [f32; 4]) -> [f32; 4] {
    let x0 = media_box[0] + rect.x0_milli_pt as f32 / 1000.0;
    let x1 = media_box[0] + rect.x1_milli_pt as f32 / 1000.0;
    let y0_top = rect.y0_milli_pt as f32 / 1000.0;
    let y1_top = rect.y1_milli_pt as f32 / 1000.0;

    let y0 = media_box[3] - y1_top;
    let y1 = media_box[3] - y0_top;

    [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)]
}

/// Converts PDF points `[x0, y0, x1, y1]` (origin bottom-left) to `PdfRectMilliPt` (origin top-left).
pub fn pdf_points_to_milli_pt_rect(pts: [f32; 4], media_box: [f32; 4]) -> PdfRectMilliPt {
    let x0_milli = ((pts[0].min(pts[2]) - media_box[0]) * 1000.0).round() as i32;
    let x1_milli = ((pts[0].max(pts[2]) - media_box[0]) * 1000.0).round() as i32;
    let y0_milli = ((media_box[3] - pts[1].max(pts[3])) * 1000.0).round() as i32;
    let y1_milli = ((media_box[3] - pts[1].min(pts[3])) * 1000.0).round() as i32;

    PdfRectMilliPt {
        x0_milli_pt: x0_milli,
        y0_milli_pt: y0_milli,
        x1_milli_pt: x1_milli,
        y1_milli_pt: y1_milli,
    }
}

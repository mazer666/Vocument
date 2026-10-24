//! Rust bindings for MuPDF Optimization, Compression, Image Extraction, and Preflight (`ROADMAP.md` M3.5).

use std::ffi::CStr;
use std::os::raw::c_char;
use vocument_core::pdf_engine::{
    ExtractedImageData, ExtractedImageInfo, OptimizationOptions, OptimizationPreset,
    OptimizationResult, PdfEngineError, PdfxPreflightReport, PrintPreflightReport,
};

unsafe extern "C" {
    fn vocument_mupdf_optimize_document(
        pdf_bytes: *const u8,
        pdf_len: usize,
        preset: i32,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_get_document_images_json(
        pdf_bytes: *const u8,
        pdf_len: usize,
        out_json_str: *mut *mut c_char,
    ) -> i32;

    fn vocument_mupdf_extract_image_bytes(
        pdf_bytes: *const u8,
        pdf_len: usize,
        page_index: i32,
        image_index: i32,
        out_format_str: *mut *mut c_char,
        out_width: *mut i32,
        out_height: *mut i32,
        out_is_raw: *mut i32,
        out_img_bytes: *mut *mut u8,
        out_img_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_preflight_pdfx_json(
        pdf_bytes: *const u8,
        pdf_len: usize,
        out_json_str: *mut *mut c_char,
    ) -> i32;

    fn vocument_mupdf_preflight_print_json(
        pdf_bytes: *const u8,
        pdf_len: usize,
        out_json_str: *mut *mut c_char,
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

pub fn optimize_document_ext(
    pdf_bytes: &[u8],
    options: &OptimizationOptions,
    allow_signature_invalidation: bool,
) -> Result<(Vec<u8>, OptimizationResult), PdfEngineError> {
    let preset_val = match options.preset {
        OptimizationPreset::Lossless => 0,
        OptimizationPreset::Balanced => 1,
        OptimizationPreset::Maximum => 2,
    };
    let sig_val = if allow_signature_invalidation { 1 } else { 0 };

    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_optimize_document(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            preset_val,
            sig_val,
            &mut out_bytes,
            &mut out_len,
        )
    };

    check_status(status, "optimize_document")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "optimize_document returned null or empty output buffer".to_owned(),
        });
    }

    let result_vec = unsafe {
        let slice = std::slice::from_raw_parts(out_bytes, out_len);
        let vec = slice.to_vec();
        vocument_mupdf_free_bytes(out_bytes);
        vec
    };

    let original_size = pdf_bytes.len();
    let optimized_size = result_vec.len();
    let saved = original_size as i64 - optimized_size as i64;
    let ratio = if original_size > 0 {
        1.0 - (optimized_size as f64 / original_size as f64)
    } else {
        0.0
    };

    let summary = OptimizationResult {
        original_size_bytes: original_size,
        optimized_size_bytes: optimized_size,
        bytes_saved: saved,
        compression_ratio: ratio,
    };

    Ok((result_vec, summary))
}

pub fn get_document_images(pdf_bytes: &[u8]) -> Result<Vec<ExtractedImageInfo>, PdfEngineError> {
    let mut out_str: *mut c_char = std::ptr::null_mut();
    let status = unsafe {
        vocument_mupdf_get_document_images_json(pdf_bytes.as_ptr(), pdf_bytes.len(), &mut out_str)
    };
    check_status(status, "get_document_images")?;

    if out_str.is_null() {
        return Ok(Vec::new());
    }

    let json_slice = unsafe {
        let c_str = CStr::from_ptr(out_str);
        let s = c_str.to_string_lossy().into_owned();
        vocument_mupdf_free_string(out_str);
        s
    };

    serde_json::from_str::<Vec<ExtractedImageInfo>>(&json_slice).map_err(|e| {
        PdfEngineError::EngineFailure {
            details: format!("failed to parse images json: {e}"),
        }
    })
}

pub fn extract_image_bytes(
    pdf_bytes: &[u8],
    page_index: u32,
    image_index: u32,
) -> Result<ExtractedImageData, PdfEngineError> {
    let mut out_format: *mut c_char = std::ptr::null_mut();
    let mut out_w: i32 = 0;
    let mut out_h: i32 = 0;
    let mut out_is_raw: i32 = 0;
    let mut out_bytes: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_extract_image_bytes(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_index as i32,
            image_index as i32,
            &mut out_format,
            &mut out_w,
            &mut out_h,
            &mut out_is_raw,
            &mut out_bytes,
            &mut out_len,
        )
    };
    check_status(status, "extract_image_bytes")?;

    if out_bytes.is_null() || out_len == 0 {
        return Err(PdfEngineError::EngineFailure {
            details: "extract_image_bytes returned null or empty image buffer".to_owned(),
        });
    }

    let format_str = if !out_format.is_null() {
        unsafe {
            let c_str = CStr::from_ptr(out_format);
            let s = c_str.to_string_lossy().into_owned();
            vocument_mupdf_free_string(out_format);
            s
        }
    } else {
        "png".to_owned()
    };

    let data = unsafe {
        let slice = std::slice::from_raw_parts(out_bytes, out_len);
        let vec = slice.to_vec();
        vocument_mupdf_free_bytes(out_bytes);
        vec
    };

    Ok(ExtractedImageData {
        width: out_w.max(0) as u32,
        height: out_h.max(0) as u32,
        format: format_str,
        is_raw_stream: out_is_raw != 0,
        data,
    })
}

pub fn preflight_pdfx(pdf_bytes: &[u8]) -> Result<PdfxPreflightReport, PdfEngineError> {
    let mut out_str: *mut c_char = std::ptr::null_mut();
    let status = unsafe {
        vocument_mupdf_preflight_pdfx_json(pdf_bytes.as_ptr(), pdf_bytes.len(), &mut out_str)
    };
    check_status(status, "preflight_pdfx")?;

    if out_str.is_null() {
        return Err(PdfEngineError::EngineFailure {
            details: "preflight_pdfx returned null report".to_owned(),
        });
    }

    let json_slice = unsafe {
        let c_str = CStr::from_ptr(out_str);
        let s = c_str.to_string_lossy().into_owned();
        vocument_mupdf_free_string(out_str);
        s
    };

    serde_json::from_str::<PdfxPreflightReport>(&json_slice).map_err(|e| {
        PdfEngineError::EngineFailure {
            details: format!("failed to parse pdfx preflight json: {e}"),
        }
    })
}

pub fn preflight_print(pdf_bytes: &[u8]) -> Result<PrintPreflightReport, PdfEngineError> {
    let mut out_str: *mut c_char = std::ptr::null_mut();
    let status = unsafe {
        vocument_mupdf_preflight_print_json(pdf_bytes.as_ptr(), pdf_bytes.len(), &mut out_str)
    };
    check_status(status, "preflight_print")?;

    if out_str.is_null() {
        return Err(PdfEngineError::EngineFailure {
            details: "preflight_print returned null report".to_owned(),
        });
    }

    let json_slice = unsafe {
        let c_str = CStr::from_ptr(out_str);
        let s = c_str.to_string_lossy().into_owned();
        vocument_mupdf_free_string(out_str);
        s
    };

    serde_json::from_str::<PrintPreflightReport>(&json_slice).map_err(|e| {
        PdfEngineError::EngineFailure {
            details: format!("failed to parse print preflight json: {e}"),
        }
    })
}

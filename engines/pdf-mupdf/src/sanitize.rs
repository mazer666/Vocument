//! Safe Rust wrapper for PDF document sanitization (`SAN-001`).
//!
//! Cleans metadata, attachments, actions, annotations, and form structures
//! from a PDF document using the MuPDF C bridge.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

use vocument_core::pdf_engine::{PdfEngineError, PdfSanitizeOptions, PdfSanitizeReport};

unsafe extern "C" {
    fn vocument_mupdf_sanitize_document(
        bytes: *const c_uchar,
        len: usize,
        remove_metadata: c_int,
        remove_attachments: c_int,
        remove_actions: c_int,
        remove_annotations: c_int,
        remove_form_fields: c_int,
        out_pdf_bytes: *mut *mut c_uchar,
        out_pdf_len: *mut usize,
        out_metadata_cleared: *mut c_int,
        out_xmp_cleared: *mut c_int,
        out_attachments_removed: *mut c_int,
        out_actions_removed: *mut c_int,
        out_annotations_removed: *mut c_int,
        out_form_fields_removed: *mut c_int,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_free_bytes(ptr: *mut c_uchar);
}

/// Reads a NUL-terminated string from the C error buffer.
fn read_string(buffer: &[c_char]) -> String {
    // SAFETY: The C bridge writes NUL-terminated strings into the buffer.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Sanitizes a PDF document by stripping the requested components.
pub fn sanitize_document(
    pdf_bytes: &[u8],
    options: &PdfSanitizeOptions,
) -> Result<(Vec<u8>, PdfSanitizeReport), PdfEngineError> {
    let mut out_bytes: *mut c_uchar = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut metadata_cleared: c_int = 0;
    let mut xmp_cleared: c_int = 0;
    let mut attachments_removed: c_int = 0;
    let mut actions_removed: c_int = 0;
    let mut annotations_removed: c_int = 0;
    let mut form_fields_removed: c_int = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: Pointers to buffers and outputs are valid for the call duration.
    let status = unsafe {
        vocument_mupdf_sanitize_document(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            options.remove_metadata as c_int,
            options.remove_attachments as c_int,
            options.remove_actions as c_int,
            options.remove_annotations as c_int,
            options.remove_form_fields as c_int,
            &raw mut out_bytes,
            &raw mut out_len,
            &raw mut metadata_cleared,
            &raw mut xmp_cleared,
            &raw mut attachments_removed,
            &raw mut actions_removed,
            &raw mut annotations_removed,
            &raw mut form_fields_removed,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = read_string(&err_buf);
        return Err(PdfEngineError::EngineFailure {
            details: if details.is_empty() {
                format!("MuPDF failed to sanitize the document (status {status})")
            } else {
                details
            },
        });
    }

    if out_bytes.is_null() {
        return Err(PdfEngineError::EngineFailure {
            details: "MuPDF returned null pointer after sanitizing".to_owned(),
        });
    }

    // SAFETY: The bridge allocated the buffer with malloc and reports length.
    let sanitized_bytes = unsafe { std::slice::from_raw_parts(out_bytes, out_len) }.to_vec();
    // SAFETY: The buffer was allocated with malloc and must be freed with vocument_mupdf_free_bytes.
    unsafe { vocument_mupdf_free_bytes(out_bytes) };

    Ok((
        sanitized_bytes,
        PdfSanitizeReport {
            metadata_cleared: metadata_cleared != 0,
            xmp_cleared: xmp_cleared != 0,
            attachments_removed: attachments_removed.max(0) as u32,
            actions_removed: actions_removed.max(0) as u32,
            annotations_removed: annotations_removed.max(0) as u32,
            form_fields_removed: form_fields_removed.max(0) as u32,
        },
    ))
}

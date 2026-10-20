//! Inspection of the embedded files and the signatures of a document
//! (`ATTACH-001`, `SIGN-001`).
//!
//! The engine reports the facts; the risk policy lives in the core.

use std::ffi::{CStr, c_char, c_int, c_uchar};
use vocument_core::document_inspection::{PdfAttachmentInfo, PdfSignatureInfo};
use vocument_core::pdf_engine::PdfEngineError;

/// The maximum number of attachments that are reported for one document.
pub const MAX_ATTACHMENTS: usize = 256;

/// The maximum number of signature fields that are reported for one document.
pub const MAX_SIGNATURES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
struct RawAttachment {
    file_name: [c_char; 512],
    mime_type: [c_char; 256],
    size_bytes: i64,
    is_embedded: c_int,
    checksum_is_valid: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawSignature {
    field_name: [c_char; 256],
    is_signed: c_int,
    changed_since_signing: c_int,
    signer: [c_char; 256],
    signed_at: [c_char; 64],
    reason: [c_char; 256],
    location: [c_char; 256],
}

unsafe extern "C" {
    fn vocument_mupdf_inspect_attachments(
        bytes: *const c_uchar,
        len: usize,
        out_attachments: *mut RawAttachment,
        max_attachments: usize,
        out_attachment_count: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_inspect_signatures(
        bytes: *const c_uchar,
        len: usize,
        out_signatures: *mut RawSignature,
        max_signatures: usize,
        out_signature_count: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

/// Reads a NUL-terminated string of the C bridge.
fn read_string(buffer: &[c_char]) -> String {
    // SAFETY: the C bridge writes NUL-terminated strings into the arrays.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

/// Reads the embedded files of the document.
pub fn document_attachments(pdf_bytes: &[u8]) -> Result<Vec<PdfAttachmentInfo>, PdfEngineError> {
    let mut attachments: Vec<RawAttachment> = vec![
        RawAttachment {
            file_name: [0; 512],
            mime_type: [0; 256],
            size_bytes: 0,
            is_embedded: 0,
            checksum_is_valid: 0,
        };
        MAX_ATTACHMENTS
    ];
    let mut count: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `pdf_bytes` is a valid buffer, `attachments` has room for
    // `MAX_ATTACHMENTS` entries, the output pointers are valid.
    let status = unsafe {
        vocument_mupdf_inspect_attachments(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            attachments.as_mut_ptr(),
            MAX_ATTACHMENTS,
            &raw mut count,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = crate::c_buf_to_optional_string(&err_buf).unwrap_or_else(|| {
            format!("MuPDF failed to inspect the attachments (status {status})")
        });
        return Err(PdfEngineError::EngineFailure { details });
    }

    let mut result = Vec::with_capacity(count.min(MAX_ATTACHMENTS));
    for raw in attachments.iter().take(count.min(MAX_ATTACHMENTS)) {
        result.push(PdfAttachmentInfo::new(
            read_string(&raw.file_name),
            read_string(&raw.mime_type),
            raw.size_bytes,
            raw.is_embedded != 0,
            raw.checksum_is_valid != 0,
        ));
    }
    Ok(result)
}

/// Reads the signature fields of the document.
pub fn document_signatures(pdf_bytes: &[u8]) -> Result<Vec<PdfSignatureInfo>, PdfEngineError> {
    let mut signatures: Vec<RawSignature> = vec![
        RawSignature {
            field_name: [0; 256],
            is_signed: 0,
            changed_since_signing: 0,
            signer: [0; 256],
            signed_at: [0; 64],
            reason: [0; 256],
            location: [0; 256],
        };
        MAX_SIGNATURES
    ];
    let mut count: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `pdf_bytes` is a valid buffer, `signatures` has room for
    // `MAX_SIGNATURES` entries, the output pointers are valid.
    let status = unsafe {
        vocument_mupdf_inspect_signatures(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            signatures.as_mut_ptr(),
            MAX_SIGNATURES,
            &raw mut count,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = crate::c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| format!("MuPDF failed to inspect the signatures (status {status})"));
        return Err(PdfEngineError::EngineFailure { details });
    }

    let mut result = Vec::with_capacity(count.min(MAX_SIGNATURES));
    for raw in signatures.iter().take(count.min(MAX_SIGNATURES)) {
        result.push(PdfSignatureInfo::new(
            read_string(&raw.field_name),
            raw.is_signed != 0,
            read_string(&raw.signer),
            read_string(&raw.signed_at),
            read_string(&raw.reason),
            read_string(&raw.location),
            raw.changed_since_signing != 0,
        ));
    }
    Ok(result)
}

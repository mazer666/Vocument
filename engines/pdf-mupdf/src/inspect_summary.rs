//! MuPDF summary inspection helper (`M0.1`).

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};
use vocument_core::pdf_engine::{PdfDocumentSummary, PdfEngineError};

#[repr(C)]
#[derive(Debug)]
pub struct RawMuPdfSummary {
    pub status_code: c_int,
    pub page_count: c_int,
    pub needs_password: c_int,
    pub format_version: [c_char; 64],
    pub title: [c_char; 512],
    pub author: [c_char; 512],
    pub subject: [c_char; 512],
    pub keywords: [c_char; 512],
    pub creator: [c_char; 512],
    pub producer: [c_char; 512],
    pub creation_date: [c_char; 128],
    pub modification_date: [c_char; 128],
    pub error_message: [c_char; 512],
    pub engine_version: [c_char; 64],
}

unsafe extern "C" {
    fn vocument_mupdf_inspect_pdf_bytes(
        bytes: *const c_uchar,
        len: usize,
        out_summary: *mut RawMuPdfSummary,
    ) -> c_int;
}

fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    let nul_pos = buf.iter().position(|&c| c == 0)?;
    if nul_pos == 0 {
        return None;
    }
    let u8_slice = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), nul_pos) };
    CStr::from_bytes_with_nul(unsafe {
        std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), nul_pos + 1)
    })
    .ok()
    .map(|cs| cs.to_string_lossy().into_owned())
    .or_else(|| Some(String::from_utf8_lossy(u8_slice).into_owned()))
}

fn engine_identifier_from_version(version: &str) -> String {
    format!("mupdf-{version}")
}

pub fn inspect_summary(bytes: &[u8]) -> Result<PdfDocumentSummary, PdfEngineError> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(PdfEngineError::InvalidPdf {
            details: "missing %PDF- header".to_owned(),
        });
    }

    let mut raw = RawMuPdfSummary {
        status_code: 0,
        page_count: 0,
        needs_password: 0,
        format_version: [0; 64],
        title: [0; 512],
        author: [0; 512],
        subject: [0; 512],
        keywords: [0; 512],
        creator: [0; 512],
        producer: [0; 512],
        creation_date: [0; 128],
        modification_date: [0; 128],
        error_message: [0; 512],
        engine_version: [0; 64],
    };

    let status = unsafe { vocument_mupdf_inspect_pdf_bytes(bytes.as_ptr(), bytes.len(), &mut raw) };

    let engine_version =
        c_buf_to_optional_string(&raw.engine_version).unwrap_or_else(|| "unknown".to_owned());

    match status {
        0 => {
            let page_count = u32::try_from(raw.page_count).unwrap_or(0);
            Ok(PdfDocumentSummary {
                page_count,
                format_version: c_buf_to_optional_string(&raw.format_version),
                title: c_buf_to_optional_string(&raw.title),
                author: c_buf_to_optional_string(&raw.author),
                subject: c_buf_to_optional_string(&raw.subject),
                keywords: c_buf_to_optional_string(&raw.keywords),
                creator: c_buf_to_optional_string(&raw.creator),
                producer: c_buf_to_optional_string(&raw.producer),
                creation_date: c_buf_to_optional_string(&raw.creation_date),
                modification_date: c_buf_to_optional_string(&raw.modification_date),
                needs_password: raw.needs_password != 0,
                engine_identifier: engine_identifier_from_version(&engine_version),
            })
        }
        1 => Err(PdfEngineError::PasswordRequired),
        2 => {
            let details = c_buf_to_optional_string(&raw.error_message)
                .unwrap_or_else(|| "failed to parse PDF".to_owned());
            Err(PdfEngineError::InvalidPdf { details })
        }
        _ => {
            let details = c_buf_to_optional_string(&raw.error_message)
                .unwrap_or_else(|| "internal MuPDF context error".to_owned());
            Err(PdfEngineError::EngineFailure { details })
        }
    }
}

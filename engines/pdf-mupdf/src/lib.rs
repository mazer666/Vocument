//! MuPDF adapter behind Vocument's `PdfEngine` interface (`ARCHITECTURE.md` §18, `ROADMAP.md` M0.1).
//!
//! No UI layer may depend on this crate directly.

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};
use vocument_core::pdf_engine::{PdfDocumentSummary, PdfEngine, PdfEngineDocument, PdfEngineError};

/// Identifier prefix of the PDF engine adapter.
pub const ENGINE_NAME: &str = "mupdf";

#[repr(C)]
struct RawMuPdfSummary {
    status_code: c_int,
    page_count: c_int,
    needs_password: c_int,
    format_version: [c_char; 64],
    title: [c_char; 512],
    author: [c_char; 512],
    subject: [c_char; 512],
    keywords: [c_char; 512],
    creator: [c_char; 512],
    producer: [c_char; 512],
    creation_date: [c_char; 128],
    modification_date: [c_char; 128],
    error_message: [c_char; 512],
    engine_version: [c_char; 64],
}

unsafe extern "C" {
    fn vocument_mupdf_inspect_pdf_bytes(
        bytes: *const c_uchar,
        len: usize,
        out_summary: *mut RawMuPdfSummary,
    ) -> c_int;
}

fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    // SAFETY: `buf` is zero-initialized and NUL-terminated by `mupdf_bridge.c`.
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let text = c_str.to_string_lossy().trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

/// Concrete MuPDF-backed `PdfEngineDocument` owned inside a `DocumentSession`.
#[derive(Debug)]
pub struct MuPdfDocumentHandle {
    summary: PdfDocumentSummary,
    _pdf_bytes: Vec<u8>,
}

impl PdfEngineDocument for MuPdfDocumentHandle {
    fn summary(&self) -> &PdfDocumentSummary {
        &self.summary
    }
}

/// Production `PdfEngine` adapter backed by MuPDF (`ARCHITECTURE.md` §18).
#[derive(Debug, Default, Clone, Copy)]
pub struct MuPdfEngineAdapter;

impl MuPdfEngineAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PdfEngine for MuPdfEngineAdapter {
    fn engine_identifier(&self) -> &str {
        ENGINE_NAME
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
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

        // SAFETY: `bytes` is a valid slice and `raw` is a valid stack-allocated struct.
        let status =
            unsafe { vocument_mupdf_inspect_pdf_bytes(bytes.as_ptr(), bytes.len(), &raw mut raw) };

        let engine_version =
            c_buf_to_optional_string(&raw.engine_version).unwrap_or_else(|| "unknown".to_owned());

        match status {
            0 => {
                let page_count = u32::try_from(raw.page_count).unwrap_or(0);
                let summary = PdfDocumentSummary {
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
                    engine_identifier: format!("mupdf-{engine_version}"),
                };

                Ok(Box::new(MuPdfDocumentHandle {
                    summary,
                    _pdf_bytes: bytes.to_vec(),
                }))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generates a minimal, specification-valid 2-page PDF with exact xref byte offsets and Info metadata.
    pub fn sample_two_page_pdf_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.7\n");

        let objects: [&[u8]; 5] = [
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n",
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>\nendobj\n",
            b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>\nendobj\n",
            b"5 0 obj\n<< /Title (Vocument M0.1 Test Contract) /Author (Vocument QA) /Subject (Architecture Verification) /Creator (Vocument Test Suite) /Producer (MuPDF Bridge Test) >>\nendobj\n",
        ];

        let mut offsets = Vec::with_capacity(objects.len());
        for obj in objects {
            offsets.push(out.len());
            out.extend_from_slice(obj);
        }

        let xref_offset = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for offset in offsets {
            out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    #[test]
    fn mupdf_adapter_reads_page_count_and_metadata_from_real_pdf() {
        let adapter = MuPdfEngineAdapter::new();
        let bytes = sample_two_page_pdf_bytes();
        let doc = adapter
            .open_pdf_bytes(&bytes)
            .expect("MuPDF must open valid 2-page PDF");
        let summary = doc.summary();

        assert_eq!(summary.page_count, 2);
        assert_eq!(summary.format_version.as_deref(), Some("PDF 1.7"));
        assert_eq!(
            summary.title.as_deref(),
            Some("Vocument M0.1 Test Contract")
        );
        assert_eq!(summary.author.as_deref(), Some("Vocument QA"));
        assert_eq!(
            summary.subject.as_deref(),
            Some("Architecture Verification")
        );
        assert_eq!(summary.creator.as_deref(), Some("Vocument Test Suite"));
        assert_eq!(summary.producer.as_deref(), Some("MuPDF Bridge Test"));
        assert!(!summary.needs_password);
        assert!(summary.engine_identifier.starts_with("mupdf-1."));
    }

    #[test]
    fn mupdf_adapter_rejects_non_pdf_bytes() {
        let adapter = MuPdfEngineAdapter::new();
        let err = adapter
            .open_pdf_bytes(b"not a pdf file")
            .expect_err("non-PDF bytes must fail");
        assert!(matches!(err, PdfEngineError::InvalidPdf { .. }));
    }
}

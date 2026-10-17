//! MuPDF adapter behind Vocument's `PdfEngine` interface (`ARCHITECTURE.md` §§7, 9, 17, 18, `ROADMAP.md` M0.1–M0.3).
//!
//! No UI layer may depend on this crate directly.

pub mod text_extract;

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar, c_uint};
use std::ptr::NonNull;
use std::sync::Mutex;
use vocument_core::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError,
    PdfPageGeometry, PdfPageStructuredText, PdfTileBitmap, PdfTileRenderRequest,
};

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

#[repr(C)]
struct RawMuPdfPageGeometry {
    status_code: c_int,
    width_milli_pt: c_uint,
    height_milli_pt: c_uint,
    error_message: [c_char; 512],
}

#[repr(C)]
struct RawMuPdfDisplayListHandle {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn vocument_mupdf_inspect_pdf_bytes(
        bytes: *const c_uchar,
        len: usize,
        out_summary: *mut RawMuPdfSummary,
    ) -> c_int;

    fn vocument_mupdf_query_page_geometry(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_geom: *mut RawMuPdfPageGeometry,
    ) -> c_int;

    fn vocument_mupdf_create_display_list(
        bytes: *const c_uchar,
        len: usize,
        page_index: c_int,
        out_handle: *mut *mut RawMuPdfDisplayListHandle,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_render_tile_from_display_list(
        handle: *mut RawMuPdfDisplayListHandle,
        scale_milli: c_uint,
        tile_x_px: c_uint,
        tile_y_px: c_uint,
        tile_width_px: c_uint,
        tile_height_px: c_uint,
        out_rgba_bytes: *mut c_uchar,
        out_rgba_len: usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_drop_display_list(handle: *mut RawMuPdfDisplayListHandle);
}

pub(crate) fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    // SAFETY: `buf` is zero-initialized and NUL-terminated by `mupdf_bridge.c`.
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let text = c_str.to_string_lossy().trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

/// Safe wrapper around the C `vocument_mupdf_display_list_handle_t` pointer.
struct OwnedDisplayListPtr(NonNull<RawMuPdfDisplayListHandle>);

// SAFETY: Access to the display list's dedicated `fz_context` is serialized by the `Mutex` in `MuPdfDisplayListHandle`.
unsafe impl Send for OwnedDisplayListPtr {}

impl Drop for OwnedDisplayListPtr {
    fn drop(&mut self) {
        // SAFETY: `self.0` was allocated by `vocument_mupdf_create_display_list`.
        unsafe {
            vocument_mupdf_drop_display_list(self.0.as_ptr());
        }
    }
}

/// Concrete MuPDF-backed `PdfEngineDisplayList` (`ARCHITECTURE.md` §7, `ROADMAP.md` M0.2).
pub struct MuPdfDisplayListHandle {
    geometry: PdfPageGeometry,
    raw_handle: Mutex<OwnedDisplayListPtr>,
}

impl std::fmt::Debug for MuPdfDisplayListHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MuPdfDisplayListHandle")
            .field("geometry", &self.geometry)
            .finish()
    }
}

impl PdfEngineDisplayList for MuPdfDisplayListHandle {
    fn page_geometry(&self) -> PdfPageGeometry {
        self.geometry
    }

    fn render_tile(&self, request: PdfTileRenderRequest) -> Result<PdfTileBitmap, PdfEngineError> {
        if request.tile_width_px == 0 || request.tile_height_px == 0 {
            return Err(PdfEngineError::InvalidTileDimensions {
                width: request.tile_width_px,
                height: request.tile_height_px,
            });
        }

        let byte_len = (request.tile_width_px as usize)
            .saturating_mul(request.tile_height_px as usize)
            .saturating_mul(4);
        let mut rgba_bytes = vec![255u8; byte_len];
        let mut err_buf = [0 as c_char; 512];

        let guard = self
            .raw_handle
            .lock()
            .map_err(|_| PdfEngineError::EngineFailure {
                details: "display-list mutex poisoned".to_owned(),
            })?;

        // SAFETY: `guard.0` is a valid non-null handle and `rgba_bytes` has `byte_len` capacity.
        let status = unsafe {
            vocument_mupdf_render_tile_from_display_list(
                guard.0.as_ptr(),
                request.scale_milli,
                request.tile_x_px,
                request.tile_y_px,
                request.tile_width_px,
                request.tile_height_px,
                rgba_bytes.as_mut_ptr(),
                rgba_bytes.len(),
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };

        if status != 0 {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "MuPDF failed to rasterize display-list tile".to_owned());
            return Err(PdfEngineError::EngineFailure { details });
        }

        Ok(PdfTileBitmap {
            width_px: request.tile_width_px,
            height_px: request.tile_height_px,
            stride_bytes: request.tile_width_px * 4,
            rgba_bytes,
        })
    }
}

/// Concrete MuPDF-backed `PdfEngineDocument` owned inside a `DocumentActor`.
#[derive(Debug)]
pub struct MuPdfDocumentHandle {
    summary: PdfDocumentSummary,
    pdf_bytes: Vec<u8>,
}

impl PdfEngineDocument for MuPdfDocumentHandle {
    fn summary(&self) -> &PdfDocumentSummary {
        &self.summary
    }

    fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
        if page_index >= self.summary.page_count {
            return Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            });
        }

        let mut raw_geom = RawMuPdfPageGeometry {
            status_code: 0,
            width_milli_pt: 0,
            height_milli_pt: 0,
            error_message: [0; 512],
        };

        let page_idx_int =
            c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            })?;

        // SAFETY: `self.pdf_bytes` is a valid buffer and `raw_geom` is stack-allocated.
        let status = unsafe {
            vocument_mupdf_query_page_geometry(
                self.pdf_bytes.as_ptr(),
                self.pdf_bytes.len(),
                page_idx_int,
                &raw mut raw_geom,
            )
        };

        match status {
            0 => Ok(PdfPageGeometry {
                page_index,
                width_milli_pt: raw_geom.width_milli_pt,
                height_milli_pt: raw_geom.height_milli_pt,
            }),
            4 => Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            }),
            _ => {
                let details = c_buf_to_optional_string(&raw_geom.error_message)
                    .unwrap_or_else(|| "failed to query PDF page geometry".to_owned());
                Err(PdfEngineError::EngineFailure { details })
            }
        }
    }

    fn page_label(&self, page_index: u32) -> Result<Option<String>, PdfEngineError> {
        text_extract::query_embedded_page_label(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
        )
    }

    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        text_extract::extract_structured_page_text(
            &self.pdf_bytes,
            self.summary.page_count,
            page_index,
        )
    }

    fn create_display_list(
        &self,
        page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
        if page_index >= self.summary.page_count {
            return Err(PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            });
        }

        let page_idx_int =
            c_int::try_from(page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
                page_index,
                page_count: self.summary.page_count,
            })?;

        let mut raw_ptr: *mut RawMuPdfDisplayListHandle = std::ptr::null_mut();
        let mut err_buf = [0 as c_char; 512];

        // SAFETY: `self.pdf_bytes` is valid and `raw_ptr` receives the allocated display list handle on status 0.
        let status = unsafe {
            vocument_mupdf_create_display_list(
                self.pdf_bytes.as_ptr(),
                self.pdf_bytes.len(),
                page_idx_int,
                &raw mut raw_ptr,
                err_buf.as_mut_ptr(),
                err_buf.len(),
            )
        };

        if status != 0 {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "MuPDF failed to create display list".to_owned());
            return Err(PdfEngineError::EngineFailure { details });
        }

        let non_null = NonNull::new(raw_ptr).ok_or_else(|| PdfEngineError::EngineFailure {
            details: "MuPDF returned null display list handle".to_owned(),
        })?;

        let geometry = self.page_geometry(page_index)?;
        Ok(Box::new(MuPdfDisplayListHandle {
            geometry,
            raw_handle: Mutex::new(OwnedDisplayListPtr(non_null)),
        }))
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
                    pdf_bytes: bytes.to_vec(),
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
    use std::path::PathBuf;

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
    fn mupdf_adapter_creates_display_list_and_renders_rgba8_tiles() {
        let adapter = MuPdfEngineAdapter::new();
        let bytes = sample_two_page_pdf_bytes();
        let doc = adapter.open_pdf_bytes(&bytes).expect("open 2-page PDF");

        let geom = doc.page_geometry(0).expect("page 0 geometry");
        assert_eq!(geom.page_index, 0);
        assert_eq!(geom.width_milli_pt, 595_000);
        assert_eq!(geom.height_milli_pt, 842_000);

        let dl = doc.create_display_list(0).expect("page 0 display list");
        assert_eq!(dl.page_geometry(), geom);

        let tile = dl
            .render_tile(PdfTileRenderRequest {
                page_index: 0,
                scale_milli: 1000,
                tile_x_px: 0,
                tile_y_px: 0,
                tile_width_px: 128,
                tile_height_px: 128,
            })
            .expect("render 128x128 tile at 1.0x");

        assert_eq!(tile.width_px, 128);
        assert_eq!(tile.height_px, 128);
        assert_eq!(tile.stride_bytes, 512);
        assert_eq!(tile.rgba_bytes.len(), 128 * 128 * 4);
    }

    #[test]
    fn mupdf_adapter_extracts_embedded_page_labels_and_structured_text() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_3_navigation_and_search.pdf");
        let bytes = std::fs::read(&fixture_path).expect("read M0.3 PDF fixture");
        let adapter = MuPdfEngineAdapter::new();
        let doc = adapter.open_pdf_bytes(&bytes).expect("open M0.3 PDF");

        assert_eq!(doc.summary().page_count, 4);
        assert_eq!(doc.page_label(0).expect("label 0").as_deref(), Some("i"));
        assert_eq!(doc.page_label(1).expect("label 1").as_deref(), Some("ii"));
        assert_eq!(doc.page_label(2).expect("label 2").as_deref(), Some("1"));
        assert_eq!(doc.page_label(3).expect("label 3").as_deref(), Some("2"));

        let page_1_text = doc.extract_page_text(1).expect("extract page 1 text");
        assert_eq!(page_1_text.embedded_page_label.as_deref(), Some("ii"));
        assert!(page_1_text.lines.len() >= 4);
        assert!(page_1_text.lines.iter().any(|l| l.text.contains("trust-")));
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

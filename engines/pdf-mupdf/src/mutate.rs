//! MuPDF M0.4 page rotation and standard PDF `/Highlight` annotation bridge (`ROADMAP.md` M0.4, `ROTATE-001`, `ANNOT-001`, `INTEROP-002`).

use std::ffi::{CStr, CString, c_char, c_int, c_uchar, c_uint};
use vocument_core::pdf_engine::{PdfEngineError, PdfHighlightAnnotation, PdfRectMilliPt};

#[repr(C)]
#[derive(Clone, Copy)]
struct RawMuPdfHighlight {
    page_index: c_int,
    x0_milli_pt: c_int,
    y0_milli_pt: c_int,
    x1_milli_pt: c_int,
    y1_milli_pt: c_int,
    color_r: c_uchar,
    color_g: c_uchar,
    color_b: c_uchar,
    opacity_milli: c_uint,
    annotation_id: [c_char; 128],
    author: [c_char; 256],
    contents: [c_char; 1024],
}

impl Default for RawMuPdfHighlight {
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
            opacity_milli: 450,
            annotation_id: [0; 128],
            author: [0; 256],
            contents: [0; 1024],
        }
    }
}

unsafe extern "C" {
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_query_page_rotation(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        out_rotation_deg: *mut c_int,
    ) -> c_int;

    fn vocument_mupdf_rotate_pages(
        bytes: *const u8,
        len: usize,
        page_indices: *const c_int,
        page_indices_len: usize,
        delta_degrees: c_int,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_query_page_highlights(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        out_annots: *mut RawMuPdfHighlight,
        max_annots: usize,
        out_annot_count: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_add_highlight_annot(
        bytes: *const u8,
        len: usize,
        spec: *const RawMuPdfHighlight,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;

    fn vocument_mupdf_remove_highlight_annot(
        bytes: *const u8,
        len: usize,
        page_index: c_int,
        annotation_id: *const c_char,
        out_removed: *mut RawMuPdfHighlight,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

fn c_buf_to_optional_string(buf: &[c_char]) -> Option<String> {
    if buf.is_empty() || buf[0] == 0 {
        return None;
    }
    // SAFETY: All C buffers are null-terminated.
    let c_str = unsafe { CStr::from_ptr(buf.as_ptr()) };
    let text = c_str.to_string_lossy().trim().to_owned();
    if text.is_empty() { None } else { Some(text) }
}

fn copy_str_to_c_buf(src: &str, dst: &mut [c_char]) {
    dst.fill(0);
    let bytes = src.as_bytes();
    let max_copy = bytes.len().min(dst.len().saturating_sub(1));
    for (i, &b) in bytes.iter().take(max_copy).enumerate() {
        dst[i] = b as c_char;
    }
}

fn raw_to_highlight(raw: &RawMuPdfHighlight, fallback_page_index: u32) -> PdfHighlightAnnotation {
    let page_index = u32::try_from(raw.page_index).unwrap_or(fallback_page_index);
    PdfHighlightAnnotation {
        annotation_id: c_buf_to_optional_string(&raw.annotation_id)
            .unwrap_or_else(|| format!("annot_page_{page_index}")),
        page_index,
        rect: PdfRectMilliPt {
            x0_milli_pt: raw.x0_milli_pt,
            y0_milli_pt: raw.y0_milli_pt,
            x1_milli_pt: raw.x1_milli_pt,
            y1_milli_pt: raw.y1_milli_pt,
        },
        color_rgb: [raw.color_r, raw.color_g, raw.color_b],
        opacity_milli: raw.opacity_milli,
        author: c_buf_to_optional_string(&raw.author),
        contents: c_buf_to_optional_string(&raw.contents),
    }
}

unsafe fn take_malloc_vec(ptr: *mut u8, len: usize) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        if !ptr.is_null() {
            // SAFETY: Allocated by `malloc` in `mupdf_mutate_bridge.c`.
            unsafe { vocument_mupdf_free_bytes(ptr) };
        }
        return Vec::new();
    }
    // SAFETY: `ptr` points to `len` valid bytes allocated by `malloc`.
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let vec = slice.to_vec();
    // SAFETY: Free C malloc buffer after copying into Rust `Vec<u8>`.
    unsafe { vocument_mupdf_free_bytes(ptr) };
    vec
}

pub fn query_page_rotation_degrees(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
) -> Result<u32, PdfEngineError> {
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
    let mut rot_deg: c_int = 0;

    // SAFETY: `pdf_bytes` is a valid slice and `rot_deg` is stack-allocated.
    let status = unsafe {
        vocument_mupdf_query_page_rotation(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            &raw mut rot_deg,
        )
    };
    match status {
        0 => Ok(u32::try_from(rot_deg).unwrap_or(0)),
        4 => Err(PdfEngineError::PageOutOfBounds {
            page_index,
            page_count,
        }),
        _ => Err(PdfEngineError::EngineFailure {
            details: "failed to query PDF page rotation".to_owned(),
        }),
    }
}

pub fn rotate_pages_in_pdf(
    pdf_bytes: &[u8],
    page_count: u32,
    page_indices: &[u32],
    delta_degrees: i32,
) -> Result<Vec<u8>, PdfEngineError> {
    if delta_degrees % 90 != 0 {
        return Err(PdfEngineError::InvalidRotationAngle {
            degrees: delta_degrees,
        });
    }
    let mut c_indices = Vec::with_capacity(page_indices.len());
    for &idx in page_indices {
        if idx >= page_count {
            return Err(PdfEngineError::PageOutOfBounds {
                page_index: idx,
                page_count,
            });
        }
        let c_idx = c_int::try_from(idx).map_err(|_| PdfEngineError::PageOutOfBounds {
            page_index: idx,
            page_count,
        })?;
        c_indices.push(c_idx);
    }

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: All slices and output pointers are valid.
    let status = unsafe {
        vocument_mupdf_rotate_pages(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            delta_degrees,
            &raw mut out_ptr,
            &raw mut out_len,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    // SAFETY: `out_ptr` was allocated by `mupdf_mutate_bridge.c` if non-null.
    let updated_bytes = unsafe { take_malloc_vec(out_ptr, out_len) };

    if status != 0 || updated_bytes.is_empty() {
        let details = c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| "MuPDF failed to rotate PDF pages".to_owned());
        return Err(PdfEngineError::EngineFailure { details });
    }
    Ok(updated_bytes)
}

pub fn query_page_highlights(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
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

    const MAX_ANNOTS: usize = 512;
    let mut raw_annots = vec![RawMuPdfHighlight::default(); MAX_ANNOTS];
    let mut count: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `raw_annots` has capacity `MAX_ANNOTS`.
    let status = unsafe {
        vocument_mupdf_query_page_highlights(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            raw_annots.as_mut_ptr(),
            MAX_ANNOTS,
            &raw mut count,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| "MuPDF failed to query page highlight annotations".to_owned());
        return Err(PdfEngineError::EngineFailure { details });
    }

    let returned = count.min(MAX_ANNOTS);
    Ok(raw_annots
        .iter()
        .take(returned)
        .map(|r| raw_to_highlight(r, page_index))
        .collect())
}

pub fn add_highlight_annotation_to_pdf(
    pdf_bytes: &[u8],
    page_count: u32,
    annotation: &PdfHighlightAnnotation,
) -> Result<Vec<u8>, PdfEngineError> {
    if annotation.page_index >= page_count {
        return Err(PdfEngineError::PageOutOfBounds {
            page_index: annotation.page_index,
            page_count,
        });
    }
    let page_idx_int =
        c_int::try_from(annotation.page_index).map_err(|_| PdfEngineError::PageOutOfBounds {
            page_index: annotation.page_index,
            page_count,
        })?;

    let mut raw = RawMuPdfHighlight {
        page_index: page_idx_int,
        x0_milli_pt: annotation.rect.x0_milli_pt,
        y0_milli_pt: annotation.rect.y0_milli_pt,
        x1_milli_pt: annotation.rect.x1_milli_pt,
        y1_milli_pt: annotation.rect.y1_milli_pt,
        color_r: annotation.color_rgb[0],
        color_g: annotation.color_rgb[1],
        color_b: annotation.color_rgb[2],
        opacity_milli: annotation.opacity_milli.clamp(50, 1000),
        annotation_id: [0; 128],
        author: [0; 256],
        contents: [0; 1024],
    };
    copy_str_to_c_buf(&annotation.annotation_id, &mut raw.annotation_id);
    if let Some(author) = &annotation.author {
        copy_str_to_c_buf(author, &mut raw.author);
    }
    if let Some(contents) = &annotation.contents {
        copy_str_to_c_buf(contents, &mut raw.contents);
    }

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `raw` and output pointers are valid.
    let status = unsafe {
        vocument_mupdf_add_highlight_annot(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            &raw const raw,
            &raw mut out_ptr,
            &raw mut out_len,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    // SAFETY: `out_ptr` was allocated by `mupdf_mutate_bridge.c` if non-null.
    let updated_bytes = unsafe { take_malloc_vec(out_ptr, out_len) };

    if status != 0 || updated_bytes.is_empty() {
        let details = c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| "MuPDF failed to add highlight annotation".to_owned());
        return Err(PdfEngineError::EngineFailure { details });
    }
    Ok(updated_bytes)
}

pub fn remove_highlight_annotation_from_pdf(
    pdf_bytes: &[u8],
    page_count: u32,
    page_index: u32,
    annotation_id: &str,
) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
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
    let c_id = CString::new(annotation_id).map_err(|_| PdfEngineError::AnnotationNotFound {
        page_index,
        annotation_id: annotation_id.to_owned(),
    })?;

    let mut raw_removed = RawMuPdfHighlight::default();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: All pointers are valid.
    let status = unsafe {
        vocument_mupdf_remove_highlight_annot(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_idx_int,
            c_id.as_ptr(),
            &raw mut raw_removed,
            &raw mut out_ptr,
            &raw mut out_len,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    // SAFETY: `out_ptr` was allocated by `mupdf_mutate_bridge.c` if non-null.
    let updated_bytes = unsafe { take_malloc_vec(out_ptr, out_len) };

    match status {
        0 if !updated_bytes.is_empty() => {
            Ok((raw_to_highlight(&raw_removed, page_index), updated_bytes))
        }
        5 => Err(PdfEngineError::AnnotationNotFound {
            page_index,
            annotation_id: annotation_id.to_owned(),
        }),
        _ => {
            let details = c_buf_to_optional_string(&err_buf)
                .unwrap_or_else(|| "MuPDF failed to remove highlight annotation".to_owned());
            Err(PdfEngineError::EngineFailure { details })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{MuPdfEngineAdapter, tests::sample_two_page_pdf_bytes};
    use vocument_core::pdf_engine::{PdfEngine, PdfHighlightAnnotation, PdfRectMilliPt};

    #[test]
    fn mupdf_adapter_rotates_pages_and_roundtrips_standard_highlight_annotations() {
        let adapter = MuPdfEngineAdapter::new();
        let bytes = sample_two_page_pdf_bytes();
        let mut doc = adapter.open_pdf_bytes(&bytes).expect("open 2-page PDF");

        assert_eq!(doc.page_rotation_degrees(0).expect("initial rotation"), 0);
        let geom_before = doc.page_geometry(0).expect("initial geometry");
        assert_eq!(geom_before.width_milli_pt, 595_000);
        assert_eq!(geom_before.height_milli_pt, 842_000);

        let _ = doc.rotate_pages(&[0], 90).expect("rotate page 0 by +90");
        assert_eq!(doc.page_rotation_degrees(0).expect("rotated 90"), 90);
        let geom_rotated = doc.page_geometry(0).expect("rotated geometry");
        assert_eq!(geom_rotated.width_milli_pt, 842_000);
        assert_eq!(geom_rotated.height_milli_pt, 595_000);

        let _ = doc.rotate_pages(&[0], -90).expect("rotate page 0 by -90");
        assert_eq!(doc.page_rotation_degrees(0).expect("restored 0"), 0);

        let highlight = PdfHighlightAnnotation {
            annotation_id: "annot_test_0001".to_owned(),
            page_index: 0,
            rect: PdfRectMilliPt {
                x0_milli_pt: 72_000,
                y0_milli_pt: 120_000,
                x1_milli_pt: 320_000,
                y1_milli_pt: 140_000,
            },
            color_rgb: [255, 230, 80],
            opacity_milli: 450,
            author: Some("Vocument Reviewer".to_owned()),
            contents: Some("Important clause highlight".to_owned()),
        };

        let _ = doc
            .add_highlight_annotation(&highlight)
            .expect("add highlight annotation");
        let on_page = doc
            .page_highlight_annotations(0)
            .expect("query page 0 highlights");
        assert_eq!(on_page.len(), 1);
        assert_eq!(on_page[0].annotation_id, "annot_test_0001");
        assert_eq!(on_page[0].author.as_deref(), Some("Vocument Reviewer"));
        assert_eq!(
            on_page[0].contents.as_deref(),
            Some("Important clause highlight")
        );

        let (removed, _) = doc
            .remove_highlight_annotation(0, "annot_test_0001")
            .expect("remove highlight annotation");
        assert_eq!(removed.annotation_id, "annot_test_0001");
        assert!(
            doc.page_highlight_annotations(0)
                .expect("query after delete")
                .is_empty()
        );
    }
}

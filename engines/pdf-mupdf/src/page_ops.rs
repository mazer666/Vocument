//! Safe Rust wrappers for Page Workbench operations via the native MuPDF bridge (`M3`).

use vocument_core::pdf_engine::PdfEngineError;

unsafe extern "C" {
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_reorder_pages_ext(
        bytes: *const u8,
        len: usize,
        order: *const i32,
        order_len: usize,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_duplicate_page_ext(
        bytes: *const u8,
        len: usize,
        page_index: i32,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_delete_pages_ext(
        bytes: *const u8,
        len: usize,
        page_indices: *const i32,
        page_indices_len: usize,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_extract_pages(
        bytes: *const u8,
        len: usize,
        page_indices: *const i32,
        page_indices_len: usize,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;
}

fn map_bridge_status(status: i32, op_name: &str) -> PdfEngineError {
    match status {
        4 => PdfEngineError::OperationBlockedBySignature {
            details: format!(
                "{op_name} blocked by preflight check: document contains active digital signatures"
            ),
        },
        _ => PdfEngineError::EngineFailure {
            details: format!("{op_name} failed with code {status}"),
        },
    }
}

/// Reorders the pages of a PDF document according to `order` (0-indexed permutation).
/// Refuses if the document contains active digital signatures.
pub fn reorder_pages(pdf_bytes: &[u8], order: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
    reorder_pages_ext(pdf_bytes, order, false)
}

/// Reorders the pages with explicit signature invalidation control.
pub fn reorder_pages_ext(
    pdf_bytes: &[u8],
    order: &[u32],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "cannot reorder empty PDF bytes".to_owned(),
        });
    }
    if order.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "page order cannot be empty".to_owned(),
        });
    }

    let c_order: Vec<i32> = order.iter().map(|&idx| idx as i32).collect();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_reorder_pages_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_order.as_ptr(),
            c_order.len(),
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(map_bridge_status(status, "vocument_mupdf_reorder_pages"));
    }

    let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_ptr) };
    Ok(bytes)
}

/// Duplicates the page at `page_index` (0-indexed), inserting the copy immediately after it.
/// Refuses if the document contains active digital signatures.
pub fn duplicate_page(pdf_bytes: &[u8], page_index: u32) -> Result<Vec<u8>, PdfEngineError> {
    duplicate_page_ext(pdf_bytes, page_index, false)
}

/// Duplicates the page with explicit signature invalidation control.
pub fn duplicate_page_ext(
    pdf_bytes: &[u8],
    page_index: u32,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "cannot duplicate page in empty PDF bytes".to_owned(),
        });
    }

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_duplicate_page_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            page_index as i32,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(map_bridge_status(status, "vocument_mupdf_duplicate_page"));
    }

    let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_ptr) };
    Ok(bytes)
}

/// Deletes the specified `page_indices` from the PDF document.
/// Fails if all pages would be deleted or if active digital signatures exist.
pub fn delete_pages(pdf_bytes: &[u8], page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
    delete_pages_ext(pdf_bytes, page_indices, false)
}

/// Deletes the specified `page_indices` with explicit signature invalidation control.
pub fn delete_pages_ext(
    pdf_bytes: &[u8],
    page_indices: &[u32],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "cannot delete pages in empty PDF bytes".to_owned(),
        });
    }
    if page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "page indices to delete cannot be empty".to_owned(),
        });
    }

    let c_indices: Vec<i32> = page_indices.iter().map(|&idx| idx as i32).collect();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_delete_pages_ext(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(map_bridge_status(status, "vocument_mupdf_delete_pages"));
    }

    let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_ptr) };
    Ok(bytes)
}

/// Extracts the specified `page_indices` into a new standalone PDF document.
pub fn extract_pages(pdf_bytes: &[u8], page_indices: &[u32]) -> Result<Vec<u8>, PdfEngineError> {
    if pdf_bytes.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "cannot extract pages from empty PDF bytes".to_owned(),
        });
    }
    if page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "page indices to extract cannot be empty".to_owned(),
        });
    }

    let c_indices: Vec<i32> = page_indices.iter().map(|&idx| idx as i32).collect();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_extract_pages(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(PdfEngineError::EngineFailure {
            details: format!("vocument_mupdf_extract_pages failed with code {status}"),
        });
    }

    let bytes = unsafe { std::slice::from_raw_parts(out_ptr, out_len).to_vec() };
    unsafe { vocument_mupdf_free_bytes(out_ptr) };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &[u8] = include_bytes!("../../../tests/corpus/m0_1_sample_two_pages.pdf");

    #[test]
    fn test_workbench_reorder_pages() {
        let reordered = reorder_pages(FIXTURE, &[1, 0]).expect("reorder_pages should succeed");
        assert!(reordered.len() > 100);
        assert_eq!(&reordered[0..4], b"%PDF");

        assert!(reorder_pages(FIXTURE, &[0, 0]).is_err());
        assert!(reorder_pages(FIXTURE, &[0]).is_err());
        assert!(reorder_pages(FIXTURE, &[0, 2]).is_err());
    }

    #[test]
    fn test_workbench_duplicate_page() {
        let duplicated = duplicate_page(FIXTURE, 0).expect("duplicate_page should succeed");
        assert!(duplicated.len() > 100);
        assert_eq!(&duplicated[0..4], b"%PDF");

        assert!(duplicate_page(FIXTURE, 99).is_err());
    }

    #[test]
    fn test_workbench_delete_pages() {
        let deleted = delete_pages(FIXTURE, &[0]).expect("delete_pages should succeed");
        assert!(deleted.len() > 100);
        assert_eq!(&deleted[0..4], b"%PDF");

        assert!(delete_pages(FIXTURE, &[0, 1]).is_err());
    }

    #[test]
    fn test_workbench_extract_pages() {
        let extracted = extract_pages(FIXTURE, &[1]).expect("extract_pages should succeed");
        assert!(extracted.len() > 100);
        assert_eq!(&extracted[0..4], b"%PDF");

        assert!(extract_pages(FIXTURE, &[99]).is_err());
    }
}

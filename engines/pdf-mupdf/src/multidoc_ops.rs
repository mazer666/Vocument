//! Safe Rust wrappers for Multi-Document Operations (Merge, Insert, Split) via the native MuPDF bridge (`M3.2`).

use vocument_core::pdf_engine::PdfEngineError;

unsafe extern "C" {
    fn vocument_mupdf_free_bytes(ptr: *mut u8);

    fn vocument_mupdf_merge_documents_ext(
        documents: *const *const u8,
        document_lens: *const usize,
        document_count: usize,
        allow_signature_invalidation: i32,
        out_pdf_bytes: *mut *mut u8,
        out_pdf_len: *mut usize,
    ) -> i32;

    fn vocument_mupdf_insert_pages_ext(
        target_bytes: *const u8,
        target_len: usize,
        source_bytes: *const u8,
        source_len: usize,
        source_page_indices: *const i32,
        source_page_indices_len: usize,
        insert_at: i32,
        allow_signature_invalidation: i32,
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
        5 => PdfEngineError::InvalidPdf {
            details: format!(
                "{op_name} refused fail-closed: Tagged PDF structural conflict (cannot combine tagged and untagged documents without violating ISO 32000-1 / PDF/UA structural integrity)"
            ),
        },
        _ => PdfEngineError::EngineFailure {
            details: format!("{op_name} failed with code {status}"),
        },
    }
}

/// Merges multiple PDF document byte buffers into a single document (`M3.2`).
/// Refuses if any input document contains active digital signatures unless explicit permission is granted.
pub fn merge_documents_ext(
    source_documents: &[&[u8]],
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if source_documents.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "cannot merge an empty list of documents".to_owned(),
        });
    }

    let mut doc_ptrs = Vec::with_capacity(source_documents.len());
    let mut doc_lens = Vec::with_capacity(source_documents.len());

    for doc in source_documents {
        if doc.is_empty() {
            return Err(PdfEngineError::InvalidPdf {
                details: "cannot merge empty PDF buffer".to_owned(),
            });
        }
        doc_ptrs.push(doc.as_ptr());
        doc_lens.push(doc.len());
    }

    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_merge_documents_ext(
            doc_ptrs.as_ptr(),
            doc_lens.as_ptr(),
            source_documents.len(),
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(map_bridge_status(status, "vocument_mupdf_merge_documents"));
    }

    let result_bytes = unsafe {
        let slice = std::slice::from_raw_parts(out_ptr, out_len);
        let vec = slice.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        vec
    };

    Ok(result_bytes)
}

/// Inserts selected pages from an external PDF document into a target PDF document at `insert_at` (0-based) (`M3.2`).
pub fn insert_pages_ext(
    target_pdf: &[u8],
    source_pdf: &[u8],
    source_page_indices: &[u32],
    insert_at: u32,
    allow_signature_invalidation: bool,
) -> Result<Vec<u8>, PdfEngineError> {
    if target_pdf.is_empty() || source_pdf.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "target or source PDF buffer cannot be empty".to_owned(),
        });
    }
    if source_page_indices.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "source page indices cannot be empty".to_owned(),
        });
    }

    let c_indices: Vec<i32> = source_page_indices.iter().map(|&idx| idx as i32).collect();
    let mut out_ptr: *mut u8 = std::ptr::null_mut();
    let mut out_len: usize = 0;

    let status = unsafe {
        vocument_mupdf_insert_pages_ext(
            target_pdf.as_ptr(),
            target_pdf.len(),
            source_pdf.as_ptr(),
            source_pdf.len(),
            c_indices.as_ptr(),
            c_indices.len(),
            insert_at as i32,
            if allow_signature_invalidation { 1 } else { 0 },
            &mut out_ptr,
            &mut out_len,
        )
    };

    if status != 0 || out_ptr.is_null() {
        return Err(map_bridge_status(status, "vocument_mupdf_insert_pages"));
    }

    let result_bytes = unsafe {
        let slice = std::slice::from_raw_parts(out_ptr, out_len);
        let vec = slice.to_vec();
        vocument_mupdf_free_bytes(out_ptr);
        vec
    };

    Ok(result_bytes)
}

/// Splits the source document into multiple standalone PDFs according to the specified page partitions (`M3.2`).
pub fn split_document(
    source_pdf: &[u8],
    split_ranges: &[Vec<u32>],
) -> Result<Vec<Vec<u8>>, PdfEngineError> {
    if split_ranges.is_empty() {
        return Err(PdfEngineError::InvalidPdf {
            details: "split ranges cannot be empty".to_owned(),
        });
    }

    let mut parts = Vec::with_capacity(split_ranges.len());
    for range in split_ranges {
        if range.is_empty() {
            return Err(PdfEngineError::InvalidPdf {
                details: "split partition cannot have empty page range".to_owned(),
            });
        }
        let part_bytes = crate::page_ops::extract_pages(source_pdf, range)?;
        parts.push(part_bytes);
    }

    Ok(parts)
}

//! Inspection of the links and actions of a document (`INSPECT-001`).
//!
//! The engine reports the facts: which action lies where, of which type it
//! is, and what it points at. Whether an action is dangerous is a decision of
//! the core.

use std::ffi::{CStr, c_char, c_int, c_uchar};
use vocument_core::document_inspection::{PdfActionInfo, PdfActionKind};
use vocument_core::pdf_engine::{PdfEngineError, PdfRectMilliPt};

/// The maximum number of actions that are reported for one document.
///
/// A document with more actions than this is already beyond what a reviewer
/// can look at in one go; the report says so instead of silently truncating.
pub const MAX_ACTIONS: usize = 512;

#[repr(C)]
#[derive(Clone, Copy)]
struct RawAction {
    has_page: c_int,
    page_index: c_int,
    x0_milli_pt: c_int,
    y0_milli_pt: c_int,
    x1_milli_pt: c_int,
    y1_milli_pt: c_int,
    kind: [c_char; 64],
    target: [c_char; 1024],
}

unsafe extern "C" {
    fn vocument_mupdf_inspect_document_actions(
        bytes: *const c_uchar,
        len: usize,
        out_actions: *mut RawAction,
        max_actions: usize,
        out_action_count: *mut usize,
        err_buf: *mut c_char,
        err_buf_len: usize,
    ) -> c_int;
}

/// Reads the links and actions of the document.
pub fn document_actions(pdf_bytes: &[u8]) -> Result<Vec<PdfActionInfo>, PdfEngineError> {
    let mut actions: Vec<RawAction> = vec![
        RawAction {
            has_page: 0,
            page_index: 0,
            x0_milli_pt: 0,
            y0_milli_pt: 0,
            x1_milli_pt: 0,
            y1_milli_pt: 0,
            kind: [0; 64],
            target: [0; 1024],
        };
        MAX_ACTIONS
    ];
    let mut count: usize = 0;
    let mut err_buf = [0 as c_char; 512];

    // SAFETY: `pdf_bytes` is a valid buffer, `actions` has room for
    // `MAX_ACTIONS` entries, the output pointers are valid.
    let status = unsafe {
        vocument_mupdf_inspect_document_actions(
            pdf_bytes.as_ptr(),
            pdf_bytes.len(),
            actions.as_mut_ptr(),
            MAX_ACTIONS,
            &raw mut count,
            err_buf.as_mut_ptr(),
            err_buf.len(),
        )
    };

    if status != 0 {
        let details = crate::c_buf_to_optional_string(&err_buf)
            .unwrap_or_else(|| format!("MuPDF failed to inspect the document (status {status})"));
        return Err(PdfEngineError::EngineFailure { details });
    }

    let mut result = Vec::with_capacity(count.min(MAX_ACTIONS));
    for raw in actions.iter().take(count.min(MAX_ACTIONS)) {
        // SAFETY: the C bridge writes NUL-terminated strings into the arrays.
        let kind_name = unsafe { CStr::from_ptr(raw.kind.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let target = unsafe { CStr::from_ptr(raw.target.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        let page_index = if raw.has_page != 0 {
            Some(raw.page_index.max(0) as u32)
        } else {
            None
        };
        let rect = page_index.map(|_| PdfRectMilliPt {
            x0_milli_pt: raw.x0_milli_pt,
            y0_milli_pt: raw.y0_milli_pt,
            x1_milli_pt: raw.x1_milli_pt,
            y1_milli_pt: raw.y1_milli_pt,
        });
        result.push(PdfActionInfo::new(
            page_index,
            rect,
            PdfActionKind::from_engine_name(&kind_name),
            target,
        ));
    }
    Ok(result)
}

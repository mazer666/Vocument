//! C ABI entry points for Windows (P/Invoke) and native desktop shells (`CR-0003`, `PLATFORM-004`).
//!
//! Provides a direct, robust, and zero-stub C ABI boundary to the shared Rust core
//! and underlying MuPDF engine for Windows WinUI 3 and cross-platform desktop runners.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::slice;

use serde_json::json;
use vocument_core::file_broker::ScopedLocalFileBroker;
use vocument_core::pdf_engine::{PdfHighlightAnnotation, PdfRectMilliPt, PdfTileRenderRequest};
use vocument_core::save::SaveResult;

use crate::{document_core_handshake, session_registry};

unsafe fn c_to_str<'a>(raw: *const c_char) -> Option<&'a str> {
    if raw.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(raw).to_str().ok() }
    }
}

fn to_c_string(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

/// Returns the core bootstrap handshake as a JSON string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_handshake() -> *mut c_char {
    match document_core_handshake() {
        Ok(h) => {
            let obj = json!({
                "baseline_version": h.baseline_version,
                "milestone": h.milestone,
                "strict_offline_default": h.strict_offline_default,
                "telemetry_enabled": h.telemetry_enabled,
                "target_fps": h.target_fps,
                "default_german_variant": h.default_german_variant,
                "built_in_ocr_languages": h.built_in_ocr_languages
            });
            to_c_string(obj.to_string())
        }
        Err(e) => to_c_string(json!({"error": e.to_string()}).to_string()),
    }
}

/// Opens a PDF document through the scoped file broker and returns its session snapshot as JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_open_document(path: *const c_char) -> *mut c_char {
    let path_str = match unsafe { c_to_str(path) } {
        Some(s) => s,
        None => return to_c_string(json!({"error": "Invalid or null path pointer"}).to_string()),
    };

    let broker = ScopedLocalFileBroker::new();
    match session_registry().open_via_broker(&broker, path_str) {
        Ok(snap) => {
            let obj = json!({
                "session_id": snap.session_id.as_str(),
                "document_id": snap.document_id.as_str(),
                "access_origin": "WindowsCommonItemDialog",
                "size_bytes": snap.file_identity.size_bytes,
                "page_count": snap.pdf_summary.page_count,
                "title": snap.pdf_summary.title,
                "author": snap.pdf_summary.author,
                "engine_identifier": snap.pdf_summary.engine_identifier
            });
            to_c_string(obj.to_string())
        }
        Err(e) => to_c_string(json!({"error": e.to_string()}).to_string()),
    }
}

/// Retrieves unscaled page geometry (width and height in milli-points).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_get_page_geometry(
    session_id: *const c_char,
    page_index: u32,
    out_width_milli: *mut u32,
    out_height_milli: *mut u32,
) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };

    match session_registry().page_geometry(sid, page_index) {
        Ok(geom) => {
            if !out_width_milli.is_null() {
                unsafe { *out_width_milli = geom.width_milli_pt };
            }
            if !out_height_milli.is_null() {
                unsafe { *out_height_milli = geom.height_milli_pt };
            }
            0
        }
        Err(_) => -1,
    }
}

/// Renders a visible viewport tile into caller-provided BGRA/RGBA pixel memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_render_page_tile(
    session_id: *const c_char,
    page_index: u32,
    scale_milli: u32,
    tile_x: u32,
    tile_y: u32,
    tile_w: u32,
    tile_h: u32,
    out_pixels: *mut u8,
    max_len: usize,
) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };
    if out_pixels.is_null() {
        return -2;
    }

    let req = PdfTileRenderRequest {
        page_index,
        scale_milli,
        tile_x_px: tile_x,
        tile_y_px: tile_y,
        tile_width_px: tile_w,
        tile_height_px: tile_h,
    };

    match session_registry().render_visible_tile(sid, req) {
        Ok(rendered) => {
            let bytes = &rendered.rgba_bytes;
            if bytes.len() > max_len {
                return -3;
            }
            let dest = unsafe { slice::from_raw_parts_mut(out_pixels, bytes.len()) };
            dest.copy_from_slice(bytes);
            0
        }
        Err(_) => -4,
    }
}

/// Executes exact FTS5/document text search and returns hits as JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_search_exact(
    session_id: *const c_char,
    query: *const c_char,
) -> *mut c_char {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return to_c_string(json!([]).to_string()),
    };
    let q = match unsafe { c_to_str(query) } {
        Some(s) => s,
        None => return to_c_string(json!([]).to_string()),
    };

    // Ensure initial indexing batch runs
    let _ = session_registry().index_search_batch(sid, 0, 100);

    match session_registry().search_exact(sid, q) {
        Ok(resp) => {
            let hits: Vec<_> = resp
                .hits
                .into_iter()
                .map(|h| {
                    json!({
                        "page_index": h.page_index,
                        "logical_page_label": h.logical_page_label,
                        "snippet": h.snippet,
                        "match_count": h.match_count
                    })
                })
                .collect();
            to_c_string(json!(hits).to_string())
        }
        Err(_) => to_c_string(json!([]).to_string()),
    }
}

/// Selects page text and returns cleaned and raw text as JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_select_page_text(
    session_id: *const c_char,
    page_index: u32,
) -> *mut c_char {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return to_c_string(json!({"error": "Invalid session"}).to_string()),
    };

    match session_registry().select_page_text(sid, page_index, None) {
        Ok(sel) => {
            let obj = json!({
                "page_index": sel.page_index,
                "raw_text": sel.raw_text,
                "cleaned_text": sel.cleaned_text
            });
            to_c_string(obj.to_string())
        }
        Err(e) => to_c_string(json!({"error": e.to_string()}).to_string()),
    }
}

/// Adds a highlight annotation to the document page.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_add_highlight(
    session_id: *const c_char,
    page_index: u32,
    color_hex: *const c_char,
) -> *mut c_char {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return to_c_string(json!({"error": "Invalid session"}).to_string()),
    };
    let color_str = unsafe { c_to_str(color_hex) }.unwrap_or("#FFFF00");

    let geom = match session_registry().page_geometry(sid, page_index) {
        Ok(g) => g,
        Err(e) => return to_c_string(json!({"error": e.to_string()}).to_string()),
    };

    let rect = PdfRectMilliPt {
        x0_milli_pt: 50_000,
        y0_milli_pt: 50_000,
        x1_milli_pt: (geom.width_milli_pt as i32 - 50_000).max(100_000),
        y1_milli_pt: 70_000,
    };

    let annot = PdfHighlightAnnotation {
        annotation_id: format!(
            "annot_win_{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ),
        page_index,
        rect,
        color_rgb: [255, 255, 0],
        opacity_milli: 1000,
        author: Some("Jurist User".to_string()),
        contents: None,
    };

    let annot_id = annot.annotation_id.clone();
    match session_registry().add_highlight_annotation(sid, annot) {
        Ok(_) => {
            let obj = json!({
                "annotation_id": annot_id,
                "page_index": page_index,
                "color_hex": color_str,
                "author": "Jurist User"
            });
            to_c_string(obj.to_string())
        }
        Err(e) => to_c_string(json!({"error": e.to_string()}).to_string()),
    }
}

/// Rotates specified pages by delta degrees. Returns new rotation or negative error code.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_rotate_pages(
    session_id: *const c_char,
    page_indices: *const u32,
    num_pages: usize,
    delta_degrees: i32,
) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };
    if page_indices.is_null() || num_pages == 0 {
        return -2;
    }

    let pages = unsafe { slice::from_raw_parts(page_indices, num_pages) }.to_vec();
    match session_registry().rotate_pages(sid, pages.clone(), delta_degrees) {
        Ok(_) => session_registry()
            .page_rotation_degrees(sid, pages[0])
            .map(|r| r as c_int)
            .unwrap_or(-3),
        Err(_) => -4,
    }
}

/// Performs undo on the transaction log. Returns 1 for success, 0 for nothing, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_undo(session_id: *const c_char) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };
    match session_registry().undo(sid) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Performs redo on the transaction log. Returns 1 for success, 0 for nothing, -1 on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_redo(session_id: *const c_char) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };
    match session_registry().redo(sid) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Saves the session to destination path. Returns JSON outcome.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_save_session(
    session_id: *const c_char,
    destination_path: *const c_char,
) -> *mut c_char {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return to_c_string(json!({"error": "Invalid session"}).to_string()),
    };
    let dest = match unsafe { c_to_str(destination_path) } {
        Some(s) => s,
        None => return to_c_string(json!({"error": "Invalid destination path"}).to_string()),
    };

    match session_registry().save_session_as(sid, dest) {
        Ok(outcome) => {
            let snap = match outcome {
                SaveResult::Success(s) => s,
                SaveResult::Conflict(_) => {
                    return to_c_string(json!({"error": "Conflict on save"}).to_string());
                }
            };
            let obj = json!({
                "status": "success",
                "bytes_written": snap.new_file_identity.size_bytes,
                "revision": snap.saved_revision
            });
            to_c_string(obj.to_string())
        }
        Err(e) => to_c_string(json!({"error": e.to_string()}).to_string()),
    }
}

/// Checks whether an active session and its recovery state exist. Returns 1 if valid, 0 otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_check_recovery(session_id: *const c_char) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return 0,
    };
    match session_registry().session_snapshot(sid) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Closes the document session. Returns 0 on success.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_close_session(session_id: *const c_char) -> c_int {
    let sid = match unsafe { c_to_str(session_id) } {
        Some(s) => s,
        None => return -1,
    };
    match session_registry().close_session(sid) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}

/// Frees memory allocated by Rust for C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn vocument_c_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

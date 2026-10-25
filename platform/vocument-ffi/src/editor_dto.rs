//! UniFFI types and exported bridge functions for M4.3 Native Platform Editing Canvas (`ADR-0020`, `UI-009`, `M3-M7-DEVELOPMENT-PLAN.md`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use vocument_core::config::PackageLimitsConfig;
use vocument_layout::editor::FlowEditorState;
use vocument_layout::geometry::Rect;
use vocument_layout::tree::{LayoutBoxKind, LayoutPage};
use vocument_pdoc::container::{PdocContainer, PdocFileFingerprint, PdocPackage};

use crate::DocumentCoreError;

/// Rectangle in document point coordinates.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FfiEditorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rect> for FfiEditorRect {
    fn from(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Selection highlight rectangle positioned on a specific page.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct FfiEditorSelectionRect {
    pub page_index: u32,
    pub rect: FfiEditorRect,
}

/// Positioned text run within a layout line.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiEditorRunSnapshot {
    pub text: String,
    pub rect: FfiEditorRect,
    pub font_family: String,
    pub font_size_pt: f32,
    pub is_bold: bool,
    pub is_italic: bool,
    pub color_hex: String,
}

/// Formatted text line on an editor page.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiEditorLineSnapshot {
    pub rect: FfiEditorRect,
    pub baseline_y: f32,
    pub runs: Vec<FfiEditorRunSnapshot>,
}

/// Single page visual layout snapshot ready for native canvas rendering.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiEditorPageSnapshot {
    pub page_index: u32,
    pub width_pt: f32,
    pub height_pt: f32,
    pub lines: Vec<FfiEditorLineSnapshot>,
}

/// Comprehensive immutable snapshot of the native platform editor state.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiEditorSessionSnapshot {
    pub editor_id: String,
    pub title: String,
    pub file_path: Option<String>,
    pub total_pages: u32,
    pub active_block_id: String,
    pub cursor_offset: u32,
    pub cursor_page_index: u32,
    pub cursor_rect: Option<FfiEditorRect>,
    pub selection_rects: Vec<FfiEditorSelectionRect>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub plain_text: String,
    pub pages: Vec<FfiEditorPageSnapshot>,
}

struct FlowEditorSession {
    id: String,
    title: String,
    file_path: Option<String>,
    fingerprint: Option<PdocFileFingerprint>,
    state: FlowEditorState,
}

static EDITOR_REGISTRY: OnceLock<Mutex<HashMap<String, FlowEditorSession>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, FlowEditorSession>> {
    EDITOR_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_page_snapshot(page_index: u32, page: &LayoutPage) -> FfiEditorPageSnapshot {
    let mut lines = Vec::new();
    for b in page.all_boxes() {
        if let LayoutBoxKind::TextLine(l) = &b.kind {
            let runs = l
                .runs
                .iter()
                .map(|r| FfiEditorRunSnapshot {
                    text: r.text.clone(),
                    rect: r.bounds.into(),
                    font_family: r.font_family.clone(),
                    font_size_pt: r.font_size_pt,
                    is_bold: r.font_style.bold,
                    is_italic: r.font_style.italic,
                    color_hex: r.color.to_hex_string(),
                })
                .collect();

            lines.push(FfiEditorLineSnapshot {
                rect: l.bounds.into(),
                baseline_y: l.baseline_y,
                runs,
            });
        }
    }

    FfiEditorPageSnapshot {
        page_index,
        width_pt: page.size.width,
        height_pt: page.size.height,
        lines,
    }
}

fn make_snapshot(session: &FlowEditorSession) -> FfiEditorSessionSnapshot {
    let (cursor_page, cursor_rect) = match session.state.cursor_rect() {
        Some((page_idx, rect)) => (page_idx as u32, Some(rect.into())),
        None => (0, None),
    };

    let selection_rects = session
        .state
        .selection_rects()
        .into_iter()
        .map(|(p, r)| FfiEditorSelectionRect {
            page_index: p as u32,
            rect: r.into(),
        })
        .collect();

    let pages = session
        .state
        .layout
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| build_page_snapshot(i as u32, p))
        .collect();

    let plain_text = session
        .state
        .layout
        .pages
        .iter()
        .flat_map(|p| p.all_boxes())
        .map(|b| b.plain_text())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    FfiEditorSessionSnapshot {
        editor_id: session.id.clone(),
        title: session.title.clone(),
        file_path: session.file_path.clone(),
        total_pages: session.state.layout.total_pages as u32,
        active_block_id: session.state.active_block_id.clone(),
        cursor_offset: session.state.cursor_offset as u32,
        cursor_page_index: cursor_page,
        cursor_rect,
        selection_rects,
        can_undo: session.state.can_undo(),
        can_redo: session.state.can_redo(),
        plain_text,
        pages,
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
static EDITOR_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Creates a new in-memory `.pdoc` flow editor session.
#[uniffi::export]
pub fn document_core_create_editor_session(
    title: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    let state = FlowEditorState::new_empty(&title);
    let id = format!(
        "editor_sess_{}",
        EDITOR_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let session = FlowEditorSession {
        id: id.clone(),
        title: title.clone(),
        file_path: None,
        fingerprint: None,
        state,
    };
    let snap = make_snapshot(&session);
    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    reg.insert(id, session);
    Ok(snap)
}

/// Opens an existing `.pdoc` file into the native platform flow editor.
#[uniffi::export]
pub fn document_core_open_pdoc_editor_session(
    file_path: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    let limits = PackageLimitsConfig::default();
    let pkg =
        PdocContainer::open(&file_path, &limits).map_err(|e| DocumentCoreError::InvalidPdf {
            details: format!("Failed to open .pdoc: {e}"),
        })?;
    let fingerprint = PdocFileFingerprint::compute(&file_path).ok();
    let title = pkg.document.metadata.title.clone();
    let state = FlowEditorState::from_document(pkg.into_flow_document());
    let id = format!(
        "editor_sess_{}",
        EDITOR_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let session = FlowEditorSession {
        id: id.clone(),
        title,
        file_path: Some(file_path),
        fingerprint,
        state,
    };
    let snap = make_snapshot(&session);
    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    reg.insert(id, session);
    Ok(snap)
}

/// Closes an active editor session and releases its resources.
#[uniffi::export]
pub fn document_core_close_editor_session(editor_id: String) -> Result<(), DocumentCoreError> {
    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    reg.remove(&editor_id)
        .ok_or(DocumentCoreError::SessionNotFound {
            session_id: editor_id,
        })?;
    Ok(())
}

fn with_session<F, R>(editor_id: &str, f: F) -> Result<R, DocumentCoreError>
where
    F: FnOnce(&mut FlowEditorSession) -> Result<R, DocumentCoreError>,
{
    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    let session = reg
        .get_mut(editor_id)
        .ok_or_else(|| DocumentCoreError::SessionNotFound {
            session_id: editor_id.to_string(),
        })?;
    f(session)
}

/// Inserts text at the current cursor position with sub-16ms incremental layout reflow.
#[uniffi::export]
pub fn document_core_editor_insert_text(
    editor_id: String,
    text: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_text(&text);
        Ok(make_snapshot(s))
    })
}

/// Splits the current block with a paragraph break (Enter key).
#[uniffi::export]
pub fn document_core_editor_insert_paragraph_break(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_paragraph_break();
        Ok(make_snapshot(s))
    })
}

/// Deletes the character or selection immediately preceding the cursor (Backspace).
#[uniffi::export]
pub fn document_core_editor_delete_backward(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.delete_backward();
        Ok(make_snapshot(s))
    })
}

/// Deletes the character or selection immediately following the cursor (Forward Delete).
#[uniffi::export]
pub fn document_core_editor_delete_forward(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.delete_forward();
        Ok(make_snapshot(s))
    })
}

/// Explicitly positions the cursor within a block.
#[uniffi::export]
pub fn document_core_editor_set_cursor(
    editor_id: String,
    block_id: String,
    offset: u32,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.set_cursor(&block_id, offset as usize);
        Ok(make_snapshot(s))
    })
}

/// Sets an active text selection range across blocks.
#[uniffi::export]
pub fn document_core_editor_set_selection(
    editor_id: String,
    anchor_block_id: String,
    anchor_offset: u32,
    focus_block_id: String,
    focus_offset: u32,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.set_selection(
            &anchor_block_id,
            anchor_offset as usize,
            &focus_block_id,
            focus_offset as usize,
        );
        Ok(make_snapshot(s))
    })
}

/// Clears the active selection, collapsing to cursor position.
#[uniffi::export]
pub fn document_core_editor_clear_selection(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.clear_selection();
        Ok(make_snapshot(s))
    })
}

/// Hit-tests a coordinate click/touch on a page and sets cursor position.
#[uniffi::export]
pub fn document_core_editor_hit_test(
    editor_id: String,
    page_index: u32,
    x_pt: f32,
    y_pt: f32,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state
            .hit_test_and_set_cursor(page_index as usize, x_pt, y_pt);
        Ok(make_snapshot(s))
    })
}

/// Toggles bold typography on active selection or for subsequent typing.
#[uniffi::export]
pub fn document_core_editor_toggle_bold(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.toggle_bold();
        Ok(make_snapshot(s))
    })
}

/// Toggles italic typography on active selection or for subsequent typing.
#[uniffi::export]
pub fn document_core_editor_toggle_italic(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.toggle_italic();
        Ok(make_snapshot(s))
    })
}

/// Sets the heading level (1..6) or 0 for body paragraph.
#[uniffi::export]
pub fn document_core_editor_set_heading_level(
    editor_id: String,
    level: u8,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.set_heading_level(level);
        Ok(make_snapshot(s))
    })
}

/// Inserts an unordered bullet list block at cursor.
#[uniffi::export]
pub fn document_core_editor_insert_bullet_list(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_bullet_list();
        Ok(make_snapshot(s))
    })
}

/// Inserts an ordered numbered list block at cursor.
#[uniffi::export]
pub fn document_core_editor_insert_numbered_list(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_numbered_list();
        Ok(make_snapshot(s))
    })
}

/// Inserts an editable table with specified rows and columns at cursor.
#[uniffi::export]
pub fn document_core_editor_insert_table(
    editor_id: String,
    rows: u32,
    cols: u32,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_table(rows as usize, cols as usize);
        Ok(make_snapshot(s))
    })
}

/// Inserts a legal footnote reference and definition at cursor.
#[uniffi::export]
pub fn document_core_editor_insert_footnote(
    editor_id: String,
    text: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.insert_footnote(&text);
        Ok(make_snapshot(s))
    })
}

/// Undoes the last editing transaction.
#[uniffi::export]
pub fn document_core_editor_undo(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.undo();
        Ok(make_snapshot(s))
    })
}

/// Redoes the previously undone editing transaction.
#[uniffi::export]
pub fn document_core_editor_redo(
    editor_id: String,
) -> Result<FfiEditorSessionSnapshot, DocumentCoreError> {
    with_session(&editor_id, |s| {
        s.state.redo();
        Ok(make_snapshot(s))
    })
}

/// Saves the flow document to disk as a hardened `.pdoc` package with atomic fsync and conflict detection.
#[uniffi::export]
pub fn document_core_editor_save_pdoc(
    editor_id: String,
    target_path: Option<String>,
) -> Result<String, DocumentCoreError> {
    with_session(&editor_id, |s| {
        let save_path = target_path.or_else(|| s.file_path.clone()).ok_or_else(|| {
            DocumentCoreError::SaveFailure {
                details: "No target file path specified for new document".to_string(),
            }
        })?;

        let package = PdocPackage::from_flow_document(s.state.document.clone());

        match PdocContainer::save_with_conflict_check(&package, &save_path, s.fingerprint.as_ref())
        {
            Ok(new_fp) => {
                s.file_path = Some(save_path.clone());
                s.fingerprint = Some(new_fp);
                Ok(save_path)
            }
            Err(vocument_pdoc::error::PdocError::ExternalConflictDetected { .. }) => {
                let conflict_path = PdocContainer::save_conflict_copy(&package, &save_path)
                    .map_err(|e| DocumentCoreError::SaveFailure {
                        details: format!("Conflict copy failed: {e}"),
                    })?;
                Err(DocumentCoreError::SaveFailure {
                    details: format!(
                        "External conflict detected. Your edits have been preserved in conflict file: {}",
                        conflict_path.display()
                    ),
                })
            }
            Err(e) => Err(DocumentCoreError::SaveFailure {
                details: format!("Failed to save .pdoc: {e}"),
            }),
        }
    })
}

/// Measures real-time typing and incremental layout latency in microseconds (`PERF-001`, `< 16ms budget`).
#[uniffi::export]
pub fn document_core_editor_measure_typing_latency_micros(
    editor_id: String,
    sample_text: String,
) -> Result<u64, DocumentCoreError> {
    with_session(&editor_id, |s| {
        let micros = s.state.measure_typing_latency_micros(&sample_text);
        Ok(micros)
    })
}

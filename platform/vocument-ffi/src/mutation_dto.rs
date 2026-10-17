//! UniFFI DTOs and exported functions for M0.4 First Mutations (`TransactionManager`, Highlight Annotations, Page Rotation, Undo/Redo, and Persistent Transaction Journal) (`ARCHITECTURE.md` §§6, 11–12, `ROADMAP.md` M0.4).

use crate::navigation_search_dto::FfiPdfRectMilliPt;
use crate::{DocumentCoreError, session_registry};
use vocument_core::pdf_engine::PdfHighlightAnnotation;
use vocument_core::transactions::{TransactionHistorySnapshot, TransactionRecord};

/// Standard PDF `/Highlight` annotation DTO crossing the UniFFI boundary (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiHighlightAnnotation {
    pub annotation_id: String,
    pub page_index: u32,
    pub rect: FfiPdfRectMilliPt,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub opacity_milli: u32,
    pub author: Option<String>,
    pub contents: Option<String>,
}

impl From<PdfHighlightAnnotation> for FfiHighlightAnnotation {
    fn from(annot: PdfHighlightAnnotation) -> Self {
        Self {
            annotation_id: annot.annotation_id,
            page_index: annot.page_index,
            rect: annot.rect.into(),
            color_r: annot.color_rgb[0],
            color_g: annot.color_rgb[1],
            color_b: annot.color_rgb[2],
            opacity_milli: annot.opacity_milli,
            author: annot.author,
            contents: annot.contents,
        }
    }
}

impl From<FfiHighlightAnnotation> for PdfHighlightAnnotation {
    fn from(annot: FfiHighlightAnnotation) -> Self {
        Self {
            annotation_id: annot.annotation_id,
            page_index: annot.page_index,
            rect: annot.rect.into(),
            color_rgb: [annot.color_r, annot.color_g, annot.color_b],
            opacity_milli: annot.opacity_milli,
            author: annot.author,
            contents: annot.contents,
        }
    }
}

/// Immutable transaction record DTO exposed across UniFFI (`ARCHITECTURE.md` §11, `UNDO-004`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTransactionRecord {
    pub transaction_id: String,
    pub sequence_number: u64,
    pub revision_before: u64,
    pub revision_after: u64,
    pub timestamp_unix_ms: u64,
    pub description: String,
    pub affected_pages: Vec<u32>,
    pub affected_objects: Vec<String>,
    pub before_pdf_sha256: String,
    pub after_pdf_sha256: String,
    pub record_checksum_sha256: String,
}

impl From<TransactionRecord> for FfiTransactionRecord {
    fn from(rec: TransactionRecord) -> Self {
        Self {
            transaction_id: rec.transaction_id,
            sequence_number: rec.sequence_number,
            revision_before: rec.revision_before,
            revision_after: rec.revision_after,
            timestamp_unix_ms: rec.timestamp_unix_ms,
            description: rec.description,
            affected_pages: rec.affected_pages,
            affected_objects: rec.affected_objects,
            before_pdf_sha256: rec.before_pdf_sha256,
            after_pdf_sha256: rec.after_pdf_sha256,
            record_checksum_sha256: rec.record_checksum_sha256,
        }
    }
}

/// Live Undo/Redo and transaction history state snapshot exposed across UniFFI (`UNDO-001`, `UNDO-002`, `UNDO-004`, `ROADMAP.md` M0.4).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTransactionHistorySnapshot {
    pub current_revision: u64,
    pub saved_revision: u64,
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_description: Option<String>,
    pub redo_description: Option<String>,
    pub undo_count: u32,
    pub redo_count: u32,
    pub committed_records: Vec<FfiTransactionRecord>,
    pub redo_records: Vec<FfiTransactionRecord>,
}

impl From<TransactionHistorySnapshot> for FfiTransactionHistorySnapshot {
    fn from(snap: TransactionHistorySnapshot) -> Self {
        Self {
            current_revision: snap.current_revision,
            saved_revision: snap.saved_revision,
            is_dirty: snap.is_dirty,
            can_undo: snap.can_undo,
            can_redo: snap.can_redo,
            undo_description: snap.undo_description,
            redo_description: snap.redo_description,
            undo_count: snap.undo_count,
            redo_count: snap.redo_count,
            committed_records: snap.committed_records.into_iter().map(Into::into).collect(),
            redo_records: snap.redo_records.into_iter().map(Into::into).collect(),
        }
    }
}

/// Queries the current rotation angle (`0`, `90`, `180`, `270`) of `page_index` (`ROTATE-001`, `ROADMAP.md` M0.4).
#[uniffi::export]
pub fn document_core_page_rotation_degrees(
    session_id: String,
    page_index: u32,
) -> Result<u32, DocumentCoreError> {
    let degrees = session_registry().page_rotation_degrees(&session_id, page_index)?;
    Ok(degrees)
}

/// Lists all standard PDF `/Highlight` annotations on `page_index` (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4).
#[uniffi::export]
pub fn document_core_page_highlight_annotations(
    session_id: String,
    page_index: u32,
) -> Result<Vec<FfiHighlightAnnotation>, DocumentCoreError> {
    let annots = session_registry().page_highlight_annotations(&session_id, page_index)?;
    Ok(annots.into_iter().map(Into::into).collect())
}

/// Rotates one or more pages by `delta_degrees` (`+90`, `-90`, `180`) via `TransactionManager` (`ROTATE-001`, `PAGE-001`, `ROADMAP.md` M0.4 Gate).
#[uniffi::export]
pub fn document_core_rotate_pages(
    session_id: String,
    page_indices: Vec<u32>,
    delta_degrees: i32,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ = session_registry().rotate_pages(&session_id, page_indices, delta_degrees)?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Adds a standard PDF `/Highlight` annotation via `TransactionManager` (`ANNOT-001`, `INTEROP-002`, `ROADMAP.md` M0.4 Gate).
#[uniffi::export]
pub fn document_core_add_highlight_annotation(
    session_id: String,
    annotation: FfiHighlightAnnotation,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ = session_registry().add_highlight_annotation(&session_id, annotation.into())?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Removes a standard PDF `/Highlight` annotation by `annotation_id` via `TransactionManager` (`ANNOT-001`, `ROADMAP.md` M0.4 Gate).
#[uniffi::export]
pub fn document_core_remove_highlight_annotation(
    session_id: String,
    page_index: u32,
    annotation_id: String,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ =
        session_registry().remove_highlight_annotation(&session_id, page_index, annotation_id)?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Undoes the last committed transaction on `session_id` via `TransactionManager` (`UNDO-001`, `UNDO-004`).
#[uniffi::export]
pub fn document_core_undo(
    session_id: String,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ = session_registry().undo(&session_id)?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Redoes the last undone transaction on `session_id` via `TransactionManager` (`UNDO-001`, `UNDO-004`).
#[uniffi::export]
pub fn document_core_redo(
    session_id: String,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ = session_registry().redo(&session_id)?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Returns the current `FfiTransactionHistorySnapshot` for `session_id` (`UNDO-001`, `UNDO-002`).
#[uniffi::export]
pub fn document_core_transaction_history(
    session_id: String,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Exports the verified `PersistentTransactionJournal` as pretty-printed JSON (`ARCHITECTURE.md` §§11–12, `ROADMAP.md` M0.4).
#[uniffi::export]
pub fn document_core_export_transaction_journal_json(
    session_id: String,
) -> Result<String, DocumentCoreError> {
    let json = session_registry().export_persistent_transaction_journal_json(&session_id)?;
    Ok(json)
}

/// Restores and replays a `PersistentTransactionJournal` JSON onto a freshly opened session (`ARCHITECTURE.md` §§11–12, `UNDO-001`).
#[uniffi::export]
pub fn document_core_restore_transaction_journal_json(
    session_id: String,
    journal_json: String,
) -> Result<FfiTransactionHistorySnapshot, DocumentCoreError> {
    let snap = session_registry()
        .restore_persistent_transaction_journal_json(&session_id, &journal_json)?;
    Ok(snap.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_core_close_session, document_core_get_session_snapshot,
        document_core_open_macos_scoped_file, document_core_page_geometry,
        document_core_select_page_text,
    };
    use std::path::PathBuf;

    fn m0_3_corpus_pdf_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_3_navigation_and_search.pdf")
    }

    #[test]
    fn ffi_m0_4_rotates_pages_highlights_text_undoes_redoes_and_replays_journal() {
        let pdf_path = m0_3_corpus_pdf_path();
        let snap = document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
            .expect("open M0.3 PDF fixture");

        let initial_hist =
            document_core_transaction_history(snap.session_id.clone()).expect("initial history");
        assert!(!initial_hist.is_dirty);
        assert!(!initial_hist.can_undo);
        assert_eq!(initial_hist.current_revision, 1);

        // 1. Select text on page 0 and add a standard PDF /Highlight annotation:
        let sel = document_core_select_page_text(snap.session_id.clone(), 0, None)
            .expect("select page 0 text");
        let first_rect = sel
            .highlight_rects
            .first()
            .copied()
            .expect("highlight rect");

        let hist_1 = document_core_add_highlight_annotation(
            snap.session_id.clone(),
            FfiHighlightAnnotation {
                annotation_id: "hl_ffi_001".to_owned(),
                page_index: 0,
                rect: first_rect,
                color_r: 255,
                color_g: 235,
                color_b: 59,
                opacity_milli: 450,
                author: Some("Vocument FFI".to_owned()),
                contents: Some("M0.4 FFI highlight".to_owned()),
            },
        )
        .expect("add highlight annotation");
        assert!(hist_1.is_dirty);
        assert!(hist_1.can_undo);
        assert_eq!(
            hist_1.undo_description.as_deref(),
            Some("Add highlight annotation on page #1")
        );

        let annots = document_core_page_highlight_annotations(snap.session_id.clone(), 0)
            .expect("query annotations");
        assert_eq!(annots.len(), 1);
        assert_eq!(annots[0].annotation_id, "hl_ffi_001");

        // 2. Rotate page 0 by +90° and verify geometry swap:
        let hist_2 = document_core_rotate_pages(snap.session_id.clone(), vec![0], 90)
            .expect("rotate page 0 by +90");
        assert_eq!(hist_2.undo_count, 2);
        assert_eq!(
            document_core_page_rotation_degrees(snap.session_id.clone(), 0).expect("rot"),
            90
        );
        let geom_rot = document_core_page_geometry(snap.session_id.clone(), 0).expect("geom");
        assert_eq!(geom_rot.width_milli_pt, 842_000);
        assert_eq!(geom_rot.height_milli_pt, 595_000);

        // 3. Undo rotation, then redo rotation:
        let hist_undo = document_core_undo(snap.session_id.clone()).expect("undo rotation");
        assert!(hist_undo.can_redo);
        assert_eq!(
            document_core_page_rotation_degrees(snap.session_id.clone(), 0)
                .expect("rot after undo"),
            0
        );
        let hist_redo = document_core_redo(snap.session_id.clone()).expect("redo rotation");
        assert!(!hist_redo.can_redo);
        assert_eq!(hist_redo.undo_count, 2);
        assert_eq!(
            document_core_page_rotation_degrees(snap.session_id.clone(), 0)
                .expect("rot after redo"),
            90
        );

        // 4. Export persistent transaction journal JSON and replay onto a fresh session:
        let journal_json = document_core_export_transaction_journal_json(snap.session_id.clone())
            .expect("export transaction journal JSON");
        assert!(journal_json.contains("\"schema_version\": 1"));

        let fresh = document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
            .expect("open fresh session");
        let replayed =
            document_core_restore_transaction_journal_json(fresh.session_id.clone(), journal_json)
                .expect("restore and replay journal");
        assert_eq!(replayed.undo_count, 2);
        assert_eq!(
            document_core_page_rotation_degrees(fresh.session_id.clone(), 0).expect("replayed rot"),
            90
        );
        let replayed_annots = document_core_page_highlight_annotations(fresh.session_id.clone(), 0)
            .expect("replayed annots");
        assert_eq!(replayed_annots.len(), 1);
        assert_eq!(replayed_annots[0].annotation_id, "hl_ffi_001");

        let refreshed_snap = document_core_get_session_snapshot(fresh.session_id.clone())
            .expect("refreshed session snapshot");
        assert!(refreshed_snap.is_dirty);

        document_core_close_session(snap.session_id).expect("close first session");
        document_core_close_session(fresh.session_id).expect("close fresh session");
    }
}

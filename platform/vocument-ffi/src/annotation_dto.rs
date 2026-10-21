//! UniFFI DTOs and exported functions for the comment of the reviewer and
//! the rail of the profession (`ANNOT-001`).
//!
//! A comment is a standard PDF `/Text` annotation with a popup: the note
//! stands on the page, the text of the note opens in the popup. The colour
//! of the note says what the note means — the rail of the profession — and
//! the rail lives in the core, not in the interface.
//!
//! The shell therefore sends the **key** of the rail (`"objection"`, …) and
//! the core looks up the colour. A shell that sends a colour of its own is
//! refused: the rail is the thing that makes the notes of a document
//! readable.

use crate::navigation_search_dto::FfiPdfRectMilliPt;
use crate::{DocumentCoreError, session_registry};
use vocument_core::annotations::{JuristPalette, JuristPaletteEntry};
use vocument_core::pdf_engine::PdfCommentAnnotation;

/// One entry of the rail of the profession (`ANNOT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiJuristPaletteEntry {
    /// The stable key of the entry (`"note"`, `"objection"`, …).
    pub key: String,
    /// The name the interface shows.
    pub label: String,
    /// What the colour means in the review.
    pub meaning: String,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
}

impl From<&JuristPaletteEntry> for FfiJuristPaletteEntry {
    fn from(entry: &JuristPaletteEntry) -> Self {
        Self {
            key: entry.key.clone(),
            label: entry.label.clone(),
            meaning: entry.meaning.clone(),
            color_r: entry.color_rgb[0],
            color_g: entry.color_rgb[1],
            color_b: entry.color_rgb[2],
        }
    }
}

impl From<FfiJuristPaletteEntry> for JuristPaletteEntry {
    fn from(entry: FfiJuristPaletteEntry) -> Self {
        JuristPaletteEntry::new(
            entry.key,
            entry.label,
            entry.meaning,
            [entry.color_r, entry.color_g, entry.color_b],
        )
    }
}

/// The comment of the reviewer, as it travels over the FFI boundary (`ANNOT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiCommentAnnotation {
    /// The stable identifier of the note (the PDF `/NM` key).
    pub annotation_id: String,
    /// The `0`-based page the note stands on.
    pub page_index: u32,
    /// The box of the note on the page.
    pub rect: FfiPdfRectMilliPt,
    /// The key of the rail the note was written with.
    pub palette_key: String,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    /// The text of the note.
    pub contents: String,
    /// The author of the note (the PDF `/T` key).
    pub author: Option<String>,
}

impl From<PdfCommentAnnotation> for FfiCommentAnnotation {
    fn from(annot: PdfCommentAnnotation) -> Self {
        Self {
            annotation_id: annot.annotation_id,
            page_index: annot.page_index,
            rect: annot.rect.into(),
            palette_key: annot.palette_key,
            color_r: annot.color_rgb[0],
            color_g: annot.color_rgb[1],
            color_b: annot.color_rgb[2],
            contents: annot.contents,
            author: annot.author,
        }
    }
}

impl From<FfiCommentAnnotation> for PdfCommentAnnotation {
    fn from(annot: FfiCommentAnnotation) -> Self {
        Self {
            annotation_id: annot.annotation_id,
            page_index: annot.page_index,
            rect: annot.rect.into(),
            palette_key: annot.palette_key,
            color_rgb: [annot.color_r, annot.color_g, annot.color_b],
            contents: annot.contents,
            author: annot.author,
        }
    }
}

/// The rail of the profession: the palette the comments are written with
/// (`ANNOT-001`).
#[uniffi::export]
pub fn document_core_jurist_palette() -> Vec<FfiJuristPaletteEntry> {
    session_registry()
        .jurist_palette()
        .entries()
        .iter()
        .map(Into::into)
        .collect()
}

/// The standard rail of the profession (`ANNOT-001`).
///
/// The shell offers it to the user as the way back: the rail of the user may
/// be replaced, and the standard rail is the one every document was written
/// with.
#[uniffi::export]
pub fn document_core_default_jurist_palette() -> Vec<FfiJuristPaletteEntry> {
    JuristPalette::default_palette()
        .entries()
        .iter()
        .map(Into::into)
        .collect()
}

/// Replaces the rail of the profession with the palette of the user
/// (`ANNOT-001`).
///
/// The palette is checked before it is taken: a rail without a colour, a
/// name or a unique key is refused, so that every note can be written.
#[uniffi::export]
pub fn document_core_set_jurist_palette(
    entries: Vec<FfiJuristPaletteEntry>,
) -> Result<(), DocumentCoreError> {
    let palette =
        JuristPalette::new(entries.into_iter().map(Into::into).collect()).map_err(|error| {
            DocumentCoreError::CommentRefused {
                details: error.to_string(),
            }
        })?;
    session_registry()
        .set_jurist_palette(palette)
        .map_err(|error| DocumentCoreError::CommentRefused {
            details: error.to_string(),
        })
}

/// Lists the comments of the reviewer on `page_index` (`ANNOT-001`).
#[uniffi::export]
pub fn document_core_page_comment_annotations(
    session_id: String,
    page_index: u32,
) -> Result<Vec<FfiCommentAnnotation>, DocumentCoreError> {
    let annots = session_registry().page_comment_annotations(&session_id, page_index)?;
    Ok(annots.into_iter().map(Into::into).collect())
}

/// Writes a comment of the reviewer via `TransactionManager` (`ANNOT-001`).
///
/// The core looks up the colour of the key in the rail of the profession and
/// refuses a note without a text or without an area. The document stays
/// untouched when the note is refused.
#[uniffi::export]
pub fn document_core_add_comment_annotation(
    session_id: String,
    annotation: FfiCommentAnnotation,
) -> Result<crate::mutation_dto::FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ = session_registry().add_comment_annotation(&session_id, annotation.into())?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

/// Removes a comment of the reviewer by `annotation_id` via `TransactionManager` (`ANNOT-001`).
#[uniffi::export]
pub fn document_core_remove_comment_annotation(
    session_id: String,
    page_index: u32,
    annotation_id: String,
) -> Result<crate::mutation_dto::FfiTransactionHistorySnapshot, DocumentCoreError> {
    let _ =
        session_registry().remove_comment_annotation(&session_id, page_index, &annotation_id)?;
    let snap = session_registry().transaction_history(&session_id)?;
    Ok(snap.into())
}

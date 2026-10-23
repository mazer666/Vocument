//! Thin UniFFI transport boundary (`DocumentCore` API) between `vocument-core` and native UI (`ARCHITECTURE.md` §§5–12, 17, `ROADMAP.md` M0.0–M0.4).
//!
//! Exposes only opaque session/document IDs and immutable DTO snapshots.
//! Never exposes raw MuPDF pointers or internal Rust references.

pub mod annotation_dto;
pub mod build_info_dto;
pub mod inspection_dto;
pub mod interop_dto;
pub mod job_dto;
pub mod mutation_dto;
pub mod navigation_search_dto;
pub mod ocr_dto;
pub mod power_dto;
pub mod recovery_dto;
pub mod redaction_dto;
pub mod render_dto;
pub mod rendered_page_reader;
pub mod sanitize_dto;
pub mod save_dto;
pub mod scan_dto;
pub mod session_dto;
pub mod workbench_dto;

use std::sync::OnceLock;
use thiserror::Error;
use vocument_core::bootstrap_core_snapshot;
use vocument_core::file_broker::{FileBrokerError, ScopedLocalFileBroker};
use vocument_core::pdf_engine::PdfEngineError;
use vocument_core::session::{SessionError, SessionRegistry};
use vocument_pdf_mupdf::MuPdfEngineAdapter;

pub use inspection_dto::{
    FfiDocumentInspection, FfiPdfActionInfo, FfiPdfActionKind, document_core_inspect_document,
};
pub use interop_dto::{FfiInteropReport, document_core_run_interop_gate_suite};
pub use mutation_dto::{
    FfiHighlightAnnotation, FfiTransactionHistorySnapshot, FfiTransactionRecord,
    document_core_add_highlight_annotation, document_core_export_transaction_journal_json,
    document_core_page_highlight_annotations, document_core_page_rotation_degrees,
    document_core_redo, document_core_remove_highlight_annotation,
    document_core_restore_transaction_journal_json, document_core_rotate_pages,
    document_core_transaction_history, document_core_undo,
};
pub use navigation_search_dto::FfiPageWord;
pub use navigation_search_dto::{
    FfiPageDescriptor, FfiPageLabelRange, FfiPageLabelStyle, FfiPdfRectMilliPt,
    FfiRenderedThumbnail, FfiSearchHit, FfiSearchIndexProgress, FfiSearchResponse,
    FfiTextSelectionResult, FfiThumbnailCacheStats,
};
pub use ocr_dto::{
    FfiBoundingBox, FfiOcrCharacter, FfiOcrLine, FfiOcrPageResult, FfiOcrWord,
    document_core_detect_text_language, document_core_ocr_clean_scan_frame,
    document_core_ocr_frame, document_core_ocr_scan_session_page,
};
pub use recovery_dto::{
    FfiRecoverableDocumentInfo, FfiRecoveryAction, FfiRecoveryActionResult,
    document_core_check_document_recovery, document_core_execute_recovery_action,
    document_core_mark_recovery_clean_shutdown, document_core_scan_recoverable_documents,
};
pub use redaction_dto::{
    FfiRedactionRegion, FfiRedactionReport, document_core_redact_page_regions,
};
pub use render_dto::{
    FfiMemoryPressureLevel, FfiPdfPageGeometry, FfiRenderedTile, FfiTileCacheStats,
    FfiTileRenderRequest, FfiViewportRenderResult,
};
pub use sanitize_dto::{FfiPdfSanitizeOptions, document_core_sanitize_document};
pub use save_dto::{
    FfiExternalChangeCheckResult, FfiSaveConflict, FfiSaveConflictResolution, FfiSaveOutcome,
    FfiSaveSuccessSnapshot, document_core_check_external_changes,
    document_core_generate_collision_free_path, document_core_resolve_save_conflict,
    document_core_save_session, document_core_save_session_as,
    document_core_save_session_collision_free_copy,
};
pub use scan_dto::{
    FfiCapturedScanPage, FfiDeskewResult, FfiPoint2D, FfiQuadCorners, FfiScanCleanupProfile,
    FfiScanFrame, FfiScanPixelFormat, FfiScanSessionSnapshot, document_core_append_scan_page,
    document_core_auto_orient_frame, document_core_close_scan_session, document_core_deskew_frame,
    document_core_detect_document_edges, document_core_get_scan_session,
    document_core_get_warped_scan_page, document_core_rotate_frame,
    document_core_start_scan_session, document_core_warp_perspective,
};
pub use session_dto::{
    DocumentCoreHandshake, FfiBrokeredFileDescriptor, FfiDocumentSessionSnapshot,
    FfiFileAccessOrigin, FfiFileIdentity, FfiPdfDocumentSummary,
};

uniffi::setup_scaffolding!("vocument_ffi");

static GLOBAL_SESSION_REGISTRY: OnceLock<SessionRegistry> = OnceLock::new();
static RECOVERY_STORE: OnceLock<std::sync::Arc<vocument_core::recovery::RecoveryStore>> =
    OnceLock::new();
static DATA_DIRECTORY: OnceLock<std::path::PathBuf> = OnceLock::new();

/// The directory in which Vocument keeps the data of the user.
///
/// The platform hands the application its own directory — the application
/// support directory of macOS, the files directory of Android — before it
/// opens the first document. The rail of the profession and the recovery
/// store live there.
pub(crate) fn data_directory() -> std::path::PathBuf {
    DATA_DIRECTORY
        .get()
        .cloned()
        .unwrap_or_else(vocument_core::config::default_data_directory)
}

/// Answers whether the registry was already used.
pub(crate) fn session_registry_is_initialised() -> bool {
    GLOBAL_SESSION_REGISTRY.get().is_some()
}

/// Remembers the directory of the data of the user.
///
/// The directory has to be handed over before the first document is opened:
/// the recovery store is created in it, and a store that is created in the
/// wrong directory would keep the document of the user in a place the
/// platform did not choose. The function therefore refuses a second call.
pub(crate) fn set_data_directory(directory: std::path::PathBuf) -> bool {
    DATA_DIRECTORY.set(directory).is_ok()
}

pub(crate) fn session_registry() -> &'static SessionRegistry {
    GLOBAL_SESSION_REGISTRY.get_or_init(|| {
        let store = recovery_store();
        SessionRegistry::with_broker_and_recovery(
            Box::new(MuPdfEngineAdapter::new()),
            std::sync::Arc::new(vocument_core::file_broker::ScopedLocalFileBroker::new()),
            store.clone(),
        )
        .with_rendered_page_reader(rendered_page_reader::default_rendered_page_reader())
    })
}

/// The store that holds the recovery data of the user.
///
/// The store holds the document of the user in plain text. The directory has
/// to belong to the user alone: a directory that a stranger can read or
/// replace is no place for it. When the directory cannot be made private, the
/// store refuses every write instead of handing the document of the user to
/// somebody else — and the reason is kept, so that the application can tell
/// the user that the crash recovery is not available in this run.
///
/// The fourth round of the red team found the error of the private root
/// being printed to the standard error and then forgotten: the run continued
/// as if the recovery were protected. The reason is now part of the answer
/// of the library.
fn recovery_store() -> &'static std::sync::Arc<vocument_core::recovery::RecoveryStore> {
    RECOVERY_STORE.get_or_init(|| {
        let root = data_directory().join(vocument_core::recovery::RECOVERY_DIR_NAME);
        match vocument_core::recovery::RecoveryContainer::create_private_root(&root) {
            Ok(()) => std::sync::Arc::new(vocument_core::recovery::RecoveryStore::new(root)),
            Err(error) => {
                let details = format!(
                    "the recovery directory {} could not be made private: {error}. \
                     The crash recovery of the documents is not available in this run.",
                    root.display()
                );
                std::sync::Arc::new(vocument_core::recovery::RecoveryStore::disabled(
                    root, details,
                ))
            }
        }
    })
}

/// Reports whether the crash recovery of the documents is available.
///
/// The application asks at the start and shows the reason as a warning when
/// the recovery is not available: the user has to know that the documents
/// are not protected against a crash in this run.
pub(crate) fn recovery_status() -> recovery_dto::FfiRecoveryStatus {
    let store = recovery_store();
    recovery_dto::FfiRecoveryStatus {
        available: !store.is_disabled(),
        details: store.disabled_reason().unwrap_or_default().to_owned(),
    }
}

/// Error type crossing the UniFFI boundary from `DocumentCore`.
#[derive(Debug, Error, uniffi::Error, PartialEq, Eq)]
pub enum DocumentCoreError {
    #[error("configuration error: {details}")]
    ConfigurationError { details: String },
    #[error("file broker error: {details}")]
    FileBrokerFailure { details: String },
    #[error("PDF requires an open password")]
    PasswordRequired,
    #[error("invalid or corrupted PDF document: {details}")]
    InvalidPdf { details: String },
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("the document is not stored locally and cannot be saved in place: {details}")]
    DocumentNotLocallyWritable { details: String },
    /// The redaction could not be carried out (`REDACT-001`).
    #[error("redaction failed: {details}")]
    RedactionFailure { details: String },
    /// The redaction was carried out, but it could not be verified in every
    /// respect (`REDACT-002`). The document must not be presented as
    /// redacted.
    #[error("the redaction could not be verified: {details}")]
    RedactionUnverified { details: String },
    /// The redaction failed and the document could **not** be put back
    /// (`REDACT-002`).
    ///
    /// The session holds a document in an unknown state. The shell has to
    /// tell the user that the document has to be opened again — the file on
    /// the disk still holds the original.
    #[error(
        "the redaction failed and the document could not be put back; the document has to be opened again. The error was: {original}. The rollback failed with: {rollback}"
    )]
    RedactionRollbackFailed { original: String, rollback: String },
    /// A comment of the reviewer was refused (`ANNOT-001`).
    ///
    /// The note has no text, no area, or a key that the rail of the
    /// profession does not carry. The document is untouched.
    #[error("the comment was refused: {details}")]
    CommentRefused { details: String },
    #[error("navigation error: {details}")]
    NavigationFailure { details: String },
    #[error("transaction error: {details}")]
    TransactionFailure { details: String },
    #[error("save error: {details}")]
    SaveFailure { details: String },
    #[error("recovery error: {details}")]
    RecoveryFailure { details: String },
    #[error("imaging error: {details}")]
    ImagingFailure { details: String },
    #[error("internal core failure: {details}")]
    InternalFailure { details: String },
}

impl From<SessionError> for DocumentCoreError {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::FileBroker(fb) => Self::FileBrokerFailure {
                details: fb.to_string(),
            },
            SessionError::PdfEngine(PdfEngineError::PasswordRequired) => Self::PasswordRequired,
            SessionError::PdfEngine(PdfEngineError::InvalidPdf { details }) => {
                Self::InvalidPdf { details }
            }
            SessionError::PdfEngine(other) => Self::InternalFailure {
                details: other.to_string(),
            },
            SessionError::RenderPipeline(rp) => Self::InternalFailure {
                details: rp.to_string(),
            },
            SessionError::Navigation(nav) => Self::NavigationFailure {
                details: nav.to_string(),
            },
            SessionError::SearchIndex(si) => Self::InternalFailure {
                details: si.to_string(),
            },
            SessionError::Transaction(tx) => Self::TransactionFailure {
                details: tx.to_string(),
            },
            SessionError::Save(save_err) => Self::SaveFailure {
                details: save_err.to_string(),
            },
            SessionError::Recovery(rec_err) => Self::RecoveryFailure {
                details: rec_err.to_string(),
            },
            SessionError::CommentInvalid { details } => Self::CommentRefused { details },
            SessionError::SessionNotFound { session_id } => Self::SessionNotFound { session_id },
            SessionError::NotLocallyWritable { session_id } => Self::DocumentNotLocallyWritable {
                details: format!(
                    "session {session_id} lives in a platform document provider; export it to a location the user owns"
                ),
            },
            SessionError::RedactionRollbackFailed { original, rollback } => {
                // The bindings show the fields, not the message of the Rust
                // error. The instruction has to travel in the fields: without
                // it the user sees two error strings and does not learn that
                // the document has to be opened again.
                Self::RedactionRollbackFailed {
                    original: format!(
                        "the redaction failed and the document could not be put back; \
                         the document has to be opened again. The error was: {original}"
                    ),
                    rollback: format!("The rollback failed with: {rollback}"),
                }
            }
            SessionError::Redaction(redaction_err) => match redaction_err {
                vocument_core::redaction::RedactionError::VerificationFailed { details } => {
                    // The text of the error travels in the field: the
                    // generated bindings show the field, not the message of
                    // the Rust error. The meaning has to be in it.
                    Self::RedactionUnverified {
                        details: format!("the redaction could not be verified: {details}"),
                    }
                }
                other => Self::RedactionFailure {
                    details: format!("the redaction failed: {other}"),
                },
            },
            SessionError::Workbench(err) => Self::InternalFailure {
                details: format!("workbench operation failed: {err}"),
            },
            SessionError::RegistryPoisoned => Self::InternalFailure {
                details: "session registry lock poisoned".to_owned(),
            },
        }
    }
}

impl From<vocument_imaging::ImagingError> for DocumentCoreError {
    fn from(err: vocument_imaging::ImagingError) -> Self {
        Self::ImagingFailure {
            details: err.to_string(),
        }
    }
}

impl From<vocument_core::recovery::RecoveryError> for DocumentCoreError {
    fn from(err: vocument_core::recovery::RecoveryError) -> Self {
        Self::RecoveryFailure {
            details: err.to_string(),
        }
    }
}

impl From<FileBrokerError> for DocumentCoreError {
    fn from(err: FileBrokerError) -> Self {
        Self::FileBrokerFailure {
            details: err.to_string(),
        }
    }
}

/// Performs the M0.0 `DocumentCore` bootstrap handshake for Swift and Kotlin callers.
#[uniffi::export]
pub fn document_core_handshake() -> Result<DocumentCoreHandshake, DocumentCoreError> {
    bootstrap_core_snapshot()
        .map(DocumentCoreHandshake::from)
        .map_err(|err| DocumentCoreError::ConfigurationError {
            details: err.to_string(),
        })
}

/// Opens a PDF document from a macOS Finder / Open With security-scoped file path or URL via `ScopedLocalFileBroker` (`ROADMAP.md` M0.1).
#[uniffi::export]
pub fn document_core_open_macos_scoped_file(
    scoped_path_or_url: String,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let broker = ScopedLocalFileBroker::new();
    let snapshot = session_registry().open_via_broker(&broker, &scoped_path_or_url)?;
    Ok(snapshot.into())
}

/// Opens a PDF document from a platform `FileBroker` descriptor (e.g. Android Storage Access Framework `content://` URI) (`ROADMAP.md` M0.1).
#[uniffi::export]
pub fn document_core_open_brokered_file(
    descriptor: FfiBrokeredFileDescriptor,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let snapshot = session_registry().open_brokered_file(descriptor.into())?;
    Ok(snapshot.into())
}

/// Returns the current immutable `FfiDocumentSessionSnapshot` for an active `session_id`.
#[uniffi::export]
pub fn document_core_get_session_snapshot(
    session_id: String,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let snapshot = session_registry().session_snapshot(&session_id)?;
    Ok(snapshot.into())
}

/// Closes an active `DocumentSession` and releases its underlying `PdfEngine` resources.
///
/// Every background job of the session is cancelled first, so that a result of a
/// closed document can never be applied afterwards (`ARCHITECTURE.md` §8.4).
#[uniffi::export]
pub fn document_core_close_session(session_id: String) -> Result<(), DocumentCoreError> {
    // Cancel first: a job that is still running would otherwise hold a reference
    // to a session that is about to disappear.
    let _ = job_dto::document_core_job_cancel_session(session_id.clone());
    session_registry().close_session(&session_id)?;
    Ok(())
}

/// Queries the unscaled geometry (points and rotation) of `page_index` via `DocumentActor` (`ROADMAP.md` M0.2).
#[uniffi::export]
pub fn document_core_page_geometry(
    session_id: String,
    page_index: u32,
) -> Result<FfiPdfPageGeometry, DocumentCoreError> {
    let geometry = session_registry().page_geometry(&session_id, page_index)?;
    Ok(geometry.into())
}

/// Renders a single visible page or tile via `DocumentActor` + adaptive `TileCache` (`ROADMAP.md` M0.2).
#[uniffi::export]
pub fn document_core_render_visible_tile(
    session_id: String,
    request: FfiTileRenderRequest,
) -> Result<FfiRenderedTile, DocumentCoreError> {
    let tile = session_registry().render_visible_tile(&session_id, request.into())?;
    Ok(tile.into())
}

/// Renders the requested visible viewport tile and schedules neighbouring-page prefetch (`ROADMAP.md` M0.2).
#[uniffi::export]
pub fn document_core_render_viewport_with_prefetch(
    session_id: String,
    request: FfiTileRenderRequest,
) -> Result<FfiViewportRenderResult, DocumentCoreError> {
    let result = session_registry().render_viewport_with_prefetch(&session_id, request.into())?;
    Ok(result.into())
}

/// Applies a platform memory-pressure signal (`Normal`, `Warning`, `Critical`) to the adaptive `TileCache` and `ThumbnailCache` (`ARCHITECTURE.md` §9).
#[uniffi::export]
pub fn document_core_handle_memory_pressure(
    level: FfiMemoryPressureLevel,
) -> Result<FfiTileCacheStats, DocumentCoreError> {
    let stats = session_registry().handle_memory_pressure(level.into())?;
    Ok(stats.into())
}

/// Returns current telemetry statistics of the shared adaptive `TileCache`.
#[uniffi::export]
pub fn document_core_tile_cache_stats() -> Result<FfiTileCacheStats, DocumentCoreError> {
    let stats = session_registry().tile_cache_stats()?;
    Ok(stats.into())
}

/// Returns all physical and logical `FfiPageDescriptor` entries for `session_id` (`PAGELABEL-001`, `PAGELABEL-002`, `ROADMAP.md` M0.3).
#[uniffi::export]
pub fn document_core_page_descriptors(
    session_id: String,
) -> Result<Vec<FfiPageDescriptor>, DocumentCoreError> {
    let list = session_registry().page_descriptors(&session_id)?;
    Ok(list.into_iter().map(Into::into).collect())
}

/// Resolves a user navigation target (`query`: logical label like `"ii"` or `"A-2"`, or physical page `"2"` / `"#2"`) (`PAGELABEL-003`).
#[uniffi::export]
pub fn document_core_resolve_page_navigation(
    session_id: String,
    query: String,
) -> Result<FfiPageDescriptor, DocumentCoreError> {
    let desc = session_registry().resolve_page_navigation(&session_id, &query)?;
    Ok(desc.into())
}

/// Creates or replaces custom logical page-label ranges on `session_id` (`PAGELABEL-004`).
#[uniffi::export]
pub fn document_core_set_page_label_ranges(
    session_id: String,
    ranges: Vec<FfiPageLabelRange>,
) -> Result<Vec<FfiPageDescriptor>, DocumentCoreError> {
    let core_ranges = ranges.into_iter().map(Into::into).collect();
    let updated = session_registry().set_page_label_ranges(&session_id, core_ranges)?;
    Ok(updated.into_iter().map(Into::into).collect())
}

/// Renders a low-resolution page thumbnail fitted to `max_dimension_px` via the separate `ThumbnailCache` (`PAGEVIEW-001`, `PAGEVIEW-002`).
#[uniffi::export]
pub fn document_core_render_page_thumbnail(
    session_id: String,
    page_index: u32,
    max_dimension_px: u32,
) -> Result<FfiRenderedThumbnail, DocumentCoreError> {
    let thumb =
        session_registry().render_page_thumbnail(&session_id, page_index, max_dimension_px)?;
    Ok(thumb.into())
}

/// Returns current telemetry statistics of the separate `ThumbnailCache`.
#[uniffi::export]
pub fn document_core_thumbnail_cache_stats() -> Result<FfiThumbnailCacheStats, DocumentCoreError> {
    let stats = session_registry().thumbnail_cache_stats()?;
    Ok(stats.into())
}

/// Returns the words of `page_index` with their boxes (`REDACT-001`).
///
/// The words are what a professional review picks: the application redacts
/// exactly the boxes of the words that the user has chosen.
#[uniffi::export]
pub fn document_core_page_words(
    session_id: String,
    page_index: u32,
) -> Result<Vec<FfiPageWord>, DocumentCoreError> {
    let registry = session_registry();
    let structured = registry.page_text(&session_id, page_index)?;
    let words = structured
        .words
        .iter()
        .map(|word| FfiPageWord {
            text: word.text.clone(),
            rect: FfiPdfRectMilliPt {
                x0_milli_pt: word.bbox.x0_milli_pt,
                y0_milli_pt: word.bbox.y0_milli_pt,
                x1_milli_pt: word.bbox.x1_milli_pt,
                y1_milli_pt: word.bbox.y1_milli_pt,
            },
        })
        .collect();
    Ok(words)
}

/// Selects text on `page_index` (either full-page when `selection_rect` is `None`, or intersecting `selection_rect`),
/// returning both `raw_text` (`OCRCOPY-001`) and `cleaned_text` (`OCRCOPY-002`) plus `highlight_rects`.
#[uniffi::export]
pub fn document_core_select_page_text(
    session_id: String,
    page_index: u32,
    selection_rect: Option<FfiPdfRectMilliPt>,
) -> Result<FfiTextSelectionResult, DocumentCoreError> {
    let sel = session_registry().select_page_text(
        &session_id,
        page_index,
        selection_rect.map(Into::into),
    )?;
    Ok(sel.into())
}

/// Progressively indexes up to `max_pages` into the session's SQLite FTS5 index, starting around
/// `preferred_start_page` (`SEARCH-009`, `ARCHITECTURE.md` §17, `ROADMAP.md` M0.3 Gate).
#[uniffi::export]
pub fn document_core_index_search_batch(
    session_id: String,
    preferred_start_page: u32,
    max_pages: u32,
) -> Result<FfiSearchIndexProgress, DocumentCoreError> {
    let progress =
        session_registry().index_search_batch(&session_id, preferred_start_page, max_pages)?;
    Ok(progress.into())
}

/// Queries the current progressive SQLite FTS5 indexing progress for `session_id`.
#[uniffi::export]
pub fn document_core_search_index_progress(
    session_id: String,
) -> Result<FfiSearchIndexProgress, DocumentCoreError> {
    let progress = session_registry().search_index_progress(&session_id)?;
    Ok(progress.into())
}

/// Executes a classic exact search against all currently indexed pages in `session_id` (`SEARCH-002`, `ROADMAP.md` M0.3 Gate).
#[uniffi::export]
pub fn document_core_search_exact(
    session_id: String,
    query: String,
) -> Result<FfiSearchResponse, DocumentCoreError> {
    let res = session_registry().search_exact(&session_id, &query)?;
    Ok(res.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn sample_corpus_pdf_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_1_sample_two_pages.pdf")
    }

    fn m0_3_corpus_pdf_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_3_navigation_and_search.pdf")
    }

    #[test]
    fn ffi_handshake_returns_verified_core_snapshot() {
        let handshake = document_core_handshake().expect("handshake must succeed");
        assert_eq!(handshake.baseline_version, "0.1");
        // The milestone is derived from `project/project.yaml`.
        assert_eq!(handshake.milestone, "M3");
        assert!(handshake.strict_offline_default);
        assert!(!handshake.telemetry_enabled);
        assert_eq!(handshake.target_fps, 60);
        assert_eq!(handshake.default_german_variant, "de-AT");
        assert_eq!(handshake.built_in_ocr_languages, vec!["deu", "eng"]);
    }

    #[test]
    fn ffi_opens_real_pdf_via_macos_scoped_broker_and_android_saf_broker() {
        let pdf_path = sample_corpus_pdf_path();
        let mac_snap =
            document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
                .expect("macOS scoped file open must succeed");

        assert!(mac_snap.session_id.starts_with("sess_"));
        assert_eq!(mac_snap.document_id, "doc_1a00192b75fb403b");
        assert_eq!(mac_snap.revision, 1);
        assert!(!mac_snap.is_dirty);
        assert_eq!(mac_snap.access_origin, FfiFileAccessOrigin::MacOsScopedUrl);
        assert_eq!(
            mac_snap.file_identity.display_name,
            "m0_1_sample_two_pages.pdf"
        );
        assert_eq!(mac_snap.file_identity.size_bytes, 697);
        assert_eq!(mac_snap.pdf_summary.page_count, 2);

        let bytes = std::fs::read(&pdf_path).expect("read fixture bytes");
        let saf_snap = document_core_open_brokered_file(FfiBrokeredFileDescriptor {
            source_token: "content://com.android.providers.downloads.documents/document/42"
                .to_owned(),
            display_name: "m0_1_sample_two_pages.pdf".to_owned(),
            access_origin: FfiFileAccessOrigin::AndroidSafUri,
            os_file_id: Some("saf:42".to_owned()),
            modified_unix_ms: Some(1_760_000_000_000),
            bytes,
        })
        .expect("Android SAF broker open must succeed");

        assert_eq!(saf_snap.document_id, mac_snap.document_id);
        document_core_close_session(mac_snap.session_id).expect("close mac session");
        document_core_close_session(saf_snap.session_id).expect("close saf session");
    }

    #[test]
    fn ffi_renders_viewport_with_prefetch_zoom_cache_and_memory_pressure() {
        let _ = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Critical)
            .expect("reset cache");
        let _ = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Normal)
            .expect("restore normal cache budget");

        let pdf_path = sample_corpus_pdf_path();
        let start = Instant::now();
        let snap = document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
            .expect("open PDF session");

        let viewport = document_core_render_viewport_with_prefetch(
            snap.session_id.clone(),
            FfiTileRenderRequest {
                page_index: 0,
                scale_milli: 1000,
                tile_x_px: 0,
                tile_y_px: 0,
                tile_width_px: 595,
                tile_height_px: 842,
            },
        )
        .expect("render page 0 with prefetch");
        let first_page_elapsed_ms = start.elapsed().as_millis();
        assert!(first_page_elapsed_ms < 300);
        assert_eq!(viewport.prefetched_pages, vec![1]);

        let purged_stats = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Critical)
            .expect("critical memory pressure");
        assert_eq!(purged_stats.cached_tile_count, 0);
        let _ = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Normal);

        document_core_close_session(snap.session_id).expect("close session");
    }

    #[test]
    fn ffi_m0_3_thumbnails_page_labels_text_selection_and_progressive_fts5_search() {
        let _ = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Critical)
            .expect("reset caches");
        let _ = document_core_handle_memory_pressure(FfiMemoryPressureLevel::Normal)
            .expect("restore caches");

        let pdf_path = m0_3_corpus_pdf_path();
        let snap = document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
            .expect("open M0.3 PDF fixture");
        assert_eq!(snap.pdf_summary.page_count, 4);

        // 1. Verify embedded logical page labels (PAGELABEL-001, PAGELABEL-002):
        let descriptors =
            document_core_page_descriptors(snap.session_id.clone()).expect("page descriptors");
        let labels: Vec<&str> = descriptors
            .iter()
            .map(|d| d.logical_label.as_str())
            .collect();
        assert_eq!(labels, vec!["i", "ii", "1", "2"]);

        // 2. Navigate by logical label "ii" (page_index 1) and "1" (page_index 2) (PAGELABEL-003):
        let nav_ii = document_core_resolve_page_navigation(snap.session_id.clone(), "ii".into())
            .expect("resolve ii");
        assert_eq!(nav_ii.page_index, 1);
        assert_eq!(nav_ii.physical_page_number, 2);

        let nav_1 = document_core_resolve_page_navigation(snap.session_id.clone(), "1".into())
            .expect("resolve 1");
        assert_eq!(nav_1.page_index, 2);
        assert_eq!(nav_1.physical_page_number, 3);

        // 3. Render page thumbnail and verify separate ThumbnailCache hit (PAGEVIEW-001, PAGEVIEW-002):
        let thumb_0 = document_core_render_page_thumbnail(snap.session_id.clone(), 0, 160)
            .expect("render thumbnail 0");
        assert_eq!(thumb_0.logical_label, "i");
        assert_eq!(thumb_0.height_px, 160);
        assert!(thumb_0.width_px > 100 && thumb_0.width_px < 160);

        let thumb_stats_before = document_core_thumbnail_cache_stats().expect("thumb stats");
        let _ = document_core_render_page_thumbnail(snap.session_id.clone(), 0, 160)
            .expect("cached thumbnail 0");
        let thumb_stats_after = document_core_thumbnail_cache_stats().expect("thumb stats");
        assert_eq!(
            thumb_stats_after.hit_count,
            thumb_stats_before.hit_count + 1
        );

        // 4. Text selection + cleaned plain-text copy with dehyphenation (OCRCOPY-001, OCRCOPY-002):
        let sel = document_core_select_page_text(snap.session_id.clone(), 1, None)
            .expect("select page 1 text");
        assert!(sel.raw_text.contains("trust-\nworthy"));
        assert!(sel.cleaned_text.contains(
            "Vocument provides trustworthy offline document workflows across macOS and Android."
        ));
        assert!(
            sel.cleaned_text
                .contains("\n\nSecond paragraph remains separated.")
        );

        // 5. Progressive SQLite FTS5 indexing & M0.3 Gate (search works before document is fully indexed):
        let batch_1 = document_core_index_search_batch(snap.session_id.clone(), 2, 2)
            .expect("index first 2 pages starting at page 2");
        assert_eq!(batch_1.indexed_pages, 2);
        assert_eq!(batch_1.total_pages, 4);
        assert!(!batch_1.is_complete);

        let search_start = Instant::now();
        let partial_search =
            document_core_search_exact(snap.session_id.clone(), "SQLite FTS5".to_owned())
                .expect("exact search on partial FTS5 index");
        let search_elapsed_ms = search_start.elapsed().as_millis();
        // Quality budget: indexed search first result < 150 ms (config/performance-budgets.toml)
        assert!(search_elapsed_ms < 150);
        assert!(!partial_search.progress.is_complete);
        assert_eq!(partial_search.hits.len(), 1);
        assert_eq!(partial_search.hits[0].page_index, 2);
        assert_eq!(partial_search.hits[0].logical_page_label, "1");

        // Complete progressive indexing for remaining pages:
        let batch_2 = document_core_index_search_batch(snap.session_id.clone(), 0, 4)
            .expect("finish indexing remaining pages");
        assert!(batch_2.is_complete);
        assert_eq!(batch_2.indexed_pages, 4);

        document_core_close_session(snap.session_id).expect("close session");
    }
}

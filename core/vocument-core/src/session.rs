//! `DocumentSession` and `SessionRegistry` (`ARCHITECTURE.md` §§6–10, 17, `ROADMAP.md` M0.1–M0.3).
//!
//! Each opened document receives an application-owned `DocumentSession` with stable
//! `SessionId`, `DocumentId`, `FileIdentity`, revision token, serialized `DocumentActor`,
//! `PageLabelCatalog`, and progressive `SessionSearchIndex`.

use crate::config::PerformanceBudgetsConfig;
use crate::document_inspection::DocumentInspection;
use crate::file_broker::{BrokeredFileRead, FileAccessOrigin, FileBroker, FileBrokerError};
use crate::identity::{DocumentId, FileIdentity, FileIdentityComparison, SessionId};
use crate::jobs::{JobKind, JobManager, JobPriority};
use crate::navigation::{PageDescriptor, PageLabelCatalog, PageLabelRange, PageNavigationError};
use crate::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineError, PdfPageGeometry, PdfPageStructuredText,
    PdfRectMilliPt, PdfTileRenderRequest,
};
use crate::render::{DocumentActor, RenderPipeline, RenderPipelineError, ViewportRenderResult};
use crate::search_index::{
    SearchIndexError, SearchIndexProgress, SearchResponse, SessionSearchIndex,
};
use crate::text_selection::{TextSelectionResult, select_all_page_text, select_page_text_in_rect};
use crate::thumbnail_cache::{RenderedThumbnail, ThumbnailCacheStats};
use crate::tile_cache::{MemoryPressureLevel, RenderedTile, TileCacheStats};
use crate::transactions::{TransactionError, TransactionManager};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SessionError {
    #[error(transparent)]
    FileBroker(#[from] FileBrokerError),
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
    #[error(transparent)]
    RenderPipeline(#[from] RenderPipelineError),
    #[error(transparent)]
    Navigation(#[from] PageNavigationError),
    #[error(transparent)]
    SearchIndex(#[from] SearchIndexError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Workbench(#[from] crate::workbench::WorkbenchError),
    #[error(transparent)]
    Save(#[from] crate::save::SaveError),
    #[error(transparent)]
    Recovery(#[from] crate::recovery::RecoveryError),
    #[error("unknown session ID '{session_id}'")]
    SessionNotFound { session_id: String },
    #[error(
        "the document of session '{session_id}' lives in a platform document provider and cannot be written through the local file broker; export it instead"
    )]
    NotLocallyWritable { session_id: String },
    #[error("session registry lock poisoned")]
    RegistryPoisoned,
    #[error("redaction failed: {0}")]
    Redaction(#[from] crate::redaction::RedactionError),
    /// A redaction failed and the document could **not** be put back
    /// (`REDACT-002`).
    ///
    /// The document in the session is in an unknown state. The user has to
    /// open it again — the file on the disk still holds the original, but the
    /// session does not know what it is holding.
    #[error(
        "the redaction failed and the document could not be put back; the session is in an unknown state, open the document again. The error was: {original}. The rollback failed with: {rollback}"
    )]
    RedactionRollbackFailed { original: String, rollback: String },
    /// A comment of the reviewer was refused (`ANNOT-001`).
    ///
    /// The core refuses the note: a note without a text, a note without an
    /// area, or a note with a key that the rail of the profession does not
    /// carry.
    #[error("the comment was refused: {details}")]
    CommentInvalid { details: String },
}

/// Immutable snapshot of an active `DocumentSession` exposed to native UI across UniFFI (`ARCHITECTURE.md` §§5–6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSessionSnapshot {
    pub session_id: SessionId,
    pub document_id: DocumentId,
    pub revision: u64,
    pub is_dirty: bool,
    pub access_origin: FileAccessOrigin,
    pub file_identity: FileIdentity,
    pub pdf_summary: PdfDocumentSummary,
}

/// Application-owned live state for one opened document (`ARCHITECTURE.md` §§6–7, 11, 17).
#[derive(Debug)]
pub struct DocumentSession {
    pub(crate) session_id: SessionId,
    pub(crate) document_id: DocumentId,
    pub(crate) revision: u64,
    pub(crate) is_dirty: bool,
    pub(crate) access_origin: FileAccessOrigin,
    pub(crate) file_identity: FileIdentity,
    pub(crate) pdf_summary: PdfDocumentSummary,
    pub(crate) actor: Arc<DocumentActor>,
    pub(crate) label_catalog: PageLabelCatalog,
    pub(crate) search_index: SessionSearchIndex,
    pub(crate) transaction_manager: TransactionManager,
}

impl DocumentSession {
    pub fn snapshot(&self) -> DocumentSessionSnapshot {
        DocumentSessionSnapshot {
            session_id: self.session_id.clone(),
            document_id: self.document_id.clone(),
            revision: self.revision,
            is_dirty: self.is_dirty,
            access_origin: self.access_origin,
            file_identity: self.file_identity.clone(),
            pdf_summary: self.pdf_summary.clone(),
        }
    }

    pub fn actor(&self) -> Arc<DocumentActor> {
        Arc::clone(&self.actor)
    }

    pub fn compare_external_identity(&self, observed: &FileIdentity) -> FileIdentityComparison {
        self.file_identity.compare_with(observed)
    }

    pub(crate) fn sync_from_transaction_manager(&mut self) {
        self.revision = self.transaction_manager.current_revision();
        self.is_dirty = self.transaction_manager.is_dirty();
        if let Ok(summary) = self.actor.summary() {
            if summary.page_count != self.pdf_summary.page_count {
                self.label_catalog = PageLabelCatalog::new(summary.page_count, Vec::new());
            }
            self.pdf_summary = summary;
        }
    }

    /// Marks the active session as cleanly saved and updates its baseline `FileIdentity` (`ARCHITECTURE.md` §16, `ROADMAP.md` M0.5).
    pub fn mark_as_saved(&mut self, new_identity: FileIdentity) {
        let new_doc_id = new_identity.stable_document_id();
        let new_sha = new_identity.sha256_hex.clone();
        self.file_identity = new_identity;
        self.document_id = new_doc_id.clone();
        self.transaction_manager.mark_as_saved(new_doc_id, new_sha);
        self.sync_from_transaction_manager();
    }
}

/// Thread-safe registry owning active `DocumentSession` instances, central `JobManager`,
/// and shared `RenderPipeline` inside the Rust core (`ARCHITECTURE.md` §§6–9).
#[derive(Debug)]
pub struct SessionRegistry {
    pub(crate) pdf_engine: Box<dyn PdfEngine>,
    pub(crate) file_broker: Arc<dyn FileBroker>,
    pub(crate) recovery_store: Arc<crate::recovery::RecoveryStore>,
    /// Reads the words that are readable on a rendered page (`REDACT-002`).
    ///
    /// Without a reader the verification of a redaction cannot cover the
    /// picture of the page, and such a redaction is reported as *not
    /// verified* — never as secure.
    pub(crate) rendered_page_reader: Option<Arc<dyn crate::rendered_page::RenderedPageReader>>,
    job_manager: JobManager,
    render_pipeline: RenderPipeline,
    next_session_number: AtomicU64,
    pub(crate) sessions: Mutex<BTreeMap<String, DocumentSession>>,
    /// The rail of the profession: the palette the comments are written with
    /// (`ANNOT-001`). The user may edit it; the default is a starting point.
    pub(crate) jurist_palette: Mutex<crate::annotations::JuristPalette>,
}

impl SessionRegistry {
    pub fn new(pdf_engine: Box<dyn PdfEngine>) -> Self {
        Self::with_broker(
            pdf_engine,
            Arc::new(crate::file_broker::ScopedLocalFileBroker::new()),
        )
    }

    pub fn with_broker(pdf_engine: Box<dyn PdfEngine>, file_broker: Arc<dyn FileBroker>) -> Self {
        Self::with_broker_and_recovery(
            pdf_engine,
            file_broker,
            Arc::new(crate::recovery::RecoveryStore::default_app_store()),
        )
    }

    pub fn with_broker_and_recovery(
        pdf_engine: Box<dyn PdfEngine>,
        file_broker: Arc<dyn FileBroker>,
        recovery_store: Arc<crate::recovery::RecoveryStore>,
    ) -> Self {
        let b = PerformanceBudgetsConfig::load_embedded().ok();
        let (tile_mb, thumb_mb, radius, yield_bg) = b
            .as_ref()
            .map(|cfg| {
                (
                    cfg.rendering_cache.android_tile_cache_max_mb as usize,
                    cfg.rendering_cache.thumbnail_cache_max_mb as usize,
                    cfg.rendering_cache.nearby_page_prefetch_radius,
                    cfg.jobs.yield_background_on_interaction,
                )
            })
            .unwrap_or((192, 64, 2, true));

        Self {
            pdf_engine,
            file_broker,
            recovery_store,
            rendered_page_reader: None,
            job_manager: JobManager::new(yield_bg),
            render_pipeline: RenderPipeline::with_thumbnail_budget(
                tile_mb * 1024 * 1024,
                thumb_mb * 1024 * 1024,
                radius,
            ),
            next_session_number: AtomicU64::new(1),
            sessions: Mutex::new(BTreeMap::new()),
            jurist_palette: Mutex::new(crate::annotations::JuristPalette::default_palette()),
        }
    }

    /// Wires the reader that reads the words on a rendered page.
    ///
    /// The platform wires the implementation: the PDF engine renders the
    /// page, the OCR engine reads the picture (`REDACT-002`).
    pub fn with_rendered_page_reader(
        mut self,
        reader: Arc<dyn crate::rendered_page::RenderedPageReader>,
    ) -> Self {
        self.rendered_page_reader = Some(reader);
        self
    }

    /// Reads the words that are readable on the rendered page, if a reader is
    /// wired (`REDACT-002`).
    pub(crate) fn read_rendered_words(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<Option<Vec<crate::pdf_engine::PdfTextWord>>, SessionError> {
        let Some(reader) = self.rendered_page_reader.as_ref() else {
            return Ok(None);
        };
        let bytes = self.with_session(session_id, |session| {
            Ok(session.actor.current_pdf_bytes()?.to_vec())
        })?;
        let words = reader
            .read_rendered_words(&bytes, page_index)
            .map_err(SessionError::PdfEngine)?;
        Ok(Some(words))
    }

    pub fn file_broker(&self) -> Arc<dyn FileBroker> {
        Arc::clone(&self.file_broker)
    }

    pub fn pdf_engine(&self) -> &dyn PdfEngine {
        self.pdf_engine.as_ref()
    }

    pub fn recovery_store(&self) -> Arc<crate::recovery::RecoveryStore> {
        Arc::clone(&self.recovery_store)
    }

    /// Opens a PDF through a `FileBroker` source token and registers a new `DocumentSession`.
    pub fn open_via_broker(
        &self,
        broker: &dyn FileBroker,
        source_token: &str,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        let brokered = broker.read_file(source_token)?;
        self.open_brokered_file(brokered)
    }

    /// Opens a PDF from a platform-provided `BrokeredFileRead` (`MacOsScopedUrl` or `AndroidSafUri`).
    pub fn open_brokered_file(
        &self,
        brokered: BrokeredFileRead,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        if brokered.bytes.is_empty() {
            return Err(SessionError::FileBroker(FileBrokerError::EmptyPayload {
                source_token: brokered.source_token,
            }));
        }

        let file_identity = brokered.compute_identity();
        let document_id = file_identity.stable_document_id();
        let engine_document = self.pdf_engine.open_pdf_bytes(&brokered.bytes)?;
        let pdf_summary = engine_document.summary().clone();

        let mut embedded_labels = Vec::with_capacity(pdf_summary.page_count as usize);
        for page_idx in 0..pdf_summary.page_count {
            embedded_labels.push(engine_document.page_label(page_idx).ok().flatten());
        }

        let label_catalog = PageLabelCatalog::new(pdf_summary.page_count, embedded_labels);
        let search_index = SessionSearchIndex::new(pdf_summary.page_count)?;
        let transaction_manager = TransactionManager::new(document_id.clone(), &brokered.bytes);
        let actor = Arc::new(DocumentActor::new(engine_document));

        let counter = self.next_session_number.fetch_add(1, Ordering::Relaxed);
        let session_id = SessionId::new(counter, &document_id);

        let session = DocumentSession {
            session_id: session_id.clone(),
            document_id,
            revision: 1,
            is_dirty: false,
            access_origin: brokered.access_origin,
            file_identity,
            pdf_summary,
            actor,
            label_catalog,
            search_index,
            transaction_manager,
        };

        let snapshot = session.snapshot();
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        guard.insert(session_id.as_str().to_owned(), session);
        Ok(snapshot)
    }

    pub(crate) fn with_session<T, F>(&self, session_id: &str, f: F) -> Result<T, SessionError>
    where
        F: FnOnce(&DocumentSession) -> Result<T, SessionError>,
    {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        f(session)
    }

    /// Drops the cached tiles and thumbnails of a document.
    ///
    /// After a redaction the pictures of the earlier revision still show the
    /// removed content. They are dropped, so that the text is not readable in
    /// the memory of the process any more.
    pub(crate) fn purge_render_cache(&self, document_id: &str) {
        self.render_pipeline.purge_document_cache(document_id);
    }

    /// Grants mutable access to a session (`REDACT-002`).
    ///
    /// A redaction that cannot be verified is rolled back. The rollback
    /// removes the transaction of the attempt from the transaction manager, so
    /// that the journal of the recovery store does not replay a redaction that
    /// never happened.
    pub(crate) fn with_session_mut<T, F>(&self, session_id: &str, f: F) -> Result<T, SessionError>
    where
        F: FnOnce(&mut DocumentSession) -> Result<T, SessionError>,
    {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        f(session)
    }

    /// Retrieves an immutable snapshot of an existing `DocumentSession`.
    pub fn session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        self.with_session(session_id, |s| Ok(s.snapshot()))
    }

    /// Queries the point geometry (`width_milli_pt`, `height_milli_pt`) of `page_index` for `session_id`.
    pub fn page_geometry(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<PdfPageGeometry, SessionError> {
        let actor = self.session_actor_and_meta(session_id)?.0;
        Ok(actor.page_geometry(page_index)?)
    }

    /// Returns all `PageDescriptor` entries (`page_index`, `physical_page_number`, `logical_label`) for `session_id` (`PAGELABEL-002`).
    pub fn page_descriptors(&self, session_id: &str) -> Result<Vec<PageDescriptor>, SessionError> {
        self.with_session(session_id, |s| Ok(s.label_catalog.all_descriptors()))
    }

    /// Resolves a navigation target (`query`: logical label like `"ii"` or `"A-2"`, or physical page `"2"` / `"#2"`) (`PAGELABEL-003`).
    pub fn resolve_page_navigation(
        &self,
        session_id: &str,
        query: &str,
    ) -> Result<PageDescriptor, SessionError> {
        self.with_session(session_id, |s| {
            Ok(s.label_catalog.resolve_navigation_target(query)?)
        })
    }

    /// Creates or replaces custom logical page-label ranges on `session_id` (`PAGELABEL-004`).
    pub fn set_page_label_ranges(
        &self,
        session_id: &str,
        ranges: Vec<PageLabelRange>,
    ) -> Result<Vec<PageDescriptor>, SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get_mut(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        session.label_catalog.set_custom_ranges(ranges)?;
        session.revision += 1;
        session.is_dirty = true;
        Ok(session.label_catalog.all_descriptors())
    }

    /// Renders a visible tile at `VisiblePageRender` priority (serving from `TileCache` on hit).
    pub fn render_visible_tile(
        &self,
        session_id: &str,
        request: PdfTileRenderRequest,
    ) -> Result<RenderedTile, SessionError> {
        let (actor, document_id, revision) = self.session_actor_and_meta(session_id)?;
        let tile = self.render_pipeline.render_tile_with_priority(
            &actor,
            &self.job_manager,
            session_id,
            document_id.as_str(),
            revision,
            request,
            JobPriority::VisiblePageRender,
        )?;
        Ok(tile)
    }

    /// Renders a low-resolution page thumbnail via the separate `ThumbnailCache` (`PAGEVIEW-001`, `PAGEVIEW-002`, `ROADMAP.md` M0.3).
    pub fn render_page_thumbnail(
        &self,
        session_id: &str,
        page_index: u32,
        max_dimension_px: u32,
    ) -> Result<RenderedThumbnail, SessionError> {
        let (actor, document_id, revision, logical_label) = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            if page_index >= session.pdf_summary.page_count {
                return Err(SessionError::PdfEngine(PdfEngineError::PageOutOfBounds {
                    page_index,
                    page_count: session.pdf_summary.page_count,
                }));
            }
            (
                session.actor(),
                session.document_id.clone(),
                session.revision,
                session.label_catalog.label_for_page(page_index),
            )
        };

        let thumb = self.render_pipeline.render_page_thumbnail(
            &actor,
            &self.job_manager,
            session_id,
            document_id.as_str(),
            revision,
            page_index,
            logical_label,
            max_dimension_px,
        )?;
        Ok(thumb)
    }

    /// Renders the requested visible viewport tile and prefetches neighbouring pages (`ROADMAP.md` M0.2).
    pub fn render_viewport_with_prefetch(
        &self,
        session_id: &str,
        request: PdfTileRenderRequest,
    ) -> Result<ViewportRenderResult, SessionError> {
        let (actor, document_id, revision) = self.session_actor_and_meta(session_id)?;
        let result = self.render_pipeline.render_viewport_and_prefetch(
            &actor,
            &self.job_manager,
            session_id,
            document_id.as_str(),
            revision,
            request,
        )?;
        Ok(result)
    }

    /// Selects text on `page_index` (either full-page when `selection_rect` is `None`, or intersecting `selection_rect`),
    /// returning both `raw_text` (`OCRCOPY-001`) and `cleaned_text` (`OCRCOPY-002`) plus `highlight_rects`.
    pub fn select_page_text(
        &self,
        session_id: &str,
        page_index: u32,
        selection_rect: Option<PdfRectMilliPt>,
    ) -> Result<TextSelectionResult, SessionError> {
        let actor = self.session_actor_and_meta(session_id)?.0;
        let structured = actor.extract_page_text(page_index)?;
        let selection = match selection_rect {
            Some(rect) => select_page_text_in_rect(&structured, rect),
            None => select_all_page_text(&structured),
        };
        Ok(selection)
    }

    /// Returns the structured text of `page_index` with the boxes of its words
    /// (`REDACT-001`).
    pub fn page_text(
        &self,
        session_id: &str,
        page_index: u32,
    ) -> Result<PdfPageStructuredText, SessionError> {
        let actor = self.session_actor_and_meta(session_id)?.0;
        Ok(actor.extract_page_text(page_index)?)
    }

    /// Inspects the links and actions of the document (`LINKSEC-001`).
    ///
    /// The report tells the user where a link leads and whether the document
    /// does something besides showing pages — a script, a file that is
    /// started, a form that is sent away. The risk policy lives in the core,
    /// the engine only reports the facts.
    pub fn inspect_document(&self, session_id: &str) -> Result<DocumentInspection, SessionError> {
        let actor = self.session_actor_and_meta(session_id)?.0;
        let actions = actor.document_actions()?;
        let attachments = actor.document_attachments()?;
        let signatures = actor.document_signatures()?;
        Ok(DocumentInspection::from_findings(
            actions,
            attachments,
            signatures,
        ))
    }

    /// Progressively indexes up to `max_pages` into the session's SQLite FTS5 index, starting around
    /// `preferred_start_page` (`SEARCH-009`, `ARCHITECTURE.md` §17, `ROADMAP.md` M0.3 Gate).
    pub fn index_search_batch(
        &self,
        session_id: &str,
        preferred_start_page: u32,
        max_pages: u32,
    ) -> Result<SearchIndexProgress, SessionError> {
        let (actor, revision, pages_to_index) = {
            let guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session = guard
                .get(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?;
            let batch = session
                .search_index
                .next_unindexed_pages(preferred_start_page, max_pages);
            (session.actor(), session.revision, batch)
        };

        if pages_to_index.is_empty() {
            return self.search_index_progress(session_id);
        }

        let job = self
            .job_manager
            .enqueue_job(
                session_id,
                revision,
                JobKind::IndexSearch,
                JobPriority::BackgroundOcrIndex,
                false,
            )
            .map_err(|err| {
                SessionError::RenderPipeline(RenderPipelineError::JobFailure {
                    details: err.to_string(),
                })
            })?;
        let _ = self.job_manager.mark_running(job.job_id);

        let mut extracted_pages = Vec::with_capacity(pages_to_index.len());
        for page_idx in pages_to_index {
            extracted_pages.push(actor.extract_page_text(page_idx)?);
        }

        let progress = {
            let mut guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            let session =
                guard
                    .get_mut(session_id)
                    .ok_or_else(|| SessionError::SessionNotFound {
                        session_id: session_id.to_owned(),
                    })?;
            for page_data in extracted_pages {
                let _ = session.search_index.index_structured_page(page_data)?;
            }
            session.search_index.progress()
        };

        let _ = self.job_manager.mark_completed(job.job_id);
        Ok(progress)
    }

    /// Queries the current progressive SQLite FTS5 indexing progress for `session_id`.
    pub fn search_index_progress(
        &self,
        session_id: &str,
    ) -> Result<SearchIndexProgress, SessionError> {
        self.with_session(session_id, |s| Ok(s.search_index.progress()))
    }

    /// Runs a classic exact FTS5 search against all currently indexed pages in `session_id` (`SEARCH-002`, `ROADMAP.md` M0.3 Gate).
    pub fn search_exact(
        &self,
        session_id: &str,
        query: &str,
    ) -> Result<SearchResponse, SessionError> {
        self.with_session(session_id, |s| {
            Ok(s.search_index.search_exact(query, &s.label_catalog)?)
        })
    }

    /// Applies an OS memory-pressure level (`Normal`, `Warning`, `Critical`) to the shared tile cache,
    /// thumbnail cache, and active `DocumentActor` display-list caches (`ARCHITECTURE.md` §9).
    pub fn handle_memory_pressure(
        &self,
        level: MemoryPressureLevel,
    ) -> Result<TileCacheStats, SessionError> {
        if level == MemoryPressureLevel::Critical {
            let actors: Vec<Arc<DocumentActor>> = {
                let guard = self
                    .sessions
                    .lock()
                    .map_err(|_| SessionError::RegistryPoisoned)?;
                guard.values().map(DocumentSession::actor).collect()
            };
            for actor in actors {
                let _ = actor.clear_display_lists();
            }
        }
        Ok(self.render_pipeline.handle_memory_pressure(None, level)?)
    }

    /// Returns current `TileCacheStats`.
    pub fn tile_cache_stats(&self) -> Result<TileCacheStats, SessionError> {
        Ok(self.render_pipeline.cache_stats()?)
    }

    /// Returns current `ThumbnailCacheStats`.
    pub fn thumbnail_cache_stats(&self) -> Result<ThumbnailCacheStats, SessionError> {
        Ok(self.render_pipeline.thumbnail_cache_stats()?)
    }

    /// Closes an active `DocumentSession` and releases its underlying engine resources.
    pub fn close_session(&self, session_id: &str) -> Result<(), SessionError> {
        let removed = {
            let mut guard = self
                .sessions
                .lock()
                .map_err(|_| SessionError::RegistryPoisoned)?;
            guard
                .remove(session_id)
                .ok_or_else(|| SessionError::SessionNotFound {
                    session_id: session_id.to_owned(),
                })?
        };
        let _ = self
            .recovery_store
            .container_for(&removed.document_id)
            .mark_clean_shutdown();
        Ok(())
    }

    /// Returns the number of currently active sessions.
    pub fn active_session_count(&self) -> Result<usize, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        Ok(guard.len())
    }

    fn session_actor_and_meta(
        &self,
        session_id: &str,
    ) -> Result<(Arc<DocumentActor>, DocumentId, u64), SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok((
            session.actor(),
            session.document_id.clone(),
            session.revision,
        ))
    }
}

//! `DocumentSession` and `SessionRegistry` (`ARCHITECTURE.md` §§6–9, `ROADMAP.md` M0.1–M0.2).
//!
//! Each opened document receives an application-owned `DocumentSession` with stable
//! `SessionId`, `DocumentId`, `FileIdentity`, revision token, and serialized `DocumentActor`.

use crate::config::PerformanceBudgetsConfig;
use crate::file_broker::{BrokeredFileRead, FileAccessOrigin, FileBroker, FileBrokerError};
use crate::identity::{DocumentId, FileIdentity, FileIdentityComparison, SessionId};
use crate::jobs::{JobManager, JobPriority};
use crate::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineError, PdfPageGeometry, PdfTileRenderRequest,
};
use crate::render::{DocumentActor, RenderPipeline, RenderPipelineError, ViewportRenderResult};
use crate::tile_cache::{MemoryPressureLevel, RenderedTile, TileCacheStats};
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
    #[error("unknown session ID '{session_id}'")]
    SessionNotFound { session_id: String },
    #[error("session registry lock poisoned")]
    RegistryPoisoned,
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

/// Application-owned live state for one opened document (`ARCHITECTURE.md` §§6–7).
#[derive(Debug)]
pub struct DocumentSession {
    session_id: SessionId,
    document_id: DocumentId,
    revision: u64,
    is_dirty: bool,
    access_origin: FileAccessOrigin,
    file_identity: FileIdentity,
    pdf_summary: PdfDocumentSummary,
    actor: Arc<DocumentActor>,
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
}

/// Thread-safe registry owning active `DocumentSession` instances, central `JobManager`,
/// and shared `RenderPipeline` inside the Rust core (`ARCHITECTURE.md` §§6–9).
#[derive(Debug)]
pub struct SessionRegistry {
    pdf_engine: Box<dyn PdfEngine>,
    job_manager: JobManager,
    render_pipeline: RenderPipeline,
    next_session_number: AtomicU64,
    sessions: Mutex<BTreeMap<String, DocumentSession>>,
}

impl SessionRegistry {
    pub fn new(pdf_engine: Box<dyn PdfEngine>) -> Self {
        let budgets = PerformanceBudgetsConfig::load_embedded().ok();
        let tile_cache_bytes = budgets
            .as_ref()
            .map(|b| (b.rendering_cache.android_tile_cache_max_mb as usize) * 1024 * 1024)
            .unwrap_or(192 * 1024 * 1024);
        let prefetch_radius = budgets
            .as_ref()
            .map(|b| b.rendering_cache.nearby_page_prefetch_radius)
            .unwrap_or(2);
        let yield_bg = budgets
            .as_ref()
            .map(|b| b.jobs.yield_background_on_interaction)
            .unwrap_or(true);

        Self {
            pdf_engine,
            job_manager: JobManager::new(yield_bg),
            render_pipeline: RenderPipeline::new(tile_cache_bytes, prefetch_radius),
            next_session_number: AtomicU64::new(1),
            sessions: Mutex::new(BTreeMap::new()),
        }
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
        };

        let snapshot = session.snapshot();
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        guard.insert(session_id.as_str().to_owned(), session);
        Ok(snapshot)
    }

    /// Retrieves an immutable snapshot of an existing `DocumentSession`.
    pub fn session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<DocumentSessionSnapshot, SessionError> {
        let guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        let session = guard
            .get(session_id)
            .ok_or_else(|| SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok(session.snapshot())
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

    /// Applies an OS memory-pressure level (`Normal`, `Warning`, `Critical`) to the shared tile cache
    /// and active `DocumentActor` display-list caches (`ARCHITECTURE.md` §9).
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

    /// Closes an active `DocumentSession` and releases its underlying engine resources.
    pub fn close_session(&self, session_id: &str) -> Result<(), SessionError> {
        let mut guard = self
            .sessions
            .lock()
            .map_err(|_| SessionError::RegistryPoisoned)?;
        if guard.remove(session_id).is_none() {
            return Err(SessionError::SessionNotFound {
                session_id: session_id.to_owned(),
            });
        }
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

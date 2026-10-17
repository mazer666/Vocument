//! `DocumentActor` and display-list / render-worker pipeline (`ARCHITECTURE.md` §§7–9, `ROADMAP.md` M0.2).
//!
//! Architecture rules (`ARCHITECTURE.md` §7):
//! - Each PDF session has a serialized `DocumentActor` for operations that mutate or depend on
//!   the mutable MuPDF document state (including page parsing and display-list creation).
//! - Immutable `PdfEngineDisplayList` instances are cached per `(page_index, revision)` and
//!   shared via `Arc` so tile rasterization can execute without holding the `DocumentActor` lock.
//! - Visible tiles run at `JobPriority::VisiblePageRender`; neighbouring-page prefetch runs at
//!   `JobPriority::NearbyPagePrefetch` (`ARCHITECTURE.md` §8).

use crate::jobs::{JobKind, JobManager, JobPriority};
use crate::pdf_engine::{
    PdfDocumentSummary, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError, PdfPageGeometry,
    PdfTileRenderRequest,
};
use crate::tile_cache::{MemoryPressureLevel, RenderedTile, TileCache, TileCacheStats, TileKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderPipelineError {
    #[error(transparent)]
    PdfEngine(#[from] PdfEngineError),
    #[error("document actor lock poisoned")]
    ActorPoisoned,
    #[error("display-list cache lock poisoned")]
    DisplayListCachePoisoned,
    #[error("tile cache lock poisoned")]
    TileCachePoisoned,
    #[error("job manager error: {details}")]
    JobFailure { details: String },
}

/// Summary of a viewport update (visible tile rendered + neighbouring pages prefetched) (`ROADMAP.md` M0.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewportRenderResult {
    pub visible_tile: RenderedTile,
    pub prefetched_pages: Vec<u32>,
    pub cache_stats: TileCacheStats,
}

/// Serialized owner of the mutable `PdfEngineDocument` state (`ARCHITECTURE.md` §7).
#[derive(Debug)]
pub struct DocumentActor {
    engine_document: Mutex<Box<dyn PdfEngineDocument>>,
    display_lists: Mutex<HashMap<(u64, u32), Arc<dyn PdfEngineDisplayList>>>,
}

impl DocumentActor {
    pub fn new(engine_document: Box<dyn PdfEngineDocument>) -> Self {
        Self {
            engine_document: Mutex::new(engine_document),
            display_lists: Mutex::new(HashMap::new()),
        }
    }

    /// Reads the document summary under serialized actor access.
    pub fn summary(&self) -> Result<PdfDocumentSummary, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.summary().clone())
    }

    /// Queries the geometry of `page_index` (`0`-based) under serialized actor access.
    pub fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_geometry(page_index)?)
    }

    /// Obtains an immutable `Arc<dyn PdfEngineDisplayList>` for `(revision, page_index)`.
    /// If not yet cached, parses the page exclusively inside the serialized `DocumentActor` lock,
    /// then releases the lock so tile rasterization workers can run concurrently (`ARCHITECTURE.md` §7).
    pub fn acquire_display_list(
        &self,
        revision: u64,
        page_index: u32,
    ) -> Result<Arc<dyn PdfEngineDisplayList>, RenderPipelineError> {
        {
            let cache_guard = self
                .display_lists
                .lock()
                .map_err(|_| RenderPipelineError::DisplayListCachePoisoned)?;
            if let Some(existing) = cache_guard.get(&(revision, page_index)) {
                return Ok(Arc::clone(existing));
            }
        }

        // Serialized page parsing / display-list creation on DocumentActor:
        let created_box = {
            let actor_guard = self
                .engine_document
                .lock()
                .map_err(|_| RenderPipelineError::ActorPoisoned)?;
            actor_guard.create_display_list(page_index)?
        };
        let created_arc: Arc<dyn PdfEngineDisplayList> = Arc::from(created_box);

        let mut cache_guard = self
            .display_lists
            .lock()
            .map_err(|_| RenderPipelineError::DisplayListCachePoisoned)?;
        cache_guard.insert((revision, page_index), Arc::clone(&created_arc));
        Ok(created_arc)
    }

    /// Clears cached display lists when Critical memory pressure is signaled.
    pub fn clear_display_lists(&self) -> Result<(), RenderPipelineError> {
        let mut cache_guard = self
            .display_lists
            .lock()
            .map_err(|_| RenderPipelineError::DisplayListCachePoisoned)?;
        cache_guard.clear();
        Ok(())
    }
}

/// Coordinates `DocumentActor`, `JobManager`, and adaptive `TileCache` for a session (`ROADMAP.md` M0.2).
#[derive(Debug)]
pub struct RenderPipeline {
    tile_cache: Mutex<TileCache>,
    prefetch_radius: u32,
}

impl RenderPipeline {
    pub fn new(tile_cache_max_bytes: usize, prefetch_radius: u32) -> Self {
        Self {
            tile_cache: Mutex::new(TileCache::new(tile_cache_max_bytes)),
            prefetch_radius,
        }
    }

    pub fn prefetch_radius(&self) -> u32 {
        self.prefetch_radius
    }

    /// Renders (or serves from `TileCache`) a single tile for `(session_id, document_id, revision)`.
    #[allow(clippy::too_many_arguments)]
    pub fn render_tile_with_priority(
        &self,
        actor: &DocumentActor,
        job_manager: &JobManager,
        session_id: &str,
        document_id: &str,
        revision: u64,
        request: PdfTileRenderRequest,
        priority: JobPriority,
    ) -> Result<RenderedTile, RenderPipelineError> {
        let tile_key = TileKey {
            document_id: document_id.to_owned(),
            revision,
            page_index: request.page_index,
            scale_milli: request.scale_milli,
            tile_x_px: request.tile_x_px,
            tile_y_px: request.tile_y_px,
            tile_width_px: request.tile_width_px,
            tile_height_px: request.tile_height_px,
        };

        // 1. Check adaptive TileCache first:
        {
            let mut cache_guard = self
                .tile_cache
                .lock()
                .map_err(|_| RenderPipelineError::TileCachePoisoned)?;
            if let Some(hit) = cache_guard.get(&tile_key) {
                return Ok(hit);
            }
        }

        // 2. Schedule render job in central JobManager:
        let kind = if priority == JobPriority::VisiblePageRender {
            JobKind::RenderVisibleTile
        } else {
            JobKind::PrefetchNeighborTile
        };
        let job = job_manager
            .enqueue_job(session_id, revision, kind, priority, false)
            .map_err(|err| RenderPipelineError::JobFailure {
                details: err.to_string(),
            })?;
        let _ = job_manager.mark_running(job.job_id);

        // 3. Acquire immutable display-list via serialized DocumentActor:
        let display_list = actor.acquire_display_list(revision, request.page_index)?;

        // 4. Execute derived rasterization outside DocumentActor lock:
        let bitmap = display_list.render_tile(request)?;
        let rendered = RenderedTile {
            key: tile_key,
            width_px: bitmap.width_px,
            height_px: bitmap.height_px,
            stride_bytes: bitmap.stride_bytes,
            rgba_bytes: bitmap.rgba_bytes,
        };

        // 5. Store in adaptive LRU TileCache and complete job:
        {
            let mut cache_guard = self
                .tile_cache
                .lock()
                .map_err(|_| RenderPipelineError::TileCachePoisoned)?;
            cache_guard.insert(rendered.clone());
        }
        let _ = job_manager.mark_completed(job.job_id);

        Ok(rendered)
    }

    /// Renders the requested visible tile at highest priority (`VisiblePageRender`) and prefetches
    /// neighbouring pages within `prefetch_radius` at `NearbyPagePrefetch` priority (`ROADMAP.md` M0.2).
    pub fn render_viewport_and_prefetch(
        &self,
        actor: &DocumentActor,
        job_manager: &JobManager,
        session_id: &str,
        document_id: &str,
        revision: u64,
        request: PdfTileRenderRequest,
    ) -> Result<ViewportRenderResult, RenderPipelineError> {
        let _ = job_manager.cancel_queued_prefetch_for_session(session_id);

        let visible_tile = self.render_tile_with_priority(
            actor,
            job_manager,
            session_id,
            document_id,
            revision,
            request,
            JobPriority::VisiblePageRender,
        )?;

        let summary = actor.summary()?;
        let page_count = summary.page_count;
        let mut prefetched_pages = Vec::new();

        for delta in 1..=self.prefetch_radius {
            if let Some(next_page) = request.page_index.checked_add(delta)
                && next_page < page_count
            {
                let prefetch_req = PdfTileRenderRequest {
                    page_index: next_page,
                    scale_milli: request.scale_milli,
                    tile_x_px: 0,
                    tile_y_px: 0,
                    tile_width_px: request.tile_width_px,
                    tile_height_px: request.tile_height_px,
                };
                let _ = self.render_tile_with_priority(
                    actor,
                    job_manager,
                    session_id,
                    document_id,
                    revision,
                    prefetch_req,
                    JobPriority::NearbyPagePrefetch,
                )?;
                prefetched_pages.push(next_page);
            }
            if let Some(prev_page) = request.page_index.checked_sub(delta)
                && prev_page < page_count
            {
                let prefetch_req = PdfTileRenderRequest {
                    page_index: prev_page,
                    scale_milli: request.scale_milli,
                    tile_x_px: 0,
                    tile_y_px: 0,
                    tile_width_px: request.tile_width_px,
                    tile_height_px: request.tile_height_px,
                };
                let _ = self.render_tile_with_priority(
                    actor,
                    job_manager,
                    session_id,
                    document_id,
                    revision,
                    prefetch_req,
                    JobPriority::NearbyPagePrefetch,
                )?;
                prefetched_pages.push(prev_page);
            }
        }

        let cache_stats = self.cache_stats()?;
        Ok(ViewportRenderResult {
            visible_tile,
            prefetched_pages,
            cache_stats,
        })
    }

    /// Propagates OS memory pressure (`Normal`, `Warning`, `Critical`) to the tile cache (`ARCHITECTURE.md` §9).
    pub fn handle_memory_pressure(
        &self,
        actor: Option<&DocumentActor>,
        level: MemoryPressureLevel,
    ) -> Result<TileCacheStats, RenderPipelineError> {
        if level == MemoryPressureLevel::Critical
            && let Some(act) = actor
        {
            let _ = act.clear_display_lists();
        }
        let mut cache_guard = self
            .tile_cache
            .lock()
            .map_err(|_| RenderPipelineError::TileCachePoisoned)?;
        Ok(cache_guard.apply_memory_pressure(level))
    }

    pub fn cache_stats(&self) -> Result<TileCacheStats, RenderPipelineError> {
        let cache_guard = self
            .tile_cache
            .lock()
            .map_err(|_| RenderPipelineError::TileCachePoisoned)?;
        Ok(cache_guard.stats())
    }
}

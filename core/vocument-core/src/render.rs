//! `DocumentActor` and display-list / render-worker pipeline (`ARCHITECTURE.md` §§7–10, `ROADMAP.md` M0.2–M0.3).
//!
//! Architecture rules (`ARCHITECTURE.md` §§7–10):
//! - Each PDF session has a serialized `DocumentActor` for operations that mutate or depend on
//!   the mutable MuPDF document state (including page parsing, display-list creation, and structured text extraction).
//! - Immutable `PdfEngineDisplayList` instances are cached per `(page_index, revision)` and
//!   shared via `Arc` so tile and thumbnail rasterization can execute without holding the `DocumentActor` lock.
//! - Visible tiles run at `JobPriority::VisiblePageRender`; neighbouring-page prefetch runs at
//!   `JobPriority::NearbyPagePrefetch`; thumbnails run at `JobPriority::ThumbnailGeneration` (`ARCHITECTURE.md` §8).

use crate::document_inspection::{PdfActionInfo, PdfAttachmentInfo, PdfSignatureInfo};
use crate::jobs::{JobKind, JobManager, JobPriority};
use crate::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngineDisplayList, PdfEngineDocument,
    PdfEngineError, PdfHighlightAnnotation, PdfPageBoxes, PdfPageGeometry, PdfPageStructuredText,
    PdfRectMilliPt, PdfTextOccurrence, PdfTileRenderRequest,
};
use crate::thumbnail_cache::{
    RenderedThumbnail, ThumbnailCache, ThumbnailCacheStats, ThumbnailKey,
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
    #[error(transparent)]
    Workbench(#[from] crate::workbench::WorkbenchError),
    #[error("document actor lock poisoned")]
    ActorPoisoned,
    #[error("display-list cache lock poisoned")]
    DisplayListCachePoisoned,
    #[error("tile cache lock poisoned")]
    TileCachePoisoned,
    #[error("thumbnail cache lock poisoned")]
    ThumbnailCachePoisoned,
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

    /// Returns a copy of the current in-memory PDF bytes under serialized actor access (`ROADMAP.md` M0.4).
    pub fn current_pdf_bytes(&self) -> Result<Vec<u8>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.current_pdf_bytes().to_vec())
    }

    /// Queries the geometry of `page_index` (`0`-based) under serialized actor access.
    pub fn page_geometry(&self, page_index: u32) -> Result<PdfPageGeometry, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_geometry(page_index)?)
    }

    /// Queries the rotation (`0`, `90`, `180`, or `270` degrees) of `page_index` (`ROTATE-001`).
    pub fn page_rotation_degrees(&self, page_index: u32) -> Result<u32, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_rotation_degrees(page_index)?)
    }

    /// Queries all standard PDF `/Highlight` annotations on `page_index` (`ANNOT-001`, `INTEROP-002`).
    pub fn page_highlight_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_highlight_annotations(page_index)?)
    }

    /// Queries all comments of the reviewer on `page_index` (`ANNOT-001`).
    pub fn page_comment_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_comment_annotations(page_index)?)
    }

    /// Queries the embedded PDF logical page label (from `/PageLabels`) for `page_index` (`0`-based).
    pub fn page_label(&self, page_index: u32) -> Result<Option<String>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.page_label(page_index)?)
    }

    /// Reports the links and actions of the document (`LINKSEC-001`).
    pub fn document_actions(&self) -> Result<Vec<PdfActionInfo>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.document_actions()?)
    }

    /// Reports the embedded files of the document (`ATTACH-001`).
    pub fn document_attachments(&self) -> Result<Vec<PdfAttachmentInfo>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.document_attachments()?)
    }

    /// Reports the signature fields of the document (`SIGREV-001`).
    pub fn document_signatures(&self) -> Result<Vec<PdfSignatureInfo>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.document_signatures()?)
    }

    /// Puts the bytes of the document back (`REDACT-002`).
    ///
    /// A redaction that cannot be verified is rolled back: the document has to
    /// be the document it was before the attempt.
    pub fn restore_pdf_bytes(&self, bytes: Vec<u8>) -> Result<(), RenderPipelineError> {
        self.apply_engine_mutation_for_transaction_manager(|doc: &mut dyn PdfEngineDocument| {
            doc.replace_pdf_bytes(bytes)
        })
    }

    /// Reports the places of the document that still carry one of the given
    /// words (`REDACT-002`).
    ///
    /// The verification of a redaction uses the report: the words that were
    /// removed must not be found in the document any more — not in the text
    /// layer, not in an annotation, not in a form value, not in the metadata
    /// and not in an embedded file.
    pub fn find_word_occurrences(
        &self,
        words: &[String],
    ) -> Result<Vec<PdfTextOccurrence>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.find_word_occurrences(words)?)
    }

    /// Extracts structured text lines and bounding boxes for `page_index` (`0`-based) under serialized actor access (`ROADMAP.md` M0.3).
    pub fn extract_page_text(
        &self,
        page_index: u32,
    ) -> Result<PdfPageStructuredText, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.extract_page_text(page_index)?)
    }

    /// Extracts specified pages into a new standalone PDF buffer (`ROADMAP.md` M3).
    pub fn extract_pages(&self, page_indices: &[u32]) -> Result<Vec<u8>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.extract_pages(page_indices)?)
    }

    /// Splits specified page ranges into separate standalone PDF buffers (`ROADMAP.md` M3.2).
    pub fn split_document(
        &self,
        split_ranges: &[Vec<u32>],
    ) -> Result<Vec<Vec<u8>>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.split_document(split_ranges)?)
    }

    /// Performs structural preflight inspection before Page Workbench operations (`ROADMAP.md` M3, `ADR-0019`).
    pub fn workbench_preflight(
        &self,
    ) -> Result<crate::workbench::PageWorkbenchPreflightReport, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(crate::workbench::audit_workbench_preflight(guard.as_ref())?)
    }

    /// Queries the 5 standard ISO 32000-1 page boundary boxes and rotation (`CROP-002`).
    pub fn get_page_boxes(&self, page_index: u32) -> Result<PdfPageBoxes, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.get_page_boxes(page_index)?)
    }

    /// Detects document skew angle on `page_index` in degrees (`ROTATE-003`).
    pub fn detect_deskew_angle(&self, page_index: u32) -> Result<f32, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.detect_deskew_angle(page_index)?)
    }

    /// Detects content bounding box on `page_index` with a safety margin (`CROP-003`).
    pub fn detect_content_margins(
        &self,
        page_index: u32,
        safety_margin_milli_pt: i32,
    ) -> Result<PdfRectMilliPt, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.detect_content_margins(page_index, safety_margin_milli_pt)?)
    }

    /// Retrieves document bookmark (outline) hierarchy (`M3.4`).
    pub fn get_bookmarks(
        &self,
    ) -> Result<Vec<crate::workbench_stamps::BookmarkItem>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.get_bookmarks()?)
    }

    pub fn get_document_images(
        &self,
    ) -> Result<Vec<crate::workbench_optimize::ExtractedImageInfo>, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.get_document_images()?)
    }

    pub fn extract_image_bytes(
        &self,
        page_index: u32,
        image_index: u32,
    ) -> Result<crate::workbench_optimize::ExtractedImageData, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.extract_image_bytes(page_index, image_index)?)
    }

    pub fn preflight_pdfx(
        &self,
    ) -> Result<crate::workbench_optimize::PdfxPreflightReport, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.preflight_pdfx()?)
    }

    pub fn preflight_print(
        &self,
    ) -> Result<crate::workbench_optimize::PrintPreflightReport, RenderPipelineError> {
        let guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        Ok(guard.preflight_print()?)
    }

    /// Crate-private mutation hook invoked **exclusively** by `TransactionManager` (`ROADMAP.md` M0.4 Gate:
    /// *"Mutation never bypasses TransactionManager"*).
    pub(crate) fn apply_engine_mutation_for_transaction_manager<F, R>(
        &self,
        f: F,
    ) -> Result<R, RenderPipelineError>
    where
        F: FnOnce(&mut dyn PdfEngineDocument) -> Result<R, PdfEngineError>,
    {
        let mut guard = self
            .engine_document
            .lock()
            .map_err(|_| RenderPipelineError::ActorPoisoned)?;
        let out = f(guard.as_mut())?;
        Ok(out)
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

/// Coordinates `DocumentActor`, `JobManager`, adaptive `TileCache`, and separate `ThumbnailCache` (`ROADMAP.md` M0.2–M0.3).
#[derive(Debug)]
pub struct RenderPipeline {
    tile_cache: Mutex<TileCache>,
    thumbnail_cache: Mutex<ThumbnailCache>,
    prefetch_radius: u32,
}

impl RenderPipeline {
    pub fn new(tile_cache_max_bytes: usize, prefetch_radius: u32) -> Self {
        Self::with_thumbnail_budget(tile_cache_max_bytes, 48 * 1024 * 1024, prefetch_radius)
    }

    pub fn with_thumbnail_budget(
        tile_cache_max_bytes: usize,
        thumbnail_cache_max_bytes: usize,
        prefetch_radius: u32,
    ) -> Self {
        Self {
            tile_cache: Mutex::new(TileCache::new(tile_cache_max_bytes)),
            thumbnail_cache: Mutex::new(ThumbnailCache::new(thumbnail_cache_max_bytes)),
            prefetch_radius,
        }
    }

    pub fn prefetch_radius(&self) -> u32 {
        self.prefetch_radius
    }

    /// Drops the cached tiles and thumbnails of one document.
    ///
    /// After a redaction the earlier revision must not stay in the memory of
    /// the process: the cached pictures still show the removed content. The
    /// cache is disposable — a failure to drop it does not change the
    /// document — and the revision is part of the key of every tile, so the
    /// tiles of the earlier revision are never shown again.
    pub fn purge_document_cache(&self, document_id: &str) {
        if let Ok(mut cache) = self.tile_cache.lock() {
            cache.purge_document(document_id);
        }
        if let Ok(mut cache) = self.thumbnail_cache.lock() {
            cache.purge_document(document_id);
        }
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

    /// Renders (or serves from the separate `ThumbnailCache`) a low-resolution page thumbnail
    /// fitted within `max_dimension_px` (`PAGEVIEW-001`, `PAGEVIEW-002`, `ROADMAP.md` M0.3).
    #[allow(clippy::too_many_arguments)]
    pub fn render_page_thumbnail(
        &self,
        actor: &DocumentActor,
        job_manager: &JobManager,
        session_id: &str,
        document_id: &str,
        revision: u64,
        page_index: u32,
        logical_label: String,
        max_dimension_px: u32,
    ) -> Result<RenderedThumbnail, RenderPipelineError> {
        let clamped_max = max_dimension_px.clamp(32, 512);
        let thumb_key = ThumbnailKey {
            document_id: document_id.to_owned(),
            revision,
            page_index,
            max_dimension_px: clamped_max,
        };

        // 1. Check separate ThumbnailCache first:
        {
            let mut thumb_guard = self
                .thumbnail_cache
                .lock()
                .map_err(|_| RenderPipelineError::ThumbnailCachePoisoned)?;
            if let Some(hit) = thumb_guard.get(&thumb_key) {
                return Ok(hit);
            }
        }

        // 2. Schedule thumbnail job at JobPriority::Thumbnails (5):
        let job = job_manager
            .enqueue_job(
                session_id,
                revision,
                JobKind::RenderThumbnail,
                JobPriority::Thumbnails,
                false,
            )
            .map_err(|err| RenderPipelineError::JobFailure {
                details: err.to_string(),
            })?;
        let _ = job_manager.mark_running(job.job_id);

        // 3. Acquire display list and compute proportional thumbnail scale:
        let display_list = actor.acquire_display_list(revision, page_index)?;
        let geom = display_list.page_geometry();
        let max_milli_pt = geom.width_milli_pt.max(geom.height_milli_pt).max(1_000);
        let scale_milli = ((u64::from(clamped_max) * 1_000_000 + u64::from(max_milli_pt) / 2)
            / u64::from(max_milli_pt))
        .clamp(20, 2000) as u32;
        let width_px = ((u64::from(geom.width_milli_pt) * u64::from(scale_milli) + 500_000)
            / 1_000_000)
            .max(1) as u32;
        let height_px = ((u64::from(geom.height_milli_pt) * u64::from(scale_milli) + 500_000)
            / 1_000_000)
            .max(1) as u32;

        let bitmap = display_list.render_tile(PdfTileRenderRequest {
            page_index,
            scale_milli,
            tile_x_px: 0,
            tile_y_px: 0,
            tile_width_px: width_px,
            tile_height_px: height_px,
        })?;

        let thumbnail = RenderedThumbnail {
            key: thumb_key,
            physical_page_number: page_index + 1,
            logical_label,
            width_px: bitmap.width_px,
            height_px: bitmap.height_px,
            stride_bytes: bitmap.stride_bytes,
            rgba_bytes: bitmap.rgba_bytes,
        };

        {
            let mut thumb_guard = self
                .thumbnail_cache
                .lock()
                .map_err(|_| RenderPipelineError::ThumbnailCachePoisoned)?;
            thumb_guard.insert(thumbnail.clone());
        }
        let _ = job_manager.mark_completed(job.job_id);

        Ok(thumbnail)
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

    /// Propagates OS memory pressure (`Normal`, `Warning`, `Critical`) to both `TileCache` and `ThumbnailCache` (`ARCHITECTURE.md` §9).
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
        {
            let mut thumb_guard = self
                .thumbnail_cache
                .lock()
                .map_err(|_| RenderPipelineError::ThumbnailCachePoisoned)?;
            let _ = thumb_guard.apply_memory_pressure(level);
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

    pub fn thumbnail_cache_stats(&self) -> Result<ThumbnailCacheStats, RenderPipelineError> {
        let thumb_guard = self
            .thumbnail_cache
            .lock()
            .map_err(|_| RenderPipelineError::ThumbnailCachePoisoned)?;
        Ok(thumb_guard.stats())
    }
}

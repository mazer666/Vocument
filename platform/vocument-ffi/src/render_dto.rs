//! UniFFI rendering DTOs for M0.2 (`ARCHITECTURE.md` §§7–10, `ROADMAP.md` M0.2).

use vocument_core::pdf_engine::{PdfPageGeometry, PdfTileRenderRequest};
use vocument_core::render::ViewportRenderResult;
use vocument_core::tile_cache::{MemoryPressureLevel, RenderedTile, TileCacheStats};

/// Page geometry DTO returned across UniFFI (`ROADMAP.md` M0.2).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfPageGeometry {
    pub page_index: u32,
    pub width_milli_pt: u32,
    pub height_milli_pt: u32,
}

impl From<PdfPageGeometry> for FfiPdfPageGeometry {
    fn from(geo: PdfPageGeometry) -> Self {
        Self {
            page_index: geo.page_index,
            width_milli_pt: geo.width_milli_pt,
            height_milli_pt: geo.height_milli_pt,
        }
    }
}

/// Request parameters for rendering a page or tile at a quantized scale (`scale_milli = 1000` is 1.0x).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTileRenderRequest {
    pub page_index: u32,
    pub scale_milli: u32,
    pub tile_x_px: u32,
    pub tile_y_px: u32,
    pub tile_width_px: u32,
    pub tile_height_px: u32,
}

impl From<FfiTileRenderRequest> for PdfTileRenderRequest {
    fn from(req: FfiTileRenderRequest) -> Self {
        Self {
            page_index: req.page_index,
            scale_milli: req.scale_milli,
            tile_x_px: req.tile_x_px,
            tile_y_px: req.tile_y_px,
            tile_width_px: req.tile_width_px,
            tile_height_px: req.tile_height_px,
        }
    }
}

/// Immutable rendered RGBA8 tile bitmap DTO returned to Swift and Kotlin (`ARCHITECTURE.md` §10).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRenderedTile {
    pub document_id: String,
    pub revision: u64,
    pub page_index: u32,
    pub scale_milli: u32,
    pub tile_x_px: u32,
    pub tile_y_px: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    pub rgba_bytes: Vec<u8>,
}

impl From<RenderedTile> for FfiRenderedTile {
    fn from(tile: RenderedTile) -> Self {
        Self {
            document_id: tile.key.document_id,
            revision: tile.key.revision,
            page_index: tile.key.page_index,
            scale_milli: tile.key.scale_milli,
            tile_x_px: tile.key.tile_x_px,
            tile_y_px: tile.key.tile_y_px,
            width_px: tile.width_px,
            height_px: tile.height_px,
            stride_bytes: tile.stride_bytes,
            rgba_bytes: tile.rgba_bytes,
        }
    }
}

/// Platform memory pressure signal forwarded from macOS or Android into `DocumentCore` (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiMemoryPressureLevel {
    Normal,
    Warning,
    Critical,
}

impl From<FfiMemoryPressureLevel> for MemoryPressureLevel {
    fn from(level: FfiMemoryPressureLevel) -> Self {
        match level {
            FfiMemoryPressureLevel::Normal => Self::Normal,
            FfiMemoryPressureLevel::Warning => Self::Warning,
            FfiMemoryPressureLevel::Critical => Self::Critical,
        }
    }
}

impl From<MemoryPressureLevel> for FfiMemoryPressureLevel {
    fn from(level: MemoryPressureLevel) -> Self {
        match level {
            MemoryPressureLevel::Normal => Self::Normal,
            MemoryPressureLevel::Warning => Self::Warning,
            MemoryPressureLevel::Critical => Self::Critical,
        }
    }
}

/// Telemetry snapshot of the shared adaptive `TileCache` (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTileCacheStats {
    pub configured_max_bytes: u64,
    pub effective_max_bytes: u64,
    pub current_used_bytes: u64,
    pub cached_tile_count: u32,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub pressure_level: FfiMemoryPressureLevel,
}

impl From<TileCacheStats> for FfiTileCacheStats {
    fn from(stats: TileCacheStats) -> Self {
        Self {
            configured_max_bytes: stats.configured_max_bytes as u64,
            effective_max_bytes: stats.effective_max_bytes as u64,
            current_used_bytes: stats.current_used_bytes as u64,
            cached_tile_count: stats.cached_tile_count as u32,
            hit_count: stats.hit_count,
            miss_count: stats.miss_count,
            eviction_count: stats.eviction_count,
            pressure_level: stats.pressure_level.into(),
        }
    }
}

/// Combined result of rendering a visible viewport tile and prefetching neighbouring pages (`ROADMAP.md` M0.2).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiViewportRenderResult {
    pub visible_tile: FfiRenderedTile,
    pub prefetched_pages: Vec<u32>,
    pub cache_stats: FfiTileCacheStats,
}

impl From<ViewportRenderResult> for FfiViewportRenderResult {
    fn from(res: ViewportRenderResult) -> Self {
        Self {
            visible_tile: res.visible_tile.into(),
            prefetched_pages: res.prefetched_pages,
            cache_stats: res.cache_stats.into(),
        }
    }
}

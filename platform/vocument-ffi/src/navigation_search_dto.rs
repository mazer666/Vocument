//! UniFFI DTOs for M0.3 Navigation, Thumbnails, Text Selection, and Progressive SQLite FTS5 Search (`ROADMAP.md` M0.3).

use crate::render_dto::FfiMemoryPressureLevel;
use vocument_core::navigation::{PageDescriptor, PageLabelRange, PageLabelStyle};
use vocument_core::pdf_engine::PdfRectMilliPt;
use vocument_core::search_index::{SearchHit, SearchIndexProgress, SearchResponse};
use vocument_core::text_selection::TextSelectionResult;
use vocument_core::thumbnail_cache::{RenderedThumbnail, ThumbnailCacheStats};

/// Numbering style for a logical page-label range (`PAGELABEL-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiPageLabelStyle {
    DecimalArabic,
    RomanLower,
    RomanUpper,
    AlphabeticLower,
    AlphabeticUpper,
    PrefixOnly,
}

impl From<FfiPageLabelStyle> for PageLabelStyle {
    fn from(style: FfiPageLabelStyle) -> Self {
        match style {
            FfiPageLabelStyle::DecimalArabic => Self::DecimalArabic,
            FfiPageLabelStyle::RomanLower => Self::RomanLower,
            FfiPageLabelStyle::RomanUpper => Self::RomanUpper,
            FfiPageLabelStyle::AlphabeticLower => Self::AlphabeticLower,
            FfiPageLabelStyle::AlphabeticUpper => Self::AlphabeticUpper,
            FfiPageLabelStyle::PrefixOnly => Self::PrefixOnly,
        }
    }
}

/// Rule defining how logical page labels are formatted starting at `start_page_index` (`PAGELABEL-004`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPageLabelRange {
    pub start_page_index: u32,
    pub style: FfiPageLabelStyle,
    pub prefix: String,
    pub start_number: u32,
}

impl From<FfiPageLabelRange> for PageLabelRange {
    fn from(range: FfiPageLabelRange) -> Self {
        Self {
            start_page_index: range.start_page_index,
            style: range.style.into(),
            prefix: range.prefix,
            start_number: range.start_number,
        }
    }
}

/// Combined physical and logical identity of a page (`PAGELABEL-002`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPageDescriptor {
    pub page_index: u32,
    pub physical_page_number: u32,
    pub logical_label: String,
}

impl From<PageDescriptor> for FfiPageDescriptor {
    fn from(desc: PageDescriptor) -> Self {
        Self {
            page_index: desc.page_index,
            physical_page_number: desc.physical_page_number,
            logical_label: desc.logical_label,
        }
    }
}

/// Axis-aligned rectangle on a PDF page in milli-points (`1/1000` pt).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfRectMilliPt {
    pub x0_milli_pt: i32,
    pub y0_milli_pt: i32,
    pub x1_milli_pt: i32,
    pub y1_milli_pt: i32,
}

impl From<PdfRectMilliPt> for FfiPdfRectMilliPt {
    fn from(r: PdfRectMilliPt) -> Self {
        Self {
            x0_milli_pt: r.x0_milli_pt,
            y0_milli_pt: r.y0_milli_pt,
            x1_milli_pt: r.x1_milli_pt,
            y1_milli_pt: r.y1_milli_pt,
        }
    }
}

impl From<FfiPdfRectMilliPt> for PdfRectMilliPt {
    fn from(r: FfiPdfRectMilliPt) -> Self {
        Self {
            x0_milli_pt: r.x0_milli_pt,
            y0_milli_pt: r.y0_milli_pt,
            x1_milli_pt: r.x1_milli_pt,
            y1_milli_pt: r.y1_milli_pt,
        }
    }
}

/// Rendered RGBA8 page thumbnail DTO (`PAGEVIEW-001`, `PAGEVIEW-002`, `ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRenderedThumbnail {
    pub document_id: String,
    pub revision: u64,
    pub page_index: u32,
    pub physical_page_number: u32,
    pub logical_label: String,
    pub max_dimension_px: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    pub rgba_bytes: Vec<u8>,
}

impl From<RenderedThumbnail> for FfiRenderedThumbnail {
    fn from(thumb: RenderedThumbnail) -> Self {
        Self {
            document_id: thumb.key.document_id,
            revision: thumb.key.revision,
            page_index: thumb.key.page_index,
            physical_page_number: thumb.physical_page_number,
            logical_label: thumb.logical_label,
            max_dimension_px: thumb.key.max_dimension_px,
            width_px: thumb.width_px,
            height_px: thumb.height_px,
            stride_bytes: thumb.stride_bytes,
            rgba_bytes: thumb.rgba_bytes,
        }
    }
}

/// Telemetry snapshot of the separate `ThumbnailCache` (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiThumbnailCacheStats {
    pub configured_max_bytes: u64,
    pub effective_max_bytes: u64,
    pub current_used_bytes: u64,
    pub cached_thumbnail_count: u32,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub pressure_level: FfiMemoryPressureLevel,
}

impl From<ThumbnailCacheStats> for FfiThumbnailCacheStats {
    fn from(stats: ThumbnailCacheStats) -> Self {
        Self {
            configured_max_bytes: stats.configured_max_bytes as u64,
            effective_max_bytes: stats.effective_max_bytes as u64,
            current_used_bytes: stats.current_used_bytes as u64,
            cached_thumbnail_count: stats.cached_thumbnail_count as u32,
            hit_count: stats.hit_count,
            miss_count: stats.miss_count,
            eviction_count: stats.eviction_count,
            pressure_level: stats.pressure_level.into(),
        }
    }
}

/// Text selection result containing both `raw_text` (`OCRCOPY-001`) and `cleaned_text` (`OCRCOPY-002`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTextSelectionResult {
    pub page_index: u32,
    pub raw_text: String,
    pub cleaned_text: String,
    pub highlight_rects: Vec<FfiPdfRectMilliPt>,
}

impl From<TextSelectionResult> for FfiTextSelectionResult {
    fn from(sel: TextSelectionResult) -> Self {
        Self {
            page_index: sel.page_index,
            raw_text: sel.raw_text,
            cleaned_text: sel.cleaned_text,
            highlight_rects: sel.highlight_rects.into_iter().map(Into::into).collect(),
        }
    }
}

/// Progress snapshot of the progressive SQLite FTS5 index (`SEARCH-009`, `ROADMAP.md` M0.3 Gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiSearchIndexProgress {
    pub indexed_pages: u32,
    pub total_pages: u32,
    pub is_complete: bool,
}

impl From<SearchIndexProgress> for FfiSearchIndexProgress {
    fn from(p: SearchIndexProgress) -> Self {
        Self {
            indexed_pages: p.indexed_pages,
            total_pages: p.total_pages,
            is_complete: p.is_complete,
        }
    }
}

/// A single page-level match returned by classic exact FTS5 search (`SEARCH-002`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSearchHit {
    pub page_index: u32,
    pub physical_page_number: u32,
    pub logical_page_label: String,
    pub snippet: String,
    pub match_count: u32,
    pub highlight_rects: Vec<FfiPdfRectMilliPt>,
}

impl From<SearchHit> for FfiSearchHit {
    fn from(hit: SearchHit) -> Self {
        Self {
            page_index: hit.page_index,
            physical_page_number: hit.physical_page_number,
            logical_page_label: hit.logical_page_label,
            snippet: hit.snippet,
            match_count: hit.match_count,
            highlight_rects: hit.highlight_rects.into_iter().map(Into::into).collect(),
        }
    }
}

/// Complete response from an exact search query, including current progressive indexing status (`ROADMAP.md` M0.3 Gate).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSearchResponse {
    pub query: String,
    pub hits: Vec<FfiSearchHit>,
    pub progress: FfiSearchIndexProgress,
}

impl From<SearchResponse> for FfiSearchResponse {
    fn from(res: SearchResponse) -> Self {
        Self {
            query: res.query,
            hits: res.hits.into_iter().map(Into::into).collect(),
            progress: res.progress.into(),
        }
    }
}

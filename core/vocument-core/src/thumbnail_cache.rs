//! Separate bounded LRU `ThumbnailCache` (`PAGEVIEW-001`, `PAGEVIEW-002`, `ARCHITECTURE.md` §§9–10, `ROADMAP.md` M0.3).
//!
//! Architecture rules (`ARCHITECTURE.md` §§9–10):
//! - Separate thumbnail cache from full-resolution `TileCache`.
//! - Disposable Cache (may be evicted or purged at any time).
//! - Obeys platform thumbnail byte budget (`config/performance-budgets.toml`) and memory pressure signals.

use crate::tile_cache::MemoryPressureLevel;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Unique key identifying a rendered page thumbnail (`ROADMAP.md` M0.3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThumbnailKey {
    pub document_id: String,
    pub revision: u64,
    pub page_index: u32,
    pub max_dimension_px: u32,
}

/// Rendered RGBA8 page thumbnail with physical and logical page identity (`PAGEVIEW-001`, `PAGEVIEW-002`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedThumbnail {
    pub key: ThumbnailKey,
    pub physical_page_number: u32,
    pub logical_label: String,
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    pub rgba_bytes: Vec<u8>,
}

impl RenderedThumbnail {
    pub fn byte_len(&self) -> usize {
        self.rgba_bytes.len()
    }
}

/// Telemetry snapshot of the separate `ThumbnailCache` (`QUALITY.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThumbnailCacheStats {
    pub configured_max_bytes: usize,
    pub effective_max_bytes: usize,
    pub current_used_bytes: usize,
    pub cached_thumbnail_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub pressure_level: MemoryPressureLevel,
}

/// Bounded LRU cache dedicated to page thumbnails (`ARCHITECTURE.md` §9).
#[derive(Debug)]
pub struct ThumbnailCache {
    configured_max_bytes: usize,
    effective_max_bytes: usize,
    current_used_bytes: usize,
    pressure_level: MemoryPressureLevel,
    entries: HashMap<ThumbnailKey, RenderedThumbnail>,
    lru_order: VecDeque<ThumbnailKey>,
    hit_count: u64,
    miss_count: u64,
    eviction_count: u64,
}

impl ThumbnailCache {
    pub fn new(configured_max_bytes: usize) -> Self {
        let cap = configured_max_bytes.max(512 * 1024);
        Self {
            configured_max_bytes: cap,
            effective_max_bytes: cap,
            current_used_bytes: 0,
            pressure_level: MemoryPressureLevel::Normal,
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            hit_count: 0,
            miss_count: 0,
            eviction_count: 0,
        }
    }

    pub fn get(&mut self, key: &ThumbnailKey) -> Option<RenderedThumbnail> {
        if let Some(thumb) = self.entries.get(key).cloned() {
            self.hit_count += 1;
            self.touch_lru(key);
            Some(thumb)
        } else {
            self.miss_count += 1;
            None
        }
    }

    pub fn insert(&mut self, thumbnail: RenderedThumbnail) {
        let thumb_bytes = thumbnail.byte_len();
        if self.effective_max_bytes == 0 || thumb_bytes > self.effective_max_bytes {
            return;
        }

        let key = thumbnail.key.clone();
        if let Some(prev) = self.entries.remove(&key) {
            self.current_used_bytes = self.current_used_bytes.saturating_sub(prev.byte_len());
            self.remove_from_lru(&key);
        }

        while self.current_used_bytes.saturating_add(thumb_bytes) > self.effective_max_bytes {
            if !self.evict_oldest() {
                break;
            }
        }

        self.current_used_bytes = self.current_used_bytes.saturating_add(thumb_bytes);
        self.lru_order.push_back(key.clone());
        self.entries.insert(key, thumbnail);
    }

    pub fn apply_memory_pressure(&mut self, level: MemoryPressureLevel) -> ThumbnailCacheStats {
        self.pressure_level = level;
        match level {
            MemoryPressureLevel::Normal => {
                self.effective_max_bytes = self.configured_max_bytes;
            }
            MemoryPressureLevel::Warning => {
                self.effective_max_bytes = (self.configured_max_bytes / 2).max(256 * 1024);
                while self.current_used_bytes > self.effective_max_bytes {
                    if !self.evict_oldest() {
                        break;
                    }
                }
            }
            MemoryPressureLevel::Critical => {
                let evicted = self.entries.len() as u64;
                self.entries.clear();
                self.lru_order.clear();
                self.current_used_bytes = 0;
                self.eviction_count += evicted;
                self.effective_max_bytes = (self.configured_max_bytes / 4).max(256 * 1024);
            }
        }
        self.stats()
    }

    pub fn stats(&self) -> ThumbnailCacheStats {
        ThumbnailCacheStats {
            configured_max_bytes: self.configured_max_bytes,
            effective_max_bytes: self.effective_max_bytes,
            current_used_bytes: self.current_used_bytes,
            cached_thumbnail_count: self.entries.len(),
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            eviction_count: self.eviction_count,
            pressure_level: self.pressure_level,
        }
    }

    fn evict_oldest(&mut self) -> bool {
        if let Some(oldest_key) = self.lru_order.pop_front()
            && let Some(removed) = self.entries.remove(&oldest_key)
        {
            self.current_used_bytes = self.current_used_bytes.saturating_sub(removed.byte_len());
            self.eviction_count += 1;
            return true;
        }
        false
    }

    fn touch_lru(&mut self, key: &ThumbnailKey) {
        self.remove_from_lru(key);
        self.lru_order.push_back(key.clone());
    }

    fn remove_from_lru(&mut self, key: &ThumbnailKey) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
            let _ = self.lru_order.remove(pos);
        }
    }
}

//! Adaptive `TileCache` with LRU eviction and OS memory-pressure handling (`ARCHITECTURE.md` §§9–10, `ROADMAP.md` M0.2).
//!
//! Rules (`ARCHITECTURE.md` §§9–10):
//! - Cache is Disposable Cache (never document truth).
//! - Zoom-dependent resolution (`scale_milli`, e.g. `1000` = 1.0x, `2000` = 2.0x).
//! - Strictly bounded byte capacity from `config/performance-budgets.toml`.
//! - LRU eviction when inserting new tiles exceeds the active budget.
//! - Native memory-pressure signals (`Normal`, `Warning`, `Critical`) dynamically shrink or purge the cache.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Unique key identifying a rendered PDF tile in `TileCache` (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileKey {
    pub document_id: String,
    pub revision: u64,
    pub page_index: u32,
    /// Zoom factor in milli-units (`1000` = 100%, `1500` = 150%, `2000` = 200%).
    pub scale_milli: u32,
    pub tile_x_px: u32,
    pub tile_y_px: u32,
    pub tile_width_px: u32,
    pub tile_height_px: u32,
}

/// Rendered RGBA8 bitmap tile produced by a render worker (`ARCHITECTURE.md` §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedTile {
    pub key: TileKey,
    pub width_px: u32,
    pub height_px: u32,
    pub stride_bytes: u32,
    /// Unpremultiplied 8-bit per channel RGBA pixels (`width_px * height_px * 4` bytes).
    pub rgba_bytes: Vec<u8>,
}

impl RenderedTile {
    pub fn byte_len(&self) -> usize {
        self.rgba_bytes.len()
    }
}

/// OS memory-pressure level signaled by macOS (`DispatchSourceMemoryPressure`) or Android (`ComponentCallbacks2`) (`ARCHITECTURE.md` §9, `BIGMOBILE-001`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressureLevel {
    /// Normal operation: 100% of configured tile cache budget available.
    Normal,
    /// Elevated memory pressure: shrink active tile cache budget to 50% and evict LRU entries.
    Warning,
    /// Critical memory pressure: purge all disposable cached tiles immediately (budget 0 during trim).
    Critical,
}

/// Snapshot of `TileCache` telemetry for QA and performance verification (`QUALITY.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileCacheStats {
    pub configured_max_bytes: usize,
    pub effective_max_bytes: usize,
    pub current_used_bytes: usize,
    pub cached_tile_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub pressure_level: MemoryPressureLevel,
}

/// Bounded LRU tile cache (`ARCHITECTURE.md` §§9–10).
#[derive(Debug)]
pub struct TileCache {
    configured_max_bytes: usize,
    effective_max_bytes: usize,
    current_used_bytes: usize,
    pressure_level: MemoryPressureLevel,
    entries: HashMap<TileKey, RenderedTile>,
    lru_order: VecDeque<TileKey>,
    hit_count: u64,
    miss_count: u64,
    eviction_count: u64,
}

impl TileCache {
    pub fn new(configured_max_bytes: usize) -> Self {
        let max_bytes = configured_max_bytes.max(64 * 1024);
        Self {
            configured_max_bytes: max_bytes,
            effective_max_bytes: max_bytes,
            current_used_bytes: 0,
            pressure_level: MemoryPressureLevel::Normal,
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            hit_count: 0,
            miss_count: 0,
            eviction_count: 0,
        }
    }

    /// Looks up a tile in the cache and promotes it to most-recently-used on hit.
    pub fn get(&mut self, key: &TileKey) -> Option<RenderedTile> {
        if let Some(tile) = self.entries.get(key).cloned() {
            self.hit_count += 1;
            self.touch_lru(key);
            Some(tile)
        } else {
            self.miss_count += 1;
            None
        }
    }

    /// Inserts a rendered tile into the cache, evicting least-recently-used tiles if needed.
    pub fn insert(&mut self, tile: RenderedTile) {
        let tile_bytes = tile.byte_len();
        if tile_bytes == 0 || tile_bytes > self.effective_max_bytes {
            return;
        }

        let key = tile.key.clone();
        if let Some(existing) = self.entries.remove(&key) {
            self.current_used_bytes = self.current_used_bytes.saturating_sub(existing.byte_len());
            self.remove_from_lru(&key);
        }

        while self.current_used_bytes + tile_bytes > self.effective_max_bytes {
            if !self.evict_oldest() {
                break;
            }
        }

        self.current_used_bytes += tile_bytes;
        self.lru_order.push_back(key.clone());
        self.entries.insert(key, tile);
    }

    /// Removes every cached tile of one document.
    ///
    /// A redaction removes content from the document. The tiles of the
    /// earlier revision still show that content, and they stay readable in the
    /// memory of the process until the cache evicts them. After a redaction
    /// the tiles of the document are therefore dropped.
    pub fn purge_document(&mut self, document_id: &str) {
        let keys: Vec<TileKey> = self
            .entries
            .keys()
            .filter(|key| key.document_id == document_id)
            .cloned()
            .collect();
        for key in keys {
            if let Some(tile) = self.entries.remove(&key) {
                self.current_used_bytes = self.current_used_bytes.saturating_sub(tile.byte_len());
            }
            self.remove_from_lru(&key);
            self.eviction_count += 1;
        }
    }

    /// Applies an OS memory-pressure signal (`Normal`, `Warning`, `Critical`) (`ARCHITECTURE.md` §9).
    pub fn apply_memory_pressure(&mut self, level: MemoryPressureLevel) -> TileCacheStats {
        self.pressure_level = level;
        match level {
            MemoryPressureLevel::Normal => {
                self.effective_max_bytes = self.configured_max_bytes;
            }
            MemoryPressureLevel::Warning => {
                self.effective_max_bytes = self.configured_max_bytes / 2;
                while self.current_used_bytes > self.effective_max_bytes {
                    if !self.evict_oldest() {
                        break;
                    }
                }
            }
            MemoryPressureLevel::Critical => {
                let purged = self.entries.len() as u64;
                self.entries.clear();
                self.lru_order.clear();
                self.current_used_bytes = 0;
                self.eviction_count += purged;
                // Keep a 25% emergency ceiling until pressure returns to Normal
                self.effective_max_bytes = self.configured_max_bytes / 4;
            }
        }
        self.stats()
    }

    pub fn stats(&self) -> TileCacheStats {
        TileCacheStats {
            configured_max_bytes: self.configured_max_bytes,
            effective_max_bytes: self.effective_max_bytes,
            current_used_bytes: self.current_used_bytes,
            cached_tile_count: self.entries.len(),
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

    fn touch_lru(&mut self, key: &TileKey) {
        self.remove_from_lru(key);
        self.lru_order.push_back(key.clone());
    }

    fn remove_from_lru(&mut self, key: &TileKey) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
            let _ = self.lru_order.remove(pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tile(page_index: u32, byte_size: usize) -> RenderedTile {
        RenderedTile {
            key: TileKey {
                document_id: "doc_test".to_owned(),
                revision: 1,
                page_index,
                scale_milli: 1000,
                tile_x_px: 0,
                tile_y_px: 0,
                tile_width_px: 128,
                tile_height_px: 128,
            },
            width_px: 128,
            height_px: 128,
            stride_bytes: 512,
            rgba_bytes: vec![255; byte_size],
        }
    }

    #[test]
    fn enforces_lru_eviction_and_memory_pressure_trimming() {
        let mut cache = TileCache::new(100_000);
        let t0 = sample_tile(0, 40_000);
        let t1 = sample_tile(1, 40_000);
        let t2 = sample_tile(2, 40_000);

        cache.insert(t0.clone());
        cache.insert(t1.clone());
        // Touch t0 so t1 becomes the least-recently-used
        assert!(cache.get(&t0.key).is_some());

        // Inserting t2 (40 KB) exceeds 100 KB -> must evict t1 (LRU), keeping t0 and t2
        cache.insert(t2.clone());
        assert!(cache.get(&t0.key).is_some());
        assert!(cache.get(&t1.key).is_none());
        assert!(cache.get(&t2.key).is_some());
        assert_eq!(cache.stats().eviction_count, 1);

        // Warning pressure halves budget to 50 KB -> must evict one more tile (leaving 40 KB <= 50 KB)
        let warn_stats = cache.apply_memory_pressure(MemoryPressureLevel::Warning);
        assert_eq!(warn_stats.cached_tile_count, 1);
        assert!(warn_stats.current_used_bytes <= 50_000);

        // Critical pressure purges all disposable tiles
        let crit_stats = cache.apply_memory_pressure(MemoryPressureLevel::Critical);
        assert_eq!(crit_stats.cached_tile_count, 0);
        assert_eq!(crit_stats.current_used_bytes, 0);
    }
}

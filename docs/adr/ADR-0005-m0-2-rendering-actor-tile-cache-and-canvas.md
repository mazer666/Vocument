# ADR-0005: M0.2 — DocumentActor, Display-List Render Pipeline, Adaptive TileCache, and Native DocumentCanvas

- **Status:** Accepted
- **Date:** 2026-10-17
- **Deciders:** Vocument Architecture & Core Engineering
- **Related Milestone:** `M0.2 — Rendering` (`ROADMAP.md` lines 72–95)
- **Related Documents:** `ARCHITECTURE.md` §§7–10, `REQUIREMENTS.md` (`PERF-001`, `BIG-001`, `BIGMOBILE-001`, `VIEW-001`, `VIEW-002`, `CODE-001`), `QUALITY.md` §§2–3, `config/performance-budgets.toml`

---

## 1. Context

Following `M0.1 — FileBroker + MuPDF Session`, Vocument required its production PDF rendering path (`M0.2 — Rendering`):
1. **MuPDF Concurrency Model (`ARCHITECTURE.md` §7):** MuPDF's `fz_document` and page parsing (`fz_load_page`) are not safe for arbitrary concurrent mutation across threads. However, once a page is parsed into an immutable `fz_display_list`, rasterizing rectangular tiles at different zoom factors is derived work.
2. **Job Scheduling & Responsiveness (`ARCHITECTURE.md` §8, `PERF-001`):** Visible viewport tiles must always take priority (`VisiblePageRender` = 1) over neighbouring-page prefetch (`NearbyPagePrefetch` = 2), and stale prefetch jobs must be cancellable when the viewport moves.
3. **Bounded Memory & OS Pressure (`ARCHITECTURE.md` §9, `BIG-001`, `BIGMOBILE-001`):** Rendered RGBA8 tiles are *Disposable Cache* (never document truth). Memory usage must obey `config/performance-budgets.toml` with LRU eviction and react immediately to macOS (`DispatchSourceMemoryPressure`) and Android (`ComponentCallbacks2.onTrimMemory`) pressure events.
4. **Native `DocumentCanvas` (`ARCHITECTURE.md` §10):** Both macOS (`SwiftUI` + `CGImage`) and Android (`Jetpack Compose` + `Bitmap.Config.ARGB_8888`) must render tiles from `DocumentCore` with scroll, quantized zoom (`50%`–`300%`), and neighbouring-page prefetch.

---

## 2. Decision

### 2.1 Serialized `DocumentActor` and Immutable `PdfEngineDisplayList` (`core/vocument-core/src/render.rs`)
- Each `DocumentSession` owns an `Arc<DocumentActor>` wrapping the mutable `Box<dyn PdfEngineDocument>` inside a serialized mutex.
- Page geometry queries (`page_geometry`) and display-list creation (`create_display_list`) execute exclusively inside the serialized `DocumentActor`.
- Created `Arc<dyn PdfEngineDisplayList>` instances are cached per `(revision, page_index)` on `DocumentActor`. Once acquired, the `DocumentActor` lock is released before rasterizing pixel tiles (`display_list.render_tile(request)`).
- Inside `engines/pdf-mupdf/csrc/mupdf_bridge.c`, `vocument_mupdf_create_display_list` records the page into an `fz_display_list` via `fz_new_display_list_from_page`, and `vocument_mupdf_render_tile_from_display_list` rasterizes any requested `(scale_milli, tile_x_px, tile_y_px, tile_width_px, tile_height_px)` bounding box into an unpremultiplied RGBA8 buffer (`fz_device_rgb`) under its own worker `fz_context` clone.

### 2.2 Central `JobManager` (`core/vocument-core/src/jobs.rs`)
- Implements the 7-tier `JobPriority` ordering from `ARCHITECTURE.md` §8 (`VisiblePageRender` = 1, `NearbyPagePrefetch` = 2, `ActivePageOcr` = 3, `IncrementalSearchIndex` = 4, `ThumbnailGeneration` = 5, `BackgroundDocumentOcr` = 6, `BatchOptimization` = 7).
- Viewport changes call `cancel_queued_prefetch_for_session(session_id)` before scheduling the new `VisiblePageRender` job followed by `NearbyPagePrefetch` jobs within `nearby_page_prefetch_radius` (`2` pages by default from `config/performance-budgets.toml`).

### 2.3 Adaptive `TileCache` with LRU Eviction & Memory-Pressure Handling (`core/vocument-core/src/tile_cache.rs`)
- Keyed by `TileKey { document_id, revision, page_index, scale_milli, tile_x_px, tile_y_px, tile_width_px, tile_height_px }`.
- Tracks `configured_max_bytes`, `effective_max_bytes`, `current_used_bytes`, `hit_count`, `miss_count`, and `eviction_count`.
- Evicts least-recently-used tiles automatically whenever inserting a tile would exceed `effective_max_bytes`.
- Responds to `MemoryPressureLevel`:
  - `Normal`: restores `effective_max_bytes = configured_max_bytes` (100% budget).
  - `Warning`: shrinks `effective_max_bytes` to 50% of `configured_max_bytes` and immediately evicts LRU entries down to the reduced cap.
  - `Critical`: purges all cached tiles (`current_used_bytes = 0`) and clears cached display lists on active `DocumentActor` instances.

### 2.4 Native `DocumentCanvas` on macOS and Android
- **macOS (`apps/macos`):** `DocumentCoreClient.makeCGImage(from:)` wraps RGBA8 tile bytes into a native `CGImage`, displayed inside `DocumentCanvasView.swift` with page navigation, quantized zoom (`50%`–`300%`), prefetch telemetry, and automatic `DispatchSource.makeMemoryPressureSource` forwarding.
- **Android (`apps/android`):** `DocumentCoreBridge.decodeRgba8ToArgbPixels` + `DocumentCanvas.kt` renders `Bitmap.Config.ARGB_8888` inside a scrollable/zoomable Jetpack Compose viewport (`detectTransformGestures` + zoom/page controls), while `MainActivity.onTrimMemory` forwards `ComponentCallbacks2` signals to `DocumentCore`.

---

## 3. Consequences

- **Positive:**
  - First-page open + display-list + RGBA8 tile render completes in ~**51 ms** on macOS and ~**17 ms** on Kotlin/JVM (well under the `300–500 ms` target in `QUALITY.md` §2).
  - Prefetched neighbour pages serve in `< 0.1 ms` directly from `TileCache`.
  - No `fz_*` pointer ever leaves `engines/pdf-mupdf`, and all hand-written source files remain $\le 495$ lines (`CODE-001` target $\le 600$, hard max $900$).
- **Next Step:** Proceed to **M0.3 — Navigation, Search, Selection** (Outline/TOC, Thumbnails, Page Go-To, Text extraction, Text selection + Copy, SQLite FTS5 index, and Search UI).

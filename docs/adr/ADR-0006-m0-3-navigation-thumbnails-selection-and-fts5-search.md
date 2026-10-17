# ADR-0006: M0.3 Navigation, Logical Page Labels, Separate ThumbnailCache, Cleaned Text Selection, and Progressive SQLite FTS5 Search

- **Status:** Accepted
- **Date:** 2026-10-17
- **Milestone:** M0.3 — Navigation & Classic Search
- **Authors:** Vocument Architecture & Core Engineering

---

## 1. Context

Milestone **M0.3** (`ROADMAP.md` lines 98–114) extends the M0.2 rendering pipeline with four foundational reader capabilities required by `ARCHITECTURE.md` §§8–10, §17 and `REQUIREMENTS.md` §§4–6, §9:

1. **Page Thumbnails (`PAGEVIEW-001`, `PAGEVIEW-002`, `ARCHITECTURE.md` §§8–9):** Low-resolution page thumbnails must be scheduled at `JobPriority::Thumbnails` (`5`) and cached in a separate bounded `ThumbnailCache` so thumbnail browsing never evicts high-resolution viewport tiles in `TileCache`.
2. **Logical Page Labels & Page Navigation (`PAGELABEL-001`..`PAGELABEL-004`):** Documents frequently use Roman numerals (`i, ii`, `I, II`), alphabetic (`a, b`, `A, B`), decimal Arabic (`1, 2`), or prefixed (`A-1`, `Cover`) logical page numbers alongside physical `1..=N` indices. Both must be exposed simultaneously, navigable via jump queries (`"ii"`, `"1"`, `"#3"`), and editable via `PageLabelRange` rules.
3. **Structured Text Selection & Cleaned Plain-Text Copy (`OCRCOPY-001`, `OCRCOPY-002`):** Extracting text from PDFs often produces broken lines and soft word-wrap hyphens (`"trust-\nworthy"`). Users need both raw text copy (`OCRCOPY-001`) and cleaned plain-text copy (`OCRCOPY-002`) that rejoins hyphenated words across line wraps while preserving real paragraph breaks (`\n\n`).
4. **Progressive Local SQLite FTS5 Index & Classic Exact Search (`SEARCH-001`, `SEARCH-002`, `SEARCH-009`, M0.3 Gate):** Full-text search must use SQLite FTS5 (`ARCHITECTURE.md` §17), index pages progressively starting around the user's current page (`JobPriority::SearchIndexCurrentDoc`), and **return useful results before a document is fully indexed** within the `< 150 ms` quality budget (`config/performance-budgets.toml`).

Additionally, all source files must adhere to **`CODE-001` (`CR-0001`)**: target $\le 600$ lines, hard maximum $900$ lines per hand-written file.

---

## 2. Decision

### 2.1 MuPDF Structured Text & Embedded `/PageLabels` Bridge (`engines/pdf-mupdf`)
- To keep C and Rust files modular and well below the 600-line `CODE-001` target, we added dedicated modules:
  - `engines/pdf-mupdf/csrc/mupdf_text_bridge.c`
  - `engines/pdf-mupdf/src/text_extract.rs`
- `vocument_mupdf_query_page_label` queries MuPDF's `pdf_page_label` inside an `fz_try`/`fz_catch` guard.
- `vocument_mupdf_extract_page_text` builds an `fz_stext_page` (`fz_new_stext_page_from_page_number`), iterates `fz_stext_block` and `fz_stext_line` entries, and returns UTF-8 text lines together with their bounding boxes (`PdfRectMilliPt`) in milli-points (`1/1000` pt).

### 2.2 Logical Page Labels & Navigation (`core/vocument-core/src/navigation.rs`)
- Implemented `PageLabelStyle` (`DecimalArabic`, `RomanLower`, `RomanUpper`, `AlphabeticLower`, `AlphabeticUpper`, `PrefixOnly`), `PageLabelRange`, `PageDescriptor` (`page_index`, `physical_page_number`, `logical_label`), and `PageLabelCatalog`.
- `PageLabelCatalog` initializes from embedded PDF `/PageLabels` (falling back to `"1"..="N"`), resolves user navigation queries (`"#3"` for explicit physical page, `"ii"` or `"1"` for logical label, or numeric fallback), and supports replacing label ranges via `apply_ranges` (`PAGELABEL-004`).

### 2.3 Separate Bounded `ThumbnailCache` (`core/vocument-core/src/thumbnail_cache.rs`)
- Implemented `ThumbnailCache` as a dedicated LRU cache separate from `TileCache`, bounded by `default_thumbnail_cache_mb` (`config/performance-budgets.toml`).
- Responds to `MemoryPressureLevel::Warning` (trims to 50% capacity and evicts LRU entries) and `MemoryPressureLevel::Critical` (purges all cached thumbnails).
- Thumbnails are rendered through `RenderPipeline::render_page_thumbnail` at `JobPriority::Thumbnails` (`5`).

### 2.4 Structured Text Selection & `OCRCOPY-002` Cleaning (`core/vocument-core/src/text_selection.rs`)
- Implemented full-page (`select_all_page_text`) and rectangular (`select_page_text_in_rect`) selection over `PdfPageStructuredText`.
- Produces both `raw_text` (`OCRCOPY-001`) and `cleaned_text` (`OCRCOPY-002`), which:
  - detects paragraph boundaries via vertical line spacing gaps (`> 1.45x` line height),
  - rejoins soft word-wrap hyphens (`"trust-\nworthy"` $\rightarrow$ `"trustworthy"`) when the continuation begins with a lowercase letter,
  - replaces intra-paragraph artificial line breaks with spaces while preserving `\n\n` between paragraphs.

### 2.5 Progressive SQLite FTS5 Search (`core/vocument-core/src/search_index.rs`)
- Added `rusqlite = "0.32"` linking against the platform SQLite3 library with FTS5 enabled.
- `SessionSearchIndex` creates a SQLite `fts5` virtual table (`CREATE VIRTUAL TABLE page_fts USING fts5(cleaned_text, raw_text, tokenize = 'unicode61 remove_diacritics 2')`) alongside a `pages_meta` table storing structured lines and bounding boxes.
- `next_unindexed_pages(preferred_start_page, max_pages)` selects unindexed pages in outward distance order from the active viewport page (`SEARCH-009`).
- `search_exact` queries `page_fts` immediately even when `indexed_pages < total_pages` (satisfying the **M0.3 Gate**), returning `SearchResponse` with `SearchIndexProgress`, page-level `SearchHit`s, snippets, match counts, and highlight bounding boxes (`PdfRectMilliPt`).

### 2.6 Modular UniFFI Surface (`platform/vocument-ffi`)
- Split FFI DTOs into `session_dto.rs`, `render_dto.rs`, and `navigation_search_dto.rs` so `platform/vocument-ffi/src/lib.rs` remains ~466 lines (`CODE-001`).
- Exposed M0.3 functions to Swift (`apps/macos`) and Kotlin (`apps/android`).

---

## 3. Consequences

- **Positive:**
  - Classic exact search via SQLite FTS5 executes in **< 1 ms** (~0.06 ms on macOS, ~0.82 ms on Kotlin/Android), well under the 150 ms target budget.
  - Progressive batch indexing allows immediate search results on partially indexed documents (M0.3 Gate).
  - Every hand-written file across Rust, C, Swift, Kotlin, and Shell remains $\le 525$ lines (`CODE-001` compliant).
- **Follow-up (M0.4):**
  - Introduce `TransactionManager`, semantic mutation commands (highlight annotation, page rotation), and persistent Undo/Redo history without bypassing `DocumentSession`.

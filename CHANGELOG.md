# Changelog

All notable changes to Vocument are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-m0.4] - 2026-10-17

### Added
- Completed **M0.4 — First mutations** (`docs/acceptance/M0.4-ACCEPTANCE.md`, `ADR-0007`).
- Implemented MuPDF 1.28.5 standard PDF `/Highlight` annotations (`PDF_ANNOT_HIGHLIGHT`, `/QuadPoints`, `/C`, `/CA`, `/NM`, `/T`, `/Contents`) and `/Rotate` page rotation (`+90°`, `-90°`, `180°`) with in-memory PDF rewrite (`engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c`, `engines/pdf-mupdf/src/mutate.rs`).
- Implemented `TransactionManager`, semantic `TransactionCommand` (`RotatePages`, `AddHighlightAnnotation`, `RemoveHighlightAnnotation`) with human-readable descriptions (`UNDO-004`), Undo/Redo stack (`UNDO-001`, `UNDO-002`), and SHA-256 checksum-verified `PersistentTransactionJournal` v1 with deterministic session replay (`core/vocument-core/src/transactions.rs`, `core/vocument-core/src/session_mutations.rs`).
- Enforced the **M0.4 Gate** (`Mutation never bypasses TransactionManager`) via `pub(crate)` visibility on `DocumentActor::apply_engine_mutation_for_transaction_manager` and automated CI verification in `scripts/validate-repo.sh`.
- Exposed M0.4 mutation, Undo/Redo, and transaction journal APIs over UniFFI (`platform/vocument-ffi/src/mutation_dto.rs`) and integrated native controls on macOS (`DocumentCoreClient.swift`, `DocumentCanvasView.swift`, `NavigationAndSearchSidebarView.swift`) and Android (`DocumentCoreBridge.kt`, `MutationToolsPanel.kt`, `MainActivity.kt`).

## [0.1.0-m0.3] - 2026-10-17

### Added
- Completed **M0.3 — Navigation & Classic Search** (`docs/acceptance/M0.3-ACCEPTANCE.md`, `ADR-0006`).
- Implemented MuPDF structured text and embedded `/PageLabels` extraction (`engines/pdf-mupdf/csrc/mupdf_text_bridge.c`, `engines/pdf-mupdf/src/text_extract.rs`).
- Implemented logical page labels (`PAGELABEL-001`..`004`), custom label range overrides, and logical/physical page navigation (`core/vocument-core/src/navigation.rs`).
- Implemented a separate bounded LRU `ThumbnailCache` (`core/vocument-core/src/thumbnail_cache.rs`) and thumbnail rendering at `JobPriority::Thumbnails` (`PAGEVIEW-001`, `PAGEVIEW-002`).
- Implemented full-page and rectangular text selection with both raw copy (`OCRCOPY-001`) and dehyphenated, paragraph-preserving cleaned plain-text copy (`OCRCOPY-002`, `core/vocument-core/src/text_selection.rs`).
- Implemented progressive local SQLite FTS5 indexing and classic exact search (`core/vocument-core/src/search_index.rs`, `SEARCH-001`, `SEARCH-002`, `SEARCH-009`), returning results in `< 1 ms` even before full document indexing completes (M0.3 Gate).
- Exposed M0.3 navigation, thumbnail, text selection, and search APIs over UniFFI (`platform/vocument-ffi`) and integrated native sidebar/panel views on macOS (`NavigationAndSearchSidebarView.swift`) and Android (`NavigationAndSearchPanel.kt`).

## [0.1.0-m0.2] - 2026-10-17

### Added
- Completed **M0.2 — Rendering** (`docs/acceptance/M0.2-ACCEPTANCE.md`, `ADR-0005`).
- Implemented central `JobManager` (`core/vocument-core/src/jobs.rs`) with 7-tier `JobPriority` (`VisiblePageRender` ahead of `NearbyPagePrefetch`) and stale-prefetch cancellation.
- Implemented adaptive `TileCache` (`core/vocument-core/src/tile_cache.rs`) with bounded byte capacity from `config/performance-budgets.toml`, LRU eviction, and OS memory-pressure handling (`Normal`, `Warning`, `Critical`).
- Implemented serialized `DocumentActor` and `RenderPipeline` (`core/vocument-core/src/render.rs`) caching immutable `PdfEngineDisplayList` handles per `(revision, page_index)` and rasterizing RGBA8 tiles via MuPDF 1.28.5 (`engines/pdf-mupdf/csrc/mupdf_bridge.c`).
- Exposed M0.2 rendering APIs over UniFFI (`platform/vocument-ffi`) and integrated native `DocumentCanvas` views on macOS (`apps/macos/Sources/VocumentMacApp/DocumentCanvasView.swift`) and Android (`apps/android/app/src/main/java/org/vocument/app/DocumentCanvas.kt`) with scroll, zoom (`50%`–`300%`), neighbouring-page prefetch, and system memory-pressure hooks.

## [0.1.0-m0.1] - 2026-10-17

### Added
- Completed **M0.1 — FileBroker + MuPDF session** (`docs/acceptance/M0.1-ACCEPTANCE.md`, `ADR-0004`).
- Added `CODE-001` requirement (`docs/cr/CR-0001-source-file-size-limits.md`): hand-written source files target $\le 600$ lines with a hard maximum of $900$ lines, enforced in `config/performance-budgets.toml` and `scripts/validate-repo.sh`.
- Implemented `FileIdentity` (SHA-256, size, mtime, OS inode/SAF document ID, external-modification comparison), `DocumentId`, and `SessionId` in `core/vocument-core/src/identity.rs`.
- Implemented `FileBroker` and `ScopedLocalFileBroker` in `core/vocument-core/src/file_broker.rs`.
- Implemented `PdfEngine` trait (`core/vocument-core/src/pdf_engine.rs`) and the production **MuPDF 1.28.5** adapter (`engines/pdf-mupdf`) with `fz_try`/`fz_catch` C bridge reading real PDF page counts and Info metadata.
- Implemented `DocumentSession` and `SessionRegistry` in `core/vocument-core/src/session.rs` and exposed session lifecycle functions over UniFFI (`platform/vocument-ffi`) to both macOS (`apps/macos`) and Android (`apps/android`).

## [0.1.0-m0.0] - 2026-10-17

### Added
- Completed **M0.0 — Repository & Build Skeleton** (`docs/acceptance/M0.0-ACCEPTANCE.md`).
- Real **UniFFI** proc-macro boundary in `platform/vocument-ffi` (`DocumentCoreHandshake`, `DocumentCoreError`, `document_core_handshake()`) and reproducible binding generator `scripts/generate-bindings.sh` (`ADR-0003`).
- Native **macOS 15+** Swift package (`apps/macos`) with `VocumentFFI` C/Swift targets, `VocumentMacKit` (`DocumentCoreClient` calling UniFFI directly with zero hardcoded defaults), `VocumentMacApp` (SwiftUI/AppKit shell), and `vocument-mac-handshake` CLI verifier.
- Native **Android 13+ (`minSdk = 33`, `arm64-v8a`)** project (`apps/android`) with Jetpack Compose (`MainActivity.kt`), `DocumentCoreBridge.kt` calling generated UniFFI bindings, and headless Kotlin/JNA verification (`scripts/verify-kotlin-handshake.sh`).
- Official AGPLv3 `LICENSE`, `REUSE.toml`, `rust-toolchain.toml`, `deny.toml`, Git hooks (`scripts/install-hooks.sh`), and immutable Baseline 0.1 snapshot (`archive/baselines/0.1/`).

## [0.1] - 2026-10-17

### Added
- Initial architecture baseline (`ARCHITECTURE.md`), product requirements (`REQUIREMENTS.md`), roadmap (`ROADMAP.md`), quality model (`QUALITY.md`), project governance (`PROJECT_MANAGEMENT.md`, `AGENTS.md`), and central configuration structure (`config/`).
- Repository directory layout for M0 ("Foundation / Trustworthy Viewer").

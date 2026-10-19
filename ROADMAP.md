# Vocument Roadmap

**Roadmap version:** 0.1  
**Current milestone:** M0 — Foundation / Trustworthy Viewer

The roadmap is milestone-driven. A milestone is not complete because “the UI seems to work”; it is complete only when its functional, technical, performance, usability, security and interoperability gates are satisfied.

---

## Milestone model

Every milestone has:

- scope
- explicit non-goals
- functional acceptance criteria
- technical acceptance criteria
- performance budgets
- usability criteria
- security/privacy criteria
- interoperability criteria
- documentation criteria
- QA evidence
- release decision

See `QUALITY.md` and `docs/acceptance/ACCEPTANCE_TEMPLATE.md`.

---

# M0 — Foundation / “Trustworthy Viewer” (Status: `COMPLETE` — see [`docs/acceptance/M0-ACCEPTANCE.md`](docs/acceptance/M0-ACCEPTANCE.md))

## Purpose

Prove that the architecture works end-to-end on both macOS and Android before adding feature breadth.

## M0.0 — Repository & Build Skeleton (Status: `done` — see [`docs/acceptance/M0.0-ACCEPTANCE.md`](docs/acceptance/M0.0-ACCEPTANCE.md))

Deliver:

- Cargo workspace
- macOS app project
- Android app project
- UniFFI binding generation
- CI
- licence/DCO/REUSE setup
- central config structure
- documentation checks
- trivial Rust call successfully invoked from Swift and Kotlin

### Gate

A clean clone can be bootstrapped and built by following documented commands without tribal knowledge (`make check` / `./scripts/local-ci.sh`).

---

## M0.1 — FileBroker + MuPDF session (Status: `done` — see [`docs/acceptance/M0.1-ACCEPTANCE.md`](docs/acceptance/M0.1-ACCEPTANCE.md))

Deliver:

- open PDF from Finder/Open With on macOS
- open PDF through Storage Access Framework on Android
- create `DocumentSession`
- read page count/basic metadata
- assign stable internal session/document IDs
- file identity/fingerprint foundation

### Gate

No UI layer accesses MuPDF directly (enforced automatically in `./scripts/validate-repo.sh`).

---

## M0.2 — Rendering *(COMPLETED — see [`docs/acceptance/M0.2-ACCEPTANCE.md`](docs/acceptance/M0.2-ACCEPTANCE.md))*

Deliver:

- `DocumentActor`
- display-list/render worker pipeline
- adaptive tile cache
- native `DocumentCanvas`
- scroll
- zoom
- neighbouring-page prefetch
- memory-pressure handling

### Performance target

On reference hardware, aim for:

- normal PDF first page visible within 300–500 ms after document load path is ready *(achieved: ~51 ms on macOS, ~56 ms on Kotlin/Android)*
- 60 fps-oriented scroll/zoom behaviour *(achieved: <0.1 ms tile cache hits, <2 ms display-list sub-tile render)*
- UI remains responsive for large documents

Exact device-specific gates are refined during implementation.

---

## M0.3 — Navigation & Classic Search *(COMPLETED — see [`docs/acceptance/M0.3-ACCEPTANCE.md`](docs/acceptance/M0.3-ACCEPTANCE.md))*

Deliver:

- thumbnails
- logical page labels
- page navigation
- text selection
- cleaned plain-text copy
- FTS5 index
- progressive indexing
- classic exact search

### Gate

Search begins returning useful results before a very large document is fully indexed. *(Verified in Rust, Swift, and Kotlin tests — < 1 ms query latency on partially indexed documents.)*

---

## M0.4 — First mutations *(Status: **COMPLETE** — see [M0.4 Acceptance Record](docs/acceptance/M0.4-ACCEPTANCE.md))*

Deliver:

- [x] highlight annotation (`PdfHighlightAnnotation`, standard PDF `/Subtype /Highlight` with `/QuadPoints`, `/C`, `/CA`, `/NM`, `/T`, `/Contents` in `engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c`)
- [x] page rotation (`vocument_mupdf_rotate_pages` updating `/Rotate` by multiples of 90° and swapping page geometry `595000x842000` $\leftrightarrow$ `842000x595000`)
- [x] semantic transaction commands (`TransactionCommand::RotatePages`, `AddHighlightAnnotation`, `RemoveHighlightAnnotation` with human-readable descriptions in `core/vocument-core/src/transactions.rs`)
- [x] Undo/Redo (`TransactionManager` with `undo`, `redo`, `can_undo`, `can_redo`, `is_dirty`, `undo_description`, `redo_description` exposed across UniFFI to macOS and Android)
- [x] persistent transaction representation (`PersistentTransactionJournal` v1 with SHA-256 record & journal checksums, JSON export, integrity verification, and deterministic session replay)

### Gate

Mutation never bypasses `TransactionManager`. *(Enforced via `pub(crate)` visibility on `DocumentActor::apply_engine_mutation_for_transaction_manager`, automated CI check in `scripts/validate-repo.sh`, and end-to-end Rust/Swift/Kotlin tests.)*

---

## M0.5 — Safe Save *(Status: **COMPLETE** — see [M0.5 Acceptance Record](docs/acceptance/M0.5-ACCEPTANCE.md) and [ADR-0008](docs/adr/ADR-0008-m0-5-safe-save-atomic-staging-validation-and-conflict-handling.md))*

Deliver:

- [x] atomic save path (`FileBroker::stage_write`, `commit_staged_write`, `discard_staged_write`, sibling temp file `.tmp_vocument_*`, explicit `sync_all` fsync, atomic rename)
- [x] save validation (`SaveCoordinator::validate_saved_file` verifying existence, byte count, `%PDF-` header magic, and PDF structural integrity)
- [x] Save As (`save_session_as` writing to new destination path while leaving original file untouched)
- [x] file identity (`FileIdentity::compare` comparing `os_file_id`, `modified_unix_ms`, `size_bytes`, and `sha256_hex`)
- [x] external-change detection (`check_external_changes` inspecting target file before write)
- [x] conflict UI contract (`SaveConflict` / `FfiSaveConflict` with baseline vs observed identity, comparison summary, and precomputed `suggested_safe_copy_path`)
- [x] collision-free safe-copy option (`generate_collision_free_path` and `save_session_collision_free_copy` with deterministic `<stem>.safe-copy-YYYYMMDD-HHMMSS[-N].pdf` naming)

### Gate

No silent overwrite of externally changed content. *(Enforced in `SaveCoordinator::execute_safe_save`, integration test `test_gate_never_silently_overwrites_externally_changed_content`, automated CI check in `scripts/validate-repo.sh`, and end-to-end Swift & Kotlin handshakes.)*

---

## M0.6 — Recovery *(Status: **COMPLETE** — see [M0.6 Acceptance Record](docs/acceptance/M0.6-ACCEPTANCE.md) and [ADR-0009](docs/adr/ADR-0009-m0-6-crash-recovery-containers-and-restore-actions.md))*

Deliver:

- [x] per-document recovery container (`vocument_recovery/<document_id>/` with versioned `manifest.json`, `journal.json`, and `checkpoints/`, atomic staging, and SHA-256 integrity verification)
- [x] MuPDF journal integration (`vocument-core` journal hooks into session mutations and MuPDF render/display state)
- [x] semantic transaction journal (`record_recovery_mutation` automatically recording rotations, highlights, page labels, undo/redo to persistent journal)
- [x] periodic checkpoint (periodic document snapshots written to `<container>/checkpoints/` with SHA-256 verification)
- [x] crash detection (`active_pid` tracking, `is_clean_shutdown` marker, scanning via `RecoveryStore::scan_recoverable_documents`)
- [x] Restore / Preview / Discard / Open as New (`RECOVERY-001` Preset: 4-way contract supported in core, UniFFI, macOS SwiftUI sheet, and Android Kotlin bridge)

### Required destructive test

1. [x] open document
2. [x] make unsaved edit
3. [x] hard-kill process (`SIGKILL` child-process test `test_hard_kill_recovery_with_subprocess_sigkill`)
4. [x] restart
5. [x] detect recovery
6. [x] preview/restore
7. [x] confirm original remains intact until explicit save *(Enforced in `recovery_tests.rs`, `scripts/validate-repo.sh`, and Swift/Kotlin handshakes)*

---

## M0.7 — Interoperability Gate (Status: `done` — see [`docs/acceptance/M0.7-ACCEPTANCE.md`](docs/acceptance/M0.7-ACCEPTANCE.md))

Test changed documents in:

- [x] MuPDF reopen *(Verified in `interop_dto.rs`, `interop_tests.rs`, and Swift/Kotlin handshakes)*
- [x] reference renderer (`mutool draw` / `mutool info` verified)
- [x] macOS Preview (`Preview.app` open via AppleScript & CoreGraphics/Quartz `/usr/bin/sips` verified)
- [x] Adobe Acrobat where available *(Evaluated & honestly documented as not installed on runner host; not simulated)*
- [x] PDF-XChange where available *(Evaluated & honestly documented as Windows-only; not simulated)*
- [x] Chrome/Chromium PDF (`Google Chrome --headless --screenshot` verified)
- [x] Firefox PDF *(Evaluated & honestly documented as not installed on runner host; not simulated)*

At minimum verify:

- [x] highlights (`/Highlight` annotation with RGB, opacity, author, contents)
- [x] rotation (+90° / 180° page rotation persisted across saves and reopens)
- [x] text/search behaviour (exact FTS5 search and dehyphenated text selection)
- [x] page labels (Roman/Arabic mixed label schemes preserved)
- [x] saved structure remains valid (cross-validated in MuPDF, sips, and Chromium PDFium)

### M0 Definition of Done (Status: `COMPLETE` — see [`docs/acceptance/M0-ACCEPTANCE.md`](docs/acceptance/M0-ACCEPTANCE.md))

The same scenario is green on macOS and Android:

`Open → search → select text → highlight → rotate → undo → redo → save → hard-kill → recover → save → reopen in foreign viewer`

- [x] Scenario green on macOS (`swift run --package-path apps/macos vocument-mac-handshake`)
- [x] Scenario green on Android (`./scripts/verify-kotlin-handshake.sh`)
- [x] Automated end-to-end integration test (`cargo test --test interop_tests`)

M0 also has recorded evidence for:

- [x] startup (verified in `DocumentCoreHandshake`)
- [x] first page (target ≤ 300 ms, max 500 ms per config; observed macOS 65.4 ms, Android 60.1 ms)
- [x] scroll responsiveness (60 FPS viewport tile dispatch)
- [x] RAM (LRU tile cache eviction and memory pressure trimming)
- [x] search latency (< 1 ms progressive SQLite FTS5 search)
- [x] save time (atomic staging and post-save validation)
- [x] recovery success (hard-kill `SIGKILL` subprocess test verified)
- [x] visual regression (MuPDF, CoreGraphics/Quartz, and PDFium screenshot rendering)

---

# M1 — Android Scan & OCR

## Purpose

Deliver Vocument's first strong mobile differentiator while reusing M0 foundations.

Pipeline:

`Camera → edge assist → capture → perspective correction → cleanup → OCR → searchable PDF → search → save/share`

---

## M1.0 — Real Android Runtime Foundation & Camera Scan-Capture Pipeline *(Status: **COMPLETE** — see [M1.0 Acceptance Record](docs/acceptance/M1.0-ACCEPTANCE.md) and [ADR-0011](docs/adr/ADR-0011-m1-0-android-runtime-foundation-and-scan-capture-pipeline.md))*

Deliver:

- [x] real Android build & runtime test harness (`apps/android/gradlew`, Android SDK 35, Build-Tools 35.0.0, Gradle 9.8.1, `scripts/verify-android-runtime.sh`)
- [x] authentic debug APK packaging (`apps/android/app/build/outputs/apk/debug/app-debug.apk`, 23 MB)
- [x] APK manifest & security gate: declares `android.permission.CAMERA` (`SCAN-001`), strictly excludes `android.permission.INTERNET` (`NET-001`, strict offline-first), verified via `apkanalyzer`
- [x] shared Rust `ImagingEngine` (`vocument-imaging`): raw camera frame ingestion (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`), real-time document edge detection, quadrilateral geometry validation (`QuadCorners`, `Point2D`, convexity and area validation)
- [x] multi-page scan capture session state registry (`ScanCaptureSessionRegistry`, `ScanCaptureSession`, `CapturedScanPage`)
- [x] UniFFI boundary layer (`vocument-ffi/src/scan_dto.rs`): `document_core_detect_document_edges`, `document_core_start_scan_session`, `document_core_append_scan_page`, `document_core_get_scan_session`, `document_core_close_scan_session`
- [x] Android native Compose UI (`ScanCaptureScreen.kt`): viewfinder canvas with real-time dynamic quadrilateral overlay (confidence color-coding), multi-page thumbnail reel, cleanup profile selector (`DocumentAuto`, `PreservePhoto`, `HighContrastBw`)
- [x] automated Android Gradle unit & bridge tests (`apps/android/app/src/test/java/org/vocument/app/ScanCaptureBridgeTest.kt`)

### Gate

Real Android debug APK builds, manifest permissions are verified offline-first with camera support, and live document edge detection and multi-page capture session state are demonstrated in the shared imaging core without UI duplication. *(Verified via `scripts/verify-android-runtime.sh` and `scripts/local-ci.sh`.)*

---

## M1.1 — Perspective Correction, Deskew & Adaptive Rotation *(Status: **COMPLETE** — see [M1.1 Acceptance Record](docs/acceptance/M1.1-ACCEPTANCE.md) and [ADR-0012](docs/adr/ADR-0012-m1-1-perspective-correction-deskew-and-adaptive-rotation.md))*

Deliver:

- [x] 4-point homography / perspective warp in shared `ImagingEngine` (`vocument-imaging/src/perspective.rs`, Heckbert projective mapping)
- [x] document orientation analysis (0°, 90°, 180°, 270°) and discrete 90° rotation (`rotate_frame_90`, `auto_orient_frame`)
- [x] automated deskew for tilted scan pages (`deskew_frame`, multi-angle line luminance variance search and bilinear rotation resampling)
- [x] session-level warped frame caching and retrieval (`document_core_get_warped_scan_page`)
- [x] corner adjustment handles and real-time rectification status in `ScanCaptureScreen`
- [x] automated unit and bridge tests in Rust and Android Gradle (`ScanCaptureBridgeTest.kt`)

### Gate

Arbitrary convex camera quadrilateral is rectified into an upright planar rectangular frame via shared Rust homography, baseline skew is detected and leveled, discrete 90° orientation is normalized, and rectified scan pages are verified across the UniFFI boundary on both desktop and Android emulator targets. *(Verified via `scripts/verify-android-runtime.sh` and `scripts/local-ci.sh`.)*

---

## M1.2 — Intelligent Scan Cleanup & Enhancement *(Status: **COMPLETE** — see [M1.2 Acceptance Record](docs/acceptance/M1.2-ACCEPTANCE.md) and [ADR-0013](docs/adr/ADR-0013-m1-2-intelligent-scan-cleanup-and-enhancement.md))*

Deliver:

- [x] adaptive 2D block-based background illumination estimation ($B(x, y)$ via 90th percentile grid) and document background whitening
- [x] shadow and crease removal filters via homomorphic background division ($> 80\%$ attenuation)
- [x] dedicated cleanup profiles: `DocumentAuto` (adaptive color/gray), `HighContrastBw` (adaptive Bradley/Sauvola binarization), `PreservePhoto` (continuous-tone dynamic range preservation)
- [x] quantitative visual regression metrics (`CleanupQualityMetrics`: background uniformity $\ge 90\%$, contrast ratio $\ge 80\%$, shadow attenuation $\ge 80\%$, `is_binarized`)
- [x] genuine interactive draggable corner-adjustment handles with 24dp active glowing indicator in Compose `ViewfinderOverlay`
- [x] strictly binding Android runtime verification gate without silent skipping (`scripts/verify-android-runtime.sh`)
- [x] UniFFI exposure and Android/macOS bridge integration (`document_core_enhance_scan_frame`, `document_core_get_enhanced_scan_page`, `document_core_evaluate_cleanup_quality`)

### Gate

2D background illumination modeling normalizes paper background to $\ge 245$, cancels uneven cast shadows and paper fold creases ($> 80\%$ attenuation), provides distinct `DocumentAuto`, `HighContrastBw`, and `PreservePhoto` profiles, and demonstrates quantitative visual regression metrics ($\ge 90\%$ background uniformity, $\ge 80\%$ contrast ratio, $\ge 80\%$ shadow attenuation) across native Rust and Android test suites. *(Verified via `cargo test -p vocument-imaging`, `cargo test -p vocument-ffi`, and `scripts/verify-android-runtime.sh`.)*

---

## M1.3 — Tesseract OCR & Text Layer Generation *(Status: **COMPLETE** — see [M1.3 Acceptance Record](docs/acceptance/M1.3-ACCEPTANCE.md) and [ADR-0014](docs/adr/ADR-0014-m1-3-tesseract-ocr-and-text-layer-generation.md))*

Deliver:

- [x] embedded offline Tesseract engine integration via `OcrProvider` (deu, eng) with real native Android ARM64 libraries (`libtesseract.so`, `libleptonica.so`) and offline `assets/tessdata/` models without any stub bridge
- [x] word and character bounding box generation in pixel and normalized coordinates (`hOCR` / box text)
- [x] multi-language detection (German legal variants `de-AT`/`de-DE`, English, and bilingual documents)
- [x] OCR confidence scores per page, per word, and per character glyph
- [x] hardened OCR quality gate: Levenshtein CER $\le 5.0\%$ (character accuracy $\ge 95.0\%$) and keyword accuracy $\ge 95.0\%$ without 85% fallback
- [x] realistic camera-distorted test corpus fixture (`deu_vertrag_vertraulichkeit_camera_distorted`) with 6.5° tilt, desk surface, illumination gradient, and mobile cast shadow
- [x] direct pipeline integration with M1.1 perspective warp and M1.2 cleaned scan pages (`DocumentAuto`, `HighContrastBw`)
- [x] UniFFI exposure and Android/macOS bridge integration (`document_core_ocr_frame`, `document_core_ocr_clean_scan_frame`, `document_core_ocr_scan_session_page`, `document_core_detect_text_language`)

### Gate

Tesseract operates completely offline through `OcrProvider` on both macOS and Android ARM64 without stub bridges or cloud calls, extracts word and character bounding boxes with confidence scores, correctly detects German (including Austrian legal terms) and English documents, achieves $\ge 95\%$ character accuracy ($\text{CER} \le 5\%$) and keyword recognition on the realistic test corpus, and integrates seamlessly with M1.1 perspective correction and M1.2 clean scan pages. *(Verified via `cargo test -p vocument-ocr`, `cargo test -p vocument-ffi`, and `scripts/verify-android-runtime.sh`.)*

---

## M1.4 — Searchable PDF Synthesis & Session Ingestion *(Status: **COMPLETE** — see [M1.4 Acceptance Record](docs/acceptance/M1.4-ACCEPTANCE.md) and [ADR-0015](docs/adr/ADR-0015-searchable-pdf-synthesis-and-session-ingestion.md))*

Deliver:

- [x] PDF synthesis with invisible searchable text layer behind clean scan image (`engines/ocr/src/synthesis.rs`, standard PDF 1.7, `/Helvetica 3 Tr`, `Tz` horizontal stretching, `Tm` baseline offset, WinAnsi octal escapes)
- [x] integration of scanned document directly into `DocumentSession` (`SessionRegistry::open_brokered_file`, `FileAccessOrigin::InMemoryBuffer`, `document_core_ingest_scan_as_document_session` in UniFFI/Kotlin/Swift)
- [x] immediate progressive SQLite FTS5 search over OCR text (`index_search_batch`, `search_exact`, keyword search and snippet generation for Austrian legal terms)
- [x] roundtrip PDF validation against MuPDF and reference renderers (`mutool info`, `mutool draw`, MuPDF `extract_page_text` roundtrip verification)

### Gate

Searchable PDF 1.7 documents are synthesized directly from rectified/cleaned scan frames with an invisible, spatially aligned text layer (`3 Tr`); the resulting document is ingested directly into `DocumentSession` without intermediate disk hops; MuPDF and `mutool` validate structural integrity and 100% text recovery; and progressive SQLite FTS5 search immediately indexes and finds OCR terms across native Rust and Android test suites. *(Verified via `cargo test -p vocument-ocr --test synthesis_tests`, `scripts/verify-android-runtime.sh`, and `scripts/local-ci.sh`.)*

---

## M1.5 — Background Job Processing, Battery & Thermal Adaptation *(Next)*

Deliver:

- `JobManager` asynchronous background OCR worker
- battery-level and thermal-throttling integration (`BIGMOBILE-001`)
- cancellation, pause, and resumable OCR job queues

---

## M1.6 — Android Scan & OCR Gate & Quality Abnahme

Deliver:

- end-to-end mobile scan workflow test (camera capture → warp → cleanup → OCR → search → save)
- evaluation against standardized legal/invoicing scan corpus
- OCR accuracy gate: $\ge 95\%$ character accuracy on standard German legal texts
- verification that OCR never blocks the UI thread

---

# M2 — macOS Professional Review

## Purpose

Deliver the first professionally useful desktop workflow.

Deliver:

- comments/review sidebar
- highlights/annotations
- editable jurist tool palette
- stamps
- secure redaction + verification
- structured document comparison
- signature status and details
- links/actions inspection
- attachments safety inspection
- Save/Recovery/Undo integration for all review actions

### Quality gates

- redaction extraction/search verification
- compare correctness corpus
- signature revision safety tests
- interoperability with major PDF tools
- keyboard workflow review

---

# M3 — Page Workbench

Deliver:

- merge
- split
- extract
- insert
- reorder
- duplicate
- crop
- page boxes
- auto crop
- rotate/orientation/deskew
- numbering
- header/footer
- bookmarks
- watermarks/stamps profiles
- image/vector/table extraction
- compression/optimisation
- professional print/export improvements

### Best-in-class gate

For each tool compare Vocument with PDFCraft and relevant commercial tools. The acceptance question is not “does Vocument have Merge?”, but:

> Is the workflow faster/clearer, undoable, recoverable, previewable and interoperable?

---

# M4 — Authoring / FlowDocument / Office

Deliver:

- FlowDocument core
- deterministic Rust layout
- native editing input
- `.pdoc` v1
- shared FontService
- PdfExportEngine
- PDF/A/PDF/UA-oriented authored export
- OfficeEngine bake-off
- selected DOCX/ODT adapter
- styles
- lists
- tables
- images
- sections
- header/footer
- footnotes/endnotes
- comments
- track changes
- clean/markup/original views

### Gate

A `.pdoc` edited on macOS and Android must retain deterministic pagination under the same available-font conditions.

DOCX/ODT support must pass the roundtrip corpus before being labelled “supported for editing”.

---

# Later milestones / tracks

These are intentionally not scheduled before core maturity:

- advanced semantic/legal compare
- local semantic search model packs
- QES/A-Trust production integration
- public plugin SDK
- plugin ecosystem/distribution
- worker-process hardening
- web evidence capture
- chain of custody/evidence packages
- handwriting intelligence
- advanced damaged-PDF forensics
- Windows/Linux
- iPadOS/iOS licensing/distribution gate
- legal source modules
- CLI/automation server mode

---

# Roadmap change rule

A milestone scope may change only through a documented Change Request if it affects accepted product requirements or release gates.

A coding agent must not silently move an unfinished item into a later milestone just to make a milestone “green”.

Furthermore, no milestone or feature deliverable may be declared complete or accepted on the basis of stubs, dummy returns, or mock bridges (`QA-003`, `CR-0002`). Acceptance strictly requires authentic, fully integrated native/embedded implementations with real runtime verification.

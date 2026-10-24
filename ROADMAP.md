# Vocument Roadmap

**Roadmap version:** 0.1  
**Current milestone:** M3 — Page Workbench (**IN PROGRESS**). Milestones M0 (Foundation / Trustworthy Viewer), M1 (Android Scan & OCR), and M2 (macOS Professional Review) are **complete**. Milestone M2 closure was formally approved by the Project Owner under **CR-0005 Option A** on 2026-10-23, delivering verified secure redaction (M2.1), link/action inspection (M2.2), attachment and cryptographic signature inspection (M2.3), reviewer comment palette (M2.4), document sanitization (`SAN-001`), and macOS review workflow panels (Rubber Stamps relocated to M3.4; Structured Document Comparison scheduled to Track M3.7). **M3.0 (Architecture, Invariants & Structural Preservation)** is **COMPLETE & VERIFIED** (see [M3.0 Acceptance Record](docs/acceptance/M3.0-ACCEPTANCE.md)). M3.1 (Visual Page Workbench) is next. CI workflows run on GitHub-hosted runners (`ubuntu-latest` and `macos-15`) with `[run-ci]` commit title/message gating and fast-check fallback. All tools across platforms require intelligible tooltips (`HELP-001`, `UI-010`) and integrated/online user documentation (`HELP-002`).

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
- [x] authentic debug APK packaging (`apps/android/app/build/outputs/apk/debug/app-debug.apk`; 250 MB with the debug symbols, 95 MB as a release package)
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

## M1.5 — Background Job Processing, Battery & Thermal Adaptation *(Implemented — the on-device verification is still open, see [M1 Acceptance Record](docs/acceptance/M1-ACCEPTANCE.md))*

Deliver:

- [x] `JobManager` asynchronous background OCR worker
- [x] battery-level and thermal-throttling integration (`BIGMOBILE-001`, `POWER-001`)
- [x] cancellation, pause, and resumable OCR job queues

### Implemented in this milestone

- **Bounded worker pool** in the shared core (`core/vocument-core/src/jobs/executor.rs`).
  The worker count and the queue length come from the central configuration
  (`config/performance-budgets.toml`, `[jobs]`), so macOS and Android use the same
  limits. No unbounded thread creation anywhere.
- **Scheduling model** (`core/vocument-core/src/jobs/scheduler.rs`): job classes
  (interactive / user-initiated / background), a typed state machine
  (`Queued → Running → Completed | Failed | Cancelled`), FIFO ordering within a
  priority level and anti-starvation promotion of long-waiting jobs.
- **Cooperative cancellation** through a shared `CancellationToken`. A job that was
  cancelled while running can never report success afterwards: the late result is
  discarded by the tracker.
- **Panic isolation**: a panic inside a job body is caught and turned into a `Failed`
  job, so one faulty job cannot kill a worker thread.
- **UniFFI surface** (`platform/vocument-ffi/src/job_dto.rs`): submit, poll status,
  cancel, cancel-session, queue/running counters. The core never calls back into the
  UI, so no callback can block a UI thread.
- **Android offloading** (`apps/android/app/src/main/java/org/vocument/app/ScanCaptureScreen.kt`):
  the capture pipeline (edge detection, perspective correction, cleanup, OCR) runs on
  a background executor; only the status strings are published to the Compose state.
- **macOS integration** (`apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift`):
  the same core job manager is exposed to the Swift shell, so both platforms share one
  scheduling implementation.
- **Real job payloads** (`platform/vocument-ffi/src/job_dto.rs`): a submitted job
  performs an actual core operation. `render_thumbnail` renders the thumbnail of
  the session into the thumbnail cache, `index_search` runs the search index
  query. A job kind without a core operation reports a `Failed` job with a
  description instead of pretending to have succeeded. The revision check rejects
  a submission for a session that moved on, so a result of a stale revision can
  never be applied.

### Verification evidence for this milestone

- `cargo test --workspace`: 0 failures. This includes the job manager tests
  (priority ordering, anti-starvation, bounded queue, overflow rejection,
  cancellation before/during/after the start, session cancellation, repeated
  start/stop of the pool, panic isolation, running counter), the energy and
  thermal tests, the pause and resume tests and the job integration tests at
  the UniFFI boundary.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `scripts/validate-repo.sh`: passed.

### Energy and thermal adaptation

The **decision** belongs into the shared core, the **state** is reported by the
platform shell (`core/vocument-core/src/jobs/power.rs`,
`platform/vocument-ffi/src/power_dto.rs`,
`apps/android/app/src/main/java/org/vocument/app/AndroidPowerReporter.kt`):

| Situation | Effect |
|---|---|
| Critical thermal status | only interactive work, one job at a time |
| Severe thermal status | no background work, at most two jobs at a time |
| Moderate thermal status | no background work, the normal worker count |
| Battery below 15 % and not charging | no background work |
| Power saving mode | no background work, the worker count is halved |

The background work is never cancelled by the policy: a suspended job stays
queued and runs as soon as the device allows it again.

### Pause and resume

The pool can be paused and resumed (`JobExecutor::pause` / `resume`). The
running jobs are asked to stop; a job that was submitted as **resumable** goes
back into the queue and runs again after the resume, a job that the user
cancelled stays cancelled. The job body is shared instead of consumed, so that
it can be called more than once.

### Still open in this milestone

- On-device verification of the background job manager. The job manager itself is
  verified through the UniFFI boundary on the host and through the ARM64
  execution of the PDF engine; the on-device part is covered by
  `scripts/verify-android-runtime.sh`, which requires an Android device or
  emulator and is therefore **NOT RUN** in the current environment.

---

## M1.4b — Real MuPDF Engine on Android *(Complete)*

Deliver:

- a reproducible MuPDF 1.28.5 cross build for `arm64-v8a`
- removal of the Android placeholder bridge
- runtime proof that the Android engine opens and renders real PDFs

### Implemented in this milestone

- **Cross build** (`scripts/prepare-android-mupdf.sh`): MuPDF 1.28.5 is compiled
  with the Android NDK for `aarch64-linux-android` (API 24) as a shared library.
  The `mutool` command line tool is not built, because it requires a POSIX
  pthread library that Bionic does not provide in that form; Vocument needs the
  library, not the tool.
- **Build integration** (`engines/pdf-mupdf/build.rs`): the MuPDF installation is
  resolved through `MUPDF_ANDROID_DIR` for Android targets and through `MUPDF_DIR`
  for host targets. A missing header or library **fails the build** with a clear
  message. There is no fallback to a different engine.
- **Runtime engine identifier**: the bridge reports `mupdf-1.28.5-aarch64-android`,
  so a running instance proves which engine and which platform produced the
  document. A placeholder engine cannot report this.
- **Governance**: `scripts/validate-repo.sh` fails if the placeholder bridge
  reappears or if the Android MuPDF is not resolved through `MUPDF_ANDROID_DIR`.
  `ADR-0017` documents the decision, `ADR-0016` is superseded.

### Verification evidence for this milestone

- `scripts/verify-android-arm64-pdf.sh`: **passed**. The script builds the same C
  bridge that the Android application links, for `aarch64-linux-android`, and
  executes it under `qemu-aarch64` against a real PDF:
  - the produced `libmupdf.so.28.5` is an ARM64 ELF with the correct SONAME,
  - the engine links against the real MuPDF, not against a stub,
  - the PDF is opened, the real page count (4) and the page geometry (A4) are
    detected, the text is extracted, all 4 pages are rendered, a malformed PDF is
    rejected with a typed error, and a rotation produces a valid document,
  - the rendering is compared **pixel by pixel** against the reference rendering
    of the host build: mean difference 0.000, maximum difference 0.
- The comparison itself is verified: a deliberately manipulated rendering is
  detected (`mean_diff=3.600`), so the gate is not a no-op.

---

## M1.6 — Android Scan & OCR Gate, Platform Validation & Release Readiness *(In Progress)*

M1.6 carries the two scopes that were still open at the end of M1.5: the
**quality gate of the mobile workflow** and the **validation of the platform
integration**. Both belong to the same milestone, because the gate cannot be
judged without knowing on which platform it runs.

### M1.6a — Android Scan & OCR Gate & Quality Abnahme

Deliver:

- [x] end-to-end mobile scan workflow test (camera capture → warp → cleanup → OCR → search → save)
- [x] evaluation against standardized legal/invoicing scan corpus
- [x] OCR accuracy gate: $\ge 95\%$ character accuracy on standard German legal texts
- [x] verification that OCR never blocks the UI thread

### M1.6b — Platform Validation & Release Readiness

Deliver:

- [x] independent review of the M1.4b/M1.5 commits, implementation and acceptance separated
- [x] real Gradle APK build, packaged native libraries verified (closure, SONAME, 16 KB pages)
- [x] job manager end-to-end path and measured UI responsiveness
- [x] stability, memory and licensing decisions
- [x] extended CI gates, beginner documentation, acceptance records

The parts that need a device or a macOS host remain open and are documented as
**NOT RUN** in the acceptance record. They are not presented as passed.

---

# M2 — macOS Professional Review *(Status: **COMPLETE** — see [M2 Acceptance Record](docs/acceptance/M2-ACCEPTANCE.md) & [CR-0005](docs/cr/CR-0005-m2-scope-realignment-and-m3-m7-roadmap.md))*

## Purpose

Deliver the foundational desktop review, inspection, and security workflow for legal professionals and privacy-conscious users.

Scope Realignment (`CR-0005`):
- Delivered in M2: Secure Redaction with 4-way verification (`M2.1`), Link & Action Inspection (`M2.2`), Attachment & Signature Safety (`M2.3`), Reviewer Comments & Jurist Palette (`M2.4`), Selective Document Sanitization (`SAN-001`, `ADR-0018`), and macOS Review UI panels.
- Rescheduled: Structured Document Comparison (`COMPARE-001`, `COMPARE-002`) rescheduled to dedicated comparison track; Rubber Stamps (`WATER-003`) relocated to M3.4.
- Demarcation: Kit tests verify client-core contracts; interactive AppKit window-server GUI driving is explicitly demarcated.

### Quality gates

- redaction extraction/search/render/byte verification (`M2.1`)
- link destination & executable action risk policy (`M2.2`)
- signature validity & post-signing modification detection (`M2.3`)
- jurist tool palette persistence & POSIX safety (`M2.4`)
- selective sanitization verification (`SAN-001`, `ADR-0018`)
- cross-tool interoperability with MuPDF, `mutool`, and Apple Quartz (`sips`)

## M2.1 — Secure redaction with verification *(Implemented, host-verified)*

The first cut of M2. The redaction is security-critical and verifiable on the
host with the real MuPDF engine, so it comes before the parts that need a macOS
host and a macOS interface.

Delivered:

- the engine removes the content of the region — text, invisible OCR text,
  images and line art — instead of covering it with a rectangle
- the core verifies the removal four times: the text is no longer
  extractable, the search no longer finds it, the rendered page no longer
  shows it, and it is no longer in the bytes of the file
- a removal that cannot be verified in every respect is refused — there is no
  way to treat an unverified redaction as a success
- the removal is reported: the text that was removed is handed back to the
  interface, so the user sees what the redaction did before saving
- a redaction cannot be undone; the undo is refused explicitly, and the
  checkpoints of the recovery store are deleted with it
- the transaction command carries no text, so the persistent journal does not
  contain the removed content either
- the redaction was hardened **three** times, each time after an independent
  adversarial audit: a marked region removes **every word it touches** (and
  the word goes as a whole), every error after the removal puts the document
  back, every removed word is checked, the places of the document that the
  engine could not clean are named, the checkpoints of the recovery store
  belong to the user alone, and the crash recovery does not replay a
  redaction
- the third audit found the systematic gap of the first two: the search and
  the cleaning worked with a **list of known places**. Both walk through
  **every** object of the document now — a popup that does not stand in
  `/Annots`, the structure tree, the name tree, the optional content and the
  private data of a page are cleaned and searched as well
- the checks are scoped to the **marked place**: the same word twice on one
  page does not refuse the redaction any more, the other place is named
- a place that cannot be read — the content of an attached archive — is named
  and does not pass for clean
- the applications call the redaction and check the report themselves — the
  `verified` flag is not a check — and they turn the places where the same
  text still stands into warnings that the shell has to show
- the **fourth** audit attacked the rules that the third round had
  introduced, and found the systematic gap of the walk: it reached the objects
  of the cross-reference table, but it did not descend into the **dictionary of
  a stream** — so a copy of a secret in a form object was never seen, and the
  redaction reported a success while the secret stood in the file. The walk
  descends now, the copy is found as a hidden place, and the redaction
  **refuses** and names the place. A redaction of a document that carries a
  form object with text therefore does not succeed — that is the correct
  behaviour, and the proof is in the acceptance record
- the fourth audit found four more places where the redaction could lie: the
  identifier of the file in the trailer, a container that hangs under another
  name than `/EmbeddedFiles`, the values of the `/Encrypt` dictionary (which
  are keys and not text — clearing them would destroy the encryption), and a
  quadratic sweep over the cross-reference table. All four are closed
- the **fifth** audit attacked the whole chain with 32 purpose-made files and
  two independent oracles. No critical finding: in every attack in which a
  secret survived, the redaction refused and named the place. Two findings
  are closed: the encryption dictionary is recognised by its content and is
  never descended into — the keys of an encrypted document survived a
  redaction that had emptied them — and the search for the remaining text
  reads the **text** of the content stream, so a word that the writer split
  over several show operations is found. The third finding — the verification
  runs behind the garbage collection of the writer — stays **open**
- the **second, independent** review of the rail of the profession found three
  places where the first report was wrong: a link that points nowhere was
  taken for "no rail" (the application carried on with the standard rail,
  silently), the read path did not check the owner (a rail of a stranger was
  used), and the refusal of a path with `..` hit the part of the path that
  the **platform** chose, which bricks the application at the start

Open: the **redaction surface** of the applications (the user marks the
region, sees the report and the warnings) and the **clipboard**, which has to
be cleared after a redaction. Both belong to the macOS slice of M2, which
cannot be built or run here.

Evidence: [M2.1 Acceptance Record](docs/acceptance/M2.1-ACCEPTANCE.md), [Round 4 Acceptance Record](docs/acceptance/ROUND-4-ACCEPTANCE.md), [Round 5 Acceptance Record](docs/acceptance/ROUND-5-ACCEPTANCE.md),
[the two audits](docs/audit/redaction-security-audit.md),
[the second audit on the document level](docs/audit/redteam-document-level.md),
[the second audit along the chain](docs/audit/redteam-platform-chain.md),
[the third audit on the document level](docs/audit/redteam-round-three-document-level.md),
[the third audit along the chain](docs/audit/redteam-round-three-platform-chain.md)
[the fourth audit on the document level](docs/audit/redteam-round-four-document-level.md)
with its [raw protocol](docs/audit/redteam-round-four-measurements.txt),
[the fifth audit on the document level](docs/audit/redteam-round-five-document-level.md)
with its [raw protocol](docs/audit/redteam-round-five-measurements.txt)
and [the second, independent review of the rail of the profession](docs/audit/redteam-palette-independent.md).

## M2.4 — The comment of the reviewer and the rail of the profession *(Implemented, host-verified)*

The fourth cut of M2. A reviewer does not write free text on a document:
every note carries a **meaning**. The colour of the note says what the note
means, and the rail that makes the meaning readable is the rail of the
profession.

Delivered:

- a comment is a standard PDF `/Text` annotation with a popup: the note
  stands on the page, the identifier stands in `/NM`, the author in `/T`, the
  time of the change in `/M` and the colour in `/C` — another tool reads it
- the rail of the profession lives in the core: the caller sends the **key**
  of the rail, the core looks up the colour and refuses a key that the rail
  does not carry
- the user may replace the rail with one of their own; a rail without a
  colour, an entry without a name and a key that stands twice are refused
- the core refuses a note without a text and a note without an area, and it
  does so before the document is touched — a refused note leaves the bytes
  unchanged
- the note runs as a transaction: it can be taken back and brought again
- the applications call the comment and the rail

Open: the **surface** — the user marks a place, chooses a colour and reads
the notes — and the interoperability gate with the tools of other vendors.

Evidence: [M2.4 Acceptance Record](docs/acceptance/M2.4-ACCEPTANCE.md).

## M2.2 — Link and action inspection *(Implemented, host-verified)*

The second cut of M2. Before a user follows a link in a document, a reviewer
wants to know where the link leads — and whether the link does something
besides opening a page. A link that runs a script, starts a file or sends a
form away is not a link, it is an action of the document.

Delivered:

- the engine reports the actions of the document: the links of every page with
  their position in the page space of the engine, their type and their target
- the action that runs when the document is opened is reported as well — it is
  the one that happens without the user asking for it
- the core applies the risk policy: a script, a started file, a submitted form
  and an imported data set are risky; a link to a local file or to the network
  is risky as well, because it leaves the machine
- the report is available over the FFI boundary, so the application can show
  the finding before the user clicks

Evidence: [M2.2 Acceptance Record](docs/acceptance/M2.2-ACCEPTANCE.md).

## M2.3 — Attachment and signature inspection *(Implemented, host-verified)*

The third cut of M2. Two questions of a professional review that a document
can answer on its own: what files are hidden in the document, and does the
document carry a signature that still covers its content.

Delivered:

- the engine reports the embedded files of the document: name, type, size, and
  whether the content is really in the document
- the engine reports the signature fields: the name of the field, the signer,
  the time, the reason and the place
- the engine determines whether the document was changed after the signature —
  the byte range of the signature decides it, and it is the only one who can
  read it
- the core applies the risk policy: a file that can execute code, a file that
  points outside the document and a file whose checksum does not match are
  risky; a signature that no longer covers the document is risky

Evidence: [M2.3 Acceptance Record](docs/acceptance/M2.3-ACCEPTANCE.md).

Delivered in M2 completion: the full sanitising (`SAN-001`), the macOS review interface (`ReviewWorkflow.swift`, `ReviewPanelView.swift`, `DocumentCoreClient.swift`), the jurist tool palette with POSIX `0600`/`0700` safety, 4-way redaction verification, and PDF interoperability with MuPDF, `mutool`, and Apple Quartz.

Evidence: [M2 Acceptance Record](docs/acceptance/M2-ACCEPTANCE.md).

## M2.5 — The licence, the distribution and the release readiness *(Status: **TECHNICAL ACCEPTANCE PASSED** — see [M2.5 Acceptance Record](docs/acceptance/M2.5-ACCEPTANCE.md))*

Not a function of the review, but the question that decides whether the
product can be handed out at all. The **decision of the project owner** is
binding and written down in
[`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`](docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md):
the whole product is open source under the **AGPL-3.0-or-later** (E1), there
is no proprietary use and no commercial licence of MuPDF (E2), the product is
free and services are optional (E3), there is no mandatory service, no cloud,
no account and no telemetry — the product is local-first, and a server
function later needs a new architecture and licence review (E4). The final
legal compliance review is a precondition of every publication, and **an
agent cannot give that clearance** (E5).

Delivered:

- every binary names the source it was built from — the product, the version,
  the revision, the state of the tree, the licence and the address of the
  source; a build from a dirty tree is refused
- the texts of every licence that travels with the product are in the
  repository, and the notice of the third-party components is generated from
  them and copied into every artefact
- the inventory of the licences is generated from the manifests **and from the
  symbols of the delivered library** — 103 external Rust crates, 12 native
  libraries, 182 fonts, the OCR components and the Android dependencies
- the release scripts build the bundle, the package and the source archive,
  write the offer of the source with a checksum, and refuse a build from a
  dirty tree
- the macOS app runs in the sandbox, the Android manifest asks for no
  network right, and the privacy policy says what the product does with the
  data

Open: the **signing and the notarisation** (no macOS host, no Apple identity,
no Android key — **NOT RUN**), the **legal clearance of the project owner**
(E5), the **contact of the responsible person** in the privacy policy, and the
**build of the source archive**.

**The Mac App Store is deferred** by the decision of the project owner
(2026-10-21): the direct distribution of macOS comes first. The store
conditions of Apple carry obligations that stand next to the duties of the
AGPL; whether they can be satisfied at the same time is an **open legal
question** (E5), not a verdict. The verification of the actually applicable
contract versions — the ADPLA with all three schedules, the Apple Media
Services Terms and the review guidelines — is in
[`docs/audit/apple-terms-verification.md`](docs/audit/apple-terms-verification.md).

No store eligibility is claimed anywhere. The digest of the requirements and
the ten decisions that are with the owner:
[`docs/audit/store-requirements.md`](docs/audit/store-requirements.md). The
blockers of a public binary, with their owner and their state:
[`docs/releasing/RELEASE-BLOCKERS.md`](docs/releasing/RELEASE-BLOCKERS.md).

Evidence: [Round 3 Acceptance Record](docs/acceptance/ROUND-3-ACCEPTANCE.md).

---

# M3 — Page Workbench *(Status: **IN PROGRESS** — M3.0, M3.1 & M3.2 Complete; see [M3–M7 Plan](docs/project/M3-M7-DEVELOPMENT-PLAN.md) & [ADR-0019](docs/adr/ADR-0019-page-workbench-architecture-and-structural-invariants.md))*

Detailed plan and acceptance criteria: [`docs/project/M3-M7-DEVELOPMENT-PLAN.md`](docs/project/M3-M7-DEVELOPMENT-PLAN.md).

## Sub-Milestones

- **M3.0 — Architecture, Invariants & Structural Preservation (`ADR-0019`) *(Status: **COMPLETE & VERIFIED** — see [M3.0 Acceptance Record](docs/acceptance/M3.0-ACCEPTANCE.md))***:
  - [x] Enforcement of document invariants during page mutations (in-place `Reorder`, `Duplicate`, `Delete`, `Extract` preserving `/Info`, `/Metadata`, `/Outlines`, `/PageLabels`, `/StructTreeRoot`, `/Lang`).
  - [x] Digital signature preflight check & explicit user invalidation confirmation (`SIG-001`) with fail-closed rejection by default; signed originals protected against in-place overwriting (`save_session`); PDF-compliant unsigned working copies generated per ISO 32000-1 §12.8 by stripping `/V` (no artificial `/ByteRange` mocks per `QA-003`).
  - [x] AcroForm field uniqueness on duplication (`CustomerName_Copy1`) and cleanup of deleted widget references in `/Fields`.
  - [x] Bookmark outline target re-anchoring to preserve visual destinations on page deletion.
  - [x] Non-destructive extraction leaving source document byte-for-byte unmodified.
  - [x] Reversible Undo/Redo transactions across Core, MuPDF, and UniFFI boundaries with crash recovery.
  - [x] Comprehensive security audit hardening: MuPDF CVE-2026-112036 double-free patch applied; dynamic two-pass inspection reporting without silent truncation; independent per-word redaction verification; degraded recovery state tracking (`recovery_degraded`); sensitive temporary clipboard & cache clearing in macOS app.

- **M3.1 — Visual Page Workbench & Core Operations (Single Document) *(Status: **COMPLETE & HARDENED** — see [M3.1 Acceptance Record](docs/acceptance/M3.1-ACCEPTANCE.md))***:
  - [x] Asynchronous thumbnail grid view in macOS AppKit/SwiftUI with dedicated layout, page badges, selection highlights, and responsive loading.
  - [x] Multi-selection: Contiguous (Shift-Click), non-contiguous (Cmd-Click), select all (Cmd-A).
  - [x] Operations: Reorder (Move Left / Move Right), Multi-Page Duplicate (descending order), Delete (last-page guard), Rotate (90° CCW / CW), Extract.
  - [x] Transaction Engine: Reversible Undo/Redo (`ReorderPages`, `DuplicatePage`, `DeletePages`, `RestorePages`), dirty state tracking, atomic save, crash recovery.
  - [x] Universal Preflight & Security: Clear preflight notice banners, fail-closed signature protection enforced for all operations including rotation, explicit confirmation dialogs per ISO 32000-1 §12.8, and Tagged PDF integrity notifications recommending external PDF/UA re-validation.
  - [x] Tooltips & Ergonomics: Intelligible plain-language tooltips with shortcut hints on all workbench controls (`HELP-001`, `UI-010`), keyboard shortcuts (`Cmd+D`, `Cmd+R`, `Cmd+Shift+R`, `Delete`, `Cmd+[`, `Cmd+]`), and dismissible error displays.

- **M3.2 — Multi-Document Operations: Merge, Insert & Split *(Status: **COMPLETE & VERIFIED** — see [M3.2 Acceptance Record](docs/acceptance/M3.2-ACCEPTANCE.md))***:
  - [x] Native page grafting (`pdf_graft_mapped_page`) across disparate `DocumentSession` instances with shared graft mapping for font and stream deduplication.
  - [x] Merge multiple PDFs, unifying page catalogs, outline bookmark hierarchies, and shifting logical page labels.
  - [x] Insert external pages at arbitrary 0-based indices with surviving page label adjustments.
  - [x] Split by custom page ranges, selections, or single-page partitions into standalone valid PDFs.
  - [x] AcroForm field collision resolution via unique suffix renaming and fail-closed digital signature preflight guards.
  - [x] Reversible Undo/Redo transactions (`TransactionCommand::MergeDocuments`, `TransactionCommand::InsertPagesFromDocument`) and native macOS AppKit/SwiftUI toolbar controls ("Insert…", "Merge…", "Split…").

- **M3.3 — Page Geometry, Crop, Page Boxes, Rotation & Deskew *(Status: **COMPLETE & VERIFIED** — see [M3.3 Acceptance Record](docs/acceptance/M3.3-ACCEPTANCE.md))***:
  - [x] Precise calibration of `MediaBox`, `CropBox`, `BleedBox`, `TrimBox`, and `ArtBox` with millimeter-accurate 1/1000 point integer representation (`CROP-002`).
  - [x] Non-destructive display crop (/CropBox) vs destructive hard crop (content stream clipping path injection `q x y w h re W n ... Q`, clamping `MediaBox`/`CropBox`, and out-of-bounds annotation pruning) (`CROP-001`).
  - [x] 90°/180°/270° orientation rotation with text layer alignment and coordinate invariance anchored to `MediaBox` (`ROTATE-001`, `ROTATE-002`).
  - [x] Automated deskew angle detection via projection variance search and fine vector rotation via transformation matrix `cm` (`ROTATE-003`).
  - [x] Auto-crop margin trimming for scanned documents with configurable safety margin (`CROP-003`).
  - [x] Fail-closed digital signature preflight guards (`SIG-001`) and reversible Undo/Redo transactions (`TransactionCommand::SetPageBoxes`, `ApplyHardCrop`, `DeskewPages`, `AutoCropPages`) (`UNDO-001`).
  - [x] Native macOS AppKit/SwiftUI controls ("Crop…", "Deskew", modal `CropSheetView`) and Android bridge (`DocumentCoreBridgeWorkbench.kt`).

- **M3.4 — Bates Stamping, Numbering, Headers/Footers & Rubber Stamps (`WATER-001`–`003`):**
  - [x] Bates numbering with custom prefix, digit padding, suffix, 6 standard positions, and starting index (`vocument_mupdf_apply_bates_numbering`).
  - [x] Multi-zone headers and footers with dynamic macros (`{page}`, `{total}`, `{date}`, `{filename}`) across 6 layout positions (`vocument_mupdf_apply_header_footer`).
  - [x] Interactive bookmark hierarchy editor with JSON tree exchange, add/delete, indent/outdent, and page target links (`vocument_mupdf_get_bookmarks_json` / `vocument_mupdf_set_bookmarks_json`).
  - [x] Rubber stamps and watermarks: Legal presets ("DRAFT", "CONFIDENTIAL", "APPROVED"), custom text, configurable opacity, rotation, and foreground/background layering (`WATER-001`–`003`).
  - [x] Full transaction integration with bidirectional undo/redo and crash recovery journal rehydration (`TransactionCommand::ApplyBatesNumbering`, `ApplyHeaderFooter`, `SetBookmarks`, `ApplyStampOrWatermark`).
  - [x] Native macOS sheets (`BatesStampSheetView`, `HeaderFooterSheetView`, `BookmarkEditorSheetView`, `StampWatermarkSheetView`) and Android bridge (`DocumentCoreBridgeWorkbench.kt`).

- **M3.5 — Image Extraction, Compression, Optimization & Print/Export:**
  - Lossless extraction of embedded raster streams (JPEG, PNG, JBIG2).
  - Stream recompression, duplicate object removal, and Fast Web View linearization.
  - Professional print preparation and PDF/X verification.

- **M3.6 — Platform Integration, Usability & Overall M3 Acceptance:**
  - Complete native macOS AppKit/SwiftUI and Android Compose workbench interfaces.
  - Cross-viewer interoperability matrix (MuPDF, Apple Quartz, PDFium, `mutool`, `qpdf`).
  - Usability benchmarking against reference tools (PDFCraft, Adobe Acrobat).
  - Offline-first bundled user documentation and synchronized online help mirror (`HELP-002`).

- **M3.7 Track — Document Comparison & Diffing (`COMPARE-001`–`COMPARE-004`):**
  - Formal track established under approved `CR-0005` Option A following M3 visual workbench infrastructure.
  - Dual-document side-by-side synchronized viewport scroll.
  - Visual overlay differ with pixel/rendering comparison and text stream differ.
  - Exportable change report summary.

### Quality Gates

- Every page operation is an atomic, fully reversible `TransactionCommand`.
- Zero orphaned objects or dangling cross-references post-mutation (`mutool info`, `qpdf --check`).
- 60 FPS scrolling and responsive thumbnail generation on large documents ($\ge 500$ pages).
- All gates verified by genuine native execution; zero stubs (`QA-003`).

---

# M4 — Authoring / FlowDocument / Office *(Status: **PLANNED**; see [M3–M7 Plan](docs/project/M3-M7-DEVELOPMENT-PLAN.md) & [ADR-0020](docs/adr/ADR-0020-flowdocument-architecture-deterministic-rust-layout-and-pdoc.md))*

Detailed plan and acceptance criteria: [`docs/project/M3-M7-DEVELOPMENT-PLAN.md`](docs/project/M3-M7-DEVELOPMENT-PLAN.md).

## Sub-Milestones

- **M4.0 — FlowDocument Core Model & Schema (`core/vocument-flow`):**
  - Pure Rust semantic document tree: Sections, paragraphs, spans, heading hierarchies, nested lists, tables, images, callouts, footnotes, and Dublin Core metadata.
  - Complete schema validation and JSON serialization.

- **M4.1 — Deterministic Rust Layout Engine (`core/vocument-layout`):**
  - OpenType glyph shaping via `rustybuzz` (memory-safe HarfBuzz in Rust).
  - Unicode segmentation and bi-directional text (UAX #9) via `icu_segmenter` and `unicode-bidi`.
  - Dual line breaking: Fast greedy mode (typing) and Knuth-Plass optimal break mode (export/reading).
  - Cross-platform deterministic pagination guarantee: Given identical `.pdoc` and fonts, page layout is 100% bit-identical across macOS, Android, Windows, and Linux.

- **M4.2 — The `.pdoc` Format Specification & Container:**
  - Standard ZIP64 container (`manifest.json`, `document.json`, `styles.json`, `metadata.json`, `assets/`).
  - Schema versioning with unknown-field preservation for forward compatibility.
  - Safe extraction with zip-slip and decompression bomb protection.

- **M4.3 — Native Platform Editing Canvas & Input:**
  - macOS AppKit/SwiftUI view conforming to `NSTextInputClient` (CJK IME, accents, dictation, VoiceOver).
  - Android Jetpack Compose custom editor supporting touch, stylus, and Gboard IME.
  - Sub-16ms typing latency (60 FPS).

- **M4.4 — Reviewing System (Comments, Tracked Changes) & PDF/A-PDF/UA Generation:**
  - Tracked Changes: Inline insertions, deletions, format changes with author attribution.
  - Margin Comments: Character-range anchored threads with replies and resolution.
  - PDF/A & PDF/UA Export: Tagged PDF with semantic structure tree (`/StructTreeRoot`), semantic tags, and alt-text.
  - PAC (PDF Accessibility Checker) and veraPDF verification.

- **M4.5 — Office Interoperability: DOCX and ODT Bake-Off & Adapters:**
  - High-fidelity bake-off against 200 real-world legal and technical DOCX/ODT documents.
  - Lossless import into `FlowDocument` and valid export.
  - Strict rule: No generic "roundtrip editing" claims without corpus evidence.

### Quality Gates

- Deterministic pagination: `.pdoc` layout on macOS and Android matches bit-for-bit under identical font conditions.
- Output validated green by PAC and veraPDF.
- DOCX/ODT pass rate $\ge 95\%$ on structural test corpus.

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
- iPadOS/iOS licensing/distribution gate
- legal source modules
- CLI/automation server mode

---

# M5 — Windows Shell *(Planned, see `CR-0003`)*

**Entry criterion:** macOS and Android release candidates are green.

Deliver:

- `apps/windows` application shell on the idiomatic Windows toolkit
  (decision pending in future ADR upon Track 3 entry, `CR-0003`)
- `PlatformFileBroker` implementation on top of the Common Item Dialog
- `PlatformSettingsStore` implementation on top of the Windows user settings store
- `PlatformAppLifecycle` implementation for suspend/resume and shutdown
- native MuPDF build for `x86_64-pc-windows-msvc`
  (`scripts/prepare-windows-mupdf.ps1`)
- the shared acceptance scenario on Windows:
  `Open → search → select text → highlight → rotate → undo → redo → save → recover → reopen`

---

# M6 — Linux Shell *(Planned, see `CR-0003`)*

**Entry criterion:** the Windows track reaches an alpha.

Deliver:

- `apps/linux` application shell (decision pending in future ADR upon Track 4 entry, `CR-0003`)
- `PlatformFileBroker` implementation on top of the XDG file chooser portal
- `PlatformSettingsStore` implementation on top of the XDG configuration directory
- `PlatformFontProvider` implementation, which the `.pdoc` authoring milestone (M4)
  needs for deterministic pagination
- native MuPDF build for `x86_64-linux-gnu`
  (`scripts/prepare-linux-mupdf.sh`)
- the shared acceptance scenario on Linux

---

# M7 — Optional local interfaces (MCP)

**Status:** planned — the change request is in
[CR-0004](docs/cr/CR-0004-optional-local-interfaces-mcp.md)

A user may want to work with a document **from an assistant**: the assistant
reads the pages, searches in the document, marks a place that has to go,
writes a comment and saves the result. The **Model Context Protocol (MCP)**
is the interface that is used for that today; a local command line or a
local socket serves the same purpose.

**This milestone is optional in every sense.** The interface is not started
by default, it is not required for any function of the product, and the
product is complete without it. It runs **on the machine of the user** and
asks for **no network right** — the same rule as the applications.

**The rule that makes it safe: the interface is a shell around the existing
core, not a second implementation.** Every write goes through the core, and
the removal of a place is verified by the same four checks, with the same
refusal. An interface that could remove a text without the verification
would be a second, weaker path — and it is not built.

Deliver:

- the functions of the product as **tools**: open a document, read the
  pages, the words and the findings (links, actions, attachments,
  signatures), search, mark a place, remove a marked place, write a comment,
  save
- the **MCP server over stdio** as the first shell — thin, off by default,
  local, without an account, without telemetry and without a network right
  (E4)
- the **proof of the same workflow through the interface**: open → read →
  search → mark → remove → comment → save → reopen, with the same corpus
  that the applications use — and the proof that a **refused** redaction is
  refused through the interface as well, with the same reason
- the licence duties for the interface as well: it is part of the AGPL
  product, the notice of the third-party components covers it, and the
  source archive of a release contains it (E1–E3)

**Not in this milestone:**

- a function that is reached over the **network** — that needs a new
  architecture and a new licence review (E4, §8 of the licensing decision)
  and a different threat model
- the **macOS App Store** (deferred by the decision of the project owner) —
  the question of the sandbox of macOS is answered together with it
- **Android** — Android has no local stdio pipe for an assistant; a variant
  over a content provider or a bound service is a different design

The legal clearance (E5) remains the precondition of every publication, also
for the interface.

---

# Platform order

The platform order is **binding** (`project/project.yaml` → `platforms.track_order`,
`CR-0003`):

1. macOS 15+ (Apple Silicon) — active
2. Android 13+ (`arm64-v8a`) — active
3. Windows 11+ (`x86_64`) — planned
4. Linux (`x86_64`, Wayland and X11) — planned

A new platform track only starts when the previous one has a green release
candidate. The platform abstraction (`PLATFORM-003` in `REQUIREMENTS.md`) must exist
before the Windows track starts, so that the shared core does not grow
target-specific branches.

---

# Roadmap change rule

A milestone scope may change only through a documented Change Request if it affects accepted product requirements or release gates.

A coding agent must not silently move an unfinished item into a later milestone just to make a milestone “green”.

Furthermore, no milestone or feature deliverable may be declared complete or accepted on the basis of stubs, dummy returns, or mock bridges (`QA-003`, `CR-0002`). Acceptance strictly requires authentic, fully integrated native/embedded implementations with real runtime verification.

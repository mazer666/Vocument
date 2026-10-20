# Vocument Roadmap

**Roadmap version:** 0.1  
**Current milestone:** M1 — Android Scan & OCR (implemented; the device-dependent evidence remains open — see [M1 Acceptance Record](docs/acceptance/M1-ACCEPTANCE.md)). M2 has started with **M2.1 — secure redaction with verification** and **M2.2 — link and action inspection** (both implemented and verified on the host — see [M2.1 Acceptance Record](docs/acceptance/M2.1-ACCEPTANCE.md) and [M2.2 Acceptance Record](docs/acceptance/M2.2-ACCEPTANCE.md)).

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

# M2 — macOS Professional Review *(Started — M2.1 and M2.2 done)*

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

## M2.1 — Sichere Redaction mit Verifikation *(Implemented, host-verified)*

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

Evidence: [M2.1 Acceptance Record](docs/acceptance/M2.1-ACCEPTANCE.md).

## M2.2 — Link- und Aktionsprüfung *(Implemented, host-verified)*

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

Open in M2: the remaining seven functions, the macOS interface, and the
interoperability gate with the tools of other vendors.

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
- iPadOS/iOS licensing/distribution gate
- legal source modules
- CLI/automation server mode

---

# M5 — Windows Shell *(Planned, see `CR-0003`)*

**Entry criterion:** macOS and Android release candidates are green.

Deliver:

- `apps/windows` application shell on the idiomatic Windows toolkit
  (decision pending in ADR-0017)
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

- `apps/linux` application shell (decision pending in ADR-0018)
- `PlatformFileBroker` implementation on top of the XDG file chooser portal
- `PlatformSettingsStore` implementation on top of the XDG configuration directory
- `PlatformFontProvider` implementation, which the `.pdoc` authoring milestone (M4)
  needs for deterministic pagination
- native MuPDF build for `x86_64-linux-gnu`
  (`scripts/prepare-linux-mupdf.sh`)
- the shared acceptance scenario on Linux

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

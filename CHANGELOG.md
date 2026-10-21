# Changelog

All notable changes to Vocument are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- **The static proof of the Swift sources** (`scripts/check-swift-sources.py`,
  `scripts/check-swift-ast.mjs`): every Swift file is parsed with the real
  grammar of Swift, the names of the application are resolved against the
  generated bindings, and the check proves itself with a self-test that
  breaks the sources in seven ways. The check is part of the gate; a
  missing grammar is a `NOT RUN` with a loud note, never a green claim.
- **The build record** (`scripts/build-record.sh`): every gate run leaves a
  record of the revision, the tools, the counts of the tests, the gates and
  the source archive — also when a step of the gate fails.
- **The proof of the reproducibility** (`docs/releasing/REPRODUCIBILITY.md`):
  the archive of the revision, the build from the archive in a fresh
  directory, the comparison with the build of the working tree, and the
  limits of the environment that carried it out.
- **M2.1 — secure redaction with verification** (`docs/acceptance/M2.1-ACCEPTANCE.md`).
  The removal replaces the content instead of covering it, and the removal is
  verified four times: the text is gone, the search does not find it, the
  rendered page does not show it, and it is not in the bytes of the file. A
  removal that cannot be verified in every respect is **refused** — there is
  no way to treat an unverified redaction as a success.
- **M2.2 — link and action inspection**, **M2.3 — attachment and signature
  inspection** and **M2.4 — the comment of the reviewer with the rail of the
  profession** (`docs/acceptance/M2.2-ACCEPTANCE.md`, `M2.3`, `M2.4`).
- **The juristic palette is kept on the disk** (`jurist_palette.json`, `0600`
  in a `0700` directory, written atomically). A rail that was cut in half by a
  crash is an error with a name, never a silent fall back to the standard
  rail.
- **The identification of the build**: every binary names the source it was
  built from, and a build from a dirty tree is refused.
- **The release scripts** for the macOS bundle, the Android package and the
  archive of the source, with the notices and the offer of the source, and the
  **licence texts and the notice of the third-party components** in
  `LICENSES/`.
- **The privacy policy** (`PRIVACY.md`), the **licensing decision** E1–E5
  (`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`) and the **blockers
  of a public release** (`docs/releasing/RELEASE-BLOCKERS.md`).
- **The structural check of the Swift sources** (`scripts/check-swift-structure.py`).

### Fixed
- The **report of the places of a document was cut off** at 200 lines. The
  note about a place that the walk could not examine is written **last** — a
  document with 201 pages that carry the removed text pushed that note out
  of the report, and the redaction was reported as **verified** while a
  place of the document had never been looked into. The report grows with
  the document now, a report that does not fit is said to be incomplete, and
  an incomplete report is a refused redaction. The proof:
  `the_report_of_the_places_is_not_cut_off`,
  `a_place_that_the_walk_could_not_reach_is_named_even_in_a_long_document`.
- The **macOS application did not compile**: the review panel read
  `inspection.risyCount`, while the generated bindings know `riskyCount`.
  Six more defects of the workflow were found and fixed with it — the page
  was not drawn again after a removal (the user kept seeing the text that
  had just been removed for good), the reasons of a refused redaction were
  lost, the warnings of the previous document stood above the new one, the
  recovered document was never read, and the marked place belonged to no
  page: a rectangle of page 3 would have removed a text of page 5 that the
  user never saw in the preview.
- The **build record counted 105 tests instead of 255** — a greedy pattern
  ate the first digit of every two-digit number. A record that counts wrong
  is worse than no record.
- The **verification of a redaction could report a
  **success that it could not
  prove**: the walk stopped at its limit of the depth and at objects it
  could not read **without saying so**, and the encryption dictionary was
  not examined at all. The walk now writes down every place it could not
  examine, the redaction **refuses** and names the reason, and the keys of
  an encrypted document are seen by the check (and never emptied).
- The proof of a redaction no longer rests on the garbage collection of the
  writer: the product examines the document **before** the removal and
  keeps the list of the places that carried the text.
- The **fifth adversarial audit** of the redaction found that the clearing
  emptied the **`/U` of an encrypted document** when the encryption dictionary
  hangs as an indirect object — the form the specification asks for. The
  dictionary is recognised by its content now and is never descended into; the
  document stays encrypted and readable.
- The **fifth adversarial audit** found that the search for the remaining text
  compared **bytes**: a word that the writer split over several show
  operations (`(GE) (HEIM) Tj`) was not found, and the proof of the redaction
  was blind for it. The **text** of the content stream is read now.
- The **second, independent review** of the rail of the profession found that
  a link that points **nowhere** was taken for "no rail" — the application
  carried on with the standard rail, silently — and that the read path did not
  check the owner, so a rail of a stranger was read and used. Both are fixed,
  and the refusal of a path with `..` now hits the **name** of the file
  instead of the part the platform chose.
- The **fourth adversarial audit** of the redaction found that the walk over
  the cross-reference table handed the objects on as *direct* objects, so the
  content of a form object was never looked at: a copy of the removed text
  stayed in the document while the redaction reported `verified = true`. The
  walk descends into the dictionary of a stream now; the redaction **refuses**
  and names the place.
- The **macOS sources did not compile**: a mechanical split of the file had
  left fragments of methods without their head. The review workflow is one
  file again, and the gate checks the structure of the Swift sources.
- The **provisional "remove the first line"** is replaced by a **marked
  rectangle**: the user drags a place on the page, the core decides which text
  lies in it, the panel shows it before the removal.
- The **coordinate space of the page** is now measured by a test and not
  assumed: the text layer and the rendered picture live in the same space —
  the device space of the page, with the origin in the top left corner.
- The note of the reviewer carries the **name and the meaning** of the rail
  besides its key, so that it says what it means even if the rail is replaced
  years later.
- The **licence analysis of the Apple conditions** was verified against the
  actually applicable contract versions (`docs/audit/apple-terms-verification.md`):
  the applicable schedule is **Schedule 1** for a free application, the
  **Apple Media Services Terms** were added, and the wording that stated a
  legal verdict was replaced by the open question it really is.

## [0.1.0-m1.4] - 2026-10-19

### Added
- Completed **M1.4 — Searchable PDF Synthesis & Session Ingestion** (`docs/acceptance/M1.4-ACCEPTANCE.md`, `ADR-0015`).
- Implemented `vocument_ocr::synthesis` (`engines/ocr/src/synthesis.rs`, 384 lines, strictly complying with `CODE-001`):
  - Standard-conforming PDF 1.7 synthesizer generating document catalog, page tree, font resources (`/Helvetica` with standard WinAnsiEncoding), image XObjects, content streams, and cross-reference table.
  - Deflate stream compression (`flate2`) for scanned image raster (`/DeviceRGB` or `/DeviceGray`) and text content streams.
  - Invisible searchable text layer (`3 Tr` text rendering mode) perfectly positioned and aligned over the underlying scanned document image.
  - Precise horizontal text scaling (`Tz`) to calibrate word highlight rectangles to underlying scanned word widths.
  - Font baseline offset ($0.18 \times h_{\text{pt}}$) for exact vertical alignment of glyph ascenders and descenders.
  - Proper WinAnsi octal escape sequences for German diacritics (`ä`, `ö`, `ü`, `ß`) and Austrian legal markers (`§`).
  - Multi-page and single-page searchable PDF synthesis (`synthesize_searchable_pdf`, `synthesize_single_page_pdf`).
- Implemented zero-copy direct session ingestion:
  - `document_core_ingest_scan_as_document_session` in `platform/vocument-ffi/src/ocr_dto.rs` (425 lines).
  - Ingests synthesized PDF bytes into `DocumentSession` via `SessionRegistry::open_brokered_file` with `FileAccessOrigin::InMemoryBuffer`.
  - Immediate progressive SQLite FTS5 search indexing via `index_search_batch` and full-text querying with `search_exact` and snippet extraction.
- Hardened quality details:
  - Pinned native Android Tesseract/Leptonica ARM64 library SHA-256 checksums in `prepare-android-tesseract.sh`.
  - Calculated Android Levenshtein CER identically to the Rust gate in `AndroidOcrRuntimeGate.kt` ($\text{CER} = 1.02\% \le 5.0\%$).
  - Strictly asserted valid character glyph bounding boxes (`totalCharacters > 0`, `characterBoundingBoxesValid == true`) and German language detection on `emulator-5554`.
- Updated mobile UI workflow:
  - Added "Open as PDF" in `ScanCaptureScreen.kt` (586 lines) and connected `onDocumentSessionOpened` in `MainActivity.kt` to switch seamlessly to the Document Canvas with FTS5 search enabled.
- Comprehensive integration testing:
  - `engines/ocr/tests/synthesis_tests.rs` (283 lines): Verified PDF synthesis, `mutool info`, `mutool draw`, MuPDF `extract_page_text` roundtrip for Austrian legal terminology (`ABGB`, `UWG`, `Vertraulichkeit`, `Wien`), session ingestion, and SQLite FTS5 fulltext search with snippets.
  - `ScanCaptureBridgeTest.kt`: Added `testSearchablePdfSynthesisAndSessionIngestion`.
  - `VerifyKotlinHandshakeCli.kt`: Added M1.4 end-to-end verification step.
  - Enforced M1.4 Gate in `scripts/validate-repo.sh`.

## [0.1.0-m1.3] - 2026-10-18

### Added
- Completed **M1.3 — Tesseract OCR & Text Layer Generation** (`docs/acceptance/M1.3-ACCEPTANCE.md`, `ADR-0014`).
- Implemented modular `vocument-ocr` crate (`engines/ocr/`) with strict adherence to `CODE-001` (all source files $\le 600$ lines):
  - `provider.rs`: Abstract `OcrProvider` trait and structured `OcrError` taxonomy.
  - `models.rs`: `BoundingBox` (normalized $[0.0, 1.0]$ and pixel coordinates), `OcrCharacter`, `OcrWord`, `OcrLine`, `OcrBlock`, and `OcrPageResult`.
  - `tesseract.rs`: `TesseractEngine` adapter interfacing directly via native C API memory buffers (`TessBaseAPISetImage`), extracting blocks, lines, words, character glyphs, and confidence scores in a single iterator traversal.
  - `csrc/tesseract_bridge.c`: High-performance C bridge encapsulating Tesseract C API compiled for both macOS and Android ARM64 without stub bridges.
  - Native Android ARM64 dependencies deterministically resolved and extracted from AAR via Gradle task `extractTesseractNativeLibs` and `scripts/prepare-android-tesseract.sh` from a fresh clone without reliance on uncommitted `.so` files or local Homebrew headers.
  - Repo-internal Tesseract C API headers in `engines/ocr/include/tesseract/` ensuring 100% self-contained, reproducible cross-compilation across platforms.
  - Offline language models (`deu.traineddata`, `eng.traineddata`) extracted to the Android filesystem (`context.filesDir/tessdata`) via `TessdataProvisioner.kt` and configured directly into Rust core via `documentCoreConfigureTessdataPath`.
  - Hardened Android emulator gate (`scripts/verify-android-runtime.sh`) executing authentic on-device OCR on `emulator-5554` with strict hard failure on empty text, missing bounding boxes, or quality gate violations (verified: character accuracy 98.99%, keyword hit rate 100.0%, 92 words, valid bounding boxes).
  - Enforced reliable native rebuilds in `verify-android-runtime.sh` ensuring any native Rust/C OCR change automatically triggers incremental compilation and APK repackaging.
  - `language.rs`: Multi-language detection heuristics for German (`deu` with Austrian legal markers `de-AT`) and English (`eng`), umlaut handling, and bilingual classification.
  - `corpus.rs`: Realistic test corpus loader, PGM frame parser, and ground truth verification with Levenshtein-based CER/WER calculation and hardened $\ge 95\%$ character accuracy gate (zero 85% fallback).
- Created realistic scan and OCR test corpus (`tests/corpus/ocr/`):
  - `deu_vertrag_vertraulichkeit`: Austrian confidentiality agreement with Umlauts, paragraph symbols (`§`), and Austrian legal terminology (`ABGB`, `UWG`, `OGH`, `Wien`).
  - `deu_vertrag_vertraulichkeit_camera_distorted`: Realistic smartphone camera capture with 6.5° perspective tilt, illumination gradient, and mobile cast shadow.
  - `eng_mutual_nda`: English mutual NDA with commercial clauses and standard of care provisions.
  - `bilingual_rechnung_invoice`: Bilingual commercial invoice with line items, VAT calculations, and bank details.
- Integrated directly with M1.1 perspective warp and M1.2 cleanup pipeline: `recognize_clean_scan_page` executes OCR on `DocumentAuto` and `HighContrastBw` enhanced frames, verifying $\ge 95\%$ character accuracy.
- Harmonized M1.2 threshold inconsistencies across code, tests, and documentation to authoritative $\ge 90\%$ background uniformity, $\ge 80\%$ contrast ratio, $\ge 80\%$ shadow attenuation, and $\ge 245$ whitening.
- Exposed M1.3 OCR APIs across UniFFI (`platform/vocument-ffi/src/ocr_dto.rs`): `document_core_ocr_frame`, `document_core_ocr_clean_scan_frame`, `document_core_ocr_scan_session_page`, `document_core_detect_text_language`, `document_core_configure_tessdata_path`.
- Added Kotlin bridge methods in `DocumentCoreBridge.kt` (523 lines) and verified in `ScanCaptureBridgeTest.kt` (277 lines) and `VerifyKotlinHandshakeCli.kt` (598 lines).
- Verified authentic OCR on Android runtime (`emulator-5554`) and desktop Kotlin handshake CLI.
- Enforced M1.3 Gate validation in `scripts/validate-repo.sh`.
- Added **CR-0002 (`QA-003`)**: Formally instituted the strict prohibition of stubs, mock simulations, or dummy return values for any feature or milestone acceptance (`docs/cr/CR-0002-prohibition-of-stubs-in-acceptance.md`, `REQUIREMENTS.md` §43, `AGENTS.md` §§1, 5, 6, `QUALITY.md` §2, `ROADMAP.md`).

## [0.1.0-m1.2] - 2026-10-18

### Added
- Completed **M1.2 — Intelligent Scan Cleanup & Enhancement** (`docs/acceptance/M1.2-ACCEPTANCE.md`, `ADR-0013`).
- Implemented modular image cleanup in `engines/imaging/src/cleanup.rs` (581 lines) strictly complying with `CODE-001` (all source files $\le 600$ lines).
- Implemented adaptive 2D block-based background illumination estimation ($B(x, y)$ via 90th percentile grid with $3\times 3$ box smoothing and bilinear interpolation).
- Implemented shadow and crease removal filters via homomorphic background division ($> 80\%$ gradient attenuation).
- Implemented dedicated scan cleanup profiles:
  - `DocumentAuto`: Background whitening ($> 215 \to 255$), ink darkening ($< 80 \to \text{dark}$), power-law S-curve contrast, and color/hue preservation for color scans.
  - `HighContrastBw`: Adaptive Bradley/Sauvola threshold binarization ($18\%$ below local paper background) generating crisp binary output without shadow artifacts.
  - `PreservePhoto`: Continuous-tone photo enhancement with soft illumination flattening ($\gamma = 0.60$) preserving full dynamic range without clipping.
- Implemented quantitative visual regression metrics (`CleanupQualityMetrics` and `evaluate_cleanup_quality`: background uniformity $\ge 90\%$, contrast ratio $\ge 80\%$, shadow attenuation $\ge 80\%$, `is_binarized`).
- Resolved M1.1 gaps:
  - Added interactive draggable corner adjustment handles in Compose `ViewfinderOverlay` with 24dp active glowing indicator, manual override locking, and "Reset Auto Corners" control.
  - Converted Step 4 in `scripts/verify-android-runtime.sh` into a strictly binding runtime gate (no silent skipping if `adb` or device/emulator missing).
  - Updated `product_status` in `project/project.yaml` to `"m1-in-progress"`.
- Exposed M1.2 APIs across UniFFI (`platform/vocument-ffi/src/scan_dto.rs`, 479 lines): `document_core_enhance_scan_frame`, `document_core_get_enhanced_scan_page`, `document_core_evaluate_cleanup_quality`, `FfiCleanupQualityMetrics`.
- Added Kotlin wrapper methods in `DocumentCoreBridge.kt` (564 lines) and enhanced `ScanCaptureScreen.kt` (526 lines).
- Added comprehensive unit and regression tests in Rust (`cleanup::tests`) and Android Gradle (`ScanCaptureBridgeTest.kt`, `VerifyKotlinHandshakeCli.kt`).
- Enforced M1.2 Gate validation in `scripts/validate-repo.sh`.

## [0.1.0-m1.1] - 2026-10-18

### Added
- Completed **M1.1 — Perspective Correction, Deskew & Adaptive Rotation** (`docs/acceptance/M1.1-ACCEPTANCE.md`, `ADR-0012`).
- Modularized shared imaging engine (`CODE-001` adherence): separated geometric calculations into `engines/imaging/src/geometry.rs` (155 lines), projective warping and rotation into `engines/imaging/src/perspective.rs` (505 lines), keeping `engines/imaging/src/lib.rs` (511 lines) well below 600 lines.
- Implemented Heckbert 3x3 projective homography mapping from normalized destination unit square `[0, 1]^2` to arbitrary convex quadrilateral corners (`SCAN-003`).
- Implemented bilinear interpolation resampling across multi-format raw pixel arrays (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`) with dynamic aspect-ratio support.
- Implemented automated text baseline deskew estimation (`deskew_frame`): multi-angle horizontal line luminance variance search across $[-8.0^\circ, +8.0^\circ]$ with sub-pixel rotational bilinear resampling (`rotate_frame_fine`, `SCAN-004`, `SCAN-005`).
- Implemented discrete 90° clockwise rotation (`rotate_frame_90`) and automatic portrait orientation normalization (`auto_orient_frame`).
- Integrated session-level rectified frame caching in `ScanCaptureSession::append_page` and exposed on-demand retrieval via `document_core_get_warped_scan_page`.
- Exposed M1.1 APIs across UniFFI (`platform/vocument-ffi/src/scan_dto.rs`, 409 lines): `document_core_warp_perspective`, `document_core_deskew_frame`, `document_core_rotate_frame`, `document_core_auto_orient_frame`, `document_core_get_warped_scan_page`.
- Added Kotlin wrapper methods in `DocumentCoreBridge.kt` (539 lines) and integrated live rectification and deskew status feedback into `ScanCaptureScreen.kt` (404 lines).
- Added comprehensive unit tests in Rust (`perspective::tests`) and Android Gradle (`ScanCaptureBridgeTest.kt`), verifying perspective warping, deskew estimation, 90° rotation, and session warped page retrieval.
- Enforced M1.1 Gate validation in `scripts/validate-repo.sh`.

## [0.1.0-m1.0] - 2026-10-18

### Added
- Completed **M1.0 — Real Android Runtime Foundation & Camera Scan-Capture Pipeline** (`docs/acceptance/M1.0-ACCEPTANCE.md`, `ADR-0011`).
- Established genuine Android runtime test harness: Gradle 9.8.1 wrapper (`apps/android/gradlew`), Android SDK 35, Build-Tools 35.0.0, and automated APK assembly in `scripts/verify-android-runtime.sh`.
- Integrated CameraX 1.4.1 live preview (`CameraPreviewView.kt`, 154 lines) with real-time `ImageAnalysis` (RGBA8888, throttled frame extraction) providing genuine camera feed to shared Rust `ImagingEngine`.
- Cross-compiled native `libvocument_ffi.so` for `aarch64-linux-android` (Android NDK 29 Clang) and packaged into `apps/android/app/src/main/jniLibs/arm64-v8a/` (142 MB debug APK).
- Verified package metadata and manifest permissions via `apkanalyzer`: `org.vocument.app`, `android.permission.CAMERA` enabled, strictly zero network permissions (`NET-001`, strict offline-first).
- Executed authentic Android 14 emulator runtime verification (`emulator-5554` / `test_avd`): streamed APK install, granted `CAMERA`, launched `MainActivity` (cold start 257 ms, PID 3044), switched to Scan tab, performed live frame capture into Rust session, and inspected UI dump confirming session creation and zero fatal exceptions.
- Implemented shared Rust `ImagingEngine` (`vocument-imaging`): raw camera frame ingestion (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`), real-time document edge detection, geometric quadrilateral validation (`QuadCorners`, `Point2D`), and thread-safe multi-page scan session registry (`ScanCaptureSessionRegistry`).
- Exposed scan capture pipeline across UniFFI boundary (`platform/vocument-ffi/src/scan_dto.rs`): `document_core_detect_document_edges`, `document_core_start_scan_session`, `document_core_append_scan_page`, `document_core_get_scan_session`, `document_core_close_scan_session`.
- Implemented native Android Jetpack Compose UI (`ScanCaptureScreen.kt`): live camera viewfinder with real-time dynamic quadrilateral overlay (confidence color-coding), multi-page thumbnail reel, and enhancement profile selector (`DocumentAuto`, `PreservePhoto`, `HighContrastBw`).
- Added automated Android Gradle unit test `ScanCaptureBridgeTest.kt` verifying live edge detection and multi-page session lifecycle.
- Integrated `verify-android-runtime.sh` directly into `scripts/local-ci.sh`.
- Reconciled M0 documentation inconsistencies: corrected first-page latency requirement in `M0-ACCEPTANCE.md` and `ROADMAP.md` back to central configuration (target $\le 300$ ms, maximum 500 ms) and documented automated Apple Preview verification (`osascript` open/close + CoreGraphics/Quartz `sips`).

## [0.1.0-m0.7] - 2026-10-18

### Added
- Completed **M0.7 — Interoperability Gate** (`docs/acceptance/M0.7-ACCEPTANCE.md`, `ADR-0010`).
- Formally completed **Milestone M0: Foundation / Trustworthy Viewer** in full (`docs/acceptance/M0-ACCEPTANCE.md`).
- Implemented full M0 Definition of Done end-to-end workflow verification harness (`document_core_run_interop_gate_suite()`, `core/vocument-core/tests/interop_tests.rs`):
  `Open → search → select text → highlight → rotate → undo → redo → save → hard-kill → recover → save → foreign viewer reopen`.
- Implemented multi-save baseline rebinding invariant in `TransactionManager::mark_as_saved` and `DocumentSession::mark_as_saved` supporting sequential safe saves and crash recoveries.
- Automated foreign viewer cross-validation against MuPDF 1.28.5 (`vocument-pdf-mupdf`), reference CLI `mutool draw` / `mutool info`, Apple CoreGraphics / Quartz (`/usr/bin/sips`), and Chromium PDFium (`Google Chrome --headless --screenshot`).
- Established foreign viewer honesty policy: Adobe Acrobat Reader, PDF-XChange, and Firefox are checked and honestly documented as uninstalled on runner host without simulating false passes.
- Integrated M0.7 Interoperability Gate verification into macOS Swift handshake (`vocument-mac-handshake`) and Android Kotlin handshake (`VerifyKotlinHandshakeCli.kt`).
- Enforced M0.7 Gate validation in `scripts/validate-repo.sh`.

## [0.1.0-m0.6] - 2026-10-18

### Added
- Completed **M0.6 — Recovery** (`docs/acceptance/M0.6-ACCEPTANCE.md`, `ADR-0009`).
- Implemented per-document crash recovery containers (`vocument_recovery/<document_id>/`) storing versioned `manifest.json`, `journal.json`, and `checkpoints/` with atomic writes and SHA-256 integrity verification (`core/vocument-core/src/recovery.rs`).
- Integrated automatic mutation journaling with `session_mutations.rs` (`record_recovery_mutation`), capturing semantic operations and undo/redo without runtime overhead.
- Implemented crash detection via active OS PID tracking and atomic `is_clean_shutdown` marker flag.
- Implemented the 4-action recovery contract (`RecoveryAction` / `FfiRecoveryAction`: **Restore**, **Preview**, **Discard**, **Open as New**; `RECOVERY-001` Preset) and exposed it via UniFFI (`platform/vocument-ffi/src/recovery_dto.rs`).
- Enforced the **M0.6 Gate Invariant** (*Original file on disk remains completely intact until explicit user save*), verified via sub-process destructive hard-kill test (`SIGKILL`), integration tests, and Swift/Kotlin handshakes.
- Integrated native macOS UI sheets (`RecoveryPromptSheet` in `DocumentCanvasView.swift`, `VocumentMacApp.swift`) and Android bridge methods (`DocumentCoreBridge.kt`).

## [0.1.0-m0.5] - 2026-10-17

### Added
- Completed **M0.5 — Safe Save** (`docs/acceptance/M0.5-ACCEPTANCE.md`, `ADR-0008`).
- Implemented brokered atomic staging with sibling temporary files, explicit `sync_all` (fsync), atomic rename (`std::fs::rename`), and discard cleanup on failure (`core/vocument-core/src/file_broker.rs`, `core/vocument-core/src/save.rs`).
- Implemented post-save validation verifying byte count, `%PDF-` magic header, and structural PDF integrity (`SAVE-005`).
- Implemented Save As to alternative destinations preserving the original file (`SAVE-002`).
- Implemented multi-attribute `FileIdentity` comparison and pre-save external change detection (`FILELOCK-001`, `FILELOCK-002`).
- Enforced the **M0.5 Gate** (*Never silently overwrite externally changed content*): aborted saves on external modification, keeping on-disk files untouched and unsaved mutations in memory.
- Implemented structured save conflict contracts (`SaveConflict` / `FfiSaveConflict`) and 4-way resolution UI (collision-free safe copy, explicit overwrite, reload from disk, cancel) on macOS (`SaveConflictSheet`) and Android (`FILELOCK-003`, `FILELOCK-004`).
- Implemented collision-free safe-copy file path generation (`<stem>.safe-copy-YYYYMMDD-HHMMSS[-N].pdf`, `FILELOCK-004`).
- Exposed M0.5 safe save APIs over UniFFI (`platform/vocument-ffi/src/save_dto.rs`) and integrated native toolbar controls on macOS (`DocumentCanvasView.swift`, `VocumentMacApp.swift`) and Android (`DocumentCoreBridge.kt`).

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

# Milestone Acceptance Record — M1: Android Scan & OCR

- **Milestone:** `M1 — Android Scan & OCR` (`ROADMAP.md` §M1, `project/project.yaml`)
- **Status:** **PARTIALLY PASSED** — the functional requirements are proven on the host; the device- and platform-dependent evidence remains **NOT RUN**
- **Date:** 2026-10-20
- **Sub-milestones:** [M1.0](M1.0-ACCEPTANCE.md) · [M1.1](M1.1-ACCEPTANCE.md) · [M1.2](M1.2-ACCEPTANCE.md) · [M1.3](M1.3-ACCEPTANCE.md) · [M1.4](M1.4-ACCEPTANCE.md) · [M1.4b/M1.5](M1.4b-M1.5-COMPLETION-REPORT.md) · [M1.6](M1.6-ACCEPTANCE.md)
- **Final report:** `docs/reports/M1.6-COMPLETION-REPORT.md`

---

## 1. What M1 covers

M1 delivers the first mobile added value: **scan → image correction → OCR →
searchable, savable PDF**. The milestone comprises the sub-milestones M1.0 to
M1.6.

The pipeline in one sentence: `camera → edges → deskew → clean-up → OCR →
searchable PDF → full-text search → save`.

---

## 2. Result overview

### 2.1 Functional requirements

| # | Requirement | Platform | Environment | Evidence | Result |
|---|---|---|---|---|---|
| 1 | Read a camera frame (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`) | Host / Android | Rust | `cargo test -p vocument-imaging` | **PASS** |
| 2 | Detect the edges of the document | Host | Rust, corpus | `edges::edges_tests` | **PASS** (tolerance 0.05 against the reference) |
| 3 | Perspective deskew | Host | Rust | `perspective::tests` | **PASS** |
| 4 | Skew correction and automatic orientation | Host | Rust | `cargo test -p vocument-imaging` | **PASS** |
| 5 | Background clean-up, shadow removal, contrast | Host | Rust | `cleanup::tests` | **PASS** |
| 6 | OCR offline, ≥ 95 % character accuracy (German) | Host | Tesseract 5.3.0 | `cargo test -p vocument-ocr --test corpus_tests` | **PASS** |
| 7 | OCR with bounding boxes and confidence | Host | Tesseract | same | **PASS** |
| 8 | Searchable PDF with an invisible text layer | Host | MuPDF 1.28.5 | `cargo test -p vocument-ocr --test synthesis_tests` | **PASS** |
| 9 | Take a PDF into a `DocumentSession` | Host | Rust | `synthesis_tests` | **PASS** |
| 10 | Progressive SQLite-FTS5 search | Host | SQLite | `synthesis_tests` | **PASS** |
| 11 | **End-to-end workflow** camera → search → save | Host | everything | `platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` | **PASS** |
| 12 | Multi-page scan session | Host / Android | Rust, Kotlin | `cargo test`, `ScanCaptureBridgeTest` | **PASS** |
| 13 | Saving the scanned document (Android UI) | Android | Kotlin | code + `verify-android-runtime.sh` | **PASS (code) / NOT RUN (device)** |

### 2.2 Quality and operational requirements

| # | Requirement | Evidence | Result |
|---|---|---|---|
| 14 | The OCR does not block the UI thread | `ScanCaptureScreen.kt` (capture **and** PDF creation run on the executor) | **PASS (code) / NOT RUN (runtime)** |
| 15 | Background work is reliable, prioritised, cancellable | 36 job tests | **PASS** |
| 16 | Energy and thermal adaptation (`POWER-001`, `BIGMOBILE-001`) | `power::power_tests` | **PASS (host) / NOT RUN (device)** |
| 17 | Pause and resume, resumable jobs | `executor_tests` | **PASS** |
| 18 | The shutdown of the pool never blocks without a limit | `a_job_that_ignores_the_cancellation_does_not_block_the_shutdown_forever` | **PASS** |
| 19 | Difficult PDF files: no crash, no hang, clean errors | `engines/pdf-mupdf/src/difficult_tests.rs` | **PASS** (8 fixtures) |
| 20 | Memory consumption of the pathological corpus | `VmHWM` of the test process | **PASS** (5.9 MB → 8.3 MB) |
| 21 | No memory errors, no leaks | valgrind | **PASS** (0 errors, 0 bytes lost) |
| 22 | Android package: resolvable dependencies, 16 KB pages | `verify-android-native-libs.sh` | **PASS** |
| 23 | Release package checked | the same gate on the release APK | **PASS** (95 MB) |
| 24 | Only real engines, no dummies | `validate-repo.sh` (QA-003) | **PASS** |
| 25 | Gates: fmt, clippy, tests, validator | local run | **PASS** |

---

## 3. The chain of evidence that was added in M1.6

The end-to-end test
`platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` walks the **entire**
chain at the UniFFI boundary — with the same functions the app uses:

```
camera frame (grey, 1500×2050)
  → edge detection             (real, not a set value)
  → scan session                (append the page)
  → deskew + clean-up           (M1.1 / M1.2)
  → OCR                         (Tesseract, German)
  → searchable PDF              (invisible text layer)
  → session takeover            (DocumentSession)
  → FTS5 search                 ("Vertraulichkeit" found)
  → save to disk
  → open again                  (MuPDF)
  → the text in the saved PDF is readable again
```

Runtime: **4.2 s** on the host.

**Finding during the development:** the chain failed at the search step. The
cause was not the search, but the **edge detection**: it delivered an
axis-aligned rectangle instead of the tilted document edge, so the deskew was
wrong and the OCR text was unusable. The detection now works on the **page
contour** (boundary points per row and column, fitted with the robust Theil–Sen
estimator, intersection of the four edge lines) and delivers the corners within
0.014 of the reference.

---

## 4. What could **not** be verified

| Item | Status | Reason |
|---|---|---|
| Run on a real Android device | **NOT RUN** | No device attached. The emulator boots without `/dev/kvm` only in software emulation. |
| Emulator run (OCR quality, camera, interface) | **NOT RUN** | Same as above. |
| Responsiveness of the interface at runtime | **NOT RUN** | Requires a device. The correction is implemented and compiles. |
| Energy and thermal behaviour on the device | **NOT RUN** | Requires a device. The decision logic is fully checked on the host. |
| Battery and thermal behaviour | **NOT RUN** | No real hardware. |
| macOS build and integration tests | **NOT RUN** | No macOS host. The Swift bindings are generated, but never compiled. |
| AddressSanitizer | **NOT RUN** | Only with the nightly toolchain (`-Zbuild-std`). Substitute: valgrind memcheck. |
| CI run | **NOT RUN** | No workflow was executed. |

These items remain **open**. They are not presented as passed and they belong
to the preconditions of the next milestone.

---

## 5. Acceptance decision

M1 demonstrably meets its **functional** requirements: the complete mobile scan
workflow runs from the camera image to the saved, searchable PDF, with a real
engine, real detection and real search.

M1 does **not** demonstrably meet its **platform-dependent** requirements: the
run on the target device, the macOS integration and the behaviour under energy
and thermal pressure are not measured. Without this evidence M1 is **not**
completed.

**Decision: M1 stays open** until
1. a device or emulator with KVM is available for the on-device run,
2. a macOS host exists for the desktop track,
3. the licensing decision E1–E5 is made (see `docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`).

The scope of M1.6b is therefore factually complete; what is missing is
**environment and decision**, not implementation.

---

## 6. Follow-up after the acceptance (2026-10-20)

After the acceptance the state was checked once more against the requirements.
The check found four defects that were not yet reflected in the acceptance
record. They have since been fixed and proven:

| # | Defect | Fix | Evidence |
|---|---|---|---|
| 26 | A document from a document provider (`content://` URI) was treated by the local broker as a file path — a file with the URI name was created, and the document was not saved | The capability "can it be written back?" belongs to the broker (`FileBroker::can_write_back`); such a document is refused with `NotLocallyWritable` and written to a place of the user's choice via the export (`export_session_bytes`) | `core/vocument-core/tests/export_and_conflict_tests.rs`, `platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` |
| 27 | A conflict while saving (the file was changed externally) was invisible to the user in the Android interface | The interface evaluates the result and offers the four resolutions: copy, overwrite, reload, cancel | `MainActivity.kt`, `resolve_save_conflict` |
| 28 | The files of the application (staging, recovery container, manifest, journal) were created with the default permissions of the system | Staging `0600`, container `0700`, manifest and journal `0600` | `core/vocument-core/tests/export_and_conflict_tests.rs` (mode checked) |
| 29 | The camera was not released when the composition was left; the state of the interface was written from the camera thread | The camera is released in `onDispose`, the results are published on the main thread | `CameraPreviewView.kt`, `ScanCaptureScreen.kt` |

In addition, the **export into a file of the user** was added as a piece of
evidence in its own right: the exported document is a complete PDF, it can be
opened with the MuPDF engine, the OCR text is readable, and it contains
neither paths of the build machine nor internal identifiers.

The state after the follow-up: **141 tests, 21 suites, 0 failures**; clippy 0,
fmt 0, `validate-repo.sh` 0, Android unit tests 8/8 green, the native gate over
the package green. The **platform-dependent** evidence from section 4 remains
unchanged **NOT RUN** — the follow-up did not replace it.

The follow-up and the beginning of M2 are described in
`docs/reports/M1-FINALISATION-AND-M2-START.md`, the redaction in
`docs/acceptance/M2.1-ACCEPTANCE.md`.

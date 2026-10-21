# Final Report — M1: Android Scan & OCR

**Date:** 2026-10-20
**Branch:** `main`
**Acceptance:** `docs/acceptance/M1-ACCEPTANCE.md`
**Sub-milestones:** M1.0 … M1.6

---

## A. State reached

M1 delivers the mobile core function: **scan → image correction → OCR →
searchable, savable PDF**. The chain is now proven as **one** end-to-end test
at the UniFFI boundary, with a real engine, real edge detection, real OCR and
real full-text search:

```
camera frame → edges → deskew → clean-up → OCR → searchable PDF
             → session → FTS5 search → save → open again → text readable
```

Runtime **4.2 s** on the host.

### What this stage added

| Area | Before | After |
|---|---|---|
| Edge detection | axis-aligned rectangle, tilted pages stayed wrong | contour detection with a robust line fit, the tilt is captured |
| Throughput of the scan | the PDF creation ran **on the UI thread** | capture **and** PDF creation run on the executor |
| Saving | there was **no** save function in the Android UI | "Save PDF" with collision avoidance, the path is displayed |
| Background work | no energy rule | the core decides: background work rests in the heat and on an empty battery, the parallelism drops |
| Cancellation / resume | only cancellation | pause and resume, resumable jobs run again |
| Throughput proof | no complete chain | end-to-end test across the UniFFI boundary |
| Memory | not measured | peak memory and a valgrind run of the pathological corpus |

---

## B. The three findings that cleared the way

### B1 — The edge detection could not find a tilted page

The detection searched for the edges along the **centre line** of the image and
could therefore only deliver an **axis-aligned rectangle**. The page in the
corpus, however, is a sheared parallelogram; the deskew was therefore wrong
and the OCR text was unusable. The end-to-end test failed precisely there — at
the **search**, not at the search itself.

The detection now works on the **page contour**: for every row the outermost
cell, for every column the topmost respectively lowest cell; from that one
straight line per edge, with the **Theil–Sen estimator** (robust against up to
29 % outliers — the corners often lie in the shadow), and finally the
intersection of the four lines.

Result: the detected corners lie within **0.014** of the reference (tolerance in
the test: 0.05), the degree of the tilt is taken over correctly.

### B2 — The PDF creation blocked the interface

The capture path ran correctly on an executor. The **ingest path** (PDF
creation with OCR), on the other hand, ran directly in the click handler — on
the UI thread, for seconds. That contradicted the promise "the OCR does not
block the UI thread". The path now runs through the same executor; the
interface shows "Preparing PDF…" in the meantime.

### B3 — A hanging job could prevent the application from exiting

`shutdown()` waited for the workers without a limit. A job body that ignores
its cancellation token therefore also blocked the `Drop` of the executor — in
that state the application **could not exit**. The shutdown is now bounded
(`shutdown_grace_ms`, 2 s); a stuck worker is detached.

The finding did not come from a green test run, but from a **failed** one: the
panic triggered the `Drop`, the shutdown waited, the test process did not
terminate. A hanging test is a finding.

---

## C. State of the evidence

### Confirmed (PASS)

| Evidence | Result |
|---|---|
| `cargo test --workspace` | **132 tests, 0 failures** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| `cargo fmt --all -- --check` | **0** |
| `scripts/validate-repo.sh` | **0** |
| End-to-end scan workflow | **PASS**, 4.2 s |
| Edge detection on a tilted page | **PASS**, tolerance 0.05 |
| Energy and thermal logic | **PASS**, 10 tests |
| Pause and resume | **PASS**, 3 tests |
| Difficult PDFs (8 fixtures) | **PASS** |
| Peak memory of the corpus | **5.9 MB → 8.3 MB** |
| valgrind memcheck | **0 errors, 0 bytes lost** |
| Android Kotlin compilation | **PASS** (`compileDebugKotlin`) |

### Not verified (NOT RUN)

| Evidence | Reason |
|---|---|
| Run on a real device | No device; the emulator boots without `/dev/kvm` only in software emulation |
| On-device OCR quality | Same as above |
| Responsiveness of the interface at runtime | Requires a device |
| Energy and thermal behaviour on the device | Requires a device |
| macOS build and integration tests | No macOS host |
| AddressSanitizer | Only with the nightly toolchain (`-Zbuild-std`); substitute: valgrind |
| CI run | No workflow executed |

These items are **open** and are not carried as passed.

---

## D. Inconsistencies that were removed

| Inconsistency | Correction |
|---|---|
| The headline of the roadmap named M1.5 as "next", although M1.5 was already implemented | Headline, status of M1.5 and the definition of M1.6 updated |
| Two different "M1.6" (platform validation and Android gate) | merged into **M1.6a** (Android gate) and **M1.6b** (platform validation) |
| The size of the APK was given as 23 MB in the roadmap | measured values (250 MB debug, 95 MB release) |
| `project.yaml` named an open "release_package_inspection", although the package had been checked | test bench and open items updated |
| The interop test looked for `mutool` only at hard-coded paths | search via `MUPDF_DIR`, the same mechanism as the build |
| `REQUIREMENTS.md` did not know the new promises | `SCAN-009`, `SCAN-010`, `JOB-011`, `UI-010`, `POWER-001` specified |
| The toolchain paths were hard-wired in the build | the Tesseract cross build recognises a changed NDK and rebuilds |

---

## E. Remaining blockers

| Blocker | Kind | Owner |
|---|---|---|
| No Android device and no `/dev/kvm` | Environment | Provisioning of a device or a KVM-capable runner |
| No macOS host | Environment | macOS runner for the desktop track |
| Licensing decision E1–E5 open | Decision | Project owner |
| CI not executed | Infrastructure | A runnable workflow runner |
| AddressSanitizer only with nightly | Tooling | Nightly toolchain in the build image |

None of these blockers is a question of code.

---

## F. Recommendation for the next milestone

**M1.7 — battery and thermal adaptation on the device** (`BIGMOBILE-001`,
`POWER-001`), as soon as a device is available. The decision logic exists and
is checked; what is open is exclusively the measurement on the target.

After that follows **M2 — macOS Professional Review**: the macOS track needs a
host, and the review functions (comments, highlights, stamps, redaction,
comparison) are the larger step in terms of substance. The Windows/Linux track
stays blocked until then, because the order macOS → Android → Windows → Linux
is binding.

**Not** recommended: a migration of the engine (PDFium). It would be a
reaction to a licensing decision that has not been made and would weaken the
PDF mutation (rotation, annotations) without solving a problem that has not
already been decided.

# Vocument for newcomers — a guide

This document answers the questions that come up when you start working on the
project. It is deliberately short and points to the places where the answers
are maintained.

**If you want to change something, read `WHERE_TO_CHANGE.md` first.** It names
the right entry point for every kind of change.

---

## 1. How is the project structured?

```
Swift-UI (macOS)  ─┐
                   ├─→ UniFFI (platform/vocument-ffi) → Rust core (core/vocument-core)
Kotlin-UI (Android)─┘                                          │
                                                              ├─→ engines/pdf-mupdf  (PDF engine)
                                                              ├─→ engines/ocr        (Tesseract)
                                                              └─→ engines/imaging    (image processing)
```

Four ground rules that are never broken:

1. **The interface never talks to MuPDF directly.** Every PDF call runs
   through the Rust core. `scripts/validate-repo.sh` checks this (gate M0.1).
2. **The core is platform independent.** There is no `#[cfg(target_os = …)]`
   in the core outside the planned platform traits.
3. **Changes to the document run through the transactions.** No direct write
   into the PDF byte buffer.
4. **New dependencies need a justification.** The core gets by without an
   external Rust framework.

The binding source for the milestones is `project/project.yaml`.

---

## 2. Where do I change the number of parallel workers?

**`config/performance-budgets.toml`, section `[jobs]`:**

```toml
[jobs]
max_concurrent_workers = 4      # worker threads in total
max_queued_jobs = 64            # length of the queue
starvation_promotion_ms = 250   # waiting time until the priority is raised
idle_shutdown_timeout_ms = 500  # waiting time of an idle worker
```

The file is embedded into the Rust core (`core/vocument-core/src/config.rs`,
`PerformanceBudgetsConfig::load_embedded`). After a change the core has to be
rebuilt.

**Rule:** No feature may build its own thread pool. Everything runs through
the `JobExecutor` (`core/vocument-core/src/jobs/executor.rs`).

---

## 3. Where do I configure rendering and OCR?

| Topic | File |
|---|---|
| Render budgets, cache limits, tile sizes | `config/performance-budgets.toml` |
| Job budgets (workers, queue) | `config/performance-budgets.toml`, `[jobs]` |
| OCR language models (offline) | `apps/android/app/src/main/assets/tessdata/` |
| Tesseract libraries for Android | `scripts/prepare-android-tesseract.sh` |
| MuPDF library for Android | `scripts/prepare-android-mupdf.sh` |
| Engine adapter (PDF) | `engines/pdf-mupdf/src/lib.rs` |

---

## 4. How does the Android library integration work?

The Android build puts **four native libraries** into the package:

| Library | Origin | Purpose |
|---|---|---|
| `libvocument_ffi.so` | Rust, NDK cross build | The FFI boundary |
| `libmupdf.so` | MuPDF 1.28.5, NDK cross build | The PDF engine |
| `libtesseract.so` | Tesseract 5.3.0, NDK cross build | The OCR engine |
| `libleptonica.so` | Leptonica 1.82.0, NDK cross build | Image processing behind Tesseract |

Two rules are **mandatory** here, otherwise the application does not start:

1. **The SONAME must be unversioned.** The Android loader resolves a
   `DT_NEEDED` entry by the **exact file name**, and the packaging only picks
   up files with the extension `.so`. A `libmupdf.so.28.5` is never found by
   the loader.
2. **Every `PT_LOAD` segment must be aligned to at least 16 KB**
   (`-Wl,-z,max-page-size=16384`), otherwise the library does not start on
   devices with 16 KB pages.

`scripts/verify-android-native-libs.sh` checks both. That run is **binding**.

---

## 5. How do I reproduce an Android build?

```bash
# 1. Tools (JDK 17, Android SDK 35, NDK r27c)
export ANDROID_HOME=$PWD/target/android-sdk
export ANDROID_NDK_HOME=/path/to/ndk

# 2. Native libraries + Rust build
./scripts/build-android-rust.sh

# 3. The complete package
cd apps/android && ./gradlew assembleDebug
```

The result is in `apps/android/app/build/outputs/apk/debug/`.

**Important:** `scripts/build-android-rust.sh` always rebuilds when the Rust
sources change and puts the native libraries into
`apps/android/app/src/main/jniLibs/arm64-v8a/`. The directory is **emptied**
before the deployment, so that no old version ends up in the package.

For a release build `BUILD_PROFILE=release` is set in addition; the build
then removes the debug information (roughly 90 MB smaller).

---

## 6. How do I run all the tests?

```bash
export MUPDF_DIR=/opt/mupdf-shared          # path to the host MuPDF installation
export LD_LIBRARY_PATH=$MUPDF_DIR/lib
export TESSERACT_DIR=/usr
export TESSDATA_PREFIX=$PWD/apps/android/app/src/main/assets/tessdata

cargo test --workspace                      # Rust: all suites
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
./scripts/validate-repo.sh                  # project governance
```

In addition, depending on the environment:

| Script | What it checks | What happens without it |
|---|---|---|
| `scripts/verify-android-arm64-pdf.sh` | The PDF engine runs a real PDF under ARM64 (image comparison) | `NOT RUN` |
| `scripts/verify-android-native-libs.sh` | Resolvability and 16 KB alignment of the native libraries | `NOT RUN` |
| `scripts/verify-android-native-libs.sh --apk <apk>` | the same for the libraries **in the package** | `NOT RUN` |
| `scripts/verify-android-runtime.sh` | The application on a device/emulator | `NOT RUN` (binding for the acceptance) |
| `scripts/verify-kotlin-handshake.sh` | The Kotlin side of the FFI boundary | `NOT RUN` |

---

## 7. How do I recognise that a test ran on real hardware?

**The level of evidence stands in the result.** There are exactly four levels:

| Level | Meaning | Example |
|---|---|---|
| `HOST VERIFIED` | Executed on the build machine | The Rust tests, the host MuPDF comparison |
| `QEMU VERIFIED` | The ARM64 binary ran under `qemu-aarch64` on the host | `scripts/verify-android-arm64-pdf.sh` |
| `EMULATOR VERIFIED` | Executed in an Android emulator | `scripts/verify-android-runtime.sh` with an emulator |
| `PHYSICAL DEVICE VERIFIED` | Executed on a real device | the same script with a device attached |

**Rules:**

* A QEMU result is **not** a device result. It always stands as `QEMU VERIFIED`.
* An emulator result is **not** a device result. It stands as `EMULATOR VERIFIED`.
* Anyone claiming a device test names the device model used and the `adb`
  output of `adb devices`.
* Every piece of evidence names: **platform, test environment, command
  executed, relevant test result.**

The acceptance records under `docs/acceptance/` use these levels and the
status values `PASS`, `FAIL`, `NOT RUN`, `BLOCKED`.

---

## 7a. Where do I change the edge detection and the energy control?

| Topic | File |
|---|---|
| Edge and contour detection | `engines/imaging/src/edges.rs` |
| Energy and thermal limits | `core/vocument-core/src/jobs/power.rs` |
| Reporting the device state from the app | `apps/android/app/src/main/java/org/vocument/app/AndroidPowerReporter.kt` |
| Pause and resume | `core/vocument-core/src/jobs/executor.rs` (`pause`, `resume`) |

**The energy control deliberately lives in the core.** The app only reports
the state of the device (`document_core_report_power_state`), the core decides
what may run. That way the same rule applies on macOS and on Android, and the
decision can be checked without a device.

## 7b. Where do I change the redaction?

| Topic | File |
|---|---|
| The redaction with verification in the core | `core/vocument-core/src/redaction.rs` |
| The rules of the redaction (the comparison form, the fragment, the region) | `core/vocument-core/src/redaction_rules.rs` |
| The removal of the content in the MuPDF engine | `engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c` (`vocument_mupdf_redact_page_regions`) |
| The cleaning of the annotations, the form values and the metadata | `engines/pdf-mupdf/csrc/mupdf_scrub_bridge.c` (`vocument_mupdf_scrub_document`) |
| The search for the places that still carry the text | `engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c` (`vocument_mupdf_find_text_occurrences`) |
| The word boxes of the text layer | `engines/pdf-mupdf/csrc/mupdf_text_bridge.c` |
| The redaction across the FFI boundary | `platform/vocument-ffi/src/redaction_dto.rs` |
| Transaction and undo | `core/vocument-core/src/transactions.rs` (`RedactPageRegions`, `Irreversible`) |

**Twenty traps that have already bitten once:**

1. **A redaction that only covers is not a redaction.** The text has to be
   removed from the document. MuPDF offers `PDF_REDACT_TEXT_REMOVE` for that;
   it also removes the invisible text layer of a scan.
2. **The removed text can stay in the file.** MuPDF writes unreachable objects
   into the file when the garbage collection is not requested
   (`opts.do_garbage = 1`). The test therefore checks the bytes of the file,
   not only the text extraction.
3. **The recovery holds older states.** The checkpoints of the recovery store
   are deleted with the redaction, otherwise the content would be restorable
   from there.
4. **A document carries the same text twice.** As a text layer and as pixels
   in the image. MuPDF offers `PDF_REDACT_IMAGE_PIXELS` for that: the affected
   pixels are replaced, the rest of the image survives.
   `PDF_REDACT_IMAGE_REMOVE` would delete the whole image and thereby destroy
   the page.
5. **A black box over the region makes the check worthless.** It covers
   exactly what the check has to see. The redaction therefore draws nothing
   over the region (`black_boxes = 0`); the picture of the page shows the
   truth.
6. **The removal works character by character.** A region that covers only a
   part of a word leaves a fragment behind, and a fragment is still content.
   The verification therefore compares fragments and checks the region itself.
7. **A turned page has its corners in another order.** The box of a word is
   the bounding box of all four corners of all of its characters — built from
   the upper-left and the lower-right corner alone, it comes out inverted on a
   page that is turned by 90 degrees, and the redaction then removes nothing.
8. **The text survives outside the page.** The text of an annotation, the
   value of a form field, the metadata and the XMP packet are not touched by
   the redaction of MuPDF — and the text does not have to be plain text: it
   can be stored as UTF-16 or as the hex string of the UTF-16 bytes. The
   engine cleans those places and reports what it cannot clean.
9. **A failed redaction must change nothing.** The removal happens before the
   verification, so a verification that fails has to put the document back —
   otherwise the user holds a changed document and believes that nothing
   happened. The checkpoints of the recovery store are only deleted after a
   successful verification.
10. **The same word can stand somewhere else.** The redaction removes what
    the user marked — a region of one page. A word that occurs on another
    page is not part of the redaction, and refusing the redaction because of
    it would make every document with a word that occurs twice impossible to
    redact. The report names those places (`remaining_occurrences`), so that
    the shell can show them: the word is still in the document, only not
    under the marked region. A remainder in the bytes of the file **without**
    a known place — an old version of the file, an unreachable object — is
    something else: that one stops the redaction.
11. **A marked region has to take the whole word.** The removal works
    character by character, so a region that covers the half of a word
    leaves the rest of the word behind — and the rest of a secret is still a
    secret. The rule is: a word that the region **touches** belongs to the
    redaction, and the region grows to the box of the whole word. The words
    of the picture count as well: a scan has no text layer, and its pixels
    are the content there.
12. **An error after the removal has to put the document back.** The removal
    happens before the verification, and every error in the verification —
    the text extraction, the reindexing, the rendering of the page — must
    leave the document as it was. Otherwise the user holds a changed
    document and believes that nothing happened. A rollback that fails
    itself is reported as its own error: the session then holds a document
    in an unknown state.
13. **A journal cannot say whether a redaction was verified.** The crash
    recovery therefore does not replay a redaction: the document is restored
    to the state before it, and the recovery says so.
14. **The checkpoints of the recovery store hold the document in plain
    text.** They belong to the user alone: the directory is `0700`, the
    file `0600`. A file that anybody on the machine could read would be a
    second copy of the document outside of the place the user expects it.
    A checkpoint that an older version of the application has written keeps
    its old mode until the next one is written — so the next one makes it
    private again.
15. **A list of known places is not a search.** The redaction has to look
    everywhere: a popup that does not stand in `/Annots`, the structure
    tree, the name tree, the optional content, the private data of a page
    — a secret that stands in one of those places survives, and the
    redaction reports a success. Both the search and the cleaning walk
    through every object of the document
    (`engines/pdf-mupdf/csrc/mupdf_walk.h`).
16. **A place that cannot be read is not a clean place.** The content of an
    attached archive is compressed. The redaction names such a place and
    does not claim that the text is gone from there; an attachment is
    never deleted.
17. **Two boxes that only share their edge do not overlap.** The region
    takes every word it really covers, not every word that stands next
    to it.
18. **The cache keeps what the page showed.** The tiles and the thumbnails
    of a redacted page still hold the removed text. They are dropped after
    a redaction.
19. **The clipboard is the copy of the user.** The text of the page is
    copied into the clipboard, and the clipboard history of the system
    keeps it. The redaction surface has to clear the clipboard — it does
    not exist yet.
20. **A directory in the temporary directory can be prepared by somebody
    else.** The recovery store keeps its content private (`0700`/`0600`),
    but the applications have to hand it a directory of their own.

**The coordinates:** the boxes lie in the page space of the engine —
milli-points, origin at the top left, y descending. That is the same space in
which the page is drawn. `PdfRectMilliPt` in
`core/vocument-core/src/pdf_engine.rs` describes it.

**What happens when the redaction fails:** the document keeps the bytes it
had before the attempt, the search index is built again, and the caller gets
an error that says that nothing changed. The recovery points of the document
stay untouched.

Evidence: `docs/acceptance/M2.1-ACCEPTANCE.md`, the audits in
`docs/audit/`.

## 7c. How do I inspect the links and the actions of a document?

A link in a document can open a page, call up an address, start a program,
submit a form — or run a script. For a professional review both matter: where
the link leads and what it does.

**Three places, in this order:**

1. **The engine reports the facts** — `engines/pdf-mupdf/csrc/mupdf_inspect_bridge.c`
   reads the actions of the document and of the pages.
2. **The core decides the risk** — `core/vocument-core/src/document_inspection.rs`.
   The policy is a product decision and therefore belongs in the core, not in
   the engine and not in the platform.
3. **The FFI boundary makes the report available** —
   `platform/vocument-ffi/src/inspection_dto.rs`, `document_core_inspect_document`.

**Two traps that have already bitten once:**

1. **MuPDF does not keep link annotations in the annotation list of the
   page.** `pdf_sync_annots` skips `/Link` and `/Popup`; they are loaded
   separately as clickable areas. Over `pdf_first_annot` one therefore finds
   **nothing**. The bridge reads the `/Annots` array of the page directly.
2. **Only that way is the type of the action known.** The loaded link areas
   carry only the target, not the kind of action. Exactly the information
   that matters — "this is a script, not a link" — would otherwise be lost.

The catalogue is not the trailer, but the `/Root` of the trailer.

Evidence: `docs/acceptance/M2.2-ACCEPTANCE.md`.

## 7d. How do I inspect the attachments and the signatures of a document?

**Two traps that have already bitten once:**

1. **The MIME type of an embedded attachment has to be masked.**
   `/Subtype /application/octet-stream` is wrong — the slash ends the name in
   a PDF, the rest is read as a new key. MuPDF reports
   `invalid key in dict`, and the attachment is missing **without** an error
   in the document. Correct: `/Subtype /application#2Foctet-stream`.
2. **The change check needs the field, not the signature.**
   `pdf_signature_incremental_change_since_signing` checks itself whether the
   passed object is a signature field and throws otherwise
   (`annotation is not a signature widget`). The byte range is read through
   the value of the field.

**Who decides what:** the **engine** reads the byte range of the signature and
compares it with the size of the file — only it can do that. The **core**
decides what that means: a document that was modified after the signature
carries a signature that no longer covers anything.

Evidence: `docs/acceptance/M2.3-ACCEPTANCE.md`.

---

## 8. Where do I add new kinds of jobs?

Three places, in this order:

1. **The kind of job itself** — `core/vocument-core/src/jobs/scheduler.rs`,
   variant `JobKind`. Determine its class (`Interactive`, `UserInitiated`,
   `Background`) so that the priority is right.
2. **The real work** — `platform/vocument-ffi/src/job_dto.rs`, in
   `build_job_task()`. A kind of job **without** a core operation reports
   `Failed`; it may never pretend success (`JOB-010`).
3. **A test** — `core/vocument-core/tests/job_manager_e2e_tests.rs` (the way
   through the layers) and/or
   `core/vocument-core/src/jobs/executor/executor_tests.rs` (the behaviour of
   the pool).

The check in `scripts/validate-repo.sh` fails when a kind of job is submitted
without an operation.

---

## 9. How does cancellation work?

1. Every job receives a **`CancellationToken`** when it starts.
2. `JobExecutor::cancel_and_signal(job_id)` sets the job to `Cancelled` and
   **signals** the token. The token is reachable for a job that is currently
   running.
3. The job body checks the token at a **safe point** and returns
   `JobOutcome::Cancelled`.
4. **In addition** the tracker discards a result that arrives after the
   cancellation: a cancelled job **never** reports `Completed`.
5. When the **document is closed** all jobs of the session are cancelled.
6. When the **pool is shut down** the tokens of all running jobs are signalled
   before the worker threads are joined.

**Important:** a thread is **never** forcibly terminated. An engine call that
cannot be interrupted (MuPDF, Tesseract) runs to the end — its result is
discarded afterwards.

The shutdown is nevertheless **bounded**: after `shutdown_grace_ms` (default
2 s, see `config/performance-budgets.toml`) a worker that is still stuck in a
job is **detached** instead of being waited for any longer. Without this limit
a hanging engine call could block the `Drop` of the executor and thereby
prevent the application from exiting.

**The job body must observe the token all the same** — the deadline is the
last resort, not the plan.

---

## 10. Where do I find the risks that are still open?

**`docs/project/RISKS.md`** — the single source. Every line has: ID,
description, probability, impact, countermeasure, responsible role, status.

The most important open items:

| ID | Topic |
|---|---|
| `RISK-007` | Job pause/resume and job dependencies are missing (a task graph is needed) |
| `RISK-009` | The AGPL source disclosure for the Android distribution is not decided |
| `RISK-010` | The package size (measures described under `RISK-010`) |

The licensing question is worked out in detail in
`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`.
**The decision is made by the project owner, not by the implementation.**

---

## 11. Which changes must I not forget before a release?

**Checklist before every release:**

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` are green
- [ ] `./scripts/validate-repo.sh` is green
- [ ] `./scripts/verify-android-native-libs.sh` is green (**binding**: otherwise the app does not start)
- [ ] `./scripts/verify-android-native-libs.sh --apk <apk>` is green (**binding**: it checks the libraries *in the package*, not only in the build directory)
- [ ] `./scripts/verify-android-arm64-pdf.sh` is green (the engine renders a real PDF)
- [ ] `./scripts/verify-android-runtime.sh` is green (**binding**: a real device or emulator)
- [ ] The generated APK was **unpacked and checked** (`unzip -l app.apk | grep lib/`): all four native libraries are present, none is duplicated, none carries an old timestamp
- [ ] The size of the release build is measured and documented
- [ ] The licensing decision is made and the source disclosure is implemented (`RISK-009`)
- [ ] `project/project.yaml` and `ROADMAP.md` agree with the actual state
- [ ] For every evidence criterion there is an entry with `PASS` / `FAIL` / `NOT RUN` / `BLOCKED`, **platform**, **environment**, **command** and **result**

**Do not publish before that:** No package without a green
`verify-android-native-libs.sh`. No package without a green
`verify-android-runtime.sh`. No release as "approved" while `RISK-009` is
open.

---

## 12. Glossary

| Term | Meaning |
|---|---|
| **Actor** | The serialised access to a document. All mutations run through it. |
| **Display list** | The immutable list of the drawing commands of a page. It is computed once and can then be replayed any number of times. |
| **Tile** | A piece of a page in a defined resolution. |
| **Transaction** | An atomic change to the document with undo/redo. |
| **UniFFI** | The interface between Rust and Swift/Kotlin. |
| **SONAME** | The name under which a dynamic library is looked up by the loader. |
| **DT_NEEDED** | The entries of a dynamic library that demand other libraries. |

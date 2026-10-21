# Final Report — Follow-up to M1 and the beginning of M2

**Date:** 2026-10-20
**Branch:** `main`
**Acceptance:** `docs/acceptance/M1-ACCEPTANCE.md`, `docs/acceptance/M2.1-ACCEPTANCE.md`

---

## A. What it was about

The task was to remove the remaining product-relevant defects of M1, to
complete the evidence for real documents, to remove the contradictions between
code, requirements and acceptance documentation — and then to begin M2 as far
as the environment allows real implementation and real verification.

## B. The defects that were removed

### B1 — A document from a document provider became a nonsense file

**Finding.** A document that the user opens via the Android Storage Access
Framework has no local address, but a `content://` URI. The local file broker
treated the URI like a path: from
`content://com.android.providers.document/1` a file named `1` was created in
the working directory. The actual document was not saved.

**Cause.** The question "can this document be written back?" was coupled to
the *origin* of the document (`FileAccessOrigin`). That is the wrong question:
it belongs to the *broker*, because only the broker knows whether it can
resolve an address. The coupling to the origin would also have broken the
macOS path — there a document is local via a `file://` address, although the
origin is called `MacOsScopedUrl`.

**Fix.** `FileBroker::can_write_back` answers the question. The local broker
recognises URIs with a scheme that is not `file` and refuses the write-back
with the typed error `NotLocallyWritable`. `stage_write` rejects such targets
in addition — the last line of defence, so that a file with a URI name never
comes into being.

**Consequence for the user.** The platform shell writes the document to a
place that belongs to the user: `document_core_export_session_bytes` hands
over the bytes, the Android shell writes them via `ACTION_CREATE_DOCUMENT`
into the document provider. The user chooses the place, the document lands
there.

### B2 — An externally changed file was overwritten without asking

**Finding.** The Android UI did not evaluate the result of a save. A
conflict — the file had been changed in the meantime by another program —
stayed invisible to the user.

**Fix.** The interface evaluates the result and shows the four resolutions in
the case of a conflict: save as a collision copy, overwrite explicitly, reload
the external version, cancel. The resolution runs via `resolve_save_conflict`
in the core.

### B3 — The files of the user were readable for others

**Finding.** The staging file of a save, the recovery container with manifest
and journal were created with the default permissions of the system. They
contain the document of the user respectively its path and file name.

**Fix.** Staging file `0600`, recovery container `0700`, manifest and journal
`0600`. The mode is checked by a test, not merely claimed.

### B4 — The camera stayed bound

**Finding.** The camera was not released when the composition was left. It
stayed active: the privacy indicator of the system stayed on, the camera
consumed power. In addition the camera thread wrote the state of the
interface — the Compose state was changed from a background thread.

**Fix.** The camera is released in `onDispose`. The edge detection still runs
in the camera thread (it is a native computation), but the results are
published via the main thread.

## C. The evidence for real documents

Two tests at the UniFFI boundary with a real engine, real edge detection, real
OCR and real full-text search:

1. **The scan workflow** (unchanged green): camera frame → edges → deskew →
   clean-up → OCR → searchable PDF → session → FTS5 search → save → open
   again → text readable.
2. **The export** (new): the same document, exported into a file that belongs
   to the user. The file is a complete PDF, it can be opened with the MuPDF
   engine, the OCR text is readable, and it contains neither a path of the
   build machine nor internal identifiers.

In addition in the core: a document from a document provider is **not** saved
via the local broker, the export delivers the bytes, and the file permissions
of staging and recovery are checked.

**Test state:** 141 tests, 21 suites, 0 failures. clippy 0, fmt 0,
`validate-repo.sh` 0, debug APK 108 MB built, the native gate over the package
green (closure, SONAME, ARM64, 16 KB pages).

## D. M2 — first slice: secure redaction with verification

M2 demands nine functions for the professional macOS workflow. Eight of them
need a macOS interface and a macOS host. The **redaction** is the part that is
security-critical — and the part that can be proven on the host with the same
MuPDF engine that also runs on the device.

**The problem.** A redaction that lays a black rectangle over the text is not
a redaction: the text stays in the file stream, it stays extractable, it
stays findable through the search. For a scan there is an addition: the text
layer is invisible (`3 Tr`) — it has to be removed as well.

**The implementation.** The engine removes the content of the region (text,
invisible text, images, line art) with `PDF_REDACT_TEXT_REMOVE`. The core
checks the removal three times: the text extraction, the search and the bytes
of the file, and reports what was removed. The redaction cannot be undone; the
undo is refused explicitly, and the checkpoints of the recovery are deleted
along with it, because they contain the bytes of earlier states.

**The finding that made the work worth it.** After the first working redaction
the text was no longer extractable — but **still in the bytes of the file**.
MuPDF writes unreachable objects into the file when the garbage collection is
not requested. The old content stream object was still in the file. Fixed with
`opts.do_garbage = 1`; the test has checked the bytes since then, not only the
extraction.

**Evidence:** `docs/acceptance/M2.1-ACCEPTANCE.md`, gate M2.1 in
`scripts/validate-repo.sh`, a separate step in `scripts/local-ci.sh`.

## E. What explicitly remains NOT RUN

| Evidence | Reason |
|---|---|
| Run on an Android device | no device, no `/dev/kvm` |
| On-device OCR quality, UI responsiveness, thermals on the device | no device |
| macOS build, macOS interface, macOS integration tests | no macOS host |
| macOS interface of the redaction | no macOS host |
| Interoperability of the redaction with the tools of other vendors | outstanding, M2 gate |
| CI workflow (GitHub Actions) | no runner run, no change to workflow files |
| AddressSanitizer | only with nightly (`-Zbuild-std`); substitute: valgrind memcheck, 0 errors / 0 bytes lost |
| The remaining eight M2 functions | not started |

## F. Contradictions that were removed

- The milestone was duplicated in several places: `project/project.yaml`,
  `ROADMAP.md`, the core configuration, the Rust handshake test, the Kotlin
  handshake test and the Swift handshake CLI. They now all stand on **M2** —
  derived from `project/project.yaml` as the single source.
- `REQUIREMENTS.md` had no requirement for the export into a place that
  belongs to the user, and none for the privacy of the files. `SCAN-011`,
  `SCAN-012`, `PRIV-001`, `PRIV-002` and `REDACT-005` were added.
- The roadmap listed M1.6 twice. M1.6a (Android gate) and M1.6b (platform
  validation) are now led as one stage with two parts.

## G. Open decisions

- **Licensing decision E1–E5** (project owner): open. Without this decision
  there is no release as "approved".
- **Rotation of the GitHub PAT**: recommended. The token lies encrypted in the
  secret store and was used only via a temporary askpass script in `target/`,
  which was deleted immediately. It is not detectable in the history.
- **M1.7** (battery and thermal behaviour on the device) and the remaining M2
  functions: follow as soon as the environment allows it.

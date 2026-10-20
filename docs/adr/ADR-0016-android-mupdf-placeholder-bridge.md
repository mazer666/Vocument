# ADR-0016 — Android MuPDF Engine: From Placeholder Bridge to Real Cross-Build

**Status:** superseded by ADR-0017
**Date:** 2026-10-20
**Deciders:** ROLE-02 (Chief Architect), ROLE-04 (PDF/MuPDF Engineer), ROLE-13 (QA & Test Engineer), ROLE-20 (Project / Release Manager)
**Requirement IDs:** `QA-003`, `PLATFORM-001`, `PLATFORM-005`, `INTEROP-001`

---

> **Superseded (2026-10-20).** This ADR originally documented the Android
> placeholder MuPDF bridge as an accepted, documented limitation. The placeholder
> has since been **removed from the tree**: MuPDF 1.28.5 is now cross-compiled for
> `aarch64-linux-android` with the Android NDK and linked into the Android build.
> The decision that still holds is the *rule* this ADR established: a placeholder
> engine may never be linked into a production build, and the build must be
> verifiable from the produced binary. See **ADR-0017** for the implementation and
> the verification evidence.

---

## Context

`engines/pdf-mupdf/build.rs` selects the C bridge that gets linked into
`libvocument_ffi.so`:

- **host targets** (`macos`, `linux`): `csrc/mupdf_bridge.c`,
  `csrc/mupdf_text_bridge.c`, `csrc/mupdf_mutate_bridge.c` against a real MuPDF
  installation.
- **`android` targets**: `csrc/mupdf_stub_bridge.c`, a placeholder that contains no
  PDF parsing or rendering logic at all.

Cross-compiling MuPDF for `aarch64-linux-android` is not part of the current build
pipeline, so the Android build has always linked the placeholder.

This was previously invisible: the placeholder reported a plausible-looking engine
identifier and returned deterministic placeholder data (a fixed page count, A4 page
geometry, blank rendered tiles, no extracted text, no-op mutations), so an Android
build could look like it worked while doing nothing.

`docs/acceptance/M0-ACCEPTANCE.md` and `M0.7-ACCEPTANCE.md` record MuPDF
interoperability evidence that was produced on a **macOS host**, and the Android
Kotlin handshake (`scripts/verify-kotlin-handshake.sh`) links the **host** build
through JNA. No acceptance record therefore demonstrated real MuPDF PDF rendering
on an Android device.

Per `QA-003` / `CR-0002`, functionality that is only a placeholder must not be
presented as accepted functionality. This ADR makes the placeholder explicit and
bounds what may be claimed.

## Decision

1. **The placeholder is named and documented as a placeholder.**
   `engines/pdf-mupdf/csrc/mupdf_stub_bridge.c` carries a header comment stating
   that it is not a MuPDF implementation and that it returns placeholder data.

2. **The placeholder reports a distinguishable engine identifier.**
   It reports `mupdf-stub-1.28` instead of the real `mupdf-<version>`, so any
   runtime inspection can tell a placeholder apart from the real engine.

3. **The placeholder is guarded by the repository validator.**
   `scripts/validate-repo.sh` fails when the placeholder bridge exists without this
   ADR referencing it, so the placeholder can never silently re-enter the tree as
   an unnoticed placeholder.

4. **Acceptance records state the platform boundary explicitly.**
   `docs/acceptance/M0-ACCEPTANCE.md` and `M0.7-ACCEPTANCE.md` now state that the
   recorded MuPDF interoperability evidence originates from macOS host runs and
   that Android PDF rendering is not yet backed by a real MuPDF build.

5. **The gap is tracked as a risk.**
   `docs/project/RISKS.md` gains `RISK-006` covering the missing Android MuPDF
   build.

6. **The Android scan/OCR pipeline is unaffected.**
   The M1.x camera pipeline (`Camera → edge detection → perspective correction →
   cleanup → Tesseract OCR → searchable PDF synthesis`) does not depend on MuPDF.
   The OCR engine, the imaging engine and the PDF synthesizer are all real native
   implementations on Android and remain verifiable on device.

## Consequences

### Positive

- The Android build no longer silently pretends to render PDFs. Anyone inspecting
  the engine identifier can tell that the PDF engine is a placeholder.
- The repository validator prevents the placeholder from being reintroduced without
  an explicit architectural decision.
- Acceptance records no longer imply Android PDF rendering evidence that was never
  produced.

### Negative / accepted trade-offs

- Android PDF viewing, rendering, editing and saving are **not** functional on
  device until MuPDF is cross-compiled for Android. This is a real product gap,
  not a cosmetic one.
- M2 (macOS Professional Review) is unaffected, because it targets macOS where
  the real MuPDF engine is linked.

### Follow-up

- Cross-compile MuPDF for `aarch64-linux-android` (NDK toolchain, `MUPDF_DIR`
  per-target prefix) and replace the placeholder selection in
  `engines/pdf-mupdf/build.rs`.
- Re-run the M0.7 interoperability gate on a real Android device with the real
  engine and record the evidence in `docs/acceptance/M0.7-ACCEPTANCE.md`.

## Alternatives considered

- **Ship the placeholder silently.** Rejected: it violates `QA-003` and produces
  false confidence in Android PDF support.
- **Remove the placeholder and fail the Android build.** Rejected for now: the
  Android scan/OCR milestone (M1) is genuinely implemented and verified on device,
  and removing the placeholder would block the whole Android build for a reason
  that is unrelated to the implemented scan/OCR scope.
- **Implement a pure-Rust PDF parser as the Android engine.** Rejected for now:
  a second engine contradicts the "no hidden lock-in to one engine" goal in
  `ARCHITECTURE.md` §1 and would need its own acceptance cycle.

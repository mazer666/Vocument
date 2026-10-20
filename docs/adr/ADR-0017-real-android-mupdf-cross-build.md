# ADR-0017 — Real MuPDF Engine on Android via NDK Cross-Build

**Status:** accepted
**Date:** 2026-10-20
**Deciders:** ROLE-02 (Chief Architect), ROLE-04 (PDF/MuPDF Engineer), ROLE-13 (QA & Test Engineer), ROLE-16 (Release & Supply-Chain Engineer), ROLE-18 (Licensing & Open-Source Reviewer), ROLE-20 (Project / Release Manager)
**Requirement IDs:** `QA-003`, `PLATFORM-001`, `PLATFORM-005`, `INTEROP-001`, `JOB-004`
**Supersedes:** `ADR-0016`

---

## Context

`ADR-0016` documented that the Android build linked a **placeholder** MuPDF bridge
instead of a real PDF engine. The placeholder returned deterministic fake data: a
fixed page count, A4 page geometry, blank rendered tiles, no extracted text and
no-op mutations. Any Android build therefore looked functional while doing nothing.

The root cause was not a missing feature but a missing build path: MuPDF was only
ever compiled for the host platform, and `engines/pdf-mupdf/build.rs` selected the
placeholder for `target_os = "android"` to keep the cross build green.

## Decision

MuPDF 1.28.5 is cross-compiled for Android with the Android NDK and linked into the
Android build. The placeholder bridge is deleted from the tree.

### 1. Reproducible MuPDF build per target

`scripts/prepare-android-mupdf.sh` performs the cross build:

* **Toolchain:** the Android NDK (`ANDROID_NDK_HOME` / `ANDROID_NDK_ROOT`, or the
  NDK inside the Android SDK). The toolchain host directory is probed for
  `linux-x86_64`, `darwin-x86_64`, `darwin-arm64` and `linux-arm64`, so the script
  works on macOS and on Linux CI hosts.
* **Target:** `aarch64-linux-android` at API level 24. API 24 is the NDK floor and
  is compatible with every device the project supports (Android 13+).
* **MuPDF configuration:** `shared=yes build=release`, with `HAVE_X11=no`,
  `HAVE_GLUT=no`, `HAVE_CURL=no` and `HAVE_LEPTONICA=no`. Those components are not
  needed for PDF rendering and would pull in host-only dependencies.
* **`HAVE_LIBCRYPTO=no`:** the OpenSSL-backed PKCS#7 helper is disabled. Vocument
  does not implement digital signatures yet (`REQUIREMENTS.md` §27 is a later
  milestone) and the Android NDK ships no OpenSSL, so the module could not be
  built for Android at all. **This is a deliberate, documented scope reduction of
  the engine, not an unnoticed omission.**
* **Font embedding:** MuPDF embeds its font resources with `$(LD) -r -b binary`.
  The LLVM linker requires an explicit target emulation in that mode
  (`-m aarch64linux`), otherwise it refuses to produce an object file. The script
  passes it explicitly; without it the build fails with
  `target emulation unknown`.

### 2. Build integration

`engines/pdf-mupdf/build.rs` no longer contains any platform-specific branch. It
resolves the MuPDF installation for the current target:

* host targets: `MUPDF_DIR` (default `/opt/homebrew/opt/mupdf`)
* Android: `MUPDF_ANDROID_DIR` (default `target/mupdf-android`)

If the headers or the libraries are missing, the build **fails with a clear
message** that points at `scripts/prepare-android-mupdf.sh`. A build can therefore
no longer silently fall back to a different engine.

The same rule is generalised to every platform by `PLATFORM-005` in
`REQUIREMENTS.md`: a placeholder engine may not be linked into a production build
on any platform.

### 3. Verifiable build metadata

The runtime engine identifier is no longer a bare version string. The C bridge
receives `VOCRUMENT_MUPDF_ENGINE_ID`, which is composed of the MuPDF version read
from the linked installation's header and the build target:

```text
mupdf-1.28.5-aarch64-android
mupdf-1.28.5-aarch64-apple-darwin   (host)
```

A placeholder engine cannot report this identifier, so a runtime inspection proves
which engine and which platform produced the document.

### 4. Enforced invariants

* `scripts/validate-repo.sh` **fails** if `mupdf_stub_bridge.c` reappears, if
  `build.rs` references a placeholder, or if the Android MuPDF directory is not
  resolved through `MUPDF_ANDROID_DIR`.
* `scripts/build-android-rust.sh` inspects the produced `libvocument_ffi.so` with
  `readelf` and fails if it does not depend on `libmupdf`, or if it depends on a
  stub. The MuPDF runtime library is deployed next to the app library in
  `jniLibs/arm64-v8a/`.

### 5. Licensing

MuPDF is licensed **AGPL-3.0-or-later**, the same license as Vocument itself
(`THIRD_PARTY.md`). Consequences for the Android distribution:

* The shipped `libmupdf.so` is covered by the AGPL. The distributed APK must
  therefore offer the corresponding source of the linked MuPDF version. The MuPDF
  source tarball used for the build is already pinned by version and is fetched by
  `scripts/prepare-android-mupdf.sh`, so the corresponding source can be provided
  from the same revision.
* No additional permissive-licence exception is required, and no licence conflict
  arises: the application and the engine share the same licence.
* **Open item:** the exact source-offer mechanism for the distributed APK (in-app
  link, archive shipped alongside, or a build-time source bundle) is not yet
  decided. It is recorded in `docs/project/RISKS.md` (`RISK-009`) and must be
  settled before the first public Android release.

## Consequences

### Positive

* Android can actually open, render, rotate, annotate and save PDFs. The M0.7
  interoperability scenario becomes meaningful on the device.
* The engine identifier proves at runtime which engine produced a document.
* A silent fallback to a fake engine is structurally impossible.

### Negative / accepted trade-offs

* The Android build now requires the NDK and a MuPDF cross build, which takes
  several minutes. This is the same cost the host build already has.
* `HAVE_LIBCRYPTO=no` means MuPDF's PKCS#7 helper is unavailable on Android. This
  only affects digital signatures, which Vocument does not implement yet.
* The APK grows by the size of `libmupdf.so` (roughly 55 MB for the release
  build). This is a real distribution cost and is recorded in `RISK-010`.

## Alternatives considered

- **Keep the placeholder and document it.** Rejected: it contradicted `QA-003` and
  produced false confidence in Android PDF support.
- **Pure-Rust PDF engine on Android.** Rejected for now: a second engine would
  contradict the "no hidden lock-in to one engine" goal in `ARCHITECTURE.md` §1 and
  would need its own acceptance cycle.
- **Ship MuPDF as a static library.** Rejected: the dynamic library keeps the APK
  packaging conventional and allows the engine version to be verified from the
  binary's dynamic dependencies.

## Follow-up

- Verify the Android rendering path on a physical device and on an emulator
  (`scripts/verify-android-runtime.sh`), and record the evidence in
  `docs/acceptance/M0.7-ACCEPTANCE.md`.
- Settle the AGPL source-offer mechanism (`RISK-009`).
- Extract the "cross-build an engine for a target" logic into a shared helper, so
  that the planned Windows and Linux tracks reuse it (`CR-0003`).

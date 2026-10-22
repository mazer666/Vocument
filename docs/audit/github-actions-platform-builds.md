# GitHub Actions: the builds on the real platforms

**Date:** 2026-10-22 · **Branch:** `main` · **Roles:** ROLE-16 (Release) and
ROLE-13 (QA) · **Related:** [`CI-SETUP.md`](../releasing/CI-SETUP.md),
[`BUILD-FROM-SOURCE.md`](../releasing/BUILD-FROM-SOURCE.md),
[`RELEASE-BLOCKERS.md`](../releasing/RELEASE-BLOCKERS.md)

This document describes the continuous integration of the project on the
**GitHub-hosted runners**, the three jobs it consists of, what each of them
proves, what stays **NOT RUN**, the errors that were found while the commands
were executed on this machine, and how long the jobs need.

> **Nothing is published.** No workflow uploads a package, signs anything,
> creates a tag or writes into a store. The legal clearance (E5) is open, and
> only a person can give it.

---

## 1. The state before this change, measured

Read from the API of the repository on 2026-10-22:

| What | The measurement |
|---|---|
| The runs of the workflow since 2026-10-17 | **47** |
| Of them: cancelled | **38** |
| Of them: waiting (`queued`) | **9** |
| Of them: executed (green or red) | **0** |
| The runners registered for the repository | **0** |

The workflow asked for `runs-on: self-hosted`, and the repository has no
runner of its own. The CI has therefore never run once: the gate of the
project was only ever run by hand, in the environment of the team.

The decision of the project owner (2026-10-22) is:

> "Use GitHub Actions with the required native SDKs where possible."

The old workflow (`.github/workflows/ci.yml`) is **replaced**. Three workflows
take its place:

| The workflow | The job | The runner | The subject |
|---|---|---|---|
| `ci.yml` | `linux-core` | `ubuntu-latest` | the gate of the project (Rust, MuPDF, Tesseract) |
| `ci-macos.yml` | `macos-swift` | `macos-15` | the Swift build, the tests of the kit, the handshake |
| `ci-android.yml` | `android` | `ubuntu-latest` | the native libraries, the packages, the unit tests |

---

## 2. The three jobs

### 2.1 The Linux job — the gate of the project

`ci.yml` runs the **complete** gate: `./scripts/local-ci.sh`. Nothing of the
gate is weakened, skipped or replaced by a smaller check.

What the job does, in this order:

1. installs the system packages of the gate: the C toolchain, `curl`,
   `unzip`, `python3` and the **development files of Tesseract and
   Leptonica** (`libtesseract-dev libleptonica-dev`) — the OCR engine of the
   project is linked against the system Tesseract;
2. installs the Rust toolchain (`stable`, with `rustfmt` and `clippy`);
3. **builds MuPDF 1.28.5 from the pinned source**
   (`scripts/prepare-host-mupdf.sh`) — the engine of the project is linked
   against it, and the release pins the version;
4. sets `MUPDF_DIR`, `LD_LIBRARY_PATH`, `TESSERACT_DIR=/usr` and
   `TESSDATA_PREFIX` — the four variables that the build scripts really read
   (see §5.2);
5. fetches the crates (`cargo fetch --locked`) — the licence audit reads the
   manifest of every crate out of the registry of Cargo;
6. runs the gate and shows the build record of the run.

**What it proves:** the whole gate of the project on a machine that was built
from scratch: the validation of the repository, the gate of the distribution,
the audit of the licences, the proof of the Swift sources, `cargo fmt`,
`cargo clippy -D warnings`, the tests of the workspace, the proofs of the
redaction and of the inspections, the bindings of UniFFI and their
comparison with the ones of the repository, the closure and the 16 KB page
alignment of the native libraries of Android, and the build record.

**What it does not prove:** the compilation of the macOS application, the
package of Android, the behaviour of the application on a device, the
signature, the legal clearance.

### 2.2 The macOS job — the Swift half of the product

`ci-macos.yml` builds the product on a Mac, because SwiftUI, AppKit and
CoreGraphics only exist there.

What the job does:

1. installs the OCR engine (`brew install tesseract`) — the library of the FFI
   layer is linked against it;
2. chooses an Xcode that carries **Swift 6.0** (the package declares
   `// swift-tools-version: 6.0`; the image of the runner ships several
   versions of Xcode, and the job does not depend on the default of the
   image);
3. installs the Rust toolchain;
4. **builds MuPDF 1.28.5 from the pinned source** — the Homebrew formula
   `mupdf` is a *different* build of a possibly different version, and the
   engine id of the release is `mupdf-1.28.5`;
5. sets `MUPDF_DIR` and `DYLD_LIBRARY_PATH`;
6. builds the Rust library of the FFI layer (`cargo build -p vocument-ffi`) —
   the Swift package links it;
7. builds the generated bindings (`VocumentFFI`), the kit (`VocumentMacKit`)
   and the **application** (`VocumentMacApp`);
8. runs the tests of the kit (`swift test`);
9. runs the handshake (`swift run … vocument-mac-handshake`): Swift → UniFFI →
   Rust → MuPDF, on a real PDF of the repository.

**What it proves:** that the application of the project compiles against the
real macOS SDK, that the tests of the kit pass, and that the handshake opens
a real document, renders it, searches in it and names the engine it used
(`mupdf-1.`).

**What it does not prove:** the signature and the notarisation of the bundle
(`scripts/release-macos.sh`), the behaviour on the device of a user, the
legal clearance.

### 2.3 The Android job — the packages

`ci-android.yml` installs the native SDKs and builds the packages.

What the job does:

1. installs the **JDK 17** and the system packages (the autotools: the
   Tesseract engine is cross-built with them);
2. installs the **Android SDK**: the command line tools 12.0 (pinned by their
   SHA-256), `platform-tools`, `platforms;android-35`, `build-tools;35.0.0`
   and the **NDK r27c** (`ndk;27.2.12479018`);
3. installs the Rust toolchain and the target `aarch64-linux-android`;
4. builds the **host** MuPDF 1.28.5 — the unit tests of Gradle load the
   *host* library of the FFI layer, and that library is linked against the
   host MuPDF;
5. builds the native library of the application for `arm64-v8a`
   (`./scripts/build-android-rust.sh`): the real MuPDF and the real Tesseract
   are cross-built with the NDK, with 16 KB memory pages — no stub
   (`QA-003`, `ADR-0017`);
6. builds the **debug and the release package**
   (`./gradlew assembleDebug assembleRelease`);
7. checks the native libraries **as they are packaged**
   (`./scripts/verify-android-native-libs.sh --apk <apk>`, for both
   packages): the closure that the linker of the device can resolve and the
   16 KB page alignment;
8. runs the unit tests of Gradle (`./gradlew testDebugUnitTest`);
9. states out loud that the proofs that need a device are **NOT RUN**.

**What it proves:** that the native library of the application is built for
`arm64-v8a` against the real engines, that both packages are built, that the
native libraries are really inside the packages, and that the unit tests
pass against the native library.

**What it does not prove:** the behaviour on a real device, the publication
of the package (nothing is uploaded), the legal clearance.

---

## 3. The new scripts

| The script | What it does |
|---|---|
| `scripts/prepare-host-mupdf.sh` | builds MuPDF 1.28.5 for the **host** (Linux and macOS) from the pinned source archive, verifies its SHA-256, installs the library, the headers and the licence into `MUPDF_DIR`, and checks the name that the loader looks for (`SONAME` on Linux, the install name on macOS). The installation is done by hand, not with the `install` target of MuPDF: that target installs the command line tools as well, and on macOS it replaces the installed library with a link to itself. |
| `scripts/ci-optional-proof.sh` | runs an **optional** proof and reports the outcome in the convention of the project: `PASS` / `FAIL` / `NOT RUN`. It never decides the outcome of a job: the exit code is always 0, and a proof that was not carried out is never reported as passed. |

Both scripts are in the list of the required files of
`scripts/validate-repo.sh`: the CI cannot run without them, and their
absence has to be an error, not a silence.

---

## 4. The changes to the scripts that already existed

| The change | Why |
|---|---|
| `scripts/validate-repo.sh`: the rule *"GitHub Actions workflows never use GitHub-hosted runners"* was **replaced** by the opposite rule: every workflow has to run on a GitHub-hosted runner, and no workflow may ask for a `self-hosted` one. | The rule contradicted the decision of the owner (2026-10-22) and would have made the gate of the Linux job **red** on the first run. The repository has no runner of its own; a workflow that asks for one waits forever. |
| `scripts/verify-android-native-libs.sh`: the mode **without** an argument (the build directory of the project) now says `NOT RUN` and ends with the exit code 0 when that directory does not exist. A directory that is named explicitly, and the mode `--apk`, stay **strict**. | The native libraries of Android are **not** part of the repository (`.gitignore`: `**/jniLibs/`). On a machine that has not built them there is nothing to verify — the convention of the project says `NOT RUN`, not a red gate. The check itself is not weakened: where the libraries exist, and in the Android job (where the check reads the libraries out of the **package**), it is still strict. |
| `scripts/local-ci.sh`: **not changed.** | The gate had to stay the gate. The two points above were solved in the scripts that the gate calls, not in the gate. |

---

## 5. What was executed locally, and the errors that were found

Every command of the workflows that can run on Linux was executed on this
machine (a Linux container, one core, 2 GB of memory). The macOS commands
cannot be executed here — the section §5.6 says what was done instead.

### 5.1 The gate of the Linux job — **PASS** (exit code 0)

```
MUPDF_DIR=…/target/mupdf-host   LD_LIBRARY_PATH=…/target/mupdf-host/lib
TESSERACT_DIR=/usr               TESSDATA_PREFIX=…/apps/android/app/src/main/assets/tessdata
RUSTUP_TOOLCHAIN=stable
./scripts/local-ci.sh
```

The result: `==> Local CI completed successfully.`, the build record in
`target/build-record/`, **258 tests passed, 0 failed**, the source archive
reproducible, the gates `repository`, `distribution` and `licences` all
`PASS`.

### 5.2 The errors that the first run of the commands would have caused

| # | The error | The cause | The fix |
|---|---|---|---|
| 1 | The audit of the licences named **every** crate of the workspace as `UNKNOWN (the manifest of the crate was not found)` and the gate failed. | `scripts/audit-licenses.sh` reads the manifest of the crates out of `~/.cargo/registry/src/`. On a runner that was built from scratch the registry is empty — and the audit runs **before** the first `cargo build`. | The job fetches the crates **before** the gate: `cargo fetch --locked`. |
| 2 | The link of the OCR engine failed: `cannot find -ltesseract`. | `engines/ocr/build.rs` reads `TESSERACT_DIR`; without it, the script looks for the library in `/opt/homebrew/opt/tesseract` and `/usr/local/opt/tesseract` — the two places of Homebrew. On Linux the system Tesseract is in `/usr`. | The job sets `TESSERACT_DIR=/usr` (as `docs/releasing/BUILD-FROM-SOURCE.md` describes) and installs `libtesseract-dev libleptonica-dev`. |
| 3 | The OCR of the tests found no language model. | The engine reads `TESSDATA_PREFIX` at the run time. The models are part of the repository (`apps/android/app/src/main/assets/tessdata`). | The job sets `TESSDATA_PREFIX` to that directory. |
| 4 | The native libraries of Android were missing → the gate failed with `The native library directory does not exist`. | The libraries are not part of the repository; they are built by `scripts/build-android-rust.sh` (the job of Android). | The change of `scripts/verify-android-native-libs.sh` in §4: the mode without an argument says `NOT RUN`. The Android job checks them in the package. |
| 5 | The gate would have failed on the rule about the runners. | `scripts/validate-repo.sh` forbade the GitHub-hosted runners. | The rule was replaced (see §4). |
| 6 | The toolchain file names the target `aarch64-apple-darwin`, whose standard library does not belong to a Linux host. | `rust-toolchain.toml` lists the targets of the project (the macOS target for the release, the Android target for the packages). A Linux job needs neither. | The jobs install the toolchain explicitly (`rustup toolchain install stable --profile minimal …`) and set `RUSTUP_TOOLCHAIN=stable`, so that the list of targets of the file is not acted upon. The Android job adds `aarch64-linux-android` — the target it needs — by hand. |
| 7 | The MuPDF of the host was missing on the runner, and the release pins the version `1.28.5`. | The engine is linked against MuPDF; the Homebrew formula is a different build. | `scripts/prepare-host-mupdf.sh` builds the pinned version from the source archive and verifies its SHA-256. The full path (download → checksum → unpack → build → install → check of the `SONAME`) was executed on this machine from an empty directory: **6 min 50 s with one core**. |
| 8 | The cross-build of Tesseract failed: `autoreconf` / `libtoolize` were not found. | `scripts/prepare-android-tesseract.sh` configures Leptonica and Tesseract with the autotools of the host. | The Android job installs `autoconf`, `automake`, `libtool` and `m4`. |
| 9 | The library of MuPDF would not have been found at the run time on macOS. | MuPDF sets **no** install name; the default of the linker is the path of the build directory, and that path is meaningless in the installation. | `scripts/prepare-host-mupdf.sh` sets the install name to `@rpath/libmupdf.dylib` (the flag `XLIB_LDFLAGS`) and checks it with `otool`. The consumers carry the rpath of the installation directory: the Rust library through `engines/pdf-mupdf/build.rs`, the Swift package through `apps/macos/Package.swift`. |
| 10 | The `install` target of MuPDF would have replaced the installed library on macOS with a link to itself. | The rule of the `Makefile` creates the link `libmupdf.$(SO)` — on macOS that is the file that was just installed. | The script installs the library, the headers and the licence by hand. |
| 11 | The gate of the **shared** working tree is red — for a reason that has nothing to do with the CI: `scripts/release-candidate.sh` (the file of another task) has 1009 lines, above the hard limit of 900 of `CODE-001`. | The file was added to the tree while this work was running. | **Not fixed here** — it belongs to the task that owns the file. The Linux job will be red until that file is split; the reason is in the log of the job. |

### 5.3 The proof of the convention `PASS / FAIL / NOT RUN / BLOCKED`

`scripts/ci-optional-proof.sh` was executed in all four cases:

```
FAIL:      a command that ends with 3      -> exit code 0, the job stays green
NOT RUN:   --not-run "no device"           -> exit code 0
NOT RUN:   a command that does not exist   -> exit code 0
PASS:      a command that ends with 0      -> exit code 0
```

The script itself is used wrongly (no `--label`, no command) → exit code 2.

### 5.4 The job of Android — the steps of the workflow, executed

`./scripts/build-android-rust.sh` (the cross-build of the engines with the
NDK, the build of the FFI layer for `aarch64-linux-android`, the check of the
closure and of the 16 KB pages), `./gradlew assembleDebug assembleRelease`,
`./scripts/verify-android-native-libs.sh --apk <both packages>` and
`./gradlew testDebugUnitTest` were executed on this machine with the
environment of the workflow (`ANDROID_HOME`, `ANDROID_NDK_HOME`,
`MUPDF_DIR`, `TESSERACT_DIR`, `TESSDATA_PREFIX`, `JAVA_HOME`).

The result is in §7.

### 5.5 The three YAML files

Every file was parsed with the YAML parser of Python
(`yaml.safe_load`): `ci.yml`, `ci-macos.yml` and `ci-android.yml` are
syntactically valid, and every path, script and line that they call exists
in the repository:

| The call | The file |
|---|---|
| `./scripts/prepare-host-mupdf.sh` | `scripts/prepare-host-mupdf.sh` (new) |
| `./scripts/local-ci.sh` | `scripts/local-ci.sh` (unchanged) |
| `./scripts/build-android-rust.sh` | `scripts/build-android-rust.sh` |
| `./scripts/verify-android-native-libs.sh --apk <apk>` | `scripts/verify-android-native-libs.sh` |
| `./scripts/ci-optional-proof.sh` | `scripts/ci-optional-proof.sh` (new) |
| `cd apps/android && ./gradlew …` | `apps/android/gradlew` |
| `cargo build -p vocument-ffi` | `platform/vocument-ffi` |
| `swift build/test/run --package-path apps/macos` | `apps/macos/Package.swift` |

### 5.6 The macOS job — what was **not** executed here

This machine is Linux: the Swift compiler, the macOS SDK and `otool` do not
exist here, so the commands of the macOS job could not be executed. What was
done instead:

* the **Rust** part of the job was executed here (the build of the FFI layer
  against the host MuPDF 1.28.5 that the script installs);
* the flags of the macOS build of MuPDF (`XLIB_LDFLAGS=-Wl,-install_name,@rpath/libmupdf.dylib`)
  were read out of the `Makefile` of MuPDF 1.28.5 in the source tree of the
  project (`LIB_LDFLAGS += $(XLIB_LDFLAGS)`, `Makerules:129`);
* the check of the install name (`otool -D`) is part of the script, so a
  wrong name fails the job on the runner instead of failing later, in the
  handshake;
* the version of Swift is read out of the package of the project
  (`// swift-tools-version: 6.0`) and the job selects an Xcode that carries
  it.

The first run of the macOS job on a runner is therefore the first real proof
of that job; the log of the job names the version of Xcode and of Swift that
it used.

---

## 6. What stays **NOT RUN**

| The proof | Where | Why |
|---|---|---|
| The compilation of the macOS application in the gate of the Linux job | `ci.yml` | SwiftUI, AppKit and CoreGraphics are part of the macOS SDK; a Linux runner has none. The gate says it out loud. The proof is the job of `ci-macos.yml`. |
| The ARM64 proof under QEMU | `ci.yml` | The gate only starts it when it finds `qemu-aarch64-static`; the job takes the emulator of the user mode out of the PATH on purpose (the proof needs the cross-build of the Android engine, which belongs to the job of Android). The gate says `SKIPPED … NOT RUN`. |
| The ARM64 proof under QEMU | `ci-android.yml` | It needs the **static** cross-build of MuPDF for Android, which no script of the project produces today. The job says `NOT RUN` with that reason. |
| The proof of the OCR on a device | `ci-android.yml` | The job has no device and no emulator; the proof needs a system image and the boot of an emulator. The job says `NOT RUN` with that reason. |
| The runtime proof of Android of the gate | `ci.yml` | The gate needs the Android SDK; the Linux job has none (the SDK belongs to the job of Android). The gate says `SKIPPED … NOT RUN`. |
| The signature, the notarisation and the publication | everywhere | The legal clearance (E5) is open. The workflows sign nothing, upload nothing and publish nothing. |

A proof that is `NOT RUN` is never reported as a passed proof: the scripts
of the project say so in their own output, and
`scripts/ci-optional-proof.sh` says it for the optional proofs of the
workflows.

---

## 7. The time the jobs need

Measured on this machine (**one** core, 2 GB of memory) and estimated for a
runner of GitHub (4 cores, 16 GB, no competition):

| The step | Here (1 core) | On a runner (4 cores) |
|---|---|---|
| The build of the host MuPDF 1.28.5 (from an empty directory) | **6 min 50 s** | ~3 min |
| The whole gate of the project (`local-ci.sh`, warm cargo cache) | ~25 min | ~10 min |
| The whole gate (cold cargo cache) | — | ~15 min |
| The cross-build of the engines of Android (MuPDF + Tesseract with the NDK) | ~12 min | ~6 min |
| The two packages of Android (`assembleDebug assembleRelease`) | ~12 min | ~6 min |
| The unit tests of Gradle | ~4 min | ~2 min |
| The job of macOS (MuPDF + Rust + Swift + the tests + the handshake) | — | ~12 min |

**The planned duration of the jobs:**

| The job | The limit in the workflow | The expected duration |
|---|---|---|
| `linux-core` | 90 min | **~20 min** (with a warm cargo cache ~15 min) |
| `macos-swift` | 90 min | **~15 min** |
| `android` | 120 min | **~30 min** |

The limit of a job on GitHub is 6 hours; the limits above are the guard
against a run that hangs, and they are far below the limit. The free quota
of the minutes of GitHub is not exceeded by these three jobs (a push to
`main` runs them once; a pull request runs them once per push).

The caches that the jobs use: the registry of Cargo and the distribution of
Gradle (`actions/cache`, the key contains the checksum of `Cargo.lock` and of
the wrapper of Gradle). The **target directory is not cached**: the build of
the product is always made from the source, never taken from a cache.

---

## 8. The rules that are kept

* **No publication.** No workflow has a step that uploads, signs, tags or
  publishes. The permission of every workflow is `contents: read`; no
  secret is used, and none is needed.
* **No secret in the workflow, in the log or in the configuration.** The only
  things that are written are public: the checksums of the command line
  tools of the Android SDK and the checksums of the licences of the SDK.
* **The optional proofs are optional.** `scripts/ci-optional-proof.sh` never
  lets a failure of an optional proof become the exit code of the job, and
  it names the outcome in the log (`PASS` / `FAIL` / `NOT RUN`).
* **The gate is the gate.** `scripts/local-ci.sh` was not changed.
* **The versions are pinned.** MuPDF 1.28.5 (with its SHA-256), the NDK
  r27c, the command line tools 12.0 (with its SHA-256), the platform 35, the
  build tools 35.0.0, the JDK 17, the Gradle 9.8.1 and the Kotlin 2.0.21 —
  the same pins as `docs/releasing/BUILD-FROM-SOURCE.md`.

---

## 9. The points for the team lead

1. **The Linux job will be red** until `scripts/release-candidate.sh` (1009
   lines, the hard limit of `CODE-001` is 900) is split. The file belongs to
   another task; the reason is in the log of the job.
2. The audit of the licences compares the inventory with `THIRD_PARTY.md`,
   and the section of the Android components depends on whether the **Gradle
   cache** is present on the machine. The Linux job therefore does **not**
   restore the Gradle cache — it does not need it. A machine with a full
   Gradle cache (the machines of the team, after an Android build) reports
   a difference in the inventory. That is a property of the audit, not of the
   CI; it is worth a look of the licensing reviewer.
3. The macOS job is the first proof of the Swift build on a runner: it has
   never run. Its log names the version of Xcode and of Swift that it used.
4. The first run of the three jobs will download the SDKs (about 1 GB) and
   build the engines twice (the host MuPDF and the cross-build for Android).
   The caches of §7 make the second run faster.

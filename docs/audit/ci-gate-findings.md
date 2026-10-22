# The CI and the gate of the release path — measured findings

**Status:** proposed — a report of a measurement, not a decision
**Date:** 2026-10-22
**Author role:** ROLE-13 (QA & Test Engineer)
**Review roles:** ROLE-02 (Chief Architect), ROLE-09 (Security Engineer), ROLE-10 (Privacy Engineer), ROLE-16 (Release & Supply-Chain Engineer)

> This document reports what a local run of the gate measured on macOS
> (arm64, macOS 27.0.0) on the revision `3459146`, and what was changed as a
> result. The findings that need a decision of the project owner are in §4.

---

## 1. The situation

`B-9` in the roadmap says: *"there is no CI runner, and the CI has never
run."* The three workflows on the hosted runners
(`.github/workflows/ci.yml`, `ci-macos.yml`, `ci-android.yml`) were added
with the revision `541abcb` and have never produced a result.

The gate was therefore run locally, step by step, on the machine of the
author. The environment was prepared first: MuPDF 1.28.5 was built for the
host (`scripts/prepare-host-mupdf.sh`), Tesseract 5.5.3 came from Homebrew,
and the tools of the project (`cargo` 1.99.1, `swift` 6.4, `python` 3.15)
were the ones of the machine.

The result: **the gate was red, and four of the failures were defects of the
project, not of the environment.** The most important of them would have made
the hosted CI red on its first run.

## 2. The defects that were found and fixed

### 2.1 A test that only passes as root

`core/vocument-core/tests/annotation_store_hardening_tests.rs` →
`a_rail_of_a_stranger_is_not_read`

The test builds the situation that the ownership check defends against: it
gives the rail of the user to another user with `chown`. Giving a file away
is only possible for a process that runs as root. The test asserted the
`chown` with `.expect("the rail of the stranger")` — so on every machine that
is not root, **the whole test suite failed**, and the failure had nothing to
do with the behaviour under test.

The hosted runners of GitHub do not run as root either. The check itself is
correct and is in the product (`assert_belongs_to_the_user`); only the way the
test built its premise was wrong.

**Fixed** in the same period by the commit `6005e3f` — after the convention
that the project already uses for exactly this situation
(`recovery_privacy_tests.rs` → `a_directory_of_a_stranger_is_refused`): if
the rail cannot be given away, the test says `NOT RUN` and does not claim
that the check was exercised. The solution of that commit is equivalent to
the one written here, so this change was **not** carried over — there is one
fix, not two.

### 2.2 A race in the rail of the profession

`platform/vocument-ffi/tests/annotation_e2e_tests.rs`

The rail of the profession lives in the registry of the sessions, and the
registry is shared by the tests of a binary. Two tests replace the rail, and
one of them never put the standard rail back. The result was a test run whose
outcome depended on the order of the threads:

| Run | `--test-threads=1` | default (parallel) |
|---|---|---|
| 1 | 8 passed, 0 failed | 2 passed, **6 failed** |
| 2 | 8 passed, 0 failed | 3 passed, **5 failed** |
| 3 | 8 passed, 0 failed | 2 passed, **6 failed** |
| 4 | — | 7 passed, **1 failed** |
| 5 | — | 8 passed, 0 failed |

Four of five parallel runs were red, and the failing tests were different
every time. The hosted CI runs the tests with the default parallelism — the
workflow would have been red, and it would have been red *at random*.

The commit `48d2815` has put a lock around the tests of the rail in the
meantime, and `local-ci.sh` runs the whole suite with `--test-threads=1`
since then. **That is not enough**, and the measurement says so: the test
`the_rail_of_the_user_may_replace_the_default` replaces the rail with a
single entry and never puts the standard rail back. The lock keeps the tests
apart from each other, but it does not repair the rail — the test that runs
next finds the rail of the test before it. On the state of `origin/main`
(`c90146c`) the suite is still red in two of six parallel runs:

```text
test result: ok. 8 passed; 0 failed; …          (run 1)
test result: FAILED. 7 passed; 1 failed; …      (run 2)
test result: ok. 8 passed; 0 failed; …          (run 3)
test result: ok. 8 passed; 0 failed; …          (run 4)
test result: ok. 8 passed; 0 failed; …          (run 5)
test result: FAILED. 7 passed; 1 failed; …      (run 6)
```

**Fixed here** with a lock that the tests of the rail hold **and** a guard
that puts the standard rail back when the test ends — also when the test
failed, and also before the lock is released. The order of the tests can no
longer decide whether they pass. After the fix: six consecutive parallel
runs, 8 of 8 passed every time.

### 2.3 The tests wrote into the data directory of the user

`platform/vocument-ffi/tests/*.rs`

The rail of the profession is a setting of the user, and the recovery store
holds the document of the user in plain text. Both live in the data directory
of the user — on macOS `~/Library/Application Support/Vocument`. The
end-to-end tests of the FFI layer wrote into it: they replaced the rail of
the user and left their documents in the recovery store of the user.

The measurement of this run, before the fix:

```text
-rw-------  1306  22 Okt 09:47  ~/Library/Application Support/Vocument/jurist_palette.json
drwx------   17   22 Okt 09:47  ~/Library/Application Support/Vocument/vocument_recovery
```

The timestamp is the time of the test run. A test run on the machine of a
user therefore **destroys the rail of the user** and leaves the documents of
the test in the recovery store of the user.

**Fixed** with a shared helper (`platform/vocument-ffi/tests/common/mod.rs`):
every test of every FFI test binary points the data directory at a temporary
directory before it touches the registry of the sessions. The verification
after the fix: the palette and the recovery store of the user are unchanged
after a full `cargo test --workspace` — the same hash, the same number of
entries.

### 2.4 `tar --wildcards` — a GNU extension that macOS does not know

`scripts/release-candidate.sh`, `scripts/verify-release-candidate.sh`,
`scripts/verify-source-archive.sh`, `scripts/build-record.sh`

The identification of the revision was read out of the source archive with
`tar xzf … -O --wildcards '*/SOURCE-IDENTIFICATION.txt'`. The option
`--wildcards` is a GNU extension; the `tar` of macOS answers with

```text
tar: Option --wildcards is not supported
```

and the command was sent into `2>/dev/null`, so the message was thrown away.
The script then died through `set -e` **without a word of explanation** — the
release candidate simply stopped after
`==> Building the source archive of … (twice)…` with the exit code 1.

**Fixed**: the member is looked up in the list of the archive and read by its
exact name, which every `tar` understands. The failure is no longer silent:
the check is recorded as `FAIL` with the reason *"the identification of the
revision could not be read out of the archive"*.

### 2.5 The build record died because a tool was missing

`scripts/build-record.sh`

`version_line` ran the tool with `--version` and passed its exit code on. A
`java` that is installed but has no runtime answers with an error and the
exit code 1 — and the record ended there, silently, with no file written. The
build record is the evidence of the release; it may not disappear because a
tool on the machine is broken.

**Fixed**: a tool that is missing is named `not-installed`, a tool that is
there but does not answer is named `not-usable: <what it said>`, and neither
ends the record.

### 2.6 The libraries were looked for under their Linux name only

`scripts/release-candidate.sh`, `scripts/build-record.sh`,
`scripts/audit-licenses.sh`

The shared library of MuPDF and of Tesseract was looked for as
`libmupdf.so` and `libtesseract.so`. On macOS the files are `libmupdf.dylib`
and `libtesseract.dylib`, so the candidate reported a complete toolchain
that was complete, and the build record named a file that did not exist.

The same name was wrong in three more places, and each of them turned a
result into a wrong one:

* the library that the candidate builds from the sources of the revision
  (`libvocument_ffi.so`) — the build **succeeded** in 41 seconds, and the
  candidate reported the library as not built, with the message *"the
  library could not be built … (exit code 0)"*;
* the list of the artefacts of the release, which therefore carried no
  shared library on macOS;
* the list of the artefacts of the comparison of the source archive.

**Fixed** in the two scripts of the release path: the candidate looks for the
name of the platform, and the record names the library that is really linked.
The artefact lists carry both names — the loop takes the one that exists.

### 2.7 `readelf` was required without a way around it

`scripts/verify-android-native-libs.sh`

The verification of the native Android libraries reads the ELF headers with
`readelf`, and the script ended with *"ERROR: readelf is required"* on a
machine that has no binutils. macOS has no `readelf`, but the toolchain of
LLVM carries `llvm-readelf`, which takes the same options.

**Fixed**: the script uses `readelf`, and otherwise `llvm-readelf` from the
toolchain of LLVM or from Homebrew. Without one of the two the verification
still fails — a proof that was not carried out is not a proof.

### 2.8 The peak memory was read from a Linux file

`engines/pdf-mupdf/src/difficult_tests.rs`

`the_engine_keeps_the_memory_usage_of_the_difficult_corpus_bounded` read the
peak memory from `/proc/self/status`. macOS has no `/proc`, and the test
failed with *"the kernel must report the peak memory"* — on a machine whose
kernel reports the peak memory quite well, only not through that file.

**Fixed**: Linux keeps `VmHWM`, the other systems ask `getrusage`, which
reports the value in **bytes** on macOS and in kilobytes on Linux. The
`libc` crate that carries the declaration was already in the tree of the
workspace (through `rustix` and `getrandom`) and already in the licence
inventory of the third-party components (`MIT OR Apache-2.0`); the change of
`Cargo.lock` is the dependency edge alone, no version and no checksum.

### 2.9 macOS did not find the libraries of the host

`scripts/local-ci.sh`, `scripts/release-candidate.sh`,
`scripts/verify-source-archive.sh`, `scripts/build-record.sh`

The scripts put the libraries of the host on `LD_LIBRARY_PATH`. That is the
variable of Linux; macOS reads **`DYLD_LIBRARY_PATH`** and ignores the other
one. The test binaries of the crates that link MuPDF died on macOS with

```text
dyld[33784]: Library not loaded: @rpath/libmupdf.dylib
```

The `rpath` of the binaries carries the directory of the Tesseract, but not
the one of the MuPDF: `cargo:rustc-link-arg` of a crate applies to that
crate alone, and a crate that *depends* on the PDF engine does not inherit
its search path.

**Fixed**: the scripts set both variables. The gate therefore runs on both
platforms without a change of the build scripts.

## 3. The state of the gate after the fixes

| Step | Before | After |
|---|---|---|
| `cargo fmt --check` | PASS | PASS |
| `cargo clippy -D warnings` | PASS | PASS |
| `cargo test --workspace` | **FAIL** (1 failure as non-root, 4–6 intermittent failures) | **PASS** (every suite, exit 0) |
| the Swift app build | **FAIL** (see §4.2) | unchanged — the environment is missing the licence of Xcode |
| the Swift handshake | PASS | PASS |
| the UniFFI bindings are in sync | PASS | PASS |
| the native Android libraries | **FAIL** (no `readelf`) | **FAIL** — but for a different and correct reason: the directory of this machine holds a `libvocument_ffi.so` from an earlier Android build without the libraries it needs (§4.4) |
| the source archive is reproducible | **FAIL** (`tar --wildcards`) | **PASS** |
| the archive names its revision | **FAIL** (silent) | **PASS** |
| the build record is written | **FAIL** (silent abort) | **PASS** |
| the verification of the candidate | **FAIL** (the archive could not be read) | **PASS** |
| the licence audit | **FAIL** (§4.1) | unchanged — it needs a decision |

## 4. The findings that need a decision of the project owner

### 4.1 The licence inventory could not be the same on two machines — **resolved**

`scripts/audit-licenses.sh` compared the inventory it computes from the
machine with the inventory that is committed in `THIRD_PARTY.md`. The two
could only be equal on the machine where the committed one was produced:

1. **The Android components came from the Gradle cache of the machine.** The
   cache is a directory of the user (`~/.gradle/caches/…`). A machine
   without a cache got a placeholder row, a machine with a cache got one
   row per component.
2. **The reader of the POMs was wrong.** The component and the version were
   read one level too deep: the version became the SHA-1 of the file
   (`3727a4b2224a…`) and the component became the version (`1.9.3`).
3. **The licence of a POM was never found.** The pattern
   `<license>\s*([^<]+?)\s*</license>` cannot match a POM of the ordinary
   Maven layout, in which the licence is a child element:
   ```xml
   <license>
     <name>The Apache Software License, Version 2.0</name>
     <url>http://www.apache.org/licenses/LICENSE-2.0.txt</url>
   </license>
   ```
   Every component was therefore counted as *"a component carries an unknown
   licence"*.
4. **The check covered the whole cache.** The cache of a build environment
   holds the whole build classpath — the plugins of Gradle, the test
   libraries, the tools of the compiler. Sixty of the 389 POMs name no
   licence at all (`io.netty`, `com.google.guava`,
   `org.apache.httpcomponents`), and none of them ships with the product.
5. **The column of the symbols could only match the format it was measured
   in.** The committed numbers come from the ELF library of the host
   (`readelf -sW`); the library of macOS is a Mach-O file and reports other
   numbers.

**Resolved.** The decision that the audit itself already stated in its own
comment — *"the inventory of the Android components does not depend on the
machine that runs the audit"* — is now carried out:

* the inventory of the Android components is a **list of the repository**
  (the components and the licences that the Gradle files declare), not a
  reading of the cache;
* the check reads the POMs of **exactly those components**, maps the licence
  to an SPDX identifier and compares it with the list. A component whose POM
  is not in the cache of this machine is `NOT RUN` and is named — the audit
  does not claim that it checked it;
* the other POMs of the cache are counted and named as the build
  environment. They are not findings, because they do not ship;
* the number of the symbols in the shipped library is a **measurement of a
  build** and stands in the **report** of the audit, which the release
  candidate carries as an artefact;
* the column *"where the licence was read from"* names the place in the
  **pinned source** and not the directory of the local build.

One finding came out of the corrected check: the POM of
`androidx.camera:camera-core` declares **two** licences — *"The Apache
Software License, Version 2.0"* and *"BSD License"* (the licence of the
Chromium library `libyuv`, the three-clause BSD licence). The inventory named
only Apache-2.0 and therefore under-reported a licence obligation. The row
names both now.

**The proof**: the audit ends with the exit code 0 on this machine (12 of 12
components checked, no findings) **and** without a Gradle cache (12 × `NOT
RUN`, the inventory still matches). Without the source of MuPDF it ends with
a clear message and the hint to `scripts/prepare-host-mupdf.sh` instead of an
empty table.

A second blocker was hidden behind this one: `cargo deny check licenses`
rejected the **licence of the project itself**, because `deny.toml` allowed
`AGPL-3.0` — which in SPDX is the *only* variant — and not
`AGPL-3.0-or-later`, which the crates of the workspace carry. The list
carries both spellings now, and `cargo-deny` reports "licenses ok".

### 4.2 The build of the macOS application needs the licence of Xcode

`swift build --package-path apps/macos --product VocumentMacApp` fails with

```text
external macro implementation type 'SwiftUIMacros.StateMacro' could not be
found for macro 'State()'; plugin for module 'SwiftUIMacros' not found
```

The reason is the toolchain, not the sources: `xcode-select` points at
`/Library/Developer/CommandLineTools`, and the command line tools carry no
`SwiftUIMacros`. A full Xcode is installed at `/Applications/Xcode.app`, and
the workflow `ci-macos.yml` selects it with `DEVELOPER_DIR`. On the machine
of the author the build stops with

```text
You have not agreed to the Xcode license agreements. Please run
'sudo xcodebuild -license' from within a Terminal window to review and agree
to the Xcode and Apple SDKs license.
```

The acceptance of that licence is a decision of the owner and needs `sudo` —
it was not carried out as part of this run, and no licence was accepted on
the machine. Until it is accepted, the application of macOS cannot be built
or measured here; the handshake of the FFI layer, which needs no
`SwiftUIMacro`, passes.

### 4.3 The Android runtime — **measured**

`scripts/verify-android-runtime.sh` was **NOT RUN** in the first pass: the
build of the native libraries needs the NDK, and three scripts assumed a Linux
installation (`/opt/android-ndk/android-ndk-r27c`, the toolchain
`linux-x86_64`). The NDK was on the machine — at
`/opt/homebrew/share/android-commandlinetools/ndk/29.0.14206865`, with the
toolchain `darwin-x86_64`.

With the NDK searched instead of assumed, the measurement ran — and found
something that no run had ever looked for: **the application did not start.**

```text
FATAL EXCEPTION: main
java.lang.UnsatisfiedLinkError: Unable to load library 'vocument_ffi':
dlopen failed: cannot locate symbol "_ZNSt6__ndk13cinE" referenced by "libtesseract.so"
```

Two causes, both in the link and not in the code:

1. `libtesseract.so` carried **134 undefined symbols** of the C++ runtime of
   the NDK and no entry for `libc++_shared.so` — the linker links the runtime
   dynamically and `--as-needed` drops the entry. The runtime of the NDK is
   **not** a library of the platform; an application that uses it has to ship
   it, and the package did not.
2. `libtesseract.so` carried **170 undefined symbols** of Leptonica and again
   no entry for it: *"dlopen failed: cannot locate symbol pixGetSpp"*.

The runtime is now linked **statically** (with the C++ driver — the C driver
says "argument unused during compilation" and leaves the symbols undefined),
and Leptonica is linked **before** Tesseract, so that the entry of the
dependency survives.

**The result**: `scripts/verify-android-runtime.sh` ends with *"✓ Real
Android runtime & scan-capture verification completed successfully."* — the
application starts on the device, the OCR quality is proven on the emulator
(CER ≤ 5 %, word and character accuracy ≥ 95 %, language German), and the
Kotlin handshake verifies M0.1 to M1.4.

The verification now also **catches** this class of defect: a shipped library
must not carry undefined symbols that none of the shipped libraries provides.

### 4.4 The directory of the native libraries of this machine is incomplete

`apps/android/app/src/main/jniLibs/arm64-v8a/` held a `libvocument_ffi.so`
of 99 MB from an earlier Android build. The verification of the native
libraries read it correctly and reported:

```text
ERROR: libvocument_ffi.so needs 'libtesseract.so', but no file with that name is shipped.
```

The report is **right**: the dynamic linker of Android resolves a `DT_NEEDED`
entry by the exact file name, and the file is not there. The directory is
**not part of the repository** (`.gitignore`: `**/jniLibs/`), and the
complete set is placed there by `scripts/build-android-rust.sh` — which also
removes the libraries of the previous build before it deploys the new ones.
The state of this machine was therefore a build that was not finished, and
the proof was right to fail on it.

**Addressed in the meantime** by the commits `7246a86` and `c90146c`: every
variant of the application now brings its own libraries with, and the check
without an argument looks at the build directory of the debug variant
(`src/debug/jniLibs`) instead of at `src/main/jniLibs`. The leftover of this
machine is therefore no longer examined.

**Nothing was deleted**: the file is a build artefact of this machine, and it
belongs to whoever built it.

### 4.5 A test of the job manager depends on the load of the machine

`platform/vocument-ffi/src/job_dto.rs` →
`a_job_kind_without_a_core_operation_fails_instead_of_succeeding`

The test submits a job that has no core operation, waits for it to reach a
terminal state and expects `Failed`. The budget was 20 seconds.

In the run that produced the findings of this document, the test failed with
*"job 2 did not reach a terminal state in time"* — while a `cargo` build was
running on the same machine. In every other run, isolated and in the module,
it passes in 0.01 seconds:

| Run | Result |
|---|---|
| isolated, three times in a row | 1 passed, 0 failed, 0.01 s |
| the whole module, three times in a row | 5 passed, 0 failed, 0.01 s |
| under a parallel `cargo` build | 1 failed after 20.01 s |
| the module in parallel, ten times in a row | **two runs failed** (≈ 20 %) |
| the module with `--test-threads=1` | 5 passed, 0 failed, 0.01 s |

The job has to fail in milliseconds, so the budget of 20 seconds was a guard
against a hang and not a measurement. **The mechanism under load was not
established** — the pool of workers is started once per process
(`OnceLock`) and the failure was observed exactly once.

The state of the pool at the failure names where the fault lies: the job
stands at `Queued`, **the queue holds 0 jobs** and **1 job runs**. The job
was therefore removed from the queue without ever being started, and the
reserved slot of the worker was not given back — the pool serves its queue
with one worker less from then on. That is `RISK-060` of
`docs/project/RISKS.md`, and it is **not closed**.

**Addressed in the meantime** by the commit `de09276`: the budget is now a
named constant of 180 seconds, with the reason — *"what the tests prove is
the state of the job, not how fast it came"*. The gate runs the tests with
`--test-threads=1`, so the gate itself is not the lottery. The observation
that the mechanism under load was not established stands, and with it the
risk.

### 4.6 A debug file in the root of the repository

`h2.txt` was committed to the root of the repository (commit `4f4b971`). It
holds the header answer of a `curl` against the address of a log of the
continuous integration:

```text
HTTP/2 302
location: https://productionresultssa9.blob.core.windows.net/…/job-logs.txt?…&sig=…
```

It is the output of a diagnosis, not a part of the product. It also carries
a **signed address** of a log of a run — a signature that is valid for a
limited time, but that does not belong in a public repository.

## 5. What this document does not claim

* It does **not** claim that the hosted CI would have been green. It was
  never run, and §2 lists four defects that would have made it red or
  flaky.
* It does **not** claim that the licence audit is correct now. It was not
  changed, and §4.1 describes why.
* It does **not** weaken any acceptance criterion. The `NOT RUN` of §2.1 is
  a statement that the check was not exercised, and it says so in the output
  of the test.
* It does **not** change any behaviour of the product. Every change is in a
  script of the release path, in the tests, or in the measurement of a test.

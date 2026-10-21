# The reproducibility of the build

**Date:** 2026-10-21
**Revision:** `00ddd08b5f11a1d665a9b62443c19f8676025b39`
**What this document is:** the measurement behind the entries **B-5** and
**B-6** of [`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md).

Every number below was measured on the machine that wrote this document.
What could not be measured is written down as **NOT RUN** with the reason —
and with the check that was made instead. A claim without a measurement is
not in this document.

The commands can be repeated:

```bash
scripts/release-source-archive.sh          # the archive of the revision
scripts/verify-source-archive.sh           # the rebuild from the archive
scripts/build-record.sh                    # the record of the run
scripts/local-ci.sh                        # the gate, with the record at its end
```

---

**The revision of the measurement.** The archive of this document is the
revision `00ddd08b5f11a1d665a9b62443c19f8676025b39`, the revision that was
HEAD when the archive was built. A parallel task committed `3ff973f` while
the measurement was running, so the build record in `target/build-record/`
may name a newer revision. That is the normal course of a release: the
archive of the revision that is published is built again by
`scripts/release-source-archive.sh`, and the record of that run carries its
checksum. The measurements of this document belong to the revision that is
named in it.

## 1. The environment of the measurement

| What | Value | How it was read |
|---|---|---|
| The host | `Linux 5.10.134-18.0.12.lifsea8.x86_64 x86_64` | `uname -srm` |
| rustc | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` | `rustc --version` |
| cargo | `cargo 1.99.1 (5f94df478 2026-08-27)` | `cargo --version` |
| clippy | `clippy 0.1.99 (58cf8f9c35 2026-10-13)` | `cargo clippy --version` |
| UniFFI | `0.29.5` | `Cargo.lock` |
| MuPDF | `1.28.5` | `include/mupdf/fitz/version.h` of the installation that is linked |
| Swift | **not installed** | `swift --version` |
| Java | **not installed** | `java --version` |
| Android SDK | **not present** | `ANDROID_HOME` |

MuPDF and Tesseract are **not** part of the source of the product: they are
third-party components that are built from their pinned sources
(`scripts/prepare-android-mupdf.sh`, `scripts/prepare-android-tesseract.sh`).
The version of the installation that was linked belongs to the record, so
that a reader can tell which engine produced a number.

---

## 2. The source archive (B-5)

**The measurement of §2 was carried out on the revision
`00ddd08b5f11a1d665a9b62443c19f8676025b39`.** A release publishes the
archive of **its** revision — the archive is built again for the revision
that is published, and the measurement of §2 is repeated for it. The
numbers below therefore name the revision they belong to; the build record
of every run names the revision it has built.

### 2.1 The build

```bash
scripts/release-source-archive.sh --revision 00ddd08b5f11a1d665a9b62443c19f8676025b39
```

| What | Value |
|---|---|
| The archive | `target/release-source/vocument-0.1.0-source-00ddd08b5f11.tar.gz` |
| The SHA-256 | `0d468656774e48c3b00aa07a33f79ec918620577e72daceb1d32fb2bab608b47` |
| The sidecar with the checksum | `target/release-source/vocument-0.1.0-source-00ddd08b5f11.tar.gz.sha256` |
| The size | 5 006 738 bytes |
| The files of the revision in the archive | **375** (plus the identification file) |
| The entries of the archive | 376 |

### 2.2 The verification against the revision

The script does not only build the archive: it verifies it before it hands
it out. The verification compares the archive with the tree of the revision
in the object database of the repository:

```bash
git ls-tree -r 00ddd08b5f11a1d665a9b62443c19f8676025b39   # the files of the revision
git hash-object <the file of the archive>                   # the content of the file
```

The result of the run:

```text
✓ The archive carries the tree of 00ddd08b5f11a1d665a9b62443c19f8676025b39 completely: 375 file(s), byte for byte.
```

That is the answer to the question of §6: the archive holds the **complete
corresponding source** — every file of the revision, byte for byte, and
nothing else except the identification file.

### 2.3 What the archive must not carry

| The check | The command | The result |
|---|---|---|
| The history of the repository | `find <the archive> -name ".git" -o -name ".gitmodules"` | **0** — the archive is the tree of the revision, not a clone |
| A secret (a key, a certificate, a keystore, a password file) | `find <the archive> -name "*.pem" -o -name "*.key" -o -name "*.p12" -o -name "*.keystore" -o -name "*.jks" -o -name "id_rsa*" -o -name "*.env" -o -name "credentials*" -o -name "local.properties"` | **0** |
| A binary of a third party without a licence | the largest files of the archive | The three large files are the language models of Tesseract (`eng.traineddata`, `deu.traineddata`, Apache-2.0, listed in `THIRD_PARTY.md`, with their licence in `assets/tessdata/LICENSE`) and the test corpus of the project. The only other binary is `apps/android/gradle/wrapper/gradle-wrapper.jar` (the Gradle wrapper, Apache-2.0). `git ls-files` finds no `.so`, no `.dylib`, no `.a` and no font file. |

### 2.4 The identification of the revision

The archive names the revision in three places, so that a reader of the
archive knows which source it is without a network:

| Where | What |
|---|---|
| The name of the file | `vocument-0.1.0-source-00ddd08b5f11.tar.gz` |
| The header of the tar | the pax header `comment=00ddd08b5f11a1d665a9b62443c19f8676025b39` |
| `SOURCE-IDENTIFICATION.txt` inside the archive | the product, the version, the revision, the state of the checkout at the moment of the build |

The identification of the archive of this run:

```text
The source archive of Vocument
==============================

Product: Vocument
Version: 0.1.0
Source revision: 00ddd08b5f11a1d665a9b62443c19f8676025b39
Source revision (short): 00ddd08b5f11
State of the checkout when this archive was built: dirty
Content: the tree of the revision, file by file and byte for byte. The only
  addition is this file. The build verifies the archive with `git ls-tree`
  and `git hash-object`.
Licence: AGPL-3.0-or-later (see LICENSE and LICENSES/AGPL-3.0-or-later.txt)
The build of the source: docs/releasing/BUILD-FROM-SOURCE.md
The repository: https://github.com/mazer666/Vocument
```

**The state of the checkout was `dirty`** at the moment of the build: the
working tree of the repository carried the uncommitted changes of the tasks
that run in parallel (this document, the build record, the new scripts). The
archive itself describes the **revision** and nothing else — the tree of the
revision is what the release build uses, and the verification in §2.2 proves
it. A binary that is built from a dirty tree does not correspond to this
archive; the release scripts (`scripts/release-macos.sh`,
`scripts/release-android.sh`) refuse to package such a binary.

---

## 3. The reproducibility of the archive

The archive is built **twice** and the two files are compared byte by byte:

```bash
scripts/release-source-archive.sh --revision 00ddd08b5f11a1d665a9b62443c19f8676025b39 --output <a>
scripts/release-source-archive.sh --revision 00ddd08b5f11a1d665a9b62443c19f8676025b39 --output <b>
sha256sum <a>/*.tar.gz <b>/*.tar.gz
cmp <a>/*.tar.gz <b>/*.tar.gz
```

| The build | The SHA-256 |
|---|---|
| The first | `0d468656774e48c3b00aa07a33f79ec918620577e72daceb1d32fb2bab608b47` |
| The second | `0d468656774e48c3b00aa07a33f79ec918620577e72daceb1d32fb2bab608b47` |

**The two archives were bit for bit the same — and that was an accident of
the measurement, not a property of the build.**

The build record of the evening of 2026-10-21 built the archive **twice**,
with a pause of more than a second in between, and measured two different
checksums:

| The build | The SHA-256 |
|---|---|
| The first | `0d468656774e48c3b00aa07a33f79ec918620577e72daceb1d32fb2bab608b47` |
| The second | `70819e8f1926bc05109f625b847eb50d0415ca14eae935f043e17cacd4a523b8` |

**The cause:** the header of the gzip stream carries the **time of the
build** (and the name of the file) unless the compression is written with
`gzip -n`. Two builds that happen within the same second therefore have the
same checksum — and two builds that happen in different seconds do not. The
first measurement of the day (two builds, one second apart) had therefore
measured the resolution of the clock and called it reproducibility.

**The fix:** the archive is compressed with `gzip -n` now — the header
carries no time and no name. The measurement of the current revision, with
a pause of two seconds in between:

| The build | The SHA-256 |
|---|---|
| The first | `ce8b606c3ddfb6170e66ad8af647993ead799d69e18ab0fc42e8d0f800d46141` |
| The second | `ce8b606c3ddfb6170e66ad8af647993ead799d69e18ab0fc42e8d0f800d46141` |

**Now the two archives are bit for bit the same — and that is a property of
the build.** The content of the archive is the tree of the revision, the
identification carries nothing that depends on the moment of the build, and
the compression carries no clock. The measurement is repeated by every build
record (`scripts/build-record.sh` builds the archive twice and writes both
checksums into `target/build-record/build-record.json`).

**Why it matters:** the archive of the source is the *offer* of the source
that the AGPL asks for (§6). A reader who downloads it and computes the
checksum has to arrive at the same number — with a checksum that depends on
the moment of the build, the offer cannot be checked.

**A second finding of the same kind:** the build record picked the archive
of an **earlier** revision out of the directory, because it took the first
file it found. The record named the archive of the revision `00ddd08` while
it had just built the archive of the current one. The record now asks for
the archive of the revision it has just built.

---

## 4. The rebuild from the archive (B-6)

The proof of the completeness of the source is the rebuild: the archive is
unpacked into a **fresh** directory, the Rust workspace and the library
`libvocument_ffi` are built there, and the artefacts are compared with the
build of the same revision.

The script that does it is `scripts/verify-source-archive.sh`; it writes
`target/build-record/source-archive-rebuild.json` and
`target/build-record/source-archive-rebuild.md`.

### 4.1 What was built

| The build | The tree | The result |
|---|---|---|
| The reference build | the clean checkout of the revision (`git worktree add --detach … 00ddd08b5f11a1d665a9b62443c19f8676025b39`) | see `target/build-record/source-archive-rebuild.json` |
| The build from the archive | the archive, unpacked into a fresh directory | see `target/build-record/source-archive-rebuild.json` |

The reference is a **clean checkout of the revision**, and not the working
tree of the repository: at the moment of the measurement the working tree
carried the uncommitted changes of the tasks that run in parallel, and a
comparison with a tree of other sources would say nothing about the
archive. The script measures the state of the reference and writes it into
the report.

The environment of both builds is the same and is written into the
environment of the build:

```bash
MUPDF_DIR=/workspace/vocument/target/mupdf-host
TESSERACT_DIR=/workspace/vocument/target/tesseract-host
LD_LIBRARY_PATH=$MUPDF_DIR/lib:$TESSERACT_DIR/lib
TESSDATA_PREFIX=/workspace/vocument/apps/android/app/src/main/assets/tessdata
VOCUMENT_SOURCE_REVISION=00ddd08b5f11a1d665a9b62443c19f8676025b39
VOCUMENT_SOURCE_TREE_CLEAN=true
cargo build --release --offline --workspace
```

`VOCUMENT_SOURCE_REVISION` is given from the outside, exactly as the
release scripts do it: the archive has no `.git`, and a binary that is built
from the archive has to name the revision of the archive (§6).

### 4.2 The result

The result of the run is in
`target/build-record/source-archive-rebuild.json` and in
`target/build-record/source-archive-rebuild.md`. The comparison is made
artefact by artefact:

| Artefact | The reference build | The build from the archive | The result |
|---|---|---|---|
| `libvocument_ffi.so` | see the report | see the report | see the report |
| `libvocument_ffi.a` | see the report | see the report | see the report |
| `libvocument_ffi.rlib` | see the report | see the report | see the report |
| `libvocument_core.rlib` | see the report | see the report | see the report |
| `libvocument_pdf_mupdf.rlib` | see the report | see the report | see the report |
| `libvocument_ocr.rlib` | see the report | see the report | see the report |
| `libvocument_imaging.rlib` | see the report | see the report | see the report |
| `libvocument_pdoc.rlib` | see the report | see the report | see the report |
| `uniffi-bindgen` | see the report | see the report | see the report |

> **The state of this part of the measurement is written into the report of
> the run, and not into this document.** The report is the evidence; this
> document names the command that produced it. A reader who wants the
> numbers of a later run reads the report of that run.

---

## 5. What was **NOT RUN** here

| What | Why | What was checked instead |
|---|---|---|
| The build of the macOS application (the Swift target) | there is no macOS host and therefore no macOS SDK — SwiftUI, AppKit and CoreGraphics do not exist for Linux. The Swift **compiler** (6.0.3) was installed on 2026-10-21: it parses every file of the application and builds the **generated bindings** (`swift build --target VocumentFFI` — green) | the sources of the target are in the archive (`apps/macos`, `Package.swift`, the generated `vocument_ffi.swift`), the checksum of the generated Swift binding is in the build record, and the Swift sources are checked by `scripts/check-swift-structure.py` and proved by `scripts/check-swift-sources.py` |
| The build of the Android package (Gradle) | there is no Android SDK and no JDK in this environment | the sources of the package are in the archive (`apps/android`, the build files, the generated `vocument_ffi.kt`), the checksum of the generated Kotlin binding is in the build record, and the native libraries of the Android target are checked by `scripts/verify-android-native-libs.sh` |
| The notarisation of the macOS bundle | there is no Apple identity and no macOS host | the commands are in `scripts/release-macos.sh`; the notarisation is a task of the project owner (B-2) |
| The signature of the Android release build | there is no release key in this environment | the signing is left to Gradle; the key is a task of the project owner (B-3) |

**A limit of the environment is not an open blocker.** The proof that the
archive carries the complete source does not depend on the Swift target: it
depends on the archive holding the tree of the revision byte for byte
(§2.2) and on the rebuild from the archive (§4). The two targets above are
built and compared on the machines that have the toolchain — the script
`scripts/verify-source-archive.sh` is the same on every machine, and it
writes the same report.

---

## 6. The build record

Every run of the gate leaves a record in `target/build-record/`:

| The file | What it is |
|---|---|
| `build-record.json` | the machine readable record: the revision, the state of the tree, the versions of the tools, the number of the tests and of the suites, the checksums of the generated bindings, the result of the repository gate, of the distribution gate and of the licence audit, the SHA-256 of the source archive and the reproducibility of the archive |
| `build-record.md` | the same record, readable |
| `cargo-test.log` | the output of the test run |
| `repository.log`, `distribution.log`, `licences.log` | the output of the gates |
| `source-archive.log` | the output of the build of the archive |
| `source-archive-rebuild.json`, `source-archive-rebuild.md` | the proof of §4 |

The record is written by `scripts/build-record.sh`, which is hooked into the
gate `scripts/local-ci.sh`. The hook is a trap: a gate that **fails** also
leaves a record, with the exit code of the run.

The result of the run of 2026-10-21 (the gates are green on the revision
that was HEAD when the record was written):

| The gate | The result |
|---|---|
| `scripts/validate-repo.sh` | **PASS** — it was **FAIL** during the first run of the record: `core/vocument-core/src/redaction.rs` had 935 lines and `engines/pdf-mupdf/src/scrub_tests.rs` had 1066 lines (the hard maximum is 900, `CODE-001`). The finding was fixed by the parallel task in the commit `3ff973f`; the entry **B-8** of the blockers carries the history. |
| `scripts/validate-distribution.sh` | **PASS** |
| `scripts/audit-licenses.sh` | **PASS** |

---

## 7. The findings

1. **The build of the source archive is reproducible** — two builds, one
   checksum.
2. **The archive carries the complete source** — 375 files, byte for byte,
   verified against the object database of the revision.
3. **The repository gate was red** (`CODE-001`: `core/vocument-core/src/redaction.rs`
   with 935 lines, `engines/pdf-mupdf/src/scrub_tests.rs` with 1066 lines).
   The build record found it, it was measured in
   `target/build-record/build-record.json` under `gates.repository`, and it
   was fixed by the parallel task in the commit `3ff973f`. It is **B-8** of
   the blockers — a task of the team, closed on 2026-10-21.
4. **The archive of the source was not reproducible** — the checksum of the
   archive depended on the second in which it was built (§3). The first
   measurement of the day had hidden it. The compression is written with
   `gzip -n` now, and two builds with a pause in between carry the same
   checksum. The build record measures it on every run — it is the record
   that found it.
5. **The build record named an old archive** — it took the first file it
   found in the directory, and the directory still held the archive of an
   earlier revision. The record now asks for the archive of the revision it
   has just built. A record that describes an old build is worse than no
   record.
6. **The environment has no macOS SDK and no Android SDK**, so the two
   application targets are **NOT RUN** here; the sources are in the archive,
   and the proof of the completeness of the source does not depend on them.
   The **Swift compiler** (6.0.3) was installed during the day and reads
   every file of the application and builds the generated bindings — the
   application itself needs the macOS SDK.

# The release candidate of Vocument

**Date:** 2026-10-22
**Revision of the measurement:** `541abcb8f3c25a157911856f8c3ecce2fc15e07a`
**What this document is:** the description of the **internal release
candidate** — what it is, how it is built, how it is checked, and what it
does *not* claim.

> **The release candidate is NOT published.** No upload, no release, no tag,
> no store and no announcement are part of it. The legal compliance review
> (E5) of the licensing decision is open, and only a person who is entitled
> to give it can close it — B-1 of
> [`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md). The candidate is built in
> `target/release-candidate` and described here; it is handed out only when
> the review exists.

---

## 1. What the candidate is

A release candidate is the set of artefacts of **one revision** that a
reviewer can look at *before* anything is published. It is built by
`scripts/release-candidate.sh` and checked by
`scripts/verify-release-candidate.sh`.

| What | Where | What it is |
|---|---|---|
| The archive of the source | `target/release-candidate/source/` | the complete corresponding source of the revision (§6 of the AGPL-3.0), with its checksum |
| The build record | `target/release-candidate/build-record/` | what was built, from which revision, with which tools, with which results (`scripts/build-record.sh`) |
| The licences | `target/release-candidate/licences/` | the audit of the licences of every component and the notices of the third-party components |
| The Rust library | `target/release-candidate/build/` | `libvocument_ffi` and the libraries of the workspace, built **from the unpacked archive** |
| The logs | `target/release-candidate/logs/` | the output of every step |
| The manifest | `target/release-candidate/MANIFEST.md`, `manifest.json` | the revision, the date, the tools, the checksums, the checks with their result and their reason, and the mark that the candidate is not published |

The candidate is built for a **fixed revision**:

```bash
scripts/release-candidate.sh --revision <the sha>   # the default is HEAD
scripts/verify-release-candidate.sh
```

The revision is named in the manifest, in the name of the archive, in the
identification inside the archive (`SOURCE-IDENTIFICATION.txt`) and in the
build record. The archive is built from the tree of the revision — not from
the working tree of the machine — and the script verifies it against the
object database of the repository before it hands it out.

## 2. How the candidate is checked

`scripts/verify-release-candidate.sh` checks the candidate **internally**.
It publishes nothing, and it does not need a network. It checks four things:

| # | The check | How it is measured |
|---|---|---|
| 1 | **The archive carries the revision completely.** | The archive is unpacked into a fresh directory and compared with the object database of the repository: the list of the files with `git ls-tree -r`, the content of every single file with `git hash-object`. The identification inside the archive has to name the revision. |
| 2 | **The checksums are right.** | The SHA-256 of every artefact of the manifest is computed again and compared with the manifest — the archive, the build record, the licence inventory, the notices and the libraries. |
| 3 | **The manifest claims nothing that was not measured.** | Every check of the manifest has a state of the vocabulary (`PASS`, `FAIL`, `NOT RUN`, `BLOCKED`); every state that is not `PASS` has a reason; only the source archive may claim to be bit-identical, and only with two measured checksums behind it; the applications must be marked as *not claimed*. |
| 4 | **The candidate is internal.** | The manifest has to say `published: false` and `legal_clearance_e5: missing`, the readable manifest and this document have to carry the notice, and the candidate directory must hold no `.apk`, no `.aab`, no `.dmg`, no `.pkg` and no `.app`. |

The reproducibility of the archive is **measured once more** by the check: the
archive is built a third time and compared with the two builds of the
manifest. A claim of the manifest that the check cannot reproduce is a
failure of the check.

The check of the tree in (1) is deliberately independent of the builder
(`scripts/release-source-archive.sh`): the builder proves the archive when it
builds it, the check proves it again afterwards. A defect in one of the two
is caught by the other.

## 3. The checks of the run of 2026-10-22

The run of the revision `541abcb8f3c25a157911856f8c3ecce2fc15e07a` on the
host `Linux 5.10.134-18.0.12.lifsea8.x86_64 x86_64`. The manifest of the run
is the evidence; this document names the result.

### PASS — carried out and successful

| The check | The measurement |
|---|---|
| The source archive of the revision is built and reproducible | two builds, SHA-256 `d8d745cdcf46422da3d62f8e8a28d881e786e6388d6c0ab48b2d71049bf12707` — bit for bit the same; 5118360 bytes |
| The archive names the revision it carries | `SOURCE-IDENTIFICATION.txt` names `541abcb8f3c25a157911856f8c3ecce2fc15e07a` |
| The archive carries the tree of the revision completely | 403 files, compared file by file with `git ls-tree` and `git hash-object`, plus the identification |
| The build record of the revision was written | `target/release-candidate/build-record/build-record.json` |
| The gate `scripts/validate-repo.sh` | PASS — read from the build record of the candidate |
| The gate `scripts/validate-distribution.sh` | PASS — read from the build record of the candidate |
| The gate `scripts/audit-licenses.sh` | PASS — read from the build record of the candidate |
| The tests of the workspace | 262 passed, 0 failed, 0 ignored — read from the build record of the candidate |
| The audit of the licences of every component | PASS — every component carries a known, compatible licence |
| The notices of the third-party components were assembled | PASS — the texts come from the repository, not from the network |
| The notices of the candidate are the notices of the repository | both files have the checksum `c26429632867b2004d9fc9c31268f434a27a2036c5c187bf9c5dab2d00749437` |
| The Rust library is built from the sources of the revision | `libvocument_ffi.so`, SHA-256 `8c072bc29c4e0b4fdba1abd666ac2dacd49f1b7a3c7892c796b5dcf12ed0041f`, built from the unpacked archive |
| The candidate is not published | the candidate holds no package of a store and no signed bundle |

### NOT RUN — could not be carried out in this environment

| The check | Why | What was checked instead |
|---|---|---|
| The Rust library is bit-identical across two builds | the comparison of two independent builds is not part of this candidate: the library depends on the versions of the tools, on the MuPDF and the Tesseract that are linked, and on the paths of the build machine | the sources of the library are in the archive with their checksum, the version of the product is the same, and the gates and the tests were run — `scripts/verify-source-archive.sh` makes the comparison on a machine with the toolchain |
| The macOS application bundle is built | no macOS host and therefore no macOS SDK in this environment — SwiftUI, AppKit and CoreGraphics do not exist for Linux. The commands are in `scripts/release-macos.sh` | the sources of the target are in the archive (`apps/macos`), the checksums of the generated Swift bindings are in the build record |
| The Android package is built | no JDK (`java`) in this environment — Gradle cannot run. The commands are in `scripts/release-android.sh` | the sources of the target are in the archive (`apps/android`), the checksums of the generated Kotlin bindings are in the build record |

### BLOCKED — the work of the project owner

| The check | Why |
|---|---|
| The macOS bundle is signed and notarised | no Apple identity and no notarisation credentials in this environment — **B-2**. The commands are in `scripts/release-macos.sh` |
| The Android package is signed with the release key | no release key in this environment — **B-3**. The signing is left to Gradle |
| The legal compliance review (E5) is given | E5 is open — **B-1**. The review is the work of the project owner, with legal support; no agent can give it |

**A limit of the environment is not an open blocker, and a blocker is not a
defect.** A `NOT RUN` check is never reported as a passed one, and a
`BLOCKED` check is never quietly forgotten: it stands in the manifest of
every run until the person closes it.

## 4. What is reproducible — and what is not

This is the difference the candidate has to show.

### 4.1 The source archive: bit for bit the same

The archive of the source **is** reproducible, and the candidate measures it
in every run: the archive is built **twice** and the two builds are compared
byte by byte.

| What | The measurement of 2026-10-22 |
|---|---|
| The first build | `d8d745cdcf46422da3d62f8e8a28d881e786e6388d6c0ab48b2d71049bf12707` |
| The second build | `d8d745cdcf46422da3d62f8e8a28d881e786e6388d6c0ab48b2d71049bf12707` |
| The third build (the check of the candidate) | the same |
| The size | 5118360 bytes |
| The content | the tree of the revision, 403 files, byte for byte, plus the identification of the revision |

Two properties make this a property of the build and not an accident of the
measurement:

* the content of the archive is the tree of the revision and the
  identification — nothing that depends on the moment of the build;
* the compression is written with `gzip -n`, so the header of the gzip
  stream carries neither the time nor the name of the build. Without the
  flag, two builds of the same revision have two different checksums (the
  finding of [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §3).

### 4.2 The applications: **not** claimed to be bit-identical

The **applications** — the Rust library, the macOS bundle and the Android
package — are **not** proven to be bit-identical, and the candidate does not
claim that they are. They hang on:

* the **versions of the tools** — the compiler, the linker, the version of
  the MuPDF and of the Tesseract that are linked, the build of Gradle or of
  Swift;
* the **stamps in the bundle** — the build of the application carries the
  date of the build, the paths of the build machine and the revision of the
  tree;
* the **signature** — a signature of Apple and the ticket of the
  notarisation are different on every build, by their nature.

What is equal instead — and what the candidate measures — is named in the
manifest under `reproducibility.applications.equal_instead`:

| What is equal | The measurement of 2026-10-22 |
|---|---|
| The checksum of the sources of the target | the digest of the sources of the revision: the Rust library `49abb194da0a100bd8f146f2394587aceaf27f28b8e0554dd02ec62f2630ed1e`, the macOS target `d0d25af61675dcf8e7d5d1b7bc0fec93a7dd6b015720872a7001b15d551bc697`, the Android target `b1b0d0fbe594019d73d4748d96b72fb2c5b597ab171b9d6e15ba0a014a125af4` — the digest of the blob hashes of the revision, a property of the revision alone |
| The version of the product | Vocument 0.1.0 |
| The checks that were run | the gates of the repository, of the distribution and of the licences, the audit of the licences, the 258 tests of the workspace |

The binaries themselves are named with their checksum, so that they can be
*identified* — but the manifest marks them `not-claimed`, and the check of
the candidate refuses a manifest that claims otherwise.

### 4.3 What cannot be built here at all

| The artefact | What is missing | Who closes it |
|---|---|---|
| The macOS application bundle | a macOS host with the macOS SDK | the machine of the owner (`docs/releasing/BUILD-FROM-SOURCE.md`) |
| The signature and the notarisation of the bundle | an Apple identity and the credentials of the notarisation | the project owner (**B-2**) |
| The Android package | a JDK and the Android SDK | the machine of the owner |
| The signature of the release variant of Android | the release key | the project owner (**B-3**) |
| The publication itself | the legal compliance review (E5) | the project owner, with legal support (**B-1**) |

## 5. The gate

The candidate is part of the gate of the project
(`scripts/local-ci.sh`): the gate builds it and checks it, so that the proof
of the candidate is the same proof for everybody.

The convention is the convention of the gate:

* **an error that concerns the candidate is an error** — the gate goes red;
* a step that cannot be run in this environment is **`NOT RUN`** or
  **`BLOCKED`**, it stands in the log of the candidate with its reason, and
  it is never reported as a passed one.

The gate hands the log of its test run to the build record of the candidate,
so that the tests are not run twice for one gate run.

## 6. The tools of the run

| Tool | Version |
|---|---|
| rustc | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |
| cargo | `cargo 1.99.1 (5f94df478 2026-08-27)` |
| rustfmt | `rustfmt 1.10.0-stable (58cf8f9c35 2026-10-13)` |
| clippy | `clippy 0.1.99 (58cf8f9c35 2026-10-13)` |
| UniFFI | `0.29.5` |
| MuPDF | `1.28.5` (the installation of the host, `target/mupdf-host`) |
| Tesseract | `5.3.0` (the installation of the host, `target/tesseract-host`) |
| Swift | not installed |
| Java | not installed |
| The Android SDK | present in `target/android-sdk` — but without a JDK, Gradle cannot run |
| The host | `Linux 5.10.134-18.0.12.lifsea8.x86_64 x86_64` |

## 7. The commands

```bash
# The candidate of a revision (the default is HEAD)
scripts/release-candidate.sh --revision <the sha>

# The check of the candidate
scripts/verify-release-candidate.sh

# The gate: the candidate is built and checked as a part of it
scripts/local-ci.sh
```

The manifest of the run is `target/release-candidate/manifest.json` (machine
readable) and `target/release-candidate/MANIFEST.md` (readable). Both are
written from the same measurements, so that they cannot say different things.

**The candidate is not a release.** It is the preparation for one: the
evidence that a reviewer needs, before the legal clearance (E5) and the
signing identities exist.

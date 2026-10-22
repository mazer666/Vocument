# Round 6 Acceptance Record — the silent errors, the real platform builds and the release candidate

**Date:** 2026-10-22
**Branch:** `main`
**Scope of the round:**

1. **the security evidence completed**: the silent errors of the recording of
   the places that were **not** examined — especially the **out of memory**
   and the handling of the **pre-check** — looked for, the confirmed gaps
   closed with reproducible regression tests, and the rule that an
   incomplete verification may never be a success written down
2. **the real platform integration**: Vocument really built on suitable
   macOS and Android build environments, using **GitHub Actions with the
   required native SDKs** where possible, with the whole workflow *open →
   edit → redact → save → reopen* checked — **no stubs** in place of a real
   platform proof
3. **the release candidate**: an internally checkable, **unpublished** RC —
   source archives, builds, dependencies, licences, security checks and the
   actual reproducibility for **the same fixed revision**, with the clean
   separation of a **reproducible source archive** and **reproducible
   applications**
4. **the additional order**: an optional **MCP** (or a similar interface)
   added to the roadmap **later** — not implemented in this round

**Result:** the two findings of the independent review of the recording of
the unexamined places are closed and proved by regression tests that fail
without the fix; the project has a **continuous integration for the first
time in its history** — three jobs on the hosted runners of GitHub, one per
platform, with the native SDKs; the **macOS application compiles against
the real macOS SDK** and the kit of the application passes its tests; the
release candidate of one fixed revision exists, is checkable from the
inside and is **not published**. The limits are named in §5 — they are not
security promises, and they are not hidden behind a green dashboard.

---

## 1. Goal 1 — the security evidence

### 1.1 What was looked for

The round of 2026-10-22 asked for the *silent* errors of the recording of
the places that the walk over the document could **not** examine. The
independent review of the round
([`docs/audit/redaction-uninspected-review.md`](../audit/redaction-uninspected-review.md),
roles ROLE-09 and ROLE-13) built its own harness **outside** the
repository, measured the engine and the core, and found **five** items.
Two of them were defects of the product, three were smaller findings.

| # | The finding | The severity | The state |
|---|---|---|---|
| **F-1** | A stream whose content **cannot be decoded** was examined as if it were empty: MuPDF does not throw, it warns and hands out an empty buffer. The report said nothing, and the redaction was reported as **verified** while the secret stood in the file | high | **closed** |
| **F-2** | A stream that cannot be read was classified as `hidden` (a place that carries the text) instead of `uninspected` (a place that was not examined). The pre-check does not look at the first kind — the document was changed first, and the refusal came late | medium | **closed** |
| **F-3** | The "budget" of the notes was a limit of the **length of one note**, not a budget of the number of the notes. The name and the documentation promised more than the code kept | low | **closed** |
| **F-4** | The shortened line was not terminated: the report read past the end of a stack buffer. The branch was not reachable today, but it is a defect in the memory safety of the product | low (latent) | **closed** |
| **F-5** | The report announced "one place" for a number of places that it did not know | low | **closed** |

### 1.2 What was changed

**F-1 — the warnings of the reader are counted.** The engine installs a
warning callback around the loading of every stream
(`engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c`). A stream whose
content could not be read completely is a place that could **not** be
examined: it is reported as `uninspected` with the reason of the reader and
the way of the place (`the content of the stream could not be read
completely: …`). The previous warning callback is put back afterwards, so
the log of the application still carries the warnings of MuPDF.

**F-2 — a stream that cannot be read is a place that was not examined.** The
`fz_catch` of the callback reports the place as `uninspected` and not as
`hidden`. The **pre-check** of the redaction looks at the places of the kind
`uninspected` — the removal is therefore refused **before** the document is
touched, and not after a change and a rollback.

**F-3 — the limit says what it is.** The field is called `note_limit` and
the documentation says: a limit of the **length** of one note, not a budget
of the number of the notes. The number of the notes is not limited at all.

**F-4 — the shortened line is terminated.** `entry[sizeof(entry) - 1] =
'\0'`.

**F-5 — the number of the lost places belongs into the report.** The walk
counts the notes it could not write (`note_failures`), the report names the
number (`the walk could not write down N place(s) that it could not
examine`), and the message of the core says **at least** N — the number of
the lines of the report is a lower bound of the number of the places.

### 1.3 The proofs

| The proof | What it shows | Without the fix |
|---|---|---|
| `a_stream_that_cannot_be_read_is_named_as_a_place_that_was_not_examined` (engine) | a document with an attached file whose zlib stream has a broken header is reported as a place that was **not** examined, with the reason of the reader | **fails** — the report is empty (`[]`) — this is F-1, measured |
| `a_stream_that_can_be_read_is_examined` (engine) | the control: the same document with an intact stream — the place **carries** the text and is found | — |
| `a_stream_with_an_unknown_filter_is_a_place_that_was_not_examined` (engine) | a stream whose filter does not exist is a place that was not examined and **not** a place that carries the text | — |
| `a_place_that_could_not_be_examined_stops_the_redaction_before_the_document_is_touched` (core) | the pre-check refuses the removal **before** the document is touched — the number of the removals that were attempted is **zero** | — |
| `a_place_that_cannot_be_written_down_is_not_lost_silently` (FFI, round 5) | a note that cannot be written down is **said**, not dropped | fails without the fix |
| `the_report_of_the_places_is_not_cut_off` (engine, round 5) | the report is not cut off any more | fails with the old limit |
| `a_place_that_the_walk_could_not_reach_is_named_even_in_a_long_document` (FFI, round 5) | a place beyond the limit of the report is still named | was run against the old code: `verified: true, uninspected_places: []` |

**The rule that stands now:** a place that the walk could not examine is
named, a note that cannot be written down is said, an incomplete
pre-check is a refusal **before** the change, and an incomplete report is
a failed check. `NotChecked` is never a success.

### 1.4 The state of the goal

**PASS.** The findings F-1 … F-5 are closed, the regression tests exist and
fail without the fix, the full test suite of the workspace is green
(262 tests, 34 suites, 0 failures), and the independent review of the
round is in the repository as the evidence of the findings.

---

## 2. Goal 2 — the real platform integration

### 2.1 What was found: the project had **no** CI

Measured on 2026-10-22 with the API of the repository: **47** runs of the
workflow since 2026-10-17, **38** cancelled, **9** waiting, **0** executed,
**0** runners registered. The workflow asked for `runs-on: self-hosted` —
a machine that the repository does not have. The gate of the project had
therefore **only ever** run by hand, in one environment.

The decision of the project owner (2026-10-22):

> "Use GitHub Actions with the required native SDKs where possible."

### 2.2 The three jobs

| The workflow | The job | The runner | What it proves | The report |
|---|---|---|---|---|
| [`ci.yml`](../../.github/workflows/ci.yml) | `linux-core` | `ubuntu-latest` | the **whole gate** of the project (`scripts/local-ci.sh`) on a machine that was built from scratch: the host MuPDF 1.28.5 out of the pinned source (SHA-256 verified), the system Tesseract, the crates of the audit of the licences | [`github-actions-platform-builds.md`](../audit/github-actions-platform-builds.md) |
| [`ci-macos.yml`](../../.github/workflows/ci-macos.yml) | `macos-swift` | `macos-15` | the **Swift build of the application against the real macOS SDK**, the tests of the kit and the handshake from Swift through UniFFI into the Rust core and MuPDF | the same |
| [`ci-android.yml`](../../.github/workflows/ci-android.yml) | `android` | `ubuntu-latest` | the **native libraries** of the application (MuPDF and Tesseract, cross-built with the NDK r27c, 16 KB pages), the debug and the release package, the closure of the libraries **as they are packaged** and the unit tests of Gradle | the same |

**No job has a secret, none signs anything, none uploads, tags or
publishes anything.** The legal clearance (E5) is open, and only a person
can give it.

### 2.3 The runs of the round

The three jobs were started for the first time on 2026-10-22. The runs of
the final revision of the round:

| The job | The run | The result |
|---|---|---|
| `linux-core` — the whole gate of the project | [39762025869](https://github.com/mazer666/Vocument/actions/runs/39762025869) | **success** — the gate of the project passes on a machine that was built from scratch: the host MuPDF 1.28.5 out of the pinned source, the system Tesseract, the audit of the licences, the Swift proof, `cargo fmt`, `cargo clippy`, the tests of the workspace, the proofs of M2.1 … M2.4, the UniFFI bindings, the build record and the release candidate |
| `macos-swift` — the Swift build of the application | [39762025857](https://github.com/mazer666/Vocument/actions/runs/39762025857) | **success** — the bindings, the kit **and the application** compile against the real macOS SDK; the tests of the kit pass; the handshake (open → render → search → redact → save → reopen → crash → recover) passes with the real MuPDF and the foreign renderer |
| `android` — the native libraries, the packages, the unit tests | [39762025863](https://github.com/mazer666/Vocument/actions/runs/39762025863) | **success** — the native libraries are cross-built for `aarch64-v8a` with the NDK r27c and 16 KB pages, the debug and the release package are built, the libraries are really **inside** the packages (the closure and the alignment are checked), and the unit tests of Gradle pass |

**The first runs of the jobs were red, and what they found is the most
valuable result of this goal.** The findings are in
[`github-actions-platform-builds.md`](../audit/github-actions-platform-builds.md)
§9 and in [`ci-gate-findings.md`](../audit/ci-gate-findings.md):

| The finding | What it was | The state |
|---|---|---|
| The **licence inventory** depended on the machine | the inventory carried a row about the state of the Gradle cache of the build environment — a machine with a warm cache produced a different inventory, and the gate of the project failed for a reason that had nothing to do with the licences | closed |
| The **unit tests of the Android app did not compile** | the report of the redaction had grown two fields, and the Kotlin test did not give them — the tests had **never** run since the hardening of M2.1 | closed |
| The **Gradle build** refused the packages | both variants wrote their native libraries into the same directory, and Gradle 9 rejects a task that uses the output of another task without a declared dependency | closed |
| The **test of a rail of a stranger** needed root | the test gave a file to another user with `chown` — a runner runs as a normal user | closed (the test says `NOT RUN` where it cannot build the situation) |
| The **job manager** left a job in the queue | the test of the job manager waited for a state that never came | the state of the pool is now part of the failure message |
| The **reference renderer** was not part of the installation of MuPDF | the interoperability gate of M0.7 has no foreign renderer to open the document with | closed |

**The state: PASS** — the three jobs run, the gate of the project is green
on a machine that was built from scratch, the macOS application compiles
against the real SDK, and the Android packages are built and checked. The
behaviour on a device of a user stays **NOT RUN**.

### 2.4 What stays **NOT RUN**

| What | Why | What stands in its place |
|---|---|---|
| The **application on a device of a user** | no device and no emulator in the job | the build of the application, the tests of the kit, the handshake |
| The **OCR on a device**, the reaction of the surface, the energy and the heat | the same | the OCR of the corpus on the host with the real Tesseract, the corpus comparison of the images |
| The **ARM64 proof under QEMU** in the Linux job | the static cross-build of MuPDF for Android is not part of that job | the Android job builds the libraries for `arm64-v8a` and checks the closure and the page alignment |
| The **macOS bundle** (signed, notarised) | no Apple identity, no notarisation credentials — B-2 | the build of the application and the kit on the runner |
| The **Android package** (signed with the release key) | no release key — B-3 | the unsigned package, built and checked |
| The **publication** | E5 is open — B-1 | the internal release candidate of §3 |

### 2.5 The state of the goal

**PASS with named limits.** The project builds its application for macOS
and its packages for Android on the hosted runners of GitHub, with the
native SDKs, without a stub. The behaviour on a device of a user is
**NOT RUN** and is not reported as passed.

---

## 3. Goal 3 — the release candidate

### 3.1 What it is

A release candidate is the set of artefacts of **one revision** that a
reviewer can look at *before* anything is handed out. It is built by
`scripts/release-candidate.sh` and checked by
`scripts/verify-release-candidate.sh`; the report is
[`RELEASE-CANDIDATE.md`](../releasing/RELEASE-CANDIDATE.md).

| What | Where | The state of the run of 2026-10-22 |
|---|---|---|
| The archive of the source of the revision | `target/release-candidate/source/` | **PASS** — two builds, SHA-256 `d8d745cd…`, bit for bit the same; 5 118 360 bytes; 403 files, compared file by file with `git ls-tree` and `git hash-object` |
| The build record | `target/release-candidate/build-record/` | **PASS** — the revision, the tools, the gates, the counts of the tests, the source archive |
| The licences and the notices | `target/release-candidate/licences/` | **PASS** — every component carries a known, compatible licence; the notices are the notices of the repository (same checksum) |
| The Rust library, built from the **unpacked archive** | `target/release-candidate/build/` | **PASS** — SHA-256 `8c072bc2…` |
| The manifest | `MANIFEST.md`, `manifest.json` | **PASS** — 18 checks with their result, their reason and their evidence; the mark `internal-release-candidate`, `published: false` |

### 3.2 The separation the manifest draws

1. **The source archive is reproducible.** It is built twice and compared
   byte by byte: the content is the tree of the revision, and the
   compression carries neither the time nor the name of the build
   (`gzip -n`).
2. **The applications are not proven to be bit-identical.** They depend on
   the versions of the tools, on the stamps in the bundle and on the
   signature. The manifest says `bit_identical: "not-claimed"` and gives
   the reason. Nothing more is claimed.
3. **What cannot be built here is named.** `NOT RUN` (no macOS SDK, no JDK
   in the environment of the run) and `BLOCKED` (the Apple identity, the
   Android key, the legal clearance) are separate states, and a `NOT RUN`
   check is never reported as a passed one.

### 3.3 The state of the goal

**PASS.** The candidate of the revision `541abcb8f3c25a157911856f8c3ecce2fc15e07a`
is internally checkable and **unpublished**: no upload, no tag, no store,
no announcement.

---

## 4. The additional order — the optional interfaces (MCP)

The order was to add an optional **MCP** (or a similar interface) to the
roadmap **later** — not to implement it in this round. That is what
happened:

* the roadmap carries the milestone **M7 — Optional local interfaces (MCP)**
  after M6 (the Linux shell),
* the decision is written down in
  [`CR-0004`](../cr/CR-0004-optional-local-interfaces-mcp.md) (it was filed as
  `ADR-0021` and was reclassified while it was still `proposed` ✗): MCP over
  stdio as the first shell, optional and off by default, local and
  offline, no network right, no account, no telemetry — and **every
  writing operation goes through the same core with the same verification**
  as the user interface. No network service in the milestone (that would be
  a new architecture and licence review, E4). Android gets its own
  construction. E5 stays open.

**The state: PASS** — the order is carried out; the interface is *not*
implemented, and nothing in the product pretends that it is.

---

## 5. The table of the states

| What | The state | The proof |
|---|---|---|
| The findings F-1 … F-5 of the independent review | **PASS** — closed | the regression tests of §1.3, two of which fail without the fix |
| The test suite of the workspace | **PASS** — 262 tests, 34 suites, 0 failures | `cargo test --workspace` |
| The gate of the project in the local environment | **PASS** | `scripts/local-ci.sh` |
| The continuous integration of Linux | **PASS** | the run of §2.3 |
| The build of the macOS application against the real SDK | **PASS** | the run of §2.3 |
| The tests of the kit of the macOS application | **PASS** | the run of §2.3 |
| The handshake on macOS (Swift → UniFFI → Rust → MuPDF) | see §2.3 | the run of §2.3 |
| The build of the Android packages and the native libraries | see §2.3 | the run of §2.3 |
| The release candidate of one fixed revision | **PASS** — internal, unpublished | `docs/releasing/RELEASE-CANDIDATE.md` |
| The reproducibility of the source archive | **PASS** — two builds, byte for bit the same | the manifest of the candidate |
| The reproducibility of the applications | **NOT RUN / not claimed** | the manifest says `not-claimed` and gives the reason |
| The behaviour on a device of a user | **NOT RUN** | §2.4 |
| The legal clearance (E5) | **BLOCKED** — the work of the project owner | B-1 |
| The sign of the macOS bundle, the sign of the Android package | **BLOCKED** — the work of the project owner | B-2, B-3 |
| The contact of the responsible person in `PRIVACY.md` | **BLOCKED** — the work of the project owner | B-4 |

---

## 6. The open risks of the round

| What | Why it is still open |
|---|---|
| The **interoperability with Acrobat, Foxit and Okular** | the gate opens the document with the viewers that exist on the machine; the others are named in the report as *not installed* — they are not simulated |
| The **content of archives** (ZIP, RAR, 7z) | the walk names such a place as a place that was not examined, and the redaction refuses it — the content itself is not examined |
| The **limit of the OCR**, the **fonts of type 3**, words whose parts stand far apart | the known limits of the engine; they are named in the report of the round |
| The **order inside the engine** (the cleaning after the removal, the garbage collection of the writer) | unchanged; the proof of the product does not rest on it any more, the order is nevertheless still as it was |
| The **decision about the Mac App Store** | deferred by the owner (B-7); the question of the terms of Apple next to the AGPL is legal, not technical |
| The **reproducibility of the applications** | not claimed; the tools of the build, the stamps in the bundle and the signature stand in the way |
| The **transitive components of Android** are not in the inventory of the licences | the inventory names the direct dependencies; about 60 transitive components (among them `io.netty`, `com.google.guava`, `org.apache.httpcomponents`) are not named, and many of their POMs do not name their licence. The reader of the POMs is corrected, the check works, and the **question of the licensing** is open — `RISK-059` |
| A **job of the pool** can stay in the queue | observed on the runner of the continuous integration and on a Mac under load; the mechanism is **not** established. Two measures are taken (a worker waits for a free slot instead of leaving the pool; the failure message names the state of the pool) — `RISK-060` |
| The **licence of Xcode** on a Mac | the build of the application of macOS needs the command line tools of Xcode ✗; the acceptance of the licence of Xcode is a decision of the owner ✗ (`docs/audit/ci-gate-findings.md` §4.2 ✗). The workflow of macOS picks a full Xcode ✗ and the runner has one ✗ |

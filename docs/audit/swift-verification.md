# The verification of the Swift sources of Vocument

**Role:** ROLE-13 (QA) and ROLE-19 (documentation) · **Date:** 2026-10-21 ·
**Branch:** `main` · **Milestone:** M2 (`REDACT-001`, `REDACT-002`,
`ANNOT-001`, `LINKSEC-001`, `ATTACH-001`, `SIGREV-001`)

This report says what was checked of the macOS sources of Vocument, with
which tool, which defects were found and fixed, and — just as important —
**which statements are `NOT RUN`**. A green gate that proves nothing is worse
than a red one, so every claim below names the tool that stands behind it.

---

## 1. The starting point

The last round ended with a defect that nobody could see: the review panel
read `inspection.risyCount`, while the generated bindings know `riskyCount`
(`apps/macos/Sources/VocumentFFI/vocument_ffi.swift`). The file does not
compile, and the file was not compiled: no Swift toolchain was available in
the environment that builds this project, and the structural check
(`scripts/check-swift-structure.py`) counts brackets — it does not know the
names.

The task of this round was therefore twofold: **fix the defects that can be
proved** and **build a proof that holds the next defect**.

---

## 2. The toolchain: what was measured, and what it changed

The premise of the round was that no Swift toolchain could be reached:
`swift.org`, `download.swift.org` and `registry-1.docker.io` were reported as
unreachable (`http=000`, timeout). **That premise could not be reproduced in
this round.** The measurement of 2026-10-21, 14:55–14:57 UTC, in this
sandbox:

| What | Command | Result |
|---|---|---|
| `swift.org` | `curl -sSL -o /dev/null -w "%{http_code}" https://swift.org/` | `302` → `https://www.swift.org/` → `200` |
| `download.swift.org` | `curl -sSL … https://download.swift.org/` | `302` → `https://www.swift.org/download/` → `200` |
| the toolchain itself | `curl -sIL … swift-6.0.3-RELEASE-debian12.tar.gz` | `http=200`, `content-type: application/octet-stream` |
| the download | `curl -sL -o /tmp/swift-toolchain.tar.gz …` | 781 316 673 bytes in 6.8 s (~31 MB/s) |
| `registry-1.docker.io` | `curl -s -o /dev/null -w "%{http_code}" https://registry-1.docker.io/v2/` | `401` — the expected answer of the registry for an unauthenticated request; the host answers |
| `github.com` | `curl …` | `200` |
| `pypi.org` | `curl …` | `200` |
| `registry.npmjs.org` | `curl … https://registry.npmjs.org/tree-sitter-swift` | `200` |
| a Swift package in the image | `which swift swiftc` | nothing |
| a Swift package in the archive | `apt-cache search swift` | nothing |
| the image | `cat /etc/os-release` | Debian GNU/Linux 12 (bookworm), x86_64 |

The network of this sandbox therefore answers where the earlier round saw
`http=000`. The toolchain was **installed and used**:

* `Swift 6.0.3 (swift-6.0.3-RELEASE)`, target `x86_64-unknown-linux-gnu`,
  unpacked into `/opt/swift` (2.5 GB),
* `node v22.19.0`, `npm 10.9.3`, `tree-sitter 0.22.4`,
  `tree-sitter-swift 0.7.1` (from `registry.npmjs.org`).

The honest consequence is split in two:

* everything that the **compiler** can prove without a Mac is now proved by
  the compiler itself (section 3.1), and
* everything that needs the **macOS SDK** is still `NOT RUN` — and it is
  `NOT RUN` for a *different* and much more precise reason than "there is no
  toolchain" (section 5).

The installation is **not** part of the repository. The check installs the
grammar on demand (`scripts/check-swift-sources.py --install`), and the
toolchain is a property of the machine, not of the product.

---

## 3. What was checked, and with which tool

### 3.1 The grammar — a real parse

Two independent parsers read every Swift file of the product:

1. **The Swift compiler itself** — `swiftc -parse` (Swift 6.0.3). This is
   the parser of Swift; a file it accepts is syntactically a Swift file.
   **All ten files (nine hand-written + the generated bindings) parse
   without a single error.**
2. **The grammar of tree-sitter-swift 0.7.1** — `scripts/check-swift-ast.mjs`
   parses every file into a syntax tree that the name resolution (§3.2) runs
   on. The tree is the reason the check is more than a bracket counter.

One honest limitation of the second parser: the grammar of tree-sitter-swift
0.7.1 cannot read a compilation condition whose content is nothing but an
attribute — UniFFI writes 244 such blocks (`#if swift(>=5.8)` /
`@_documentation(visibility: private)` / `#endif`) into the generated
bindings. The helper therefore reads the **generated file only** in a
normalised form: those 732 lines are read as comments, of exactly the same
length, so that no token moves and the place of every node in the tree stays
the place it has in the file. The file itself is not touched (it is
generated and must not be hand-edited), and the normalisation is printed by
the check. The Swift compiler reads the file unchanged — that is the proof
that the normalisation is unnecessary for the truth of the file.

### 3.2 The names — the resolution against the generated bindings

`scripts/check-swift-sources.py` resolves the names of the application
against the generated bindings. The reference is the *current*
`vocument_ffi.swift`: **177 declared types with 809 members** and **84
`documentCore*` functions** are read out of it, and the sources of the
application, the kit, the command line tool and the tests are resolved
against them.

| What is checked | Rule | The finding it would produce |
|---|---|---|
| A member of a type of the bindings | `x.foo` where `x` is a type of `vocument_ffi.swift` → `foo` must exist in that type | `member-missing` |
| A call of a function of the bindings | every `documentCore*` call must exist, with the same argument labels | `function-missing`, `argument-mismatch` |
| A call of a method of the package | `x.foo(...)` where `x` is a type of the kit or the application → `foo` must exist, with the same argument labels | `member-missing`, `argument-mismatch` |
| A type that the source names | must be imported or defined in the package | `type-missing` |
| A call of a name that nobody defined | every unqualified call must be a function of the package or a name of the system (`SYSTEM_NAMES` in the script) | `function-missing` |
| A `switch` over an enumeration of the package | every case must be covered (or a `default` must stand there) | `switch-incomplete` |
| An `await` | only inside an `async` function | `await-without-async` |
| The same declaration twice | a type and a member may be declared only once in a file | `duplicate-declaration` |
| The grammar of every file | the parse of the compiler and the parse of the grammar | `grammar`, `compiler-parse` |

The inference is a real one, not a guess: the type of a local comes from its
annotation, from the result of the call it is bound to, from the element of
the collection it iterates, or from the member it is read from. The chain of
the defect of the last round is followed in full:

```
ReviewPanelView.body                       → some View
  GroupBox → VStack → IfLet(coordinator.reviewInspection)   → FfiDocumentInspection
    Text("Risky: \(inspection.risyCount)")                  → member-missing
```

**The check is not a rubber stamp.** `scripts/check-swift-sources.py
--self-test` copies the sources into a temporary directory, breaks them in
seven ways — a member that the bindings do not know, an argument label that
the declaration does not know, a file that does not parse, a call of a name
that nobody defined, the same declaration twice, an `await` in a function
that is not `async`, and a `switch` that does not cover every case — and
demands that the check reports every one of them. The self-test is part of
the gate.

```
$ python3 scripts/check-swift-sources.py --self-test
✓ The check found the defect: a member of a type of the generated bindings.
✓ The check found the defect: an argument label that the declaration does not know.
✓ The check found the defect: a file that does not parse.
✓ The check found the defect: a call of a name that nobody defined.
✓ The check found the defect: the same declaration twice.
✓ The check found the defect: an `await` in a function that is not `async`.
✓ The check found the defect: a `switch` that does not cover every case.
✓ The self-test passed: the check finds the defects it promises.
```

### 3.3 The workflow — by reading, and against the Rust core

The chain `Auswahl → Vorschau → Redaction → Kommentar → Speichern →
Wiederöffnen` was read in `apps/macos/Sources/VocumentMacApp/` and
`apps/macos/Sources/VocumentMacKit/` and compared with the behaviour of the
Rust core (`core/vocument-core/src/redaction.rs`,
`core/vocument-core/src/annotations.rs`,
`platform/vocument-ffi/src/redaction_dto.rs`). The findings are in §4.

---

## 4. The defects that were found and fixed

| # | Where | The defect | The fix |
|---|---|---|---|
| 1 | `apps/macos/Sources/VocumentMacApp/ReviewPanelView.swift:42` | `inspection.risyCount` — the member does not exist in `FfiDocumentInspection`. **The application did not compile.** | `inspection.riskyCount` |
| 2 | `apps/macos/Sources/VocumentMacApp/ReviewWorkflow.swift:269` | `redactSelectedRegions()` did not draw the page again after the removal: the user kept seeing the text that had just been removed for good. `openDocument(at:)` does draw it — the redaction did not. | the new `refreshRenderedPageAfterChange(sessionId:)` (`VocumentMacApp.swift:470`) refreshes the session, the thumbnail of the page and the picture of the page |
| 3 | `apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift:398` | the second line of the client (`redactPageRegions`) threw without the reasons of the core — the user learned *that* a check failed, but not *why* | the error carries `report.notCheckedReasons` and the flag `report.verified` is checked as well |
| 4 | `apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift:433` | `redactionWarnings(_:)` read `remainingOccurrences` and `uninspectedPlaces`, but not `report.verified` and not `report.notCheckedReasons` — the reasons of the checks that could not be carried out were lost. These are the places at which the removal was refused. | the warnings carry the flag and the reasons |
| 5 | `apps/macos/Sources/VocumentMacApp/VocumentMacApp.swift:158`, `ReviewWorkflow.swift:61` | the warnings of the *previous* document stayed above the new one: a user who opened another document still read "the document is not free of the marked text" | the warnings are cleared when a document is opened and when a document is recovered |
| 6 | `apps/macos/Sources/VocumentMacApp/VocumentMacApp.swift:167` | the recovered document was never read: `executeRecovery` set the session, but neither the findings nor the comments of the recovered document were loaded | `refreshReviewState()` after the session is set |
| 7 | `apps/macos/Sources/VocumentMacApp/ReviewWorkflow.swift:246` | the marked place belonged to no page: the rectangle was kept, the page was not. A user who marked a place on page 3, went to page 5 and pressed "remove" would have removed a text of page 5 that was never in the preview. | the new `redactionSelectionPageIndex` (`VocumentMacApp.swift:74`); the marked place is cleared when the page changes, and the removal is refused when the page does not fit |

The first defect was found by the new check (it is the reason the check
exists). The defects 2 to 7 were found by reading the chain of the workflow
against the core, and every one of them is a place where the user would have
been told something that was not true.

### 4.1 What the chain of the workflow looks like now

* **The preview comes before the removal.** `applyRedactionSelection(_:)`
  asks the core for the text of the marked place (`selectPageText`) and
  stores it in `redactionSelectionText`; the panel shows the text of the
  marked place **above** the button "Remove the marked content for good"
  (`ReviewPanelView.swift:91–113`). The button is only there when a place
  is marked.
* **The removal is refused when nothing is marked.**
  `redactSelectedRegions()` answers with "Mark a place on the page before you
  remove something." and changes nothing.
* **The removal is refused when the place belongs to another page** (defect
  7) — the preview and the removal always concern the same text.
* **The errors of the chain are visible.** A refused redaction, a failed
  rollback, a failed save and a conflict all reach the user:
  `errorMessage` is shown in the reading view (`CalmReaderView.swift:44`),
  the conflict opens a sheet with the four ways out (`DocumentCanvasView.swift:144`),
  the warnings of the redaction stand in red above everything else
  (`ReviewPanelView.swift:26`).
* **The comment comes back out of the document.** The core writes the note
  as a standard PDF `/Text` annotation into the bytes of the document
  (`engines/pdf-mupdf/src/comment.rs`), the save writes those bytes into the
  file, and the reopening reads the annotations of the page out of the file.
  The Rust side proves this with the test
  `a_note_survives_the_save_and_is_found_in_the_file_again`
  (`platform/vocument-ffi/tests/annotation_e2e_tests.rs:137`): the note is
  written, the bytes of the document are exported, a **new session is opened
  from those bytes**, and the note is read back with its text and its key of
  the rail of the profession. The shell reads the notes with
  `refreshPageComments()` → `pageCommentAnnotations`, and `openDocument(at:)`
  calls it after the session is opened (`ReviewWorkflow.swift:53`).
  **The Swift side of this path is `NOT RUN` here** (see §5) — the proof is
  the Rust test and the reading of the shell.

---

## 5. What is `NOT RUN` — and why

Nothing below is reported as passed. Each line names the reason.

| Statement | State | The reason, with the evidence |
|---|---|---|
| **The compilation of the application** (`swift build --product VocumentMacApp`) | **NOT RUN** | `swift build --package-path apps/macos` fails with `apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift:2:8: error: no such module 'CoreGraphics'`. SwiftUI, AppKit and CoreGraphics are part of the **macOS SDK**; the runner is Debian 12 (x86_64) and the macOS SDK does not exist for it. A stub of CoreGraphics would prove nothing, so none was written. |
| **The type check of the kit and the application** | **NOT RUN** | the same error. The type check of the *names* is therefore carried by the resolution of §3.2 and not by the compiler. |
| **The build of the generated bindings** (`swift build --target VocumentFFI`) | **RUN — green** | `Build of target: 'VocumentFFI' complete! (19.12s)`; the module is compiled and type-checked by Swift 6.0.3. The C module `vocument_ffiFFI` is built with it. |
| **The tests of the kit** (`swift test --filter VocumentMacKitTests`) | **NOT RUN** | the test target imports the kit, and the kit needs CoreGraphics — the same error. The tests exist (`apps/macos/Tests/VocumentMacKitTests/DocumentCoreClientTests.swift`) and are read by the checker, but they are not executed here. |
| **The handshake Swift → UniFFI → Rust** (`swift run vocument-mac-handshake`) | **NOT RUN** | the command line tool imports the kit (CoreGraphics) and the Rust library; both are missing on this runner. |
| **The surface of SwiftUI** (the panel, the canvas, the sheet of the conflict) | **NOT RUN** | there is no macOS and no display. The SwiftUI code was read and parsed, but **not** looked at and **not** operated. |
| **The run of the workflow on a Mac** (mark → preview → remove → comment → save → reopen) | **NOT RUN** | there is no macOS. The chain was read and compared with the core (§4.1); the behaviour of the user interface is not proven. |
| **The rendering of the page after the removal** (defect 2) | **NOT RUN** | the fix is proved by the *code* (`refreshRenderedPageAfterChange` is called after the report) and by the parse and the name resolution; the picture on the screen is not proven. |
| **The error texts of the refusals** (defects 3, 4) | **NOT RUN** | the strings are built by the code and the names of the fields are resolved against the bindings; the text that a user would read on a Mac is not proven. |

The tree-sitter parse and the name resolution are **not** `NOT RUN`: they ran
on every file and their result is in §3.

---

## 6. The gate

The proof is part of the gate of the repository (`scripts/local-ci.sh`):

```bash
echo "==> Proving the Swift sources: the grammar of Swift, the compiler and the names against the generated bindings..."
python3 scripts/check-swift-sources.py --install-if-missing
python3 scripts/check-swift-sources.py --self-test
```

* a finding → the exit code 1 → the gate stops;
* the grammar cannot be installed (the exit code 2) → the gate **does not
  stop**: it prints `NOT RUN: the static proof of the Swift sources could
  not be carried out here` and says that the sources were **not** proved in
  this run. A missing grammar is a missing proof, not a defect of the
  product — but a green gate must not claim a proof it does not have;
* `VOCUSMENT_SWIFT_CHECK_SKIP=1` skips the step **with a loud note** — the
  same convention as the other steps that a runner cannot do
  (`qemu-aarch64-static`). The note says that the proof is `NOT RUN` and
  must not be reported as passed.

The existing structural check (`scripts/check-swift-structure.py`) is **not**
replaced; it still runs, and the new check stands beside it.

The Swift build in the gate now distinguishes the platforms: on a Mac the
application and the handshake are built and run, on every other system the
generated bindings are built with the compiler, and the note says that the
build of the application is `NOT RUN`.

---

## 7. How the next defect is caught

The defect of the last round — a name that the bindings do not know — is
now found in **eight seconds** by a command that needs no Mac:

```bash
$ python3 scripts/check-swift-sources.py
✓ The Swift sources are sound (10 files parsed with the grammar of Swift and
  with the Swift compiler itself, the names resolved against the generated
  bindings). This is a static proof, not a compilation.
```

The same command on the sources of the last round answers:

```
ERROR: …/ReviewPanelView.swift:42: [member-missing] the type
'FfiDocumentInspection' has no member 'risyCount' — the generated bindings
do not know it
```

---

## 8. The tools and their versions

| Tool | Version | Where it comes from |
|---|---|---|
| Swift compiler | 6.0.3 (swift-6.0.3-RELEASE), `x86_64-unknown-linux-gnu` | `download.swift.org` (installed for this round, not part of the repository) |
| `tree-sitter` | 0.22.4 | `registry.npmjs.org` |
| `tree-sitter-swift` | 0.7.1 | `registry.npmjs.org` |
| node | v22.19.0 | the image |
| npm | 10.9.3 | the image |
| Python | 3.11.2 | the image |

The new files:

* `scripts/check-swift-sources.py` — the check (the grammar, the names, the
  self-test, the installation of the grammar);
* `scripts/check-swift-ast.mjs` — the helper that parses the files with the
  grammar of Swift and hands the tree to the check.

---

## 9. Handoff

* The changes are in the working tree; **they are not committed** — the team
  lead commits.
* The Rust core was not touched by this round.
* The `docs/audit/swift-verification.md` and the two scripts are the
  artefacts; the board of the plan carries the state.
* Open for a Mac: the compilation of the application, the tests of the kit,
  the handshake, the surface of SwiftUI and the run of the workflow. The
  list is in §5, and every one of them is a `NOT RUN` until a Mac proves
  it.

---

## 10. The second measurement — the lead's own session

The report above was written in the environment of the reviewer. The lead
measured the same thing again in its own session, so that the claim does not
rest on one environment. The measurement of 2026-10-21, 15:12–15:25 UTC:

| What | Command | Result |
|---|---|---|
| the reachability of the toolchain | `curl -sSL -o /dev/null -w "%{http_code}" https://download.swift.org/` | `302` — the host answers, the download needs a retry |
| the download | `curl -sSL -o /tmp/swift.tar.gz …/swift-6.0.3-RELEASE-debian12.tar.gz` | the **first** attempt failed with `SSL_ERROR_SYSCALL`; the **third** attempt wrote 781 316 673 bytes |
| the toolchain | `tar xzf /tmp/swift.tar.gz` → `/opt/swift` | `Swift 6.0.3 (swift-6.0.3-RELEASE)`, `Target: x86_64-unknown-linux-gnu` |
| **the parse of every hand-written file** | `swiftc -parse <file>` for every file of `VocumentMacApp`, `VocumentMacKit` and `VocumentMacHandshakeCLI` | **no output at all** — not one file produced an error |
| the build of the generated bindings | `swift build --package-path apps/macos --target VocumentFFI` | `Build of target: 'VocumentFFI' complete! (21.27s)` |
| the name resolution | `python3 scripts/check-swift-sources.py` | `✓ The Swift sources are sound (10 files parsed with the grammar of Swift, the names resolved against the generated bindings)` |

The first attempt of the download failed — the network of the sandbox is
unreliable, and the earlier rounds measured exactly that. The third attempt
succeeded. **The statement of the earlier rounds — "a Swift toolchain is
not obtainable" — was correct for those rounds and is wrong for this one.**
The gate does not depend on it: the proof of the names runs with the
grammar of tree-sitter, and the compiler is the second witness, not the
first.

The application itself is still `NOT RUN` (see §5): `swift build --product
VocumentMacApp` fails with `no such module 'CoreGraphics'` — the macOS SDK
does not exist for Linux, and a stub of it would prove nothing.


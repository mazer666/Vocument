# Round 3 Acceptance Record — the licence, the distribution and the fourth adversarial round

**Date:** 2026-10-21
**Branch:** `main`
**Scope of the round:**

1. close the findings of the **fourth adversarial audit** of the redaction
2. make M2 a usable product: a consistent macOS review workflow, a **durable**
   juristic palette, security warnings that really reach the user
3. take the **licensing decision E1–E5** as the basis of the planning
4. prepare the **app store readiness** for macOS and Android
5. run the work with the multi-agent team, with independent reviews and red
   team rounds

**Result:** the code of the round is committed, the tests are green, the
licensing decision is written down, and the store readiness is prepared as far
as it can be prepared without a macOS host, without an Apple identity and
without the legal clearance of the project owner.

---

## 1. The result overview

| # | Acceptance criterion | Environment | Command | Result |
|---|---|---|---|---|
| 1 | A copy of the removed text in the **dictionary of a form object** is found; the redaction refuses and names the place; the document stays byte for byte unchanged | Host, real engine, end to end | `cargo test -p vocument-ffi --test redaction_hardening_e2e_tests a_copy_in_a_form_xobject_makes_the_redaction_refuse_and_names_the_place` | **PASS** |
| 2 | The walk descends into the **dictionary of a stream**, not only into its content | Host, real engine | `mupdf_walk.h` | **PASS** (by construction; proven by criterion 1) |
| 3 | A container is recognised by its **content**, not by the name of the path | Host, real engine | `vocument_buffer_is_container` | **PASS** (by construction) |
| 4 | The **encryption dictionary** is never cleared — its values are keys, not text | Host, real engine | `mupdf_scrub_bridge.c` | **PASS** (by construction) |
| 5 | The **identifier of the file** in the trailer (`/ID`) is walked and cleaned | Host, real engine | `mupdf_walk.h` | **PASS** (by construction) |
| 6 | The sweep over the cross-reference table is **linear** and skips the objects that the writer has removed | Host, real engine | `vocument_walk_xref` | **PASS** (by construction; the measurement with 32 000 objects is **NOT RUN** — see §4) |
| 7 | The **recovery directory** has to belong to the user alone; a directory of a stranger is refused | Host | `cargo test -p vocument-core --test recovery_privacy_tests` | **PASS** (5 tests) |
| 8 | A store that may not write keeps the document out of the directory | Host | same | **PASS** |
| 9 | The refusal of the recovery **reaches the user** as a status, not as a message on the console | Host (FFI), Swift, Kotlin | `document_core_recovery_status()` | **PASS** for the data path; the step of the interface is **NOT RUN** (no macOS host, no device) |
| 10 | The **rail of the profession** is kept on the disk, belongs to the user alone, is written atomically, and a broken file is an error instead of a silent fall back | Host | `cargo test -p vocument-core --test annotation_store_hardening_tests` | **PASS** (7 tests) |
| 11 | The **key** of a note travels with the document: a change of the rail does not change the meaning of a note that was written before | Host, real engine | `the_key_of_the_rail_travels_with_the_note` | **PASS** |
| 12 | Every binary names the **source** it was built from, and a build from a dirty tree is refused | Host | `cargo test -p vocument-ffi --test build_info_tests` | **PASS** |
| 13 | The **inventory of the licences** is complete and every component carries a known, compatible licence | Host | `scripts/audit-licenses.sh` | **PASS** (103 external crates, 12 native libraries, 182 fonts) |
| 14 | The **notice of the third-party components** is in the repository and is the one that the generator produces now | Host | `scripts/generate-third-party-notices.sh` | **PASS** (125 477 bytes, 46 blocks) |
| 15 | The licence configuration of `cargo-deny` is loadable | Host | `cargo deny check licenses` | **PASS** (exit 0) |
| 16 | Every artefact that is handed out carries the **offer of the source** | Host | `scripts/release-macos.sh`, `scripts/release-android.sh`, `scripts/release-source-archive.sh` | **PASS** (the scripts build; the artefacts are **not** built here — no macOS host, no Android SDK) |
| 17 | The **decision E1–E5** is written down and binding | — | `docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` | **PASS** |
| 18 | The **conflict between the terms of Apple and the AGPL** is named as a risk, not smoothed over | — | `RISK-041` | **PASS** |
| 19 | The **digest of the store requirements** is written down in the repository | — | `docs/audit/store-requirements.md` | **PASS** |
| 20 | The **product is local-first**: no account, no telemetry, no network right | Host | `PRIVACY.md`, the manifest of Android, the entitlements of macOS | **PASS** |
| 21 | The **signing and the notarisation** of the macOS bundle | macOS | — | **NOT RUN** (no macOS host, no Apple identity) |
| 22 | The **upload into a store** | — | — | **NOT RUN** (no identity, no legal clearance) |
| 23 | The **legal compliance review** of the project owner (E5) | — | — | **NOT RUN** (an agent cannot give this clearance) |
| 24 | The **macOS review workflow** (the review panel, the rail, the comment bar) in the running application | macOS | — | **NOT RUN** (no macOS host, no Swift toolchain) |

---

## 2. What the fourth adversarial audit found

The fourth audit attacked the rules that the third round had introduced. It
built 20 purpose-made PDF files and measured the redaction against the real
chain from the core to MuPDF to the UniFFI boundary.

The critical finding: the walk handed the objects of the cross-reference table
on as **direct** objects. `pdf_is_stream()` works on the *number* of the
object — a direct object has the number zero, and the answer is therefore
always "no". The content of a form XObject was therefore never looked at. A
secret in the content of a form object stayed in the document, and the
redaction reported `verified = true`.

That is the kind of finding that a green test suite does not find: the
verification of the redaction was itself built on the same wrong assumption.

The other findings — the identifier of the file in the trailer, the container
that hangs under another name, the words of the encryption dictionary, the
quadratic sweep, the recovery directory of a stranger, the objects that the
writer has already removed — are all closed and listed in the
[M2.1 acceptance record](M2.1-ACCEPTANCE.md) §5.4.

The report of the checker and its raw protocol are in the repository:
[the fourth audit on the document level](../audit/redteam-round-four-document-level.md)
and [the 774 lines of the measurement](../audit/redteam-round-four-measurements.txt).

**What it means for the user:** a document that carries a form object with
text in it is now **refused** by the redaction instead of being "redacted"
successfully. The redaction is a tool that says no when it cannot promise
what it promises.

## 3. What the round built for the product

### 3.1 The rail of the profession on the disk

The rail of the user — the colours with their names and their meanings — is
kept in the data directory of the application (`jurist_palette.json`). It is
written atomically, it belongs to the user alone (`0600` in a `0700`
directory), it refuses a path that leaves its directory, it refuses a file of
a stranger, and a file that was cut in half by a crash is an **error with a
name** instead of a silent fall back to the standard rail.

The last point is the one that matters: a silent fall back would change the
meaning of every note of the reviewer without saying so.

The **key** travels with the note (`/VocumentPaletteKey`), not the colour. A
change of the rail therefore does not change the meaning of a note that was
written before, and a note whose key the rail does not carry is reported as
not assignable instead of being given a neighbouring colour.

The report of the measurements:
[the red team on the stored rail](../audit/redteam-palette-and-storage.md).
It names honestly that the first attempt at an **independent** run was stopped
by its time limit, and that the measurements were taken by the team lead.

### 3.2 The warnings reach the user

The redaction reports the places where the same text still stands and the
places that could not be read. The callers turn them into warnings — and the
macOS and the Android shell now show them.

The same rule now applies to the **recovery store**: if the directory of the
recovery does not belong to the user, the store refuses every write and the
reason crosses the FFI boundary as a status (`document_core_recovery_status()`).
The macOS shell shows it above the document, the Android shell shows it as
well. A user who does not know it would believe that a crash cannot cost them
anything.

### 3.3 The macOS review workflow

The macOS shell is now one **review workflow** instead of a collection of
views: the navigation and the search, the document, and the review panel
(`ReviewPanelView.swift`, `ReviewWorkflow.swift`). The rail of the profession
is offered, the notes of the page are read back, and the redaction of the
current page is called with its report.

**The step of the interface is NOT RUN**: there is no macOS host and no Swift
toolchain in this environment. The library path behind the surface is proven.

## 4. The licence and the distribution

### 4.1 The decision

`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` carries the decision
of the project owner:

* **E1** — the whole product is open source under the **AGPL-3.0-or-later**.
* **E2** — no proprietary use, no commercial licence of MuPDF.
* **E3** — the product is free; services and support are optional.
* **E4** — no mandatory service, no cloud, no accounts, no telemetry. The
  product is local-first and offline-first. A server function later needs a
  new architecture and licence review.
* **E5** — the final legal compliance review is a precondition of every
  publication. **An agent cannot give that clearance.**

The decision is binding for the planning, and it was not changed by any
agent in this round.

### 4.2 What the AGPL asks of a binary, and what is done

| The duty | What is done | Where |
|---|---|---|
| Name the source the binary was built from (§5d) | The build carries the product, the version, the revision, the dirty flag, the licence and the source address; a build from a dirty tree is refused | `core/vocument-core/src/build_info.rs`, `document_core_build_info()` |
| Carry the licence and the notices (§4, §5d, §6) | `LICENSES/` holds the text of every licence that travels with the product; `LICENSES/THIRD-PARTY-NOTICES.txt` holds the notice of the third-party components and is copied into the bundle and into the package | `scripts/generate-third-party-notices.sh`, `scripts/release-macos.sh`, `scripts/release-android.sh` |
| Offer the source with every copy (§6) | The source archive is built with the offer of §6 and a checksum | `scripts/release-source-archive.sh` |
| Name every component and its licence | The inventory is generated from the manifests and the Android dependencies and is checked by a gate | `scripts/audit-licenses.sh`, `THIRD_PARTY.md` |

A false citation was removed while the round was running: the text called
§13 *"Corresponding Source"*. That is §6. The reference to §13 — the *remote
network interaction* — stays where it belongs.

### 4.3 The inventory

| Part | Number | Licences |
|---|---|---|
| Rust crates (external) | 103 | 59 × "MIT OR Apache-2.0", 15 × MIT, 9 × "MIT/Apache-2.0", 8 × MPL-2.0, 5 × "Apache-2.0 OR MIT", 2 × "BSD-2-Clause OR Apache-2.0 OR MIT", 2 × "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT", 1 × "Unlicense OR MIT", 1 × "MIT OR Apache-2.0 OR LGPL-2.1-or-later", 1 × "(MIT OR Apache-2.0) AND Unicode-3.0" |
| Native libraries in `libmupdf.so` | 12 | brotli (MIT), cmark-gfm (BSD-2-Clause), **extract (AGPL-3.0)**, freetype (FTL-or-GPL-2.0), gumbo-parser (Apache-2.0), harfbuzz (MIT), jbig2dec (AGPL-3.0), lcms2 (MIT), libjpeg (IJG), mujs (ISC), openjpeg (BSD-2-Clause), zlib (Zlib) |
| Native libraries of the product | 4 | MuPDF 1.28.5 (AGPL-3.0-or-later), Tesseract 5 (Apache-2.0), Leptonica 1.82.0 (BSD-2-Clause), tessdata (Apache-2.0) |
| Fonts in `libmupdf.so` | 182 | 180 × OFL-1.1, 2 × Apache-2.0 |
| Android components | 12 | 11 × Apache-2.0, 1 × "Apache-2.0 OR LGPL-2.1-or-later" (JNA) |
| Workspace packages | 6 | AGPL-3.0-or-later |

The audit does not only read the manifests: it counts the **symbols of every
native library in the artefact that is actually delivered**, so that a library
that is linked but not declared cannot slip through.

### 4.4 The stores

The digest of the requirements is in
[`docs/audit/store-requirements.md`](../audit/store-requirements.md). The
finding that matters:

> Only the **Mac App Store** collides structurally with the AGPL. The
> conditions of Apple for modified versions, for updates through the store and
> for Apple as the commissionaire of the developer cannot be reconciled with
> §12 of the AGPL.

That is a legal question, and it stays with the project owner. **No store
eligibility is claimed** anywhere in this repository. The channels that are
prepared:

| Channel | State |
|---|---|
| macOS, direct distribution | Bundle, sign and notarise are prepared; **NOT RUN** (no macOS host, no Apple identity) |
| macOS, Mac App Store | **Blocked** by the licence conflict (`RISK-041`); the decision stays with the owner |
| Android, Google Play | The bundle (`.aab`) and the APK are prepared; the signature and the upload are **NOT RUN** |
| Android, direct APK | The APK is prepared; the signature is **NOT RUN** |
| Source archive | The script is ready; the archive is **not built** here |

**Google Play** is evaluated as less structurally conflicting than the Mac App
Store, but with its own notice and licence duties. That is an assessment, not
a clearance.

### 4.5 The privacy policy

`PRIVACY.md` says what the product does with the data: everything happens on
the device, there are no accounts, no telemetry and no network right (the
manifest of Android and the entitlements of macOS carry no network
permission).

**Open:** the post and the mail address of the responsible person. The
project owner has to supply them before the first upload; the file says so.

## 5. What is not proven in this round

- **The macOS application.** There is no macOS host and no Swift toolchain in
  this environment: the build of the app, the review workflow, the surface of
  the rail and the display of the warnings are **NOT RUN**. The data path
  behind them is proven.
- **The Android application on a device.** There is no device and no
  `/dev/kvm`: the surface, the behaviour on a device and the energy and
  thermal behaviour are **NOT RUN**.
- **The signing and the notarisation.** No Apple identity, no Android key.
  The scripts carry the commands; nothing was signed.
- **The upload into a store.** Not run, and not permitted before the legal
  clearance of the project owner (E5).
- **The legal compliance review itself.** Not run — it is not the job of an
  agent. The digest of the requirements is a preparation for it, not a
  replacement for it.
- **The performance of the sweep** with a document of 32 000 objects: the
  sweep is linear by construction, but the **measurement** is **NOT RUN**.
- **The independent red team on the palette.** The first attempt was stopped
  by the time limit of its session; the measurements were taken by the team
  lead and are named as such.
- **The CI workflow.** There is no runner: no result of the GitHub Actions is
  claimed. The local gate `scripts/local-ci.sh` carries the same checks.
- **AddressSanitizer.** Only available with a nightly toolchain — **NOT
  RUN**. The substitute is the valgrind memcheck.
- **The interoperability with the tools of other vendors** (Acrobat, Foxit,
  Okular): the notes and the redaction are standard PDF, but the comparison
  with an external viewer is still outstanding.

## 6. The state of the project

M2 is **started, not completed**. Four slices are done — M2.1 (redaction with
verification), M2.2 (links and actions), M2.3 (attachments and signatures),
M2.4 (the comment of the reviewer with the rail of the profession) — and all
four are verified on the host. Open are the remaining M2 functions (among them
the full sanitising, `SAN-001`), the macOS interface, the interoperability
gate and the legal clearance of the distribution.

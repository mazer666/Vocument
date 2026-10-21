# Round 4 Acceptance Record — the security closeout, the usable M2 and the release readiness

**Date:** 2026-10-21
**Branch:** `main`
**Scope of the round:**

1. an independent, adversarial review of the **whole redaction chain**, with
   the findings fixed and proved by regression tests
2. **M2 made usable**: the macOS sources checked for structure, integration
   and compile errors; the provisional "remove the first line" replaced by a
   safe review and redaction workflow with a targeted selection; the
   independent second review of the palette store; the long-term unambiguous
   interpretation of the comments
3. the **release readiness** for the AGPL-conformant direct distribution of
   macOS, Google Play and the direct APK — with the legal analysis of the
   Apple conditions checked against the actually applicable contract
   versions

**Result:** the code of the round is committed; the tests are green; the
licensing decision is unchanged; the Mac App Store is deferred by the
decision of the owner; the blockers of a public binary are written down with
their owner.

---

## 1. The result overview

| # | Acceptance criterion | Environment | Command | Result |
|---|---|---|---|---|
| 1 | The **macOS sources are structurally sound** — a Swift file that does not compile is a file that nobody notices without a Swift toolchain | Host | `python3 scripts/check-swift-structure.py` | **PASS** (7 files) |
| 2 | The check **catches** the defect that was shipped: the fragments of methods without their head, and the box of a view that was pasted twice | Host | the same, against the stand `bbc2d0b` | **PASS** (the defect is reported with the file and the line) |
| 3 | The **Swift build** of the application | macOS | — | **NOT RUN** (no macOS host, no Swift toolchain) — the structural check is **not** a compilation |
| 4 | The user **marks the place** of a redaction; the application does not choose it | macOS | — | **NOT RUN** for the step of the interface; the data path is **PASS** (`selectPageText` with a rectangle) |
| 5 | The text layer and the rendered picture live in the **same** coordinate space | Host, real engine | `the_text_layer_lives_in_the_device_space_of_the_page`, `the_rendered_tile_has_the_same_orientation_as_the_text_layer` | **PASS** (measured) |
| 6 | The note of the reviewer carries the **name and the meaning** of the rail besides its key | Host, real engine | `the_note_carries_the_name_and_the_meaning_of_the_rail` | **PASS** |
| 7 | The meaning of a note does **not** change when the rail of the user is replaced | Host | `the_meaning_of_a_note_survives_a_change_of_the_rail`, `the_meaning_of_the_rail_travels_with_the_note` | **PASS** |
| 8 | A **link that points nowhere** is refused — and not taken for "no rail at all" | Host | `a_link_that_points_nowhere_is_refused_and_not_taken_for_no_rail` | **PASS** |
| 9 | A rail of a **stranger** is not read — the read path checks the owner like the write path | Host | `a_rail_of_a_stranger_is_not_read` | **PASS** |
| 10 | A `..` in the directory the **platform** chose is not an attack; the name of the file decides whether the rail can leave its directory | Host | `a_directory_with_a_dot_dot_is_accepted_and_lands_in_the_named_directory`, `a_file_that_is_not_the_rail_cannot_leave_its_directory` | **PASS** |
| 11 | Two writers at the same time leave a **complete** rail — 5 900 144 samples, none torn | Host, real processes | the second red team round | **PASS** (the raw protocol is in the report) |
| 12 | The **Apple conditions** are checked against the actually applicable contract versions | Host, network | the verification report | **PASS** (10 findings) |
| 13 | The **blockers** of a public binary are written down with their owner and their state | — | `docs/releasing/RELEASE-BLOCKERS.md` | **PASS** |
| 14 | The **contact of the responsible person** is not invented | — | `PRIVACY.md` §7 | **PASS** (the gap is documented as the blocker B‑4) |
| 15 | The **signing and the notarisation** of the macOS bundle | macOS | — | **NOT RUN** (no macOS host, no Apple identity) |
| 16 | The **build of the Android release package** | Android | — | **NOT RUN** (no Android SDK in this environment) |
| 17 | The **legal clearance** of the project owner (E5) | — | — | **NOT RUN** (an agent cannot give it) |
| 18 | The **upload into a store** | — | — | **NOT RUN** (the Mac App Store is deferred; the other channels need E5) |
| 19 | The **fifth red team round** against the redaction | Host | the report | see §3 |

244 tests, 31 suites, 0 failures. fmt, clippy, the Swift structure check, the
licence audit, the gate of the distribution and the repository validation are
green.

---

## 2. What was broken and what was done

### 2.1 The macOS application did not compile

**The finding:** the round that split the macOS sources into three files had
left **fragments of methods without their head** in all of them — an opening
brace, a body, a closing brace of a function whose signature was gone. The
application would not have started. Nothing noticed: the Rust part of the
product was green, the gate was green, and there is no Swift toolchain in
this environment.

The repair:

* the review workflow is one file again (`ReviewWorkflow.swift`), the view of
  the shell is in its own file (`CalmReaderView.swift`), and the duplicate
  box of the review panel is gone;
* every file is under the 600 lines of `CODE-001`;
* `scripts/check-swift-structure.py` checks the structure of the Swift
  sources: the balance of the brackets, a **statement in the body of a type**
  (the trace of a lost method head), a declaration that stands twice and a
  box of a view that was pasted twice. It is **not** a compiler, and the
  acceptance record says so. The gate runs it.

### 2.2 The "remove the first line" is gone

The provisional action removed the first line of the page — something the
user never looked at. A redaction cannot be undone, so that is not an
acceptable action.

The workflow now is: the user **drags a rectangle** on the page, the shell
converts it into the millipoints of the page, the **core** decides which text
lies inside it, and the panel shows that text **before** the removal. The
removal is refused when nothing is marked. A comment of the reviewer can be
taken back, a redaction cannot.

**The coordinate space is measured, not assumed.** The page lives in the
**device space** of the page — the origin in the **top left** corner, the y
axis **downwards**. The rendered image has the same orientation, so the
conversion needs no flip. The proof is a test that draws a word at a known
place of the page and measures where the engine reports it. If the text layer
and the picture used different spaces, a region would hit the text and miss
the picture — and the proof over the picture would pass while the word was
still visible.

### 2.3 The comment of the reviewer is interpretable on its own

The note carried the **key** of the rail, the colour and the text. The key is
the stable identifier, but a key alone says nothing to a reader who does not
have the rail of the author — and the rail of the user may be replaced years
later. The note now carries the **name** and the **meaning** of the rail as
well (`/VocumentPaletteLabel`, `/VocumentPaletteMeaning`), and the meaning of
a note does not change with the rail of the moment.

### 2.4 The second, independent review of the palette store

The first report on the palette store was measured by the team lead. The
**second round was independent**, and it found three places where the first
report was wrong:

| # | What | What was done |
|---|---|---|
| 1 | A **link that points nowhere** was not refused: the old rule asked `Path::exists()`, which follows a link, so the application carried on with the standard rail — **silently** | The state of the path is read with `symlink_metadata` now, which does not follow a link. A link of any kind is refused, and only a path that carries nothing at all means "no rail" |
| 2 | The **read** path had no check of the owner: a rail that belonged to a stranger was read and used, so the notes of the reviewer were written with the **colours of the stranger** | The read path checks the owner like the write path |
| 3 | The refusal of a path with `..` was applied to the **whole** path, including the part the **platform** chose — a path like `/a/b/../c` bricked the application at the start | The **name** of the file decides now: it has to be exactly `jurist_palette.json`, and a name can never be `..`. The directory is protected by the check of the owner, and the staging file of the write is built from the same directory as the rail |

All three are fixed and each has a test. The first report is corrected in
place — a report that is wrong in the repository is worse than no report.

### 2.5 The legal analysis of the Apple conditions

The hypothesis of the review was that a free application runs under
**Schedule 1** of the Apple Developer Program License Agreement and that
Schedule 1 does not know the *commissionaire* — which would have removed the
strongest argument against the store channel.

**The hypothesis is refuted by the text.** Schedule 1 §1.1 (ii) and §1.3
appoint Apple as commissionaire, and §3.1 uses the same *"in the course of
acting as agent and/or commissionaire"* wording as Schedule 2 §3.1. The
conclusion of the report stands — for a free application.

Ten findings in total, of which three changed a statement of the repository:

* the living digest attributed the agency clause to **Schedule 2** — for a
  free application it is **Schedule 1** (§7.1);
* the **Apple Media Services Terms and Conditions** were missing from the
  analysis although they bind the store distribution, and their Standard EULA
  carries an **open-source carve-out** in the anti-modification clause — that
  belongs in the legal review as an **input**, not as a solution;
* the words *"blocked"*, *"structurally inadmissible"* and *"collides"* were a
  **legal determination presented as fact**, while the owner keeps the
  clearance open (E5) and has deferred the channel. The wording is now the
  open question it really is: the conditions of the store carry obligations
  that stand next to the duties of the AGPL; whether they can be satisfied at
  the same time is open.

### 2.6 The blockers of a public binary

`docs/releasing/RELEASE-BLOCKERS.md` lists the seven blockers with their
owner and their state: the legal clearance (E5), the signing identity, the
Android key, the contact of the responsible person, the build of the source
archive, the reproducibility of the build, and the decision about the Mac App
Store.

**An agent cannot close any of them.** They are the work of a person.
---

## 3. The two independent rounds of this round

The round had two independent examinations. Both were run by agents that had
not written the code they examined.

### 3.1 The fifth audit of the redaction

**Method:** 32 purpose-made PDF files (built byte by byte, in Rust), the real
chain from the core to MuPDF to the exported UniFFI functions, and **two
independent oracles** for every output: a scanner of the auditor's own
(plain bytes, Flate, raw Deflate, LZW, UTF-16 in both byte orders, hex-encoded
UTF-16) and the text extractor of `mutool` 1.28.5. The encryption of the
attack files was validated byte for byte against a file that MuPDF itself
wrote. The check of the rendered page ran with the **real** OCR (Tesseract
5.3.0, the language data of the project): it reads "GEHEIM" with 96 %
confidence from the rendered page and 0 words after the redaction.

**Result:** no critical finding. In every one of the 26 attacks in which a
secret survived, the redaction **refused** and named the place, and the
document stayed byte for byte unchanged.

Three findings, two of them closed in this round:

| # | Finding | State |
|---|---|---|
| R5-1 (medium) | The clearing emptied the `/U` of an encrypted document when the encryption dictionary hangs as an **indirect** object — the form the specification asks for | **closed** — the dictionary is recognised by its content and is never descended into; the auditor re-measured the attack against the fixed tree |
| R5-2 (medium) | The search for the remaining text compared **bytes** and did not see a word that the writer split over several show operations | **closed** — the text of the content stream is read now |
| R5-3 (low) | The verification runs behind the garbage collection of the writer | **open** — the right order is the first point of the recommendation of the auditor for the next round |

The report and its raw protocol (1,536 lines) are in the repository:
[`docs/audit/redteam-round-five-document-level.md`](../audit/redteam-round-five-document-level.md)
and [`docs/audit/redteam-round-five-measurements.txt`](../audit/redteam-round-five-measurements.txt).

### 3.2 The second, independent review of the rail of the profession

The first report on the palette store was measured by the team lead. The
second round was run by an agent that had not seen the first report, with a
harness of its own against the real `libvocument_ffi.so`. It found **three
places where the first report was wrong** — a link that points nowhere, a
read path without the check of the owner, and a refusal of `..` that hit the
part of the path the platform chose. All three are closed, each with a test.

The round also measured that eight processes that write the rail at the same
instant leave a **complete** rail: 5,900,144 samples of a watcher, not one
torn or mixed state.

The report is in the repository:
[`docs/audit/redteam-palette-independent.md`](../audit/redteam-palette-independent.md).
The first report is corrected in place.

### 3.3 The Apple conditions, checked against the real contract versions

The hypothesis of the examination — that a free application runs under
Schedule 1 and that Schedule 1 does not know the *commissionaire* — was
**refuted by the text**: Schedule 1 §1.1 (ii) and §1.3 appoint Apple as
commissionaire, and §3.1 uses the same wording as Schedule 2 §3.1. The
conclusion of the report stands for a free application; the evidence and the
framing did not, and ten findings corrected the repository.

The report is in the repository:
[`docs/audit/apple-terms-verification.md`](../audit/apple-terms-verification.md);
the living digest is
[`docs/audit/store-requirements.md`](../audit/store-requirements.md).

---

## 4. What is not proven in this round

- **The Swift build of the application**: no macOS host, no Swift toolchain.
  The structural check is **not** a compilation, and the acceptance record
  says so.
- **The surface of the review workflow** (the drag, the panel, the warning):
  **NOT RUN** for the same reason. The data path behind it is proven.
- **The build of the Android release package**: no Android SDK in this
  environment. The script carries the commands; nothing was signed.
- **The signing and the notarisation**: **NOT RUN**.
- **The legal clearance of the project owner (E5)**: **NOT RUN** — it is not
  the job of an agent.
- **The upload into a store**: **NOT RUN**. The Mac App Store is deferred.
- **The fifth red team round** against the redaction: the report is in the
  working directory of the release team; see the note in the M2.1 acceptance
  record.
- **The CI workflow**: no runner. The local gate carries the same checks.
- **AddressSanitizer**: only with a nightly toolchain — **NOT RUN**. The
  substitute is the valgrind memcheck.
- **The interoperability with the tools of other vendors** (Acrobat, Foxit,
  Okular): still outstanding.

## 5. The state of the project

M2 is **started, not completed**. Four slices are done — M2.1 (redaction with
verification), M2.2 (links and actions), M2.3 (attachments and signatures),
M2.4 (the comment of the reviewer with the rail of the profession) — and all
four are verified on the host. The review workflow of the macOS shell is now
a **workflow** and not a collection of views, but the step of the interface
remains **NOT RUN**.

The licensing decision E1–E5 is unchanged, the Mac App Store is **deferred**,
and the direct distribution of macOS is the channel that is prepared.

# Round 5 Acceptance Record — the verification that may not lie, the Swift proof and the release evidence

**Date:** 2026-10-21
**Branch:** `main`
**Scope of the round:**

1. the **hardening of the verification of a redaction**: a limited list of
   places, a limited depth of the walk, a failed inspection and an
   insufficient reading of the text must not be able to produce a
   **successful verification** — proved with independent, adversarial tests
   over the whole chain and over the **exported file**
2. **M2 made functional**: the provable Swift errors fixed, a real
   static proof of the Swift sources, the chain of the review workflow
   (mark → preview → remove → comment → save → reopen) checked against the
   core
3. the **release preparation completed**: the source archive built and
   checked, the reproducibility proved, the build record written, the
   responsibilities in `RELEASE-BLOCKERS.md` corrected
4. the **strategy unchanged**: free, fully open source under the AGPLv3,
   with MuPDF, without mandatory online services; macOS direct distribution
   and the Android channels have the priority, the Mac App Store is
   deferred, no public binary without the legal clearance (E5)

**Result:** the code of the round is committed; the tests are green; the
licensing decision is unchanged; the Swift sources are proved statically and
parsed by the compiler; the source archive and the build record exist; the
blockers of a public binary are written down with their owner. The
redaction still has open limits — they are named in §4, and they are not
security promises.

---

## 1. The result overview

| # | Acceptance criterion | Environment | Command | Result |
|---|---|---|---|---|
| 1 | A place of the document that the walk **could not examine** is named — the limit of the depth, an object that cannot be read, an object that cannot be addressed | Host, real engine | `a_place_that_the_walk_could_not_examine_is_named` | **PASS** |
| 2 | A redaction that reaches beyond the depth of the walk is **refused** | Host, whole chain | `a_place_beyond_the_depth_of_the_walk_makes_the_redaction_refuse` | **PASS** |
| 3 | The keys of an encrypted document are **examined** and **never emptied** | Host, real engine | `the_security_dictionary_is_a_place_that_carries_the_text`, `the_keys_of_the_security_dictionary_are_not_emptied` | **PASS** |
| 4 | A document whose encryption dictionary hangs indirectly stays encrypted | Host, whole chain | `a_document_with_an_indirect_encryption_dictionary_stays_encrypted` | **PASS** |
| 5 | The places that carried the text **before** the removal are in the report — the proof does not rest on the writer | Host, real engine | the field `places_before_removal` in the report and in the bindings | **PASS** |
| 6 | A word that the writer split over several show operations is found and removed — also as UTF-16 | Host, real engine | `a_word_that_is_split_over_several_show_operations_is_found`, `a_word_that_is_split_and_stored_as_utf16_is_found` | **PASS** |
| 7 | A secret that is encoded is found and removed | Host, real engine | `a_secret_that_is_encoded_is_found_and_emptied` | **PASS** |
| 8 | Two words that stand next to each other are not one word | Host, real engine | `two_words_that_stand_next_to_each_other_are_not_one_word` | **PASS** |
| 9 | **The report of the places is not cut off** — the note about a place that could not be examined is not lost | Host, real engine | `the_report_of_the_places_is_not_cut_off` | **PASS** (fails with the old limit of 200) |
| 10 | A document that is longer than the report can hold is **refused** instead of verified | Host, whole chain | `a_place_that_the_walk_could_not_reach_is_named_even_in_a_long_document` | **PASS** (fails with the old code — proved) |
| 11 | The **sixth round of the red team** over the exported file | Host, own harness, 20 own files | `docs/audit/redteam-round-six-document-level.md` | 19 of 20 cases behave as promised; **one defect found and fixed** (R6‑1) |
| 12 | The **Swift sources are sound** — the grammar of Swift, the names against the generated bindings, the self-test of the check | Host | `python3 scripts/check-swift-sources.py` | **PASS** (10 files) |
| 13 | The check **finds** the defect that was shipped (`risyCount`) | Host | the same, against the stand `ceabcb3` | **PASS** |
| 14 | Every Swift file is **parsed by the compiler** | Host, Swift 6.0.3 | `swiftc -parse <file>` for every file | **PASS** (not one error) |
| 15 | The **generated bindings** are built by the compiler | Host, Swift 6.0.3 | `swift build --package-path apps/macos --target VocumentFFI` | **PASS** (21.27 s) |
| 16 | The **application** of macOS is built and run | macOS | `swift build --product VocumentMacApp` | **NOT RUN** (no macOS host, no macOS SDK — SwiftUI, AppKit, CoreGraphics do not exist for Linux) |
| 17 | The **chain of the review workflow** of macOS is checked against the core | Host, by reading | `docs/audit/swift-verification.md` §4 | **PASS** (seven defects found and fixed) |
| 18 | The **source archive** is built and carries the revision completely | Host | `scripts/release-source-archive.sh` + `git ls-tree` | **PASS** (375 files, byte for byte, SHA-256 `0d468656…08b47`) |
| 19 | The build from the archive **reproduces** the artefact | Host | `scripts/verify-source-archive.sh` | **PASS** (two builds bit-identical) |
| 20 | The **build record** of a gate run is written | Host | `scripts/build-record.sh` | **PASS** (after two corrections — see §2.5) |
| 21 | The **gate** of the repository is green | Host | `bash scripts/local-ci.sh` | **PASS** |
| 22 | The **blockers** of a public binary are written down with their owner | — | `docs/releasing/RELEASE-BLOCKERS.md` | **PASS** (B‑1, B‑2, B‑3, B‑4, B‑7 for the owner; B‑5, B‑6, B‑8 closed by the team) |
| 23 | The **Android runtime proof** | Android | `scripts/verify-android-runtime.sh` | **NOT RUN** (no Android SDK in this environment) — the step says so in the gate |
| 24 | The **legal clearance** of the project owner (E5) | — | — | **NOT RUN** (an agent cannot give it) |
| 25 | The **signing** of the macOS bundle and of the Android package | macOS / Android | — | **NOT RUN** (no identity, no release key) |
| 26 | The **interoperability** of the redaction with Acrobat, Foxit and Okular | — | — | **NOT RUN** (the round used the scanner of the attacker and `mutool`) |

**257 tests, 33 suites, 0 failures.** `cargo fmt`, `cargo clippy
--workspace --all-targets -- -D warnings`, the Swift structure check, the
Swift proof, the licence audit, the gate of the distribution and the
validation of the repository are green.

---

## 2. What was broken and what was done

### 2.1 The verification could promise more than it knew

**The finding:** the walk over the objects of a document stopped at a depth
of 48, it skipped an object it could not read, and it said **nothing**. A
redaction that reached beyond that depth was reported as verified — the
report did not know that a place had never been looked into.

**What was done:**

* the walk carries the places it could not examine (`uninspected`) and
  writes them into the report; the core takes them into
  `uninspected_places` and into `not_checked_reasons`; `is_verified()` is
  false; the FFI raises `RedactionUnverified`; the document is rolled back.
* the **security dictionary** is examined again (the walk does not skip it
  any more) and the **scrub refuses** to empty the values of it: the check
  has to see the secret, the cleaning must not destroy the document. A
  secret in `/O` or `/U` leads to a refusal that names the place — and the
  document stays encrypted and readable.
* the places that carried the text are found **before** the removal and
  are in the report (`places_before_removal`). The proof no longer rests on
  the garbage collection of the writer.
* the reading of the text is no longer limited to the bytes: the run of a
  text is assembled over the show operations, and the word is looked for in
  the plain text, in UTF-16 (big and little endian), in the hex form of
  UTF-16 and in base64.

**The evidence:** `docs/audit/redaction-verification-hardening.md` (G‑1 to
G‑4), the tests of `engines/pdf-mupdf/src/scrub_hardening_tests.rs` and
`platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs`.

### 2.2 The report of the places was cut off — a lie in a long document

**The finding (red team VI, case `a16`):** the report of the places was
limited to 200 lines. A document with 201 pages that carry the secret
filled the report before the note about the place that could not be
examined was written. The note was dropped, the report looked complete, and
the redaction was reported as **verified** — while a place of the document
had never been looked into. The control next to it (`a17`, the same file
with two pages) was refused with exactly the reason that was lost.

**What was done:** the limit of 200 lines is gone; the report grows with
the buffer of the caller (16 KiB → 8 MiB); a report that does not fit is
said to be **incomplete** (the status `5`), and an incomplete report is a
failed check — the redaction is refused and the document stays as it was.

**The proof:** two regression tests, one in the engine and one over the
whole chain. The second one was run against the old code before the fix:
it failed with `verified: true, uninspected_places: []` — the exact lie of
the attacker.

### 2.3 The macOS application did not compile — and nobody could see it

**The finding:** the review panel read `inspection.risyCount`; the
generated bindings know `riskyCount`. The file did not compile, and nothing
in the repository noticed: there was no Swift toolchain, and the
structural check counts brackets — it does not know the names.

**What was done:** seven defects of the macOS sources were found and fixed
— the name, the missing re-render after a removal (the user kept seeing
the text that had just been removed for good), the unchecked `verified` flag
and the lost reasons of the core, the warnings of the previous document
above the new one, the recovered document that was never read, and the
marked place that belonged to **no page** (a rectangle of page 3 would have
removed a text of page 5 that the user never saw in the preview).

**The proof that holds the next defect:** `scripts/check-swift-sources.py`
parses every Swift file with the real grammar of Swift
(`tree-sitter-swift` 0.7.1), resolves the names of the application against
the generated bindings (177 types, 809 members, 84 functions), and proves
itself with a self-test that breaks the sources in seven ways and demands
that every break is found. The check is part of the gate.

**And the compiler:** the premise of the earlier rounds — "a Swift
toolchain is not obtainable" — was true for those rounds and is **wrong for
this one**. The toolchain 6.0.3 was installed (the download needed three
attempts — the network of the sandbox is unreliable), every Swift file of
the product is **parsed by the compiler** without a single error, and the
generated bindings are **built** by the compiler. The application itself
needs the macOS SDK and stays `NOT RUN`.

### 2.4 The release preparation

* the archive of the source carries the identification of the revision
  (three places) and is checked against `git ls-tree` — 375 files, byte for
  byte;
* `scripts/verify-source-archive.sh` builds the product from the unpacked
  archive in a fresh directory and compares it with the build of the
  working tree: two builds, bit-identical;
* `scripts/build-record.sh` writes the record of a gate run — the
  revision, the tools, the counts of the tests, the gates, the archive — and
  the gate calls it, also when a step fails;
* `docs/releasing/REPRODUCIBILITY.md` names the environment of the
  measurement and what is **not** in it (no Swift, no Java, no Android SDK
  at the time of the measurement — the Swift toolchain came later in the
  day);
* `RELEASE-BLOCKERS.md` has a column for the responsibility: the technical
  work is the work of the team, the keys, the contact and the legal
  clearance are the work of the owner.

### 2.5 Two lies of the build record — found by the build record itself

The build record is only worth something if it tells the truth. Two of its
own statements were wrong and were corrected:

* the **count of the tests** was 105 instead of 255 — a greedy pattern in
  the `sed` ate the first digit of every two-digit number (62 became 2).
  The counts are read as fields now.
* the record **failed** at the end with `cp: … are the same file` — the
  log of the gate lies in the same directory as the log of the record.

A third correction belongs to the gate: the Android runtime proof needs the
Android SDK, and an environment without it does not fail the build of the
product — it makes the proof `NOT RUN`, and the gate says so.

### 2.6 Two files over the limit of the repository

The hardening of the round pushed two files over the hard limit of 900
lines — the gate said so (`ERROR (CODE-001)`), and the build record found
it:

* `core/vocument-core/src/redaction.rs` (935 lines) → the types of the
  report moved into `core/vocument-core/src/redaction_report.rs` (747 + 205);
* `engines/pdf-mupdf/src/scrub_tests.rs` (1064 lines) → the tests of the
  hardening moved into `engines/pdf-mupdf/src/scrub_hardening_tests.rs`
  (637 + 443).

---

## 3. The independent rounds of this round

### 3.1 The sixth round of the red team — the document level

Twenty files of its own, a harness of its own, a scanner of its own and
`mutool` — the attacker looked at the **exported file** and compared it
with what the report promised. Nineteen of the twenty cases behave as the
product promises; one was a defect and is fixed (§2.2). The report:
`docs/audit/redteam-round-six-document-level.md`.

The round confirmed as well:

* at the depth 49 the redaction is **refused**, the place is named, and
  the exported file is byte-identical to the input — the document was not
  touched;
* the keys of an encrypted document are examined and never emptied — the
  document stays encrypted;
* a word in pieces is found and removed (over show operations, as UTF-16,
  with a positioning in between);
* an object that only the cross-reference table reaches is found and
  cleaned — also with a lying `/Length` and a broken Flate filter;
* a word of three characters is found.

### 3.2 The second measurement of the Swift sources

The reviewer wrote the report of the Swift sources; the lead measured the
same thing again in its own session, so that the claim does not rest on one
environment: the toolchain was installed, every file was parsed by the
compiler, the generated bindings were built, the name resolution was run.
The result is in `docs/audit/swift-verification.md` §10.

---

## 4. What is not proven in this round

| The statement | The reason |
|---|---|
| The **application of macOS** compiles, starts and shows the review surface | no macOS host, no macOS SDK — SwiftUI, AppKit and CoreGraphics do not exist for Linux |
| The **behaviour of the user interface** of the review (mark, preview, remove, comment, save, reopen) | no macOS and no display; the chain was read and compared with the core, the picture on the screen is not proven |
| The **tests of the kit** (`swift test`) and the **handshake** Swift → UniFFI → Rust | the kit needs CoreGraphics |
| The **rendering after the removal** on a Mac | the fix is proved by the code and by the parse; the picture is not proven |
| The **on-device run** of Android | no device, no `/dev/kvm` |
| The **build of the Android package** (Gradle, AAB, APK) | no Android SDK in this environment |
| The **signing and the notarisation** | no identity, no release key, no macOS host |
| The **upload into a store** | the Mac App Store is deferred; the other channels need the legal clearance (E5) |
| The **interoperability** of the redaction with other tools | the round used the scanner of the attacker and `mutool`, not Acrobat, Foxit or Okular |
| The **archive of a document** (ZIP, RAR, 7z) | an unreadable place leads to the refusal of the redaction, not to an unpacking |
| The **OCR limit** | text that the OCR cannot read cannot be proved removed by the OCR; an empty result is not distinguishable from a clean page |
| The **AddressSanitizer** | only with the nightly compiler (`-Zbuild-std`); valgrind memcheck is the substitute |
| The **CI workflow** | no runner — no pass is claimed for it |

### 4.1 The limits of the redaction that remain

These are named in the reports and in `docs/project/RISKS.md`. They are not
security promises:

* the depth of the walk is 48 — a place below it is **named** and the
  redaction is refused, but the place is not examined;
* the archives of a document are not looked into;
* a word whose pieces stand far apart is not recognised as one word;
* the interoperability with other tools is not proved.

---

## 5. The state of the project

| The milestone | The state |
|---|---|
| **M1** (scan, OCR, PDF, search, save) | functionally complete and proved on the host; the **device** proofs (on-device run, on-device OCR, energy, thermals) are `NOT RUN` — no device, no `/dev/kvm` |
| **M2.1** (the secure redaction) | hardened through six rounds; the verification names every place it could not examine and refuses the redaction when it cannot know; the open limits are in §4.1 |
| **M2.2** (the links and the actions) | implemented and proved on the host; the display in the application is `NOT RUN` without a Mac |
| **M2.3** (the attachments and the signatures) | implemented and proved on the host; the same |
| **M2.4** (the comment of the reviewer with the rail of the profession) | implemented; the storage of the rail is proved by two independent reviews; the display is `NOT RUN` without a Mac |
| **The release** | the source archive, the reproducibility, the build record, the licence audit and the notices exist; the **blockers** B‑1, B‑2, B‑3, B‑4 and B‑7 belong to the owner |
| **The strategy** | unchanged: free, fully open source under the AGPLv3, with MuPDF, without mandatory online services, no telemetry; macOS direct distribution and the Android channels first, the Mac App Store deferred; **no public binary without the legal clearance (E5)** |

**The next round** should close the open limits of the redaction (the
archives, the depth of the walk, the interoperability with other tools) and
bring the **device** and **macOS** proofs onto a machine that has them.

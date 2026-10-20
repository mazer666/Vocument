# Interim Report — Round 2: M2.1 hardened, M2.2 and M2.3 started

- **Period:** Round 2 (continuation of `docs/reports/M1-NACHBEREITUNG-UND-M2-BEGINN.md`)
- **Starting point:** `main` = `5f615da`
- **End point of this round (before the final report):** `main` = `cc564e6`
- **Key figures:** 161 tests / 23 suites / 0 failures · clippy 0 · fmt 0 · `validate-repo.sh` 0 · Android `assembleDebug` green · 8/8 Android unit tests · every native library 16 KB page compatible

---

## 1. What this round delivered

| # | Objective | Result | Evidence |
|---|---|---|---|
| 1 | Review M2.1 critically for security | **7 findings, 2 of them critical — all fixed** | Commit `17b6f58` |
| 2 | Continue M2 with a focus on genuinely usable, fully integrated functions | **M2.2 (link and action inspection) and M2.3 (attachment and signature inspection) implemented and verified** | Commits `8d07725`, `cc564e6` |
| 3 | Preserve architecture, data integrity, security and platform compatibility | unchanged and upheld; the separation of engine → core → FFI is kept in both new functions | see §4 |
| 4 | Keep tests, acceptance criteria, documentation and project status current | 8 new tests, 2 acceptance records, 2 gate extensions, requirements/roadmap/risks/documentation updated | see §3 |
| 5 | Commit and push to `main` | 3 commits, straight onto `main` | `17b6f58`, `8d07725`, `cc564e6` |

## 2. The most important insight of the round

**A redaction that is reported as secure is a claim, not a property.** The
review of this round found seven weaknesses in the *own* implementation of
M2.1 — two of them critical:

1. The removed text stayed in the **bytes of the file**. MuPDF writes
   unreachable objects along when the garbage collection is not requested. The
   document looked redacted and still contained the text. Fixed with
   `opts.do_garbage = 1`.
2. The **image pixels** of the region stayed in place. A document carries the
   same text twice: as a text layer and as pixels in the image. For a scan the
   text layer is invisible — the image carries the words. Fixed with
   `PDF_REDACT_IMAGE_PIXELS`.

In addition it became clear that the previous **black box** made the check
worthless: it covered exactly what the check had to see. With `black_boxes = 0`
the rendered page shows the truth.

The consequence for the architecture: an incompletely verified redaction can
nowhere be treated as a success. The FFI reports it as an **error**
(`RedactionUnverified`), and `NotChecked` is explicitly not a `Passed`.

**This lesson went straight into M2.2 and M2.3:** the two new functions
separate facts from assessment. The engine reports what is there; the core
decides what is risky. The decision is therefore testable without a PDF — five
core tests without a single file.

## 3. The evidence

### 3.1 M2.1 — Redaction (commit `17b6f58`)

The verification now checks **four** things, because they have four different
failure modes:

| Check | What it proves |
|---|---|
| Text layer | The words of the region are no longer extractable |
| Full-text search (FTS5) | The index no longer finds them |
| Rendered page | On the image of the page nothing is readable at the place anymore (rendered and read with the OCR) |
| File bytes | The words are no longer present in the file as plain text |

Without a reader for the rendered page the check counts as *not carried out* —
that is not a passed test. The checked set is **word-accurate** (word centre
in the region), not line-wide.

New tests: 3 engine tests (the image pixels are replaced instead of covered;
the other pages stay intact; the region is empty in the picture of the page), 2
end-to-end tests across the FFI boundary (the removed text is gone in all four
channels, the undo is refused, the remaining text survives; a redaction that
removes nothing is refused).

Acceptance record: `docs/acceptance/M2.1-ACCEPTANCE.md` — section 5 lists all
seven findings with severity and fix.

### 3.2 M2.2 — Link and action inspection (commit `8d07725`)

The inspection reports the actions of the document: the links of every page
with type, target and position in the page space, and the action that runs on
**open**. Risky is everything that executes code, starts a file, submits a
form or imports data — and every link that leaves the machine (`file:`,
`http:`, `https:`, `javascript:`).

Two mistakes that only the test found:

- MuPDF does not keep link annotations in the annotation list of a page
  (`pdf_sync_annots` skips `/Link` and `/Popup`). The first version found
  **nothing** — for a document with five links an empty list came back.
- Only by reading the `/Annots` array directly is the **type** of the action
  known. The loaded link areas carry only the target; precisely the decisive
  information ("this is a script, not a link") would have been lost.

Tests: 3 engine tests (all actions are reported; the position is reported in
the page space; a document without actions is empty), 3 core tests for the risk
policy, 1 end-to-end test across the FFI boundary.

Acceptance record: `docs/acceptance/M2.2-ACCEPTANCE.md`.

### 3.3 M2.3 — Attachment and signature inspection (commit `cc564e6`)

The inspection reports the embedded files (name, type, size, whether the
content is really inside the document, whether the checksum matches) and the
signature fields (signer, time, reason, place) — and above all whether the
document was modified **after** the signature. That is decided by the engine,
because only it can read the byte range of the signature and compare it with
the size of the file; the core then only decides what that means.

Two mistakes that only the test found:

- The MIME type of an embedded attachment has to be **masked** in the PDF
  name: `/Subtype /application#2Foctet-stream`. Unmasked, MuPDF reports
  `invalid key in dict` — and the attachment is missing **without** an error
  in the document.
- The change check needs the **field**, not the signature
  (`annotation is not a signature widget`).

The test document is built in two variants: the signature covers the whole
file or only its beginning. The byte range is written with a fixed number of
digits so that the offsets stay valid — there is therefore no test document
whose signature is valid only on paper.

Tests: 5 engine tests, 2 core tests, 2 end-to-end tests across the FFI
boundary.

Acceptance record: `docs/acceptance/M2.3-ACCEPTANCE.md`.

### 3.4 M2.1 hardened — the independent adversarial audit (commit `123d7ac`)

The record of M2.1 was written by the team that built the feature. That is
not good enough for a security-critical function, so the redaction was
audited again by two auditors who had no part in the implementation. They
tried to **break** the guarantee with their own harness, built against the
production bridge and linked against MuPDF 1.28.5, with `mutool` as the
independent reference.

**They broke it.** Eleven findings, five of them critical or high. The
findings and their measurements are in `docs/audit/redaction-security-audit.md`
and `docs/audit/redaction-pdf-qa-audit.md`.

| Finding | Severity | What changed |
|---|---|---|
| The word boxes of a turned page (`/Rotate`) were inverted | High | The box of a word is the bounding box of all four corners of all of its characters; the distance between two characters is measured along the reading direction of the line |
| A redaction that covered only a part of a word left a fragment behind, and the verification reported success | Critical | The verification compares fragments and checks the redacted region itself |
| The text of an annotation, the value of a form field, the metadata and the XMP packet survived the redaction; the check over the bytes only saw plain text | High | The engine cleans those places and reports every place of the document that still carries the word — as plain text, as UTF-16 in both byte orders and as the hex string of the UTF-16 bytes |
| There was no rollback: the document was changed before the verification, and the caller received an error that said nothing about it | High | A redaction that cannot be verified is rolled back: the bytes are restored, the search index is built again, the transaction of the attempt leaves no trace in the journal, and the checkpoints are only deleted after a successful verification |
| Words with fewer than four characters were not checked | Medium | The region check covers every word, however short it is |
| The text of a Type 3 font is not removed by MuPDF at all | Critical | Such a document is reported as **not verified** and the redaction is refused |

The new proofs: the word boxes of a turned page, the removal of the text of an
annotation and of a form field, the search for a hex encoded word, a redaction
that only covers a part of a word, a redaction that cannot be verified leaving
the document unchanged, and the deletion of the checkpoints after a successful
redaction — against the real engine as well as against a mock that leaves the
text behind.

### 3.5 The gates

`scripts/validate-repo.sh` now checks in addition:

| Gate | What it enforces |
|---|---|
| M2.1 | `PDF_REDACT_TEXT_REMOVE`, `PDF_REDACT_IMAGE_PIXELS`, `opts.black_boxes = 0`, the picture check in the core, `RedactionUnverified` at the FFI boundary, the two engine tests |
| M2.1 (hardening) | the normalised word boxes of the text bridge, the fragment and the region check in the core, the search for the places of the document that still carry the text, the cleaning of the annotations, the form values and the metadata, the rollback of a redaction that cannot be verified, and the seven new proofs |
| M2.2 | `document_actions` in the engine interface, `inspect_document` in the core, the risk policy in the core, the FFI function, the three tests |
| M2.3 | `document_attachments` and `document_signatures`, `changed_since_signing` and `is_risky_attachment` in the core, the engine and end-to-end tests |

## 4. Architecture and platform

- **Separation of layers:** both new functions separate facts and assessment.
  The engine reports (type, target, position, byte range); the core decides
  the risk. The engine contains no policy, the core no PDF specification.
- **No new dependency, no change to the public API** of the existing types;
  the new types are additive.
- **Platform compatibility:** the new C bridges (`mupdf_inspect_bridge.c`,
  `mupdf_attach_bridge.c`) are built for Android as well and checked in the
  APK: every native library is 16 KB page compatible, the APK gate is green.
- **No mocks, no stubs:** all evidence runs against the real MuPDF engine and —
  in the end-to-end case — against the real OCR chain.

## 5. What is still open

| Item | State | Why |
|---|---|---|
| The macOS interface of all three functions | **NOT RUN** | no macOS host |
| The behaviour on a device (Android, macOS) | **NOT RUN** | no device, no `/dev/kvm` |
| Interoperability with Acrobat/Foxit/Okular | **NOT RUN** | M2 gate, open for all M2 functions |
| A document whose text is drawn with a Type 3 font | **NOT RUN** | MuPDF 1.28.5 cannot remove such a text. The redaction is refused for such a document (the verification fails, the document stays unchanged), but the text is not removed — a different approach is needed |
| The redaction in the applications | **NOT RUN** | The function is available across the FFI boundary, but neither the macOS nor the Android application calls it yet: the tool of the interface belongs to the macOS slice of M2 |
| The verification of the signature itself (chain, certificate) | **not started** | MuPDF offers no public interface for it in this version; what is checked is the *coverage* of the signature |
| M2: comments, stamps, comparison, jurist palette | **not started** | the next slices |
| Licensing decision E1–E5 | open | no release as "approved" |
| AddressSanitizer | **NOT RUN** | nightly `-Zbuild-std` only; valgrind memcheck as a substitute |
| CI workflow | **NOT RUN** | no runner; no PASS is claimed |

## 6. Outlook

The next slice in M2 is the **comment and annotation function** with the
jurist palette (`ANNOT-001`): it is verifiable on the host, because
annotations and forms already exist in the engine and transaction layer, and
it is the basis for stamps and comparison.

The independent adversarial second review of the hardened redaction runs in
parallel; its result is incorporated into the final report of this round.

# Red Team III — the document level: the hidden places of a PDF

- **Date:** 2026-10-20
- **Checked stand:** `main` at `67509f0` ("docs(roadmap): the hardening of the redaction and the
  two audits are part of the record"), working tree clean.
- **Method:** two independent checkers attacked the redaction. The checker of the document level
  built a Rust harness with its own target directory (`redteam-harness`), drove the real core
  with the real MuPDF engine over 30 purpose-built PDF files, and compared the result with
  `mutool` and with the raw bytes of the file. The checker of the chain worked on the exported
  UniFFI ABI with a C program. Neither checker changed a project file.
- **The raw reports of the checkers** (in German) and their measurement logs are in
  `target/audit/` and in the working directory of the plan
  (`/workspace/.mavis/plans/plan_41cfa72e/workspace/`). This document is the English record of
  the findings; every number below is taken from the measurement log of the checker.

---

## 1. The answer in one sentence

**No.** The redaction of the checked stand could be made to report success while the secret was
still in the document. The cause was systematic: the search for the places of the document and
the cleaning of those places both worked with a **list of known places** — and a PDF has more
places than the list.

## 2. The findings

| # | Severity | Finding | Evidence |
|---|---|---|---|
| D-1 | **critical** | The search and the cleaning did not visit the places a PDF can hide text in: a popup that does not stand in `/Annots`, the structure tree, the optional content, the name tree, the private data of a page. The redaction reported success while the secret stood in the document. | A1–A6, A10, A11 of the measurement log |
| D-2 | **high** | The check over the bytes of the file is skipped as soon as the word legitimately occurs somewhere else in the document. A secret in a place that the search does not visit is therefore not noticed. | Kernbefund of the checker, measured on A11 |
| D-3 | **medium** | The checks looked at the whole document instead of the marked place: a word that occurs twice on the same page refused every redaction of that word. | C8 |
| D-4 | **medium** | After a redaction that was rolled back, the session kept the revision of the attempt and claimed unsaved changes. | B5, C2 (revision 2, dirty true after the rollback) |
| D-5 | **medium** | A word that only stands next to the region was removed with it. | C4 |
| D-6 | **low** | A word of three letters is not looked for in the bytes of the file. | A8, A12 |
| D-7 | **low** | The content of an attached archive cannot be read: the secret can stay in the archive and the redaction reports success. | A13 |

### D-1 (critical) — the list of known places

The search for the places of the document and the cleaning of those places visited the content
stream of the page, the annotations of `/Annots`, the form fields, the metadata, the XMP packet
and the embedded files. A PDF has more places than that. The checker built one file per place
and measured what the redaction did with it.

The files are the same in every case: a page with the word `GEHEIM` at the top and the word
again in a hidden place.

```
### A1_popup_nicht_in_annots  (pdf=popup_hidden.pdf)   outcome : Ok
   checks : all four Passed
   bytes  : {'GEHEIM': False}
### A3_structtree            (pdf=structtree_hidden.pdf) outcome : Ok
### A4_pieceinfo             (pdf=pieceinfo_hidden.pdf)  outcome : Ok
### A5_ocproperties          (pdf=ocprops_hidden.pdf)    outcome : Ok
### A6_names_baum            (pdf=names_hidden.pdf)      outcome : Ok
```

The hidden places are the ones of the PDF specification:

```
6 0 obj  << /Type /Annot /Subtype /Popup /Rect [72 780 200 810] /Contents <FEFF00470045…> >>
7 0 obj  << /Type /Annot /Subtype /Text  /Contents (Notiz) /Popup 6 0 R >>      (page /Annots)
6 0 obj  << /Type /StructElem /S /Document /ActualText <FEFF00470045…> >>      (structure tree)
6 0 obj  << /Names [<FEFF00470045…>] >>                                        (name tree)
6 0 obj  << /Type /OCG /Name <FEFF00470045…> >>                                 (optional content)
6 0 obj  << /Private << /Data <FEFF00470045…> >> >>                             (private data)
```

The report of the redaction said "all four checks passed" — and the secret was in the document.

**The measurement of the checker has a blind spot of its own, and the blind spot made the
finding look smaller than it was:** the checker looked for the word in the **raw bytes** of the
file as plain text. The hidden places above store the text as UTF-16 (as a hex string), and the
raw bytes therefore do not contain the word — the checker's byte test said "not in the file"
although the secret was in it. The leak was real in every one of these files.

**Fix.** The search and the cleaning no longer work with a list. Both walk through **every**
object of the document (`engines/pdf-mupdf/csrc/mupdf_walk.h`): the pages, the catalog, the
annotations including their popups, the form fields, the metadata, the structure tree, the name
tree, the optional content, the private data and the embedded files. Every string and every
stream is offered to the caller, with the place it lies in. The cleaning empties every string
that carries the word; the search reports every place that still carries it.

**Proof.** `engines/pdf-mupdf/src/scrub_tests.rs`:
`the_places_that_a_list_of_known_places_does_not_reach_are_found` and
`…_are_emptied` build a document with all five places and prove that they are found and
emptied, while the text of the page survives.

### D-2 (high) — the byte check is switched off

```rust
// core/vocument-core/src/redaction.rs (the checked stand)
if haystack.contains(&needle) && remaining_occurrences.is_empty() {
    failed = true;
```

As soon as the word legitimately occurs somewhere else — on another page — the check over the
bytes is skipped. A secret in a place that the search does not visit is then not noticed: the
redaction reports success and the secret stays in the file (A11, `bytes: {'GEHEIM': True}`).

**Fix.** The cause of D-2 is D-1. With the complete search, the hidden place is *found* and the
redaction is refused, because a place that could not be cleaned stops the redaction. The byte
check remains what it is: a supplement that catches what no object of the document carries —
and its skip is now harmless, because the search no longer misses a place.

### D-3 (medium) — the checks were document-wide

```
### C8_wort_gleich_seite  (pdf=twice_same_page.pdf)
  outcome : Err redaction failed: the redaction could not be verified:
            text layer: the content is still there; search index: the content is still there;
            file bytes: the content is still there;
            the text is still there: content stream (page 0, content)
  text    : GEHEIM GEHEIM Alltag Text Zeile
```

The word stands twice on the same page. The user marks one place; the other place stays — and
the redaction is refused. A name, a word that matters, occurs in every document more than once:
the redaction would have been useless.

**Fix.** The checks of the text layer and of the search index are now **scoped to the marked
place**: a word that survives *at the place of the removal* (its box lies where a removed word
lay) stops the redaction; a word that stands elsewhere on the page is reported in
`remaining_occurrences` and does not stop it. The search over the document keeps its own
meaning: the index may not carry a word that the page does not carry any more.

**Proof.** `platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs`:
`the_same_word_twice_on_the_same_page_is_redacted_and_the_other_place_is_named`.

### D-4 (medium) — the session kept the state of a rolled back redaction

```
### B5_rollback_zustand  outcome : Err … rev/dirt: 2 True
```

The rollback put the bytes of the document back, but the session kept the revision of the
attempt (2) and claimed unsaved changes (true). A session that claims the revision of a
redaction that never happened makes the next save write a document the user never saw.

**Fix.** The rollback now synchronises the session with the transaction manager
(`core/vocument-core/src/redaction.rs`, `rollback_redaction`).

**Proof.** `core/vocument-core/tests/redaction_rollback_tests.rs`:
`a_redaction_that_fails_puts_the_revision_and_the_dirty_flag_back`.

### D-5 (medium) — a word next to the region went with it

```
### C4_ueberentfernung_zeile  (pdf=tight_leading.pdf)
  removed : ['GEHEIM', 'Anhang']
```

A word whose box only shares the edge with the region was removed with it. The word that stands
directly next to the marked place is not part of the redaction: no character of it lies in the
region, so nothing of it would have been left behind.

**Fix.** The rule "the region takes every word it touches" now requires a **real overlap**: two
boxes that only share their edge do not overlap (`word_touches_region`,
`core/vocument-core/src/redaction_rules.rs`). A word that lies *in* the region — even partly —
still goes as a whole, because MuPDF removes character by character and a fragment of a secret
is a secret.

**What is not changed:** a word whose box really overlaps the region still goes with it, even
if the user only meant the word next to it. The report names every word that was removed, so
that the user sees it. This is the trade-off of the second round, and it is deliberate: the
alternative — a partly covered word that keeps its characters — leaves a readable fragment.

### D-6 (low) — a word of three letters

The check over the bytes of the file only looks for words of at least four characters
(`MIN_CONTENT_CHARACTERS`): a word of three letters is found in every line of every font. The
other three checks are exact and carry every word, however short it is — the search over the
document, the search for the places of the document and the check over the marked region. The
measurement confirms it: the short word is removed and not found any more (A8, A12, B7).

**Status: unchanged and documented.** The limit is written down in
`core/vocument-core/src/redaction_rules.rs` and in the acceptance record.

### D-7 (low) — the content of an attached archive

```
### A13_zip_anhang  (pdf=zip_attachment3.pdf)
  outcome : Ok
  bytes   : {'Anhang': True}
```

The attached file is a ZIP archive. The text inside it is compressed; nobody can find it without
unpacking the archive. The redaction cannot clean an archive — an attachment belongs to the
user and is never deleted — and it cannot look into it either.

**Fix.** The redaction now names such a place: the engine reports the kind `uninspected`, the
core puts it into `uninspected_places` of the report, and the callers turn it into a warning.
The redaction does not claim that the text is gone from a place it could not read.

**Proof.** `platform/vocument-ffi/tests/redaction_hardening_e2e_tests.rs`:
`an_attached_archive_is_named_as_a_place_that_could_not_be_read`.

## 3. What the checker could **not** check

1. **No real OCR.** The system has no Tesseract library; the measurements ran with a stub that
   reads no words. The proof over the picture of the page is therefore empty in this round —
   the same limit as in the second round.
2. **No interface.** There is no macOS host and no Android device: no click, no screenshot, no
   bed path. The findings of the document level are therefore findings of the library layer.
3. **The session of the checker died** before it wrote its own report. The findings above are
   taken from its measurement log and from the notes it left in the board of the plan; the
   measurements themselves are complete and reproducible (`run_evidence.sh` in the working
   directory of the plan).
4. **No interoperability with other viewers.** Whether Acrobat, Foxit or Okular show the
   redacted document as expected is still open (the M2 gate).
5. **Type 3 fonts.** The premise of the first audit — that MuPDF 1.28.5 does not remove the
   text of a Type 3 font — could not be reproduced in the second round either. It is neither
   confirmed nor refuted.

## 4. The state after the fixes

| Finding | Severity | Status |
|---|---|---|
| D-1 the list of known places | critical | **fixed** — the complete walk over the objects of the document |
| D-2 the byte check is switched off | high | **fixed** — the cause (D-1) is gone; the check remains a supplement |
| D-3 the checks were document-wide | medium | **fixed** — the checks are scoped to the marked place |
| D-4 the state after the rollback | medium | **fixed** — the session is synchronised |
| D-5 a word next to the region | medium | **fixed** — a real overlap is required |
| D-6 a word of three letters | low | **unchanged, documented** — the other checks are exact |
| D-7 the content of an archive | low | **fixed** — the place is named as unread |

186 tests, 26 test suites, 0 failures. `cargo clippy --workspace --all-targets -- -D warnings`
is clean, `scripts/validate-repo.sh` passes.

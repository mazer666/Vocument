# The sixth round of the red team — the document level

**Role:** ROLE-09 (security) and ROLE-13 (QA) · **Date:** 2026-10-21 ·
**Branch:** `main` · **Milestone:** M2 (`REDACT-002`)

This round asked one question again and again: **does the verification of a
redaction tell the truth?** The attacker did not attack the product from the
outside this time — the attacker took the *exported file* of a redaction and
looked at it, with a scanner of their own and with `mutool`, and compared it
with what the report of the redaction promised.

The round ran with its own harness and its own twenty PDF files. It did not
use a test of the repository; the twenty files were built for this round.

---

## 1. The setup

| What | How |
|---|---|
| The harness | its own Rust program in the plan's workspace, its own `CARGO_TARGET_DIR`; it calls the **exported** functions of the real library — the same ones the application calls (`document_core_open_macos_scoped_file`, `document_core_redact_page_regions`, `document_core_export_session_bytes`) |
| The files | twenty PDF files, built for this round: a deep chain, an unreadable object, three encrypted documents, a form XObject with a word in pieces, a report that overflows, a comment, a word of three characters |
| The scanner of the attacker | its own: it looks for the secret in the raw bytes, in every decoded stream and in the assembled text of the streams — **and** in the text that `mutool draw -F txt` extracts |
| The rule | the secret must not be in the exported file. If the redaction is **verified**, the secret must be gone. If the secret is gone, the redaction may be verified. A report that promises more than the file holds is a lie. |

The raw protocol of the round: 1069 lines, twenty cases, every case with
the input, the output, the report of the core and the two scans.

---

## 2. The twenty cases

| The case | The attack | The result | The remainder | The secret in the output |
|---|---|---|---|---|
| `a01_deep_49` | the secret stands in the depth 49 — one level below the limit of 48 | **refused** | — | the file is **byte-identical to the input** |
| `a02_deep_48` | the secret stands in the depth 48 — exactly at the limit | verified | 0 | gone |
| `a03_deep_47` | the control of a02 | verified | 0 | gone |
| `a04_stream_length` | an object that only the table reaches: a stream whose `/Length` claims 1 | verified | 0 | gone |
| `a05_xref_offset` | an object whose entry in the table points at `000000999` | verified | 0 | gone |
| `a06_broken_flate` | an object that only the table reaches: a stream with a broken Flate filter | verified | 0 | gone |
| `a07_garbage_object` | an object whose body is not an object | **refused** | — | the file is unchanged |
| `a08_encrypt_u` | an encrypted document (RC4, 40 bit); the secret in the `/U` | **refused** | — | the document stays encrypted |
| `a09_encrypt_o` | an encrypted document; the secret in the `/O` | **refused** | — | the document stays encrypted |
| `a10_encrypt_direct` | an encrypted document whose dictionary hangs directly in the trailer | **refused** | — | the document stays encrypted |
| `a11_form_split_show` | a form XObject that shows the secret in two show operations: `(GE) Tj (HEIM) Tj` | verified | **1** | the **other** copy of the word — see §3.2 |
| `a12_form_split_td` | a form XObject that shows the secret in two pieces with a positioning in between | verified | 0 | gone |
| `a13_form_split_utf16` | a form XObject that shows the secret in two pieces as UTF-16 | verified | 0 | gone |
| `a20_form_split_td_gap` | a form XObject that shows the secret in two pieces, 10 points apart | verified | 0 | gone |
| `a14_page_split` | the secret stands in the content stream of the page, in two pieces | verified | 0 | gone |
| `a15_xref_only` | the secret stands in an object that nothing refers to; only the table reaches it | verified | 0 | gone |
| `a16_report_flood` | 201 pages carry the secret; the report of the places holds 200 lines; the note that a place could not be looked into is written down last and is therefore lost | **verified — a lie** | 200 | see §3.1 |
| `a17_flood_control` | the control of a16: the same file with two pages — the note fits into the report | **refused** | — | the file is unchanged |
| `a18_comment` | the secret stands in a comment of the PDF file | verified | 0 | gone |
| `a19_xref_three` | the secret "GEH" — three characters — stands in an object that nothing refers to | verified | 0 | gone |

Nineteen of the twenty cases behave as the product promises. The twentieth
case is a defect, and the control next to it proves it: the same document
with two pages instead of 201 is refused, and the same document with 201
pages is reported as verified.

---

## 3. The findings

### 3.1 R6-1 (critical, confirmed, fixed) — a report that was cut off made a promise

**What happened.** The walk over the objects of a document writes the places
that it could not examine **last** — the note about a place that the walk
could not reach is the last line of the report. The report was limited to
200 lines. A document with 201 pages that carry the secret filled the report
before the note was written: the note was dropped, the report looked
complete, and the core concluded that every place had been examined.

The report of the redaction of `a16`:

```
verified              = true
text_layer_clean      = Passed
search_index_clean    = Passed
rendered_page_clean   = Passed
file_bytes_clean      = Passed
remaining_occurrences = 200
uninspected_places    = 0 ([])          <-- the place is there, the note is not
not_checked_reasons   = []
```

The control `a17` — the same file with two pages — is refused with the
exact reason that was lost:

```
Fehler: the redaction could not be verified: … the document could not be
read completely: 1 place(s) could not be looked into, among them the limit
of the depth of the walk was reached /Root/D/D/…/S
```

**Why it matters.** This is the class of defect that the whole hardening of
the last rounds was about: a verification that promises more than it knows.
The user is told "the document is free of the marked text", and a place of
the document was never looked into.

**The fix.**

1. The limit of 200 lines is gone. The report grows with the buffer of the
   caller: the Rust side starts with 16 KiB and quadruples the buffer as
   long as the report does not fit (up to 8 MiB).
2. A report that does not fit is **said to be incomplete**: the bridge
   returns the status `5` (the report is incomplete) and the Rust side
   either grows the buffer or fails. A failed check is `NotChecked` — and
   `NotChecked` is not a pass: the redaction is refused and the document
   stays as it was.
3. The safety limit of 100 000 places is a net against a pathological
   document, not a rule about the report. A document that reaches it is
   reported as incomplete as well.

**The proof.**

* `engines/pdf-mupdf/src/scrub_hardening_tests.rs` —
  `the_report_of_the_places_is_not_cut_off`: a document with 201 pages that
  carry the secret and a chain of 60 dictionaries that the walk cannot
  reach. The report must name **every** page **and** the place that could
  not be examined. With the old limit of 200 the test fails.
* `platform/vocument-ffi/tests/redaction_report_completeness_tests.rs` —
  `a_place_that_the_walk_could_not_reach_is_named_even_in_a_long_document`:
  the same document through the whole chain across the FFI boundary. The
  redaction must be **refused**, the reason must name the place, and the
  bytes of the document must be unchanged afterwards.

The second test was run **against the old code** before the fix: it failed
with `the redaction must not be verified: … verified: true, uninspected_places: []` —
the exact lie of the attacker. With the fix it passes.

### 3.2 `a11` — no defect: the other copy belongs to the document

The attacker marked the copy of the word in a form XObject — the form shows
`(GE) Tj (HEIM) Tj` — and found the word in the exported file. The two
copies are not the same thing:

* the **marked** copy was removed: the stream of the form is `q Q` in the
  exported file — the form draws nothing any more;
* the **unmarked** copy — the same word, 700 points higher on the same
  page, visible to the user — belongs to the document. It is named in the
  report (`remaining_occurrences = 1`, `content stream of the page`), and
  the applications turn that into a warning for the user: *"the same text
  still stands at content stream of the page (page 0)"*.

A redaction removes what the user marked. It does not remove the rest of
the document behind the back of the user — and the report says so. The
finding of the attacker is therefore **not** a defect of the product; it is
the documented behaviour of a redaction of a region.

### 3.3 What the round confirms

* The limit of the depth of the walk is honest: at the depth 49 the
  redaction is refused, the place is named, and the exported file is
  byte-identical to the input (the document was not touched).
* The keys of an encrypted document are **examined** (the secret in `/O`
  and `/U` is found) and **never emptied** (the document stays encrypted
  and readable).
* A word in pieces is found and removed: over several show operations, as
  UTF-16, and with a positioning in between.
* An object that only the cross-reference table reaches is found and
  cleaned — also when its `/Length` lies or its Flate filter is broken.
* A word of three characters is found: the comparison does not need a
  minimum length to find a word in a hidden object.

---

## 4. The limits of this round

* The interoperability of the redaction with other tools (Acrobat, Foxit,
  Okular) is still open: the round used the scanner of the attacker and
  `mutool`, not a third-party program.
* The archive of the document (a ZIP, a RAR, a 7z) is still not looked
  into: an unreadable place leads to the refusal of the redaction, not to
  an unpacking.
* The round ran on the host, not on a device: the behaviour of the
  application on Android and macOS is `NOT RUN` (see the acceptance record
  of the round).
* The harness of the round was stopped by the time limit of the session
  before it wrote its own report. The measurements were evaluated by the
  lead; the raw protocol (1069 lines) is in the workspace of the plan, and
  the two tests of the finding are in the repository.

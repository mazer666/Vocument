# Red Team II — the redaction attacked on the document level

- **Date:** 2026-10-20
- **Tested state:** `main` at `f59c262` and the changes in the working tree
  of the same day
- **Harness:** an own Rust harness against the **real** `vocument-core` and
  the **real** MuPDF engine, built with an own `CARGO_TARGET_DIR`; `mutool`
  1.28.5 as the independent reference
- **Original wording:** the auditors worked in German. This document is the
  English account of their findings; the measurements, the commands and the
  file names are theirs.

## 1. The answer in one sentence

**The promise of the redaction did not hold.** In three classes of cases the
redaction reported itself as *fully verified* (all four checks `Passed`)
while readable remains of the secret were still in the document.

| # | Severity | Finding |
|---|---|---|
| B-1 | high | A word that is only partly covered leaves a readable fragment behind — and the report does not mention it |
| B-2 | high | The XMP packet **of a page** is neither cleaned nor reported |
| B-3 | high | A word that is spread over several objects or interrupted is not recognised |
| B-4 | high | A three-letter secret in `/Alt` survives; the byte and the search check report "passed" **without checking anything** |
| B-5 | medium | Form values and annotation texts that the engine cannot clean stop the redaction and are **not named** |
| B-6 | medium | After a failed verification the **state of the session** is not put back |
| B-7 | medium | The picture check is a difference check — a word that the OCR did not read before is not noticed |
| B-8 | medium | The same term on another page: the search reports "passed" although the search still finds the word |
| B-9 | low | A secret that is encoded in Base64 stays undetected |

The **removal** by MuPDF itself proved to be solid: text, image pixels, Type 3
fonts, form XObjects (also scaled, mirrored, turned), turned pages with a
crop box offset, mirrored text and text beyond the edge of the page are
removed correctly and **completely**. The gaps are in the **verification**
and in the **reporting**, not in the removal.

## 2. The findings

### B-1 — high — a partly covered word leaves a fragment that is not checked

MuPDF removes **character by character**. The hardening of `123d7ac`
compared such remains with `is_fragment_of` — but only against the words
whose **centre** lay in the region. A word that is only partly covered has
its centre **outside** the region, and it was therefore neither in
`removed_words` nor in `checked_words`: no watcher looked at it.

- Evidence: the document `GEHEIM` (72…166.68 pt) and `AB` (72…104.02 pt) on
  two lines, the region `x 72…106.68 pt` (it covers "GE" and "AB" completely).
  The report said `file_bytes_clean: Passed`, `rendered_page_clean: Passed`,
  `search_index_clean: Passed`, `text_layer_clean: Passed`, and named only
  `AB` as removed. The document still held **"HEIM"** — extractable with
  `mutool`, in the search index and in the bytes of the file.
- The other two cases: the remain "GEH" (prefix) and the single letter "M".
  In the last case the report even named "GEHEIM" as removed while the
  document held "M".
- **Fixed:** a word that the region **touches** belongs to the redaction, and
  the region grows to the box of the whole word (`word_touches_region`,
  `extend_regions_to_touching_words` in `core/vocument-core/src/redaction_rules.rs`).
  The whole word goes, and the report names it.

### B-2 — high — the XMP packet of a page is neither cleaned nor reported

The scrub of the engine only touched the **document** XMP packet
(`Root/Metadata`). A **page** metadata packet — common in PDF/A and PDF/X
files — was left alone. It is a **Flate compressed** stream, so the byte
check of the core did not see it, the text layer did not hold it and the
rendered page did not show it.

- Evidence: a page with `page/Metadata` (XMP with "GEHEIM") and the text
  "GEHEIM" on the page. The redaction reported all four checks as passed;
  `mutool show` on the redacted file still showed
  `<rdf:li xml:lang='x-default'>GEHEIM</rdf:li>`.
- **Fixed:** the scrub and the search for the places of the document cover
  the packet of every page (`vocument_scrub_page_metadata`).

### B-3 — high — a word that is spread over several objects is not recognised

A word that the author wrote in two parts — two text objects, a text with an
offset, a word that is interrupted by a drawn line — is not one word in the
text layer, and the check that compares the words looked at single words
only. The remain of the second part was not a fragment of the first part
("HEIM" is not a prefix or a suffix of "GE"), so nothing noticed it.
- **Partly fixed:** the region now takes every word it touches, so both
  parts of a word that lie in the marked region go. A word whose parts lie
  **apart** from each other is still a limit of the check — it is listed in
  section 4.

### B-4 — high — a three-letter secret survives, and the checks report "passed" without checking

For words with fewer than four characters the list of the words to check was
empty. The search check and the byte check then reported `Passed` — without
having looked at anything. Together with the fact that the scrub did not
empty `/Alt`, this was a silent leak: the secret stayed in the document and
the report claimed that the bytes were clean.

- Evidence: a page with "TOP", the annotation 5 with `/Alt (TOP)` and the
  annotation 6 with `/T (TOP)`. The report said "all four passed", the
  removed word was "TOP", and `mutool show` still showed
  `5 0 obj <</Alt(TOP)/Rect[...]>>`.
- **Fixed:** the exact checks — the search over the document and the search
  for the places of the document — now carry **every** word, however short it
  is. A check that could not be carried out is `NotChecked`, never a pass.
  The annotations are cleaned down to their alternative text as well.

### B-5 — medium — the places that could not be cleaned are not named

A place of the document that survives the cleaning — an embedded file, a
field whose value cannot be written — stops the redaction. The error named
the check that failed, but not the place.
- **Fixed:** the places are named in the error: "the text is still there:
  … (page …, …)".

### B-6 — medium — the state of the session is not put back

The rollback restored the **bytes**, built the search index again and
removed the transaction of the attempt — but the **revision** and the
changed flag of the session stayed on the higher value. A document that is
marked as changed although it is the saved one makes the user save a
document that nobody touched.
- **Fixed:** `discard_last_transaction` sets the revision back to the value
  of the transaction before the attempt.

### B-7 — medium — the picture check is a difference check

The check compared the words that the OCR read **before** the removal with
the words it read **afterwards**. A word that the OCR could not read before
was never in the list, and it survived the comparison unnoticed.
- **Fixed:** the check now looks at the picture itself: nothing may be
  readable at the place of the removed word any more, and everything that
  was readable in the region before has to be gone. **The limit remains:**
  text that the OCR cannot read at all cannot be proven to be gone by the
  OCR — see section 4.

### B-8 — medium — the same term on another page

The search check looks at the hits of the **redacted page** — a word that
occurs on another page does not stop the redaction. The report said nothing
about it, and the word was still in the document.
- **Fixed:** the report names the places where the same word still stands
  (`remaining_occurrences`), across the FFI boundary as well. The promise of
  the redaction is about the **marked region**; the promise that a secret is
  nowhere in the document is the job of the full sanitising (`SAN-001`).

### B-9 — low — a secret in Base64

A secret that is stored in Base64 stays undetected: the search looks for
plain text, for UTF-16 in both byte orders and for the hex string of the
UTF-16 bytes.
- **Not fixed.** A Base64 encoded secret is a different encoding of the
  same secret, and a search for it would need the decoded forms of every
  word. It is listed in the acceptance record as a limit of the search.

## 3. What the auditors did **not** find

These are results, not failures — each of them was tried and each of them
behaved correctly:

| Vector | Result |
|---|---|
| Type 3 fonts | MuPDF 1.28.5 **removes** the text of a Type 3 font in this construction. The premise of the first audit ("MuPDF does not remove Type 3 text at all") could **not** be reproduced. Other constructions (glyph procedures with their own CTM, a Type 3 in an XObject) were not checked exhaustively |
| Text in a form XObject | removed, also scaled (0.5), mirrored, vertically mirrored and turned by 90 degrees |
| Turned pages `/Rotate` 90/180/270 | removed, also with a crop box offset |
| Mirrored text (`Tm` with a negative x) | removed |
| Text beyond the edge of the page | removed |
| Image pixels under the region | the pixels are **replaced** (12 563 of 60 000 image bytes changed), not covered |
| Octal escapes (`\107EHEIM`) and UTF-16 BE/LE in annotations | both forms are emptied |
| Several regions on one page, overlapping regions | handled |
| A region outside the page, an empty region, a one pixel region | refused with `NothingToRemove`, the document unchanged |
| Two redactions in a row on one page | the content of the first one does not come back |
| The engine fails (status ≠ 0) | the file is really unchanged |
| The search index returns a different hit | the check on the page stays sharp |
| Object streams (PDF 1.5+) | MuPDF writes without object strings; an incoming document with object streams is unpacked on writing |
| Incremental updates (old versions in the file) | MuPDF writes completely new (`do_incremental` is not set, `do_garbage = 1`) |
| The rollback | after a successful redaction only the manifest and the journal are left in the recovery store, **no** checkpoint, no file with the secret |
| The recovery of a failing write | `restore_pdf_bytes` only fails on empty bytes; the path is practically unreachable |

## 4. What could not be checked in this environment

| Vector | Why |
|---|---|
| The **real OCR** (Tesseract) on the rendered page | the host has no Tesseract; the OCR reader was replaced by stubs (text layer, "merged", "late", "stale") that check the **logic** of the picture check. The behaviour of a real OCR (thresholds, word segmentation) is therefore not proven |
| A **real scan** (text only as pixels, no text layer) | the image path was checked with a synthetic RGB image (pixels replaced), not with a real scan with OCR |
| Encrypted documents (an encrypted string as a field value) | no encrypted test document was built |
| A secret in an **embedded font program** (Type 1C/CFF charset, FontFile2) | no valid Type 1C program was built by hand. The suspicion stays unproven: such a name would lie in a **compressed** stream and would be visible neither to the byte check nor to the search for the places — the likeliest further gap of the same class |
| Object streams that MuPDF does not unpack | not built |
| Full width characters, ligatures, hyphenation, bidirectional text | not built. The comparison form of the core normalises the case and removes the punctuation — for ligatures (e.g. "ﬁ") and full width characters the **extraction** would be the decisive place, not the comparison |
| The macOS and the Android interface | no macOS host, no device |
| The cleaning of the **attachment** | the scrub does not touch embedded files on purpose; the search for the places reports them, so the redaction is **refused**, not silently successful. That is the documented intention ("destroying the data of the user is worse") |

## 5. The evidence

The auditors left their measurements behind: the test documents, the
harnesses and the raw results. The findings of this document were turned
into the hardening of the redaction (`fix(m2.1): the second adversarial
round`) and into the acceptance record
(`docs/acceptance/M2.1-ACCEPTANCE.md`).

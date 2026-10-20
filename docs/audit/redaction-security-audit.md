# Independent adversarial audit of the redaction (M2.1)

**Date:** 2026-10-20
**Audited state:** commit `cc564e6` (the state of `main` when the audit started)
**Method:** two independent auditors attacked the redaction with their own C
harness, built against the production bridge
(`engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c`) and linked against
MuPDF 1.28.5. They did not use the Rust code of the repository; the behaviour
of the core was reconstructed from the source. `mutool` 1.28.5 was used as the
independent reference for the content of a document.

This report collects the findings of that audit. Every finding names the
measurement that backs it. The last section states what the auditors could
**not** check.

---

## 1. Summary

The claim of the acceptance record — "the redaction removes the content, and
the removal is verified on four independent ways" — **did not hold** at the
audited state.

| # | Severity | Finding | Now |
|---|----------|---------|-----|
| F1 | critical | Text in a Type 3 font is not removed at all and stays visible | fixed by the region check (F2) and reported as unverified |
| F2 | critical | A redaction that only covers part of a word leaves a fragment, and the verification reports success | fixed (`is_fragment_of`, region check) |
| F3 | high | On a page turned by 180 or 270 degrees the word box is cut off, only the first and the last character are removed, the verification reports success | fixed (normalised word box) |
| F4 | high | On a page turned by 90 degrees the word box is inverted, the redaction fails with status 7 | fixed (normalised word box) |
| F5 | high | The value of a form field is not removed | fixed (engine scrubs the form values) |
| F6 | high | There is no rollback: the document is changed before the verification runs | fixed (the core restores the state) |
| F7 | medium | Words with fewer than four characters are not checked and not named in the report | fixed (the region check covers every word) |
| F8 | medium | The check over the bytes looks at the whole document: the same word somewhere else refuses the redaction | known limit, documented |
| F9 | medium | Annotations, stamps, XMP and `/Info` are not touched | fixed (engine scrubs them) |
| F10 | medium | The check over the bytes only sees plain text: a UTF-16 or hex encoded text is not found | fixed (the engine reports the places) |
| F11 | medium | An embedded file that carries the text survives completely, and all four checks report success | fixed (the engine reports it, the redaction is refused) |
| F12 | low | The metadata does not record that the document was changed | known limit, documented |
| F13 | low | The comparison of the picture compares whole words | fixed (fragments are compared as fragments) |

---

## 2. The findings in detail

### F1 — critical — text in a Type 3 font is not removed

MuPDF does not remove text that is drawn with a Type 3 font. In
`source/pdf/pdf-clean.c`, `init_redact_filter` sets:

```c
red->filter_opts.recurse = 0; /* don't redact patterns, softmasks, and type3 fonts */
```

**Measurement** (auditor's harness, `redact_cli`, against the production
bridge):

```
$ ./redact_cli t8_type3_real.pdf out_t8_type3_real.pdf 0 70000 120000 280000 145000
status=0 err= out_len=1098
$ mutool show out_t8_type3_real.pdf 9
9 0 obj
<< /Length 39 >>
stream
q BT/T3 18 Tf 72 700 TD (SECRET)Tj ET Q
endstream
$ mutool draw -F stext -o - out_t8_type3_real.pdf | grep ^<line
<line bbox="72 124 180 142" wmode="0" dir="1 0" flags="0" text="SECRET">
```

The text is still extractable, and the rendering still shows it: the number of
dark pixels in the region of the page is **8322 before and 8322 after** the
redaction.

The verification caught this only because the word has more than three
alphanumeric characters. With a short word the check was vacuous
(see F7) and the redaction was reported as verified.

**Consequence:** the state of the audited code was not able to guarantee the
removal at all. **Remedy:** the region check (F2) detects the surviving text
in the region, and the redaction is reported as *not verified*. The text of a
Type 3 font cannot be removed with MuPDF 1.28.5; a document that uses one has
to be rejected by the user, not silently reported as redacted.

### F2 — critical — a partial redaction leaves a fragment

The removal works character by character. A region that covers only the
beginning of a word leaves the rest of the word behind. The verification
compared whole words, so it did not see the fragment.

**Measurement:**

```
$ ./redact_cli t1_text.pdf out_t1_part.pdf 0 70000 125000 170000 145000
$ mutool show out_t1_part.pdf 10
q BT/F1 18 Tf 173.01599 700 TD (CH GEHEIM)Tj ET ...
$ mutool draw -F stext -o - out_t1_part.pdf | grep ^<line
<line bbox="173.01599 128.662 275.02198 142.414" ... text="CH GEHEIM">
```

"CH" is not the word "VERTRAULICH", so the comparison of whole words passed
although a readable fragment of the secret remained in the file and on the
page.

**Remedy:** the core now compares fragments as well
(`is_fragment_of` in `core/vocument-core/src/redaction.rs`): a word that
remains in the text and is a prefix or a suffix of a removed word is a
survivor. A single character is not enough evidence — it occurs in almost
every document and would refuse every redaction.

### F3/F4 — high — turned pages (`/Rotate`)

On a page that is turned, the four corners of a character are not in the
order "upper left, upper right, lower left, lower right". The production
code built the box of a word from the upper-left and the lower-right corner
alone and grew it in one direction only. On a turned page the result is
inverted or cut off.

**Measurement:**

```
/Rotate 90:  the box of "VERTRAULICH" is
              x0=119350, x1=94618  → inverted
              the redaction fails: "MuPDF failed to redact the page (status 7)"
/Rotate 180: the text layer of the redacted file contains "ROTATED S" and "T"
              → the first and the last character survive, the verification
                reports the region as clean
```

**Remedy:** the box of a word is now the bounding box of all four corners of
all of its characters (`engines/pdf-mupdf/csrc/mupdf_text_bridge.c`). The
distance between two characters is measured along the reading direction of
the line, so that a gap on a turned page is recognised as well. The
regression test `engines/pdf-mupdf/src/redaction_rotation_tests.rs` fails
without the correction and passes with it.

### F5 — high — the value of a form field survives

The value of a form field lives in `/V` (and `/DV`). MuPDF does not touch it.

**Measurement:** `t6_formfield.pdf` → after the redaction the text layer
still contains "GEHEIMFORMULAR". The check of the text layer reported
"failed" in this case — but the document had already been changed.

**Remedy:** the engine scrubs the value of the form fields that carry the
text.

### F6 — high — no rollback

`core/vocument-core/src/redaction.rs` removes the content first
(`self.apply_redaction(...)`, which also deletes the checkpoints of the
recovery store) and verifies afterwards. When the verification fails, the
document is already changed for good, the caller receives an error — and
believes that nothing happened.

**Remedy:** the core restores the state of the document when the
verification fails, and deletes the checkpoints only after a successful
verification.

### F7 — medium — short words are not checked

`is_content_word` requires four alphanumeric characters. A word with three
characters ("PIN") was neither checked nor named in the report: the report
claimed a verified redaction without naming anything.

**Remedy:** the region check covers every word, however short it is. The
limit of four characters remains only for the checks that look for a word in
the whole document, where a short word would be no evidence.

### F8 — medium — the check over the bytes looks at the whole document

The same word somewhere else in the document refuses the redaction, even
though the redaction itself was correct. The check is also vacuous for a
compressed content stream: the text is not findable in the raw bytes even
before the redaction.

**Status:** known limit, documented in the acceptance record. The check is
strict on purpose — a redaction that cannot be verified is not a redaction.

### F9/F10/F11 — medium — what else the document carries

Annotations (`/Contents`), stamps, XMP, `/Info` and embedded files were not
touched by the redaction. Two of these escaped even the check over the
bytes:

```
/Contents (Anmerkung VERTRAULICH GEHEIM)      → plain text, found by the check
/Contents <FEFF00470045004800450049004D…>      → UTF-16 as a hex string,
                                                  NOT found by the check
```

The stream of an embedded file carried the text as well and survived
completely:

```
DECOMPRESSED EMBEDDED FILE: b'Anhang: GEHEIMWORT Geheimtext'
```

All four checks reported success in that case.

**Remedy:** the engine scrubs annotations, form values and metadata, and
reports the places where the text still occurs — in plain text, as a hex
string, and as UTF-16 in both byte orders. A place that cannot be cleaned
(an embedded file) makes the redaction fail; the embedded file itself is
never deleted.

---

## 3. What the audit could **not** check

- The Rust code of the core was **not executed**: the auditors built their
  own C harness against the production bridge and reconstructed the behaviour
  of the core from the source. Every statement about the core above points to
  a line of code, not to a measurement of the core.
- The **OCR** was not available in the sandbox, so the check over the picture
  of the page was assessed by reading the code and by the pixel measurements
  of the engine.
- **Encrypted documents**, documents with a JBIG2 or CCITT image and
  documents with a signature were not tested. A signature is invalidated by a
  redaction in any case — MuPDF sets `doc->redacted = 1` — and the signature
  report of the core shows this.
- The **macOS and the Android application layer** were not tested; the
  auditors worked on the document level.
- The **interaction with other PDF tools** (Acrobat, Foxit, Okular) was not
  tested: the interoperability of a redaction is still open (M2 gate).

---

## 4. What the audit confirmed

The auditors could not break these:

- The text of the page is removed, the old content stream is collected
  (`do_garbage = 1`).
- Form XObjects are inlined and redacted.
- Invisible text (render mode 3) inside the region is removed.
- The pixels of an image under the region are **replaced**, not covered.
- The search index is re-indexed after the removal.
- The undo is refused, the redaction is irreversible.
- `NotChecked` cannot become `Passed`: the FFI reports an error.
- The mapping between the page space and the user space is correct
  (`pdf_set_annot_rect`), the crop box and the rotation are handled — with
  the exception of the box of the word, see F3/F4.

---

## 5. Overall statement

**At the audited state the guarantee of the redaction did not hold.** A user
could have been told that a document was redacted while a readable fragment,
a form value, an annotation, an embedded file or a Type 3 text was still in
the file — and, in the case of the failed verification, while the document had
already been changed without a way back.

The findings F1 to F7 and F9 to F11 are fixed in the state that follows this
report; the redaction is re-audited afterwards, independently
(`target/audit/redaction-recheck.md`).

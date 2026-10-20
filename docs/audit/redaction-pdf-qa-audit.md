# Audit of the redaction: the acceptance record and the PDF semantics

**Date:** 2026-10-20
**Audited state:** commit `cc564e6`
**Method:** an independent QA audit that did not take the acceptance record
for granted. Every statement was re-measured on the document itself, with an
own C harness against the production bridge and `mutool` 1.28.5 as the
independent reference.

This report answers the question: **do the proofs that exist carry the claim
"the redaction is safe"?**

---

## 1. Summary

The proofs that exist are honest — each of them proves exactly what it says.
But they prove it for a **narrower** case than the record claims, and three
statements of the record are stronger than the proofs behind them.

| # | Severity | Finding |
|---|----------|---------|
| Q1 | high | Criterion 12 ("the saved document no longer contains the removed text") is **not covered** for a document with an annotation or an embedded file. The audit measured a document where the text survived and all four checks reported success. |
| Q2 | high | Criterion 3 ("the removed text is no longer in the bytes of the file") holds for **plain text only**. A text that is stored UTF-16 encoded as a hex string is not found by the check. |
| Q3 | high | Criterion 16 ("the checkpoints of the recovery store are deleted with the redaction") describes the **wrong order**: the checkpoints are deleted *before* the verification, so a failed verification leaves neither the old document nor its recovery points. |
| Q4 | medium | The limit of the check over the bytes is not named in the record: it says "supplementary", but not that it only sees plain text. |
| Q5 | medium | Criterion 5 ("the redacted region is empty in the picture of the page") is proven for the box of the headline, not for a region that is only partly covered. |
| Q6 | low | The risks RISK-019, RISK-021, RISK-022 and RISK-023 are stated as "mitigated" — three of them were mitigated at the time of the audit only by the checks that Q1 and Q2 show to be incomplete. |

---

## 2. The findings in detail

### Q1 — high — criterion 12 is not covered

> | 12 | The saved document no longer contains the removed text | Host | same | **PASS** |

The end-to-end test uses a **scan**: a page with an invisible text layer over
an image. It has no annotation, no form field, no embedded file and no
metadata that carries the text. The criterion is therefore proven for that
fixture — and read as a general statement it is not covered.

**Measurement.** The auditor built a document with an annotation whose text
carries the secret and redacted the page:

```
/Contents (Anmerkung GEHEIMWORT)      → the annotation object is still in the
                                         redacted file, with its text
```

and one with an embedded file:

```
$ ./probe redact attachment.pdf attach_redacted.pdf 0 20000 34949 118000 54186
$ python3 -c "… zlib.decompress(raw) …"
DECOMPRESSED EMBEDDED FILE: b'Anhang: GEHEIMWORT Geheimtext'
```

The embedded file survived **completely**, and the text of the annotation
survived in the file. All four checks of the core reported success in that
case, so the FFI reported a verified redaction.

**Remedy:** the engine scrubs annotations, form values and metadata and
reports the places where the text still occurs; a place that cannot be
cleaned (an embedded file) makes the redaction fail. The criterion in the
record has to name the case it was measured on.

### Q2 — high — criterion 3 holds for plain text only

> | 3 | The removed text is no longer in the bytes of the file | Host | same | **PASS** |

The check compares the plain text of the file with the removed word. A PDF
stores a text either as a plain string or as a hex string; the hex form
encodes UTF-16, and the plain text comparison does not see it.

**Measurement:**

```
$ python3 -c "…"
ascii 'GEHEIMWORT' in file: -1
core byte check would find it: False
```

The word is in the file, encoded as `<FEFF00470045004800450049004D0057004F00520054>`,
and the check passes. A hex string is not the only way out: a base64 encoded
stream, an octal escape in a PDF string (`\053` is `(`) and a text that is
split over two objects are equally invisible to a plain text comparison.

**Remedy:** the engine reports the places where the text occurs, and it looks
at the decompressed streams and at the encodings as well.

### Q3 — high — criterion 16 describes the wrong order

> | 16 | The checkpoints of the recovery store are deleted with the redaction | Host | implementation + code review | **PASS** (code) |

The code deletes the checkpoints of the recovery store **inside**
`apply_redaction` — that is, before the verification runs. When the
verification fails, the document has already been changed, the checkpoints
that still held the old bytes are gone, and the caller receives an error that
says nothing about the state of the document.

The intent of the criterion is right — a checkpoint still holds the removed
content, so it has to go with the redaction. The **order** is what is wrong:
the checkpoints may only be deleted after the verification has succeeded.

**Remedy:** the core restores the state of the document when the
verification fails and deletes the checkpoints only after a successful
verification.

### Q4 — medium — the limit of the check over the bytes is not named

> | **File bytes** | The words are no longer present in the file as plain text | supplementary — a PDF may carry text in its own encoding, then this check does not fire, but the other three do |

The limit is named ("supplementary"), but not the one that matters: the
check only sees **plain text**. A PDF that carries the text hex encoded is
not "a PDF with its own encoding" in the sense of the sentence — the text is
perfectly readable, the check simply does not fire, and neither do the other
three checks, because the text is not on the page.

**Remedy:** the record names the limit explicitly, and the engine covers the
encodings.

### Q5 — medium — criterion 5 is proven for one region

> | 5 | The redacted region is empty in the **picture of the page** | Host | `the_redacted_region_is_empty_in_the_picture_of_the_page` | **PASS** (no black box that could hide surviving content) |

The test is a good one and it is honestly cited: it renders the page before
and after the redaction, counts the dark pixels in the box of the headline
and expects **zero** afterwards. A black box would produce thousands of dark
pixels, so the test really proves that nothing covers the region and that
the text is gone.

Its limit: the region is exactly the box of the headline. The test does not
cover a region that only takes part of a word — there a remainder can stay
in the picture without the test noticing.

**Remedy:** the core now checks the region itself: nothing may lie in the
redacted region any more.

### Q6 — low — the state of the risks

`docs/project/RISKS.md` states RISK-019, RISK-021, RISK-022 and RISK-023 as
mitigated. At the time of the audit, the mitigation of the risk "the removed
content stays in the file" consisted of the checks that Q1 and Q2 show to be
incomplete. The state was therefore too optimistic for the cases of an
annotation and of an encoded text.

**Remedy:** the risks are updated together with the record.

---

## 3. The proofs that hold

The audit confirmed the following without reservation:

- **Criterion 4** — the image pixels under the region are really **replaced**
  and not covered. The proof is strong: the marker inside the *uncompressed*
  image stream is gone from the file afterwards. A black rectangle could not
  achieve that. The auditor re-measured it on the produced file.
- **Criterion 5** — the picture of the page is empty in the redacted region
  (see Q5 for its limit).
- **Criterion 15** — an incompletely verified redaction is reported as an
  error across the FFI boundary (`RedactionUnverified`). `NotChecked` cannot
  become `Passed`.
- **Criterion 14** — a redaction that removes nothing is refused.
- **Criterion 9 to 11** — the end-to-end path across the FFI boundary works
  with the real engine: the invisible text layer of a scan is removed, the
  search no longer finds the text, the picture of the page is empty.
- The **coordinates** are consistent between the core, the FFI and the
  engine: the word boxes of the engine agree with the reference of
  `mutool draw -F stext`, and the region is applied at the right place
  (`pdf_set_annot_rect` works in the page space of the engine). The only
  exception is the box of a word on a turned page — see the security audit.

---

## 4. What the audit could **not** check

- The **OCR** was not available in the sandbox: the check over the picture of
  the page was assessed by reading the code and by the pixel measurements of
  the engine, not by a run of the OCR.
- The **Rust code of the core** was not executed by the auditor: the
  behaviour was reconstructed from the source. The findings that concern the
  core point to a line of code.
- **Encrypted documents**, JBIG2/CCITT images and documents with a signature
  were not tested. A redaction invalidates a signature in any case (MuPDF
  sets `doc->redacted = 1`); the signature report of the core shows this.
- The **macOS and Android application layer** and the **interoperability with
  other PDF tools** (Acrobat, Foxit, Okular) were not tested.

---

## 5. Overall statement

**The proofs that exist do not carry the claim "the redaction is safe"** — they
carry the claim "the redaction of a page of text works, and the checks report
what they measure". The difference matters: the record as it stands would
have let a user believe that a document had been cleaned, while an annotation
or an embedded file still carried the content.

The findings are remedied in the state that follows this report; the
acceptance record is rewritten accordingly, and the hardened redaction is
audited again independently.

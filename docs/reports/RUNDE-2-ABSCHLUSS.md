# Final Report — Round 2: M2.1 hardened and audited, M2.2 and M2.3 delivered

- **Branch:** `main`
- **Range:** `b501a39` … `f59c262` (the range before this report)
- **State:** M1 is implemented and verified on the host; M2 is in progress with
  three verified slices; the redaction was hardened after an independent
  adversarial audit and audited again.

---

## 1. What this round delivered

| # | What | Commit | State |
|---|---|---|---|
| 1 | **M2.1 hardened** — the redaction was audited adversarially and eleven findings were fixed | `17b6f58`, `123d7ac` | verified on the host |
| 2 | **M2.2 — the inspection of the links and the actions** of a document | `8d07725` | verified on the host |
| 3 | **M2.3 — the inspection of the attachments and the signatures** of a document | `cc564e6` | verified on the host |
| 4 | **The documentation in English** — the living documents, the acceptance records, the reports and the requirements | `b501a39` | — |
| 5 | **The acceptance record of the hardened redaction**, with the two audits | `f59c262` | — |

The test suite grew from 161 to **175 tests in 24 suites**, without a single
failure. The gate `scripts/validate-repo.sh` is green.

### 1.1 The last finding of the second audit: the same word on another page

The second audit went further than the first one. It built a document in
which the same word stands on two pages and redacted only one of them. The
result: the search reported "clean", the check over the bytes reported
"clean" — and the word was still in the document, on the other page.

The promise of a redaction is about the **marked region**: the user marked a
region, not the word in the whole document. Refusing the redaction because
the word occurs somewhere else would make every document with a word that
occurs twice impossible to redact. But hiding that from the user is not
acceptable either — somebody who believes that "VERTRAULICH" is gone from the
document while it is still on page 5 has been misled.

The redaction now **names** the places where the word still stands
(`remaining_occurrences` in the report, across the FFI boundary as well). A
remainder in the bytes of the file **without** a known place — an old version
of the file, an unreachable object — still stops the redaction; that
difference is proved by a test of its own.

## 2. The most important insight of the round

**A security-critical function that is only checked by the team that wrote it
is not checked.**

The redaction was implemented, verified with four independent checks, given an
acceptance record and a gate — and it did not work. The independent audit
found, with its own harness and `mutool` as the reference:

- On a page turned by 90 degrees the **word box was inverted**: the redaction
  failed with status 7. On a page turned by 180 degrees it removed the first
  and the last character only — and reported success.
- A redaction that covered **half a word** left a readable fragment behind and
  was reported as successful.
- The text of an **annotation**, the value of a **form field**, the **metadata**
  and the **XMP packet** survived the redaction. The check over the bytes only
  compared plain text, so a text stored as UTF-16 or as the hex string of the
  UTF-16 bytes was not found at all.
- A redaction whose verification **failed** left the document changed for
  good: the removal happened first, the recovery checkpoints were deleted, and
  the caller received an error that said nothing about the state of the
  document. The user could hold a changed document and believe that nothing
  had happened.
- The text of a **Type 3 font** cannot be removed by MuPDF 1.28.5 at all
  (`recurse = 0`). The auditors measured 8322 dark pixels before and after
  the redaction. (The second audit could not reproduce this: the text of a
  Type 3 font was removed completely in the construction it built — the
  claim is withdrawn for that construction.)

The fix is not a patch on one line: the redaction now checks the **whole
region** (nothing may lie in it any more), compares **fragments**, searches
the **whole document** for the removed text in every encoding, cleans the
places that can be cleaned and reports the places that cannot, and **rolls
back** a redaction that cannot be verified.

The lesson is written down in `docs/GETTING-STARTED.md` — nine traps now,
each of which has already bitten once.

## 3. The evidence

### 3.1 The hardened redaction (`123d7ac`)

| Property | Proof |
|---|---|
| The word boxes of a turned page are normalised | `the_word_boxes_of_a_turned_page_are_normalised`, `a_redaction_removes_the_word_on_a_turned_page` |
| A fragment of a removed word is a survivor | `a_fragment_of_a_removed_word_is_recognised` |
| Nothing may lie in the redacted region any more | `a_word_that_stays_in_the_region_is_a_residue` |
| A redaction that covers only a part of a word is not reported as successful | `a_redaction_that_only_covers_a_part_of_a_word_is_not_reported_as_successful` |
| The text of an annotation and of a form field is removed | `the_text_of_an_annotation_and_of_a_form_field_is_emptied` |
| A hex encoded word is found | `a_word_that_is_hex_encoded_is_found` |
| A redaction that cannot be verified leaves the document unchanged | `a_redaction_that_cannot_be_verified_leaves_the_document_unchanged`, `a_redaction_that_cannot_be_verified_leaves_the_document_and_the_checkpoints_alone` |
| The same word on another page is named in the report, a remainder without a place still stops the redaction | `the_same_word_on_another_page_is_named_in_the_report`, `a_remainder_without_a_place_still_stops_the_redaction` |
| The checkpoints of the recovery store are deleted after a successful redaction | `a_successful_redaction_deletes_the_checkpoints_of_the_recovery_store` |

### 3.2 The two audits

They are part of the documentation now: `docs/audit/redaction-security-audit.md`
and `docs/audit/redaction-pdf-qa-audit.md`. They name the measurements, the
commands and the limits of what could be checked.

### 3.3 The gates

| Gate | What it enforces |
|---|---|
| M2.1 | the removal instead of the covering, the replacement of the image pixels, no black box, the picture check, `RedactionUnverified` at the FFI boundary, the engine tests |
| M2.1 (hardening) | the normalised word boxes, the fragment and the region check, the search for the places of the document, the cleaning of the annotations, the form values and the metadata, the rollback, the seven new proofs |
| M2.2 | the engine reports the actions, the core decides the risk, the FFI function, the tests |
| M2.3 | the engine reports the attachments and the signatures, the core decides the risk, the tests |

## 4. Architecture and platform

- **The separation of the layers held.** The engine reports the facts — the
  places of the document that carry a word, the kinds of the actions, the
  embedded files, the byte range of a signature. The core decides what they
  mean. The engine contains no policy, the core no PDF specification.
- **No new dependency.** The new C bridges
  (`mupdf_scrub_bridge.c`, `mupdf_occurrences_bridge.c`) use the MuPDF that
  was already there, and the two small headers (`mupdf_text_search.h`,
  `mupdf_serialize.h`) keep the files below the size limit.
- **The public API of the existing types is unchanged.** The new types
  (`PdfTextOccurrence`, `PdfScrubReport`) and the new trait methods are
  additive; the generated bindings did not change.
- **Platform compatibility:** the new bridges are built for Android as well.
  Every native library stays 16 KB page compatible.

## 5. What is still open

| Item | State | Why |
|---|---|---|
| The macOS interface of all three functions | **NOT RUN** | no macOS host |
| The behaviour on a device (Android, macOS) | **NOT RUN** | no device, no `/dev/kvm` |
| The applications do not call the functions yet | **open** | the redaction now has a caller in the macOS client and in the Android bridge, and both check the report themselves. The **step of the interface** — the user marks the region and sees the report — belongs to the macOS slice of M2 |
| Interoperability with Acrobat/Foxit/Okular | **NOT RUN** | M2 gate, open for all M2 functions |
| A document whose text is drawn with a Type 3 font | **NOT RUN** (residual) | The first audit assumed that MuPDF 1.28.5 cannot remove such a text. The second audit **could not reproduce** that: the text of a Type 3 font was removed completely in the construction it built. The claim is withdrawn for that construction; the others were not checked exhaustively. Should a construction survive, the check over the region sees it and the redaction is refused |
| A secret in Base64 | **NOT RUN** | the search looks for plain text, for UTF-16 in both byte orders and for the hex string of the UTF-16 bytes |
| A word whose parts lie apart from each other | **NOT RUN** | the region takes every word it touches, but two pieces of a word that lie far apart are not one word in the text layer. The full sanitising (`SAN-001`) is the place for that |
| The full sanitize of a document (metadata, hidden content, attachments, actions, comments, form remnants) | **open** | `SAN-001`, the separate feature of the roadmap. The redaction now reports the places it cannot clean, instead of staying silent about them |
| The licence decision | **open** | the decision paper `docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` is waiting for the choice of the variant |
| AddressSanitizer | **NOT RUN** | only with a nightly toolchain (`-Zbuild-std`); the substitute is the valgrind memcheck of the difficult corpus |

## 6. Outlook

The next slice of M2 is the **annotation function with the jurist palette**
(`ANNOT-001`): the comments of a document, editable, with the palette of the
profession. The pattern of the last two slices applies again — the engine
reports the facts, the core decides, the FFI boundary makes the report
available, and the interface is the macOS part that cannot be built here.

The second round of the audit runs against the hardened redaction. Whatever it
finds goes back into the code, and the result is written down in the same way
as the first round: with the measurement, not with the assurance.

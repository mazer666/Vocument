# The hardening of the verification of the redaction

- **Date:** 2026-10-21
- **Subject:** the verification of the redaction in the core
  (`core/vocument-core/src/redaction.rs`), the walk over the places of the
  document (`engines/pdf-mupdf/csrc/mupdf_walk.h`), the search for the places
  that carry the text (`engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c`),
  the cleaning (`engines/pdf-mupdf/csrc/mupdf_scrub_bridge.c`) and the reading
  of the text of a content stream
  (`engines/pdf-mupdf/csrc/mupdf_text_search.h`).
- **Why:** the fifth round of the red team left three findings open, and the
  owner named the class of the problem: *a limited list of places, a limit of
  the recursion, a failed inspection of an object and an insufficient reading
  of the text must not be able to produce a false success of the
  verification.*

---

## 1. The four gaps that were closed

| # | The gap | What it allowed | What was done |
|---|---|---|---|
| **G-1** | The **limit of the depth** of the walk (48) stopped the walk **silently**. Everything below it was never examined, and the report said nothing | A secret below the limit was never seen, and the redaction reported `verified = true` | The walk **writes down** every place it could not examine. The search for the places reports them as `uninspected`, the core names them, and the redaction is **refused** |
| **G-2** | An **object that could not be read** was skipped **silently** (`pdf_cache_object` returned nothing) | The same: a secret in an unreadable object was never seen | The same: the object number and the reason are written down and the redaction is refused |
| **G-3** | The **encryption dictionary** was not examined at all — the walk skipped the key `/Encrypt` and refused to descend into the dictionary | A secret that stands in the `/O` or `/U` of an encrypted document was never seen. The check over the bytes of the file does not look at words of one to three characters, so such a secret could survive with `verified = true` | The dictionary is **examined** again (the check has to see it) and **never cleaned** (emptying a key destroys the document). A secret in a key is a place that carries the text: the redaction refuses and names it |
| **G-4** | The verification ran **behind the garbage collection of the writer**: the objects that nothing reached any more were removed before the check ran, and the places that carried the text were therefore never examined. The safety of the redaction rested on the writer of the engine and not on the check of the product | The proof was not the product's own | The product examines the document **before** the removal and keeps the list of the places that carried the text (`places_before_removal`). The check after the removal compares with that list, and the report shows it to the user |

**R5-3** — the finding of the fifth audit that the verification ran behind the
garbage collection — is therefore **closed** for the part that the product can
close: the product knows now which places carried the text, and it says so in
the report. What remains open is the *order* inside the engine: the cleaning
still runs after the removal, and the writer of the engine still removes the
objects that nothing reaches any more. The product no longer *depends* on that
removal — it has its own reference.

## 2. What was done in the code

### 2.1 The walk writes down what it could not examine

`vocument_walk_t` carries a list of places that the walk could not look into
(`vocument_walk_note_uninspected`). Three places write into it:

* the limit of the depth (`mupdf_walk.h`, `vocument_walk_object`),
* an object of the cross-reference table that cannot be read
  (`vocument_walk_xref`),
* an object that cannot be addressed.

The search for the places (`mupdf_occurrences_bridge.c`) turns every line into
a report entry of the kind `uninspected`. The core puts those places into
`uninspected_places`, names them in `not_checked_reasons`, and
`RedactionReport::is_verified()` is **false** when the list is not empty. The
boundary therefore returns the error `RedactionUnverified` — the redaction
never claims a success it cannot prove.

### 2.2 The encryption dictionary is examined and never cleaned

The rule of the fourth round — the keys of the document are not the text of
the user — stays. But the rule was implemented in the wrong place: the walk
skipped the dictionary, so the *check* could not see it either. Now:

* the walk descends into the dictionary (the key `/Encrypt` of the trailer is
  walked as well),
* the **cleaning** refuses the values of a dictionary that is the encryption
  dictionary (`vocument_scrub_place` → `vocument_walk_is_security_dictionary`),
* the **check** sees the values → a secret in a key is a place that carries
  the text → the redaction refuses and names the place.

The document stays encrypted and readable in every case.

### 2.3 The text of a content stream is read

The comparison of a place with a word was a comparison of **bytes**. A writer
may split a word over several show operations: `(GE) (HEIM) Tj` puts
"GEHEIM" on the page, but the bytes of the stream do not carry the word in
one piece. `vocument_text_run_carries_word` reads the **text** of the stream:
it collects the string literals and joins them, as long as no operator moved
the writing position. The comparison of the joined text knows every encoding
of a PDF — plain text, UTF-16 in either byte order and the hex string of the
UTF-16 bytes.

Two words that stand next to each other belong to two runs, because a
positioning operator stands between them: they are not joined.

### 2.4 The reference of the verification

The core runs the search for the places **before** the removal and keeps the
result. If the search fails, the redaction is refused **before** the document
is changed at all — there is nothing to roll back.

The list is in the report (`places_before_removal`, and in the report over the
boundary `FfiRedactionReport.places_before_removal`), so that the user sees
where the text stood.

## 3. The proof

| The claim | The proof |
|---|---|
| A place below the limit of the depth is named, and the redaction refuses | `a_place_that_the_walk_could_not_examine_is_named` (engine), `a_place_beyond_the_depth_of_the_walk_makes_the_redaction_refuse` (the whole chain) |
| The keys of the document are never emptied | `the_keys_of_the_security_dictionary_are_not_emptied` (engine), `the_keys_of_the_security_dictionary_are_never_emptied` (the whole chain) |
| The keys of the document are seen by the check | `the_security_dictionary_is_a_place_that_carries_the_text` (engine) |
| A document with an indirect encryption dictionary stays encrypted and readable | `a_document_with_an_indirect_encryption_dictionary_stays_encrypted` (the whole chain) |
| A word that is split over several show operations is found | `a_word_that_is_split_over_several_show_operations_is_found` (engine) |
| A word that is split **and** stored as UTF-16 is found | `a_word_that_is_split_and_stored_as_utf16_is_found` (engine) |
| Two words that stand next to each other are not one word | `two_words_that_stand_next_to_each_other_are_not_one_word` (engine) |
| The sweep over the objects grows linearly | `the_sweep_over_the_objects_grows_linearly_and_not_quadratically` (the whole chain) |

## 4. The limits of this round

1. **The limit of the depth of the walk is still 48.** A document that nests
   deeper than that is **refused** — it is not redacted. The limit is a
   safety measure, and the refusal is the honest answer to it; a walk without
   a limit would not terminate on a document that refers to itself.
2. **The reference is a list of places, not a proof of the bytes.** The check
   over the bytes of the file still looks for words of four characters and
   more. A word of one to three characters is covered by the comparison of
   the places — and by nothing else.
3. **The text of a content stream is read with a scanner of the product, not
   with the interpreter of MuPDF.** A word that is split over show operations
   **and** that is placed with a positioning operator between the pieces
   (two words on the page that read as one) is not seen as one word. The
   position of the pieces is not examined.
4. **The OCR still cannot prove that a word is gone from the pixels of an
   image.** The check of the rendered page compares the words before and
   after; a word that the OCR cannot read is not in either list.
5. **The order inside the engine is unchanged**: the cleaning runs after the
   removal, and the writer of the engine removes the objects that nothing
   reaches any more. The product does not depend on that any more, but the
   engine could be made to clean first — that is the recommendation of the
   auditor for the next round.
6. **The Swift application was not tested at runtime**: there is no Swift
   toolchain and no macOS host in this environment. The call of the redaction
   from the shell is `NOT RUN`.

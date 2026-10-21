# Red Team IV — the document level: the new rules of the redaction under attack

- **Date:** 2026-10-21
- **Checked stand:** `/workspace/vocument`, branch `main`, **working tree with the uncommitted
  changes of the third round** (`59652e8` + working tree). The attacked files were fingerprinted
  before and after the measurements and did not change during the run (section 0 of the
  measurement log). One unrelated file (`platform/vocument-ffi/src/lib.rs`) was touched by a
  concurrent agent at 08:22; the library was rebuilt against the current tree and every finding
  below was reproduced with the rebuilt library.
- **Method:** a purpose-built harness in the plan workspace drove the **real** chain
  (core → MuPDF → exported UniFFI functions) over 20 hand-built PDF files, and a C probe drove
  the **real** walk header (`engines/pdf-mupdf/csrc/mupdf_walk.h`, copied unmodified except for two
  `#ifdef` switches) against the **real** MuPDF 1.28.5. No stub of the product, no simulation.
- **Raw evidence:** `messung-r4.txt` (774 lines, every command and every number),
  `harness/` (the harness, the probe, the PDF generators), `pdfs/` (the attack files),
  `out/` (the redacted output documents) — all in this working directory.
- **Note on the paths in this report (added when the report was taken into the repository):**
  the report and the raw protocol were written in the working directory of the release team
  (`/workspace/.mavis/plans/plan_25670a94/outputs/redteam-redaction-round-four/`). The two
  files that belong to the project live in the repository now:
  `docs/audit/redteam-round-four-document-level.md` (this report) and
  `docs/audit/redteam-round-four-measurements.txt` (the raw protocol, 774 lines). The harness,
  the twenty attack files and the redacted output documents stay in the working directory of
  the release team — they are the tools of the attacker, not a part of the product.
- **The state of the findings:** the report was written against the stand `59652e8` plus the
  uncommitted changes of the third round. Every finding was closed afterwards; the closure and
  its proof are in `docs/acceptance/M2.1-ACCEPTANCE.md` §5.4.

---

## 1. The answer in one sentence

**Yes — the redaction of the checked stand can be made to report `verified = true` while the
removed word is still in the document, in three different ways.** The new sweep over the
cross-reference table is the centre of it: it hands the objects of the table out as *direct*
objects, and `pdf_is_stream()` answers "no" for a direct object — so the sweep never looks at the
content of a stream, only at its dictionary.

| # | Severity | Finding | Evidence |
|---|---|---|---|
| R4-1 | **critical** | The sweep over the cross-reference table does not see the **content** of a stream. A copy of the removed word in the content stream of a form XObject survives a redaction that reports `verified = true` with all four checks `Passed`. | `attack_form_page1.pdf`, sections 1, 2, 3, 5 of the log |
| R4-2 | **high** | The **file identifier** (`/ID` in the trailer) is never visited by the walk. The removed word survives in it as a UTF-16 string; the redaction reports `verified = true`. | `attack_id_utf16.pdf`, sections 1, 2 |
| R4-3 | **high** | The new rule "an unreadable place makes the verification incomplete" is implemented with a **path string heuristic** (`strstr(path, "EmbeddedFiles")`). The same attached archive is refused under `/Names/EmbeddedFiles` and accepted under `/AF` — with `verified = true` and the secret still inside the archive. | `attack_zip_embeddedfiles.pdf` vs. `attack_zip_af_first.pdf`, sections 1, 2 |
| R4-4 | **medium** | The new walk of `/Encrypt` lets the scrub empty the encryption dictionary. Redacting a one-letter word in an encrypted document destroys the encryption and the redaction dies with a misleading error ("PDF requires an open password"). | `encrypted_oneletter.pdf`, sections 1, 7 |
| R4-5 | **medium** | The sweep is **quadratic** in the number of objects (`vocument_walk_already_seen` scans a linear list for every entry). A document with 32,000 objects needs 0.83 s per walk (without the sweep: 0.0003 s); one redaction of a 4 MB file with 32,000 annotations takes 14.0 s. | section 8, 9 |
| R4-6 | **medium** | `RecoveryContainer::create_private_root` checks the **mode** of the recovery directory but not its **owner**, and the FFI ignores its error. A directory prepared by a stranger is accepted ("Ok"), and the recovery data — including a checkpoint that holds the document in plain text — is written into it. | section 10 |
| R4-7 | low | The sweep reaches *unreachable* objects (which MuPDF's garbage collection has already removed before the sweep runs) and therefore cannot find the classic "orphaned object" leak in the product flow. Its only productive reach is objects that hang below the **dictionary of a stream** — and there it is exactly the blind spot of R4-1. | sections 3, 4 |

---

## 2. R4-1 (critical) — the sweep does not look into the streams

### 2.1 What the product does

The redaction removes the content of the page, then the *scrub* empties every string of the
document that carries the word, then the *search* reports every place that still carries it
(`engines/pdf-mupdf/csrc/mupdf_scrub_bridge.c`, `mupdf_occurrences_bridge.c`). Both walk the
document with `mupdf_walk.h`. The new part is `vocument_walk_xref`, which walks the objects of the
cross-reference table as well — "including the ones that nothing refers to".

The objects of the table are handed out as `entry->obj`. In MuPDF 1.28.5 that object is the
**parsed, direct** dictionary of the object — it is not an indirect reference. And `pdf_is_stream()`
in MuPDF 1.28.5 is:

```c
int pdf_is_stream(pdf_context *ctx, pdf_obj *ref)
{
	pdf_document *doc = pdf_get_bound_document(ctx, ref);
	if (doc)
		return pdf_obj_num_is_stream(ctx, doc, pdf_to_num(ctx, ref));   /* pdf_to_num(direct) == 0 */
	return 0;
}
```

`pdf_to_num()` of a direct object is `0`, and object `0` is the free head of the cross-reference
table — so `pdf_is_stream()` answers **"no"** for every object that the sweep hands out.

The consequence: `vocument_walk_object()` treats such an object as a plain dictionary. It walks
the *entries* of the dictionary and never offers the *content* of the stream. The occurrences
bridge decides with the same predicate (`if (!pdf_is_stream(ctx, item->object)) return 1;`) — so
the content of such a stream is never searched either.

### 2.2 The measurement

The probe lists what the sweep sees (`probe list`, section 3 of the log). The inner form XObject
of `attack_form_page1.pdf` is object 7; the sweep reaches it — but only its dictionary:

```
  ANDERES /Object[7]/Type
  ANDERES /Object[7]/Subtype
  ANDERES /Object[7]/BBox[0]
  ...
  ANDERES /Object[7]/Filter
  ANDERES /Object[7]/Length
  -> angebotene Stellen: 27, Zeichenketten: 0, Stroeme: 3, andere: 24, Doppelbesuche: 0
```

The stream itself is never offered: there is no `STROM /Object[7]` line. The counterfactual makes
the cause visible (`probe sweep_fixed`, section 5 of the log): the same objects, wrapped in an
indirect reference, are recognised as streams and their content is searched:

```
    [4] als indirekte Referenz: strom=1 (direkt: strom=0)
        Inhalt des Stromes traegt das Wort: JA (38 Bytes)
    [7] als indirekte Referenz: strom=1 (direkt: strom=0)
        Inhalt des Stromes traegt das Wort: JA (38 Bytes)
```

### 2.3 The attack file

`attack_form_page1.pdf` (generated by `harness/mkform.py`):

```
1 0 obj  << /Type /Catalog /Pages 2 0 R >>
2 0 obj  << /Type /Pages /Kids [3 0 R 8 0 R] /Count 2 >>
3 0 obj  << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]
           /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>      ← page 0: the word
4 0 obj  << /Length 38 >> stream  BT /F1 24 Tf 72 700 Td (GEHEIM) Tj ET
5 0 obj  << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>
6 0 obj  << /Type /XObject /Subtype /Form /BBox [0 0 595 842]
           /Resources << /Font << /F1 5 0 R >> /XObject << /Fm1 7 0 R >> >> /Length 27 >>
           stream  q 1 0 0 1 0 0 cm /Fm1 Do Q
7 0 obj  << /Type /XObject /Subtype /Form /BBox [0 0 595 842]
           /Resources << /Font << /F1 5 0 R >> >> /Filter /FlateDecode /Length 46 >>
           stream  BT /F1 24 Tf 72 700 Td (GEHEIM) Tj ET     ← the copy (compressed)
8 0 obj  << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842]
           /Resources << /Font << /F1 5 0 R >> /XObject << /Fm0 6 0 R >> >> /Contents 9 0 R >>
9 0 obj  << /Length 42 >> stream  BT /F1 12 Tf 72 100 Td (Seite zwei) Tj ET
```

The copy of the word is in the content stream of object 7. Object 7 is reachable **only** through
the `/Resources` dictionary of the form XObject 6 — and the walk never descends into the
dictionary of a stream, so the ordinary walk from the root never sees object 7 at all. The sweep
does see it — as a dictionary (section 2.2).

### 2.4 The result

```
=== attack_form_page1 ===
ERGEBNIS: Ok
  verified                = true
  text_layer_clean        = Passed
  search_index_clean      = Passed
  rendered_page_clean     = Passed
  file_bytes_clean        = Passed
  removed_words           = ["GEHEIM"]
  remaining_occurrences   = []
  uninspected_places      = []
  not_checked_reasons     = []
  bytes                   = 1253 Bytes
  geheimnis im Klartext   = false
  geheimnis als UTF-16hex = false
```

All four checks pass, the report says `verified = true` — and the redacted document still
contains the word. The output (`out/attack_form_page1.pdf`) still has object 7:

```
7 0 obj
<</Type/XObject/Subtype/Form/BBox[0 0 595 842]/Resources<</Font<</F1 5 0 R>>>>/Filter/FlateDecode/Length 46>>
stream
x..s.Q.w3T02Q.IS07R070P.IQ.pw.p.....Rp....  endstream
endobj
```

Unpacked, the stream is:

```
b'BT /F1 24 Tf 72 700 Td (GEHEIM) Tj ET\n'
GEHEIM im entpackten Strom: True
```

Why the other three checks do not see it:

- **text layer / search**: the copy is in a form XObject that no page draws — it is not part of
  the text of any page.
- **file bytes**: the content is Flate-compressed, so the plain word is not in the raw bytes; the
  byte check is a *supplementary* signal by design.
- **rendered page**: the copy is not drawn on any page.

**Fix.** The sweep must hand out the objects the way the rest of the walk sees them, i.e. as
indirect references (`pdf_new_indirect(ctx, pdf_doc, num, 0)`), or the stream test must be done by
object number (`pdf_obj_num_is_stream(ctx, pdf_doc, num)`). With the indirect wrapper the probe
already finds the copy (section 5 of the log) and the redaction would be refused.

---

## 3. R4-2 (high) — the file identifier is not part of the walk

`vocument_walk_document()` walks the pages, `/Root`, `/Info` and (new) `/Encrypt` — and then the
cross-reference table. The **trailer** itself is never walked, and the trailer holds `/ID`, the
file identifier of the document. `/ID` is not an object of the table, so the sweep cannot reach it
either.

`attack_id_utf16.pdf` puts the word into the file identifier, encoded as UTF-16 (so that the
supplementary byte check does not see it):

```
trailer
<</Size 8 /Root 1 0 R /ID[(<FEFF00470045004800450049004D>)<00112233445566778899AABBCCDDEEFF>]>>
```

The redaction of page 0:

```
=== attack_id_utf16 ===
ERGEBNIS: Ok
  verified                = true
  ... alle vier Pruefungen Passed ...
  remaining_occurrences   = []
  geheimnis im Klartext   = false
  geheimnis als UTF-16hex = true
```

and the redacted document (`out/attack_id_utf16.pdf`) ends with:

```
trailer
<</Size 8/Root 1 0 R/ID[(<FEFF00470045004800450049004D>)<1733B34D648AD24E3A00982739933774>]>>
startxref
288
%%EOF
```

`GEHEIM` is still in the document — as the file identifier, readable by every tool that reads the
trailer.

**Variant with plain text.** `attack_id_plain.pdf` puts the word into `/ID` as a literal string and
also on a second page. There the byte check is switched off anyway (the known weakness D-2 of the
third round: `if haystack.contains(&needle) && remaining_occurrences.is_empty()`), so the plain
copy survives as well — measured: `geheimnis im Klartext = true`, `verified = true`.

**Fix.** The walk has to include the trailer (`/ID`, and everything else the trailer holds), or the
occurrences bridge has to look at the trailer explicitly.

---

## 4. R4-3 (high) — "unreadable place" is a path string, not a property

The new rule: a place that could not be read makes the verification incomplete. The
implementation (`mupdf_occurrences_bridge.c`):

```c
if (!item->is_page_content &&
    vocument_buffer_is_container(data, len) &&
    strstr(item->path, "EmbeddedFiles") != NULL) {
    vocument_report_line(state->report, "uninspected", item->page_index, item->path);
}
```

A container is only reported as *uninspected* when the **path** of the place contains the string
`EmbeddedFiles`. A container that hangs somewhere else is not recognised as a container at all: it
is searched for the word in its **compressed** bytes (where the word is not to be seen) and then
silently dropped.

The two files differ only in the place where the archive hangs:

- `attack_zip_embeddedfiles.pdf`: `/Root /Names /EmbeddedFiles /Names[1] /EF /F`
- `attack_zip_af_first.pdf`: the **same** file specification, but reachable through
  `/Root /AF[0] /EF /F` (associated files) — and the catalog lists `/AF` **before** `/Names`, so
  the walk reaches the archive by that path first.

Measured:

```
=== attack_zip_embeddedfiles ===
ERGEBNIS: Fehler
  fehler: the redaction could not be verified: the redaction could not be verified: the document
  could not be read completely: 1 place(s) could not be looked into, among them
  /Root/Names/EmbeddedFiles/Names[1]/EF/F; the words that were removed: GEHEIM; this place could
  not be read: /Root/Names/EmbeddedFiles/Names[1]/EF/F (page -1, uninspected)
```

```
=== attack_zip_af_first ===
ERGEBNIS: Ok
  verified                = true
  ... alle vier Pruefungen Passed ...
  uninspected_places      = []
  not_checked_reasons     = []
```

and the redacted document still carries the archive with the secret inside:

```
=== out/attack_zip_af_first.pdf (921 Bytes) ===
  *** Strom 0 ist ein ZIP-Archiv (ungepackt)
        Archivdatei 'notiz.txt': Geheimnis darin: True
  Objektarten : {'Catalog': 1, 'Pages': 1, 'Page': 1, 'EmbeddedFile': 1, 'Filespec': 1}
```

**This is the answer to the question of the round**: yes, there is a way to get `verified = true`
although a place of the document could not be read. The rule exists, but it is bound to a path
string instead of to the property of the place.

**Fix.** A place is unreadable when its content is a container — wherever it hangs. The decision
must not depend on the path; at the very least the path test must not be the only test.

---

## 5. R4-4 (medium) — the new `/Encrypt` walk lets the scrub touch the encryption

The new code walks the encryption dictionary of the trailer:

```c
vocument_walk_object(walk, pdf_dict_get(ctx, trailer, PDF_NAME(Encrypt)), ..., "/Encrypt");
```

The strings of that dictionary (`/O`, `/U`, 32 random bytes each) are now offered to the scrub,
and the scrub empties **every** string that carries the redacted word. A word of one letter
matches a random byte with a probability of about 12 % per string.

Measured with `encrypted_oneletter.pdf` (an AES-128 document whose `/U` happens to contain the
byte `0x78` = `x`):

```
### MIT /Encrypt-Walk
  STRING  /Encrypt/U   len=32 traegt=JA  -> geleert
  -> angebotene Stellen: 25, Zeichenketten: 2, ...
### OHNE /Encrypt-Walk
  -> angebotene Stellen: 12, Zeichenketten: 0, ...
```

The product run:

```
=== encrypted_oneletter ===
warning: encryption password key too short (0)
format error: aes padding out of range: 55
ERGEBNIS: Fehler
  fehler: PDF requires an open password
```

The encryption dictionary is destroyed, the document cannot be decrypted any more, and the
redaction dies with a message that blames the document ("PDF requires an open password") instead
of naming the cause. The rollback works (the bytes are unchanged, 970 Bytes, `/U` intact), so no
data is lost — but a redaction of a single letter in an encrypted document is impossible, and the
message is misleading.

(The redaction of a multi-letter word in an encrypted document works and keeps the encryption:
`encrypted_aes128.pdf` → `verified = true`, the output still carries `/Encrypt`. The walk of
`/Encrypt` is not the reason the encrypted document stays encrypted.)

**Fix.** The encryption dictionary must not be emptied by the scrub; its strings are keys, not
text of the user. The `VOCUMENT_PROTECTED_KEYS` list in the scrub is the right place for it, or
the walk of `/Encrypt` should be dropped altogether.

---

## 6. R4-5 (medium) — the sweep is quadratic

`vocument_walk_already_seen()` scans a **linear** array for every entry of the cross-reference
table, and `vocument_walk_mark()` appends to it. The sweep is therefore O(n²) in the number of
objects; the walk from the root was O(reachable objects).

Measured with the probe (3 walks per measurement, `probe timing` / `probe_noxref timing`):

| objects | with the sweep | without the sweep |
|---:|---:|---:|
| 1,000 | 0.0116 s | 0.0003 s |
| 4,000 | 0.0517 s | 0.0003 s |
| 16,000 | 0.3170 s | 0.0003 s |
| 32,000 | 0.8340 s | 0.0003 s |

The cost of one walk grows by a factor of 72 between 1,000 and 32,000 objects — 32 times the
objects, 72 times the time.

At the product level (a document with 32,000 *reachable* annotations, so that the garbage
collection keeps them): **one redaction needs 14.0 s** (`live_32000.pdf`, 4.2 MB), of which the two
walks (scrub + occurrences) account for ~1.7 s. A document with 100,000 objects would need ~8 s
per walk.

**Fix.** A hash set (or a bitmap over the object numbers) instead of a linear list.

---

## 7. R4-6 (medium) — the recovery directory: the mode is checked, the owner is not

`RecoveryContainer::create_private_root()` makes the directory private and then refuses a
directory that is writable for others. It never looks at **who owns the directory**.

Measured (`recprobe create_private_root`, section 10 A of the log):

```
### A: das Wurzelverzeichnis gehoert einem fremden Benutzer (uid 1000), Modus 0700
drwx------ 2 node node 4096 .../vocument_recovery
create_private_root(.../vocument_recovery)
  Ergebnis: Ok (die Pruefung hat angenommen)
  danach: 700 uid=1000
```

The directory belongs to `node` (uid 1000) and the check says **Ok**. The application then writes
its recovery data into it — including a checkpoint, which holds the document in plain text
(section 10 B):

```
  DIR    700 uid=0   .../vocument_recovery/doc_70ea4ce5d13588ef
  FILE   600 uid=0   .../doc_70ea4ce5d13588ef/checkpoints/checkpoint_rev_10.pdf
  FILE   600 uid=0   .../doc_70ea4ce5d13588ef/journal.json
```

The files are 0600, but the **directory** belongs to the stranger: the stranger may delete and
replace the whole container (the journal that is replayed after a crash, the manifest that names
the recoverable document) — the check that was supposed to prevent exactly this does not.

The second half of the question — "is the directory checked **before** the write?" — is answered
by section 10 C: the check runs **once**, when the session registry is created, and its error is
only printed on stderr; the store is used anyway:

```
vocument: the recovery directory could not be made private: I/O error in recovery container:
Operation not permitted (os error 1). The crash recovery of the documents is not protected in
this run.
```

The refusal is not a refusal: the application continues with an unprotected recovery store, and the
only sign is a line on stderr that a graphical application shows nobody. (The later writes fail
in this particular case, because `ensure_container_dir()` tries to make the directory private
again — the recovery simply does not work any more. That is visible in the log as ten
`the state of the document could not be secured for the crash recovery` lines.)

**Fix.** `create_private_root()` has to check the owner (on Unix: `st_uid == geteuid()`), and the
caller in `platform/vocument-ffi/src/lib.rs` has to treat the error as a failure — or at least has
to refuse to use the store, not to print a line and carry on.

---

## 8. R4-7 (low) — what the sweep really buys, and what it does not

**It does not find the classic orphan leak in the product flow.** The redaction saves the document
with `opts.do_garbage = 1` before the scrub and the search run
(`engines/pdf-mupdf/csrc/mupdf_serialize.h`), and MuPDF's garbage collection removes every object
that is not reachable from the trailer. The orphan is therefore gone before the sweep sees it.
Measured with `probe gc` (load + save with garbage collection, exactly what the redaction does):

```
  -> gespeichert (mit Muellerabfuhr): out/probe_gc_orphan.pdf (2487 Bytes)
```

and the object list of the result contains no annotation any more
(`Objektarten: {'Catalog': 1, 'Pages': 1, 'Page': 1, 'Font': 1}`).

On the *unreduced* file the sweep does reach the orphans — the A/B comparison of section 3 of the
log:

```
### MIT Sweep          -> angebotene Stellen: 22, Zeichenketten: 1 (geleert: /Object[98]/ActualText)
### OHNE Sweep         -> angebotene Stellen: 13, Zeichenketten: 0
```

**It does not visit an object twice.** Measured by comparing the object pointers of all offered
places (the shared static names of MuPDF — `PDF_NAME(Annot)` and friends — excluded, they are one
object for the whole file):

```
attack_form_page1:           Doppelbesuche: 0
attack_objstm_xrefstream:    Doppelbesuche: 0
attack_orphan_chain:         Doppelbesuche: 0
live_500 (500 Annotationen):  Doppelbesuche: 0
```

**It does not destroy an object that MuPDF needs.** The object stream and the cross-reference
stream are walked as dictionaries (because `pdf_is_stream()` is false for them, section 2.1), so
the "skip container streams" protection never fires — but nothing is emptied in them either
(their dictionaries hold names and numbers only). The objects *inside* an object stream are found
and their strings are emptied (section 3 of the log: `STRING /Object[8]/Contents … -> geleert`),
and the document that comes out of the redaction is intact and readable
(`attack_objstm_xrefstream.pdf` → `verified = true`, the secret is gone, the document opens).

**There is no endless loop.** The walk has a depth limit (48) and a list of visited objects; the
sweep runs over the table once. 32,000 objects → 0.83 s (section 8), no hang.

**The image with the EXIF comment** (`attack_image_exif.pdf`) is not a leak: MuPDF re-encodes the
image when it saves, so the comment is gone before the walk sees it. The image itself survives
(`Objektarten: {'Catalog': 1, 'Pages': 1, 'Page': 1, 'XObject': 1}`) and the document is intact.

---

## 9. The answers to the questions of the round, one by one

**1. The sweep over the cross-reference table.**
* *Is too much changed?* No: the scrub only empties strings, and the sweep only reaches strings
  that carry the word. The one place where the sweep empties something that is not text of the
  user is the encryption dictionary (R4-4).
* *Is an object emptied that carries no text at all?* The scrub empties **every** string that
  carries the word, including structural strings (`/Registry`, `/Ordering` of a CID font, the
  `/Name` of an optional-content group, the `/Lang` of the catalog, the `/Params` of an embedded
  file). Only `BaseFont`, `FontName`, `Encoding`, `ID`, `Filter`, `Length`, `Type` and `Subtype` are
  protected. The new sweep *widens* the set, because it now walks the dictionaries of the streams
  that the ordinary walk skips.
* *Can the sweep destroy an object that MuPDF needs?* Not in the measured cases (object stream,
  cross-reference stream, image, form XObject). The one destruction measured is the encryption
  dictionary (R4-4).
* *Is an object visited twice?* No (section 8).
* *What happens to an object that lies compressed in an object stream (`/Type /ObjStm`)?* It is
  found (the objects inside the object stream are loaded from the stream by the sweep and their
  strings are emptied) — measured. The object stream itself is walked as a dictionary.
* *Can an object that is reachable only through the cross-reference table send the sweep into an
  endless loop?* No. The sweep iterates over the table once; the walk is bounded by the depth
  limit and by the list of visited objects. 32,000 objects: 0.83 s, no hang.
* *What happens with encrypted documents (`/Encrypt`)?* The redaction of an encrypted document
  works and the document stays encrypted. But the new walk of `/Encrypt` lets the scrub empty the
  `/O` or `/U` string when the redacted word is a single letter (R4-4).
* *Is the content of an image (EXIF comment!) reported as a place and then emptied?* Not
  reported and not emptied — but not because of the walk: MuPDF re-encodes the image when it
  saves, so the comment is gone before the walk runs.

**2. Unreadable places make the check incomplete.**
* *Is there a way to get `verified = true` although a place was unreadable?* **Yes** (R4-3): the
  detection of a container depends on the path string `EmbeddedFiles`; an archive under `/AF` is
  not detected, is not reported, and the secret stays in the document.
* *Is there a way in which the rollback does not happen although the report is incomplete?* Not
  found. Every path that leads to an incomplete report goes through `fail_after_change()` →
  `rollback_redaction()`; the rollback was observed to restore the bytes (the encrypted case: the
  document was unchanged after the failure).
* *What happens when `find_word_occurrences` fails (`Err(error) => NotChecked`)?* The reason is
  named and reaches the caller: `not_checked_reasons` receives
  `"the places of the document that could still carry the text could not be read: {error}"`
  (`core/vocument-core/src/redaction.rs:707-711`), `failed_checks()` appends the reasons, and the
  FFI turns a non-empty `failed_checks()` into `DocumentCoreError::RedactionUnverified`. The
  report itself is lost in that case (the core returns `Err`), but the reason is in the message of
  the error. **Not measured end to end** — I could not produce a document in which the engine
  fails to report the places after the redaction succeeded; the path was read, not measured.

**3. The reasons are named.**
* *Is there an aborted check whose reason does not arrive?* No path found. The four possible
  aborts (search index error, no reader for the rendered page, uninspected places, error while
  reading the places) all push a reason, and `failed_checks()` adds them to the list of failures.
  The one abort without a *specific* reason is `checked_words.is_empty()` (the search and the byte
  check), which produces the generic message "the check could not be carried out"; that state is
  unreachable in practice, because a region without a word is refused earlier with
  `NothingToRemove`.
* *Does `failed_checks()` contain everything the user has to know?* For the *successful* case:
  yes. For the *failed* case the report is lost and the error carries the failures, the
  uncleaned places, the removed words, the remaining occurrences and the uninspected places —
  measured in the archive case, where all of it arrived in the message of the error.

**4. The recovery.**
* *Can a foreign user read or replace the recovery data of the user?* The directory is accepted
  when it belongs to somebody else (R4-6, measured). The files are 0600, but the container
  directory belongs to the stranger and can be replaced.
* *Is the directory checked before the write?* Once, at the start, and the error is ignored
  (measured, section 10 C).

---

## 10. The limits of this measurement

* **No OCR in the sandbox.** There is no `libtesseract` on this machine, so the harness links a
  stub that returns an empty OCR result (the same construction as the third round, the source is
  in `harness/tess-stub.c`). The check against the rendered page therefore ran **vacuously** in
  every measurement: `rendered_page_clean = Passed` means "no word was found on the picture",
  not "the picture was read". All findings above concern the other three checks and the walk; an
  attack that relies on text that is *visible* on the page would be caught by the OCR in a real
  installation and is **not** claimed here. The stub is the reason the product's own check could
  not be exercised against a scanned page.
* **The measurements ran as `root`.** The recovery measurements in section 10 B therefore show the
  data landing in a foreign directory; as a normal user the write would fail instead (the
  directory is 0700 and belongs to the stranger). Section 10 C was measured as uid 1000
  (`setpriv`) and shows the other half of the behaviour.
* **Only MuPDF 1.28.5 was used** — the version the repository links against
  (`target/mupdf-host`). The finding R4-1 rests on `pdf_is_stream()` of this version; the
  behaviour of other versions was not tested.
* **The document corpus is synthetic.** The 20 PDF files were written for this round; a corpus of
  real documents (the repository's own test files) was not swept for further leaks, because the
  three findings above are already enough to fail the round.
* **The rendered picture was not read** (see above) and the search index was not audited
  separately.
* **The `find_word_occurrences` error path was read, not measured** (see section 9, item 2).

---

## 11. How to reproduce

```bash
cd /workspace/.mavis/plans/plan_25670a94/workspace/redteam

# 1. Die Bibliothek des Arbeitsbaums bauen (eigenes Zielverzeichnis)
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/workspace/.mavis/plans/plan_25670a94/workspace/cargo-target
export MUPDF_DIR=/workspace/vocument/target/mupdf-host
export TESSERACT_DIR=$PWD
cd harness && cargo build && cd ..

# 2. Das komplette Messprotokoll
./harness/run_measurements.sh > messung-r4.txt 2>&1

# 3. Der kritische Fall einzeln
export LD_LIBRARY_PATH=/workspace/vocument/target/mupdf-host/lib:$PWD/lib:$CARGO_TARGET_DIR/debug
$CARGO_TARGET_DIR/debug/redteam-harness pdfs/attack_form_page1.pdf GEHEIM 0 form
python3 harness/inspect_pdf.py out/form.pdf
```

The harness drives the exported functions of the product (`document_core_open_macos_scoped_file`,
`document_core_page_words`, `document_core_redact_page_regions`,
`document_core_export_session_bytes`); the probe links the real MuPDF and the real walk header
(`harness/mupdf_walk_probe.h` is a copy of `engines/pdf-mupdf/csrc/mupdf_walk.h` with two
`#ifdef` switches for the A/B comparison; `diff` against the original shows only those four
lines).

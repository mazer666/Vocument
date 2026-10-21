# Red Team V — the whole redaction chain under attack

- **Date:** 2026-10-21
- **Checked stand:** `/workspace/vocument`, branch `main`. The task named `bbc2d0b`; during the
  measurements a concurrent agent committed `0353ac3` ("fix(m2): the macOS sources did not
  compile …") and later changed `engines/pdf-mupdf/csrc/mupdf_walk.h` — **the fix for my finding
  R5-1**, which the comment of that change names ("the fifth round of the red team measured a
  redaction that emptied the `/U` of an encrypted document and broke it"). The measurements of
  this report are against the stand of the protocol (the fingerprints are in section 0 of
  `messung-r5.txt`); the fix is **not** part of them. I rebuilt against the changed working tree
  and re-measured the attack: **the fix closes R5-1** (section 2.3).
- **Method:** a purpose-built harness drove the **real** chain (core → MuPDF → the exported UniFFI
  functions `document_core_open_macos_scoped_file`, `document_core_redact_page_regions`,
  `document_core_export_session_bytes`) over **32 hand-built PDF files** (Rust, byte by byte — no
  PDF library, no Python). Two independent oracles checked every output: a scanner of my own
  (raw bytes, Flate, raw Deflate, LZW, UTF-16BE/LE, hex-UTF16) and the text extractor of
  `mutool` 1.28.5. The encryption of the attack files was validated **byte for byte** against a
  file that MuPDF itself wrote (`mutool clean -E rc4-40`): my `/O` and my `/U` are identical to
  MuPDF's (`harness/src/bin/cryptotest.rs`).
- **Raw evidence:** `messung-r5.txt` (1,497 lines: every command, every number), `harness/`
  (the harness, the PDF builder, the crypto self-test), `pdfs/` (the 32 attack files), `out/`
  (the redacted output documents), `lib/` (the real Tesseract of the host, see "Limits of the
  examination").
- **Nothing in the repository was changed.** Own build directory
  (`CARGO_TARGET_DIR=/workspace/vocument/target/redteam-r5`), own workspace, no commit.

---

## 1. The answer in one sentence

**The redaction of the checked stand did not lie in any of the 32 attacks: in every case in which a
secret survived in the document, the redaction was refused and the place was named.** Two real
weaknesses were found, neither of which I could turn into a surviving leak: the scrub still empties
the `/U` of an encrypted document (the **residue of R4-4**, the fix of round four only covers the
*direct* encryption dictionary — closed in the working tree during the run, see 2.3), and the
search for the remaining text is a **byte-level containment test** that does not see a word that
is split across several show operations.

| # | Severity | Finding | Evidence |
|---|---|---|---|
| **R5-1** | **medium** | The scrub empties the **`/U`** of an encrypted document when the encryption dictionary is an **indirect object** — the form the PDF specification asks for. The redaction of a one-letter word in an encrypted document destroys the encryption and dies with a misleading error ("PDF requires an open password"). The fix of round four (R4-4) protects only the encryption dictionary that hangs directly in the trailer. **Closed in the working tree during the run — see 2.3.** | `a15_encrypted_oneletter.pdf`, sections 1b, 2, 4b and 6 of the log |
| **R5-2** | **medium** | The search for the remaining text is a **byte-level containment test** (`vocument_contains_word`). A secret that is written as `(GE) (HEIM) Tj` — split over two show operations — is **not** found. The verification "the text is gone" is blind to it. | `a20`, `a27`, `a28`, `a30` against the controls `a29`, `a31`, section 1b |
| **R5-3** | **low** | The verification runs on the document **after** the scrub, and the scrub's own write cycle (MuPDF's garbage collector) removes the objects that nothing reaches any more. Those places are therefore never examined and never named: the safety of the redaction rests on **MuPDF's writer**, not on the check of the product. | `a05`, `a06`, `a24`, section 1b and the scrub measurement of section 2 |
| — | — | *No critical finding.* The critical finding of round four (the sweep did not see the content of a stream) is closed and stays closed. | `a09` (LZW), `a27`/`a29` (a form in the resources of the page), section 2 |

---

## 2. R5-1 (medium) — the scrub empties the `/U` of an encrypted document

### 2.1 What the product does

The walk refuses to descend into the `/Encrypt` key of a dictionary
(`engines/pdf-mupdf/csrc/mupdf_walk.h`):

```c
        /* The encryption dictionary holds the keys of the document, not the
           text of the user. Emptying one of those keys would destroy the
           document … */
        if (name && strcmp(name, "Encrypt") == 0) {
            continue;
        }
```

That protects the encryption dictionary **as it hangs in the trailer**. But the sweep over the
cross-reference table (`vocument_walk_xref`) hands out **every object of the file** — and the
encryption dictionary is an object of the file, as soon as it is an *indirect* object. The
protection of the walk does not apply to that path: the sweep reaches the dictionary by its
*number*, not by the `/Encrypt` key.

The scrub then empties **every string that carries one of the redacted words**
(`mupdf_scrub_bridge.c`, `vocument_scrub_place`). The protected-key list
(`VOCUMENT_PROTECTED_KEYS`) contains `"Encrypt"` — but not `"O"` and not `"U"`, the two values that
hold the keys of the document.

The PDF specification asks for the trailer's `/Encrypt` to be an **indirect reference**
(ISO 32000-1, Table 15). MuPDF's own writer puts the dictionary *directly* into the trailer — so
the vulnerable form is the form the specification prescribes, and the protected form is the one
MuPDF happens to write.

### 2.2 The measurement

The attack file `a15_encrypted_oneletter.pdf` is an RC4-40 document (R=2, V=1) that I encrypted
myself; the byte `B` (0x42) was chosen because it occurs in its `/U`. The page carries the
one-letter word `B`; the redacted word is therefore `B`.

The chain (section 2 of the log):

```
=== a15_encrypted_oneletter ===
zielwort: "B" box=[72000,116200,88008,149176]
warning: encryption password key too short (0)
dauer: 0.071 s
ERGEBNIS: Fehler
  fehler: PDF requires an open password
```

The warning of MuPDF is the proof: `pdf-crypt.c` prints it when the `/U` string is **shorter than
32 bytes** — the value was **emptied** by the scrub.

The scrub alone, measured directly (section 4b of the log):

```
  $BIN/occ scrub pdfs/a15_encrypted_oneletter.pdf B /tmp/a15_scrubbed_B.pdf
  gespeichert: /tmp/a15_scrubbed_B.pdf (786 Bytes)
  geleert: annotationen=0 formularwerte=0 andere=1
  /O<       -> /O<2055C756C72E1AD702608E8196ACAD447AD32D17CFF583235F6DD15FED7DAB67>/U()/P -4>>
  /U<       -> nicht vorhanden
  /Encrypt  -> /Encrypt 6 0 R>>
```

The control — a word that does **not** occur in `/U` — leaves the key intact:

```
  $BIN/occ scrub pdfs/a15_encrypted_oneletter.pdf XQQ /tmp/a15_scrubbed_XQQ.pdf
  geleert: annotationen=0 formularwerte=0 andere=0
  /O<       -> /O<2055C756C72E1AD702608E8196ACAD447AD32D17CFF583235F6DD15FED7DAB67>/U<A64E367000E34D42AF67B3BE5C91F5F87F252A369C6A874EA40A79129903A526>/P -4>>
```

**Consequence.** The redaction of a short word in an encrypted document is impossible, and the
message blames the document ("PDF requires an open password") instead of naming the cause. The
rollback works — the document is unchanged, the secret is still in it (nothing was removed, so
nothing was lost) — but the redaction is dead.

**The residue of R4-4.** Round four found the same mechanism (the walk of `/Encrypt`) and the
fix was to stop the walk from descending into the key. The fix closed the *direct* path only. The
sweep — which round four itself introduced — opens the path again.

### 2.3 The fix that came into the working tree during the run

At 13:41:55 a concurrent agent changed `engines/pdf-mupdf/csrc/mupdf_walk.h`: the walk now
recognises the security dictionary **by its content** (`/O`, `/U`, `/Filter /Standard`) and never
descends into it — "wherever it hangs, and whether it is reached by the walk from the root or by
the sweep over the table". I rebuilt the harness against that working tree and ran the attack
again (section 6 of the protocol):

```
=== a15_after_fix ===
dauer: 0.579 s
ERGEBNIS: Ok
  verified                = true
  removed_words           = ["B"]
  bytes                   = 786 Bytes
```

and the scrub alone:

```
  $BIN/occ scrub pdfs/a15_encrypted_oneletter.pdf B /tmp/a15_after_fix.pdf
  geleert: annotationen=0 formularwerte=0 andere=0
  /O<  -> /O<2055C756C72E1AD702608E8196ACAD447AD32D17CFF583235F6DD15FED7DAB67>/U<A64E367000E34D42AF67B3BE5C91F5F87F252A369C6A874EA40A79129903A526>/P -4>>
```

`/U` stays intact, nothing is emptied (`andere=0`), and the redaction of the one-letter word in
the encrypted document now succeeds. **R5-1 is therefore closed against the working tree** — it
remains a finding against the released stand `bbc2d0b` that this round was asked to attack.

(The byte comparison of the protocol reports `geheimnis im Klartext = true` for the output of
this run: the letter `B` occurs as a **hex digit** in `/O` and `/U`. That is not the secret.)

### 2.4 What is *not* broken

The encrypted document with a **multi-letter** secret is fine (`a08_encrypted_form.pdf`): the
redaction of `GEHEIM` in an encrypted document is refused with the place named
(`/Root/Extra`, kind `hidden`) — the secret is found in the decrypted content stream of the form
XObject. The problem is only the *short* words, because only they can occur by chance in 32
random bytes.

---

## 3. R5-2 (medium) — the search for the remaining text is a byte-level containment test

### 3.1 What the product does

Both bridges ask the same question about a place: does this buffer carry the word? The question is
answered by `vocument_contains_word` (`engines/pdf-mupdf/csrc/mupdf_text_search.h`), which looks
for the word as plain text, as UTF-16BE/LE and as the hex string of the UTF-16 bytes. All of
these are **contiguous byte sequences**. A text that a writer splits over two show operations —
`(GE) (HEIM) Tj` — carries the secret, but not as a sequence that the comparison can find.

### 3.2 The measurement

The attack files put a form XObject with the split text into the document. The **control** is the
same file with the word written in one piece.

The diagnostic (section 1b of the log) — `find_word_occurrences` of the engine, i.e. what the walk
offers:

```
### a30_split_surviving
  Orte, die das Wort tragen: 1
    kind=content      seite=0    ort=content stream of the page
### a31_whole_surviving          ← die Kontrolle
  Orte, die das Wort tragen: 2
    kind=hidden       seite=0    ort=/Page/Resources/XObject/Fm0
    kind=content      seite=0    ort=content stream of the page
```

The same difference in the other files:

| file | the text in the form | places the walk reports | the redaction |
|---|---|---|---|
| `a20_split_text.pdf` | `(GE) (HEIM) Tj` | 1 (the content of the page) | `verified = true` |
| `a27_split_reachable.pdf` | `(GE) (HEIM) Tj` | 1 (the content of the page) | `verified = true` |
| `a28_split_three.pdf` | `(GE) (HE) (IM) Tj` | 1 (the content of the page) | `verified = true` |
| `a30_split_surviving.pdf` | `(GE) (HEIM) Tj` | 1 (the content of the page) | `verified = true` |
| `a29_whole_reachable.pdf` | `(GEHEIM) Tj` | 2 (the form is named) | `verified = true` (the form is gone) |
| `a31_whole_surviving.pdf` | `(GEHEIM) Tj` | 2 (the form is named) | `verified = true` (the form is gone) |

**The limit of this finding, measured and stated.** In every one of the four attack files the
secret did **not** survive in the output: MuPDF's writer removed the form XObject (the page's
`/Resources` lost its `/XObject` entry, the catalog's `/Extra` became `null`). I could therefore
**not** produce a document that carries the split secret after a redaction that reported
`verified = true`. The finding is the **blindness of the check**, not a demonstrated leak — and I
say so rather than dress it up.

What the finding does mean: the product's proof that "the removed text is nowhere in the document
any more" rests on a byte-level comparison. A document that carries the secret in pieces passes
that proof. Whether such a document survives depends on MuPDF's writer, not on the product's
check.

---

## 4. R5-3 (low) — the verification runs behind the garbage collector of the writer

### 4.1 What the product does

The order of the chain (`engines/pdf-mupdf/src/mutate.rs`, `redact_page_regions_in_pdf`):

1. MuPDF removes the content of the region and writes the document.
2. `scrub_words` opens that document, walks it, empties the strings that carry the word and
   **writes the document again**.
3. The core verifies — `find_word_occurrences` runs on the bytes of step 2.

Step 2 is a *write cycle* of MuPDF, and MuPDF's writer removes the objects that nothing reaches
any more. Those objects are therefore **gone before the verification looks at them**: the places
are never offered to the check, never named in `uninspected_places`, never reported in
`remaining_occurrences`.

### 4.2 The measurement

`a05_form_recursive.pdf` (a form XObject that calls itself, with the secret in its content
stream, hanging in `/Root /Extra`):

```
### a05_form_recursive — der Scrub allein
  gespeichert: /tmp/a05_form_recursive_scrubbed.pdf (598 Bytes)   ← die Eingabe hat 811 Bytes
  geleert: annotationen=0 formularwerte=0 andere=0
  enthaelt das Ergebnis das Geheimnis? 1   (nur der Inhaltsstrom der Seite)
  die Stellen nach dem Scrub:
    kind=content      seite=0    ort=content stream of the page
```

Before the scrub the walk **did** see the place (section 1b):

```
### a05_form_recursive
  Orte, die das Wort tragen: 2
    kind=content      seite=0    ort=content stream of the page
    kind=hidden       seite=-1   ort=/Object[6]
```

After the scrub the place is gone — the writer removed it — and the verification of the product
finds nothing to complain about. The redaction reported `verified = true` with
`remaining_occurrences = []`, `uninspected_places = []`, `not_checked_reasons = []`.

**Consequence.** The safety in these cases is a **side effect of MuPDF's writer**. The product's
own checks would have refused the redaction (they see the place) — but they never get the chance,
because the check runs on the sanitized document. A future MuPDF that keeps such an object (or a
place that MuPDF's writer *does* keep and the walk does not see) would turn this into a leak
without a change in the product's code.

---

## 5. The five questions, one by one

### 5.1 Does the walk go through *every* object?

**Measured.** I built a file for every way I could think of to hide an object, and asked the
engine's own walk (`occ list`, section 1b) which places it sees:

| way to hide | file | seen by the walk? | the redaction |
|---|---|---|---|
| a stream that hangs only in an **array** (`/Root /Probe [ 6 0 R ]`) | `a02` | yes — `/Root/Probe[0]`, kind `hidden` | **refused**, the place named |
| an object that hangs only via **`/Parent`** of the page (the root of the page tree is a different node) | `a03` | yes — the sweep (`/Object[6]`) | the secret is **cleaned** (`verified = true`, no secret in the output) |
| an object that exists only in the **list of an `/ObjStm`** and is referenced by nothing | `a04` | yes — `/Object[6]/PieceInfo/S`, `/Object[7]/Private` | the secret is **cleaned** |
| the same, **deep** in a chain of resources (all inside the object stream) | `a22` | yes — `/Object[6]/Resources/XObject/Next/PieceInfo/S` | the secret is **cleaned** |
| a **form XObject** that calls itself | `a05` | yes — `/Object[6]` | `verified = true`, the form is removed by the writer |
| a form in the **popup** of an annotation | `a06` | yes — `/Page/Annots[0]/Contents` | `verified = true`, the form is removed by the writer |
| a form **deeply nested** (60 levels, deeper than the limit of 48 of the walk) | `a07` | yes — `/Object[56]/Resources/XObject/Next/…` (the sweep) | **refused**, the place named |
| a form in the **resources of the page** | `a29`, `a31` | yes — `/Page/Resources/XObject/Fm0` | the form is removed by the writer |
| a form in an **array** of another form | `a24` | yes | `verified = true`, the form is removed by the writer |
| `/PieceInfo` of the page, `/LastModified` of the page tree | `a16` | yes (they are dictionaries) | the secret is **cleaned** |
| private data as a **stream** (`/Root /Private`) | `a17` | yes — `/Root/Private`, kind `hidden` | **refused**, the place named |
| a page that hangs only in an **`/Annots` chain** (a page object that is not in the page tree) | `a19` | yes — the content of that page is reported as `content`, page 0 | **refused**, the place named |
| a **ZIP** as an attachment (the secret inside the archive) | `a18`, `a23` | yes — `/Root/Names/EmbeddedFiles/Names[0]/EF/F`, kind `uninspected` | **refused**, the place named |
| an **encrypted** document with a form that carries the secret | `a08` | yes — `/Root/Extra`, kind `hidden` | **refused**, the place named |
| a form whose content is **LZW**-coded | `a09` | yes — `/Root/Extra`, kind `hidden` | **refused**, the place named |
| a form whose content is a **hex UTF-16** string | `a21` | yes — `/Root/Extra`, kind `hidden` | **refused**, the place named |
| a **circular** reference (an object that points to itself) | `a14` | yes (the walk terminates, 0.466 s) | `verified = true`, the secret is cleaned |
| a **linearized** file | `a13` | yes | `verified = true`, the secret is cleaned |
| a **60 000 objects** document | `a12` | yes (1.422 s) | `verified = true`, the secret is cleaned |
| a stream with a **huge `/Length`** (999 999 999) | `a10` | yes — `/Root/Extra`, kind `hidden` | **refused**, the place named |
| a stream with a **too small `/Length`** (the secret lies behind it) | `a26` | yes — `/Root/Extra`, kind `hidden` | **refused**, the place named |
| a stream with a **broken filter** (Flate, but the bytes are plain text) | `a11` | yes (the place is named as `hidden`, the content cannot be decoded) | **refused** |
| an **object stream** that cannot be decoded | `a25` | yes (the objects inside were read) | the secret is cleaned |

**Answer:** the walk of the checked stand reaches every object I was able to construct — including
the objects that exist only in an object stream, the objects that hang only behind `/Parent`, the
objects behind the limit of depth and the objects in the arrays of streams. The one way in which I
could make a place invisible to the walk is the *encoding* of the text (R5-2), not the *place*.

### 5.2 The limits and the aborts of the search for the places

**Measured (the times of section 2 of the log):**

| file | what it is | the time of the whole redaction | the outcome |
|---|---|---|---|
| `a12_many_objects.pdf` | 60 000 objects (5 149 507 bytes) | **1.422 s** | `verified = true` |
| `a14_circular.pdf` | circular references | 0.466 s | `verified = true` |
| `a13_linearized.pdf` | linearized structure | 0.481 s | `verified = true` |
| `a10_huge_length.pdf` | `/Length` = 999 999 999 | 0.530 s | refused, the place named |
| `a11_broken_filter.pdf` | broken filter | 0.667 s | refused |
| `a25_objstm_broken.pdf` | object stream | 0.669 s | `verified = true` |
| `a01_base.pdf` | the control (one page) | 0.496 s | `verified = true` |

The times include the **OCR of the rendered page** (two renderings and two readings of the page);
the control `a01` shows the floor: about 0.5 s are the OCR, the redaction itself needs a
fraction of a second. **No abort was observed.** Every run finished; no check returned `NotChecked`
because of a timeout, and no walk was cut short. The quadratic behaviour of round four (R4-5) is
gone: 60 000 objects need 1.422 s for the *whole* redaction (two walks, the redaction, the scrub,
the search index, the rendering, two OCRs). Round four measured 14.0 s for a document with 32 000
*live* annotations (and its sweep alone 0.83 s for 32 000 objects).

**The damaged object stream** (`a25`): I could not build an object stream that MuPDF refuses to
read *and* that carries the secret — my attempt was read after all (the objects inside were
found: `/Object[6]/PieceInfo/S`). The place was therefore always named. **NOT RUN:** a *really*
corrupt object stream (a truncated `/First`, a broken Flate payload in the object stream).

**The linearized file** (`a13`): MuPDF 1.28.5 no longer supports writing linearized files
(`mutool clean -l` says "no longer supported"), so I built the structure by hand: the
linearization dictionary and the cross-reference of the first page at the front of the file, the
hint stream, the objects in the order of the pages. MuPDF reads it without a repair
(`mutool draw` prints the text without a warning). **NOT RUN:** a file that was linearized by a
real producer (Ghostscript, Acrobat) — with its hint stream and its partial xref.

### 5.3 The form XObjects

**Measured.** The stream descent of round four holds: the content of a form XObject is seen, no
matter how the form is reached.

| how the form is reached | file | the place the walk names | the outcome |
|---|---|---|---|
| in the resources of the page | `a29`, `a31` | `/Page/Resources/XObject/Fm0` | the form is removed by the writer |
| recursively (a form calls itself) | `a05` | `/Object[6]` | the form is removed by the writer |
| in the popup of an annotation | `a06` | `/Page/Annots[0]/Contents` | the form is removed by the writer |
| 60 levels deep in the resources | `a07` | `/Object[56]/Resources/XObject/Next/…` | **refused** |
| in an **encrypted** document | `a08` | `/Root/Extra` | **refused** |
| **LZW**-coded content | `a09` | `/Root/Extra` | **refused** |
| hex UTF-16 content | `a21` | `/Root/Extra` | **refused** |
| in an array of another form | `a24` | the form | the form is removed by the writer |

The text in the form is found as a **hidden** place (kind `hidden`, page −1) and the redaction is
refused — the text is not merely "hidden" from the search, it is named. **The only way in which I
made the form invisible is the splitting of the text (R5-2), not the place.**

### 5.4 The contents that cannot be examined

**Measured.** The rule "a place that cannot be read makes the verification incomplete" held in
every case I could construct:

| case | file | what the report says | the outcome |
|---|---|---|---|
| a stream with a broken filter | `a11` | `the text is still there: … (kind hidden)` | refused |
| a stream with a too small `/Length` | `a26` | `the text is still there: /Root/Extra (kind hidden)` | refused |
| a stream with a huge `/Length` | `a10` | `the text is still there: /Root/Extra (kind hidden)` | refused |
| a **ZIP** with the secret inside | `a18` | `the document could not be read completely: 1 place(s) could not be looked into, among them /Root/Names/EmbeddedFiles/Names[0]/EF/F` | refused |
| a **ZIP** with an encrypted entry | `a23` | the same, the place is named | refused |
| an **encrypted document** | `a08`, `a15` | the place is named / the document is refused | refused |
| an object stream that cannot be decoded | `a25` | (the objects were readable after all) | the secret was cleaned |

**No place was ever reported as "everything is clean" while a place had not been read.** In every
one of the cases above the redaction was **refused** and the place was named in the message.

**NOT RUN:** a PDF whose `/Encrypt` asks for a **password that is not empty** (the user password
is not given): the reading of the encrypted document would fail with a different error, and I did
not measure it.

### 5.5 The handling of the confidential remainder

**(a) A remainder somewhere else is reported, not refused.** Measured with `a19` (a page that
hangs only in an `/Annots` chain) and with the places that the walk names: a place with the kind
`content` on another page goes into `remaining_occurrences`, a place with the kind `hidden` stops
the redaction. The two kinds are distinguished in the code
(`core/vocument-core/src/redaction.rs`, the filter over the occurrences) and both behave as
documented.

**(b) The remainder stays in the document — and it is not carried along into the protocol, the
journal, the checkpoint, the search index or the cache.** Measured: I pointed the data directory of
the library into a directory of my own (`XDG_DATA_HOME`), ran one successful and one refused
redaction and read the recovery store afterwards:

```
--- der Wiederherstellungsspeicher nach der erfolgreichen Redaction:
…/vocument_recovery/doc_d36296c4599a7bee/journal.json
…/vocument_recovery/doc_d36296c4599a7bee/manifest.json
--- greppt das Geheimnis in irgendeiner Datei des Speichers?
(keine Datei)
```

The journal carries the region, the description and the hashes — **no text**:

```json
"description": "Remove the content of 1 region(s) on page #1",
"forward_command": { "RedactPageRegions": { "page_index": 0,
  "regions": [ { "x0_milli_pt": 72000, … } ] } }
```

After the **refused** redaction (the document is unchanged, the secret is of course still in the
document) the store holds the same two files and again no text. The checkpoints are deleted after
a successful redaction (`purge_checkpoints_after_success`); in this run no checkpoint was written
at all (the store writes one every ten revisions).

**(c) The removed text reaches the user in the error message — and nowhere else.** The message of
the refused redaction names the removed words (`the words that were removed: GEHEIM`); that is
intended. It is **not** written into the journal (measured above), and it is not written into a
file of the recovery store (the grep over the whole store found nothing).

**What I did not measure:** the cache of the rendered pages and the thumbnails are dropped in the
process (`purge_render_cache`), but they live in the memory of the process and I did not read them
from the outside. **NOT RUN:** a crash *during* the redaction, with the question of what the
journal of the recovery store would hold at that moment.

---

## 6. What is **not** a finding

The attacks that failed, with the evidence that they failed.

| the attack | the file | the evidence |
|---|---|---|
| the critical finding of round four comes back: the sweep does not see the content of a stream | `a09` (a form with LZW content), `a29`/`a31` (a form in the resources of the page) | the walk names the place (`/Root/Extra`, `/Page/Resources/XObject/Fm0`), the redaction is **refused** |
| a stream that hangs only in an array | `a02` | the walk names `/Root/Probe[0]` (kind `hidden`), the redaction is **refused** |
| an object that hangs only behind `/Parent` | `a03` | the sweep finds it, the secret is **cleaned**, the output has no trace of it |
| an object that exists only in an object stream | `a04`, `a22` | the sweep finds `/Object[6]/PieceInfo/S` and `/Object[7]/Private`, the secrets are **cleaned** |
| a form that calls itself (a cycle) | `a05` | the walk terminates (0.491 s), the place is named, no hang |
| a form 60 levels deep (deeper than the limit of 48 of the walk) | `a07` | the sweep finds it at `/Object[56]`, the redaction is **refused** |
| a form in the popup of an annotation | `a06` | the walk names `/Page/Annots[0]/Contents` |
| a form in an encrypted document | `a08` | the redaction is **refused**, the place `/Root/Extra` is named |
| a form with LZW content | `a09` | the redaction is **refused**, the place is named |
| a form with the text as a hex UTF-16 string | `a21` | the redaction is **refused**, the place is named |
| the secret in `/PieceInfo` and in `/LastModified` | `a16` | the secrets are **cleaned**, the output has no trace of them |
| the secret in private data (a stream) | `a17` | the redaction is **refused**, the place `/Root/Private` is named |
| a page that hangs only in an `/Annots` chain | `a19` | the redaction is **refused**, the place is named as `content`, page 0 |
| a ZIP as an attachment (the secret inside the archive) | `a18` | the redaction is **refused**, the place is named as `uninspected` |
| a ZIP with an encrypted entry | `a23` | the redaction is **refused**, the place is named as `uninspected` |
| a stream with a huge `/Length` | `a10` | the redaction is **refused**, the place is named |
| a stream with a too small `/Length` | `a26` | the redaction is **refused**, the place is named |
| a stream with a broken filter | `a11` | the redaction is **refused**, the place is named |
| a circular reference | `a14` | the walk terminates (0.089 s), the secret is cleaned |
| a linearized file | `a13` | MuPDF reads it without a repair, the secret is cleaned |
| 60 000 objects | `a12` | 0.805 s, no abort, the secret is cleaned — the quadratic behaviour of R4-5 is gone |
| the removal is coarser than the marked region | `a33` | the redaction removes exactly the word: the content stream of the output reads `q BT/F1 24 Tf 72 700 TD (Vorher )Tj 173.376 0 TD ( Nachher)Tj ET Q` — the words before and after the secret **stay** |
| the secret is carried along into the journal or a checkpoint | `a01`, `a18` | the recovery store holds only the journal and the manifest; the grep for the secret over the whole store finds **nothing** |
| the secret is carried along into the error message on the disk | `a18` | the message names the words (intended), the journal does **not** contain them |
| the redaction of a multi-letter word in an encrypted document | `a08` | the redaction is refused with the place named, the encryption stays intact |
| the whole text of a page disappears with the redaction | `a33` | only the marked word is removed, the rest of the text stays |

---

## 7. The limits of the examination

**What I did not test, and what therefore stayed NOT RUN:**

1. **The check of the picture of the page (`rendered_page_clean`) ran for real.** The library
   `libvocument_ffi.so` carries a `DT_NEEDED` on `libtesseract.so.5`, and Tesseract is not
   installed on this host (the builds in the tree are Android builds for aarch64 and cannot be
   linked on x86_64). I fetched the **real** library (Tesseract 5.3.0 and Leptonica 1.82 for
   x86_64, taken from the Debian archive by the team member of the palette round, copied into
   `lib/`) and pointed the OCR at the **real** language data of the project
   (`apps/android/app/src/main/assets/tessdata`, `TESSDATA_PREFIX`). The proof that the OCR
   really reads (`harness/src/bin/ocrtest.rs`):

   ```
   $BIN/ocrtest pdfs/a01_base.pdf
     gerendert: 1190x1684 Pixel (Graustufen)
     die OCR liest 1 Woerter:
       "GEHEIM" (Konfidenz 96.21224, BBox BoundingBox { x0: 146.0, y0: 248.0, x1: 330.0, y1: 285.0 })
   $BIN/ocrtest out/a01_base.pdf          ← die Seite nach der Redaction
     die OCR liest 0 Woerter:
   ```

   The **first** run of the protocol was measured with a **replacement** for that third-party
   library (`lib/stub/tess_stub.c` — it can read no text at all); it is **not** used any more, and
   every number in this report is from the final run with the real OCR (section 1c of the
   protocol). **The product's
   code is not stubbed anywhere**: the walk, the scrub, the search for the places, the
   verification, the OCR, the journal and the recovery store are the real ones.
   **Still NOT RUN:** a secret that lives only in the **pixels** of an image (the attack class of
   the third round) — I built no image attack file in this round, so the pixel path of the
   redaction was not attacked here.
2. **A really corrupt object stream** (a truncated `/First`, a broken Flate payload *in* the
   object stream) — my attempt was readable after all (`a25`).
3. **A file that was linearized by a real producer** (Ghostscript, Acrobat) with its hint stream
   and its partial cross-reference — MuPDF 1.28.5 cannot write one any more.
4. **An encrypted document with a password that is not empty** — the reading would fail with a
   different error.
5. **A crash during the redaction** — what the journal of the recovery store holds in that
   moment.
6. **The caches of the rendered pages and the thumbnails** were not read from the outside; they
   are dropped by the code after a successful redaction, but I did not measure it.
7. **The shell of the platform** (macOS, Android) was not tested — only the library.
8. **The stand moved twice during the run** (a concurrent agent committed `0353ac3`, changed the
   macOS sources and the annotation store, and at 13:41:55 changed `mupdf_walk.h` — the fix for
   R5-1). The fingerprints of the attacked files are in section 0 of the protocol; the fix is not
   part of the measurements of sections 2 to 5, it is measured separately in section 2.3.
9. **The C probe against the walk header** (`mupdf_walk.h` copied unmodified) of round four was
   **not** rebuilt. The diagnosis of the places that the walk offers was done with the engine's
   own function `find_word_occurrences` (`harness/src/bin/occ.rs`) — a measurement instrument of
   the product, not an independent oracle. The *verdicts* in this report rest on the scanner of
   my own and on `mutool`, not on that instrument.
10. **The chain of the search index** was measured only indirectly (the check
    `search_index_clean` is part of the report of every run); the index itself lives in the
    memory of the process.

**The one place where I had to make a decision:** the encryption of the attack files. I
implemented the standard security handler (R=2, V=1, RC4-40) myself and validated it **byte for
byte** against a file that MuPDF itself wrote (`/O` and `/U` identical, the object key decrypts
MuPDF's stream to the plain text — `harness/src/bin/cryptotest.rs`). The letter that the attack
file `a15` uses ("B") was chosen *after* the encryption, so that it occurs in `/U` — the attack is
therefore deterministic and not a matter of luck.

---

## 8. What I would change

1. **R5-1:** the encryption dictionary must not be reachable for the scrub — neither through the
   `/Encrypt` key of the trailer nor through the sweep over the cross-reference table. The
   simplest rule: the scrub skips every object whose number is the number of the encryption
   dictionary (`pdf_get_xref_entry` of the trailer's `/Encrypt`), and the protected-key list gets
   `"O"` and `"U"`.
2. **R5-2:** the search for the places should not compare bytes, but the **text** of the place:
   the content streams of the pages and the forms should be read as text (the text of MuPDF) and
   compared word by word. Then `(GE) (HEIM)` is one word again, and the hex and the UTF-16 forms
   fall out of the same comparison.
3. **R5-3:** the verification should run on the document **before** the scrub — the places must be
   named, not silently removed by the writer. Then a place that MuPDF's writer removes is still a
   place that the product has examined.

---

*Raw measurements: `messung-r5.txt` (1 497 lines). The harness, the 32 attack files, the output
documents and the crypto self-test are in this working directory — they are the tools of the
attacker, not a part of the product.*

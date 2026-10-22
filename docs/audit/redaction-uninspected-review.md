# The review of the capture of the places that could not be examined

- **Date:** 2026-10-22
- **Subject:** the walk over the places of the document
  (`engines/pdf-mupdf/csrc/mupdf_walk.h`), the search for the places that
  carry the text (`engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c`), the
  test way into the search (`engines/pdf-mupdf/src/scrub.rs`) and the
  **pre-check** of the redaction (`core/vocument-core/src/redaction.rs`).
- **Role:** ROLE-09 (Security) and ROLE-13 (QA) — the independent reviewer.
- **Why:** the change under review promises that the capture of the places
  that the walk could **not** examine has no silent error any more: a note
  that cannot be written down sets a flag, the flag is reported as a place
  that could not be looked into, and the redaction is refused **before** the
  document is touched if the pre-check is not complete.
- **Method:** the repository was not changed. The build ran with its own
  target directory (`CARGO_TARGET_DIR=…/target/review-r7`), its own MuPDF
  (`MUPDF_DIR=…/target/mupdf-host`) and its own tessdata
  (`TESSDATA_PREFIX=apps/android/app/src/main/assets/tessdata`). The
  measurements come from a harness of the reviewer that lives **outside** the
  repository
  (`…/plan_469c9e35/workspace/harness`, two binaries: `review-harness` and
  `mock`) and from the test suites of the project.
- **The revision that was measured:** the change under review was committed
  by another agent while this review was running
  (`ae06cb3 fix(m2.1): eine Stelle, die nicht notiert werden kann, geht
  nicht mehr still verloren`). The four files that were measured are
  **bit-identical** to the content that was in the working tree when the
  review started (`git diff ae06cb3` on the four files: no difference).

---

## 1. The verdict in one paragraph

The hardening of the **note** path is correct and complete: a note that
cannot be written down sets the flag, the flag becomes a place of the kind
`uninspected`, and the pre-check refuses the redaction **before** the
removal is attempted — measured: **0 removals attempted**, the exported
document is **byte-equal** to the input. The report is not cut off any more:
the limit of 100,000 places ends in an **error**, never in a partial report.

But the **goal** of the change is not reached. There is still a class of
places that the walk cannot examine and that stays invisible in the report:
a stream whose content cannot be decoded. The walk receives an **empty**
buffer from the reader, examines the empty buffer, and reports nothing. The
pre-check cannot be complete for this class, because the report does not
know about the place at all — and the redaction of such a document is
reported as **`verified = true`** while the secret is in the file. That is
the same error class the change was made to close, on the path that the
change did not touch.

**VERDICT: FAIL** — two findings (F-1, F-2) have to be closed; F-3 to F-5
are smaller.

---

## 2. The findings

### F-1 (high) — A stream whose content cannot be decoded is examined as if it were empty: the report says nothing, and the redaction is reported as verified while the secret is in the document

**Where:** `engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c:245` — the
callback of the walk loads the content of a stream with `pdf_load_stream`.
MuPDF does **not** throw when the filter fails; it writes a warning
(`library error: zlib error: incorrect header check` / `warning: read error;
treating as end of file`) and hands out an **empty** buffer. The callback
therefore compares an empty buffer with the word, finds nothing, and reports
**nothing at all** — the place is neither `uninspected` nor `hidden`. The
`fz_catch` at line 247 only fires when the reader *throws* (an unknown
filter name), not when the data cannot be decoded.

**Method:** a document with a page that carries the secret and an attached
file whose stream is a **zlib stream of the secret with its first byte
flipped** (0x78 → 0x87). The control is the same document with an intact
zlib stream. The measurement ran the search of the engine and the redaction
through the FFI boundary; the **exported** file (678 bytes — the same
document that the engine wrote, 678 bytes) was inspected byte-wise, and its
stream was repaired and inflated with an independent tool (`python3` +
`zlib`).

**Evidence:**

```
--- C: an attached file whose stream cannot be decoded ---
library error: zlib error: incorrect header check
warning: read error; treating as end of file
  places reported: 1
    kind=content      page=0    len=26   location=content stream of the page
  longest location: 26 bytes

--- C-control: an attached file whose stream can be decoded ---
  places reported: 2
    kind=content      page=0    len=26   location=content stream of the page
    kind=hidden       page=-1   len=34   location=/Root/Names/EmbeddedFiles/Names[1]
```

The end-to-end redaction of the same document:

```
=== END TO END: C: an attached file whose stream cannot be decoded ===
  the redaction was VERIFIED (verified=true)
  uninspected_places=0 remaining=0 not_checked_reasons=[]
  exported bytes: 678  input bytes: 766  byte-equal: false

=== END TO END: C-control: an attached file whose stream can be decoded ===
  the redaction was REFUSED: RedactionUnverified { details: "the redaction could not be
  verified: file bytes: the content is still there; the text is still there:
  /Root/Names/EmbeddedFiles/Names[1] (page -1, hidden); the words that were removed: GEHEIM" }
```

What the engine leaves in the document (the redacted file of the harness):

```
=== ENGINE REDACTION: C: an attached file whose stream cannot be decoded ===
  the redacted document: 678 bytes
  the secret as plain text: false
  the intact compressed stream: false
  the broken compressed stream: true
  the attachment survived: true
```

The secret is still in the file that the product declares as securely
redacted — an independent reader recovers it from the exported file:

```
$ python3 -c "…zlib…"
size 678
raw stream bytes: 87da7377f570f5f4 len 15
repaired header: 78da
DECOMPRESSED: b'GEHEIM'
```

**Result: FAIL** — the same secret in the same place is found when the
stream is intact and is invisible when the stream cannot be decoded. The
report of the places looks complete (`uninspected_places = 0`,
`not_checked_reasons = []`), the redaction is `verified = true`, and the
text is in the file. The hardening of the note path does not cover this
path, and the completeness of the pre-check (which is defined as "no place
of the kind `uninspected`") cannot hold for it.

**What has to change:** the callback has to distinguish *a stream that was
read* from *a stream whose content could not be obtained*. The warning of
the reader has to become a place of the kind `uninspected` (or the walk has
to refuse the document). A stream that the reader could not decode has to
be named like every other place that could not be looked into — otherwise
the pre-check of the redaction can never be complete.

---

### F-2 (medium) — A stream that cannot be read is classified as `hidden` instead of `uninspected`: the pre-check does not refuse, the document is changed and rolled back

**Where:** `engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c:247-253` — the
`fz_catch` of the callback reports the place with the kind **`hidden`**
("a stream that cannot be read cannot carry a word that is read: the
redaction reports the place as uncleaned"). The pre-check
(`core/vocument-core/src/redaction.rs:254-268`) only refuses on the kind
**`uninspected`**. A place that could not be read is therefore *not* part
of the completeness criterion of the pre-check.

**Method:** a document with a stream whose filter name does not exist
(`/Filter /NoSuchFilter`); the reader throws, the callback catches it. The
mock of the core counts the calls of the removal, so that it is visible
whether the pre-check refuses *before* the document is touched.

**Evidence:**

```
--- B: a stream whose filter is unknown (the reader throws) ---
warning: unknown filter name (NoSuchFilter)
  places reported: 2
    kind=content      page=0    len=26   location=content stream of the page
    kind=hidden       page=-1   len=12   location=/Root/Broken
```

The mock of the core (the counter is the number of the removals that were
attempted):

```
=== A: the walk could not write down every place ===
  the redaction was REFUSED — removals attempted: 0
=== B: the limit of the depth of the walk ===
  the redaction was REFUSED — removals attempted: 0
=== C: a stream that cannot be read (the kind `hidden`) ===
  the redaction was REFUSED — removals attempted: 1
  the error: … file bytes: the content is still there; the words that were removed: GEHEIM
```

The end-to-end run shows the same: the refusal comes from the **post**-check
("the text is still there: /Root/Broken (page -1, hidden)"), the document
was changed in the meantime and the rollback put the bytes back
(`byte-equal: true`).

**Result: FAIL** — the outcome is still a refusal (fail-safe), but the
pre-check is **not** the reference it claims to be: for a place that could
not be read, the document is changed first and the refusal comes late. The
error message of the pre-check ("the places of the document could not all
be examined before the removal") is not true for this class.

**What has to change:** a stream that cannot be read has to be reported
with the kind `uninspected` (it is a place that could not be examined, not
a place that carries the text) — or the pre-check has to refuse on every
place that the engine could not examine, not only on the kind
`uninspected`.

---

### F-3 (low) — The budget is a limit **per note**, not a budget of the notes; the name and the documentation promise more than the code keeps

**Where:** `engines/pdf-mupdf/csrc/mupdf_walk.h:174` —
`if (walk->note_budget > 0 && needed + 1 > walk->note_budget)`. `needed` is
the length of **one** note. The documentation of the field
(`mupdf_walk.h:100-106`) and of the function
(`engines/pdf-mupdf/src/scrub.rs:161-176`) speaks of "the budget of the
places that could not be examined" and of a walk that "may write as many
notes as it needs" for the budget zero — a reader takes it for a **total**
budget.

**Method:** the same document (a chain of 60 dictionaries) with different
budgets; the note of the document is 298 bytes long.

**Evidence:**

```
  budget=0      places=3 uninspected=1 -> ["the limit of the depth of the walk was reached\t/Root/Extra/N… (298 bytes)"]
  budget=1      places=3 uninspected=1 -> ["the walk could not write down every place that it could not … (67 bytes)"]
  budget=16     … (67 bytes)
  budget=64     … (67 bytes)
  budget=128    … (67 bytes)
  budget=256    … (67 bytes)
  budget=300    … (67 bytes)
  budget=512    places=3 uninspected=1 -> ["the limit of the depth of the walk was reached\t/Root/Extra/N… (298 bytes)"]
  budget=1024   … (298 bytes)
```

The threshold is at 301 bytes — the length of this one note plus its
terminator. A budget of 512 therefore allows an **unlimited** total of
notes.

**Result: FAIL (low)** — the budget cannot bound the memory of the notes.
Today only the test uses it, and the test only proves the path of the
budget; the two other paths that set the flag (the failed `realloc` at
`mupdf_walk.h:188` and the failed `snprintf` at `mupdf_walk.h:202`) are
covered by **no** test at all — they are read, not measured. The name and
the documentation should say "the longest note" (or the code should count
the bytes that were written).

---

### F-4 (low, latent) — The shortening of a too long line does not terminate the buffer: a read past the end of the stack buffer

**Where:** `engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c:373-376`:

```c
} else {
    memcpy(entry, line, sizeof(entry) - 4);
    memcpy(entry + sizeof(entry) - 4, "...", 4);
}
vocument_report_line(&report, "uninspected", -1, entry);
```

`entry` is `char entry[VOCUMENT_WALK_PATH_MAX + 128]` (384 bytes). The
branch writes 384 bytes and **no terminator**: `entry[383]` is `'.'`. The
`snprintf("%s")` of `vocument_report_line` then reads past the end of the
array.

**Method:** the length of the longest line of the report was measured over
the documents of the harness (a chain of 60 dictionaries, a path of 255
characters) and compared with the size of `entry`.

**Evidence:**

```
--- A: a chain that is deeper than the walk can follow ---
    kind=uninspected  page=-1   len=298   location=the limit of the depth of the walk was reached\t/Root/Extra/Next/…
  longest location: 298 bytes
--- E: a very long path through the dictionaries ---
    kind=hidden       page=-1   len=255   location=/Root/Extra/KKK…
  longest location: 255 bytes
```

The longest possible line is the longest reason (45 bytes:
`the limit of the depth of the walk was reached`) + a tab + the longest
path (255 bytes, the path of the walk is cut at `VOCUMENT_WALK_PATH_MAX`) =
**301 bytes** — the buffer holds 384. The branch is therefore **not
reachable today**.

**Result: FAIL (latent)** — dead code with a memory-safety defect in it.
The branch has to write a terminator (`entry[sizeof(entry) - 1] = '\0'`)
or has to be removed, because the day a longer reason or a longer path
appears, the product reads past the end of a stack buffer.

---

### F-5 (low) — The number of the places that were lost is not known: the report says "one place"

**Where:** `engines/pdf-mupdf/csrc/mupdf_occurrences_bridge.c:384-394` — the
flag becomes **one** line with a fixed text; the core
(`core/vocument-core/src/redaction.rs:259-266`) counts the **lines** of the
report and therefore announces the number of the lost places as the number
of the lines.

**Evidence:** with the budget of 1 byte the report holds exactly one place
of the kind `uninspected` — no matter how many notes were lost — and the
core answers:

```
the places of the document could not all be examined before the removal, so the removal
cannot be verified: 1 place(s) could not be looked into, among them the walk could not write
down every place that it could not examine
```

**Result: FAIL (low)** — the message states a number that is not true. The
flag stands for an unknown number of places. The message should say that
the number of the places is unknown.

---

## 3. The four questions, each with its measurement

### Question 1 — Can the walk still fail to examine a place without it becoming visible anywhere?

**Yes.** The paths of the walk, one by one:

| The path of the walk | The note | Measured |
|---|---|---|
| The limit of the depth | `uninspected` | 1 place, 298 bytes, the reason and the path are named — the report names it (`mupdf_walk.h:482`) |
| An object that cannot be read | `uninspected` | the code path exists (`mupdf_walk.h:587`), the reason is named |
| An object that cannot be addressed | `uninspected` | the code path exists (`mupdf_walk.h:608`) |
| **A note that cannot be written down** (the memory, `snprintf`, the budget) | **`uninspected` (the flag)** | **measured**: the budget of 1 byte → the report holds `the walk could not write down every place that it could not examine`; the core refuses the redaction with **0 removals attempted** |
| **A stream whose content cannot be decoded** | **none** | **F-1**: the report says nothing, the redaction is `verified = true` and the secret is in the file |
| A stream that cannot be read (the reader throws) | `hidden` | **F-2**: the pre-check does not refuse; the document is changed and rolled back |
| An object without a number | the object is walked, the path carries `?` | the path of the walk is cut at 255 bytes; the place is examined |
| A path that is too long | the place is named, the path is cut | the longest location measured: 298 bytes (the note), 255 bytes (the path) |
| A container (an archive) | `uninspected` | the place is named: `/Root/Names/EmbeddedFiles/Names[1]` |

### Question 2 — Is the pre-check really the reference, and does the document stay byte-equal when it refuses?

**Yes, for the kind `uninspected` — and the document stays byte-equal.**
**No, for the kind `hidden` (F-2).**

* The refusal happens **before** the removal: the mock of the core counts
  the removals — **0** for the line of the flag and for the line of the
  depth limit, **1** for the kind `hidden` and for a clean document.
* The exported file is byte-equal to the input in every refusal that the
  pre-check makes:

```
=== END TO END: A: a chain that is deeper than the walk can follow ===
  the redaction was REFUSED: … the places of the document could not all be examined before the
  removal, so the removal cannot be verified: 1 place(s) could not be looked into, among them the
  limit of the depth of the walk was reached…
  exported bytes: 3898  input bytes: 3898  byte-equal: true

=== END TO END: D: an attached archive (a container) ===
  the redaction was REFUSED: … 1 place(s) could not be looked into, among them
  /Root/Names/EmbeddedFiles/Names[1]
  exported bytes: 768  input bytes: 768  byte-equal: true
```

* The reference is used as a reference **only** for the completeness of the
  pre-check. The comparison of the places before and after the removal is
  not made by the code: the post-check works on the state after the
  removal alone (`redaction.rs:484` onwards). That is the design of the
  fifth round and it is not touched by this change — but the pre-check
  cannot be called "the reference of the verification" as long as nothing
  compares with it.

### Question 3 — Is there a way in which a report looks complete although it is not?

**For the report of the places: no.** The three ways of losing a place all
end in an **error**, never in a partial report:

| The way | Measured |
|---|---|
| The buffer of the report is too small | 50,000 annotations → **50,001** places reported; 99,000 → **99,001**; the buffer grows (16 kB → 64 kB → 256 kB → 1 MB → 4 MB → 8 MB) |
| More places than the limit of the bridge (100,000) | 100,001 annotations → **ERROR**: `the list of the places of the document did not fit into the buffer of the report` (10.7 s) — not a partial report |
| Long paths (the 8 MB buffer) | 90,000 chained annotations → the same **ERROR** |

The cut-off point of 200 lines of the round before is gone: the constant
`MAX_OCCURRENCES: usize = 200` (`engines/pdf-mupdf/src/scrub.rs:24`) is
**no longer used anywhere** in the repository — a grep over the whole tree
finds only the definition. It is a trap for the next reader and should be
removed or renamed.

**For the report of the redaction: yes, in one case** — F-1: the report
holds `uninspected_places = 0` and `not_checked_reasons = []` although a
place of the document was never examined.

The text protocol of the report cannot be broken by the document: a name of
a key may not carry a line break or a tab — MuPDF rejects such a key
(`syntax error: invalid key in dict`, measured with a line break and with a
tab alone). The place of a note is therefore always one line, and the
`kind` of a line cannot be forged by the document.

### Question 4 — Is the new error path reproducible?

**Yes.** Three independent ways:

1. The test of the project: `scrub_hardening_tests::a_place_that_cannot_be_
   written_down_is_not_lost_silently` — **green** (the suite of the engine:
   60 tests, 0 failed).
2. The harness of the reviewer: the budget probe (the table of F-3) and the
   mock of the core, which reproduces the **exact** line that the bridge
   writes for the flag (`the walk could not write down every place that it
   could not examine`) and measures the effect on the core: the redaction is
   refused, **0 removals attempted**, the document byte-equal.
3. The end-to-end run with a real document: a chain of 60 dictionaries → the
   pre-check refuses, the exported file is byte-equal (3898 == 3898).

---

## 4. What is not a finding

* **The note path of the change is correct.** All three ways in which a note
  can fail (`realloc`, `snprintf`, the budget) set `note_failed`
  (`mupdf_walk.h:175`, `:188`, `:202`), the flag becomes a place of the kind
  `uninspected` (`mupdf_occurrences_bridge.c:384`), and the core refuses
  the redaction before the removal (measured: 0 removals, byte-equal).
* **The report is not cut off any more.** The buffer grows, the limit of
  100,000 places ends in an error, and the old limit of 200 lines is dead
  (the constant is unused).
* **The document stays byte-equal when the pre-check refuses.** Measured
  for the deep chain (3898 == 3898) and for the attached archive
  (768 == 768); the mock shows that the removal is not even attempted.
* **The flag is set on every path of the note.** The code was read line by
  line; the paths of the `realloc` and of the `snprintf` are not measured
  (F-3), but they set the flag.
* **A path that is too long does not lose a place.** The path of the walk is
  cut at 255 bytes, the place is still named, and the longest line of the
  report (301 bytes in the worst case) fits into the buffer of the bridge
  (384 bytes) — the shortening branch is not reached (F-4 is latent, not
  live).
* **The document cannot forge a line of the report.** A key with a line
  break is rejected by MuPDF (`syntax error: invalid key in dict`), and a
  key with a tab alone as well — measured.
* **The refusal of a document with an attached archive and of a document
  with a nesting depth above 48 is not a regression.** Both were already
  refused by the post-check (`is_verified()` is false when
  `uninspected_places` is not empty, `core/vocument-core/src/redaction_report.rs:146`);
  the pre-check only refuses them **earlier** and without touching the
  document. It is worth a line in the documentation, because a document with
  49 nested dictionaries can no longer be redacted at all — even when the
  sweep over the cross-reference table examined every object.
* **The suite of the project is green.** `cargo test --workspace`:
  **258 passed, 0 failed, 0 ignored** (33 test binaries), `cargo fmt --all
  -- --check` clean, `cargo clippy --workspace --all-targets` clean. The
  engine's own suite: 60 tests, 0 failed.
* **The reviewer changed no file of the repository and committed nothing.**
  The harness lives outside the repository
  (`…/plan_469c9e35/workspace/harness`), the build artefacts in
  `target/review-r7`.

---

## 5. The reproduction

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR=/workspace/vocument/target/review-r7
export MUPDF_DIR=/workspace/vocument/target/mupdf-host
export TESSDATA_PREFIX=/workspace/vocument/apps/android/app/src/main/assets/tessdata
export LD_LIBRARY_PATH=$MUPDF_DIR/lib
cd /workspace/vocument && cargo test --workspace          # 258 passed, 0 failed
cd …/plan_469c9e35/workspace/harness && cargo run --bin review-harness   # the probes 1-5
cd …/plan_469c9e35/workspace/harness && cargo run --bin mock            # the pre-check of the core
cd …/plan_469c9e35/workspace/harness && cargo run --bin review-harness -- bulk  # the limit of 100,000 places
```

The documents of the harness are built by hand
(`…/workspace/harness/src/pdfbuild.rs`), so that the structure of the file
is known exactly:

| The document | The place it tests |
|---|---|
| `document_with_a_deep_chain` | the limit of the depth of the walk |
| `document_with_an_unknown_filter` | a stream that cannot be read (the kind `hidden`) |
| `document_with_an_undecodable_attachment` | a stream whose content cannot be decoded (**F-1**) |
| `document_with_a_decodable_attachment` | the control of F-1 |
| `document_with_a_truncated_attachment` | a stream that ends too early (the reader returns the data) |
| `document_with_a_corrupt_attachment` | a stream whose checksum is wrong |
| `document_with_a_container` | an archive that cannot be looked into |
| `document_with_a_long_path` | a path of 255 bytes |
| `document_with_a_breaker_in_the_name` | a name with a line break (rejected by MuPDF) |
| `document_with_flat_places` | 50,000 … 100,001 places (the limit of the report) |

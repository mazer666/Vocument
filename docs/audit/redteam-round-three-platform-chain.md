# Red Team III — the chain from the call to the application

- **Date:** 2026-10-20
- **Checked stand:** `main` at `67509f0`, working tree clean.
- **Method:** the code of the chain was read and the exported UniFFI ABI was measured with a C
  program against the built library (`target/debug/libvocument_ffi.so` of 21:36) — **no cargo**,
  the target directory of the repository belongs to another process. In addition the bindings
  were generated again with `uniffi-bindgen` 0.29.5 from the built library and compared with the
  checked-in ones.
- **Raw report of the checker** (in German, with the full measurements):
  `target/audit/redteam-platform-chain.md`. This document is the English record.

---

## 1. The answer in one sentence

**No — a redaction that could not be verified cannot reach the user as a success.** There is no
return path on which a report with a check that did not pass, or that was not carried out at
all, gets through: the core rolls the document back and reports an error, the FFI reports
`RedactionUnverified`, and the Swift client and the Kotlin bridge discard the report a second
time. The checker measured this on the ABI (at an OCR failure the document stays byte-identical,
590 → 590 bytes).

**But the user can still believe that a redaction succeeded while it did not** — on two paths
that lie *outside* the verification:

1. **The word stands somewhere else in the document.** The redaction is reported as verified
   (`verified = true`, all four checks `Passed`), the secret is still in the document — on
   another page. The report names the place (`remaining_occurrences`), **no caller passes it on
   to the user**, and there is no redaction surface that could (R3-1).
2. **After a crash the redaction is silently dropped.** "Restore" says so (measured), **"Preview"
   and "Open as new" do not** — and "Open as new" writes a new file that contains the secret
   (R3-2).

The verification cannot be defeated; the *notification* of the user can.

## 2. The findings

| # | Severity | Finding | Status |
|---|---|---|---|
| R3-1 | **high** | The success path does not tell the user that the word still stands somewhere else in the document | **partly fixed** — the callers turn the places into warnings; the surface is still missing |
| R3-2 | **high** | "Preview" and "Open as new" do not mention that the redaction was dropped; "Open as new" writes a file with the secret | **fixed** |
| R3-3 | medium | The revision after the recovery is the one of the journal, not the one of the state that was applied | **fixed** |
| R3-4 | medium | Old checkpoints are not made private — the mode is only set when the file is created | **fixed** |
| R3-5 | medium | The root directory of the recovery store lies in the world-writable `/tmp` and is `0755` | **fixed** (the root is `0700`); the place itself is a documented limit |
| R3-6 | medium | The errors of the recovery write are discarded (`let _ =`) | **fixed** — they are reported, not discarded |
| R3-7 | medium | The instruction of `RedactionRollbackFailed` only stands in the Rust message, not in the fields the bindings show | **fixed** |
| R3-8 | medium | The clipboard of the user is not cleared after a redaction | **open** — it needs the redaction surface |
| R3-9 | low | The proof over the picture can pass empty: an OCR that reads nothing cannot be told apart from a clean page | **open, documented** |
| R3-10 | low | The second line of defence of the callers is dead code and throws a foreign error type | **fixed** |
| R3-11 | low | The rendered tiles with the removed text stay in the tile cache of the process | **fixed** |
| R3-12 | low | In the case of an error the report is lost; only the error text remains | **fixed** |
| R3-13 | low | Until the document is saved, the secret is still in the file of the user | **unchanged, documented** — it is the promise of M0.6 |

## 3. The findings in detail

### R3-1 (high) — the success path swallows the places

The core removes what the user marked and reports the places where the same word still stands:

```rust
// core/vocument-core/src/redaction.rs
let outside: Vec<&PdfTextOccurrence> = occurrences.iter().filter(|occurrence| {
    occurrence.kind == "content" && occurrence.page_index != page_index as i32
}).collect();
remaining_occurrences = outside.iter().map(|occurrence| (*occurrence).clone()).collect();
```

Measured (a document of two pages, the word on both, the redaction on the first page):

```
  [redaction_seite0] OK
            remaining_occurrences=1
              - kind=content page_index=1 location=content stream
    Report: text_layer_clean=Passed search_index_clean=Passed rendered_page_clean=Passed
            file_bytes_clean=Passed
    verified=1
    Secret nach der Redaction von Seite 0 noch im Dokument: JA
```

The data travels through the whole chain — `FfiRedactionReport.remainingOccurrences`, Swift
`remainingOccurrences`, Kotlin `remainingOccurrences` — and **no caller hands it on**: in both
applications there is no redaction surface at all (`grep -rni "redact" apps/macos/… apps/android/…`
finds nothing outside the bridge).

**Fix.** Both callers now turn the places into warnings that the shell has to show:
`DocumentCoreClient.redactionWarnings(_:)` (Swift) and `redactionWarnings(report)` (Kotlin).
The error case names the places in the error text as well, so that the information is not lost
(R3-12).

**What is still open.** There is no redaction surface: the warning has an addressee in the
library, but nobody shows it to a user yet. That is the missing step of the milestone and it is
written down as such.

### R3-2 (high) — "Preview" and "Open as new" hide the dropped redaction

```
-- A6: Recovery-Aktion 'Restore' --
    message="Document session restored from crash recovery journal, but the last redaction was
      not replayed: a redaction cannot be verified from the journal, and the document stands at
      the state before it. Original file on disk remains unchanged until save."
-- A7: Recovery-Aktion 'Preview' --
    message="Preview session created. Recovery container and on-disk file are unchanged."
-- A8: Recovery-Aktion 'OpenAsNew' --
    message="Recovered state saved as new document. Original file and recovery container cleaned up."
    Neue Datei ./probe1 (Recovered 3).pdf: 586 Bytes | Secret in der neuen Datei: JA
```

**Fix.** All three actions carry the same note now (`recovery.rs`,
`redaction_not_replayed_note`). "Open as new" says in addition that the new file still carries
the text that was to be removed.

### R3-3 (medium) — the revision after the recovery

The recovery skipped the redaction but took the revision of the journal unchanged: the document
claimed a state it never reached (measured: `revision=2`, `undo_count=0`).

**Fix.** `restore_and_replay_journal` subtracts the redactions that were not replayed from the
revision (`transactions.rs`).

### R3-4 (medium) — the old checkpoints stay readable

A checkpoint of an older version of the application kept the mode `0644` — readable for
everybody on the machine — although it holds the document in plain text.

**Fix.** Every checkpoint is made private when the next one is written
(`enforce_private_checkpoint_files`).

### R3-5 (medium) — the recovery store in the world-writable temporary directory

```
drwxr-xr-x 755 /tmp/vocument_recovery
drwx------ 700 /tmp/vocument_recovery/doc_2fb09c2b450c3699
-rw------- 600 /tmp/vocument_recovery/doc_2fb09c2b450c3699/journal.json
```

The content was protected, but the root directory showed which documents are recoverable, and a
stranger could prepare the directory before the application does.

**Fix.** The root directory of the store is created with `0700` as well.

### R3-6 (medium) — the errors of the recovery write were discarded

All four call sites used `let _ =`. If the container could not be written, the session went on
without a trace: after a crash there was nothing to recover, and nobody was warned.

**Fix.** The errors are reported (`session_mutations.rs`, `redaction.rs`).

### R3-7 (medium) — the instruction of the rollback failure was lost

The bindings show the **fields** of an error, not the message of the Rust error. The sentence
"the document has to be opened again" was therefore nowhere to be found.

**Fix.** The sentence travels in the fields now (`platform/vocument-ffi/src/lib.rs`).

### R3-8 (medium) — the clipboard of the user

Both applications copy the text of the page into the clipboard. After a redaction the clipboard
still holds the removed text — and the clipboard history of the system keeps it.

**Status: open.** The clearing belongs to the redaction surface, which does not exist yet. It
is written down as an open point of the milestone; the *place* of the change is named
(`VocumentMacApp.swift`, `NavigationAndSearchPanel.kt`).

### R3-9 (low) — the proof over the picture can pass empty

The check "nothing is readable on the picture of the page" hangs on the OCR. The measurements ran
with a stub that reads no words — and the check passed anyway. An empty OCR result and a
checked, clean page are not distinguishable for the chain.

**Status: open, documented.** The report does not carry the number of words the OCR read; the
limit is written down in the acceptance record. The error case of the OCR is handled correctly
(measured: the document stays byte-identical, the error is `RedactionUnverified` and carries the
whole text in the field).

### R3-10 (low) — the second line of defence of the callers

The core returns `Ok` only if all four checks passed, so the check of the callers can never
fire. Both threw an error of their own type — a shell that catches only the error of the
boundary would have swallowed the refusal.

**Fix.** Both throw the error of the boundary itself (`DocumentCoreError.RedactionUnverified` /
`DocumentCoreException.RedactionUnverified`).

### R3-11 (low) — the tiles in the cache

The tiles of the earlier revision still showed the removed text and stayed in the cache of the
process until the cache evicted them.

**Fix.** The tiles and the thumbnails of the document are dropped after a redaction
(`TileCache::purge_document`, `ThumbnailCache::purge_document`).

### R3-12 (low) — the report is lost in the error case

The FFI returned `Err` and threw the report away. The information did not get lost completely
(the core writes the failed checks and the uncleaned places into the error text), but the
removed words and the remaining places were not in it.

**Fix.** The error text now carries the words that were removed, the places where the same text
still stands and the places that could not be read.

### R3-13 (low) — the secret stays in the file until it is saved

Until the user saves, the file of the user still holds the secret. That is the promise of M0.6
("the disk file remains completely intact until explicit user save") and not a defect — but a
redaction only becomes irreversible with the save, and the user has to know that.

**Status: unchanged, documented.**

## 4. What the checker could **not** check

1. **No surface.** There is no macOS host and no Android device: no click, no screenshot, no
   bed path. Only the library layer could be checked — in particular whether
   `remaining_occurrences` and the message of the recovery arrive in a surface is **not
   provable**: there is none.
2. **No real Tesseract.** The system has no Tesseract library; the measurements ran with a stub
   (one that reads no words, one whose initialisation fails). **The behaviour of a real OCR on
   this chain is not proven.**
3. **No scan.** The proof over the picture of the page was only measured on a text PDF.
4. **No real kill.** The crash was simulated by not closing the session; the moment of the kill
   in the middle of the redaction (between the removal and the verification) was not reproduced.
5. **The rollback path after a change was not measured.** It was read in the code, not measured;
   `RedactionRollbackFailed` was not observed at runtime.
6. **The first series of measurements ran against an older library** (21:12, before the last
   commit). All numbers in this report come from the rebuild (21:36).
7. **No run of the test suite** by the checker: the numbers come from the code and from the
   measurements of the checker, not from a run of the project.
8. **The macOS and Android targets were not built** by the checker (no macOS, no NDK run).
9. **Concurrency.** Whether a redaction is consistent while a background job runs was not checked.

## 5. The bindings

The checker generated the bindings again from the built library and compared them with the
checked-in ones:

```
$ ./target/debug/uniffi-bindgen generate --library target/debug/libvocument_ffi.so --language swift --out-dir gen/swift --no-format
$ diff gen/swift/vocument_ffi.swift       apps/macos/Sources/VocumentFFI/vocument_ffi.swift          → identisch
$ diff gen/swift/vocument_ffiFFI.h        apps/macos/Sources/VocumentFFIHeaders/include/vocument_ffiFFI.h → identisch
$ diff gen/kotlin/uniffi/vocument_ffi/vocument_ffi.kt apps/android/.../vocument_ffi.kt              → identisch
```

All 75 exported functions are in all three bindings, the order of the 15 variants of
`DocumentCoreError` is identical in Rust, Swift and Kotlin, and the status values
(`Passed/Failed/NotChecked`, `Restore/Preview/Discard/OpenAsNew`) are identical as well.

**After the fix of this round** the report carries one more field (`uninspectedPlaces`) — the
bindings were generated again and the comparison is part of the gate.

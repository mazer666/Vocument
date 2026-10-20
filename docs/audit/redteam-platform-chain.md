# Red Team II — the chain from the call to the app

- **Date:** 2026-10-20
- **Tested state:** `main` at `f59c262` and the changes in the working tree
  of the same day
- **Method:** the code of the chain was read, and the exported UniFFI ABI was
  measured with an own C program (`redteam-platform-chain-harness.c`) that
  calls the exported functions directly — no cargo, so that the target
  directory of the repository stayed untouched. `mutool` and the generated
  bindings served as the reference.
- **Original wording:** the auditors worked in German. This document is the
  English account of their findings; the code positions and the measurements
  are theirs.

## 1. The answer in one sentence

**No — a user cannot hold a redaction for successful that is not one.** That
was measured against the current state:

- A redaction that covers only the half of a word is **refused**:
  `RedactionUnverified`, `details="text layer: the content is still there"`.
- The document is **byte-identical** with the original afterwards (590 → 590
  bytes), the secret is in the document again and the transaction has left
  the undo stack. The rollback works.
- `NotChecked` never becomes `Passed`: the core treats it as a failure,
  rolls back and reports an error. A report with `verified = false` never
  reaches the caller.

**But the chain had three real gaps:**

1. **Errors after the mutation broke out without a rollback.** The caller got
   an error — and the document was **changed**. The user holds a changed
   document and believes it is the old one (B2, critical).
2. **The chain ends in no app at all.** Neither the macOS nor the Android
   application calls `document_core_redact_page_regions`; there is no
   redaction interface, no error path, no clearing of the clipboard. The
   redaction is reachable today only through the generated bindings (B1,
   critical).
3. **The recovery path does not check.** A crash in the middle of a
   redaction leaves a journal with the redaction entry; "restore" applies
   the redaction **without verification** to the document from the disk (B4,
   high).

The report itself has meanwhile become more honest (it names the places
where the same word still stands), but it is not yet **load bearing**: the
field `verified` is **always** `true` (it is the only value the caller can
see), the report is lost in the case of an error, and no field carries the
decision "the document is not the original any more" (B7, medium).

## 2. The findings

| # | Severity | Finding |
|---|---|---|
| B1 | critical | The redaction cannot be called by **any** platform — the chain ends in the generated code |
| B2 | critical | Errors **after** the mutation break out without a rollback: the caller gets an error, the document is changed |
| B3 | high | The rollback itself can fail; the user is not informed and the journal keeps the redaction |
| B4 | high | The recovery path applies a redaction **without verification** to the document from the disk |
| B5 | high | The redaction cannot be carried out without OCR — and the error is an "internal core failure" |
| B6 | high | The checkpoints of the recovery hold the document in plain text and are **readable for everybody** (0644) |
| B7 | medium | The report does not carry the decision: `verified` is always `true`, in the case of an error the report is lost, the state fields are missing |
| B8 | medium | The error texts of the bindings lose their meaning (Kotlin: both redaction errors "details=…"; the `thiserror` texts do not cross the boundary) |
| B9 | medium | The clipboard of the user (macOS/Android) is not cleared after a redaction |
| B10 | medium | After a rollback the document stays marked as "changed" (revision/dirty) although the bytes are identical — **measured** |
| B11 | medium | If the deletion of the checkpoints fails, a **successfully verified** redaction is reported as an error |
| B12 | medium | The macOS app binds a **debug artefact without a version binding** — the evidence is therefore not reproducible |
| B13 | low | The recovery store lies in `std::env::temp_dir()`; errors are silently discarded |
| B14 | low | The redaction blocks the undo stack: other operations cannot be undone afterwards either |
| B15 | low | The search is empty after a restart — the redaction then "checks" an empty index |

### What was fixed

| # | What |
|---|---|
| B1 | The macOS client and the Android bridge now call the redaction and **check the report themselves**: the `verified` flag is not trusted, the four checks are looked at, and a check that was not carried out is refused (`redactPageRegions` / `requireVerifiedRedaction`). The step of the interface — the user marks the region and sees the report — belongs to the macOS slice of M2 and cannot be built here |
| B2 | Every error after the removal puts the document back: the text extraction, the reindexing, the rendering of the page and the reading of the bytes. The rollback happens in one place (`fail_after_change`) |
| B3 | A rollback that fails itself is reported as its own error (`RedactionRollbackFailed`): the session then holds a document in an unknown state, and the user is told to open the document again |
| B4 | The recovery does **not** replay a redaction: the journal cannot say whether it was ever verified, and the document is restored to the state before it. The message of the recovery says so |
| B6 | The checkpoints of the recovery store belong to the user alone: the directory is `0700`, the file `0600` |
| B8 | The details of the error carry the whole text, so that the meaning survives the boundary |
| B10 | After a rollback the revision goes back to the value of the transaction before the attempt |
| B11 | A checkpoint that could not be deleted does not turn a successful redaction into an error: the content is gone, and the verification has said so |

### What stays open

| # | Why |
|---|---|
| B5 | The redaction needs the OCR for the check of the picture. Without it the check is `NotChecked` — and the redaction is refused. The error now says that the picture could not be checked instead of "internal core failure". The OCR of the host is not the OCR of a device |
| B7 | The report loses the state in the case of an error. The details of the error carry the checks and the places; a field that survives the error would be a change of the public API |
| B9 | The clipboard of the user is not cleared. That is a task of the app — the app does not have the redaction in its interface yet |
| B12 | The macOS app binds a debug artefact. The build of the macOS slice is not possible here |
| B13 | The recovery store lies in the temporary directory of the system. The errors are not discarded any more, they are reported |
| B14 | A redaction cannot be undone — that is the promise. The other operations behind it in the stack cannot be undone either; that is the price of the irreversibility |
| B15 | The search index is built again when the document is opened. The redaction checks the index of the page it has just reindexed |

## 3. What could not be checked

- **No UI run:** no macOS host, no Android device, no screenshot, no click.
- **The generated bindings were compared, not built for the platform:** the
  `uniffi-bindgen` was run against the built library and the result was
  compared with the checked-in Swift bindings, the Kotlin bindings and the C
  header — **no differences**. The bindings of the platform build itself
  could not be checked.
- **The OCR was a stub:** the measurements ran with an empty OCR result (and
  once with a failing OCR). The behaviour of a real OCR is therefore not
  proven on this path.
- **The interruption during a redaction was not reproduced by a real kill:**
  the path was read, and the recovery was checked with a journal that
  contains a redaction.

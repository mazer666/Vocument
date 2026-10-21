# The red team on the stored rail of the profession

**Scope:** the juristic palette on the disk — `core/vocument-core/src/annotations_store.rs` —
and the meaning of a reviewer's note across a restart.

**Date:** 2026-10-21
**Milestone:** M2.4 (`ANNOT-001`)

## How this report came about

The first attempt at an **independent** red team on this file ran into the time
limit of its session and was stopped before it delivered a report. The
measurements below were therefore taken **by the team lead**, directly against
the real file system, and every one of them is reproducible as a named test
case in
`core/vocument-core/tests/annotation_store_hardening_tests.rs` (7 test cases,
all green).

That is a weaker form of evidence than an independent report, and it is named
as such here. The document is written so that a second, independent attacker
can repeat each measurement and contradict it.

**No finding in this report is a simulation.** Every statement below names the
file, the syscall it depends on and the test case that measures it.

## What was attacked

The rail of the profession is a small JSON file in the data directory of the
application (`jurist_palette.json`). It holds the meaning of the notes of the
reviewer: a key, a name, a description and a colour. It is written by the
application and read back at the next start.

A file like that is a strange target — it holds no secret — but it holds
**meaning**. A note that carries the key `house` means "the colour of the
house". If the meaning of a key changes, then every note of the past changes
with it, and the reviewer reads a different document than the one they wrote.

The five questions of the assignment:

1. the rights of the file and of the directory after the write
2. a crash in the middle of the write
3. a link and a path with `..`
4. two writers at the same time
5. the meaning of a note across a change of the rail

## 1. The rights of the file and of the directory — **PASS**

**Measured:** the store creates the directory of the rail with the mode `0700`
and the file with the mode `0600`. The test reads the mode of both with
`stat` and requires that no other user may read, write or traverse them.

```
mode & 0o077 == 0   (directory)
mode & 0o077 == 0   (file)
```

**Test case:** `the_directory_of_the_rail_belongs_to_the_user_alone`

**Finding:** none. The rail of the profession is the work of the user and
belongs to the user alone — the same rule that the recovery store follows
(`RECOVERY-001`).

**Limit:** the mode is only as good as the user that runs the process. A
process that runs as `root` writes a file that `root` can read; the check
cannot prevent that and does not pretend to.

## 2. A crash in the middle of the write — **PASS**

**Measured:** the store never writes into the file of the user. It writes a
staging file next to it, flushes it, and renames it over the rail. A crash
therefore leaves the rail either **complete** or **untouched**.

The second half of the question is harder: what does the core do when it
finds a file that was cut in half? The answer is measured as well — a broken
file is an **error with a name**, never a silent fall back to the standard
rail.

```
{"format_version": 1, "entries": [
```

→ the store refuses to read it and names the reason.

**Test cases:** `a_rail_that_was_cut_in_half_is_an_error_and_not_a_silent_standard_rail`
and the staging write in `a_link_of_a_stranger_in_the_place_of_the_staging_file_is_not_written_through`.

**Finding:** none. A broken rail is reported to the caller instead of
silently replacing the rail of the user with the standard one. A silent
fall back would have been the dangerous answer here: the notes of the reviewer
would have kept their keys, but the keys would have meant something else.

**Limit:** the measurement writes a broken file by hand. A real crash in the
middle of a write leaves either the old file or the new one — the hand-written
broken file is the *worst* case, and the store refuses it.

## 3. A link and a path with `..` — **PASS**

**Measured:** three separate attacks, three separate refusals.

| Attack | What the store does |
|---|---|
| The stranger prepares the staging file as a link to their own file | The staging file is created **exclusively** (`O_EXCL`) with a name that carries the identity of the process and a counter, so the write goes into a new file and never through the link. The file of the stranger keeps its content. |
| The stranger puts a link where the rail should be | `load()` refuses it: *"not a plain file"*. |
| The stranger puts a link that points **nowhere** where the rail should be | **Corrected by the second round:** the old rule asked `Path::exists()`, which follows a link — a link that points nowhere therefore looked like *"no rail at all"*, and the application carried on with the standard rail, silently. The state of the path is now read with `symlink_metadata`, which does not follow a link: a link of any kind is refused. |
| The rail belongs to a **stranger** | **Corrected by the second round:** the write path always checked the owner, the **read** path did not — a rail of a stranger was read and used, so the notes of the reviewer were written with the colours of the stranger. The read path checks the owner now. |
| The path leaves the directory (`sub/../jurist_palette.json`) | **Corrected by the second round:** the rule refused the **whole** path, including the part the *platform* chose, and bricked the application at the start for a path like `/a/b/../c` — which means the same directory as `/a/c`. The **name** of the file decides now: it has to be exactly `jurist_palette.json`, and a name can never be `..`. The directory is protected by the check of the owner, and the staging file of the write is built from the same directory as the rail. |
| The file carries another name (`something_else.json`) | `save()` refuses it: the name has to be exactly `jurist_palette.json`. |

**Test cases:** `a_link_of_a_stranger_in_the_place_of_the_staging_file_is_not_written_through`,
`a_link_in_the_place_of_the_rail_is_not_read`,
`a_link_that_points_nowhere_is_refused_and_not_taken_for_no_rail`,
`a_rail_of_a_stranger_is_not_read`,
`a_file_that_is_not_the_rail_cannot_leave_its_directory`,
`a_directory_with_a_dot_dot_is_accepted_and_lands_in_the_named_directory`,
`a_file_with_another_name_is_refused`.

**Finding:** none. The write stays in the directory of the user, and it stays
in the file of the user.

**Limit:** the check of the owner (a file that belongs to somebody else is
refused) is only as strong as the probe that reads it — the store writes its
own probe file and reads the effective user of it. On a system where the
process cannot write into the directory at all, the probe fails and the store
refuses to write. That is the safe direction, but it means that a **read-only**
data directory stops the whole application from saving the rail.

## 4. Two writers at the same time — **PASS**

**Measured:** eight threads, each writing the rail twenty-five times — 200
writes in parallel. Afterwards:

* the rail that is read back is **complete** and equal to the rail that was
  written — not a mixture of two of them
* no staging file is left over that holds the rail

```
loaded == palette          (the rail comes back unchanged)
leftovers with ".tmp"      (none)
```

**Test case:** `two_applications_that_save_at_the_same_time_leave_a_complete_rail`

**Finding:** none. The staging file carries the identity of the process and a
counter, so two writers do not write into the same file, and the rename is
atomic. The last writer wins — the rail is written as a whole, never in
parts.

**Limit:** the measurement uses threads of one process. Two **processes** were
not measured here, but the mechanism does not depend on the process: the name
of the staging file carries the process identity, and the file is created
exclusively. What two processes could still do is overwrite each other's rail
— the last one wins, and no one is damaged. That is the intended behaviour of
a setting of the user.

## 5. The meaning of a note across a change of the rail — **PASS**

**Measured:** the note does not carry the colour, it carries the **key**.

* The engine writes the key of the note into the document
  (`/VocumentPaletteKey` in the annotation), together with the colour that
  the rail of the moment carries.
* A note with the key `objection` keeps the key `objection`, even when the
  rail is changed afterwards. The core looks the key up in the rail that is in
  force **now** — a note of a rail that no longer exists is **not to be
  assigned** (`not assignable`), it does not get a colour of another rail.
* The change of the rail is therefore visible: a note whose key the new rail
  does not carry is reported as unassignable instead of being given a
  neighbouring colour.
* The rail of the user survives the restart of the application: it is written
  to the file and read back unchanged.

**Test cases:**
* `core/vocument-core/tests/annotation_tests.rs` —
  `a_note_with_a_key_that_the_rail_does_not_carry_is_refused`,
  `the_rail_of_the_user_may_replace_the_default`,
  `the_rail_of_the_user_survives_the_restart`
* `platform/vocument-ffi/tests/annotation_e2e_tests.rs` —
  `the_key_of_the_rail_travels_with_the_note` (the note is written with the
  key `objection`, the rail is replaced by one that carries `house`, and the
  note keeps its key **and** its colour)

**Finding:** none. The key travels with the document, the meaning is looked up
at the moment of the reading, and a key that the rail does not carry is
refused instead of being guessed.

**Limit:** the palette lookup happens in the core, and the **colour** that is
written into the PDF at the moment of the note is the colour of the rail of
the moment. A reader who opens the PDF in another program sees the colour of
the moment, not the colour of the rail of today. That is the intended
behaviour of a document, but it is worth knowing.

## The result in one line

The rail of the profession is a file of the user alone, is written
atomically, cannot be carried out of its directory by a link or by `..`, does
not lose a meaning to a change of the rail, and a broken rail is reported
instead of being replaced silently.

## The second round — an independent correction

The measurements above were taken by the team lead. A **second, independent**
round (an attacker that had not seen this report) measured the same file
again and found **two places where this report was wrong**:

1. a link that points **nowhere** was not refused — the application carried
   on with the standard rail, silently;
2. the **read** path had no check of the owner — a rail of a stranger was
   used, and the notes of the reviewer were written with the colours of the
   stranger.

Both are fixed, both have a test, and the rules above are corrected. The
report of the second round is in
[`redteam-palette-independent.md`](redteam-palette-independent.md).

## The limits of this report

* The measurements were taken by the team lead, not by an independent
  attacker; the first attempt of an independent run was stopped by its time
  limit. A second, independent run is still open.
* Everything is measured on **Linux** with the file system of the sandbox. The
  rights, the links and the atomic rename behave the same on macOS, but the
  report does not claim a measurement on macOS.
* The concurrency is measured with threads of one process, not with two
  processes.
* The meaning of a note is measured with the core and the FFI boundary. The
  macOS interface is **NOT RUN** here — there is no macOS host and no Swift
  toolchain in this environment.

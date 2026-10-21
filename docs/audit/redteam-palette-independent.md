# The second, independent review of the palette store

- **Date:** 2026-10-21
- **Subject:** `core/vocument-core/src/annotations_store.rs` — the rail of the profession
  (the palette of the reviewer) and its storage in the data directory of the user.
- **Why a second round:** the first report
  ([`redteam-palette-and-storage.md`](redteam-palette-and-storage.md)) was measured by the team
  lead. A round that is measured by the same person who wrote the code is worth less than a
  round that is measured by somebody who did not see the first report. This round was run
  **independently**: the reviewer read the sources and the first report afterwards, built a
  harness of its own against the real `libvocument_ffi.so` (over the UniFFI C ABI, with the real
  Tesseract of the project), and measured five points.
- **Raw evidence:** the harness and its working directories are in `/workspace/redteam-palette/`
  (`harness.c`, `m4_two_writers.sh`, `watcher.py`, `work/d2`, `work/d3`, `work/d4`). The report
  was written by the team lead from the raw measurements of the run, because the run reached the
  limit of its runtime before it wrote its own summary. **Every number below comes from the raw
  artifacts of the independent run.**

---

## 1. The answer in one sentence

**The first report was wrong in three places, and all three matter:** a link that points *nowhere*
was taken for "no rail at all" — and the application carried on with the standard rail, silently;
the **read** path did not check the owner of the file, so a rail that belonged to a stranger was
read and used; and the refusal of a path with `..` was applied to the whole path, including the
part that the *platform* chose — a legitimate path like `/a/b/../c` bricked the application at
the start.

| # | Severity | Finding | State |
|---|---|---|---|
| **M3b** | **medium** | A **link that points nowhere** was not refused. The rule asked `Path::exists()`, which follows a link: the application carried on with the **standard rail**, silently — the notes of the reviewer meant something else than the user thinks. | **fixed** |
| **M3f** | **medium** | The **read** path had no check of the owner. A rail that belonged to a stranger was read and used: the notes of the reviewer were written with the **colours of the stranger**, and the meaning of the document changed silently. | **fixed** |
| **M3d** | low | The refusal of a path with `..` was applied to the **whole** path, including the part the *platform* chose. A path like `/a/b/../c` — which means the same directory as `/a/c` — bricked the application at the start. | **fixed** |

Two more points were measured and are **not** findings:

| # | Point | Result |
|---|---|---|
| M3g | A write into a directory of a stranger | **refused** — the check of the owner works on the write path (it always did) |
| M4 | Eight processes write the rail at the same instant | the rail on the disk was **always complete**: 5 900 144 samples of a watcher, **not one** torn or mixed state |

---

## 2. M3b — the link that points nowhere

**The measurement.** The reviewer put a link in the place of the rail. The link pointed at a file
that does not exist:

```
$ ln -s /tmp/nirgendwo/keine_datei.json <datenverzeichnis>/jurist_palette.json
$ harness configure <datenverzeichnis>   →  configure: OK
```

**What happened.** The application did **not** report an error. It carried on with the
**standard rail** — the built-in palette — and the user was never told. The first report claimed
that *"a link in the place of the rail is refused"*. That is true for a link that points
**somewhere**; for a link that points **nowhere** the old rule asked `Path::exists()`, and a
dangling link answers "no".

**Why that matters.** A rail that is replaced by a dangling link is the shape of an accident: a
synchronisation tool that removed the target, a backup that was rolled back, a container that
mounted an empty volume. The user marks a region, the comment gets the colour of the *standard*
rail, and the meaning of the note changes **without a word to the user**.

**The fix.** The state of the path is read with `symlink_metadata`, which does **not** follow a
link. A link of any kind is refused; only a path that carries **nothing at all** means "no rail".

**The proof:** `a_link_that_points_nowhere_is_refused_and_not_taken_for_no_rail`.

---

## 3. M3f — the read path had no check of the owner

**The measurement.** The reviewer wrote a rail, changed the owner of the file **and of the
directory** to another user, and read the rail again:

```
$ chown 1000:1000 <datenverzeichnis>/jurist_palette.json
$ chown 1000:1000 <datenverzeichnis>
$ harness configure <datenverzeichnis>   →  configure: OK
   die Rail wurde gelesen: 12 Eintraege
```

**What happened.** The rail of the stranger was **read and used**. The write path has always
checked the owner — the reviewer confirmed that (M3g: a write into the directory of a stranger is
refused). The read path did not.

**Why that matters.** The rail carries the *meaning* of the notes. A rail of a stranger is a rail
with a different set of colours and a different set of meanings: the notes of the reviewer are
written with the colours of the stranger, and the meaning of the document changes silently. The
first report did not see this, because the first round only measured the write path.

**The fix.** The read path checks the owner like the write path. A rail that does not belong to
the user is refused with a named error.

**The proof:** `a_rail_of_a_stranger_is_not_read`.

---

## 4. M3d — the refusal of `..` was applied to the wrong part of the path

**The measurement.** The reviewer let the platform choose a path with a `..` in the **directory**:

```
$ harness configure /tmp/arbeit/unterordner/../datenverzeichnis
   →  configure: FEHLER: the path of the rail of the profession may not leave its directory
```

**What happened.** The application refused to start. The path `/a/b/../c` means the same
directory as `/a/c` — it is a path that a platform legitimately produces, and the refusal
bricks the application at the start for a path that is **not** an attack.

**What the rule was for.** The rule from the first round protects the *file* against leaving its
directory. But the **name** of the file already does that: the name has to be exactly
`jurist_palette.json`, and a name can never be `..`. The directory is the part the *platform*
chose, and it is protected by the check of the owner.

**The fix.** The **name** decides now. The directory may carry a `..`; the file may not leave the
directory it was given. The staging file of the write is built from the same directory as the
rail, so the two can never land apart.

**The proof:** `a_directory_with_a_dot_dot_is_accepted_and_lands_in_the_named_directory` and
`a_file_that_is_not_the_rail_cannot_leave_its_directory`.

---

## 5. M4 — two writers at the same instant

**The measurement.** Eight **real** processes, each with a rail of its own (40 entries, own keys,
own colours), were released at the same wall-clock instant. A watcher read the file of the rail
as fast as it could and checked every sample: a sample is good when the file is a complete, valid
rail and carries the entries of **one** writer.

```
SAMPLES 5900144
```

**The result.** Not one sample was torn: no broken JSON, no half-written rail, no mixture of two
writers. The only "not good" samples were `MISSING the file of the rail` — before the first
writer had written. The rail on the disk afterwards was the rail of exactly one writer, and no
staging file was left over.

**Why that matters.** The write goes into a staging file in the same directory and is moved over
the rail afterwards. Two writers at the same time therefore cannot interleave their bytes: the
last one wins, and the file is always a complete rail.

**The limit.** The watcher ran on a machine that was otherwise idle. A machine under heavy load
changes the timing, not the property.

---

## 6. The limits of this round

1. **The report was written by the team lead** from the raw measurements of the independent run —
   the run reached the limit of its runtime before it wrote its own summary. The *measurements*
   are independent; the *prose* is not.
2. **The platform was not tested** — only the library over the UniFFI C ABI. The macOS shell and
   the Android shell were not part of the round.
3. **The watcher of M4 ran on one machine**; a run under load was not measured.
4. **The recovery store** was touched by the round (the directory of the recovery is created in
   the data directory), but it was not the subject of the round.
5. **The mode of the file and of the directory** (0600 / 0700) was not measured in this round; the
   first report measured it.
6. **No concurrency test on the recovery store** — the round of the third and fourth team members
   measured the journal, not the rail.

---

## 7. The state after the fixes

| # | Finding | Fix | Proof |
|---|---|---|---|
| M3b | the dangling link was taken for "no rail" | `symlink_metadata` before everything else; a link of any kind is refused | `a_link_that_points_nowhere_is_refused_and_not_taken_for_no_rail` |
| M3f | the read path had no check of the owner | the read path checks the owner like the write path | `a_rail_of_a_stranger_is_not_read` |
| M3d | the `..`-refusal hit the part the platform chose | the **name** of the file decides | `a_directory_with_a_dot_dot_is_accepted_and_lands_in_the_named_directory`, `a_file_that_is_not_the_rail_cannot_leave_its_directory` |
| M3g | — (not a finding) | the write path refuses a directory of a stranger | `the_directory_of_the_rail_belongs_to_the_user_alone` |
| M4 | — (not a finding) | the write is atomic: a staging file and a move | `two_applications_that_save_at_the_same_time_leave_a_complete_rail`, 5 900 144 samples, not one torn |

The first report is corrected in place — a report that is wrong in the repository is worse than no
report.

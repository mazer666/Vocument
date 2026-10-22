# The format of the acceptance records — an analysis and a decision template

**Status:** proposed — a decision paper, not a decision
**Date:** 2026-10-22
**Author role:** ROLE-13 (QA & Test Engineer)
**Review roles:** ROLE-01 (Product & Requirements Steward), ROLE-02 (Chief Architect), ROLE-17 (Documentation & Developer Experience Lead)

> **What this document is:** a measurement of how far the acceptance records
> have drifted from their own template, with the options for closing the
> drift. **Nothing has been restructured.** Every record is untouched; this
> document only decides *what* should be decided.

---

## 1. The question

`docs/acceptance/ACCEPTANCE_TEMPLATE.md` prescribes the form of an acceptance
record: four header fields and nine fixed sections. The records in
`docs/acceptance/` do not follow it — with two exceptions.

The template was written on 2026-10-17 and has never changed. The records
were written between 2026-10-17 and 2026-10-22. The question is therefore not
*which* form is right, but *which* of the two existing forms is the form the
project wants to keep — and what happens to the records that follow neither.

## 2. The normative reference

`docs/acceptance/ACCEPTANCE_TEMPLATE.md` requires:

| Header field | Purpose |
|---|---|
| `**Requirement IDs:**` | the requirements the record is evidence for |
| `**Build/commit:**` | the revision that was measured |
| `**Tester/agent roles:**` | who measured it |
| `**Date:**` | when it was measured |

| # | Section | Purpose |
|---|---|---|
| 1 | Functional | scenario → expected → result → evidence |
| 2 | Technical | architecture boundaries, central config, tests, migration |
| 3 | Performance | metric → budget → measured → result, plus the reference hardware |
| 4 | Usability | task step count, discoverability, native behaviour, polish |
| 5 | Accessibility | keyboard, VoiceOver/TalkBack, scaling/high contrast |
| 6 | Security/privacy | new data, network access, negative tests, threat notes |
| 7 | Interoperability/data integrity | reopen, external viewers, roundtrip, recovery |
| 8 | Known limitations | what the record does **not** prove |
| 9 | Decision | accepted / rejected / accepted with documented limitation |

The four header fields and the nine sections are what a reader of an
acceptance record can rely on. A record that carries them can be compared
with any other record; a record that does not can only be read on its own.

## 3. How it was measured

The measurement is mechanical and repeatable. For every file in
`docs/acceptance/` (except the template itself):

* **Header field** — the field counts as present when the file contains
  `**<field>:**` as a bold label.
* **Section** — the file's `##` headings are matched against the nine names
  of the template; a section counts as present when its leading keyword
  appears in the headings. The matching is deliberately generous: it counts
  a section whose content exists under a different name.

The script is not part of the repository; it is a one-off measurement for
this document. §7 describes how it becomes permanent.

## 4. The measurement

| Record | Header fields | Sections | The sections that are missing |
|---|---|---|---|
| `M0.0-ACCEPTANCE.md`, `M0.1-ACCEPTANCE.md` | 4/4 | 9/9 | — |
| `M0-ACCEPTANCE.md` | 1/4 | 0/9 | all nine |
| `M0.2`–`M0.7` | 1/4 | 0–1/9 | all nine |
| `M1-ACCEPTANCE.md` | 1/4 | 1/9 | all nine |
| `M1.0`–`M1.3` | 1/4 | 0–1/9 | all nine |
| `M1.4`, `M1.4b`, `M1.5` | 1/4 | 0/9 | all nine |
| `M1.6-ACCEPTANCE.md` | 1/4 | 2/9 | all nine |
| `M2.1`, `M2.2`, `M2.3` | **0/4** | 0/9 | all nine |
| `M2.4-ACCEPTANCE.md` | 1/4 | 0/9 | all nine |
| `ROUND-3`, `ROUND-4`, `ROUND-5`, `ROUND-6` | 1/4 | 0–1/9 | all nine |

**Two of twenty-six records follow the template.** Both are from the first
two days of the project (2026-10-17).

### 4.1 The drift is not new — it is continuous

The deviation is not a mistake of "the last records". It runs without a break
from `M0.2` (2026-10-17) through `M2.4` and the four round records
(2026-10-21/22). The last records are only the **most complete** instance of
a drift that has been open for five days.

### 4.2 The newest records are the worst in the header

`M2.1`, `M2.2` and `M2.3` carry **none** of the four header fields. They name
the milestone, the status and the requirements in free text instead:
```text
- **Milestone:** `M2.2 — Link and action inspection` (second slice of `M2 …`)
- **Status:** **PASS (host)** — …
- **Requirements:** `LINKSEC-001`, `LINKSEC-002` (`REQUIREMENTS.md` §26)
```

The information is there — but under a form that a machine cannot check and a
reader cannot compare. The revision that was measured and the roles that
measured it are missing from the newest records entirely.

## 5. The difference between a different shape and missing content

A drift that is only a different shape costs a relabelling. A drift that is
missing content costs a measurement. The two must not be confused, and the
second one is the expensive one.

The topic search across the newest records:

| Topic | `M2.1` | `M2.2` | `M2.3` | `M2.4` | `ROUND-5` | `M0.1` (reference) |
|---|---:|---:|---:|---:|---:|---:|
| Performance / measured time | 10 | 2 | 1 | 1 | 2 | 3 |
| Security / threat notes | 3 | 0 | 0 | 0 | 4 | 3 |
| Accessibility (VoiceOver/TalkBack) | **0** | **0** | **0** | **0** | **0** | 1 |
| Usability (step count, discoverability) | **0** | **0** | **0** | **0** | **0** | 2 |
| Known limitations | 1 | 1 | 1 | 1 | 3 | 1 |
| Decision / result | 47 | 14 | 16 | 24 | 22 | 8 |

Two findings follow:

1. **The functional evidence and the decision are strong.** Every record has
   a result overview, a description of how it was verified and a state. This
   is the part that must not be lost in a restructuring.
2. **Accessibility and Usability are empty in the newest records.** The
   section is not renamed — it is *absent*. The cause is known and honest:
   the M2 slices were verified on the host, and the surface of the
   applications was **NOT RUN** (no macOS host, no device). A usability and
   an accessibility statement cannot be produced from a host that never
   started the application.

The consequence for the decision is favourable: **the missing sections can be
filled with an explicit `NOT RUN` and its technical cause**, which is what
the vocabulary of the project already prescribes
(`docs/releasing/RELEASE-CANDIDATE.md` §3). No new measurement and no new
test run is needed to bring the records into the template — with one
exception: the usability of the M2 workflow *is* claimed in the
`ROUND-4`/`ROUND-5` records and can be moved into the M2 records from there.

## 6. A second, separate defect: three records are in the wrong place

`ROUND-3-ACCEPTANCE.md`, `ROUND-4-ACCEPTANCE.md`, `ROUND-5-ACCEPTANCE.md`
and `ROUND-6-ACCEPTANCE.md` are **not milestone acceptance records**:

* no milestone, no requirement IDs, no slice — they describe a *round* of
  adversarial audits and release preparation;
* they break the naming scheme `M<x.y>-ACCEPTANCE.md` that every other file
  in the directory follows;
* they are referenced from `ROADMAP.md` (three times) and from
  `project/project.yaml` (once) as *evidence of a round*, not as the
  acceptance of a milestone.

The project already has the right place for this kind of document — and it
already used it once: **`docs/reports/ROUND-2-COMPLETION.md`**. Round 2 has
its closeout in `docs/reports/`; rounds 3, 4, 5 and 6 were filed in
`docs/acceptance/` instead.

This is the one deviation that is a **misfiling** rather than a **shape**, and
it is therefore the one that can be corrected without a decision about the
form.

## 7. Nothing enforces the form — that is the reason for the drift

`scripts/validate-repo.sh` lists the acceptance files and checks that they
**exist** (lines 41–58, 77–78, 661). It never opens them. A record that
loses its header fields, its decision or its known limitations stays green.

So the drift was not a slip of one author. It was the logical consequence of
a template that is documented but not **checked**: every new record was
written in the shape of its predecessor, and the predecessor had drifted.

The same applies to the change requests and the architecture decision
records: `docs/adr/ADR-0021-optional-local-interfaces-mcp.md` was filed as an
ADR although it is a change request — it adds a milestone to the roadmap and
states what has to be true before that milestone is built. The decision of
the project is explicit about which document belongs where:

| A change … | belongs in | source |
|---|---|---|
| of the **behaviour** of the product | `docs/cr/` (Change Request) | `docs/DOCUMENTATION_POLICY.md` §1 |
| of the **structure** or a boundary | `docs/adr/` (Architecture Decision Record) | `docs/DOCUMENTATION_POLICY.md` §1 |
| of the **milestone** or the scope | `docs/cr/`, with `ROADMAP.md` and `project/project.yaml` | `docs/DOCUMENTATION_POLICY.md` §1 |

M7 is a milestone. The document has been moved to
`docs/cr/CR-0004-optional-local-interfaces-mcp.md` and the reference in
`ROADMAP.md` has been corrected.

## 8. The options

### Option A — The template stays binding, all records are brought into it

Every record receives the four header fields and the nine sections. The
content that exists is moved into the matching section; the sections that
cannot be measured get an explicit `NOT RUN` with its technical cause.

* **What it costs:** 24 records to touch. The work is mechanical for the
  header fields and for the result/decision sections. The functional
  evidence stays where it is and is referenced from the matching section.
  The largest single cost is `M2.1-ACCEPTANCE.md` (434 lines) — it needs its
  eight thematic sections mapped onto the nine template sections without
  losing the round-by-round evidence.
* **What it buys:** every record is comparable; a reviewer can read one
  section and know the state of the dimension; the gate can check the form.
* **Risk:** a mechanical restructuring can lose evidence. The mitigation is
  that the restructuring moves text and does not rewrite it, and that the
  diff is reviewed line by line.

### Option B — The template is brought to the current house style

The template is rewritten to the form that the M1.6/M2.x records actually
use: a milestone/status/requirements header, a result overview, "how it is
verified", "what is not proven", "the state of the milestone" — plus the
four header fields as **mandatory** and the missing dimensions (usability,
accessibility, security) as **mandatory statements**, including `NOT RUN`
with a cause.

* **What it costs:** one template, plus the four header fields and the
  missing statements in the records that do not have them.
* **What it buys:** the documents stay readable; the form is documented once
  and matches the way the team actually writes.
* **Risk:** the narrative form is less comparable between records. The
  mitigation is that the mandatory statements carry the comparable part.

### Option C — Only the misfilings are corrected

`ROUND-3/4/5/6` move to `docs/reports/ROUND-{3,4,5,6}-COMPLETION.md`, next to
`ROUND-2-COMPLETION.md`, and the references in `ROADMAP.md` and
`project/project.yaml` follow. The form of the remaining records is decided
later.

* **What it costs:** four files and their references.
* **What it buys:** the directory holds only acceptance records; the
  precedent of round 2 is restored.
* **Risk:** the form question stays open, and the drift continues.

### The comparison

| | A: template binding | B: template follows practice | C: only the misfiling |
|---|---|---|---|
| Records touched | 24 | 26 | 4 |
| Machine-checkable form | yes | partly (mandatory statements) | no |
| Evidence at risk in the restructuring | medium (all records) | low (no restructuring) | none |
| Comparability of two records | high | medium | unchanged (low) |
| Answers the question the project asked | yes | yes | no |

## 9. What is recommended

**Option B, and the enforcement of §7 with it.** The reasons:

1. The form the team actually writes in — result overview, how it was
   verified, what is not proven, the state — is the better form for a record
   that has to be *read*, and it is the form of the two records that already
   follow the template's intent (`M0.0`, `M0.1` are short because their
   subject was short).
2. The valuable part of the template is not its nine headings but its
   **completeness claim**: a record that says "the performance was measured
   and was X", "accessibility was NOT RUN because there was no macOS host",
   "the threat notes are Y" is a record a reviewer can trust. That claim
   survives the move to the narrative form as long as the dimensions stay
   mandatory.
3. Option A fixes a form that the team abandoned five days ago and would
   need to be maintained from now on, at a cost that grows with every
   milestone.

Independently of the choice, two things are worth doing now, because they are
small and they stop the drift from growing:

* **The check in the gate.** A check that every file in `docs/acceptance/`
  carries the mandatory header fields and the mandatory statements, with the
  `NOT RUN` vocabulary as an accepted value. It is the same kind of check as
  the `QA-003` checks that `validate-repo.sh` already runs.
* **The naming rule.** A file in `docs/acceptance/` names a milestone
  (`M<x.y>-ACCEPTANCE.md`). A round closeout lives in `docs/reports/`
  (`ROUND-<n>-COMPLETION.md`), as `ROUND-2-COMPLETION.md` already does.

## 10. The open questions for the project owner

1. **Which form is binding?** Option A, B or C — the recommendation is B.
2. **Are the `M2.1`–`M2.4` records acceptable without a macOS host?** They
   claim `PASS (host)` and name the application surface as `NOT RUN`. If the
   usability and the accessibility of the M2 review workflow are to be
   *accepted*, the statement needs a sign-off; if they are to be *measured*,
   a macOS host is needed. The record cannot answer this by itself.
3. **Should the older records (`M0`, `M0.2`–`M0.7`, `M1`, `M1.0`–`M1.5`)
   be brought into the form as well?** They are closed milestones; the
   question is whether the historical record is corrected retroactively or
   left as it was written.

## 11. What this document does not claim

* It does **not** claim that any acceptance record is wrong in its content.
  The measurement is about the form and the presence of the dimensions.
* It does **not** claim that a record which follows the template is correct.
  `M0.0` and `M0.1` follow it; that says nothing about their evidence.
* It does **not** weaken any acceptance criterion. Nothing in this document
  turns a `NOT RUN` into a pass.
* It does **not** decide anything. §10 is the decision, and the owner makes it.

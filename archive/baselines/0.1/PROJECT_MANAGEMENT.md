# Vocument Project Management

Vocument is managed as an engineering product, not an accumulation of features.

---

## 1. Sources of truth

| Topic | Source of truth |
|---|---|
| Product behaviour | `REQUIREMENTS.md` + feature specs |
| Architecture | `ARCHITECTURE.md` + ADRs |
| Current milestone | `ROADMAP.md` + `project/project.yaml` |
| Quality/acceptance | `QUALITY.md` + acceptance specs |
| Agent process | `AGENTS.md` |
| Config defaults/policy | `/config` |
| Historical baseline | `/archive` |

When documents conflict, fix the conflict immediately; do not leave “both versions” alive.

---

## 2. Work hierarchy

```text
Product Vision
  ↓
Requirements
  ↓
Milestones
  ↓
Features
  ↓
Acceptance Criteria
  ↓
Implementation Tasks
  ↓
Tests + Evidence
  ↓
Release
```

A task should be traceable upward to a feature/requirement.

---

## 3. Milestone states

- `planned`
- `in_progress`
- `blocked`
- `acceptance`
- `done`

“Done” requires evidence, not confidence.

---

## 4. Feature states

- `proposed`
- `specified`
- `approved`
- `implementing`
- `testing`
- `accepted`
- `deferred`
- `superseded`

Deferred features must say **why**, by whom/which CR, and to which milestone/track.

---

## 5. Change requests

Material product changes require `CR-XXXX`.

A CR must include:

- problem
- requested change
- reason
- impacted requirement IDs
- impacted architecture
- impacted platforms
- milestone impact
- acceptance/test changes
- risks
- decision

Agents may propose a CR; they may not silently enact the product change first.

---

## 6. Architecture decisions

Material architecture decisions use ADRs.

ADRs are never deleted.

If a decision changes:

- old ADR becomes `superseded`
- new ADR links to old ADR
- historical rationale remains available

---

## 7. Documentation freshness

Every meaningful PR/change asks:

- Requirements stale?
- Architecture stale?
- Roadmap/status stale?
- Feature spec stale?
- Acceptance tests stale?
- Config docs stale?
- WHERE_TO_CHANGE stale?
- Benchmark status stale?

If yes, update in the same change.

A code-only PR that knowingly makes docs wrong is incomplete.

---

## 8. Archive policy

`archive/` contains immutable historical baselines and superseded material.

Do not use archive files as current source of truth.

Baseline creation:

1. update current docs
2. run validation
3. assign baseline version
4. copy/snapshot required docs into `archive/baselines/<version>/`
5. record manifest
6. tag release/baseline in Git when repository is live

---

## 9. Risk register

Maintain significant risks in `docs/project/RISKS.md`.

Each risk includes:

- ID
- description
- probability
- impact
- mitigation
- owner role
- status

---

## 10. Decision discipline

Prefer decisions that are:

- reversible where possible
- explicit
- documented
- testable
- measurable

Do not overdesign speculative later features, but keep agreed extension points intact.

---

## 11. Best-in-class reviews

At milestone acceptance, the Benchmark Analyst compares relevant workflows with reference products.

The review records:

- reference product/version/date
- task
- steps/clicks
- speed
- clarity
- output quality
- failure behaviour
- Vocument advantage/disadvantage
- follow-up

---

## 12. Release readiness

A release requires:

- scope complete
- acceptance criteria green
- no unresolved critical security issue
- no unresolved data-loss issue
- docs current
- licences/notices current
- reproducible release notes
- known limitations documented
- upgrade/migration behaviour tested where relevant

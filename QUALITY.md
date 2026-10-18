# Vocument Quality & Acceptance Model

A Vocument feature is not “done” because the happy path works once.

Every substantial feature is accepted across six dimensions.

---

## 1. Functional acceptance

Questions:

- Does the feature do what the requirement says?
- Are preset/optional behaviours correct?
- Are edge cases defined?
- Does Undo/Redo work where relevant?
- Does Recovery preserve the work where relevant?
- Are errors understandable and actionable?

Evidence:

- acceptance scenarios
- automated tests
- manual exploratory test notes where automation is unsuitable

---

## 2. Technical acceptance

Questions:

- Does implementation follow architecture boundaries?
- Are state/ownership rules respected?
- Are configs centralised?
- Are dependencies justified and pinned?
- Is failure isolated?
- Are migrations/versioning handled?
- Is code understandable and within file-size limits (`CODE-001`: target $\le 600$ lines, max $900$ lines)?
- Is the functionality implemented with authentic native/embedded logic rather than a stub or mock simulation (`QA-003`, `CR-0002`)?

Evidence:

- architecture review
- tests (executing authentic native engines and realistic assets)
- real runtime verification on target platform/device (no mock bridges)
- static analysis/lint
- dependency review

---

## 3. Performance acceptance

Questions:

- Is startup within budget?
- Does first page appear within target?
- Does scrolling remain responsive?
- Is memory bounded?
- Does background work yield to interactive work?
- Is battery/thermal behaviour acceptable on mobile?

Evidence:

- reproducible benchmark
- reference device
- input corpus
- result stored with milestone evidence

Never write “fast” without a measurement.

---

## 4. Usability acceptance

Questions:

- Can a new user discover the common path without onboarding?
- Is the default calm?
- Are advanced controls available without cluttering the default?
- Is the number of actions competitive?
- Does mobile feel native rather than like desktop UI squeezed onto a phone?
- Are exits/recovery paths obvious?

Evidence:

- task walkthrough
- step count
- visual review
- accessibility review
- comparison against benchmark products

---

## 5. Security/privacy acceptance

Questions:

- Does the feature create new network access?
- Does it store new user data?
- Are temporary files protected/minimised?
- Is untrusted input bounded?
- Can malformed input crash/escape the component?
- Is sensitive action explicit?

Evidence:

- threat review
- negative tests
- permission review
- logs/cache inspection

---

## 6. Interoperability/data-integrity acceptance

Questions:

- Does the file reopen correctly in Vocument?
- Does it reopen correctly in external relevant viewers/editors?
- Is unsupported data preserved?
- Are standards used correctly?
- Does save/recovery avoid data loss?

Evidence:

- roundtrip tests
- cross-viewer matrix
- validators/reference renderers
- hash/integrity checks where meaningful

---

# Initial performance budgets

These are targets, not excuses to hide slower behaviour.

| Metric | Initial target |
|---|---|
| App start | ideally < 1–1.5 s on reference hardware |
| Normal PDF first page | ideally 300–500 ms after load path ready |
| Common indexed search first result | ideally 150–300 ms |
| Scroll/zoom | 60 fps target |
| Large document UI | never blocked by full-document work |
| Thumbnails | progressive |
| OCR/compression | background / cancellable |

Device-specific budgets will be added after reference hardware is fixed.

---

# Severity levels

### S0 — Cosmetic
No functional impact.

### S1 — Minor
Small inconvenience; safe workaround exists.

### S2 — Significant
Core workflow degraded or confusing.

### S3 — Major
Incorrect output, interoperability failure, major performance regression.

### S4 — Critical
Data loss, security bypass, incorrect redaction/signature result, unrecoverable corruption.

A milestone/release cannot ship with known S4 issues.

---

# Acceptance package per feature

Each feature should eventually have:

1. requirement IDs
2. feature spec
3. user scenarios
4. functional acceptance tests
5. technical checks
6. performance expectations
7. usability expectations
8. security/privacy considerations
9. interoperability matrix where relevant
10. known limitations
11. evidence links/results

Use `docs/acceptance/ACCEPTANCE_TEMPLATE.md`.

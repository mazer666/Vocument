# Vocument Agent Development Team

This document defines how coding/review agents work as a development team.

**Important:** Agents are roles, not permission to change product direction. Requirements and accepted architecture outrank an agent's preference.

---

## 1. Universal rules for every agent

Every agent must:

1. read the relevant requirements before changing behaviour
2. read the relevant architecture section before changing structure
3. use clear, descriptive names
4. avoid abbreviations unless they are standard domain terms
5. keep configuration/defaults centralised
6. add/update tests with behaviour changes
7. update documentation in the same change when it becomes stale
8. never silently weaken an acceptance criterion
9. never add a dependency without licence/security/maintenance review
10. never bypass `TransactionManager`, `FileBroker`, `NetworkBroker` or other architectural boundaries for convenience
11. explain non-obvious decisions in code comments/ADR, not narrate obvious code
12. prefer small understandable changes over giant refactors
13. keep generated code generated; never hand-edit generated UniFFI bindings
14. preserve user data first
15. keep hand-written source files concise (`CODE-001`): target $\le 600$ lines per file, hard maximum $900$ lines per file
16. never implement, accept, or claim completion for any functionality based solely on stubs, dummy return values, or synthetic mock simulations (`QA-003`, `CR-0002`): acceptance strictly requires genuine native/embedded implementations with real execution

---

## 2. Naming standard

Bad:

```text
x
tmp
data2
mgr
proc
foo
handleStuff()
```

Good:

```text
currentDocumentRevision
recoveryCheckpointPath
searchIndexGeneration
pdfRenderJob
externalFileModification
applySecureRedaction()
```

A new developer should understand the intent without reverse engineering the implementation.

---

## 3. Team roles

### ROLE-01 — Product & Requirements Steward

Owns:
- product intent
- requirement IDs
- presets vs optional vs later
- change-request discipline
- scope protection

Must review:
- behaviour changes
- milestone scope changes
- feature removals/deferments

May block a change when:
- implementation silently changes product behaviour
- acceptance criteria were weakened

---

### ROLE-02 — Chief Architect

Owns:
- module boundaries
- dependency direction
- core/native separation
- provider interfaces
- architectural consistency
- ADRs

Must review:
- new top-level module
- new engine/provider
- new cross-cutting dependency
- changes to UniFFI/DocumentSession/JobManager/TransactionManager

---

### ROLE-03 — Rust Core Engineer

Owns:
- DocumentSession
- transactions
- jobs
- search services
- recovery
- common models
- headless testability

Quality bar:
- no UI assumptions in core
- deterministic behaviour where required
- excellent error typing

---

### ROLE-04 — PDF/MuPDF Engineer

Owns:
- PdfEngine adapter
- rendering pipeline
- PDF editing plans
- annotations/forms/layers
- incremental/full save integration
- PDF structure expertise

Must consider:
- interoperability
- signatures
- object preservation
- large documents

---

### ROLE-05 — macOS Engineer

Owns:
- SwiftUI/AppKit app
- NSDocument lifecycle wrapper
- FileBroker implementation
- native menus/windows/printing
- VoiceOver integration
- macOS sandbox integration

Must not:
- move product logic into Swift because it is convenient

---

### ROLE-06 — Android Engineer

Owns:
- Kotlin/Compose app
- Storage Access Framework
- background-work adapter
- camera capture
- Android lifecycle integration
- TalkBack
- S-Pen/stylus UX

Must consider:
- battery
- thermal state
- Activity/process lifecycle

---

### ROLE-07 — OCR & Imaging Engineer

Owns:
- OcrProvider
- Tesseract integration
- ImagingEngine
- scan cleanup
- perspective/deskew
- OCR evaluation corpus

Measures:
- OCR accuracy
- processing speed
- scan visual quality

---

### ROLE-08 — Document/Office Engineer

Owns:
- FlowDocument
- layout engine
- `.pdoc`
- OfficeEngine adapters
- DOCX/ODT preservation
- tracked changes
- authored PDF export semantics

Must never:
- claim roundtrip support without corpus evidence

---

### ROLE-09 — Security Engineer

Owns:
- threat model
- untrusted document handling
- plugin sandbox
- network boundaries
- script restrictions
- secure redaction verification
- update trust
- worker-isolation roadmap

Must review:
- any new network access
- plugin/native provider permission
- cryptography
- external process execution

---

### ROLE-10 — Privacy Engineer

Owns:
- data minimisation
- logging privacy
- cache/recovery privacy
- offline-default review
- metadata behaviour
- secure storage usage

Asks:
> What user data exists here, why does it exist, where is it stored, and how is it removed?

---

### ROLE-11 — Accessibility Engineer

Owns:
- VoiceOver/TalkBack
- keyboard accessibility
- focus order
- scaling/high contrast/reduced motion
- document semantics
- PDF/UA-oriented authoring/repair UX

Accessibility is not postponed until release.

---

### ROLE-12 — UX/Product Designer

Owns:
- calm default UI
- progressive disclosure
- mobile-native interaction
- no-nag principle
- tool discoverability
- information hierarchy
- visual quality

Must compare:
- step count
- clarity
- error recovery
- expert depth without beginner clutter

---

### ROLE-13 — QA & Test Engineer

Owns:
- acceptance tests
- corpus tests
- regression tests
- test evidence
- release gates
- reproducibility of bugs

May block milestone completion if evidence is missing.

---

### ROLE-14 — Performance Engineer

Owns:
- startup
- first page
- FPS/scroll
- memory/cache
- search latency
- OCR/scan throughput
- power/thermal profiling

No performance claim without measurement.

---

### ROLE-15 — Interoperability & Standards Engineer

Owns:
- PDF/A/PDF/UA/PDF structures
- cross-viewer tests
- roundtrip compatibility
- page labels/bookmarks/forms/annotations standards
- external validators/reference renderers

---

### ROLE-16 — Release & Supply-Chain Engineer

Owns:
- reproducible builds
- dependency pinning
- SBOM
- signing/notarisation
- checksums
- update security
- release packaging

---

### ROLE-17 — Documentation & Developer Experience Lead

Owns:
- README
- WHERE_TO_CHANGE
- onboarding
- architecture diagrams
- feature docs
- documentation freshness
- simple developer commands

Must optimise for:
> Can a motivated coding beginner understand what to do next?

---

### ROLE-18 — Licensing & Open-Source Reviewer

Owns:
- AGPL compatibility
- DCO process
- SPDX/REUSE
- third-party notices
- plugin/dependency licence review

May block dependency introduction.

---

### ROLE-19 — Benchmark & Best-in-Class Analyst

Owns:
- competitor/reference workflows
- PDFCraft tracking
- PDF-XChange/Acrobat/Preview/browser/mobile comparisons
- UX step comparison
- quality deltas

Must avoid feature-count theatre.

---

### ROLE-20 — Project / Release Manager

Owns:
- milestone state
- blockers
- risks
- scope tracking
- status documentation
- release-readiness checklist

Cannot declare a milestone complete without QA evidence.

---

## 4. Mandatory multi-agent review matrix

| Change type | Required roles |
|---|---|
| Requirement/behaviour | Product + QA |
| Architecture/module boundary | Architect + relevant engineer |
| PDF save/edit | PDF + QA + Interoperability |
| Redaction/security | Security + PDF + QA |
| Network access | Security + Privacy |
| New dependency | Architect + Supply Chain + Licensing |
| UI workflow | UX + platform engineer + Accessibility |
| Performance-sensitive render/search | Performance + relevant engineer |
| DOCX/ODT | Document/Office + QA + Interoperability |
| Plugin/API | Architect + Security + Licensing |
| Release | Release + QA + Project Manager |

One agent may fill several roles in a small project, but the **review perspectives must still be applied explicitly**.

---

## 5. Definition of agent-done

An agent may say “done” only when:

- implementation is complete with real native/embedded code (stubs or mocks never satisfy acceptance — `QA-003`)
- tests are present/passing
- relevant acceptance criteria are demonstrated
- docs are current
- config/default changes are centralised/documented
- no known requirement was silently deferred
- performance/security impact was considered
- any remaining limitation is documented

---

## 6. Forbidden agent behaviour

Agents must not:

- invent a new architecture because it is fashionable
- replace accepted technology without ADR
- mark tests skipped to make CI pass
- lower performance budgets without CR
- silently remove an edge case from acceptance criteria
- scatter configuration constants across code
- hide breaking changes behind vague “cleanup” commits
- commit generated binaries without policy approval
- add telemetry
- add cloud calls as fallback
- change licence headers casually
- implement or submit stubs, dummy returns, or mock bridges as accepted functionality (`QA-003`, `CR-0002`)
- declare a milestone or feature accepted when underlying functionality is only stubbed

---

## 7. Agent task handoff template

Every substantial handoff should state:

```text
Task:
Requirement IDs:
Milestone:
Files/modules changed:
Behaviour changed:
Tests added/updated:
Performance impact:
Security/privacy impact:
Documentation updated:
Known limitations:
Follow-up:
```

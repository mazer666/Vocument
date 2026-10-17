# Vocument

**Vocument** is a free, open-source, privacy-first PDF and document application for people who want a calm reader when they only need to read — and a professional tool when they need to edit, review, scan, OCR, compare, redact, sign, assemble or author documents.

> **Product principle:** A tool that does not want anything from you.

Vocument is designed to be local-first, offline-first, interoperable and understandable. No forced account. No subscription. No ads. No cloud dependency. No proprietary document lock-in.

---

## Current project status

- **Baseline:** Implementation Baseline v0.1
- **Current milestone:** M0 — Foundation / “Trustworthy Viewer” (`M0.0` & `M0.1` complete, `M0.2` next — see [`docs/acceptance/M0.1-ACCEPTANCE.md`](docs/acceptance/M0.1-ACCEPTANCE.md))
- **Phase-1 platforms:** macOS 15+ on Apple Silicon, Android 13+
- **Shared core:** Rust (`core/vocument-core`)
- **macOS UI:** SwiftUI + AppKit (`apps/macos`)
- **Android UI:** Kotlin + Jetpack Compose (`apps/android`)
- **Primary PDF engine:** MuPDF, behind Vocument's own `PdfEngine` interface (`engines/pdf-mupdf`)
- **Initial OCR engine:** Tesseract, behind `OcrProvider` (`engines/ocr`)
- **Core ↔ native bridge:** UniFFI (`platform/vocument-ffi`)
- **License:** AGPLv3
- **External contributions:** DCO, no CLA

See [`project/project.yaml`](project/project.yaml) for the machine-readable project status.

---

## Quickstart (Developer Commands)

A clean clone can be validated, built, and verified end-to-end across Rust, Swift (macOS), and Kotlin (Android) with a single command:

```bash
make check          # Runs ./scripts/local-ci.sh (repo validation, cargo fmt/clippy/test, UniFFI binding check, Swift & Kotlin handshakes)
make bindings       # Regenerates Swift & Kotlin UniFFI bindings from platform/vocument-ffi
make hooks          # Installs local Git hooks (DCO sign-off check & pre-push local CI)
```

---

## If you are new to coding: start here

You should not need to understand the entire codebase before changing one thing.

1. Read **this file**.
2. Read [`WHERE_TO_CHANGE.md`](WHERE_TO_CHANGE.md) when you want to change something specific.
3. Read [`ROADMAP.md`](ROADMAP.md) to understand what we are building now and what comes later.
4. Read [`REQUIREMENTS.md`](REQUIREMENTS.md) before changing behaviour.
5. Read [`ARCHITECTURE.md`](ARCHITECTURE.md) before changing structure, dependencies or technical boundaries.
6. Read [`AGENTS.md`](AGENTS.md) if you are using Codex/another coding agent.
7. Run `make check` (`./scripts/local-ci.sh`) before committing.

### Golden rule

**Do not “just make it work” by bypassing architecture, requirements or tests.**  
If a requirement is wrong, change the requirement through the documented change process first.

---

## What Vocument aims to be best at

Vocument's “best in class” claim must be measurable. We evaluate it across:

- startup and time-to-first-page
- rendering fidelity
- scrolling/zoom responsiveness
- search latency and quality
- OCR quality
- scan cleanup quality
- editing fidelity
- document interoperability
- undo/recovery reliability
- safe save behaviour
- redaction safety
- signature correctness
- accessibility
- battery and memory use
- number of steps/clicks for common workflows
- clarity of UI and error handling

The competitive/quality framework is documented in [`docs/BEST_IN_CLASS.md`](docs/BEST_IN_CLASS.md).

---

## Core product behaviour

Vocument is **document-centric**, not a pile of disconnected tools.

A typical flow should be:

`Open document → work in it → undo/redo → save safely → reopen anywhere`

not:

`Choose tool → upload/import → process → download a new file`.

PDFCraft is used as an important open-source benchmark for breadth. PDF-XChange Editor, Adobe Acrobat, macOS Preview, browser PDF viewers and relevant mobile apps are additional reference points. Vocument should not merely match feature names; features should be integrated, undoable, recoverable, interoperable and understandable.

---

## Repository map

```text
Vocument/
├─ README.md                  ← Start here
├─ ARCHITECTURE.md            ← Technical system design
├─ ROADMAP.md                 ← Milestones and gates
├─ REQUIREMENTS.md            ← Product requirements / source of truth
├─ AGENTS.md                  ← Rules and roles for coding agents
├─ PROJECT_MANAGEMENT.md      ← How the project is managed
├─ QUALITY.md                 ← Acceptance and quality model
├─ WHERE_TO_CHANGE.md         ← “I want to change X — where do I go?”
├─ GLOSSARY.md                ← Important terms
├─ CONTRIBUTING.md            ← Contribution rules / DCO
├─ SECURITY.md                ← Security reporting and secure development
├─ THIRD_PARTY.md             ← Dependency governance
├─ CHANGELOG.md               ← Human-readable project changes
├─ VERSION                    ← Baseline/document version
│
├─ project/                   ← Machine-readable project state
├─ config/                    ← Central defaults, limits and policy
├─ docs/                      ← Detailed living documentation
├─ archive/                   ← Immutable baselines and superseded docs
├─ scripts/                   ← Developer/validation helpers
│
├─ apps/                      ← Native apps (created during implementation)
├─ core/                      ← Shared Rust core
├─ engines/                   ← MuPDF/OCR/other adapters
├─ formats/                   ← pdoc/docx/odt/epub/comic adapters
├─ platform/                  ← Platform-specific bridges
└─ tests/                     ← Cross-platform corpus and QA
```

---

## Central configuration rule

Defaults, limits and policy must not be scattered through the codebase.

They belong under [`config/`](config/README.md), with typed code representations generated or kept in sync with the configuration schema.

Examples:

- UI defaults
- performance budgets
- cache limits
- security defaults
- network policy defaults
- recovery retention defaults
- job limits

User-specific secrets **never** belong there; passwords, tokens, certificates and private-key references belong in OS secure storage.

---

## Documentation is part of the product

A code change is not finished if the documentation becomes wrong.

Every PR/change must ask:

- Does a requirement change?
- Does architecture change?
- Does a feature specification change?
- Does a config default change?
- Does the roadmap/status change?
- Do acceptance tests change?
- Does `WHERE_TO_CHANGE.md` need an update?

If yes, documentation must be updated in the **same change**.

See [`PROJECT_MANAGEMENT.md`](PROJECT_MANAGEMENT.md) and [`docs/DOCUMENTATION_POLICY.md`](docs/DOCUMENTATION_POLICY.md).

---

## Current roadmap

1. **M0 — Foundation / Trustworthy Viewer**
2. **M1 — Android Scan & OCR**
3. **M2 — macOS Professional Review**
4. **M3 — Page Workbench**
5. **M4 — Authoring / FlowDocument / .pdoc / DOCX / ODT**

See [`ROADMAP.md`](ROADMAP.md).

---

## Licence

Vocument's own application/core code is intended to be licensed under **AGPLv3**.

External contributions use the **Developer Certificate of Origin (DCO)** rather than a CLA. Contributors retain their copyright.

Third-party dependencies keep their own licences and notices. Dependency compatibility must be reviewed before introduction.

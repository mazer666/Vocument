# Where to Change What

This file is deliberately written for someone who does **not** know the whole codebase.

When implementation begins, keep this file current.

---

## I want to change a default setting

Go to:

- `config/`
- the corresponding typed settings model in the Rust core

Do not hard-code a new default in a random UI file.

---

## I want to change product behaviour

1. Find the requirement in `REQUIREMENTS.md`.
2. If behaviour should actually change, create a Change Request if material.
3. Update acceptance criteria.
4. Then implement.

---

## I want to add a PDF operation

Expected path:

```text
UI command
→ DocumentCore API
→ semantic transaction
→ PdfEditService/PdfEngine
→ validation
→ Undo/Recovery support
```

Likely locations after implementation begins:

- `core/document/`
- `core/transactions/`
- `engines/pdf-mupdf/`

Do not call MuPDF directly from Swift/Kotlin UI.

---

## I want to add OCR behaviour

- provider contract: `engines/ocr/`
- orchestration/job behaviour: `core/jobs/`, `core/document/`
- scanner cleanup: `engines/imaging/`
- native camera capture: platform app code

---

## I want to change macOS UI

- `apps/macos/`

Product logic normally does **not** belong there.

---

## I want to change Android UI

- `apps/android/`

Product logic normally does **not** belong there.

---

## I want to change file opening/saving permissions

Use platform `FileBroker` implementations.

Never bypass them with broad filesystem access.

---

## I want to add network access

Stop.

Any network access must go through `NetworkBroker` and requires Security + Privacy review.

---

## I want to change recovery/undo

Relevant architecture:

- `TransactionManager`
- `RecoveryStore`
- per-document recovery containers
- engine journal adapter

This is data-integrity-critical and requires strong tests.

---

## I want to add a dependency

Before adding:

1. explain why existing code/library cannot solve it
2. check licence compatibility
3. check maintenance/security posture
4. check binary/platform cost
5. record third-party information
6. update SBOM/dependency files when available

See `THIRD_PARTY.md`.

---

## I want to add a setting

Ask:

- Is this really a user-facing setting or should automatic behaviour just work?
- Which canonical settings section owns it?
- What is the preset?
- Should it export/import?
- Is it sensitive?

Then update central config schema/docs.

---

## I want to add a plugin capability

Requires:

- API design
- Security review
- Privacy review
- versioning
- permission UI
- negative tests

Plugins never get raw internal pointers.

---

## I want to edit docs

Current docs live at top level and under `docs/`.
Historical baselines live under `archive/` and should not be edited.

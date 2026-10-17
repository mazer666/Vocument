# Where to Change What

This file is deliberately written for someone who does **not** know the whole codebase.

When implementation begins, keep this file current.

---

## I want to change a default setting

Go to:

- `config/app-defaults.toml`, `config/performance-budgets.toml`, or `config/security-policy.toml`
- the corresponding typed settings model in `core/vocument-core/src/config.rs`

Do not hard-code a new default in a random UI file.

---

## I want to change the UniFFI `DocumentCore` bridge

1. Update the exported Rust types/functions in `platform/vocument-ffi/src/lib.rs`.
2. Run `make bindings` (`./scripts/generate-bindings.sh`) to regenerate the Swift (`apps/macos/Sources/VocumentFFI`) and Kotlin (`apps/android/app/src/main/java/uniffi/vocument_ffi`) bindings.
3. Never hand-edit generated files inside those directories (`ADR-0003`).
4. Run `make check` to verify both Swift and Kotlin handshakes.

---

## I want to change product behaviour

1. Find the requirement in `REQUIREMENTS.md`.
2. If behaviour should actually change, create a Change Request if material (`docs/cr/`).
3. Update acceptance criteria.
4. Then implement.

---

## I want to add a PDF operation

Expected path:

```text
UI command
→ DocumentCore API (platform/vocument-ffi)
→ semantic transaction (core/vocument-core)
→ PdfEditService/PdfEngine (engines/pdf-mupdf)
→ validation
→ Undo/Recovery support
```

Likely locations:

- `core/vocument-core/`
- `engines/pdf-mupdf/`
- `platform/vocument-ffi/`

Do not call MuPDF directly from Swift/Kotlin UI.

---

## I want to add OCR behaviour

- provider contract & Tesseract adapter: `engines/ocr/`
- orchestration/job behaviour: `core/vocument-core/`
- scanner cleanup: `engines/imaging/`
- native camera capture: `apps/android/`

---

## I want to change macOS UI

- `apps/macos/Sources/VocumentMacApp/` and `apps/macos/Sources/VocumentMacKit/`

Product logic normally does **not** belong there.

---

## I want to change Android UI

- `apps/android/app/src/main/java/org/vocument/app/`

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
2. check licence compatibility (`deny.toml`)
3. check maintenance/security posture
4. check binary/platform cost
5. record third-party information in `THIRD_PARTY.md`

---

## I want to add a setting

Ask:

- Is this really a user-facing setting or should automatic behaviour just work?
- Which canonical settings section owns it?
- What is the preset?
- Should it export/import?
- Is it sensitive?

Then update central config schema/docs (`config/*.toml` and `core/vocument-core/src/config.rs`).

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

# Third-Party Dependency Governance

Every third-party library or tool introduced into Vocument must pass architectural, licensing, security, and maintenance review (`AGENTS.md` ROLE-02, ROLE-16, ROLE-18).

---

## 1. Dependency Introduction Checklist

Before adding any Cargo crate, Swift package, Gradle dependency, or C/C++ library:

1. **Necessity:** Explain why existing standard library or current dependencies cannot solve the problem.
2. **License Compatibility:** Must be compatible with **AGPL-3.0-or-later** distribution across macOS and Android.
3. **Maintenance & Security Posture:** Active maintenance, clear security track record, minimal transitive dependency bloat.
4. **Binary & Platform Cost:** Acceptable size and compilation impact on `arm64` macOS and `arm64-v8a` Android.
5. **Architecture Boundary:** Engines and external providers must sit behind Vocument-owned traits/interfaces (`PdfEngine`, `OcrProvider`, `OfficeEngine`, etc.).

---

## 2. Planned Core Third-Party Components (Baseline 0.1)

| Component | Role / Adapter Boundary | License | Status |
|---|---|---|---|
| **MuPDF** | Primary PDF rendering, text & `/PageLabels` behind `PdfEngine` | AGPL-3.0-or-later | Active (`M0.1`–`M0.3`) |
| **Tesseract** | Local OCR engine behind `OcrProvider` | Apache-2.0 | Planned (M1) |
| **UniFFI** | Rust ↔ Swift/Kotlin bindings generator | MPL-2.0 | Active (`M0.0`–`M0.3`) |
| **SQLite (with FTS5) + `rusqlite 0.32`** | `AppCatalog` bookkeeping & progressive exact full-text search (`SessionSearchIndex`) | Public Domain / MIT | Active (`M0.3`) |
| **Wasmtime** | Sandboxed community plugin runtime | Apache-2.0 WITH LLVM-exception | Planned (Later) |

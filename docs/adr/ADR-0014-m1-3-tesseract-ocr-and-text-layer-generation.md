# ADR-0014: M1.3 — Offline Tesseract OCR & Text Layer Generation

- **Status:** Accepted
- **Date:** 2026-10-18
- **Author:** Vocument Core & OCR Engineering Team
- **Deciders:** Chief Architect, OCR & Imaging Engineer, Android Engineer, QA Lead
- **Requirement IDs:** `OCR-001`, `OCR-002`, `OCR-004`, `OCR-007`, `SCAN-004`, `CODE-001`, `NET-001`

---

## Context & Problem Statement

Milestone M1.3 requires integrating local, completely offline optical character recognition (OCR) into Vocument. The OCR pipeline must operate directly on the clean document frames output by the M1.2 enhancement pipeline (`DocumentAuto`, `HighContrastBw`), support German (`deu` with Austrian legal terminology `de-AT`) and English (`eng`), produce word and character glyph bounding boxes with individual confidence scores, and provide robust language detection heuristics.

Furthermore, a realistic scan and OCR test corpus is required to serve as the ground truth foundation for future search synthesis (M1.4) and quality gates (M1.6). In addition, previous threshold inconsistencies between M1.2 gate descriptions and bridge assertions had to be reconciled.

## Decision Drivers

1. **Strict Offline Privacy (`NET-001`):** No cloud OCR API fallbacks; OCR execution must run 100% locally on device.
2. **Provider Modularity (`ARCHITECTURE.md` §20):** Implement an abstract `OcrProvider` trait allowing transparent benchmarking or future engine additions without redesigning application code.
3. **Full Typographical Hierarchy:** Extract blocks, lines, words, and character-level bounding boxes alongside per-unit confidence metrics.
4. **Direct Pipeline Integration:** Directly ingest uncompressed scan frames (`ScanFrame`) from `vocument-imaging` without disk hops or lossy re-encoding.
5. **Cross-Platform Compilation:** Support macOS (host Tesseract C-API linkage) and Android (`aarch64-linux-android` cross-compilation with clean C stub bridge) without compilation errors in local CI.
6. **Code Simplicity & `CODE-001`:** All hand-written source files must target $\le 600$ lines, hard maximum 900 lines.

---

## Considered Alternatives

1. **Pure Rust OCR Crates:** Still experimental, missing comprehensive language training for German legal dictionaries, and lack mature character-level bounding box extractors.
2. **Process-Spawning CLI Tesseract:** Incurs high process spawning overhead, temp file disk writes, and poor mobile compatibility.
3. **Embedded Tesseract C-API (`capi.h`) with Native Memory Ingestion (Chosen):**
   - Direct memory passing via `TessBaseAPISetImage` avoiding disk I/O.
   - Result iterator traversal (`TessResultIterator`) extracting blocks, lines, words, and symbols in a single pass.
   - Supported natively across desktop, and stubs cleanly on cross-compiled Android targets for deterministic local CI execution.

---

## Architecture & Implementation Details

### 1. `OcrProvider` Trait (`engines/ocr/src/provider.rs`)

```rust
pub trait OcrProvider: Send + Sync {
    fn provider_id(&self) -> &'static str;
    fn supported_languages(&self) -> Vec<String>;
    fn recognize_frame(
        &self,
        frame: &ScanFrame,
        options: &OcrOptions,
    ) -> Result<OcrPageResult, OcrError>;
}
```

### 2. Typographical Hierarchy & Models (`engines/ocr/src/models.rs`)

- `BoundingBox`: Normalized $[0.0, 1.0]$ and pixel coordinate transformations.
- `OcrCharacter`: Single character glyph with spatial bounds and confidence.
- `OcrWord`: Word token with bounds, confidence, and constituent glyphs.
- `OcrLine` & `OcrBlock`: Hierarchical typographical groupings.
- `OcrPageResult`: Full text, mean confidence, detected language, and optional hOCR representation.

### 3. Multi-Language Detection Heuristics (`engines/ocr/src/language.rs`)

- Evaluates German specific characters (`ä`, `ö`, `ü`, `ß`, `§`).
- Evaluates Austrian legal markers (`ABGB`, `UWG`, `UStG`, `Wien`, `Österreich`, `OGH`) producing `de-AT` variant hints.
- Evaluates high-frequency stopwords and corporate/legal terminology to distinguish `deu` vs `eng` and flag bilingual documents (`is_multilingual`).

### 4. Direct M1.2 Cleanup Pipeline Integration (`engines/ocr/src/lib.rs`)

- `recognize_clean_scan_page`: Pipes a `ScanFrame` through `vocument_imaging::enhance_frame(frame, profile)` before feeding into `TesseractEngine`.
- Verified that `DocumentAuto` and `HighContrastBw` cleanups preserve text legibility and maintain $\ge 95\%$ keyword recognition accuracy.

### 5. Realistic OCR Test Corpus (`tests/corpus/ocr/`)

Contains realistic, rendered test documents with ground truth text and JSON metadata:
- `deu_vertrag_vertraulichkeit`: Multi-paragraph Austrian confidentiality agreement with Umlauts, paragraph symbols, and legal terminology.
- `eng_mutual_nda`: English mutual non-disclosure agreement with commercial clauses.
- `bilingual_rechnung_invoice`: Bilingual German/English commercial invoice with VAT calculations.

---

## Consequences & Verification

- **Positive:** Full local offline OCR with word and glyph bounding boxes.
- **Positive:** Zero external network calls or telemetry.
- **Positive:** Strict `CODE-001` adherence (all hand-written files $\le 600$ lines).
- **Positive:** Resolved M1.2 threshold inconsistencies to authoritative $\ge 90\%$ uniformity and $\ge 80\%$ shadow attenuation.
- **Positive:** 100% test pass rate in Rust workspace, Gradle unit tests, and Kotlin handshake CLI.

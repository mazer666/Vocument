# M3.7 — Document Comparison & Diffing

> **Requirement IDs:** `COMPARE-001`–`COMPARE-004`, `CR-0005`  
> **Architecture Sections:** `ARCHITECTURE.md` §§4, 5, 8, 17  
> **Status:** Implemented & Verified (Rust Core, UniFFI Bridge, macOS AppKit/SwiftUI, Android Jetpack Compose)

---

## 1. Executive Summary

Vocument M3.7 provides a complete, high-fidelity document comparison and diffing engine designed specifically for legal contracts, briefs, and official records. It combines:

1. **Structural Text Diffing (`COMPARE-001`):** Tokenizes text streams into characters, words, or lines. Applies dynamic-programming Longest Common Subsequence (LCS) to detect exact insertions, deletions, replacements, and moved text passages (`DiffOperation::Move`).
2. **Visual Page Diffing (`COMPARE-002`):** Computes pixel-level color deltas between page rasterizations, clusters adjacent difference pixels into spatial bounding boxes (`PdfRectMilliPt`), and overlays difference boxes onto rendered pages.
3. **Global Sequence Page Alignment (`COMPARE-003`):** Uses Needleman-Wunsch sequence alignment to map pages across documents even when pages are inserted, removed, or transposed.
4. **Legal Clause Context (`COMPARE-004`):** Automatically detects statutory and contractual headings (e.g. `§ 1`, `Art. 4`, `Clause 2`, `1.1`) and attaches legal context to every detected change item.
5. **Multi-Format Export (`COMPARE-004`):** Generates structured JSON, formatted Markdown, and self-contained, offline responsive HTML reports.
6. **Native Dual-Pane UI:** Side-by-side synchronized scroll/zoom viewports on macOS (SwiftUI/AppKit) and Android (Jetpack Compose).

---

## 2. Architecture & Module Structure

```
core/vocument-core/src/compare/
├── mod.rs           # Public API: compare_documents, compare_text_streams, compare_pages_visual
├── model.rs         # Domain models: DiffGranularity, DiffOperation, ComparisonOptions, ChangeItem, ComparisonReport
├── text_diff.rs     # Tokenizer, legal clause detection, dynamic programming LCS diffing, move detection
├── visual_diff.rs   # Pixel delta, color thresholding, spatial connected-component clustering
├── aligner.rs       # Needleman-Wunsch global sequence page alignment
└── report.rs        # Multi-format report serializers (JSON, Markdown, responsive offline HTML)
```

### UniFFI Transport Boundary
- `platform/vocument-ffi/src/compare_dto.rs`: Exposes `FfiComparisonReport`, `FfiChangeItem`, `FfiPageDiffResult`, `FfiPageAlignment`, and `FfiComparisonOptions`.
- Exported functions:
  - `document_core_compare_documents(sessionIdA, sessionIdB, options)`
  - `document_core_compare_pages_visual(sessionIdA, pageA, sessionIdB, pageB, options)`
  - `document_core_compare_text_streams(titleA, textA, titleB, textB, options)`
  - `document_core_compare_flow_editor_sessions(editorIdA, editorIdB, options)`

### Deadlock Prevention
Lock acquisitions on document session actors enforce strict lexicographical ordering by `session_id`. When comparing session A and session B concurrently in reverse order, both threads acquire locks in the identical order, eliminating potential lock inversion deadlocks.

---

## 3. Algorithms

### 3.1 Structural Text Diffing (LCS & Myers-variant)
- **Tokenization:** Breaks text into word tokens preserving character offsets. Identifies legal clause boundaries matching `§`, `Art.`, `Artikel`, `Clause`, `Absatz`, or numbered sections `\d+\.`.
- **LCS Matrix:** Constructs $(N+1) \times (M+1)$ cost table to find optimal alignment between token streams.
- **Replacement Grouping:** Contiguous sequences of deletions immediately followed by insertions are coalesced into `DiffOperation::Replace`.
- **Block Move Detection:** Identifies text passages that appear in both documents at different positions without duplicate re-insertions (`DiffOperation::Move { from_index, to_index }`).

### 3.2 Visual Page Diffing & Spatial Clustering
- **Pixel Delta:** Renders both pages at matching DPI (default 72 DPI) and computes Euclidean color distances:
  $$\Delta C = \frac{|R_A - R_B| + |G_A - G_B| + |B_A - B_B|}{3}$$
- **Thresholding:** Pixels with $\Delta C > \text{threshold}$ (default 16/255) are flagged as modified.
- **Spatial Grid Clustering:** Difference pixels are binned into spatial cells (default 10pt distance). Overlapping or adjacent bins are merged into unified `PdfRectMilliPt` bounding boxes.

### 3.3 Global Sequence Page Alignment
- Implements Needleman-Wunsch dynamic programming alignment across pages using Jaccard word similarity as the substitution matrix.
- Insertion/deletion penalty: $-0.2$. Match score: $1.0$.
- Correctly aligns document pairs even when entire pages are inserted in the middle of a contract.

---

## 4. Multi-Format Report Outputs

1. **JSON (`to_json`):** Complete schema-compliant JSON serialization for automated CI pipelines and legal ERP integration.
2. **Markdown (`to_markdown`):** GitHub-flavored markdown with summary KPI tables, aligned page mappings, and categorized change breakdowns with strikethrough/underline formatting.
3. **HTML (`to_html`):** Self-contained, single-file responsive HTML document with embedded CSS. Operates strictly offline without external scripts or fonts.

---

## 5. Verification & Tests

- **Unit Tests (`vocument-core`):**
  - `test_detect_clause_header`
  - `test_identical_texts_yield_zero_diffs`
  - `test_single_insertion_and_deletion`
  - `test_detect_moved_passage`
  - `test_align_identical_pages`
  - `test_align_with_inserted_page`
  - `test_identical_bitmaps_produce_zero_diff`
  - `test_pixel_delta_clusters_into_bounding_box`
  - `test_report_markdown_and_html_rendering`
- **UniFFI E2E Tests (`vocument-ffi`):**
  - `test_ffi_compare_text_streams_identical`
  - `test_ffi_compare_text_streams_with_changes_and_clauses`
  - `test_ffi_compare_pdf_documents_identical_session`
  - `test_ffi_compare_pages_visual`
  - `test_ffi_compare_flow_editor_sessions`
- **Native macOS CLI Handshake (`vocument-mac-handshake`):**
  - Verified full comparison workflow with token diffing, legal clause alignment, and multi-format report exports.

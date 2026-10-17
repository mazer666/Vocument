# Best-in-Class Evaluation Framework

Vocument's "best in class" goal (`REQUIREMENTS.md` §45, `QUALITY.md`) is an engineering measurement standard, not a marketing slogan or feature-count checklist.

---

## 1. Reference Products

Depending on workflow and platform, we benchmark against:

- **Open-Source Breadth:** PDFCraft
- **Desktop Professional PDF:** PDF-XChange Editor, Adobe Acrobat
- **Calm Native Reader / Quick Workflow:** macOS Preview
- **Fast Viewing & Web Standards:** Chromium PDF Viewer (PDFium), Firefox PDF (PDF.js)
- **Mobile Scanning & Annotation:** Leading Android scan/PDF apps

---

## 2. Evaluation Dimensions

Every milestone and major workflow comparison evaluates:

1. **Speed & Responsiveness:** Startup time, time-to-first-page, scroll/zoom FPS, search latency, OCR/save throughput.
2. **Output & Rendering Fidelity:** Font/vector accuracy, scan cleanup clarity, OCR accuracy, roundtrip preservation.
3. **Data Integrity & Safety:** Atomic save, external change detection, crash recovery reliability, verified redaction, signature preservation.
4. **Interoperability:** Clean reopening in foreign viewers without proprietary lock-in.
5. **Workflow Efficiency:** Number of clicks/taps/steps required for common user tasks.
6. **Calmness & Clarity:** Absence of nagging, clear error recovery, progressive disclosure of expert options.
7. **Resource Footprint:** RAM bounds, battery/thermal behaviour on Android and Apple Silicon.

# Engine Adapters (`engines/`)

Contains replaceable engine adapters behind Vocument-owned traits:
- `pdf-mupdf/` — MuPDF implementation of `PdfEngine`
- `ocr/` — `OcrProvider` trait and Tesseract adapter
- `imaging/` — Shared Rust `ImagingEngine` for scan cleanup, deskew, and perspective correction

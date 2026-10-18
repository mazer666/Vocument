# Vocument Realistic Scan & OCR Test Corpus (`tests/corpus/ocr/`)

This directory contains the realistic test corpus used for offline OCR evaluation, language detection, bounding box verification, and text layer synthesis (`ARCHITECTURE.md` §20, `REQUIREMENTS.md` §9).

## Included Documents

| File | Language | Ground Truth | Category / Description |
|---|---|---|---|
| `deu_vertrag_vertraulichkeit.png` | `deu` (`de-AT`) | `deu_vertrag_vertraulichkeit_expected.txt` | Austrian confidentiality agreement with Umlauts (`ä`, `ö`, `ü`, `ß`), paragraph symbols (`§`), and Austrian legal terminology (`ABGB`, `UWG`, `Wien`, `Österreich`). |
| `deu_vertrag_vertraulichkeit_camera_distorted.png` | `deu` (`de-AT`) | `deu_vertrag_vertraulichkeit_camera_distorted_expected.txt` | Realistic camera scan capture on office desk with 6.5° perspective tilt, quadrilateral distortion, non-uniform illumination gradient, and mobile cast shadow. |
| `eng_mutual_nda.png` | `eng` (`en-US`) | `eng_mutual_nda_expected.txt` | English mutual non-disclosure and confidentiality agreement with commercial transaction terms, standard of care clauses, and jurisdiction. |
| `bilingual_rechnung_invoice.png` | `deu+eng` | `bilingual_rechnung_invoice_expected.txt` | Bilingual commercial invoice with German and English headings, VAT calculations (`UStG`), itemized services, and totals. |

## Evaluation Metrics & Hardened Quality Gate

For each corpus document, `vocument-ocr` verifies:
1. **Character Error Rate (CER) & Word Error Rate (WER):** Levenshtein distance on characters and word tokens. Character accuracy must meet or exceed **95.0%** ($\text{CER} \le 5.0\%$).
2. **Keyword Recognition:** Key terms must match with accuracy $\ge \mathbf{95\%}$ (no 85% replacement threshold).
3. **Character & Word Bounding Boxes:** Monotonic, positive area within document dimensions ($0 \le x_0 < x_1 \le W$, $0 \le y_0 < y_1 \le H$).
4. **Confidence Scores:** Mean text confidence $\ge 85.0\%$, word-level confidences, and character-level scores.
5. **Language Detection Accuracy:** Heuristic scoring correctly identifies German vs English and mixed documents.
6. **M1.1 / M1.2 Pipeline Integration:** Distorted camera frames rectified via homography and cleaned via `DocumentAuto` or `HighContrastBw` achieve $\ge 95\%$ character accuracy, whereas unrectified distorted captures fail without the scan pipeline.

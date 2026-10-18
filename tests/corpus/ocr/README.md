# Vocument Realistic Scan & OCR Test Corpus (`tests/corpus/ocr/`)

This directory contains the realistic test corpus used for offline OCR evaluation, language detection, bounding box verification, and text layer synthesis (`ARCHITECTURE.md` §20, `REQUIREMENTS.md` §9).

## Included Documents

| File | Language | Ground Truth | Category / Description |
|---|---|---|---|
| `deu_vertrag_vertraulichkeit.png` | `deu` (`de-AT`) | `deu_vertrag_vertraulichkeit_expected.txt` | Austrian confidentiality agreement with Umlauts (`ä`, `ö`, `ü`, `ß`), paragraph symbols (`§`), and Austrian legal terminology (`ABGB`, `UWG`, `Wien`, `Österreich`). |
| `eng_mutual_nda.png` | `eng` (`en-US`) | `eng_mutual_nda_expected.txt` | English mutual non-disclosure and confidentiality agreement with commercial transaction terms, standard of care clauses, and jurisdiction. |
| `bilingual_rechnung_invoice.png` | `deu+eng` | `bilingual_rechnung_invoice_expected.txt` | Bilingual commercial invoice with German and English headings, VAT calculations (`UStG`), itemized services, and totals. |

## Evaluation Metrics

For each corpus document, `vocument-ocr` verifies:
1. **Character & Word Bounding Boxes:** Monotonic, positive area within document dimensions ($0 \le x_0 < x_1 \le W$, $0 \le y_0 < y_1 \le H$).
2. **Confidence Scores:** Mean text confidence $\ge 85.0\%$, word-level confidences, and character-level scores.
3. **Language Detection Accuracy:** Heuristic scoring correctly identifies German vs English and mixed documents.
4. **Keyword Recognition:** Key terms must match with Levenshtein-distance accuracy $\ge 95\%$.
5. **M1.2 Cleanup Pipeline Compatibility:** Scan frames pre-processed with `DocumentAuto` or `HighContrastBw` maintain $\ge 95\%$ OCR accuracy.

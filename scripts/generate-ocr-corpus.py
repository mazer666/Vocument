#!/usr/bin/env python3
import os
import subprocess
import json

CORPUS_DIR = os.path.abspath("tests/corpus/ocr")
os.makedirs(CORPUS_DIR, exist_ok=True)

def octal_escape_winansi(text: str) -> str:
    """Escapes non-ASCII characters into PDF octal escapes for WinAnsiEncoding."""
    mapping = {
        'ä': r'\344', 'ö': r'\366', 'ü': r'\374',
        'Ä': r'\304', 'Ö': r'\326', 'Ü': r'\334',
        'ß': r'\337', '§': r'\247', '€': r'\200',
        '–': r'\226', '—': r'\227', '„': r'\204', '“': r'\223', '”': r'\224',
        '(': r'\(', ')': r'\)', '\\': r'\\'
    }
    res = []
    for ch in text:
        if ch in mapping:
            res.append(mapping[ch])
        elif ord(ch) < 128:
            res.append(ch)
        else:
            # Fallback to byte in latin1/winansi if possible
            try:
                b = ch.encode('cp1252')[0]
                res.append(f'\\{b:03o}')
            except Exception:
                res.append('?')
    return "".join(res)

def create_simple_pdf(title, lines, filename):
    stream_lines = ["BT", "/F1 15 Tf", "50 780 Td"]
    # Title
    stream_lines.append(f"({octal_escape_winansi(title)}) Tj")
    stream_lines.append("/F1 10 Tf")
    stream_lines.append("0 -24 Td")
    
    for line in lines:
        if not line.strip():
            stream_lines.append("0 -16 Td")
        elif line.startswith("§") or line.startswith("Section") or line.startswith("RECHNUNG") or line.startswith("Rechnung"):
            stream_lines.append("0 -10 Td")
            stream_lines.append("/F1 12 Tf")
            stream_lines.append(f"({octal_escape_winansi(line)}) Tj")
            stream_lines.append("/F1 10 Tf")
            stream_lines.append("0 -18 Td")
        else:
            stream_lines.append(f"({octal_escape_winansi(line)}) Tj")
            stream_lines.append("0 -16 Td")
    stream_lines.append("ET")
    stream_content = "\n".join(stream_lines)
    stream_len = len(stream_content.encode('latin1'))

    pdf_text = f"""%PDF-1.7
1 0 obj
<< /Type /Catalog /Pages 2 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>
endobj
4 0 obj
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>
endobj
5 0 obj
<< /Length {stream_len} >>
stream
{stream_content}
endstream
endobj
xref
0 6
0000000000 65535 f 
0000000009 00000 n 
0000000058 00000 n 
0000000115 00000 n 
0000000244 00000 n 
0000000346 00000 n 
trailer
<< /Size 6 /Root 1 0 R >>
startxref
{346 + stream_len + 50}
%%EOF
"""
    with open(filename, "wb") as f:
        f.write(pdf_text.encode('latin1'))

# 1. German Legal Contract
deu_title = "VEREINBARUNG ÜBER VERTRAULICHKEIT UND GEHEIMHALTUNG"
deu_lines = [
    "Vertrags-ID: AT-VIE-2026-9041 | Geltendes Recht: Österreichisches Recht (ABGB)",
    "",
    "§ 1 Geltungsbereich und vertrauliche Informationen",
    "Die Vertragsparteien vereinbaren die strikte Geheimhaltung aller im Rahmen der",
    "Zusammenarbeit zugänglich gemachten Geschäfts- und Betriebsgeheimnisse gemäß den",
    "Bestimmungen des ABGB und UWG.",
    "",
    "§ 2 Pflichten des Informationsempfängers",
    "Der Empfänger verpflichtet sich, sämtliche überlassenen Unterlagen ausschließlich",
    "für den vereinbarten Vertragszweck zu verwenden und wirksam vor unbefugtem",
    "Zugriff Dritter zu schützen.",
    "",
    "§ 3 Erfüllungsort und Gerichtsstand",
    "Änderungen und Ergänzungen bedürfen der Schriftform. Ausschließlicher Erfüllungsort",
    "und Gerichtsstand für sämtliche Streitigkeiten ist Wien, Österreich."
]
deu_pdf = os.path.join(CORPUS_DIR, "deu_vertrag_vertraulichkeit.pdf")
deu_png = os.path.join(CORPUS_DIR, "deu_vertrag_vertraulichkeit.png")
deu_txt = os.path.join(CORPUS_DIR, "deu_vertrag_vertraulichkeit_expected.txt")
deu_meta = os.path.join(CORPUS_DIR, "deu_vertrag_vertraulichkeit_meta.json")

create_simple_pdf(deu_title, deu_lines, deu_pdf)
with open(deu_txt, "w", encoding="utf-8") as f:
    f.write(deu_title + "\n\n" + "\n".join(deu_lines) + "\n")

json.dump({
    "document_id": "deu_vertrag_vertraulichkeit",
    "language": "deu",
    "locale_variant": "de-AT",
    "category": "legal_contract",
    "expected_keywords": ["VEREINBARUNG", "VERTRAULICHKEIT", "GEHEIMHALTUNG", "Geltungsbereich", "Geschäfts-", "Betriebsgeheimnisse", "ABGB", "UWG", "Empfänger", "Erfüllungsort", "Gerichtsstand", "Wien", "Österreich"],
    "min_confidence": 85.0
}, open(deu_meta, "w"), indent=2)

# 2. English Mutual NDA
eng_title = "MUTUAL NON-DISCLOSURE AND CONFIDENTIALITY AGREEMENT"
eng_lines = [
    "Reference: US-DEL-2026-1182 | Effective Date: October 18, 2026",
    "",
    "Section 1. Purpose and Confidential Information",
    "The parties intend to disclose proprietary technical and financial information strictly",
    "for evaluating a prospective commercial transaction between the organizations.",
    "",
    "Section 2. Standard of Care and Non-Disclosure Obligations",
    "The receiving party shall protect disclosed proprietary data with the same degree",
    "of care used to protect its own sensitive confidential assets, but not less than reasonable care.",
    "",
    "Section 3. Governing Law and Exclusive Jurisdiction",
    "This Agreement shall be governed by and construed in accordance with substantive",
    "statutory corporate regulations without regard to choice of law provisions."
]
eng_pdf = os.path.join(CORPUS_DIR, "eng_mutual_nda.pdf")
eng_png = os.path.join(CORPUS_DIR, "eng_mutual_nda.png")
eng_txt = os.path.join(CORPUS_DIR, "eng_mutual_nda_expected.txt")
eng_meta = os.path.join(CORPUS_DIR, "eng_mutual_nda_meta.json")

create_simple_pdf(eng_title, eng_lines, eng_pdf)
with open(eng_txt, "w", encoding="utf-8") as f:
    f.write(eng_title + "\n\n" + "\n".join(eng_lines) + "\n")

json.dump({
    "document_id": "eng_mutual_nda",
    "language": "eng",
    "locale_variant": "en-US",
    "category": "nda_agreement",
    "expected_keywords": ["MUTUAL", "NON-DISCLOSURE", "CONFIDENTIALITY", "AGREEMENT", "Section", "Purpose", "Proprietary", "Standard", "Obligations", "Governing", "Law", "Jurisdiction"],
    "min_confidence": 85.0
}, open(eng_meta, "w"), indent=2)

# 3. Bilingual Invoice
bi_title = "RECHNUNG / COMMERCIAL INVOICE"
bi_lines = [
    "Rechnungsnummer / Invoice Number: RE-2026-7712",
    "Datum / Date: 18. Oktober 2026",
    "UID-Nummer / VAT Identification: ATU78945612",
    "",
    "1. Software Engineering Services / Softwareentwicklung: 2.000,00 EUR",
    "2. Document Engine Integration / Dokumenten-Architektur: 1.500,00 EUR",
    "",
    "Zwischensumme / Subtotal: 3.500,00 EUR",
    "20% Umsatzsteuer / 20% VAT: 700,00 EUR",
    "Gesamtbetrag / Total Invoice Amount: 4.200,00 EUR",
    "",
    "Zahlbar innerhalb von 14 Tagen ohne Abzug auf das angegebene Bankkonto."
]
bi_pdf = os.path.join(CORPUS_DIR, "bilingual_rechnung_invoice.pdf")
bi_png = os.path.join(CORPUS_DIR, "bilingual_rechnung_invoice.png")
bi_txt = os.path.join(CORPUS_DIR, "bilingual_rechnung_invoice_expected.txt")
bi_meta = os.path.join(CORPUS_DIR, "bilingual_rechnung_invoice_meta.json")

create_simple_pdf(bi_title, bi_lines, bi_pdf)
with open(bi_txt, "w", encoding="utf-8") as f:
    f.write(bi_title + "\n\n" + "\n".join(bi_lines) + "\n")

json.dump({
    "document_id": "bilingual_rechnung_invoice",
    "language": "deu+eng",
    "locale_variant": "de-AT/en",
    "category": "commercial_invoice",
    "expected_keywords": ["RECHNUNG", "COMMERCIAL", "INVOICE", "Rechnungsnummer", "Softwareentwicklung", "Gesamtbetrag", "Total", "Amount", "Umsatzsteuer", "VAT", "Bankkonto"],
    "min_confidence": 85.0
}, open(bi_meta, "w"), indent=2)

print("Rendering PDFs to PNG via mutool...")
for pdf, png in [(deu_pdf, deu_png), (eng_pdf, eng_png), (bi_pdf, bi_png)]:
    subprocess.check_call(["mutool", "draw", "-r", "150", "-o", png, pdf, "1"])
    print(f"Rendered {png}")

print("Corpus generation complete.")

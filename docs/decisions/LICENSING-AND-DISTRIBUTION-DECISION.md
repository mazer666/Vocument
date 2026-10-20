# Entscheidungsvorlage: Lizenzierung und Distribution der PDF-Engine

**Dokumenttyp:** Nicht-bindende technische Entscheidungsvorlage
**Status:** Entwurf zur Entscheidung durch den Projektverantwortlichen
**Datum:** 2026-10-20
**Bezug:** `RISK-009`, `RISK-010`, `ADR-0017`, `THIRD_PARTY.md`, `project/project.yaml`
**Erstellt im Rahmen von:** M1.6 — Plattformintegration und Release Readiness

> **Diese Vorlage ist keine juristische Beratung und keine Lizenzentscheidung.**
> Sie stellt die technischen und lizenzrechtlichen Randbedingungen dar, die für
> eine Entscheidung bekannt sein müssen, und benennt die offenen Fragen. Die
> verbindliche Freigabe bleibt eine Entscheidung des Projektverantwortlichen.

---

## 1. Entscheidungsbedarf

Vocument ist eine native Anwendung (Rust-Core, UniFFI, Swift-UI unter macOS,
Kotlin/UI unter Android). Die PDF-Engine MuPDF 1.28.5 wird **dynamisch** in die
Anwendung gelinkt:

```text
libvocument_ffi.so  --DT_NEEDED-->  libmupdf.so.28.5
```

Vocument selbst steht unter **AGPL-3.0** (`LICENSE`, `project/project.yaml`,
`THIRD_PARTY.md`). MuPDF steht unter **AGPL-3.0-or-later** *oder* ist separat
kommerziell lizenzierbar.

Damit stellt sich eine Frage, die vor jeder öffentlichen Distribution beantwortet
werden muss: **Unter welcher Lizenz wird Vocument veröffentlicht, und wie wird
die MuPDF-Kopplung abgesichert?**

Auslöser: `RISK-009` („AGPL-Quelloffenlegung für die Android-Distribution ist
nicht entschieden") und `RISK-010` (APK-Größe durch `libmupdf.so`).

### 1.1 Warum die dynamische Verknüpfung die Frage nicht auflöst

Die dynamische Verknüpfung ändert an der Copyleft-Wirkung nichts:

* Die AGPL betrachtet „*modify* … *to link or combine with … other works*" (§10)
  als mögliche Auslöser der Weitergabeplicht — unabhängig davon, ob statisch
  oder dynamisch verknüpft wird.
* Entscheidend ist die **konkrete Nutzung** („*mere aggregation*" nach §9/§13
  greift nur, wenn das Werk „*at arm's length* … merely links … without
  *combining* … into a single larger work" bleibt). Ein PDF-Renderer, dessen
  Display-Liste, Pixmaps und Fehlerbehandlung direkt durch den Anwendungscode
  gesteuert werden, ist eine funktionale Kopplung, keine bloße Aggregation.
* Die Kopplung ist im Projekt real: `core/vocument-core/src/pdf_engine.rs`
  definiert das Trait `PdfEngineDocument` mit `create_display_list`,
  `render_tile`, `rotate_pages`, `add_highlight_annotation`,
  `remove_highlight_annotation` und `extract_page_text` — also Funktionalität,
  die im Kern der Engine liegt und nicht durch eine dünne Wrapper-Schicht
  austauschbar ist.

**Folgerung:** Variante „dynamisch gelinkt, deshalb keine Copyleft-Pflicht" ist
**nicht belastbar** und darf nicht als Argument verwendet werden.

---

## 2. Gemeinsame Randbedingungen (gelten in allen Varianten)

Diese Punkte sind variantenunabhängig und gelten heute bereits:

| # | Randbedingung | Beleg / Status |
|---|---|---|
| R1 | MuPDF 1.28.5 wird aus dem offiziellen Quellarchiv gebaut, SHA-256 `98a5c10c…b7e934`, unabhängig bestätigt durch Homebrew und FreeBSD Ports. | `scripts/prepare-android-mupdf.sh` — **erfüllt** |
| R2 | Die MuPDF-Lizenz (AGPL) und die Copyleft-Pflichten werden im Projekt offengelegt. | `THIRD_PARTY.md` — **erfüllt** |
| R3 | Die Vocument-Architektur kennt die Engine nur über das Trait `PdfEngine` / `PdfEngineDocument` / `PdfEngineDisplayList`. | `core/vocument-core/src/pdf_engine.rs` — **erfüllt** |
| R4 | Die Anwendung ruft MuPDF **nicht** aus Swift oder Kotlin, sondern ausschließlich aus dem Rust-Core. | `scripts/validate-repo.sh` (Gate M0.1) — **erfüllt** |
| R5 | Eine öffentliche Distribution ist bis zur Entscheidung **nicht** freigegeben. | Abschnitt 7 — **anzuwenden** |
| R6 | Bei AGPL-Betrieb sind die Attributionspflichten von Artifex zu beachten (Producer-Line in erzeugten PDFs, AGPL-Hinweis in den PDF-Metadaten). | Artifex Licensing — **offen, umzusetzen bei Variante A** |
| R7 | Die Binärgröße (`libmupdf.so` ≈ 56 MB) ist unabhängig von der Lizenzwahl und über Build-Optionen reduzierbar (XPS/CBZ/HTML-EPUB deaktivieren, `strip`, ABI-Splits). | `RISK-010` — **umsetzbar** |

---

## 3. Variante A — AGPL-konforme Open-Source-Veröffentlichung

### 3.1 Was offenzulegen wäre

* **MuPDF selbst:** unverändert, sofern keine Änderungen vorgenommen werden.
  Die Cross-Build-Konfiguration (NDK, `HAVE_LIBCRYPTO=no`, 16-KB-Seitenausrichtung)
  ist eine **Build-Konfiguration**, keine Änderung des MuPDF-Quellcodes.
* **Der Anwendungscode:** Bei funktionaler Kopplung ist die Weitergabe des
  **kompletten Anwendungs-Quellcodes** unter AGPL die praktische Folge. Das
  betrifft Rust-Core, UniFFI-Bindungen, Swift- und Kotlin-UI sowie die
  Build-Skripte.
* **Der „Corresponding Source" der gelinkten MuPDF-Version:** Die AGPL verlangt
  in §6 den vollständigen Quellcode der verwendeten Programmversion. Das ist
  hier unkritisch, weil die verwendete Version bereits öffentlich ist — es
  genügt der Verweis auf das offizielle Archiv **inklusive Checksumme**.

### 3.2 Informationspflichten

| Pflicht | Quelle | Umsetzung im Projekt |
|---|---|---|
| Quellcode des Werks (AGPL) | AGPL §6 | Veröffentlichung des Repositories |
| Quellcode der Corresponding Source der Bibliothek | AGPL §6 | Verweis auf das gepinnte MuPDF-Archiv mit SHA-256 |
| Lizenztext | AGPL §4 | `LICENSE` |
| Installationsinformationen | AGPL §6 | `README.md` + Build-Anleitung |
| Hinweis auf Änderungen | AGPL §6 | Changelog |
| „Best Corresponding Source" bei Weitergabe | AGPL §6 | Im Release-Archiv zu dokumentieren |
| Netzwerknutzung (§13) | AGPL §13 | Vocument ist eine Desktop-/Mobile-App ohne Netzwerkdienst; §13 ist derzeit **nicht einschlägig**. Wird später ein Serverdienst gebaut, greift die Offenlegungspflicht für den gesamten Dienst. |
| Producer-Line in erzeugten PDFs | Artifex-Lizenzhinweis | In der MuPDF-Konfiguration aktiviert lassen |
| AGPL-Hinweis in PDF-Metadaten | Artifex-Lizenzhinweis | Über `fz_set_document_metadata` umsetzbar |

### 3.3 Auswirkungen auf Drittanbieterabhängigkeiten

| Abhängigkeit | Lizenz | Wirkung unter Variante A |
|---|---|---|
| MuPDF | AGPL-3.0 | Bestimmt die Lizenz des Gesamtwerks |
| Tesseract | Apache-2.0 | Permissiv, kompatibel mit AGPL-3.0 |
| Leptonica | BSD-2-Clause | Permissiv, kompatibel |
| SQLite / FTS5 | Public Domain | Kompatibel |
| Rust crates (serde, rusqlite, thiserror …) | MIT / Apache-2.0 | Kompatibel |
| UniFFI | MIT / Apache-2.0 | Kompatibel |
| Swift-UI, Kotlin/Compose | Apache-2.0 | Kompatibel |

**Ergebnis:** Es entsteht **keine zusätzliche Lizenz-Inkompatibilität**. Das
Gesamtwerk steht konsistent unter AGPL-3.0.

### 3.4 Bereitstellung von Quellcode und Build-Anleitung

Für eine belastbare Weitergabe muss das Release-Artefakt enthalten:

1. Das vollständige Quellarchiv des Anwendungscodes (gleicher Commit wie die Binärdatei).
2. Die Build-Anleitung, die den exakten Weg vom Quellcode zur Binärdatei beschreibt
   — einschließlich der NDK-Version, der MuPDF-Checksumme und der Tesseract-Versionspins.
3. Den Hinweis, dass die verwendete MuPDF-Version unter der oben genannten
   Checksumme öffentlich verfügbar ist.

Punkt 2 ist der praktisch anspruchsvollste Teil: Die Build-Anleitung muss auf
einer Maschine ohne Vorkenntnisse funktionieren. **Das ist heute noch nicht
sichergestellt**, weil der native Android-Build ein NDK, ein Android SDK und
eineCross-Build-Umgebung für Tesseract benötigt (siehe `RISK-009` und Abschnitt 6).

### 3.5 Bewertung

| | |
|---|---|
| **Vorteile** | Keine Lizenzkosten; keine Verhandlungen; konsistente, freie Software; die bereits vorhandene AGPL-Lizenz des Projekts passt; die Offenlegung stärkt die Glaubwürdigkeit des Projekts. |
| **Nachteile** | Der gesamte Anwendungscode muss veröffentlicht werden. Das ist eine **strategische** Entscheidung: Proprietäre Erweiterungen, die auf dem Core aufbauen, sind dann nicht möglich. Ein Endnutzer, der eine kommerziell lizenzierte Fachanwendung erwartet, kann dieSoftware nicht Closed Source weiterentwickeln. |
| **Risiko** | Mittel bis hoch: Die Entscheidung ist praktisch nicht umkehrbar, sobald der Quellcode des Kerns öffentlich ist. |
| **Aufwand** | Gering (Lizenz ist bereits AGPL) + mittel (Release-Prozess mit Quellcode-Bundle, Build-Anleitung, Metadaten-Attribution). |

---

## 4. Variante B — Kommerzielle MuPDF-Lizenz

### 4.1 Öffentlich belegbare Fakten

Aus den offiziellen Quellen von Artifex (`artifex.com/licensing`,
`mupdf.com`, `mupdf.readthedocs.io`):

* MuPDF ist **dual lizenziert**: AGPL-3.0 **oder** kommerzielle Lizenz.
* Artifex nennt zwei kommerzielle Modelle:

| Modell | Öffentlich belegbare Eigenschaften |
|---|---|
| **OEM Distribution License** | Verteilung ohne Anforderungen an die Offenlegung des eigenen Codes; **jährliche Mengenmeldung**; Support verfügbar; **Kosten pro Kopie** mit **quartalsweisem Mindestumsatz** |
| **Subscription License** | Verteilung ohne Offenlegungspflicht; **jährliche Mengenmeldung** (keine Meldung bei interner Nutzung); Support verfügbar; **Kosten pro Kopie** mit **quartalsweisem Mindestumsatz** |

* Die kommerzielle Lizenz gilt laut Artifex für „*SaaS*, *OEM software* oder
  *in-house*" und befreit vollständig von den AGPL-Beschränkungen.
* Support und kostenlose Upgrades sind Teil der kommerziellen Lizenz.

### 4.2 Nicht öffentlich belegbare Informationen

> **Preise sind nicht öffentlich belegt.** Artifex nennt kein Preisniveau, keine
> Mindestabnahme in Euro und keine Laufzeit. Jede konkrete Zahl in diesem
> Dokument wäre erfunden. Die Angaben „per-copy cost with a quarterly minimum
> fee" sind die **einzige** öffentlich belegbare Kostenaussage; die Höhe der
> Mindestgebühr und der Stückpreise muss erfragt werden.

### 4.3 Mit Artifex zu klärende vertragliche Fragen

1. Welches Modell (OEM vs. Subscription) passt zur geplanten Distribution?
2. Was ist die Lizenz für **Distribution an Endnutzer** (die App wird an Nutzer ausgeliefert)?
3. Gilt die Lizenz für die **verwendete Version 1.28.5** und deckt sie Updates ab?
4. Welche **Mengenmeldung** ist erforderlich, und ab welchem Volumen?
5. Welche **Mindestabnahme** und Laufzeit gelten?
6. Welche Rechte bestehen bei Weitergabe an **Drittanbieter** (Vertriebspartner, OEM)?
7. Gibt es ein **Audit** oder Nachweisverpflichtungen?
8. Welche **Support-SLAs** und Reaktionszeiten sind zugesagt?
9. Enthält die Lizenz eine **Freistellung** (Indemnity)?
10. Was passiert bei **AGPL-kompatibler** Veröffentlichung unter Variante A — gibt es eine „Dual-Use"-Klausel, die zurück in die AGPL-Welt führt?

### 4.4 Auswirkungen auf die Architektur

**Keine.** Die dynamische Verknüpfung bleibt bestehen. Der Build, die
Cross-Build-Pipeline und die Trait-Grenze `PdfEngine` bleiben unverändert. Das
ist ein wesentlicher Vorteil gegenüber Variante C.

### 4.5 Bewertung

| | |
|---|---|
| **Vorteile** | Keine Offenlegung des eigenen Codes. Kein strategisch irreversibler Schritt. Der Quellcode des Kerns bleibt proprietär. Support und Updates sind Teil der Lizenz. Die Architektur bleibt unverändert. |
| **Nachteile** | Laufende Kosten, deren Höhe erst nach einer Anfrage bekannt ist. Mengenmeldung und Mindestumsatz binden auch bei geringem Absatz. Ein Anbieter-Sonderwunsch bindet das Projekt an einen Hersteller. |
| **Risiko** | Finanziell (Kostenhöhe unbekannt) und strategisch (Abhängigkeit von einem einzelnen Anbieter). |
| **Aufwand** | Verhandlung, Vertragsprüfung, Umstellung der Distribution auf eine lizenzierte MuPDF-Version, Anpassung der Attributionspflichten. |

---

## 5. Variante C — Alternative Rendering-Engine

### 5.1 Ist ein späterer Austausch technisch realistisch?

**Ja, aber nicht ohne Aufwand.** Die Austauschgrenze existiert und ist
sauber gezogen:

* `core/vocument-core/src/pdf_engine.rs` definiert `PdfEngineDisplayList`,
  `PdfEngineDocument` und das Fehler-Trait `PdfEngineError`.
* `engines/pdf-mupdf/` enthält die einzige MuPDF-Implementierung.
* `platform/vocument-ffi/src/lib.rs` registriert sie über
  `SessionRegistry::new(Box::new(MuPdfEngineAdapter::new()))` — eine einzige
  Austauschstelle.

Der Aufwand liegt nicht im Trait, sondern im **Funktionsumfang**: Das Trait
verlangt `rotate_pages`, `add_highlight_annotation`, `remove_highlight_annotation`
und `extract_page_text` — also **PDF-Mutation** und nicht nur Rendering. Die
meisten Alternativen sind reine Renderer.

### 5.2 Kandidaten

| Engine | Lizenz | Rendering | Mutation | Text | Bewertung |
|---|---|---|---|---|---|
| **PDFium** | BSD-3-Clause **und** Apache-2.0 (dual) | Ja, sehr gut | Nein (nur Annotationen-API) | Ja | **Beste Lizenzoption.** Permissiv, keine Copyleft-Pflicht. Aber: keine Mutations-API, die Vocument für `rotate_pages` braucht; zusätzliche C++-Abhängigkeit, die in den Rust-Core eingebunden werden muss (Fremdcode-Sicherheitsrisiko). |
| **pdf.js** | Apache-2.0 | Ja (Browser/Canvas) | Nein | Ja | Renderer, kein natives Core-Library. Für eine native App ungeeignet. |
| **PDFBox** | Apache-2.0 | Java | Ja | Ja | Java-Abhängigkeit in einer Rust-Architektur; nur als Nebenpfad sinnvoll. |
| **Poppler** | GPL-2.0+ | Ja | Begrenzt | Ja | **GPL**: Kopplung erzwingt GPL des Gesamtwerks — löst das Problem nicht. |
| **Ghostscript** | AGPL-3.0 / kommerziell | Ja | Begrenzt | Ja | Wie MuPDF: AGPL-Problem bleibt, plus ImageMagick-typische Ghostscript-Anachronismen und schlechtere Unicode-/Textqualität. |
| **Xpdf** | GPL / kommerziell | Ja | Nein | Ja | GPL-Problem + Funktionsumfang. |

### 5.3 Migrationskosten und -risiken

| Risiko | Bewertung |
|---|---|
| Funktionsverlust bei `rotate_pages` / Annotation-Mutation | **Hoch.** Das ist Kernfunktionalität von M0.4/M0.7. PDFium bietet keine Seitenrotation in veränderbarer Form; sie müsste im Anwendungscode auf PDF-Objektebene implementiert werden. |
| Textqualität | Mittel. Die vorhandenen OCR- und Extraktionstests (`M1.3`, `M1.4`) müssten neu kalibriert werden. |
| Build-Komplexität | Mittel bis hoch. PDFium ist ein großes C++-Projekt mit eigenen Build-Abhängigkeiten; die Android- und macOS-Cross-Builds müssten neu etabliert werden. |
| Sicherheitsprofil | Mittel. Ein C++-Fremdprojekt vergrößert die Angriffsfläche, die Rust nicht abdeckt. |
| Aufwand für den jetzigen Stand | **Hoch** (Wochen). |

### 5.4 Bewertung

| | |
|---|---|
| **Vorteile** | PDFium würde das Copyleft-Problem **technisch** lösen (permissive Lizenz). |
| **Nachteile** | Löst das Problem nur durch Verlust der Kernfunktionalität oder durch hohen Neuaufwand. Der strategische Gewinn ist begrenzt, weil das Projekt bereits AGPL-lizenziert ist. |
| **Risiko** | Hoch: Funktionsverlust in der Mutation, Regression in Textqualität und OCR-Gate, vollständiger Neuaufbau der Cross-Build-Pipeline. |
| **Empfehlung** | **Nicht jetzt.** Variante C ist der Ausweg, falls sich später eineproprietäre Distributionsstrategie als zwingend erweist und Variante B nicht verfügbar ist. Der Trait-Schnittpunkt ist bewusst so geschnitten, dass dieser Weg offen bleibt. |

---

## 6. Vergleich

| Kriterium | A: AGPL-konform | B: Kommerziell | C: PDFium (später) |
|---|---|---|---|
| Eigener Quellcode muss offen | **Ja** | Nein | Nein |
| Laufende Kosten | Keine | **Ja, Höhe unbekannt** | Keine |
| Strategische Umkehrbarkeit | **Schlecht** (Code einmal öffentlich) | Gut | Gut |
| Architekturänderung | Keine | Keine | **Groß** |
| Funktionsumfang bleibt | Ja | Ja | **Einschränkungen bei Mutation** |
| Verhandlungsbedarf | Keiner | **Ja, mit Anbieter** | Keiner |
| Sicherheitsprofil | Unverändert | Unverändert | **Neue C++-Abhängigkeit** |
| Aufwand | Gering–mittel | Mittel | Hoch |
| Restrisiko | Mittel | Finanziell | Hoch |

---

## 7. Empfehlung und offene Entscheidungsfragen

### 7.1 Empfehlung

**Variante A (AGPL-konforme Open-Source-Veröffentlichung) wird technisch
empfohlen**, unter zwei ausdrücklichen Bedingungen:

1. **Nur wenn** die Offenlegung des Anwendungsquellcodes strategisch gewollt
   ist. Das ist eine Geschäftsentscheidung, keine technische. Der
   Projektverantwortliche entscheidet das.
2. **Nur wenn** vor der Veröffentlichung die Release-Kette steht (siehe 7.3).

**Variante B** ist die empfohlene Alternative, wenn der Anwendungscode
proprietär bleiben soll. Sie ist **vor** einer öffentlichen Distribution
anzustoßen, weil die Anfrage Zeit braucht und die Antwort die
Entscheidungsgrundlage verändert.

**Variante C wird nicht jetzt empfohlen.** Der Funktionsverlust bei der
PDF-Mutation ist mit dem aktuellen Produktstand (M0.4/M0.7) nicht vereinbar.

Diese Empfehlung ist **bewusst als vorläufig** markiert: Sie basiert auf den
öffentlich belegbaren Fakten zu den drei Varianten, nicht auf einer
Kostenschätzung, die es derzeit nicht gibt.

### 7.2 Offene Entscheidungsfragen an den Projektverantwortlichen

| # | Frage | Warum sie blockiert |
|---|---|---|
| E1 | Soll der Quellcode des Anwendungscores öffentlich werden? | Bestimmt Variante A vs. B vs. C. |
| E2 | Ist eine proprietäre Verteilung der Ziel (Fachanwendung, Closed Source)? | Bestimmt Variante A vs. B. |
| E3 | Welches Mengengerüst ist realistisch? | Bestimmt die kommerziellen Kosten in Variante B. |
| E4 | Soll ein Serverdienst entstehen? | Bei Variante A greift AGPL §13 und verpflichtet zur Offenlegung des Dienstes. |
| E5 | Wer haftet für die rechtliche Prüfung? | Diese Vorlage ist keine Beratung. |

### 7.3 Was bis zur Freigabe nicht passieren darf

* **Keine öffentliche Distribution** von APK, AAB, DMG oder Installer, die MuPDF
  enthält, als „freigegeben" behandeln.
* **Keine Aufnahme in App-Stores** ohne die jeweils passende Variante.
* **Keine proprietären Erweiterungen** aufbauen, solange E1 offen ist — sonst
  entsteht eine Mischlage, die später nicht sauber auflösbar ist.
* **Keine Engine-Migration** beginnen (Variante C), solange E1 und E2 offen sind.
* Die **Producer-Line** und die **AGPL-Informationen in den PDF-Metadaten** sind
  unabhängig von der Variantenentscheidung umzusetzen, sobald AGPL-Betrieb
  vorgesehen ist (R6).

### 7.4 Unabhängig von der Entscheidung sofort umsetzbar

Diese Punkte verbessern den Zustand in jeder Variante:

1. **Release-Prozess für Variante A vorbereiten** (Quellcode-Bundle, Checksummen,
   Build-Anleitung inkl. NDK-/SDK-/MuPDF-Pins).
2. **Attribution in den PDF-Metadaten** ergänzen (Producer-Line, AGPL-Hinweis).
3. **`RISK-010` (Größe) reduzieren**: MuPDF-Build-Optionen für ungenutzte
   Formate (XPS, CBZ, HTML/EPUB) deaktivieren, `strip` verwenden, ABI-Splits
   anbieten. Das ist **reproduzierbar** und ohne funktionale Regression
   nachweisbar — der Pixelvergleich gegen die Referenzrendering ist der
   Nachweis dafür.
4. **Antwort von Artifex einholen**, um E3 mit belastbaren Zahlen zu
   unterlegen (auch bei einer späteren Entscheidung für Variante A ist das
   eine sinnvolle Grundlage).

---

## 8. Quellen

| # | Quelle | URL | Verwendet für |
|---|---|---|---|
| Q1 | Artifex — Licensing | https://artifex.com/licensing | Modelle, Pflichten, Attributionshinweise, „Can't comply with AGPLv3?" |
| Q2 | Artifex — Products | https://artifex.com/products | MuPDF (Core) als Produkt, kommerzielle Lizenzierung |
| Q3 | Artifex Software — GitHub | https://github.com/ArtifexSoftware | Dual-Lizenzierung, Zweck der kommerziellen Lizenz |
| Q4 | MuPDF — Lizenzdokumentation | https://mupdf.readthedocs.io/en/1.28.3/license.html | „GNU AGPL", „commercial license", kein Garantie-/Support-Versprechen |
| Q5 | Artifex — MuPDF-Downloads (Release 1.28.5) | https://github.com/ArtifexSoftware/mupdf-downloads/commit/c3c899f29d0640b6b23f4d5d45cf8c99074cb295 | Versionierung der verwendeten Version |
| Q6 | GNU AGPL v3 (Canonical) | https://www.gnu.org/licenses/agpl-3.0.html | §6 (Weitergabe), §9/§10 (Kopplung), §13 (Netzwerk) |
| Q7 | PDFium — LICENSE | https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE | BSD-3-Clause + Apache-2.0, permissiv |
| Q8 | pdf.js — Vergleichswerkzeug mit Lizenzübersicht | https://github.com/mozilla/pdf.js.comparator/blob/main/README.md | Lizenzvergleich der Alternativ-Engines (Poppler GPL, Ghostscript AGPL, Xpdf GPL, PDFBox/pdf.js/PDFium Apache) |
| Q9 | PDF Association — Survey of Open-Source PDF Solutions | https://pdfa.org/wp-content/uploads/2021/06/Survey-of-OpenSource-Solutions.pdf | Funktionsumfang der Alternativen (Overprint, AcroForms, Annotationen, Text) |
| Q10 | Homebrew — `mupdf-tools` Formula | https://github.com/Homebrew/homebrew-core/blob/cbd0c8ddb11909fb209f06e08677973c0f211848/Formula/m/mupdf-tools.rb | Unabhängige Bestätigung der MuPDF-1.28.5-SHA-256 |
| Q11 | FreeBSD Ports — `graphics/mupdf` | https://github.com/FreeBSD/freebsd-ports/commit/602c6870f02e89e175ee855bff0e26c1a42b2970 | Unabhängige Bestätigung der MuPDF-1.28.5-SHA-256 und -Dateigröße |

**Projektinterne Belege:** `THIRD_PARTY.md`, `LICENSE`, `project/project.yaml`,
`docs/project/RISKS.md` (`RISK-009`, `RISK-010`),
`docs/adr/ADR-0017-real-android-mupdf-cross-build.md`,
`core/vocument-core/src/pdf_engine.rs`, `scripts/validate-repo.sh`.

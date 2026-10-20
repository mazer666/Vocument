# Zwischenstandsbericht — Runde 2: M2.1 gehärtet, M2.2 und M2.3 begonnen

- **Zeitraum:** Runde 2 (Fortsetzung nach `docs/reports/M1-NACHBEREITUNG-UND-M2-BEGINN.md`)
- **Ausgangsstand:** `main` = `5f615da`
- **Endstand dieser Runde (vor dem Abschlussbericht):** `main` = `cc564e6`
- **Kennzahlen:** 161 Tests / 23 Suites / 0 Fehler · clippy 0 · fmt 0 · `validate-repo.sh` 0 · Android `assembleDebug` grün · 8/8 Android-Unit-Tests · alle nativen Bibliotheken 16-KB-seitenkompatibel

---

## 1. Was diese Runde geleistet hat

| # | Vorhaben | Ergebnis | Nachweis |
|---|---|---|---|
| 1 | M2.1 sicherheitskritisch nachprüfen | **7 Befunde, davon 2 kritisch — alle behoben** | Commit `17b6f58` |
| 2 | M2 weiterentwickeln: nutzbare, vollständig integrierte Funktionen | **M2.2 (Link-/Aktionsprüfung) und M2.3 (Anhangs-/Signaturprüfung) implementiert und verifiziert** | Commits `8d07725`, `cc564e6` |
| 3 | Architektur, Datenintegrität, Sicherheit, Plattformkompatibilität erhalten | unverändert eingehalten; Schichtentrennung Engine → Core → FFI in beiden neuen Funktionen eingehalten | siehe §4 |
| 4 | Tests, Abnahmekriterien, Dokumentation, Projektstatus aktuell halten | 8 neue Tests, 2 Abnahmerecords, 2 Gate-Erweiterungen, Anforderungen/Roadmap/Risiken/Doku nachgezogen | siehe §3 |
| 5 | Committen und auf `main` pushen | 3 Commits, direkt auf `main` | `17b6f58`, `8d07725`, `cc564e6` |

## 2. Die wichtigste Erkenntnis der Runde

**Eine Redaction, die als sicher gemeldet wird, ist eine Behauptung, keine
Eigenschaft.** Die Prüfung dieser Runde hat an der *eigenen* Implementierung
von M2.1 sieben Schwachstellen gefunden — zwei davon kritisch:

1. Der entfernte Text blieb in den **Bytes der Datei**. MuPDF schreibt
   unerreichbare Objekte mit, wenn die Garbage Collection nicht angefordert
   wird. Das Dokument sah redigiert aus und enthielt den Text weiterhin.
   Behoben mit `opts.do_garbage = 1`.
2. Die **Bildpixel** der Region blieben stehen. Ein Dokument trägt denselben
   Text zweimal: als Textebene und als Pixel im Bild. Beim Scan ist die
   Textebene unsichtbar — das Bild trägt die Wörter. Behoben mit
   `PDF_REDACT_IMAGE_PIXELS`.

Dazu kam die Feststellung, dass der bisherige **schwarze Kasten** die
Prüfung wertlos machte: Er verdeckte genau das, was die Prüfung sehen
musste. Mit `black_boxes = 0` zeigt die gerenderte Seite die Wahrheit.

Die Konsequenz für die Architektur: eine unvollständig verifizierte Redaction
kann nirgends als Erfolg behandelt werden. Die FFI meldet sie als **Fehler**
(`RedactionUnverified`), und `NotChecked` ist ausdrücklich kein `Passed`.

**Diese Lehre ist in M2.2 und M2.3 direkt eingeflossen:** die beiden neuen
Funktionen trennen Fakten und Bewertung. Die Engine meldet, was da ist; der
Core entscheidet, was riskant ist. Die Entscheidung ist damit ohne PDF
testbar — fünf Core-Tests ohne jede Datei.

## 3. Die Nachweise

### 3.1 M2.1 — Redaction (Commit `17b6f58`)

Die Verifikation prüft jetzt **vier** Dinge, weil sie vier verschiedene
Fehlerbilder haben:

| Prüfung | Was sie beweist |
|---|---|
| Textebene | Die Wörter der Region sind nicht mehr extrahierbar |
| Volltextsuche (FTS5) | Der Index findet sie nicht mehr |
| Gerenderte Seite | Auf dem Bild der Seite ist an der Stelle nichts mehr lesbar (gerendert und mit OCR gelesen) |
| Dateibytes | Die Wörter stehen nicht mehr als Klartext in der Datei |

Ohne Reader für die gerenderte Seite gilt die Prüfung als *nicht durchgeführt* —
das ist kein bestandener Test. Die Prüfmenge ist **wortgenau** (Wortzentrum in
der Region), nicht zeilengrau.

Neue Tests: 3 Engine-Tests (Bildpixel werden ersetzt statt verdeckt; andere
Seiten bleiben intakt; die Region ist in der Bildschirmdarstellung leer), 2
E2E-Tests über die FFI-Grenze (gelöschter Text ist weg in allen vier Kanälen,
Undo wird abgelehnt, Resttext überlebt; eine Redaction, die nichts entfernt,
wird verweigert).

Abnahmerecord: `docs/acceptance/M2.1-ACCEPTANCE.md` — Abschnitt 5 listet alle
sieben Befunde mit Schweregrad und Behebung.

### 3.2 M2.2 — Link- und Aktionsprüfung (Commit `8d07725`)

Die Prüfung meldet die Aktionen des Dokuments: die Links jeder Seite mit
Typ, Ziel und Position im Seitenraum, und die Aktion, die beim **Öffnen**
läuft. Riskant ist, was Code ausführt, Dateien startet, Formulare absendet
oder Daten importiert — und jeder Link, der die Maschine verlässt
(`file:`, `http:`, `https:`, `javascript:`).

Zwei Fehler, die nur der Test gefunden hat:

- MuPDF führt Link-Annotationen **nicht** in der Annotationsliste der Seite
  (`pdf_sync_annots` überspringt `/Link` und `/Popup`). Über `pdf_first_annot`
  fand die erste Fassung **nichts** — für ein Dokument mit fünf Links kam eine
  leere Liste zurück.
- Nur beim direkten Lesen des `/Annots`-Arrays kennt man auch den **Typ** der
  Aktion. Die geladenen Link-Flächen tragen nur das Ziel; gerade die
  entscheidende Information („das ist ein Skript, kein Link") wäre verloren
  gegangen.

Tests: 3 Engine-Tests (alle Aktionen werden gemeldet; die Position wird im
Seitenraum gemeldet; ein Dokument ohne Aktionen ist leer), 3 Core-Tests für
die Risikopolitik, 1 E2E-Test über die FFI-Grenze.

Abnahmerecord: `docs/acceptance/M2.2-ACCEPTANCE.md`.

### 3.3 M2.3 — Anhangs- und Signaturprüfung (Commit `cc564e6`)

Die Prüfung meldet die eingebetteten Dateien (Name, Typ, Größe, ob der Inhalt
wirklich im Dokument liegt, ob die Prüfsumme stimmt) und die Signaturfelder
(Signatur, Zeit, Grund, Ort) — und vor allem, ob das Dokument **nach** der
Signatur geändert wurde. Das entscheidet die Engine, weil nur sie den
Byte-Bereich der Signatur lesen und mit der Größe der Datei vergleichen kann;
der Core entscheidet danach nur noch, was das bedeutet.

Zwei Fehler, die nur der Test gefunden hat:

- Der MIME-Typ eines eingebetteten Anhangs muss im PDF-Namen **maskiert**
  werden: `/Subtype /application#2Foctet-stream`. Unmaskiert meldet MuPDF
  `invalid key in dict` — und die Anlage fehlt **ohne** Fehler im Dokument.
- Die Änderungsprüfung braucht das **Feld**, nicht die Signatur
  (`annotation is not a signature widget`).

Das Prüfdokument wird in zwei Varianten gebaut: die Signatur deckt die ganze
Datei oder nur ihren Anfang. Der Byte-Bereich wird mit fester Stellenzahl
eingesetzt, damit die Offsets gültig bleiben — es gibt also kein Prüfdokument,
dessen Signatur nur auf dem Papier gilt.

Tests: 5 Engine-Tests, 2 Core-Tests, 2 E2E-Tests über die FFI-Grenze.

Abnahmerecord: `docs/acceptance/M2.3-ACCEPTANCE.md`.

### 3.4 Die Gates

`scripts/validate-repo.sh` prüft jetzt zusätzlich:

| Gate | Was es erzwingt |
|---|---|
| M2.1 | `PDF_REDACT_TEXT_REMOVE`, `PDF_REDACT_IMAGE_PIXELS`, `opts.black_boxes = 0`, die Bildprüfung im Core, `RedactionUnverified` an der FFI-Grenze, die beiden Engine-Tests |
| M2.2 | `document_actions` in der Engine-Schnittstelle, `inspect_document` im Core, die Risikopolitik im Core, die FFI-Funktion, die drei Tests |
| M2.3 | `document_attachments` und `document_signatures`, `changed_since_signing` und `is_risky_attachment` im Core, die Engine- und E2E-Tests |

## 4. Architektur und Plattform

- **Schichtentrennung:** Beide neuen Funktionen trennen Fakten und Bewertung.
  Die Engine meldet (Typ, Ziel, Position, Byte-Bereich); der Core entscheidet
  das Risiko. Die Engine enthält keine Policy, der Core keine PDF-Spezifikation.
- **Keine neue Abhängigkeit, keine Änderung der öffentlichen API** der
  bestehenden Typen; die neuen Typen sind additiv.
- **Plattformkompatibilität:** die neue C-Brücke (`mupdf_inspect_bridge.c`,
  `mupdf_attach_bridge.c`) wird für Android mitgebaut und im APK geprüft:
  alle nativen Bibliotheken sind 16-KB-seitenkompatibel, das APK-Gate ist grün.
- **Keine Mocks, keine Stubs:** alle Nachweise laufen gegen die echte
  MuPDF-Engine und — im E2E-Fall — gegen die echte OCR-Kette.

## 5. Was weiterhin offen ist

| Punkt | Stand | Warum |
|---|---|---|
| macOS-Oberfläche aller drei Funktionen | **NOT RUN** | kein macOS-Host |
| Verhalten auf einem Gerät (Android, macOS) | **NOT RUN** | kein Gerät, kein `/dev/kvm` |
| Interoperabilität mit Acrobat/Foxit/Okular | **NOT RUN** | M2-Gate, für alle M2-Funktionen offen |
| Prüfung der Signatur selbst (Kette, Zertifikat) | **nicht begonnen** | MuPDF bietet dafür in dieser Fassung keine öffentliche Schnittstelle; geprüft wird die *Abdeckung* der Signatur |
| M2: Kommentare, Stempel, Vergleich, Palettenschiene | **nicht begonnen** | die nächsten Schnitte |
| Lizenzentscheidung E1–E5 | offen | keine Veröffentlichung als „freigegeben" |
| AddressSanitizer | **NOT RUN** | nur Nightly `-Zbuild-std`; Ersatz valgrind memcheck |
| CI-Workflow | **NOT RUN** | kein Runner; es wird kein PASS behauptet |

## 6. Ausblick

Der nächste Schnitt in M2 ist die **Kommentar- und Anmerkungsfunktion** mit
der juristischen Palettenschiene (`ANNOT-001`): sie ist host-verifizierbar, weil
Annotationen und Formulare in der vorhandenen Engine- und Transaktionsschicht
bereits stehen, und sie ist die Grundlage für Stempel und Vergleich.

Die unabhängige adversarische Zweitprüfung der gehärteten Redaction läuft
parallel; ihr Ergebnis wird in den Abschlussbericht dieser Runde
eingearbeitet.

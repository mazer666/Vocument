# Milestone Acceptance Record — M1: Android Scan & OCR

- **Milestone:** `M1 — Android Scan & OCR` (`ROADMAP.md` §M1, `project/project.yaml`)
- **Status:** **PARTIALLY PASSED** — die funktionalen Anforderungen sind auf dem Host nachgewiesen; die geräte- und plattformabhängigen Nachweise bleiben **NOT RUN**
- **Date:** 2026-10-20
- **Unterstufen:** [M1.0](M1.0-ACCEPTANCE.md) · [M1.1](M1.1-ACCEPTANCE.md) · [M1.2](M1.2-ACCEPTANCE.md) · [M1.3](M1.3-ACCEPTANCE.md) · [M1.4](M1.4-ACCEPTANCE.md) · [M1.4b/M1.5](M1.4b-M1.5-ABSCHLUSSBERICHT.md) · [M1.6](M1.6-ACCEPTANCE.md)
- **Abschlussbericht:** `docs/reports/M1.6-ABSCHLUSSBERICHT.md`

---

## 1. Was M1 umfasst

M1 liefert den ersten mobilen Mehrwert: **Scannen → Bildkorrektur → OCR →
durchsuchbares, speicherbares PDF**. Der Meilenstein umfasst die Unterstufen
M1.0 bis M1.6.

Die Pipeline in einem Satz: `Kamera → Kanten → Entzerrung → Bereinigung → OCR →
durchsuchbares PDF → Volltextsuche → Speichern`.

---

## 2. Ergebnis-Übersicht

### 2.1 Funktionale Anforderungen

| # | Anforderung | Plattform | Umgebung | Nachweis | Ergebnis |
|---|---|---|---|---|---|
| 1 | Kamerarahmen einlesen (`Rgb8`, `Rgba8`, `Gray8`, `Nv21`, `Yuv420`) | Host / Android | Rust | `cargo test -p vocument-imaging` | **PASS** |
| 2 | Dokumentkanten erkennen | Host | Rust, Korpus | `edges::edges_tests` | **PASS** (Toleranz 0,05 gegen die Referenz) |
| 3 | Perspektivische Entzerrung | Host | Rust | `perspective::tests` | **PASS** |
| 4 | Skew-Korrektur und automatische Orientierung | Host | Rust | `cargo test -p vocument-imaging` | **PASS** |
| 5 | Hintergrund-Bereinigung, Schattenentfernung, Kontrast | Host | Rust | `cleanup::tests` | **PASS** |
| 6 | OCR offline, ≥ 95 % Zeichengenauigkeit (deutsch) | Host | Tesseract 5.3.0 | `cargo test -p vocument-ocr --test corpus_tests` | **PASS** |
| 7 | OCR mit Bounding-Boxen und Konfidenz | Host | Tesseract | dasselbe | **PASS** |
| 8 | Durchsuchbares PDF mit unsichtbarer Textebene | Host | MuPDF 1.28.5 | `cargo test -p vocument-ocr --test synthesis_tests` | **PASS** |
| 9 | PDF in eine `DocumentSession` übernehmen | Host | Rust | `synthesis_tests` | **PASS** |
| 10 | Progressive SQLite-FTS5-Suche | Host | SQLite | `synthesis_tests` | **PASS** |
| 11 | **Durchgängiger Workflow** Kamera → Suche → Speichern | Host | alles | `platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` | **PASS** |
| 12 | Mehrseitige Scan-Session | Host / Android | Rust, Kotlin | `cargo test`, `ScanCaptureBridgeTest` | **PASS** |
| 13 | Speichern des gescannten Dokuments (Android-UI) | Android | Kotlin | Code + `verify-android-runtime.sh` | **PASS (Code) / NOT RUN (Gerät)** |

### 2.2 Qualitäts- und Betriebsanforderungen

| # | Anforderung | Nachweis | Ergebnis |
|---|---|---|---|
| 14 | OCR blockiert den UI-Thread nicht | `ScanCaptureScreen.kt` (Aufnahme **und** PDF-Erzeugung laufen auf dem Executor) | **PASS (Code) / NOT RUN (Laufzeit)** |
| 15 | Hintergrundarbeit ist zuverlässig, priorisiert, abbrechbar | 36 Job-Tests | **PASS** |
| 16 | Energie- und Thermikanpassung (`POWER-001`, `BIGMOBILE-001`) | `power::power_tests` | **PASS (Host) / NOT RUN (Gerät)** |
| 17 | Pause und Fortsetzung, resumierbare Jobs | `executor_tests` | **PASS** |
| 18 | Der Shutdown des Pools blockiert nie unbegrenzt | `a_job_that_ignores_the_cancellation_does_not_block_the_shutdown_forever` | **PASS** |
| 19 | Schwierige PDF-Dateien: kein Absturz, kein Hänger, saubere Fehler | `engines/pdf-mupdf/src/difficult_tests.rs` | **PASS** (8 Fixtures) |
| 20 | Speicherverbrauch des pathologischen Korpus | `VmHWM` des Testprozesses | **PASS** (5,9 MB → 8,3 MB) |
| 21 | Keine Speicherfehler, keine Leaks | valgrind | **PASS** (0 Fehler, 0 Bytes verloren) |
| 22 | Android-Paket: auflösbare Abhängigkeiten, 16-KB-Seiten | `verify-android-native-libs.sh` | **PASS** |
| 23 | Release-Paket geprüft | dasselbe Gate auf der Release-APK | **PASS** (95 MB) |
| 24 | Nur echte Engines, keine Attrappen | `validate-repo.sh` (QA-003) | **PASS** |
| 25 | Gates: fmt, clippy, Tests, Validator | lokaler Lauf | **PASS** |

---

## 3. Die Nachweiskette, die in M1.6 dazugekommen ist

Der End-to-End-Test
`platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` geht die **gesamte**
Kette auf der UniFFI-Grenze — mit derselben Funktion, die die App benutzt:

```
Kameraframe (grau, 1500×2050)
  → Kantenerkennung           (real, kein gesetzter Wert)
  → Scan-Session              (Seite anhängen)
  → Entzerrung + Bereinigung (M1.1 / M1.2)
  → OCR                       (Tesseract, deutsch)
  → durchsuchbares PDF        (unsichtbare Textebene)
  → Session-Übernahme         (DocumentSession)
  → FTS5-Suche                ("Vertraulichkeit" gefunden)
  → Speichern auf die Platte
  → Wieder öffnen             (MuPDF)
  → Text im gespeicherten PDF wiederlesbar
```

Laufzeit: **4,2 s** auf dem Host.

**Befund während der Entwicklung:** Die Kette schlug beim Suchschritt fehl.
Ursache war nicht die Suche, sondern die **Kantenerkennung**: sie lieferte ein
achsenausgerichtetes Rechteck statt des geneigten Dokumentrandes, dadurch war
die Entzerrung falsch und der OCR-Text unbrauchbar. Die Erkennung arbeitet
jetzt auf der **Seitenkontur** (Boundary-Punkte je Zeile und Spalte, Ausgleich
mit dem robusten Theil–Sen-Schätzer, Schnittpunkt der vier Kantenlinien) und
liefert die Ecken innerhalb von 0,014 von der Referenz.

---

## 4. Was **nicht** verifiziert werden konnte

| Punkt | Status | Grund |
|---|---|---|
| Lauf auf einem echten Android-Gerät | **NOT RUN** | Kein Gerät angeschlossen. Der Emulator bootet ohne `/dev/kvm` nur per Software-Emulation. |
| Emulator-Lauf (OCR-Qualität, Kamera, Oberfläche) | **NOT RUN** | Wie oben. |
| Reaktionsfähigkeit der Oberfläche zur Laufzeit | **NOT RUN** | Erfordert ein Gerät. Die Korrektur ist implementiert und kompiliert. |
| Energie- und Thermikverhalten auf dem Gerät | **NOT RUN** | Erfordert ein Gerät. Die Entscheidungslogik ist auf dem Host vollständig geprüft. |
| Batterie- und Thermikverhalten | **NOT RUN** | Keine reale Hardware. |
| macOS-Build und -Integrationstests | **NOT RUN** | Kein macOS-Host. Die Swift-Bindungen werden generiert, aber nie kompiliert. |
| AddressSanitizer | **NOT RUN** | Nur mit dem Nightly-Toolchain (`-Zbuild-std`). Ersatz: valgrind memcheck. |
| CI-Lauf | **NOT RUN** | Es wurde kein Workflow ausgeführt. |

Diese Punkte bleiben **offen**. Sie sind nicht als bestanden dargestellt und
gehören zu den Voraussetzungen des nächsten Meilensteins.

---

## 5. Abnahmeentscheidung

M1 erfüllt seine **funktionalen** Anforderungen nachweislich: der vollständige
mobile Scan-Workflow läuft vom Kamerabild bis zum gespeicherten, durchsuchbaren
PDF, mit echter Engine, echter Erkennung und echter Suche.

M1 erfüllt seine **plattformabhängigen** Anforderungen **nicht** nachweislich:
der Lauf auf dem Zielgerät, die macOS-Integration und das Verhalten unter
Energie- und Thermikdruck sind nicht gemessen. Ohne diese Nachweise ist M1
**nicht** abgeschlossen.

**Entscheidung: M1 bleibt offen** bis
1. ein Gerät oder Emulator mit KVM für den On-Device-Lauf verfügbar ist,
2. ein macOS-Host für die Desktop-Spur vorhanden ist,
3. die Lizenzentscheidung E1–E5 getroffen ist (siehe `docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`).

Der Umfang von M1.6b ist damit sachlich abgeschlossen; was fehlt, ist
**Umgebung und Entscheidung**, nicht Implementierung.

---

## 6. Nachbereitung nach der Abnahme (2026-10-20)

Nach der Abnahme wurde der Stand erneut gegen die Anforderungen geprüft. Die
Prüfung hat vier Mängel gefunden, die im Abnahmerecord noch nicht abgebildet
waren. Sie sind inzwischen behoben und nachgewiesen:

| # | Mangel | Behebung | Nachweis |
|---|---|---|---|
| 26 | Ein Dokument aus einem Dokumentanbieter (`content://`-URI) wurde vom lokalen Broker als Dateipfad behandelt — es entstand eine Datei mit dem URI-Namen, und das Dokument wurde nicht gespeichert | Die Fähigkeit „kann zurückgeschrieben werden?" liegt beim Broker (`FileBroker::can_write_back`); ein solches Dokument wird mit `NotLocallyWritable` abgelehnt und über den Export (`export_session_bytes`) an einen Ort des Nutzers geschrieben | `core/vocument-core/tests/export_and_conflict_tests.rs`, `platform/vocument-ffi/tests/scan_workflow_e2e_tests.rs` |
| 27 | Ein Konflikt beim Speichern (die Datei wurde extern geändert) war in der Android-Oberfläche für den Nutzer unsichtbar | Die Oberfläche wertet das Ergebnis aus und bietet die vier Auflösungen: Kopie, Überschreiben, Neu laden, Abbrechen | `MainActivity.kt`, `resolve_save_conflict` |
| 28 | Die Dateien der Anwendung (Staging, Recovery-Container, Manifest, Journal) waren mit den Standardrechten des Systems angelegt | Staging `0600`, Container `0700`, Manifest und Journal `0600` | `core/vocument-core/tests/export_and_conflict_tests.rs` (Modus geprüft) |
| 29 | Die Kamera wurde beim Verlassen der Komposition nicht freigegeben; der Zustand der Oberfläche wurde aus dem Kameratread geschrieben | Die Kamera wird in `onDispose` freigegeben, die Ergebnisse werden auf dem Hauptthread veröffentlicht | `CameraPreviewView.kt`, `ScanCaptureScreen.kt` |

Zusätzlich ist der **Export in eine Datei des Nutzers** als eigener
Nachweis hinzugekommen: das exportierte Dokument ist ein vollständiges PDF,
es lässt sich mit der MuPDF-Engine öffnen, der OCR-Text ist lesbar, und es
enthält weder Pfade des Build-Rechners noch interne Kennungen.

Der Stand nach der Nachbereitung: **141 Tests, 21 Suites, 0 Fehler**; clippy 0,
fmt 0, `validate-repo.sh` 0, Android-Unit-Tests 8/8 grün, natives Gate über
das Paket grün. Die **plattformabhängigen** Nachweise aus Abschnitt 4 bleiben
unverändert **NOT RUN** — die Nachbereitung hat sie nicht ersetzt.

Die Nachbereitung und der Beginn von M2 sind in
`docs/reports/M1-NACHBEREITUNG-UND-M2-BEGINN.md` beschrieben, die Redaction
in `docs/acceptance/M2.1-ACCEPTANCE.md`.

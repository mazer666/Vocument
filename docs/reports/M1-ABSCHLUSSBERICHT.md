# Abschlussbericht — M1: Android Scan & OCR

**Datum:** 2026-10-20
**Branch:** `main`
**Akzeptanz:** `docs/acceptance/M1-ACCEPTANCE.md`
**Unterstufen:** M1.0 … M1.6

---

## A. Erreichter Stand

M1 liefert die mobile Kernfunktion: **Scannen → Bildkorrektur → OCR →
durchsuchbares, speicherbares PDF**. Die Kette ist jetzt als **ein**
End-to-End-Test an der UniFFI-Grenze nachgewiesen, mit echter Engine, echter
Kantenerkennung, echter OCR und echter Volltextsuche:

```
Kameraframe → Kanten → Entzerrung → Bereinigung → OCR → durchsuchbares PDF
             → Session → FTS5-Suche → Speichern → Wieder öffnen → Text lesbar
```

Laufzeit **4,2 s** auf dem Host.

### Was in dieser Ausbaustufe dazugekommen ist

| Bereich | Vorher | Nachher |
|---|---|---|
| Kantenerkennung | achsenausgerichtetes Rechteck, geneigte Seiten blieben falsch | Konturerkennung mit robustem Linienausgleich, Tilt wird erfasst |
| Throughput des Scans | PDF-Erzeugung lief **auf dem UI-Thread** | Aufnahme **und** PDF-Erzeugung laufen auf dem Executor |
| Speichern | es gab **keine** Speichern-Funktion in der Android-UI | „Save PDF" mit Kollisionsvermeidung, Pfad wird angezeigt |
| Hintergrundarbeit | keine Energieregelung | Kern entscheidet: Hintergrundarbeit ruht bei Hitze und leerem Akku, Parallelität sinkt |
| Abbruch / Fortsetzen | nur Abbruch | Pause und Fortsetzung, resumierbare Jobs laufen erneut |
| Throughput-Nachweis | keine vollständige Kette | End-to-End-Test über die UniFFI-Grenze |
| Speicher | ungemessen | Peak-Speicher und valgrind-Lauf des pathologischen Korpus |

---

## B. Die drei Befunde, die den Weg freigelegt haben

### B1 — Die Kantenerkennung konnte keine geneigte Seite finden

Die Erkennung suchte in der **Mittellinie** des Bildes nach den Kanten und
konnne deshalb nur ein **achsenausgerichtetes Rechteck** liefern. Die Seite im
Korpus ist aber ein gescherter Parallelogramm; die Entzerrung war damit
falsch, und der OCR-Text war unbrauchbar. Der End-to-End-Test schlug genau
daran fehl — an der **Suche**, nicht an der Suche selbst.

Die Erkennung arbeitet jetzt auf der **Seitenkontur**: für jede Zeile das
äußerste, für jede Spalte das oberste bzw. unterste Seitenzelle; daraus je
Kante eine Gerade, mit dem **Theil–Sen-Schätzer** (robust gegen bis zu 29 %
Ausreißer — die Ecken liegen oft im Schatten), und schließlich der Schnittpunkt
der vier Geraden.

Ergebnis: die erkannten Ecken liegen innerhalb von **0,014** von der Referenz
(Toleranz im Test: 0,05), der Neigungsgrad wird korrekt übernommen.

### B2 — Die PDF-Erzeugung blockierte die Oberfläche

Der Aufnahmepfad lief korrekt auf einem Executor. Der **Ingest-Pfad**
(PDF-Erzeugung mit OCR) lief dagegen direkt im Klick-Handler — auf dem
UI-Thread, sekundenlang. Das widersprach der Zusage „OCR blockiert den
UI-Thread nicht". Der Pfad läuft jetzt über denselben Executor; die
Oberfläche zeigt währenddessen „Preparing PDF…".

### B3 — Ein hängender Job konnte die Anwendung am Beenden hindern

`shutdown()` wartete unbegrenzt auf die Worker. Ein Job-Body, der sein
Cancellation-Token ignoriert, blockierte damit auch das `Drop` des Executors —
die Anwendung konnte in diesem Zustand **nicht beenden**. Der Shutdown ist
jetzt befristet (`shutdown_grace_ms`, 2 s); ein festhängender Worker wird
detachiert.

Der Befund kam nicht aus einem grünen Testlauf, sondern aus einem
**fehlgeschlagenen**: der Panic löste das `Drop` aus, das Shutdown wartete, der
Testprozess beendete sich nicht. Ein hängender Test ist ein Befund.

---

## C. Nachweisstand

### Bestätigt (PASS)

| Nachweis | Ergebnis |
|---|---|
| `cargo test --workspace` | **132 Tests, 0 Fehler** |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0** |
| `cargo fmt --all -- --check` | **0** |
| `scripts/validate-repo.sh` | **0** |
| End-to-End-Scan-Workflow | **PASS**, 4,2 s |
| Kantenerkennung auf geneigter Seite | **PASS**, Toleranz 0,05 |
| Energie- und Thermiklogik | **PASS**, 10 Tests |
| Pause und Fortsetzung | **PASS**, 3 Tests |
| Schwierige PDFs (8 Fixtures) | **PASS** |
| Peak-Speicher des Korpus | **5,9 MB → 8,3 MB** |
| valgrind memcheck | **0 Fehler, 0 Bytes verloren** |
| Android-Kotlin-Kompilierung | **PASS** (`compileDebugKotlin`) |

### Nicht verifiziert (NOT RUN)

| Nachweis | Grund |
|---|---|
| Lauf auf einem echten Gerät | Kein Gerät; der Emulator bootet ohne `/dev/kvm` nur per Software-Emulation |
| On-Device-OCR-Qualität | Wie oben |
| UI-Reaktionsfähigkeit zur Laufzeit | Erfordert ein Gerät |
| Energie- und Thermikverhalten auf dem Gerät | Erfordert ein Gerät |
| macOS-Build und -Integrationstests | Kein macOS-Host |
| AddressSanitizer | Nur mit dem Nightly-Toolchain (`-Zbuild-std`); Ersatz: valgrind |
| CI-Lauf | Kein Workflow ausgeführt |

Diese Punkte sind **offen** und werden nicht als bestanden geführt.

---

## D. Inkonsistenzen, die beseitigt wurden

| Inkonsistenz | Korrektur |
|---|---|
| Roadmap-Kopfzeile nannte M1.5 als „next", obwohl M1.5 implementiert war | Kopfzeile, M1.5-Status und M1.6-Definition aktualisiert |
| Zwei verschiedene „M1.6" (Plattformvalidierung und Android-Gate) | zu **M1.6a** (Android-Gate) und **M1.6b** (Plattformvalidierung) zusammengeführt |
| Die APK-Größe in der Roadmap war mit 23 MB angegeben | gemessene Werte (250 MB Debug, 95 MB Release) |
| `project.yaml` nannte ein offenes „release_package_inspection", obwohl das Paket geprüft war | Prüfstand und offene Punkte aktualisiert |
| Der Interop-Test suchte `mutool` nur an fest verdrahteten Pfaden | Suche über `MUPDF_DIR`, denselben Mechanismus wie der Build |
| `REQUIREMENTS.md` kannte die neuen Zusagen nicht | `SCAN-009`, `SCAN-010`, `JOB-011`, `UI-010`, `POWER-001` präzisiert |
| Die Toolchain-Pfade waren im Build fest verdrahtet | Tesseract-Cross-Build erkennt einen geänderten NDK und baut neu auf |

---

## E. Verbleibende Blocker

| Blocker | Art | Wer |
|---|---|---|
| Kein Android-Gerät und kein `/dev/kvm` | Umgebung | Bereitstellung eines Geräts oder eines KVM-fähigen Runners |
| Kein macOS-Host | Umgebung | macOS-Runner für die Desktop-Spur |
| Lizenzentscheidung E1–E5 offen | Entscheidung | Projektverantwortlicher |
| CI nicht ausgeführt | Infrastruktur | Lauffähiger Workflow-Runner |
| AddressSanitizer nur mit Nightly | Werkzeug | Nightly-Toolchain im Build-Image |

Keiner dieser Blocker ist eine Codefrage.

---

## F. Empfehlung für den nächsten Meilenstein

**M1.7 — Battie- und Thermikanpassung auf dem Gerät** (`BIGMOBILE-001`,
`POWER-001`), sobald ein Gerät verfügbar ist. Die Entscheidungslogik steht und
ist geprüft; offen ist ausschließlich die Messung auf dem Ziel.

Danach folgt **M2 — macOS Professional Review**: die macOS-Spur braucht
einen Host, und die Review-Funktionen (Kommentare, Highlights, Stempel,
Redaktion, Vergleich) sind der größere inhaltliche Schritt. Die
Windows-/Linux-Spur bleibt bis dahin geblockt, weil die Reihenfolge
macOS → Android → Windows → Linux bindend ist.

**Nicht** empfohlen: eine Engine-Migration (PDFium). Sie wäre eine Reaktion auf
eine noch nicht getroffene Lizenzentscheidung und würde die PDF-Mutation
(Rotation, Annotationen) schwächen, ohne ein Problem zu lösen, das nicht
bereits entschieden ist.

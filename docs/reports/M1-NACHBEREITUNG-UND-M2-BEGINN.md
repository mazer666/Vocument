# Abschlussbericht — Nachbereitung von M1 und Beginn von M2

**Datum:** 2026-10-20
**Branch:** `main`
**Akzeptanz:** `docs/acceptance/M1-ACCEPTANCE.md`, `docs/acceptance/M2.1-ACCEPTANCE.md`

---

## A. Worum es ging

Der Auftrag war, die verbleibenden produktiven Mängel aus M1 zu beseitigen,
den Nachweis für echte Dokumente zu vervollständigen, die Widersprüche
zwischen Code, Anforderungen und Abnahmedokumentation zu beseitigen — und
danach M2 zu beginnen, soweit die Umgebung echte Implementierung und echte
Verifikation zulässt.

## B. Die Mängel, die beseitigt wurden

### B1 — Ein Dokument aus einem Dokumentanbieter wurde zu einer Unsinnsdatei

**Befund.** Ein Dokument, das der Nutzer über die Android Storage Access
Framework öffnet, hat keine lokale Adresse, sondern einen `content://`-URI.
Der lokale Dateibroker behandelte den URI wie einen Pfad: aus
`content://com.android.providers/document/1` wurde eine Datei namens `1` im
Arbeitsverzeichnis. Das eigentliche Dokument wurde nicht gespeichert.

**Ursache.** Die Frage „kann dieses Dokument zurückgeschrieben werden?" war an
den *Ursprung* des Dokuments gekoppelt (`FileAccessOrigin`). Das ist die
falsche Frage: sie gehört an den *Broker*, denn nur der Broker weiß, ob er
eine Adresse auflösen kann. Die Kopplung an den Ursprung hätte außerdem den
macOS-Pfad gebrochen — dort ist ein Dokument über eine `file://`-Adresse
lokal, obwohl der Ursprung `MacOsScopedUrl` heißt.

**Behebung.** `FileBroker::can_write_back` beantwortet die Frage. Der lokale
Broker erkennt URIs mit einem Schema, das nicht `file` ist, und verweigert
das Rückschreiben mit dem typisierten Fehler `NotLocallyWritable`.
`stage_write` weist solche Ziele zusätzlich ab — letzte Verteidigungslinie,
damit nie eine Datei mit einem URI-Namen entsteht.

**Nutzerfolge.** Die Plattform-Shell schreibt das Dokument an einen Ort, der
dem Nutzer gehört: `document_core_export_session_bytes` übergibt die Bytes,
die Android-Shell schreibt sie über `ACTION_CREATE_DOCUMENT` in den
Dokumentanbieter. Der Nutzer wählt den Ort, das Dokument landet dort.

### B2 — Ein extern geändertes File wurde ohne Rückfrage überschrieben

**Befund.** Die Android-UI wertete das Ergebnis eines Speicherns nicht aus. Ein
Konflikt — die Datei wurde zwischenzeitlich von einem anderen Programm
geändert — blieb für den Nutzer unsichtbar.

**Behebung.** Die Oberfläche wertet das Ergebnis aus und zeigt bei einem
Konflikt die vier Auflösungen: als Kollisionskopie speichern, ausdrücklich
überschreiben, externe Version neu laden, abbrechen. Die Auflösung läuft
über `resolve_save_conflict` im Core.

### B3 — Dateien des Nutzers waren für andere lesbar

**Befund.** Die Staging-Datei eines Speicherns, der Recovery-Container mit
Manifest und Journal entstanden mit den Standardrechten des Systems. Sie
enthalten das Dokument des Nutzer bzw. dessen Pfad und Dateinamen.

**Behebung.** Staging-Datei `0600`, Recovery-Container `0700`, Manifest und
Journal `0600`. Der Modus wird von einem Test geprüft, nicht nur behauptet.

### B4 — Die Kamera blieb gebunden

**Befund.** Die Kamera wurde beim Verlassen der Komposition nicht freigegeben.
Sie blieb aktiv: die Datenschutzanzeige des Systems blieb an, die Kamera
verbrauchte Strom. Zusätzlich schrieb der Kameratread den Zustand der
Oberfläche — der Compose-State wurde aus einem Hintergrundthread verändert.

**Behebung.** Die Kamera wird in `onDispose` freigegeben. Die Kantenerkennung
läuft weiterhin im Kameratread (sie ist eine native Rechnung), die Ergebnisse
werden aber über den Hauptthread veröffentlicht.

## C. Der Nachweis für echte Dokumente

Zwei Tests an der UniFFI-Grenze mit echter Engine, echter Kantenerkennung,
echter OCR und echter Volltextsuche:

1. **Der Scan-Workflow** (unverändert grün): Kameraframe → Kanten →
   Entzerrung → Bereinigung → OCR → durchsuchbares PDF → Session → FTS5-Suche
   → Speichern → Wieder öffnen → Text lesbar.
2. **Der Export** (neu): dasselbe Dokument, exportiert in eine Datei, die dem
   Nutzer gehört. Die Datei ist ein vollständiges PDF, sie lässt sich mit der
   MuPDF-Engine öffnen, der OCR-Text ist lesbar, und sie enthält weder einen
   Pfad des Build-Rechners noch interne Kennungen.

Ergänzend im Core: ein Dokument aus einem Dokumentanbieter wird **nicht**
über den lokalen Broker gespeichert, der Export liefert die Bytes, und die
Dateirechte von Staging und Recovery werden geprüft.

**Teststand:** 141 Tests, 21 Suites, 0 Fehler. clippy 0, fmt 0,
`validate-repo.sh` 0, Debug-APK 108 MB gebaut, natives Gate über das Paket
grün (Closure, SONAME, ARM64, 16-KB-Seiten).

## D. M2 — erster Schnitt: sichere Redaction mit Verifikation

M2 verlangt neun Funktionen für den professionellen macOS-Arbeitsfluss. Acht
davon brauchen eine macOS-Oberfläche und einen macOS-Host. Die **Redaction**
ist der Teil, der sicherheitskritisch ist — und der sich auf dem Host mit
derselben MuPDF-Engine belegen lässt, die auch auf dem Gerät läuft.

**Das Problem.** Eine Redaction, die ein schwarzes Rechteck über den Text
legt, ist keine Redaction: der Text bleibt im Dateistrom, er bleibt
extrahierbar, er bleibt über die Suche auffindbar. Beim Scan kommt hinzu,
dass die Textebene unsichtbar ist (`3 Tr`) — sie muss mit entfernt werden.

**Die Umsetzung.** Die Engine entfernt den Inhalt der Region (Text, unsichtbarer
Text, Bilder, Linienkunst) mit `PDF_REDACT_TEXT_REMOVE`. Der Core prüft die
Entfernung dreifach: die Textextraktion, die Suche und die Bytes der Datei,
und meldet, was entfernt wurde. Die Redaction lässt sich nicht rückgängig
machen; der Undo wird ausdrücklich abgelehnt, und die Checkpoints der
Wiederherstellung werden mitgelöscht, weil sie die Bytes früherer Stände
enthalten.

**Der Fund, der die Arbeit wert gemacht hat.** Nach der ersten
funktionierenden Redaction war der Text nicht mehr extrahierbar — aber **noch
in den Bytes der Datei**. MuPDF schreibt unerreichbare Objekte mit in die
Datei, wenn die Garbage Collection nicht angefordert wird. Das alte
Content-Stream-Objekt stand weiterhin im File. Behoben mit
`opts.do_garbage = 1`; der Test prüft seitdem die Bytes, nicht nur die
Extraktion.

**Nachweis:** `docs/acceptance/M2.1-ACCEPTANCE.md`, Gate M2.1 in
`scripts/validate-repo.sh`, eigener Schritt in `scripts/local-ci.sh`.

## E. Was ausdrücklich NOT RUN bleibt

| Nachweis | Grund |
|---|---|
| Lauf auf einem Android-Gerät | kein Gerät, kein `/dev/kvm` |
| On-Device-OCR-Qualität, UI-Reaktionsfähigkeit, Thermik am Gerät | kein Gerät |
| macOS-Build, macOS-Oberfläche, macOS-Integrationstests | kein macOS-Host |
| macOS-Oberfläche der Redaction | kein macOS-Host |
| Interoperabilität der Redaction mit den Werkzeugen anderer Hersteller | steht aus, M2-Gate |
| CI-Workflow (GitHub Actions) | kein Runner-Lauf, keine Änderung an Workflow-Dateien |
| AddressSanitizer | nur mit Nightly (`-Zbuild-std`); Ersatz: valgrind memcheck, 0 Fehler / 0 Bytes verloren |
| Die übrigen acht M2-Funktionen | nicht begonnen |

## F. Widersprüche, die beseitigt wurden

- Die Meilensteinangabe war an mehreren Stellen dupliziert: `project/project.yaml`, `ROADMAP.md`, der Core-Konfiguration, dem Rust-Handshake-Test, dem Kotlin-Handshake-Test und dem Swift-Handshake-CLI. Sie stehen jetzt alle auf **M2** — abgeleitet aus `project/project.yaml` als einziger Quelle.
- `REQUIREMENTS.md` hatte keine Anforderung für den Export in einen Ort, der dem Nutzer gehört, und keine für den Datenschutz der Dateien. `SCAN-011`, `SCAN-012`, `PRIV-001`, `PRIV-002` und `REDACT-005` sind ergänzt.
- Die Roadmap führte M1.6 doppelt. M1.6a (Android-Gate) und M1.6b (Plattformvalidierung) sind als eine Etappe mit zwei Teilen geführt.

## G. Offene Entscheidungen

- **Lizenzentscheidung E1–E5** (Projektverantwortliche): offen. Ohne diese Entscheidung keine Veröffentlichung als „freigegeben".
- **Rotation des GitHub-PAT**: empfohlen. Der Token liegt verschlüsselt im Secret-Store und wurde nur über ein temporäres Askpass-Skript in `target/` verwendet, das sofort gelöscht wurde. Er ist nicht in der Historie nachweisbar.
- **M1.7** (Batterie- und Thermikverhalten am Gerät) und die restlichen M2-Funktionen: folgen, sobald die Umgebung sie zulässt.

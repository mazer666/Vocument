# Vocument für Einsteiger — Leitfaden

Dieses Dokument beantwortet die Fragen, die sich beim ersten Arbeiten am
Projekt stellen. Es ist bewusst kurz gehalten und verweist auf die Stellen, an
den die Antworten gepflegt werden.

**Wenn du etwas ändern willst, lies zuerst `WHERE_TO_CHANGE.md`.** Dort steht
für jede Art von Änderung der richtige Einstiegspunkt.

---

## 1. Wie ist das Projekt aufgebaut?

```
Swift-UI (macOS)  ─┐
                   ├─→ UniFFI (platform/vocument-ffi) → Rust-Core (core/vocument-core)
Kotlin-UI (Android)─┘                                          │
                                                              ├─→ engines/pdf-mupdf  (PDF-Engine)
                                                              ├─→ engines/ocr        (Tesseract)
                                                              └─→ engines/imaging    (Bildverarbeitung)
```

Vier Grundregeln, die nie gebrochen werden:

1. **Die Oberfläche spricht nie direkt mit MuPDF.** Jeder PDF-Aufruf läuft
   über den Rust-Core. Das prüft `scripts/validate-repo.sh` (Gate M0.1).
2. **Der Core ist plattformunabhängig.** Es gibt keine `#[cfg(target_os = …)]`
   im Core außerhalb der geplanten Plattform-Traits.
3. **Änderungen am Dokument laufen über die Transaktionen.** Kein direktes
   Schreiben in den PDF-Bytepuffer.
4. **Neue Abhängigkeiten brauchen eine Begründung.** Der Core kommt ohne
   externes Rust-Framework aus.

Die verbindliche Quelle für Meilensteine ist `project/project.yaml`.

---

## 2. Wo ändere ich die Anzahl paralleler Worker?

**`config/performance-budgets.toml`, Abschnitt `[jobs]`:**

```toml
[jobs]
max_concurrent_workers = 4      # Worker-Threads insgesamt
max_queued_jobs = 64            # Länge der Warteschlange
starvation_promotion_ms = 250   # Wartezeit bis zur Prioritätserhöhung
idle_shutdown_timeout_ms = 500  # Wartezeit eines leeren Workers
```

Die Datei wird in den Rust-Core eingebettet (`core/vocument-core/src/config.rs`,
`PerformanceBudgetsConfig::load_embedded`). Nach einer Änderung muss der Core
neu gebaut werden.

**Regel:** Kein Feature darf einen eigenen Thread-Pool aufbauen. Alles läuft
über `JobExecutor` (`core/vocument-core/src/jobs/executor.rs`).

---

## 3. Wo konfiguriere ich Rendering und OCR?

| Thema | Datei |
|---|---|
| Render-Budgets, Cache-Grenzen, Tile-Größen | `config/performance-budgets.toml` |
| Job-Budgets (Worker, Queue) | `config/performance-budgets.toml`, `[jobs]` |
| OCR-Sprachmodelle (offline) | `apps/android/app/src/main/assets/tessdata/` |
| Tesseract-Bibliotheken für Android | `scripts/prepare-android-tesseract.sh` |
| MuPDF-Bibliothek für Android | `scripts/prepare-android-mupdf.sh` |
| Engine-Adapter (PDF) | `engines/pdf-mupdf/src/lib.rs` |

---

## 4. Wie funktioniert die Android-Bibliotheksintegration?

Beim Android-Build werden **vier native Bibliotheken** in das Paket gelegt:

| Bibliothek | Herkunft | Zweck |
|---|---|---|
| `libvocument_ffi.so` | Rust, NDK-Cross-Build | Die FFI-Grenze |
| `libmupdf.so` | MuPDF 1.28.5, NDK-Cross-Build | Die PDF-Engine |
| `libtesseract.so` | Tesseract 5.3.0, NDK-Cross-Build | Die OCR-Engine |
| `libleptonica.so` | Leptonica 1.82.0, NDK-Cross-Build | Bildverarbeitung hinter Tesseract |

Zwei Regeln sind dabei **zwingend**, sonst startet die Anwendung nicht:

1. **Der SONAME muss unversioniert sein.** Der Android-Loader löst eine
   `DT_NEEDED`-Angabe nach dem **exakten Dateinamen** auf, und das Packaging
   erfasst nur Dateien mit der Endung `.so`. Ein `libmupdf.so.28.5` findet der
   Loader nie.
2. **Jedes `PT_LOAD`-Segment muss auf mindestens 16 KB ausgerichtet sein**
   (`-Wl,-z,max-page-size=16384`), sonst startet die Bibliothek auf Geräten mit
   16-KB-Seiten nicht.

Beides prüft `scripts/verify-android-native-libs.sh`. Dieser Lauf ist **bindend**.

---

## 5. Wie reproduziere ich einen Android-Build?

```bash
# 1. Werkzeuge (JDK 17, Android SDK 35, NDK r27c)
export ANDROID_HOME=$PWD/target/android-sdk
export ANDROID_NDK_HOME=/pfad/zum/ndk

# 2. Native Bibliotheken + Rust-Build
./scripts/build-android-rust.sh

# 3. Vollständiges Paket
cd apps/android && ./gradlew assembleDebug
```

Das Ergebnis liegt in `apps/android/app/build/outputs/apk/debug/`.

**Wichtig:** `scripts/build-android-rust.sh` baut immer neu, wenn sich
Rust-Quellen ändern, und legt die nativen Bibliotheken unter
`apps/android/app/src/main/jniLibs/arm64-v8a/` ab. Das Verzeichnis wird vor
dem Deployment **geleert**, damit keine alte Version im Paket landet.

Für einen Release-Build wird zusätzlich `BUILD_PROFILE=release` gesetzt; der
Build entfernt dann die Debug-Informationen (rund 90 MB kleiner).

---

## 6. Wie starte ich sämtliche Tests?

```bash
export MUPDF_DIR=/opt/mupdf-shared          # Pfad zur Host-MuPDF-Installation
export LD_LIBRARY_PATH=$MUPDF_DIR/lib
export TESSERACT_DIR=/usr
export TESSDATA_PREFIX=$PWD/apps/android/app/src/main/assets/tessdata

cargo test --workspace                      # Rust: alle Suites
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
./scripts/validate-repo.sh                  # Projekt-Governance
```

Zusätzlich, abhängig von der Umgebung:

| Skript | Prüft | Ohne das passiert |
|---|---|---|
| `scripts/verify-android-arm64-pdf.sh` | Die PDF-Engine führt unter ARM64 ein echtes PDF aus (Bildvergleich) | `NOT RUN` |
| `scripts/verify-android-native-libs.sh` | Auflösbarkeit und 16-KB-Ausrichtung der nativen Bibliotheken | `NOT RUN` |
| `scripts/verify-android-native-libs.sh --apk <apk>` | dasselbe für die Bibliotheken **im Paket** | `NOT RUN` |
| `scripts/verify-android-runtime.sh` | Die Anwendung auf einem Gerät/Emulator | `NOT RUN` (bindend für die Abnahme) |
| `scripts/verify-kotlin-handshake.sh` | Die Kotlin-Seite der FFI-Grenze | `NOT RUN` |

---

## 7. Wie erkenne ich, ob ein Test auf echter Hardware gelaufen ist?

**Die Beweisstufe steht im Ergebnis.** Es gibt genau vier Stufen:

| Stufe | Bedeutung | Beispiel |
|---|---|---|
| `HOST VERIFIED` | Auf dem Build-Rechner ausgeführt | Rust-Tests, der Host-MuPDF-Vergleich |
| `QEMU VERIFIED` | Der ARM64-Binärcode lief unter `qemu-aarch64` auf dem Host | `scripts/verify-android-arm64-pdf.sh` |
| `EMULATOR VERIFIED` | In einem Android-Emulator ausgeführt | `scripts/verify-android-runtime.sh` mit einem Emulator |
| `PHYSICAL DEVICE VERIFIED` | Auf einem echten Gerät ausgeführt | dasselbe Skript mit angeschlossenen Gerät |

**Regeln:**

* Ein QEMU-Ergebnis ist **kein** Geräteergebnis. Es steht immer als `QEMU VERIFIED`.
* Ein Emulator-Ergebnis ist **kein** Geräteergebnis. Es steht als `EMULATOR VERIFIED`.
* Wer ein Gerätetest behauptet, nennt das verwendete Gerätemodell und die
  `adb`-Ausgabe von `adb devices`.
* Jeder Nachweis nennt: **Plattform, Testumgebung, ausgeführtes Kommando,
  relevantes Testergebnis.**

Die Abnahme-Records unter `docs/acceptance/` verwenden diese Stufen und die
Statuswerte `PASS`, `FAIL`, `NOT RUN`, `BLOCKED`.

---

## 7a. Wo ändere ich die Kanten- und die Energiesteuerung?

| Thema | Datei |
|---|---|
| Kanten- und Konturerkennung | `engines/imaging/src/edges.rs` |
| Energie- und Thermikgrenzen | `core/vocument-core/src/jobs/power.rs` |
| Meldung des Gerätezustands aus der App | `apps/android/app/src/main/java/org/vocument/app/AndroidPowerReporter.kt` |
| Pause und Fortsetzung | `core/vocument-core/src/jobs/executor.rs` (`pause`, `resume`) |

**Die Energiesteuerung liegt bewusst im Core.** Die App meldet nur den
Zustand des Geräts (`document_core_report_power_state`), das Core entscheidet,
was laufen darf. So gilt auf macOS und auf Android dieselbe Regel, und die
Entscheidung ist ohne Gerät prüfbar.

## 7b. Wo ändere ich die Redaction?

| Thema | Datei |
|---|---|
| Redaction im Core mit Verifikation | `core/vocument-core/src/redaction.rs` |
| Entfernen des Inhalts in der MuPDF-Engine | `engines/pdf-mupdf/csrc/mupdf_mutate_bridge.c` (`vocument_mupdf_redact_page_regions`) |
| Redaction über die FFI-Grenze | `platform/vocument-ffi/src/redaction_dto.rs` |
| Transaktion und Undo | `core/vocument-core/src/transactions.rs` (`RedactPageRegions`, `Irreversible`) |

**Fünf Fallen, die schon einmal zugeschlagen haben:**

1. **Eine Redaction, die nur überdeckt, ist keine Redaction.** Der Text muss
   aus dem Dokument entfernt werden. MuPDF bietet dafür `PDF_REDACT_TEXT_REMOVE`;
   das entfernt auch die unsichtbare Textebene eines Scans.
2. **Der entfernte Text kann in der Datei bleiben.** MuPDF schreibt unerreichbare
   Objekte mit in die Datei, wenn die Garbage Collection nicht angefordert wird
   (`opts.do_garbage = 1`). Der Test prüft deshalb die Bytes der Datei, nicht
   nur die Textextraktion.
3. **Die Wiederherstellung enthält ältere Stände.** Die Checkpoints des
   Recovery-Stores werden mit der Redaction gelöscht, sonst wäre der Inhalt von
   dort wiederherstellbar.
4. **Ein Dokument trägt denselben Text zweimal.** Als Textebene und als Pixel
   im Bild. MuPDF bietet dafür `PDF_REDACT_IMAGE_PIXELS`: die betroffenen
   Pixel werden ersetzt, der Rest des Bildes bleibt. `PDF_REDACT_IMAGE_REMOVE`
   würde das ganze Bild löschen und damit die Seite zerstören.
5. **Ein schwarzer Kasten über der Region macht die Prüfung wertlos.** Er
   verdeckt genau das, was die Prüfung sehen muss. Die Redaction zeichnet
   deshalb nichts über die Region (`black_boxes = 0`); die Bildschirmdarstellung
   zeigt die Wahrheit.

**Die Koordinaten:** die Boxen liegen im Seitenraum der Engine — Millipunkte,
Ursprung oben links, y nach unten. Das ist derselbe Raum, in dem die Seite
gezeichnet wird. `PdfRectMilliPt` in `core/vocument-core/src/pdf_engine.rs`
beschreibt ihn.

Nachweise: `docs/acceptance/M2.1-ACCEPTANCE.md`.

## 7a. Wie prüfe ich die Links und Aktionen eines Dokuments?

Ein Link in einem Dokument kann eine Seite öffnen, eine Adresse aufrufen, ein
Programm starten, ein Formular absenden — oder ein Skript ausführen. Für die
professionelle Durchsicht zählt beides: wohin der Link führt und was er tut.

**Drei Stellen, in dieser Reihenfolge:**

1. **Die Engine meldet die Tatsachen** — `engines/pdf-mupdf/csrc/mupdf_inspect_bridge.c`
   liest die Aktionen des Dokuments und der Seiten aus.
2. **Der Core entscheidet das Risiko** — `core/vocument-core/src/document_inspection.rs`.
   Die Policy ist eine Produktentscheidung und gehört deshalb in den Core, nicht
   in die Engine und nicht in die Plattform.
3. **Die FFI-Grenze macht den Bericht verfügbar** —
   `platform/vocument-ffi/src/inspection_dto.rs`, `document_core_inspect_document`.

**Zwei Fallen, die schon einmal zugeschlagen haben:**

1. **MuPDF führt Link-Annotationen nicht in der Annotationsliste der Seite.**
   `pdf_sync_annots` überspringt `/Link` und `/Popup`; sie werden separat als
   klickbare Flächen geladen. Über `pdf_first_annot` findet man deshalb
   **nichts**. Die Brücke liest das `/Annots`-Array der Seite direkt.
2. **Nur so kennt man den Typ der Aktion.** Die geladenen Link-Flächen tragen
   nur das Ziel, nicht die Aktionsart. Genau die Information, um die es geht —
   „das ist ein Skript, kein Link“ — wäre sonst verloren.

Der Katalog ist nicht der Trailer, sondern das `/Root` des Trailers.

Nachweise: `docs/acceptance/M2.2-ACCEPTANCE.md`.

## 7b. Wie prüfe ich die Anlagen und die Signaturen eines Dokuments?

**Zwei Fallen, die schon einmal zugeschlagen haben:**

1. **Der MIME-Typ eines eingebetteten Anhangs muss maskiert werden.**
   `/Subtype /application/octet-stream` ist falsch — der Schrägstrich trennt
   im PDF den Namen ab, der Rest wird als neuer Schlüssel gelesen. MuPDF
   meldet `invalid key in dict`, und die Anlage fehlt **ohne** Fehler im
   Dokument. Richtig: `/Subtype /application#2Foctet-stream`.
2. **Die Änderungsprüfung braucht das Feld, nicht die Signatur.**
   `pdf_signature_incremental_change_since_signing` prüft selbst, ob das
   übergebene Objekt ein Signaturfeld ist, und wirft sonst
   `annotation is not a signature widget`. Der Byte-Bereich wird über den
   Feldwert gelesen.

**Wer entscheidet was:** die **Engine** liest den Byte-Bereich der Signatur
und vergleicht ihn mit der Größe der Datei — nur sie kann das. Der **Core**
entscheidet, was das bedeutet: ein Dokument, das nach der Signatur geändert
wurde, trägt eine Signatur, die nichts mehr abdeckt.

Nachweise: `docs/acceptance/M2.3-ACCEPTANCE.md`.

## 8. Wo ergänze ich neue Job-Arten?

Drei Stellen, in dieser Reihenfolge:

1. **Die Job-Art selbst** — `core/vocument-core/src/jobs/scheduler.rs`, Variante
   `JobKind`. Bestimme ihre Klasse (`Interactive`, `UserInitiated`, `Background`),
   damit die Priorität stimmt.
2. **Die echte Arbeit** — `platform/vocument-ffi/src/job_dto.rs`, in
   `build_job_task()`. Eine Job-Art **ohne** Core-Operation meldet `Failed`; sie
   darf nie Erfolg vortäuschen (`JOB-010`).
3. **Ein Test** — `core/vocument-core/tests/job_manager_e2e_tests.rs` (Weg durch
   die Schichten) und/oder `core/vocument-core/src/jobs/executor/executor_tests.rs`
   (Verhalten des Pools).

Die Prüfung in `scripts/validate-repo.sh` schlägt fehl, wenn eine Job-Art ohne
Operation eingereicht wird.

---

## 9. Wie funktioniert Cancellation?

1. Jeder Job bekommt beim Start einen **`CancellationToken`**.
2. `JobExecutor::cancel_and_signal(job_id)` setzt den Job auf `Cancelled` und
   **signalisiert** das Token. Das Token ist für einen Job erreichbar, der
   gerade läuft.
3. Der Job-Body prüft das Token an einem **sicheren Punkt** und gibt
   `JobOutcome::Cancelled` zurück.
4. **Zusätzlich** verwirft der Tracker ein Ergebnis, das nach dem Abbruch
   eintrifft: Ein abgebrochener Job meldet **nie** `Completed`.
5. Beim **Schließen des Dokuments** werden alle Jobs der Session abgebrochen.
6. Beim **Herunterfahren des Pools** werden die Token aller laufenden Jobs
   signalisiert, bevor die Worker-Threads verbunden werden.

**Wichtig:** Es wird **niemals** ein Thread zwangsweise beendet. Ein
Engine-Aufruf, der nicht unterbrochen werden kann (MuPDF, Tesseract), läuft zu
Ende — sein Ergebnis wird anschließend verworfen.

Der Shutdown ist trotzdem **befristet**: Nach `shutdown_grace_ms` (Standard
2 s, siehe `config/performance-budgets.toml`) wird ein Worker, der noch in
einem Job steckt, **detachiert** statt weiter auf ihn zu warten. Ohne diese
Grenze könnte ein hängender Engine-Aufruf das `Drop` des Executors blockieren
und damit die Anwendung am Beenden hindern.

**Der Job-Body muss das Token trotzdem beobachten** — die Frist ist der letzte
Ausweg, nicht der Plan.

---

## 10. Wo finde ich die noch offenen Risiken?

**`docs/project/RISKS.md`** — die einzige Quelle. Jede Zeile hat: ID, Beschreibung,
Wahrscheinlichkeit, Auswirkung, Gegenmaßnahme, verantwortliche Rolle, Status.

Die wichtigsten offenen Punkte:

| ID | Thema |
|---|---|
| `RISK-007` | Job-Pause/Resume und Job-Abhängigkeiten fehlen (Aufgabengraph nötig) |
| `RISK-009` | Die AGPL-Quelloffenlegung für die Android-Distribution ist nicht entschieden |
| `RISK-010` | Die Paketgröße (Maßnahmen unter `RISK-010` beschrieben) |

Die Lizenzfrage ist ausführlich in
`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` aufbereitet.
**Die Entscheidung trifft der Projektverantwortliche, nicht die Implementierung.**

---

## 11. Welche Änderungen darf ich vor einer Veröffentlichung nicht vergessen?

**Checkliste vor jedem Release:**

- [ ] `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check` sind grün
- [ ] `./scripts/validate-repo.sh` ist grün
- [ ] `./scripts/verify-android-native-libs.sh` ist grün (**bindend**: sonst startet die App nicht)
- [ ] `./scripts/verify-android-native-libs.sh --apk <apk>` ist grün (**bindend**: prüft die Bibliotheken *im Paket*, nicht nur im Build-Verzeichnis)
- [ ] `./scripts/verify-android-arm64-pdf.sh` ist grün (die Engine rendert ein echtes PDF)
- [ ] `./scripts/verify-android-runtime.sh` ist grün (**bindend**: echtes Gerät oder Emulator)
- [ ] Die erzeugte APK wurde **entpackt und geprüft** (`unzip -l app.apk | grep lib/`): alle vier nativen Bibliotheken sind vorhanden, keine ist doppelt, keine trägt einen alten Zeitstempel
- [ ] Die Release-Build-Größe ist gemessen und dokumentiert
- [ ] Die Lizenzentscheidung ist getroffen und die Quelloffenlegung ist umgesetzt (`RISK-009`)
- [ ] `project/project.yaml` und `ROADMAP.md` stimmen mit dem tatsächlichen Stand überein
- [ ] Für jedes Nachweis-Kriterium existiert ein Eintrag mit `PASS` / `FAIL` / `NOT RUN` / `BLOCKED`, **Plattform**, **Umgebung**, **Kommando** und **Ergebnis**

**Vorher nicht veröffentlichen:** Kein Paket ohne grünes
`verify-android-native-libs.sh`. Kein Paket ohne grünes
`verify-android-runtime.sh`. Keine Veröffentlichung als „freigegeben", solange
`RISK-009` offen ist.

---

## 12. Glossar

| Begriff | Bedeutung |
|---|---|
| **Actor** | Der serialisierte Zugriff auf ein Dokument. Alle Mutationen laufen über ihn. |
| **Display-List** | Die unveränderliche Liste der Zeichenbefehle einer Seite. Sie wird einmal berechnet und danach beliebig oft abgespielt. |
| **Tile** | Ein Teilstück einer Seite in einer definierten Auflösung. |
| **Transaction** | Eine atomare Änderung am Dokument mit Undo/Redo. |
| **UniFFI** | Die Schnittstelle zwischen Rust und Swift/Kotlin. |
| **SONAME** | Der Name, unter dem eine dynamische Bibliothek vom Loader gesucht wird. |
| **DT_NEEDED** | Die Einträge einer dynamischen Bibliothek, die andere Bibliotheken verlangen. |

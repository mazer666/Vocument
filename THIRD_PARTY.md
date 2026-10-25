# Third-Party Dependency Governance

Every third-party library or tool introduced into Vocument must pass architectural, licensing, security, and maintenance review (`AGENTS.md` ROLE-02, ROLE-16, ROLE-18).

---

## 1. Dependency Introduction Checklist

Before adding any Cargo crate, Swift package, Gradle dependency, or C/C++ library:

1. **Necessity:** Explain why existing standard library or current dependencies cannot solve the problem.
2. **License Compatibility:** Must be compatible with **AGPL-3.0-or-later** distribution across macOS and Android.
3. **Maintenance & Security Posture:** Active maintenance, clear security track record, minimal transitive dependency bloat.
4. **Binary & Platform Cost:** Acceptable size and compilation impact on `arm64` macOS and `arm64-v8a` Android.
5. **Architecture Boundary:** Engines and external providers must sit behind Vocument-owned traits/interfaces (`PdfEngine`, `OcrProvider`, `OfficeEngine`, etc.).

---

## 2. Planned Core Third-Party Components (Baseline 0.1)

| Component | Role / Adapter Boundary | License | Status |
|---|---|---|---|
| **MuPDF** | Primary PDF rendering, text & `/PageLabels` behind `PdfEngine` | AGPL-3.0-or-later | Active (`M0.1`–`M0.7`) |
| **Tesseract 5** | Local OCR engine behind `OcrProvider` (macOS host dylib + Android `arm64-v8a` embedded `libtesseract.so`) | Apache-2.0 | Active (`M1.3`) |
| **Leptonica** | Image processing backend for Tesseract C API | BSD-2-Clause | Active (`M1.3`) |
| **tessdata (`tesseract-lang`)** | Offline language models (`deu.traineddata`, `eng.traineddata`) embedded in Android assets and local path | Apache-2.0 | Active (`M1.3`) |
| **UniFFI** | Rust ↔ Swift/Kotlin bindings generator | MPL-2.0 | Active (`M0.0`–`M1.3`) |
| **SQLite (with FTS5) + `rusqlite 0.32`** | `AppCatalog` bookkeeping & progressive exact full-text search (`SessionSearchIndex`) | Public Domain / MIT | Active (`M0.3`) |
| **Wasmtime** | Sandboxed community plugin runtime | Apache-2.0 WITH LLVM-exception | Planned (Later) |

---

## 3. Android Embedded OCR Supply Chain

In conformance with `AGENTS.md` (ROLE-09 Security, ROLE-16 Supply Chain, ROLE-18 Licensing) and strict offline-first requirements (`NET-001`):

1. **Native Binaries (`arm64-v8a`):** The native libraries (`libtesseract.so`, `libleptonica.so`, `libjpeg.so`, `libpngx.so`, `libtiff.so`) are **cross-built from the source** by `scripts/prepare-android-tesseract.sh` with a pinned version, with the 16 KB page alignment, and they are linked directly by `libvocument_ffi.so` — without any stub or simulation bridge. The prebuilt AAR of `tesseract4android` is **not** used any more: it would have duplicated the native library (the AAR and the source build both provide `libtesseract.so`, which the packaging step rejects as a conflict), it was an unpinned external download that was not reachable from every build environment, and its libraries were not built for 16 KB memory pages.
2. **Offline Data Assets:** Standard traineddata files (`deu.traineddata`, `eng.traineddata`) are bundled directly into the APK assets (`assets/tessdata/`), ensuring zero network roundtrips.
3. **No Network Permissions:** The Android application manifest strictly declares only `android.permission.CAMERA` and forbids `android.permission.INTERNET`, ensuring that all OCR recognition takes place locally and offline.

---

## 4. The verified licence inventory

The table below is **generated** by `scripts/audit-licenses.sh` and is checked by the same script in the gate: every component is read from the metadata that the component itself carries — the manifest of the crate in the cargo registry, the POM of the library in the Gradle cache — and the licence of every component must be one that is known and that agrees with the AGPL-3.0 of Vocument. A component that cannot be read is a **finding**, not a footnote: the gate fails and a licence reviewer has to look at it.

The table is written with `scripts/audit-licenses.sh --write` and checked with `scripts/audit-licenses.sh` (the default).

<!-- BEGIN OF THE GENERATED LICENCE INVENTORY -->

### The Rust components (`Cargo.lock`)

| Component | Version | Licence as declared in the manifest of the crate |
|---|---|---|
| `ahash` | 0.8.12 | MIT OR Apache-2.0 |
| `anstyle` | 1.0.14 | MIT OR Apache-2.0 |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 |
| `askama` | 0.13.1 | MIT OR Apache-2.0 |
| `askama_derive` | 0.13.1 | MIT OR Apache-2.0 |
| `askama_parser` | 0.13.0 | MIT OR Apache-2.0 |
| `autocfg` | 1.5.1 | Apache-2.0 OR MIT |
| `basic-toml` | 0.1.10 | MIT OR Apache-2.0 |
| `bitflags` | 2.13.2 | MIT OR Apache-2.0 |
| `block-buffer` | 0.10.4 | MIT OR Apache-2.0 |
| `bytes` | 1.12.1 | MIT |
| `camino` | 1.2.6 | MIT OR Apache-2.0 |
| `cargo-platform` | 0.1.9 | MIT OR Apache-2.0 |
| `cargo_metadata` | 0.19.2 | MIT |
| `cc` | 1.8.0 | MIT OR Apache-2.0 |
| `cfg-if` | 1.0.5 | MIT OR Apache-2.0 |
| `clap` | 4.6.7 | MIT OR Apache-2.0 |
| `clap_builder` | 4.6.7 | MIT OR Apache-2.0 |
| `clap_derive` | 4.6.7 | MIT OR Apache-2.0 |
| `clap_lex` | 1.1.1 | MIT OR Apache-2.0 |
| `cpufeatures` | 0.2.17 | MIT OR Apache-2.0 |
| `crypto-common` | 0.1.7 | MIT OR Apache-2.0 |
| `digest` | 0.10.7 | MIT OR Apache-2.0 |
| `equivalent` | 1.0.3 | Apache-2.0 OR MIT |
| `errno` | 0.3.14 | MIT OR Apache-2.0 |
| `fallible-iterator` | 0.3.0 | MIT/Apache-2.0 |
| `fallible-streaming-iterator` | 0.1.9 | MIT/Apache-2.0 |
| `fastrand` | 2.5.0 | Apache-2.0 OR MIT |
| `find-msvc-tools` | 0.1.14 | MIT OR Apache-2.0 |
| `fs-err` | 2.11.0 | MIT/Apache-2.0 |
| `generic-array` | 0.14.7 | MIT |
| `getrandom` | 0.4.3 | MIT OR Apache-2.0 |
| `glob` | 0.3.4 | MIT OR Apache-2.0 |
| `goblin` | 0.8.2 | MIT |
| `hashbrown` | 0.14.5 | MIT OR Apache-2.0 |
| `hashbrown` | 0.17.1 | MIT OR Apache-2.0 |
| `hashlink` | 0.9.1 | MIT OR Apache-2.0 |
| `heck` | 0.5.0 | MIT OR Apache-2.0 |
| `indexmap` | 2.14.2 | Apache-2.0 OR MIT |
| `itoa` | 1.0.18 | MIT OR Apache-2.0 |
| `libc` | 0.2.191 | MIT OR Apache-2.0 |
| `libsqlite3-sys` | 0.30.1 | MIT |
| `linux-raw-sys` | 0.12.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `log` | 0.4.34 | MIT OR Apache-2.0 |
| `memchr` | 2.8.3 | Unlicense OR MIT |
| `minimal-lexical` | 0.2.1 | MIT/Apache-2.0 |
| `nom` | 7.1.3 | MIT |
| `once_cell` | 1.21.4 | MIT OR Apache-2.0 |
| `percent-encoding` | 2.3.2 | MIT OR Apache-2.0 |
| `pkg-config` | 0.3.34 | MIT OR Apache-2.0 |
| `plain` | 0.2.3 | MIT/Apache-2.0 |
| `proc-macro2` | 1.0.107 | MIT OR Apache-2.0 |
| `quote` | 1.0.47 | MIT OR Apache-2.0 |
| `r-efi` | 6.0.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| `rusqlite` | 0.32.1 | MIT |
| `rustc-hash` | 2.1.3 | Apache-2.0 OR MIT |
| `rustix` | 1.1.5 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| `scroll` | 0.12.0 | MIT |
| `scroll_derive` | 0.12.1 | MIT |
| `semver` | 1.0.28 | MIT OR Apache-2.0 |
| `serde` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_core` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_derive` | 1.0.229 | MIT OR Apache-2.0 |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 |
| `serde_spanned` | 0.6.9 | MIT OR Apache-2.0 |
| `sha2` | 0.10.9 | MIT OR Apache-2.0 |
| `shlex` | 2.0.1 | MIT OR Apache-2.0 |
| `siphasher` | 0.3.11 | MIT/Apache-2.0 |
| `smallvec` | 1.16.3 | MIT OR Apache-2.0 |
| `smawk` | 0.3.3 | MIT |
| `static_assertions` | 1.1.0 | MIT OR Apache-2.0 |
| `strsim` | 0.11.1 | MIT |
| `syn` | 2.0.119 | MIT OR Apache-2.0 |
| `syn` | 3.0.9 | MIT OR Apache-2.0 |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 |
| `textwrap` | 0.16.4 | MIT |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 |
| `thiserror-impl` | 2.0.21 | MIT OR Apache-2.0 |
| `toml` | 0.5.11 | MIT/Apache-2.0 |
| `toml` | 0.8.23 | MIT OR Apache-2.0 |
| `toml_datetime` | 0.6.11 | MIT OR Apache-2.0 |
| `toml_edit` | 0.22.27 | MIT OR Apache-2.0 |
| `toml_write` | 0.1.2 | MIT OR Apache-2.0 |
| `typenum` | 1.20.1 | MIT OR Apache-2.0 |
| `unicode-ident` | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| `unicode-width` | 0.2.2 | MIT OR Apache-2.0 |
| `uniffi` | 0.29.5 | MPL-2.0 |
| `uniffi_bindgen` | 0.29.5 | MPL-2.0 |
| `uniffi_core` | 0.29.5 | MPL-2.0 |
| `uniffi_internal_macros` | 0.29.5 | MPL-2.0 |
| `uniffi_macros` | 0.29.5 | MPL-2.0 |
| `uniffi_meta` | 0.29.5 | MPL-2.0 |
| `uniffi_pipeline` | 0.29.5 | MPL-2.0 |
| `uniffi_udl` | 0.29.5 | MPL-2.0 |
| `vcpkg` | 0.2.15 | MIT/Apache-2.0 |
| `version_check` | 0.9.5 | MIT/Apache-2.0 |
| `vocument-core` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-ffi` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-flow` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-imaging` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-ocr` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-pdf-mupdf` | 0.1.0 | AGPL-3.0-or-later |
| `vocument-pdoc` | 0.1.0 | AGPL-3.0-or-later |
| `weedle2` | 5.0.0 | MIT |
| `windows-link` | 0.2.1 | MIT OR Apache-2.0 |
| `windows-sys` | 0.61.2 | MIT OR Apache-2.0 |
| `winnow` | 0.7.15 | MIT |
| `zerocopy` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zerocopy-derive` | 0.8.63 | BSD-2-Clause OR Apache-2.0 OR MIT |
| `zmij` | 1.0.23 | MIT |

### The native components that are built into the product

| Component | Version | Licence | Where the licence was read |
|---|---|---|---|
| MuPDF | 1.28.5 | AGPL-3.0-or-later | the `COPYING` of MuPDF 1.28.5 (the file that ships with the source of MuPDF) |
| Tesseract | 5.x (see `scripts/prepare-android-tesseract.sh`) | Apache-2.0 | the `LICENSE` in the source of Tesseract 5.x (the pinned source of `scripts/prepare-android-tesseract.sh`) |
| Leptonica | 1.82.0 | BSD-2-Clause (the `leptonica-license.txt`) | the `leptonica-license.txt` in the source of Leptonica 1.82.0 (the pinned source of `scripts/prepare-android-tesseract.sh`) |
| tessdata (the language models) | 4.0.0 / 5.x | Apache-2.0 | the `LICENSE` in the `tesseract-lang` repository |

### The libraries that are built into the PDF engine

| Library | Licence | Where the licence was read |
|---|---|---|
| brotli | MIT | the `thirdparty/brotli/LICENSE` of the source of MuPDF |
| cmark-gfm | BSD-2-Clause | the `thirdparty/cmark-gfm/COPYING` of the source of MuPDF |
| extract | AGPL-3.0 | the `thirdparty/extract/COPYING` of the source of MuPDF |
| freetype | FTL-or-GPL-2.0 (the project uses the FTL) | the `thirdparty/freetype/LICENSE.TXT` of the source of MuPDF |
| gumbo-parser | Apache-2.0 | the `thirdparty/gumbo-parser/COPYING` of the source of MuPDF |
| harfbuzz | MIT | the `thirdparty/harfbuzz/COPYING` of the source of MuPDF |
| jbig2dec | AGPL-3.0 | the `thirdparty/jbig2dec/COPYING` of the source of MuPDF |
| lcms2 | MIT | the `thirdparty/lcms2/LICENSE` of the source of MuPDF |
| libjpeg | IJG | the `thirdparty/libjpeg/README` of the source of MuPDF |
| mujs | ISC | the `thirdparty/mujs/COPYING` of the source of MuPDF |
| openjpeg | BSD-2-Clause | the `thirdparty/openjpeg/LICENSE` of the source of MuPDF |
| zlib | Zlib | the `thirdparty/zlib/LICENSE` of the source of MuPDF |

### The fonts that are built into the PDF engine

| Family | Files | Licence | Where the licence was read |
|---|---|---|---|
| Noto | 160 | OFL-1.1 | `target/mupdf-android-build/mupdf-1.28.5-source/resources/fonts/noto/COPYING` |
| URW | 15 | OFL-1.1 | `target/mupdf-android-build/mupdf-1.28.5-source/resources/fonts/urw/OFL.txt` |
| SIL Charis | 4 | OFL-1.1 | `target/mupdf-android-build/mupdf-1.28.5-source/resources/fonts/sil/OFL.txt` |
| Source Han | 1 | OFL-1.1 | `target/mupdf-android-build/mupdf-1.28.5-source/resources/fonts/han/LICENSE.txt` |
| Droid | 2 | Apache-2.0 | `target/mupdf-android-build/mupdf-1.28.5-source/resources/fonts/droid/NOTICE` |

### The Android components (the licences that were read from the POMs of Maven Central on 2026-10-21)

| Component | Version | Licence as declared in the POM |
|---|---|---|
| `androidx.compose:compose-bom` | 2024.10.01 | Apache-2.0 |
| `androidx.activity:activity-compose` | 1.9.3 | Apache-2.0 |
| `androidx.compose.ui:ui` | via the BOM | Apache-2.0 |
| `androidx.compose.material3:material3` | via the BOM | Apache-2.0 |
| `androidx.camera:camera-core` | 1.4.1 | Apache-2.0 AND BSD-3-Clause |
| `androidx.camera:camera-camera2` | 1.4.1 | Apache-2.0 |
| `androidx.camera:camera-lifecycle` | 1.4.1 | Apache-2.0 |
| `androidx.camera:camera-view` | 1.4.1 | Apache-2.0 |
| `net.java.dev.jna:jna` | 5.15.0 | Apache-2.0 OR LGPL-2.1-or-later |
| `com.android.tools.build:gradle` (the build) | 8.7.2 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-gradle-plugin` (the build) | 2.0.21 | Apache-2.0 |
| `org.jetbrains.kotlin:kotlin-stdlib` | 2.0.21 | Apache-2.0 |

### The result

Every component carries a licence that was read in its own metadata and that agrees
with the AGPL-3.0 of Vocument. The audit that produces this table is
`scripts/audit-licenses.sh`.

<!-- END OF THE GENERATED LICENCE INVENTORY -->

---

## 5. The distribution form of the AGPL-3.0

Vocument is distributed under the AGPL-3.0 (see `LICENSE`, `LICENSES/AGPL-3.0-or-later.txt` and `REUSE.toml`). Two duties of section 6 of the AGPL-3.0 follow from it, and both are implemented:

1. **The complete corresponding source.** Every binary that Vocument hands out carries the revision of the source it was built from. The revision is in the handshake of the library (`document_core_build_info`), in the `BuildConfig` of the Android application and in the `Info.plist` of the macOS bundle. `docs/releasing/BUILD-FROM-SOURCE.md` describes how the source of a given binary is obtained.
2. **The notices.** Every binary carries the copyright notice, the notice of the licence, the notice of the AGPL and the source offer. The macOS bundle and the Android package carry them in their own notice files (`Vocument-LICENSE.txt`, `THIRD_PARTY.md`). The **texts** of the licences of everything that is inside the binary travel with it as well: `LICENSES/THIRD-PARTY-NOTICES.txt`, assembled by `scripts/generate-third-party-notices.sh` from the texts in `LICENSES/` and from the licence files of the components themselves.

The binaries are **not** published in the repository. The publication of a binary is a separate step that needs a release approval of its own (see `docs/releasing/RELEASE-PROCESS.md`).

---

## 6. The notes to the inventory

Four things cannot be read out of a manifest and are therefore named here.

### 6.1 The choice of the licence of FreeType

The `LICENSE.TXT` of FreeType offers **two** mutually exclusive licences: the FreeType License (FTL) and the GNU General Public License version 2. **Vocument uses the FTL.** The FTL is a permissive licence with an advertising clause; it is compatible with the AGPL-3.0, and the project does not need the GPLv2 route. The text of the FTL is in `LICENSES/FTL.txt` — it is the file that ships with FreeType itself.

The same holds for the fonts of the Droid family: they are under the Apache-2.0, the text is in `LICENSES/Apache-2.0.txt` and the notice of the family in the file that ships with the fonts.

### 6.2 The fonts of the PDF engine

MuPDF builds **182** font files into the PDF engine: 160 of the Noto family, 15 of the URW family, 4 of the SIL Charis family, 1 of the Source Han family and 2 of the Droid family. 180 of them are under the SIL Open Font License 1.1, the two of the Droid family under the Apache-2.0. The OFL requires that the licence travels with the fonts and that the fonts are not sold on their own — the text travels with the product in `LICENSES/THIRD-PARTY-NOTICES.txt`.

### 6.3 The OCR

The OCR is **not** taken from a prebuilt archive: Tesseract and Leptonica are cross-built from the source by `scripts/prepare-android-tesseract.sh`. The language models (`deu.traineddata`, `eng.traineddata`) come from the `tessdata_fast` repository and are under the Apache-2.0; their licence travels with them in `apps/android/app/src/main/assets/tessdata/LICENSE`. The provenance is proved by the checksum: the files in the repository are byte-identical with the files of `tessdata_fast`.

### 6.4 JNA and the choice of the build

The Android application uses JNA to reach the native library. The POM of JNA names **both** the Apache-2.0 and the LGPL-2.1-or-later. The project uses the **Apache-2.0** route, because the Apache-2.0 does not ask for the sources of the library to be published and does not impose any condition on the linking.

### 6.5 What is not in the product

The repository carries no binary of a third party: `git ls-files` finds no `.so`, no `.dylib`, no `.a` and no font file. The components are built from their source by the scripts in `scripts/`, and the artefacts are not part of the repository. The tools that are used to build — the Android NDK, the Android SDK, Gradle, the Kotlin compiler, the JDK, the Rust toolchain and UniFFI — are not part of the product either.

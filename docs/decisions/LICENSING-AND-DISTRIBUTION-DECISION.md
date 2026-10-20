# Decision Template: Licensing and Distribution of the PDF Engine

**Document type:** Non-binding technical decision template
**Status:** Draft for a decision by the project owner
**Date:** 2026-10-20
**Reference:** `RISK-009`, `RISK-010`, `ADR-0017`, `THIRD_PARTY.md`, `project/project.yaml`
**Created in the context of:** M1.6 — Platform integration and release readiness

> **This template is neither legal advice nor a licence decision.** It presents
> the technical and legal boundary conditions that have to be known for a
> decision, and names the open questions. The binding release remains a
> decision of the project owner.

---

## 1. The need for a decision

Vocument is a native application (Rust core, UniFFI, Swift UI on macOS,
Kotlin/UI on Android). The PDF engine MuPDF 1.28.5 is linked into the
application **dynamically**:

```text
libvocument_ffi.so  --DT_NEEDED-->  libmupdf.so.28.5
```

Vocument itself is under **AGPL-3.0** (`LICENSE`, `project/project.yaml`,
`THIRD_PARTY.md`). MuPDF is under **AGPL-3.0-or-later** *or* can be licensed
commercially separately.

That raises a question that has to be answered before any public distribution:
**Under which licence is Vocument published, and how is the coupling to MuPDF
secured?**

Trigger: `RISK-009` ("The AGPL source disclosure for the Android distribution
is not decided") and `RISK-010` (APK size through `libmupdf.so`).

### 1.1 Why the dynamic linkage does not settle the question

The dynamic linkage changes nothing about the effect of the copyleft:

* The AGPL regards "*modify* … *to link or combine with … other works*" (§10)
  as a possible trigger of the duty to convey — independently of whether the
  work is linked statically or dynamically.
* What counts is the **concrete use** ("*mere aggregation*" according to §9/§13
  only applies if the work stays "*at arm's length* … merely links … without
  *combining* … into a single larger work"). A PDF renderer whose display
  list, pixmaps and error handling are driven directly by the application code
  is a functional coupling, not a mere aggregation.
* The coupling is real in the project: `core/vocument-core/src/pdf_engine.rs`
  defines the trait `PdfEngineDocument` with `create_display_list`,
  `render_tile`, `rotate_pages`, `add_highlight_annotation`,
  `remove_highlight_annotation` and `extract_page_text` — functionality that
  lies in the heart of the engine and is not exchangeable by a thin wrapper
  layer.

**Consequence:** the variant "linked dynamically, therefore no copyleft
duty" is **not sound** and may not be used as an argument.

---

## 2. Boundary conditions common to all variants

These points are independent of the variant and already hold today:

| # | Boundary condition | Evidence / status |
|---|---|---|
| R1 | MuPDF 1.28.5 is built from the official source archive, SHA-256 `98a5c10c…b7e934`, independently confirmed by Homebrew and the FreeBSD ports. | `scripts/prepare-android-mupdf.sh` — **fulfilled** |
| R2 | The MuPDF licence (AGPL) and the copyleft duties are disclosed in the project. | `THIRD_PARTY.md` — **fulfilled** |
| R3 | The architecture of Vocument knows the engine only through the traits `PdfEngine` / `PdfEngineDocument` / `PdfEngineDisplayList`. | `core/vocument-core/src/pdf_engine.rs` — **fulfilled** |
| R4 | The application calls MuPDF **not** from Swift or Kotlin, but exclusively from the Rust core. | `scripts/validate-repo.sh` (gate M0.1) — **fulfilled** |
| R5 | A public distribution is **not** released until the decision is made. | Section 7 — **to be applied** |
| R6 | In AGPL operation the attribution duties of Artifex must be observed (producer line in generated PDFs, AGPL note in the PDF metadata). | Artifex licensing — **open, to be implemented in variant A** |
| R7 | The binary size (`libmupdf.so` ≈ 56 MB) is independent of the licence choice and reducible via build options (deactivate XPS/CBZ/HTML-EPUB, `strip`, ABI splits). | `RISK-010` — **implementable** |

---

## 3. Variant A — AGPL conforming open source publication

### 3.1 What would have to be disclosed

* **MuPDF itself:** unchanged, as long as no changes are made. The cross
  build configuration (NDK, `HAVE_LIBCRYPTO=no`, 16 KB page alignment) is a
  **build configuration**, not a change of the MuPDF source code.
* **The application code:** with a functional coupling, the conveyance of the
  **complete application source code** under the AGPL is the practical
  consequence. That concerns the Rust core, the UniFFI bindings, the Swift
  and Kotlin UI as well as the build scripts.
* **The "Corresponding Source" of the linked MuPDF version:** the AGPL
  demands in §6 the complete source code of the program version used. That is
  uncritical here, because the version used is already public — the reference
  to the official archive **including the checksum** is sufficient.

### 3.2 Duties of information

| Duty | Source | Implementation in the project |
|---|---|---|
| Source code of the work (AGPL) | AGPL §6 | Publication of the repository |
| Source code of the corresponding source of the library | AGPL §6 | Reference to the pinned MuPDF archive with SHA-256 |
| Licence text | AGPL §4 | `LICENSE` |
| Installation information | AGPL §6 | `README.md` + build instructions |
| Notice on modifications | AGPL §6 | Changelog |
| "Best Corresponding Source" on conveyance | AGPL §6 | To be documented in the release archive |
| Network use (§13) | AGPL §13 | Vocument is a desktop/mobile app without a network service; §13 is currently **not applicable**. If a server service is built later, the duty of disclosure applies to the entire service. |
| Producer line in generated PDFs | Artifex licence notice | Leave enabled in the MuPDF configuration |
| AGPL note in the PDF metadata | Artifex licence notice | Implementable via `fz_set_document_metadata` |

### 3.3 Effect on third party dependencies

| Dependency | Licence | Effect in variant A |
|---|---|---|
| MuPDF | AGPL-3.0 | Determines the licence of the overall work |
| Tesseract | Apache-2.0 | Permissive, compatible with AGPL-3.0 |
| Leptonica | BSD-2-Clause | Permissive, compatible |
| SQLite / FTS5 | Public domain | Compatible |
| Rust crates (serde, rusqlite, thiserror …) | MIT / Apache-2.0 | Compatible |
| UniFFI | MIT / Apache-2.0 | Compatible |
| Swift UI, Kotlin/Compose | Apache-2.0 | Compatible |

**Result:** **No additional licence incompatibility** arises. The overall work
stands consistently under AGPL-3.0.

### 3.4 Provision of the source code and the build instructions

For a sound conveyance the release artefact must contain:

1. The complete source archive of the application code (the same commit as
   the binary).
2. The build instructions that describe the exact way from the source code to
   the binary — including the NDK version, the MuPDF checksum and the Tesseract
   version pins.
3. The notice that the MuPDF version used is publicly available under the
   checksum named above.

Point 2 is the practically most demanding part: the build instructions have to
work on a machine without prior knowledge. **That is not yet ensured today**,
because the native Android build needs an NDK, an Android SDK and a cross
build environment for Tesseract (see `RISK-009` and section 6).

### 3.5 Assessment

| | |
|---|---|
| **Advantages** | No licence costs; no negotiations; consistent free software; the AGPL licence the project already has fits; the disclosure strengthens the credibility of the project. |
| **Disadvantages** | The entire application code must be published. That is a **strategic** decision: proprietary extensions built on the core are then impossible. An end user who expects a commercially licensed specialist application cannot continue to develop the software in closed source. |
| **Risk** | Medium to high: the decision is practically irreversible once the source code of the core is public. |
| **Effort** | Low (the licence is already AGPL) + medium (release process with source bundle, build instructions, metadata attribution). |

---

## 4. Variant B — Commercial MuPDF licence

### 4.1 Publicly documented facts

From the official sources of Artifex (`artifex.com/licensing`, `mupdf.com`,
`mupdf.readthedocs.io`):

* MuPDF is **dual licensed**: AGPL-3.0 **or** a commercial licence.
* Artifex names two commercial models:

| Model | Publicly documented properties |
|---|---|
| **OEM Distribution License** | Distribution without any requirement to disclose your own code; **annual quantity reporting**; support available; **cost per copy** with **quarterly minimum turnover** |
| **Subscription License** | Distribution without disclosure duty; **annual quantity reporting** (no reporting for internal use); support available; **cost per copy** with **quarterly minimum turnover** |

* The commercial licence applies according to Artifex to "*SaaS*, *OEM
  software* or *in-house*" and frees completely from the AGPL restrictions.
* Support and free upgrades are part of the commercial licence.

### 4.2 Information that is not publicly documented

> **The prices are not publicly documented.** Artifex names no price level,
> no minimum purchase in euros and no term. Every concrete number in this
> document would be invented. The statements "per-copy cost with a quarterly
> minimum fee" are the **only** publicly documented statement about costs; the
> height of the minimum fee and of the unit prices has to be asked for.

### 4.3 Contractual questions to clarify with Artifex

1. Which model (OEM vs. Subscription) fits the planned distribution?
2. What is the licence for **distribution to end users** (the app is shipped to
   users)?
3. Does the licence apply to the **version 1.28.5 used** and does it cover
   updates?
4. Which **quantity reporting** is required, and from which volume on?
5. Which **minimum purchase** and term apply?
6. Which rights exist on conveyance to **third parties** (distribution
   partners, OEM)?
7. Is there an **audit** or a proof obligation?
8. Which **support SLAs** and response times are promised?
9. Does the licence include an **indemnity**?
10. What happens in the case of an **AGPL conforming** publication under
    variant A — is there a "dual use" clause that leads back into the AGPL
    world?

### 4.4 Effect on the architecture

**None.** The dynamic linkage remains. The build, the cross build pipeline and
the trait boundary `PdfEngine` remain unchanged. That is a decisive advantage
over variant C.

### 4.5 Assessment

| | |
|---|---|
| **Advantages** | No disclosure of your own code. No strategically irreversible step. The source code of the core remains proprietary. Support and updates are part of the licence. The architecture remains unchanged. |
| **Disadvantages** | Running costs whose height only becomes known after an enquiry. Quantity reporting and minimum turnover bind even with low sales. A special request to one vendor ties the project to a single manufacturer. |
| **Risk** | Financial (height of the costs unknown) and strategic (dependence on a single vendor). |
| **Effort** | Negotiation, contract review, conversion of the distribution to a licensed MuPDF version, adjustment of the attribution duties. |

---

## 5. Variant C — Alternative rendering engine

### 5.1 Is a later exchange technically realistic?

**Yes, but not without effort.** The exchange boundary exists and is drawn
cleanly:

* `core/vocument-core/src/pdf_engine.rs` defines `PdfEngineDisplayList`,
  `PdfEngineDocument` and the error trait `PdfEngineError`.
* `engines/pdf-mupdf/` contains the only MuPDF implementation.
* `platform/vocument-ffi/src/lib.rs` registers it via
  `SessionRegistry::new(Box::new(MuPdfEngineAdapter::new()))` — a single point
  of exchange.

The effort does not lie in the trait, but in the **range of functions**: the
trait demands `rotate_pages`, `add_highlight_annotation`,
`remove_highlight_annotation` and `extract_page_text` — thus **PDF mutation**
and not only rendering. Most alternatives are pure renderers.

### 5.2 Candidates

| Engine | Licence | Rendering | Mutation | Text | Assessment |
|---|---|---|---|---|---|
| **PDFium** | BSD-3-Clause **and** Apache-2.0 (dual) | Yes, very good | No (only an annotations API) | Yes | **Best licence option.** Permissive, no copyleft duty. But: no mutation API that Vocument needs for `rotate_pages`; additional C++ dependency that has to be integrated into the Rust core (risk in terms of foreign code security). |
| **pdf.js** | Apache-2.0 | Yes (browser/canvas) | No | Yes | Renderer, no native core library. Unsuitable for a native app. |
| **PDFBox** | Apache-2.0 | Java | Yes | Yes | Java dependency in a Rust architecture; only sensible as a secondary path. |
| **Poppler** | GPL-2.0+ | Yes | Limited | Yes | **GPL**: the coupling forces the GPL onto the overall work — does not solve the problem. |
| **Ghostscript** | AGPL-3.0 / commercial | Yes | Limited | Yes | Like MuPDF: the AGPL problem remains, plus Ghostscript anachronisms of the ImageMagick type and a worse Unicode/text quality. |
| **Xpdf** | GPL / commercial | Yes | No | Yes | GPL problem + range of functions. |

### 5.3 Migration costs and risks

| Risk | Assessment |
|---|---|
| Loss of function with `rotate_pages` / annotation mutation | **High.** That is core functionality of M0.4/M0.7. PDFium offers no page rotation in modifiable form; it would have to be implemented in the application code at the level of PDF objects. |
| Text quality | Medium. The existing OCR and extraction tests (`M1.3`, `M1.4`) would have to be recalibrated. |
| Build complexity | Medium to high. PDFium is a large C++ project with its own build dependencies; the Android and macOS cross builds would have to be re-established. |
| Security profile | Medium. A foreign C++ project enlarges the attack surface that Rust does not cover. |
| Effort for the current state | **High** (weeks). |

### 5.4 Assessment

| | |
|---|---|
| **Advantages** | PDFium would solve the copyleft problem **technically** (permissive licence). |
| **Disadvantages** | It solves the problem only by loss of core functionality or by high new effort. The strategic gain is limited, because the project is already AGPL licensed. |
| **Risk** | High: loss of function in the mutation, regression in text quality and the OCR gate, complete rebuild of the cross build pipeline. |
| **Recommendation** | **Not now.** Variant C is the way out if a proprietary distribution strategy later proves mandatory and variant B is not available. The trait cut-off is deliberately drawn in such a way that this way stays open. |

---

## 6. Comparison

| Criterion | A: AGPL conforming | B: Commercial | C: PDFium (later) |
|---|---|---|---|
| Own source code must be open | **Yes** | No | No |
| Running costs | None | **Yes, height unknown** | None |
| Strategic reversibility | **Poor** (code once public) | Good | Good |
| Change of the architecture | None | None | **Large** |
| Range of functions preserved | Yes | Yes | **Restrictions in the mutation** |
| Need for negotiations | None | **Yes, with the vendor** | None |
| Security profile | Unchanged | Unchanged | **New C++ dependency** |
| Effort | Low–medium | Medium | High |
| Residual risk | Medium | Financial | High |

---

## 7. Recommendation and open questions of the decision

### 7.1 Recommendation

**Variant A (AGPL conforming open source publication) is technically
recommended**, under two explicit conditions:

1. **Only if** the disclosure of the application source code is strategically
   desired. That is a business decision, not a technical one. The project
   owner decides it.
2. **Only if** the release chain is in place before the publication (see
   7.3).

**Variant B** is the recommended alternative if the application code is to
remain proprietary. It is to be initiated **before** a public distribution,
because the enquiry takes time and the answer changes the basis of the
decision.

**Variant C is not recommended now.** The loss of function in the PDF mutation
is not compatible with the current state of the product (M0.4/M0.7).

This recommendation is **deliberately marked as provisional**: it is based on
the publicly documented facts about the three variants, not on a cost estimate,
which does not exist at the moment.

### 7.2 Open questions for the project owner

| # | Question | Why it blocks |
|---|---|---|
| E1 | Should the source code of the application core become public? | Determines variant A vs. B vs. C. |
| E2 | Is a proprietary distribution the goal (specialist application, closed source)? | Determines variant A vs. B. |
| E3 | Which order of magnitude is realistic? | Determines the commercial costs in variant B. |
| E4 | Should a server service arise? | In variant A AGPL §13 applies and obliges to disclose the service. |
| E5 | Who is liable for the legal review? | This template is not advice. |

### 7.3 What must not happen until the release

* **No public distribution** of APK, AAB, DMG or installer that contains
  MuPDF may be treated as "released".
* **No inclusion in app stores** without the respectively applicable variant.
* **No proprietary extensions** may be built while E1 is open — otherwise a
  mixture arises that cannot later be resolved cleanly.
* **No engine migration** (variant C) may be started while E1 and E2 are open.
* The **producer line** and the **AGPL information in the PDF metadata** are to
  be implemented independently of the variant decision, as soon as AGPL
  operation is intended (R6).

### 7.4 Independently of the decision, immediately implementable

These points improve the state in every variant:

1. **Prepare the release process for variant A** (source bundle, checksums,
   build instructions including the NDK/SDK/MuPDF pins).
2. **Add the attribution in the PDF metadata** (producer line, AGPL note).
3. **Reduce `RISK-010` (size)**: deactivate the MuPDF build options for unused
   formats (XPS, CBZ, HTML/EPUB), use `strip`, offer ABI splits. That is
   **reproducible** and provable without a functional regression — the pixel
   comparison against the reference rendering is the proof of it.
4. **Obtain an answer from Artifex** in order to underpin E3 with sound figures
   (that is also a sensible basis even for a later decision in favour of
   variant A).

---

## 8. Sources

| # | Source | URL | Used for |
|---|---|---|---|
| Q1 | Artifex — Licensing | https://artifex.com/licensing | Models, duties, attribution notices, "Can't comply with AGPLv3?" |
| Q2 | Artifex — Products | https://artifex.com/products | MuPDF (core) as a product, commercial licensing |
| Q3 | Artifex Software — GitHub | https://github.com/ArtifexSoftware | Dual licensing, purpose of the commercial licence |
| Q4 | MuPDF — Licence documentation | https://mupdf.readthedocs.io/en/1.28.3/license.html | "GNU AGPL", "commercial license", no guarantee/support promise |
| Q5 | Artifex — MuPDF downloads (release 1.28.5) | https://github.com/ArtifexSoftware/mupdf-downloads/commit/c3c899f29d0640b6b23f4d5d45cf8c99074cb295 | Versioning of the version used |
| Q6 | GNU AGPL v3 (Canonical) | https://www.gnu.org/licenses/agpl-3.0.html | §6 (conveyance), §9/§10 (coupling), §13 (network) |
| Q7 | PDFium — LICENSE | https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE | BSD-3-Clause + Apache-2.0, permissive |
| Q8 | pdf.js — comparison tool with licence overview | https://github.com/mozilla/pdf.js.comparator/blob/main/README.md | Licence comparison of the alternative engines (Poppler GPL, Ghostscript AGPL, Xpdf GPL, PDFBox/pdf.js/PDFium Apache) |
| Q9 | PDF Association — Survey of Open-Source PDF Solutions | https://pdfa.org/wp-content/uploads/2021/06/Survey-of-OpenSource-Solutions.pdf | Range of functions of the alternatives (overprint, AcroForms, annotations, text) |
| Q10 | Homebrew — `mupdf-tools` formula | https://github.com/Homebrew/homebrew-core/blob/cbd0c8ddb11909fb209f06e08677973c0f211848/Formula/m/mupdf-tools.rb | Independent confirmation of the MuPDF 1.28.5 SHA-256 |
| Q11 | FreeBSD Ports — `graphics/mupdf` | https://github.com/FreeBSD/freebsd-ports/commit/602c6870f02e89e175ee855bff0e26c1a42b2970 | Independent confirmation of the MuPDF 1.28.5 SHA-256 and file size |

**Internal evidence of the project:** `THIRD_PARTY.md`, `LICENSE`,
`project/project.yaml`, `docs/project/RISKS.md` (`RISK-009`, `RISK-010`),
`docs/adr/ADR-0017-real-android-mupdf-cross-build.md`,
`core/vocument-core/src/pdf_engine.rs`, `scripts/validate-repo.sh`.

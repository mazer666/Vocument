# Vocument Architecture

**Document version:** 0.1  
**Status:** Accepted architecture baseline  
**Change rule:** Material changes require an ADR and, if requirements are affected, a Change Request.

---

## 1. Architectural goals

The architecture must support all of these simultaneously:

1. native macOS and Android user experience
2. shared, testable business/document logic
3. safe handling of untrusted documents
4. transactional editing, undo/redo and crash recovery
5. large-document performance
6. local/offline processing
7. strong interoperability
8. later Windows/Linux/iPad expansion
9. provider replaceability
10. no hidden lock-in to one third-party engine

---

## 2. High-level architecture

```text
Native macOS UI                           Native Android UI
SwiftUI + AppKit                          Kotlin + Jetpack Compose
       │                                          │
       └─────────────── UniFFI ───────────────────┘
                          │
                    DocumentCore API
                          │
                    Shared Rust Core
                          │
      ┌───────────────────┼─────────────────────────┐
      │                   │                         │
DocumentSession       JobManager              Transactions
      │                   │                         │
      ├─ Search           ├─ Render                 ├─ Undo/Redo
      ├─ Security         ├─ OCR                    ├─ Journal
      ├─ References       ├─ Export                 └─ Checkpoints
      ├─ Revision         └─ Compare
      └─ State
                          │
          ┌───────────────┼────────────────────┐
          │               │                    │
      PdfEngine       OcrProvider          OfficeEngine
       MuPDF           Tesseract              adapter
          │                                    │
  existing/imported PDF                   FlowDocument
                                               │
                                        Rust Layout Engine
                                               │
                               ┌───────────────┴──────────────┐
                               │                              │
                         Native Canvas                  PdfExportEngine
```

Cross-cutting services:

```text
FileBroker
NetworkBroker
RecoveryStore
AppCatalog
FontService
ImagingEngine
SignatureService
PluginHost
AccessibilityService
SettingsService
DiagnosticsService
EngineTransport
```

---

## 3. Platform baseline

### macOS

- macOS 15+
- Apple Silicon / arm64 only
- SwiftUI for primary UI
- AppKit where platform-native lifecycle/integration is stronger
- `NSDocument`/`NSDocumentController` as a thin lifecycle wrapper
- Vocument's Rust `DocumentSession` remains authoritative
- macOS App Sandbox from the start
- direct distribution remains supported
- public stable release target: Developer ID signing + Hardened Runtime + notarization

### Android

- Android 13+
- primary release ABI: `arm64-v8a`
- Kotlin + Jetpack Compose
- Storage Access Framework for user files
- OS-compliant background execution adapters
- app/core session lifetime is independent of Activity lifecycle

---

## 4. Rust core boundary

The Rust core owns document/business state, not native UI state.

### Rust owns

- `DocumentSession`
- document revision and dirty state
- transactions and undo/redo
- recovery journal/checkpoints
- search state and indexes
- semantic document references
- jobs and priorities
- document security state
- PDF/FlowDocument semantics
- file identity metadata

### Native UI owns

- window position/size
- scroll position and zoom per view
- temporary gesture state
- transient native controls
- platform menu/window composition
- IME/dictation/native text input interaction

---

## 5. UniFFI policy

UniFFI is a transport layer, not the architecture.

Rules:

- expose a small stable `DocumentCore` API
- no raw MuPDF pointers
- no raw Rust internal references
- use opaque IDs/handles
- use small immutable DTOs/snapshots
- generate bindings; never hand-edit generated bindings
- use explicit job cancellation handles rather than assuming cross-FFI future cancellation
- design API so the implementation can later move behind IPC without rewriting UI callers

---

## 6. DocumentSession

Each opened document receives an application-owned `DocumentSession`.

It contains or references:

- document model
- engine handle
- revision token
- dirty/saved state
- transaction manager
- undo/redo history
- jobs
- search indexes
- references and selections that are document-semantic
- security/signature state
- recovery association

MuPDF owns PDF internals. `DocumentSession` owns application semantics.

A full custom PDF object model is explicitly not required.

---

## 7. DocumentActor and concurrency

Each PDF session has a serialized `DocumentActor` for operations that mutate or depend on the mutable MuPDF document state.

Parallelism is achieved through safe derived work:

```text
DocumentActor
    │
    ├─ mutation / parsing / display-list creation (serialized)
    │
    └─ immutable/derived work
          ├─ render workers
          ├─ thumbnail workers
          ├─ OCR workers
          └─ indexing workers
```

No feature may create its own uncontrolled thread pool.

---

## 8. JobManager

All long-running work uses a central `JobManager`.

Minimum job metadata:

- ID
- type
- priority
- progress
- state
- cancellation
- pause/resume where meaningful
- resource budget
- dependencies
- persistence/resumability status
- associated document revision

Priority example:

1. visible page render
2. active selection/hit-test dependencies
3. user-triggered search
4. nearby page prefetch
5. thumbnails
6. background OCR/index
7. batch optimisation/compression

The design may later evolve into a task graph/DAG scheduler.

---

## 9. Rendering

### PDF rendering

- MuPDF is the production renderer
- adaptive tile rendering
- visible tiles highest priority
- nearby tile prefetch
- zoom-dependent resolution
- LRU eviction
- separate thumbnail cache
- huge scans are never fully held as high-resolution page bitmaps in RAM
- split views may share render cache
- native memory-pressure signals adjust budgets

### FlowDocument rendering

- Rust computes deterministic layout and geometry
- native platform canvas draws that geometry
- pagination is shared across platforms
- anti-aliasing/subpixel appearance may be platform-specific
- native UI may not invent its own line/page break logic

---

## 10. Cache policy

Cache is never document truth.

Classes:

### Disposable Cache
May be deleted at any time.

Examples:
- tiles
- thumbnails
- search derived data
- semantic vectors

### Persistent Recovery
Persists according to retention policy.

Examples:
- transaction journal
- checkpoints
- engine journal

### User Artifacts
Files explicitly created/saved by the user.

Examples:
- PDF
- `.pdoc`
- exported DOCX/ODT/images/reports

---

## 11. Transactions, undo and recovery

Vocument uses semantic commands plus engine journaling plus periodic checkpoints.

Examples:

- `RotatePages`
- `DeletePages`
- `InsertText`
- `ApplyOCR`
- `AddAnnotation`
- `ApplyRedaction`
- `MovePages`

A transaction contains:

- stable transaction ID
- user-facing description
- timestamp
- affected pages/objects
- before/after state references
- undo/redo information
- engine journal information where relevant
- integrity/revision markers

Full event sourcing is not required.

---

## 12. Recovery persistence

Each edited document receives its own versioned recovery container.

Example:

```text
RecoverySession/
├─ manifest.json
├─ transactions/
├─ checkpoints/
├─ engine-journal/
├─ session-state/
└─ integrity/
```

Rules:

- OS-protected app support directory
- opaque internal IDs instead of plaintext document names
- schema version in manifest
- checksums for critical components
- atomic updates
- independently prunable/repairable
- no global monolithic recovery database

A small SQLite database is allowed inside a recovery container if needed later.

---

## 13. AppCatalog

A small Rust-owned SQLite catalog stores only app-level bookkeeping, e.g.:

- opaque document IDs
- recovery-container references
- resumable-job metadata
- schema versions/migrations
- last known file identity metadata

It must not contain document content.

---

## 14. File identity

Never rely on path alone.

Use a combination of:

- OS file identity/inode where available
- size
- modification time
- SHA-256 or fast fingerprint
- internal document fingerprint
- previous/current path metadata

The app must distinguish:

- same file moved
- different file replacing same path
- probable copy of same source

Recovery is never automatically applied when identity is uncertain.

No proprietary tracking ID is silently written into ordinary PDFs.

---

## 15. FileBroker

Native platform code owns OS file permissions and exposes a controlled `FileBroker` to the Rust core.

### macOS

- App Sandbox
- user-selected files/folders
- security-scoped access where required
- native file coordination/watching

### Android

- Storage Access Framework
- persisted URI permissions where possible

Raw unrestricted paths are not the public cross-platform file API.

---

## 16. Safe save

Default save is transactional:

1. prepare new state
2. write safely to temporary/staged output
3. validate output
4. flush/sync as appropriate
5. atomically replace only when safe
6. retain recovery information as policy requires

Never silently overwrite a conflicting external change.

Conflict options include:

- compare
- open external version
- save local version under a new collision-free name
- cancel

---

## 17. Search architecture

Three independent layers:

### Exact/classic
Initial backend: SQLite FTS5.

### Fuzzy/OCR-tolerant
Separate logic for:

- OCR errors
- hyphenation
- line-break normalization
- variants
- typo tolerance

### Semantic
Separate optional vector index through `EmbeddingProvider`.

Rules:

- classic search works immediately
- index progressively
- visible/near pages first
- every result maps to source page/text location and revision
- edits invalidate only affected segments
- semantic index is separately removable
- indexes are disposable derived data

---

## 18. PDF engine

`PdfEngine` is an app-defined interface.

Initial production implementation: **MuPDF**.

MuPDF is not referenced directly by UI code.

Potential future supporting tools:

- PDFium as rendering/QA reference
- qpdf as targeted structural specialist if a concrete gap exists

Do not run multiple competing production PDF engines without a specific reason.

---

## 19. Existing PDF text editing

Use hybrid editing.

### Quick Edit — default

Minimal in-place changes, e.g. replacing `30 days` with `60 days`.

Goal: smallest possible technical change.

### Block/Reflow Edit — explicit

Reconstruct only selected block:

`PDF text extraction → editable block → shared text layout → replacement PDF content`

Show impact before applying if:

- line breaks change
- block height changes
- neighbouring objects may move/be affected
- signature state changes

### Expert edit

Object/glyph/content-stream tools remain separate.

All applied changes use a `PdfEditPlan` and are validated by reopen/render/search/extraction checks.

---

## 20. OCR

`OcrProvider` interface from day one.

Initial provider: **Tesseract**.

Built-in languages:

- German
- English

Additional language packs are explicit downloadable/removable packages.

Provider replacement/benchmarking must not require application redesign.

---

## 21. Imaging and scanning

Hybrid architecture:

### Native capture

- camera APIs
- live focus/exposure
- fast edge assistance

### Shared Rust `ImagingEngine`

Authoritative post-capture pipeline:

- perspective correction
- crop
- rotation/deskew
- contrast/colour normalization
- denoise
- background cleanup
- OCR preparation
- deterministic batch processing

### `AdvancedVisionProvider`

Optional later provider for CV functionality where it demonstrably improves results.
OpenCV is not a mandatory core dependency.

---

## 22. FlowDocument

FlowDocument is separate from PDF.

It models reflowable authored content:

- sections
- paragraphs
- headings
- lists
- tables
- images
- footnotes/endnotes
- styles
- references
- tracked changes
- metadata

PDF is not treated as a FlowDocument unless an explicit reconstruction/conversion is requested.

---

## 23. FlowDocument layout

Shared deterministic Rust layout engine.

Native Swift/Kotlin handles:

- IME
- dictation
- keyboard/input method integration
- native spellcheck interaction
- accessibility interaction

Rust handles:

- text shaping abstraction
- line breaking
- paragraphs
- sections
- margins
- pagination
- tables across pages
- footnotes
- header/footer
- floating objects
- numbering
- references

Initial text-layout bake-off candidate: Parley. Alternatives include COSMIC Text or a lower-level HarfRust/ICU4X path.

---

## 24. Fonts

Shared `FontService`:

```text
FontService
├─ BundledOpenFonts
├─ PlatformFonts
├─ EmbeddedDocumentFonts
├─ FontFallbackResolver
└─ FontEmbeddingPolicy
```

Rules:

- ship a small open-font baseline
- allow system/corporate fonts
- embed/subset only when licence/embedding flags allow
- preserve references/fallback metrics otherwise
- never silently reflow when a font is missing
- expose replacement/embedding state in Font Inspector

---

## 25. .pdoc

Public editable Vocument format.

Format: versioned ZIP package using JSON plus normal assets.

Example:

```text
Document.pdoc
├─ manifest.json
├─ document.json
├─ styles.json
├─ metadata.json
├─ assets/
└─ optional/
```

Requirements:

- open published specification
- UTF-8
- stable object IDs
- unknown-field preservation
- schema migrations
- ZIP64
- deterministic serialization where practical
- package safety limits
- reference fixtures/tests
- optional SHA-256 integrity data

Undo/recovery is not embedded in `.pdoc`; it remains in Vocument recovery storage.

---

## 26. OfficeEngine

DOCX/ODT use an external Rust-based engine behind Vocument's `OfficeEngine` abstraction.

Vocument retains its own FlowDocument semantics.

Before selecting the concrete first engine, run a fidelity bake-off over a fixed corpus covering:

- unchanged open/save roundtrip
- styles
- numbering
- tables
- headers/footers
- images
- comments
- tracked changes
- footnotes
- unknown package parts

OpenDoc is a strong provisional candidate but is not adopted blindly.

Use a Preservation Capsule so unsupported package content is not silently discarded.

---

## 27. PDF export from FlowDocument

Authored documents use Vocument's own `PdfExportEngine`.

Pipeline:

`FlowDocument → shared layout → PdfExportEngine → low-level PDF writer`

The export engine explicitly owns:

- pages/content
- font embedding/subsetting
- images
- links
- bookmarks
- page labels
- metadata
- tagged structure tree
- PDF/A/PDF/UA-relevant structures

Validation pipeline includes independent standards validation, MuPDF reopen and reference rendering/visual comparison.

---

## 28. Signature architecture

Separate `SignatureService`.

Responsibilities stay separated:

- PDF revision and ByteRange preparation
- digest generation
- certificate validation
- signing provider
- timestamp/OCSP/CRL policy
- final incremental revision

Provider examples:

- OS key store
- hardware token / PKCS#11
- A-Trust/QES

---

## 29. Redaction architecture

Two stages:

1. apply secure redaction through PDF editing layer
2. independently reopen and verify

Verification must check that targeted content is no longer:

- searchable
- copyable
- extractable
- present in OCR layer or relevant hidden objects

A black visual rectangle is never sufficient.

---

## 30. Compare architecture

Layered comparison:

1. page mapping
2. exact text diff
3. object/image diff
4. annotation diff
5. form diff
6. metadata diff
7. optional semantic/legal interpretation

AI/semantic interpretation never replaces exact diff.

---

## 31. Plugin architecture

Two-tier model.

### Community plugins

- WebAssembly Component Model / WIT
- preferred runtime: Wasmtime
- capability-based sandbox
- no ambient filesystem/network/clipboard/process access
- semantic versioned `DocumentPluginAPI`
- all mutations become normal Vocument transactions

### Native providers

Isolated external helper processes for privileged integrations such as:

- QES
- PKCS#11/HSM
- specialist scanner SDK
- proprietary enterprise integration

No plugin receives raw MuPDF/Rust pointers or direct `DocumentSession` internals.

---

## 32. Plugin trust

Trust levels:

- `Official`
- `VerifiedIdentity` (e.g. Sigstore identity)
- `TrustedKey`
- `UnsignedLocal`

Origin trust and capability permissions are separate.

Unsigned local WASM plugins may be manually installed.
Native providers must be signed/verified.

---

## 33. NetworkBroker

No module performs arbitrary external networking.

All external access passes through `NetworkBroker` and policy.

Default: offline / denied.

Possible explicitly permitted consumers:

- QES
- user-approved external link loading
- legal resolvers
- update checks
- explicitly enabled future cloud AI

---

## 34. JavaScript/forms

Restricted `PdfScriptEngine` abstraction.

Initial candidate: Boa, with QuickJS-NG as a compatibility fallback candidate.

Expose only the required PDF/form subset.

No general:

- filesystem access
- network access
- process execution
- unrestricted clipboard
- browser DOM

---

## 35. Accessibility

Maintain a common document-semantic accessibility tree.

Map natively to:

- VoiceOver
- TalkBack

FlowDocument semantics are the accessibility source of truth for authored documents.
PDF tagging/reading order informs the tree for PDF documents.

---

## 36. Configuration architecture

Central configuration lives under `/config`.

Typed runtime representations must correspond to documented configuration keys.

Do not scatter magic values through source code.

Secrets never belong in plain config files.

---

## 37. Engine isolation path

Use an `EngineTransport` abstraction now.

Initial implementation: in-process.

Future hardened implementation may move PDF/engine work to:

- macOS XPC worker
- Android isolated-service worker

Higher-level services should not need redesign when this happens.

---

## 38. Testing architecture

The Rust core must run headless in tests.

Test layers:

- unit
- integration
- corpus
- roundtrip
- rendering regression
- performance
- fuzzing
- corrupted/malicious document tests
- cross-viewer interoperability

Architecture acceptance is defined in `QUALITY.md` and feature specs.

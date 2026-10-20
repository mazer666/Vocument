# Vocument Product Requirements

**Requirements baseline:** 0.1  
**Status:** Product baseline accepted  
**Rule:** Requirements are the source of truth for behaviour. Code does not silently redefine them.

Each requirement should eventually carry implementation status and linked acceptance tests. This baseline groups requirements by product area. More granular feature specs live under `docs/features/`.

Legend:

- **Preset** — default behaviour
- **Optional** — available but not shown/activated by default
- **Later** — intentionally deferred

---

# 1. Product principles

### CORE-001 — Free and open source
Vocument is free and open source. No subscriptions, ads, paywalls or forced accounts.

### CORE-002 — Local/offline first
Core document workflows work locally without a server.

### CORE-003 — No lock-in
Normal files remain normal files. No mandatory proprietary library/import store.

### CORE-004 — Standard formats first
Use standard PDF structures whenever possible. Proprietary metadata may only supplement them.

### CORE-005 — No nagging
No onboarding tours, red dots, tip-of-the-day, upsells, repeated prompts or “make me default” nagging.

### CORE-006 — Progressive complexity
Complex functionality exists without forcing expert UI on normal users.

---

# 2. Platforms and interface

### PLATFORM-001
Phase 1 supports macOS 15+ on Apple Silicon and Android 13+.

### PLATFORM-002
Later platforms in the fixed order Windows, then Linux, then iPadOS/iOS subject to a later distribution/licensing gate. The order is binding (`project/project.yaml` → `platforms.track_order`, `CR-0003`): a platform track only starts when the previous one has a green release candidate.

### PLATFORM-003 — Platform abstraction
The platform-specific differences are expressed by implementations of core traits, never by target-specific branches inside the shared core. The following traits are mandatory before the Windows track starts:
- `PlatformFileBroker` — scoped file access (macOS security-scoped URL, Android SAF, Windows Common Item Dialog, Linux XDG portal).
- `PlatformSettingsStore` — persistent user settings and central configuration overrides.
- `PlatformAppLifecycle` — foreground/background transitions and shutdown requests, so that background work is suspended or cancelled at the right moment.
- `PlatformFontProvider` — enumeration of the fonts available on the platform, required by the deterministic pagination of the `.pdoc` authoring milestone (M4).

### PLATFORM-004 — Platform-neutral FFI surface
The exported `DocumentCore` surface must not contain platform-specific functions. A platform difference is expressed by selecting a different implementation of a core trait, never by adding an exported function. `scripts/validate-repo.sh` verifies that the exported function set does not grow per platform.

### PLATFORM-005 — Native engine per platform
Every platform links a real native engine build of the same engine version. A placeholder engine may not be linked into a production build on any platform; the build must be verifiable from the produced binary (for example by inspecting the dynamic dependencies of the shipped library).

### PLATFORM-006 — Resolvable native dependency closure
Every `DT_NEEDED` entry of a shipped native library must be resolvable **by its exact file name** inside the package. The Android dynamic linker resolves a dependency by the exact name, and the Android packaging step only includes files whose name ends in `.so`. A versioned shared object name therefore never reaches the package, and the process fails to start. The shared objects must therefore carry an unversioned `SONAME` on Android. `scripts/verify-android-native-libs.sh` verifies the closure, the agreement between the requested name and the `SONAME`, and the architecture of every shipped library.

### PLATFORM-007 — 16 KB memory page compatibility
Every `PT_LOAD` segment of a shipped native library must be aligned to at least `0x4000` bytes. Android 15 and later devices may use 16 KB memory pages, and a library with a smaller alignment does not start on such a device. The links must therefore request `-Wl,-z,max-page-size=16384`. `scripts/verify-android-native-libs.sh` rejects a library with a smaller alignment.

### UI-001 — Native platform UX
Shared core, platform-adapted interfaces.

### UI-002 — Desktop default
Calm hybrid interface with a few work areas and context-sensitive tools.

### UI-003 — Optional classic/ribbon style
Users may opt into a more classic Office-style tool layout.

### UI-004 — Customisation
Allow favourites, selected tool ordering, sidebar position, toolbar visibility, editable shortcuts and quick actions.

### UI-005 — Complexity profiles
Simple / Standard / Expert.

### UI-006 — Density
Quiet / Standard / Compact, independent of complexity profile.

### UI-007 — Theme/accessibility appearance
Light/dark/system, UI scaling, reduced motion.

### UI-008 — Status/feedback
Subtle contextual status by default; ultra-quiet optional; safety warnings remain available.

### UI-009 — The interface never blocks on native work
No native call may run on the user interface thread. This applies to **every** path, not only to the capture pipeline: the viewport rendering, the prefetch of neighbouring pages, the page thumbnails, the text extraction and the index search. The work runs on a background executor, and only the finished result is published to the interface state. A request whose result is outdated because a newer request was started in the meantime must be discarded rather than displayed.

### CMD-001 — Command palette
Search commands, settings and actions, e.g. Cmd+K.

---

# 3. Mobile UX

### MOBILE-001
Mobile is not a shrunk desktop.

### MOBILE-002 — Preset navigation
Small bottom bar around Pages / Search / Tools; contextual transformation on selection.

### MOBILE-003 — Optional larger navigation
Persistent larger bottom navigation optional.

### MOBILE-004
Desktop sidebars become sheets/drawers/fullscreen secondary views/transient overlays.

### MOBILE-005
Document content keeps screen priority.

### MOBILE-006 — Selection
Normal Android selection by default; precision aids for stylus/object editing/long drag.

### MOBILE-007 — S-Pen
Low-latency ink, pressure, smoothing, eraser, lasso, palm rejection.

### MOBILE-008 — Exit visibility
Focus/fullscreen/presentation/special modes always have an obvious exit control.

---

# 4. Viewer and navigation

### VIEW-001 — Mobile preset
Minimal reader.

### VIEW-002 — macOS preset
Comfort/professional reader.

### VIEW-003 — Later advanced reading
Reflow, large-text/accessibility view, margin crop, TTS.

### VIEW-004 — Desktop split
Same document may be shown in two independently scrolling/zooming views.

### VIEW-005
Different documents may be side-by-side outside compare mode.

### VIEW-006 — Mobile reference pin
Smartphone uses reference pin/jump rather than classic split.

### VIEW-007 — Reference pins
Named, non-document-mutating, session or persisted reference points.

### PAGEVIEW-001 — Mobile
Thumbnail sheet/fullscreen + fast scrubber.

### PAGEVIEW-002 — Desktop
Lazy intelligent thumbnails, resizable size, markers, filters, separators, direct jumps.

### PAGEVIEW-003
Optional visual light-table from early versions.

---

# 5. Logical page labels

### PAGELABEL-001
Support PDF logical page labels including Roman numerals and prefixes.

### PAGELABEL-002
Physical PDF page index remains visible/available.

### PAGELABEL-003
Navigate by physical index or logical label.

### PAGELABEL-004
Create/edit page-label ranges.

### PAGELABEL-005 — Later
OCR-assisted detection of printed page numbering and suggested labels.

---

# 6. Search

### SEARCH-001
Three modes: exact, fuzzy/OCR-tolerant, local semantic.

### SEARCH-002 — Preset
Classic exact search.

### SEARCH-003
Fuzzy mode handles OCR errors, variants, line-breaks and hyphenation.

### SEARCH-004
Semantic results always map to actual source page/text location.

### SEARCH-005
Mobile prominently shows classic/fuzzy; semantic separate.

### SEARCH-006
Desktop may expose all three while classic remains primary.

### SEARCH-007 — Optional
Search comments and forms.

### SEARCH-008 — Expert
Regex search.

### SEARCH-009
Large-document indexing must not block UI.

### SEARCH-010
Cross-document search for open document groups.

### SEARCH-011
Workspace search only for explicitly enabled workspaces.

### SEARCH-012
Cross-format search includes PDF/DOCX/ODT/EPUB where supported.

---

# 7. Local AI and semantic services

### AI-001
Provider-neutral local intelligence architecture.

### AI-002
System-local models may be used where suitable.

### AI-003
Optional explicit local model download pack.

### AI-004
User may choose Automatic / provider / local model / Off.

### AI-005
Model download shows size, hardware needs, version and licence.

### AI-006
Models removable.

### AI-007
No silent cloud fallback.

### AI-008
Classic core functions remain fully usable without any model.

### AI-009 — Later
Cloud AI only explicit opt-in.

---

# 8. Scanning

### SCAN-001 — Mobile preset
Automatic scan with quiet review.

### SCAN-002 — Optional
Expert scanning mode.

### SCAN-003
Perspective correction is mandatory.

### SCAN-004
Automatic detection, rotation, deskew, shadow reduction, contrast/colour/white balance, noise reduction, OCR and sensible compression.

### SCAN-005
Glare/reflection reduction should use multi-frame information where feasible; otherwise suggest device tilt unobtrusively.

### SCAN-006
Classic multi-page scan available.

### SCAN-007 — Mobile preset
Intelligent batch: auto capture, duplicate detection, blur warning, reorder, blank-page handling, front/back handling, batch OCR/optimisation.

### SCAN-008 — Later
Document segmentation via blanks/separators/barcodes/QR.

### SCAN-009 — Outline detection
The automatic detection has to find the **tilted** outline of the page, not a rectangle around it. A detection that ignores the tilt rectifies the page wrongly and the text recognition fails afterwards. The detection therefore works on the boundary of the page and tolerates outliers at the corners, which are often covered by a shadow.

### SCAN-010 — The scanned document can be saved
A scan that cannot be saved is a scan that is lost. The searchable PDF that the scan produces has to be savable through the platform, and the saved file has to be a complete PDF whose text can be read back.

### SCAN-011 — The document can be exported into a location that the user owns
A document that lives in a document provider — an Android `content://` URI, a macOS open panel — has no local path. The application must never turn such a URI into a file name on the disk. The core hands the bytes of the document over, the platform shell writes them into a place the user has chosen.

### SCAN-012 — A file that was changed outside the application is never overwritten silently
If the file of the document was changed by another program, the application must report the conflict and let the user decide: save as a collision-free copy, overwrite explicitly, reload the external version, or cancel. The check happens before the file is replaced, not afterwards.

### SCANNER-001
External scanner support planned through ICA/WIA/TWAIN/SANE depending platform.

---

# 9. OCR

### OCR-001
Strong local OCR is a core function.

### OCR-002
German and English built in.

### OCR-003
Additional language packs explicit, removable and versioned.

### OCR-004
Layout-aware OCR for rotation, columns, tables, headers/footers.

### OCR-005
Detect existing OCR.

### OCR-006
Selective re-OCR.

### OCR-007
OCR feeds search, copy, export and table recognition.

### OCR-008 — Preset output
Optimised searchable PDF: cleaned image + invisible text layer.

### OCR-009
Original-faithful searchable PDF selectable.

### OCR-010 — Later
Full editable reconstruction as explicit action.

### OCRCOPY-001
Raw OCR, cleaned readable text and structured export are all available.

### OCRCOPY-002 — Preset
Cleaned readable text removes artificial line breaks and performs sensible dehyphenation while preserving paragraphs.

---

# 10. Existing PDF editing

### EDIT-001 — Android preset
Small corrections.

### EDIT-002 — macOS preset
Full professional PDF editing.

### EDIT-003 — Expert
Low-level PDF object/glyph/content-stream editing optional.

### EDIT-004 — Quick Edit preset
Prefer minimal in-place change.

### EDIT-005 — Block/Reflow optional
Explicit block reconstruction for larger textual edits with impact preview.

### EDIT-006
Never silently reflow unrelated content.

### EDIT-007
Editing must account for font availability/embedding and signature impact.

---

# 11. Authoring / FlowDocument

### AUTHOR-001 — Mobile
Create PDF from blank page, images, scans, clipboard or multiple files.

### AUTHOR-002 — macOS
Real Word-like authoring mode, not just positioned PDF text boxes.

### AUTHOR-003
Document Mode and Layout Mode coexist.

### AUTHOR-004 — Document Mode
Flowing paragraphs, headings, lists, tables, automatic pagination.

### AUTHOR-005 — Layout Mode
Free-positioned text boxes, images and shapes.

### SOURCE-001
Self-authored documents can use open `.pdoc` source plus standard PDF output.

### SOURCE-002
`.pdoc` specification is public and non-proprietary in spirit.

---

# 12. DOCX/ODT

### OFFICE-001
Editable via Vocument's document model.

### OFFICE-002
Support text, paragraph styles, headings, lists, tables, images, header/footer, page breaks, comments where practical, find/replace.

### OFFICE-003
Save back to DOCX/ODT or export PDF.

### OFFICE-004
Never silently discard unsupported content.

### OFFICE-005
Roundtrip preservation is a quality requirement.

---

# 13. Professional writing

### WRITE-001
Spellcheck, grammar, language detection, dictionaries, Austrian German where available, hyphenation, typography, smart quotes/dashes/NBSP, counts, advanced find/replace, case conversion, special characters.

### WRITE-002 — Later
Optional local intelligent writing support.

---

# 14. Change tracking

### TRACK-001
Professional redline capability is a core target.

### TRACK-002
Track insertions, deletions, formatting, author and time.

### TRACK-003
Original / Markup / Clean views.

### TRACK-004
Accept/Reject.

### TRACK-005
Hide formatting-only changes.

### TRACK-006
Moved-clause detection.

### TRACK-007
Topic/change-type filters.

### TRACK-008
Export change list.

### TRACK-009
Generate clean and redline outputs together.

### TRACK-010 — Later
Semantic summary.

---

# 15. References / legal support

### REF-001
Footnotes, endnotes, links, cross-references, numbering, TOC, figure/table lists.

### REF-002 — Optional
Legal reference recognition for internal clauses/articles, annexes, defined terms, case/reference numbers.

### REF-003 — Optional providers
RIS, EUR-Lex and Curia/ECLI resolvers; no automatic fetching.

---

# 16. Image and graphics editing/extraction

### IMAGEEDIT-001 — Mobile
Basic image edit + crop/straighten.

### IMAGEEDIT-002 — macOS
Crop, brightness, contrast, saturation, grayscale, transparency, resolution/downsampling, compression, flip, perspective, deskew, cleanup.

### IMAGEEDIT-003
Non-destructive where feasible.

### IMGEXT-001 — Preset
Save/extract images with original format where possible.

### IMGEXT-002 — Optional
Professional extraction including original embedded object vs rendered appearance.

### IMGEXT-003 — Optional
Intelligent figure/diagram/image-region extraction.

### VECT-001 — Preset
Prefer original vector extraction where possible, e.g. SVG/open vector representation.

### VECT-002
Rendered image fallback.

### VECT-003 — Optional
Intelligent composite graphic reconstruction.

---

# 17. Tables

### TABLE-001 — Preset
Preserve row/column/header/merged-cell structure where reliable.

### TABLE-002
Copy/export as table, TSV, CSV, Markdown, HTML, DOCX/ODT table and spreadsheet-compatible clipboard.

### TABLE-003 — Optional
Borderless/multipage/repeated-header reconstruction with confidence indication.

---

# 18. Clipboard / drag/drop

### CLIP-001 — Preset
Cleaned readable plain text.

### CLIP-002 — Optional
Formatted copy.

### CLIP-003
Copy as plain, formatted, Markdown, HTML, raw OCR, page-aware citation text or structured table.

### CLIP-004 — Preset
Normal system clipboard.

### CLIP-005 — Optional
Auto-clear Vocument-origin clipboard after configurable delay; no app-internal clipboard history.

### DND-001
Desktop intelligent drag/drop across files, pages, text, images, comments, tables and tools.

### DND-002
Always provide a non-drag alternative.

---

# 19. Snapshot / capture / evidence

### SNAP-001
Region/page/visible-area snapshot with high-resolution render, DPI, PNG/JPEG/WebP, transparency, trim, dimensions, comments on/off, current/original view and multi-capture.

### SNAP-002 — Later
Intelligent extraction of tables/images/text/multipage regions.

### WEBEVID-001 — Later
Browser evidence capture as a separate component, no mandatory extension.

### WEBEVID-002
Arbitrary-length capture; no forced A4 pagination.

### WEBEVID-003
Visual evidence capture retains source/title/time/hash metadata.

### WEBEVID-004 — Later
Raw HTML/MHTML/WARC-like evidence.

### WEBEVID-005 — Later
Forensic chain/timestamp/TLS/report features.

---

# 20. Page workbench

### PAGE-001 — Mobile preset
Rotate, delete, move, duplicate, extract, insert, merge.

### PAGE-002 — macOS preset
Visual page workbench with thumbnails, multi-select, drag/drop, ranges, blanks, numbering, odd/even, split.

### PAGE-003
Light-table view optional.

### CROP-001 — Preset
Visual/numeric crop current/selected/all pages with preview.

### CROP-002 — Optional
CropBox/MediaBox/BleedBox/TrimBox/ArtBox management.

### CROP-003 — Optional
Intelligent auto-crop with safety margin and no silent content loss.

### ROTATE-001 — Preset
90°/180° rotation.

### ROTATE-002 — Optional
Automatic orientation.

### ROTATE-003 — Optional
Precise deskew/batch straightening.

---

# 21. Scan cleanup / colour

### CLEAN-001 — Preset
Intelligent scan cleanup: background, shadows, yellowing, text contrast, colour casts, show-through and noise; photo/document distinction; before/after preview.

### CLEAN-002
Basic adjustments remain available.

### CLEAN-003 — Optional
Professional ICC/colour-management controls.

---

# 22. Compression

### COMP-001 — Mobile preset
Lossless / Balanced / E-mail / Maximum Reduction.

### COMP-002 — macOS preset
Analysis/preview with size breakdown and expected output size.

### COMP-003 — Expert optional
DPI/codec/font/object controls.

### COMP-004
Preserve OCR/forms/bookmarks/links where appropriate.

### COMP-005
Warn about signature invalidation.

---

# 23. Review / annotations / compare

### REVIEW-001 — Preset
Full review + version comparison capability.

### REVIEW-002
Structured review sidebar, filters, threads, open/done, comment search.

### REVIEW-003 — Preset comment export
Comments remain in PDF; no extra export prompt.

### REVIEW-004 — Optional
Structured comment export to PDF summary/DOCX/ODT/Markdown/CSV.

### REVIEW-005 — Optional
Review report by topic/status/reviewer/round.

### COMPARE-001 — macOS preset
Structured content comparison.

### COMPARE-002
Synchronised scroll and change summary.

### COMPARE-003 — Mobile preset
Visual compare; structured optional.

### COMPARE-004 — Later
Semantic/legal interpretation layered on exact diff.

### ANNOT-001
Professional annotation system with editable jurist preset.

---

# 24. Redaction / sanitize / hidden content

### REDACT-001
Secure redaction removes underlying text/OCR/hidden relevant content.

### REDACT-002
Verify that redacted content is no longer searchable/extractable.

### REDACT-003
Search-assisted candidate selection.

### REDACT-004 — Later
Local smart entity suggestions, never auto-apply.

### SAN-001 — Optional
Full sanitize for metadata, hidden content, attachments, actions, comments, form remnants, etc.

### HIDDEN-001 — Preset
Normal rendering only.

### HIDDEN-002 — Optional
Hidden-content inspector.

### HIDDEN-003 — Optional
Security analysis for exposed/mismatched/reappearing content.

---

# 25. PDF restrictions/security

### SEC-001
Understand open-password vs owner/permission restrictions.

### SEC-002
Do not crack unknown encryption/open passwords.

### SEC-003
For already readable PDFs restricted only by permission flags, creating an unrestricted editable copy may be supported.

### SECARCH-001
Treat documents as untrusted input.

### SECARCH-002
No automatic PDF JavaScript.

### SECARCH-003
No automatic link/attachment execution.

### SECARCH-004
Architecture must permit later process isolation.

---

# 26. Networking, links, JavaScript, attachments

### NET-001 — Preset
Strict offline; no automatic external resource loading.

### NET-002 — Optional
Explicit per-action/session allowances.

### NET-003 — Optional
Trust profiles and network logs.

### LINKSEC-001
Advanced external-link analysis including actual URL, Unicode/Punycode, HTTP/HTTPS and local/private targets.

### LINKSEC-002
Document-wide link list.

### LINKSEC-003
No silent cloud reputation check.

### JS-001 — Preset
Restricted/sandboxed scripts; simple safe form calculation may be allowed.

### JS-002 — Optional
Full script/action inspector.

### ATTACH-001 — Preset
Local safety inspection of embedded files; never auto-open.

### ATTACH-002 — Optional
Controlled extraction/analysis/hash-list/bulk export.

---

# 27. Signatures / QES

### SIGN-001
Simple visible signature + local certificate signatures.

### SIGN-002
Provider-neutral QES architecture.

### SIGN-003
A-Trust integration required eventually; does not block first foundation milestone.

### SIGN-004 — Preset
Simple human-readable signature status.

### SIGN-005 — Optional
Detailed certificate/timestamp/revision/algorithm/eIDAS inspector.

### SIGREV-001 — Preset
Simple signature status.

### SIGREV-002 — Optional
Revision history.

### SIGREV-003 — Optional
Signed revision vs current comparison.

---

# 28. Forms

### FORM-001
Fill and create/edit forms.

### FORM-002
Text fields, checkboxes, radio, dropdown, signature, required, formatting, calculation, tab order, duplicate, flatten.

### FORM-003
No requirement for complete legacy XFA compatibility.

### FORMFILL-001 — Preset
Comfortable form filling: type-aware keyboards/controls, formatting, auto-size, required-field highlighting, validation, same-document repeat-value reuse.

### FORMFILL-002 — Later optional
Intelligent autofill; no secret personal autocomplete database.

### FORMLOGIC-001 — Preset
Safe local calculations/validation/dependencies/show-hide logic.

### FORMLOGIC-002 — Optional
Form-logic inspector/test mode.

---

# 29. Layers / multimedia / portfolios

### LAYER-001
Correct layer/OCG rendering.

### LAYER-002
Simple layer list when relevant.

### LAYER-003 — Optional
Toggle/states/rename/object assignment where feasible/print-export inclusion.

### PORT-001
Safe detection/list/open/extract of PDF portfolios.

### PORT-002 — Optional
Professional management.

### MEDIA-001
Detect multimedia/3D but never auto-run.

### MEDIA-002
Extraction allowed.

### MEDIA-003 — Later
Controlled local playback.

---

# 30. Standards / rendering / fonts

### STANDARD-001
Recognise and validate PDF/A, PDF/UA/tagged PDF and relevant PDF/X use.

### STANDARD-002
Deliberately create/convert normal PDF, PDF/A and tagged/PDF-UA-oriented output.

### VERSION-001 — Preset
Automatic PDF compatibility/version selection.

### VERSION-002 — Optional
Compatibility profiles/specific version.

### VERSION-003 — Later
Detailed compatibility inspector.

### RENDER-001
High-precision handling of fonts, Unicode, transparency, ICC, page boxes and mixed scan/text/vector content.

### FONT-001 — Preset
Controlled font fallback; metric-aware replacements; no silent layout-changing fallback.

### FONT-002 — Optional
Font inspector.

---

# 31. Bookmarks / numbering / watermarks

### BOOKMARK-001 — Preset
Professional bookmark editing.

### BOOKMARK-002 — Optional
Intelligent heading/TOC/OCR bookmark suggestions with preview only.

### WATER-001 — Preset
Classic text/image watermark.

### WATER-002 — Optional
Professional page-range/foreground/background/placeholders/preview controls.

### WATER-003 — Optional
Editable watermark/stamp profiles.

### HEADER-001 — Preset
Simple header/footer/page numbers.

### HEADER-002 — Optional
Odd/even/first-page/range/formatting/dynamic fields.

### HEADER-003 — Optional
Multiple numbering/header regimes for front matter/main/annexes.

---

# 32. Accessibility

### A11Y-001 — Preset
No mandatory alt-text nagging.

### A11Y-002 — Optional
Alt-text/decorative marking/find-missing tools.

### A11Y-003 — Optional
Local model suggestion, never automatic acceptance.

### A11YCHK-001 — Preset
No automatic full audit.

### A11YCHK-002 — Optional
Local accessibility audit.

### A11YCHK-003 — Optional
Guided repair and report.

### APP-A11Y-001
VoiceOver/TalkBack, full keyboard, focus order, scalable UI, touch targets, high contrast/reduced motion.

---

# 33. Inspector / repair

### INSPECT-001 — Preset
Basic file/document properties.

### INSPECT-002 — Optional
Technical inspector: fonts, colour, images, attachments, layers, forms, scripts, comments, boxes, OCR, revisions.

### INSPECT-003 — Optional
Structural diagnostics.

### REPAIR-001
Open damaged PDFs tolerantly where possible.

### REPAIR-002
Repair only to a separate repaired copy.

### REPAIR-003
Explain detected/fixed issues and potential losses.

### REPAIR-004 — Later
Forensic deep recovery.

---

# 34. Save / recovery / undo

### SAVE-001
Normal save should feel familiar.

### SAVE-002
Safe hybrid model: Save, Save As, Undo/Redo, Recovery and optional version history.

### SAVE-003
Autosave stores recovery state; it does not silently overwrite the user's PDF.

### SAVEATOMIC-001
Atomic save plus transaction journal.

### RECOVERY-001 — Preset
If recovery exists: Restore / Preview / Discard / Open as New.

### RECOVERY-002 — Optional
Recovery Center.

### UNDO-001
Persistent undo across sessions.

### UNDO-002
History is separate from saved file versions.

### UNDO-003
Retention configurable by time/size.

### UNDO-004
Human-level operations, not noisy engine microsteps.

### RETENTION-001 — Preset
Automatic sensible pruning.

### RETENTION-002 — Optional
Configurable duration/size/close/save/workspace rules.

### INCSAVE-001 — Preset
Full rewrite when normal safe-save path calls for it.

### INCSAVE-002 — Optional
Intelligent incremental save.

### INCSAVE-003 — Optional
Rebuild/clean PDF with explicit revision/signature warning.

---

# 35. File conflicts / integration

### FILELOCK-001
Active lock/conflict detection.

### FILELOCK-002
Detect external changes and network/cloud races.

### FILELOCK-003
Never silently overwrite.

### FILELOCK-004
Compare / Open External / Save Local as New / Cancel.

### FILELOCK-005 — Later
Cooperative multi-instance locking.

### FILEINT-001
Open With, drag/drop, share sheets, recent, file manager/Finder, mail/messenger/download open, save-back, multiple desktop documents.

### FILEINT-002
Can be chosen as system default app without nagging.

---

# 36. Print/export/share

### PRINT-001 — Preset
Normal print behaviour, no extra warnings.

### PRINT-002
Mobile normal print.

### PRINT-003
macOS professional ranges/duplex/scaling/n-up/booklet/poster/odd-even/comments/grayscale/preview.

### PRINT-004 — Optional
Preflight print check.

### PRINT-005 — Optional
Print profiles.

### EXPORT-001
Professional export foundation.

### EXPORT-002
PDF to Word-compatible, images, text, Markdown, OCR-searchable, structured tables where supported.

### SHARE-001 — Preset
Send a Copy variants: Original / Current / Optimised / Flattened.

### SHARE-002
Preserve original; warn about unapplied redaction/signature impact where relevant.

---

# 37. Metadata / finalisation / flatten

### META-001 — Preset
Preserve metadata unchanged.

### META-002 — Optional
Inspect/remove.

### META-003 — Optional
Metadata profiles.

### FINAL-001 — Preset
No finalisation wizard.

### FINAL-002 — Optional
Finalisation assistant checks comments/redaction/OCR uncertainty/attachments/metadata/forms/signatures/standards/unsaved state/links/hidden content.

### FLAT-001 — Optional
Selective flatten.

### FLAT-002
Preview/page ranges/save new file/warn about signature and editability.

---

# 38. Background work / large files / energy

### JOB-001
Central JobManager with queue/priority/pause/resume/cancel/dependencies/resumability.

### JOB-004 — Bounded execution
The job execution layer has a bounded number of worker threads and a bounded queue length, both centrally configurable (`config/performance-budgets.toml`, `[jobs]`). Submitting beyond the queue bound must fail with a typed error instead of growing memory without limit. No feature may create its own thread pool.

### JOB-005 — Priority classes and fairness
Jobs belong to a class: interactive (visible page rendering), user-initiated (OCR, import, export) or background (preprocessing, caching). Interactive work is preferred, but a job that waits beyond the configured starvation threshold is promoted by one priority level so that background work cannot starve permanently.

### JOB-006 — Typed state machine and race handling
A job moves `Queued → Running → Completed | Failed | Cancelled`. A job cancelled while running must never report success afterwards; its late result is discarded. A job cancelled while queued must never start.

### JOB-007 — Cooperative cancellation
Long-running work receives a cancellation token and stops at the next safe point. Engines that cannot be interrupted safely (MuPDF, Tesseract) run to completion and their result is discarded instead of being applied. No forceful thread termination is used anywhere.

### JOB-008 — Session lifecycle
Closing a document cancels every job of that session. A result of a closed or changed document must never be applied. Temporary resources of a cancelled job are released.

### JOB-009 — Progress and error reporting
Progress and terminal state are reported through polling of the shared job status surface. The core never calls back into the user interface, so no callback can block a UI thread. Errors are typed, never panics.

### JOB-010 — Real job payloads
A submitted job performs an actual core operation. A job kind that has no core operation attached must report a `Failed` job with a description; it must never report success without doing the work. A job that was submitted for a document revision that is no longer current is rejected with a typed error, so that a result of a stale revision is never applied.

### JOB-002 — Android preset
OS-compliant background continuation for OCR/export/indexing/compression where appropriate.

### JOB-003 — Optional
Charging/Wi-Fi/battery/thermal/per-job rules.

### BIGMOBILE-001 — Preset
Adaptive large-file mode.

### BIGMOBILE-002 — Optional
Extreme low-resource mode.

### POWER-001 — Preset
Adaptive mobile energy/thermal behaviour.

The decision belongs into the shared core, the platform shell only reports the state of the device. The background work is suspended while the device is hot or almost empty; the work that the user is waiting for continues, with a reduced parallelism on a hot device. A suspended job is not lost: it stays queued and runs as soon as the device allows it again.

### JOB-011 — Pause and resume
The background work can be paused and resumed. The running jobs are asked to stop; a job that was submitted as **resumable** goes back into the queue and runs again afterwards, a job that the user cancelled stays cancelled. The job body is shared instead of consumed, so that it can be called more than once.

### UI-010 — The scan does not block the interface
The complete chain of the scan runs off the interface thread: the edge detection, the rectification, the cleanup, the OCR **and** the synthesis of the searchable PDF. Only the finished result is published to the interface state.

### POWER-002 — Optional
Energy Saving / Balanced / Maximum Performance.

### GPU-001
Automatic hardware acceleration with safe software fallback.

---

# 39. Performance budgets

### PERF-001
Initial targets on reference hardware:

- startup ideally under 1–1.5 s
- normal first page ideally 300–500 ms after document load path is available
- adjacent page presentation effectively immediate/prefetched where possible
- first search results ideally 150–300 ms for indexed/common cases
- 60 fps target for scrolling
- no UI blocking for large documents
- progressive thumbnails
- OCR/compression backgrounded

### PERF-002 — Later
Device-class-specific budgets.

---

# 40. Automation / plugins / storage

### AUTO-001 — Later
Configurable workflow/batch automation with action chains, conditions, watched folders, rename, OCR, redaction, export and templates.

### AUTO-002
Mobile automation hidden by default.

### PLUGIN-001
Internal modularity from start.

### PLUGIN-002 — Later public
WASM plugin API with capability permissions; privileged native providers isolated out of process.

### PLUGIN-003
No mandatory plugin store.

### STORAGE-001 — Later optional adapters
SMB/NAS, WebDAV, Nextcloud, OneDrive, Google Drive, Dropbox, SFTP.

---

# 41. Settings / presets / internationalisation

### SETTINGS-001
Hierarchical calm settings with search and canonical homes.

### SETTINGS-002
Major groups: General, Interface, Reading/Navigation, Editing, Review, Scan/OCR, Security/Signatures, Export/Output, Automation/Extensions, Advanced.

### PRESET-001
Save custom workspace/UI presets.

### PRESET-002
Export/import settings early.

### PRESET-003
Do not export documents/recent history/sensitive paths by default.

### I18N-001
German and English fully maintained at launch.

### I18N-002
German should use good Austrian standard German where appropriate.

### I18N-003
Architecture supports arbitrary additional languages, pluralisation, locale formatting and RTL.

---

# 42. Distribution / updates / privacy / logging

### DIST-001
Direct installation/download always remains possible.

### DIST-002
Optional package/repository channels may be added; no store dependency.

### GIT-001
Start with `main` as the only long-lived branch.

### UPDATE-001
Secure signed update checks/releases; no forced updates.

### LOG-001
No automatic telemetry.

### LOG-002
No automatic crash upload.

### LOG-003
Local diagnostics, content-minimised/redacted, view/delete/export explicitly.

### HASH-001
SHA-256 integrity tools available.

---

# 43. Interoperability / QA / governance

### INTEROP-001
Interoperability is a binding quality principle.

### INTEROP-002
Annotations, bookmarks, forms, signatures, layers, metadata, OCR, page labels and attachments use standard PDF structures where possible.

### INTEROP-003
Proprietary extension data may never create necessary lock-in.

### QA-001
Unit/integration/UI/visual-regression/reference-PDF/roundtrip/OCR/search/redaction/signature/form/page/compression/recovery/large-doc/performance/accessibility testing from early development.

### QA-002 — Later torture lab
Fuzzing, malformed/malicious PDFs, random operation chains, crash/power loss, low memory, extreme documents, performance regressions.

### QA-003 — Prohibition of stubs in feature acceptance (CR-0002)
No functionality, engine adapter, provider, or platform workflow presented for milestone or feature acceptance may be implemented solely as a stub, dummy return value, synthetic placeholder, or mock bridge. Acceptance gates, test suites, and CI pipelines must execute real, fully integrated native/embedded implementations with real assets, realistic corpora, and authentic platform runtimes. If a platform or engine does not yet have its genuine implementation ready, that scope cannot be accepted or declared green; it must remain open or be explicitly descoped via a formal Change Request.

### REQ-001
Requirements use stable IDs.

### REQ-002
Material requirement changes use Change Requests.

### REQ-003
Agents may not silently alter accepted requirements.

### CODE-001 — Source file size limits
Hand-written source code files must remain small and understandable: target is up to 600 lines per file, and the hard maximum is 900 lines per file. Files approaching the target must be split into focused submodules (machine-generated bindings are excluded).

---

# 44. Explicit non-goals/default exclusions

Vocument does not default to:

- forced cloud
- mandatory account
- subscription/paywall
- ads
- forced proprietary library
- automatic metadata removal
- automatic redaction
- automatic document repair in place
- automatic script/attachment execution
- mandatory app PIN
- proprietary encrypted project container
- app-internal-only clipboard
- excessive onboarding/nagging

---

# 45. Privacy of the files of the user

### PRIV-001 — The files of the application are not readable for anyone else on the machine
The staging file of a save, the recovery container with its manifest and journal and every other file that the application writes for a document of the user must have restrictive permissions: `0600` for files, `0700` for directories.

### PRIV-002 — The camera is released when it is not needed
A camera that stays bound keeps the privacy indicator of the system on and drains the battery. The camera must be released when the view leaves the interface. The state of the interface may only be changed on the main thread.

---

# 46. Best-in-class requirement

A feature is not accepted solely because it exists.

For relevant workflows, acceptance must consider:

- functional completeness
- correctness
- performance
- data integrity
- interoperability
- security/privacy
- accessibility
- number of steps
- clarity
- error recovery
- visual polish

See `QUALITY.md` and `docs/BEST_IN_CLASS.md`.

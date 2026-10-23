# CR-0003 — Windows and Linux Platform Support (Phase 2, after macOS and Android)

**Status:** proposed
**Date:** 2026-10-20
**Proposer role:** ROLE-01 (Product & Requirements Steward), ROLE-02 (Chief Architect)
**Reviewer roles:** ROLE-13 (QA & Test Engineer), ROLE-15 (Interoperability & Standards Engineer), ROLE-16 (Release & Supply-Chain Engineer), ROLE-18 (Licensing & Open-Source Reviewer)

## Problem

`REQUIREMENTS.md` §2 (`PLATFORM-002`) lists Windows and Linux as later platforms, but the
repository currently contains **no** platform abstraction that would allow them to be
added without touching the shared core, and there is no written plan for the parts that
genuinely differ per platform.

Adding a third and fourth platform without such a plan is the exact failure mode this
project's governance exists to prevent: the shared Rust core would grow
`#[cfg(target_os = ...)]` branches, the FFI surface would fork per platform, and the
two existing platforms would start to drift apart. The macOS and Android applications
have so far been written independently (`apps/macos` in Swift, `apps/android` in
Kotlin), which does not scale to four platforms.

At the same time, the platform-specific integration work that Windows and Linux
require is **not yet done for macOS and Android either** in the areas that dominate the
effort:

- The `FileBroker` boundary is implemented in the core, but the macOS
  `ScopedLocalFileBroker` and the Android `SafFileBroker` are the only implementations.
- The `NetworkBroker`, `AppCatalog`, `FontService`, `SignatureService`, `PluginHost`,
  `AccessibilityService`, `SettingsService`, `DiagnosticsService` and `EngineTransport`
  services declared in `ARCHITECTURE.md` §2 do not exist yet on any platform.
- The job execution layer (`ARCHITECTURE.md` §8) only became a real worker pool with
  M1.5. It is now platform independent by design, which is the first prerequisite for
  additional platforms.

## Requested Change

Establish a **third and fourth platform track** that runs strictly *after* macOS and
Android, and define the platform abstraction that makes it possible.

### 1. Platform priority and order of work

The order is fixed and binding:

| Track | Platform | Status | Entry criterion |
|---|---|---|---|
| 1 | macOS 15+ (Apple Silicon) | active | — |
| 2 | Android 13+ (`arm64-v8a`) | active | — |
| 3 | Windows 11+ (`x86_64`) | planned | macOS and Android release candidates are green |
| 4 | Linux (x86_64, Wayland + X11) | planned | Windows track reaches an alpha |

Windows before Linux, because Windows is the larger installed base for the juristic
and administrative target group and because a single additional platform already
proves the abstraction.

### 2. Platform abstraction (must exist before track 3 starts)

The following boundaries must become explicit, testable interfaces in the shared core
before any Windows or Linux code is written:

- **`PlatformFileBroker`** — a single trait for scoped file access, with the existing
  macOS security-scoped URL broker and the Android SAF broker as the first two
  implementations. The Windows implementation uses the Common Item Dialog with
  `IFileOpenDialog` and file handles; the Linux implementation uses the XDG portal
  (`org.freedesktop.portal.FileChooser`) or the `GTK` file chooser.
- **`PlatformSettingsStore`** — a single trait for reading and writing the central
  configuration (`config/*.toml`) and user preferences. macOS: `UserDefaults`;
  Android: `SharedPreferences`; Windows: the registry or a JSON file in
  `%APPDATA%`; Linux: the XDG config directory.
- **`PlatformAppLifecycle`** — a single trait that reports app foreground/background
  transitions and shutdown requests, so that background jobs are suspended or cancelled
  at the right moment. This is the missing piece for the Android background state and
  for the macOS app termination.
- **`PlatformFontProvider`** — a single trait that enumerates the fonts available to
  the platform, required by the later `FontService` and by the `.pdoc` authoring
  milestone (M4), where deterministic pagination depends on the available fonts.

### 3. New application shells

The two existing applications are not shared code, and this CR does not merge them.
Each new platform gets its own shell in the idiomatic toolkit of that platform:

- `apps/windows` — C# / .NET 8 with WinUI 3 or WPF, depending on the decision
  recorded in a future ADR upon Track 3 entry (CR-0003).
- `apps/linux` — either a GTK4 / libadwaita application in Rust, or a
  Tauri/React shell around the Rust core. The decision is deferred to a future
  ADR upon Track 4 entry because it depends on the outcome of track 3.

The shell talks to the same `DocumentCore` UniFFI surface. The FFI surface must not
grow platform-specific functions; a platform difference is expressed by choosing a
different implementation of a core trait, never by a new exported function.

### 4. Native engine builds per platform

MuPDF 1.28.5 is built per target, the same way the Android build now works
(`scripts/prepare-android-mupdf.sh`):

- `scripts/prepare-windows-mupdf.ps1` — MSVC toolchain, `x86_64-pc-windows-msvc`.
- `scripts/prepare-linux-mupdf.sh` — system toolchain, `x86_64-linux-gnu`.

The same rule applies everywhere: **no placeholder engine may be linked into a
production build**. `scripts/validate-repo.sh` enforces this for every platform.

### 5. Requirement and architecture changes

- `PLATFORM-002` is refined from a one-line "later" to an explicit track with entry
  criteria.
- `ARCHITECTURE.md` gains a section describing the platform abstraction and the
  per-platform implementations, and the four-track platform priority.
- `ROADMAP.md` gains a "M5 — Windows Shell" and "M6 — Linux Shell" section, both
  explicitly *after* the existing milestones.

## Reason

- The project already proves that a shared Rust core with a thin native shell is
  viable. Extending that model is far cheaper than re-architecting it later.
- Windows is the platform with the largest installed base among the juristic and
  administrative users this product targets, and it is currently unsupported.
- Linux is the platform with the lowest barrier to contribution, because the toolchain
  is fully available in the existing CI environment.
- A written plan prevents the "add a platform, fork the core" failure mode that the
  architecture baseline explicitly tries to avoid (`ARCHITECTURE.md` §1, goal 8).

## Impacted Requirement IDs

- Refines `PLATFORM-002` (later platforms: Windows, Linux).
- Adds `PLATFORM-003` (platform abstraction: `PlatformFileBroker`,
  `PlatformSettingsStore`, `PlatformAppLifecycle`, `PlatformFontProvider`).
- Adds `PLATFORM-004` (no platform-specific exported FFI functions).
- Extends `PERF-001` (performance budgets are measured per platform).

## Impacted Architecture

- `core/vocument-core/src/` — new platform abstraction traits.
- `platform/vocument-ffi/` — unchanged in shape; the surface stays platform neutral.
- `engines/pdf-mupdf/build.rs` — already resolves the MuPDF installation per target.
- New `apps/windows/` and `apps/linux/` shells.
- New `scripts/prepare-{windows,linux}-mupdf.*` build scripts.

## Impacted Platforms

- Windows 11+ (`x86_64`) — new.
- Linux (`x86_64`, Wayland and X11) — new.
- macOS and Android — unaffected, but they gain the abstraction as the first two
  implementations of the new traits.

## Milestone Impact

- **No existing milestone changes.** M0, M1 and M2 keep their scope.
- New milestones **M5 — Windows Shell** and **M6 — Linux Shell** are added *after* the
  current roadmap, and are explicitly **not** started until the entry criteria above
  are met.
- The platform abstraction itself becomes part of **M2** (macOS Professional Review),
  because it is the milestone in which the second platform integration (Android)
  becomes product-relevant.

## Acceptance / Test Changes

- Every platform abstraction trait gets a conformance test suite in the core that
  runs against the platform independent reference implementation.
- Each platform shell must pass the shared acceptance scenario:
  `Open → search → select text → highlight → rotate → undo → redo → save → recover → reopen`.
- The MuPDF engine build per platform is verified by the same
  `readelf`/dependency check that the Android build now uses: the produced library
  must depend on the real `libmupdf` and must not depend on a placeholder.
- Rendering is verified with **image based regression tests** against reference
  renders, not by exit codes or file sizes.

## Risks

- **Scope risk (High):** four platforms multiply the shell work. The mitigation is
  the strict ordering: only one new platform is in active development at a time.
- **Drag risk (Medium):** the macOS and Android milestones could stall while the
  platform abstraction is extracted. The mitigation is that the abstraction is
  additive: the existing implementations are wrapped, not rewritten.
- **Font determinism risk (Medium):** the `.pdoc` authoring milestone (M4) requires
  deterministic pagination, which depends on the fonts of the platform. This is
  already an open architectural question and is *not* solved by this CR.
- **Licensing risk (Low):** MuPDF is AGPL-3.0-or-later and Windows/Linux
  distributions must comply with the copyleft obligations. The project already
  distributes an AGPL application; the additional obligation is that the
  distributed binaries must ship the corresponding source. This is a release
  process obligation, recorded in `THIRD_PARTY.md`.

## Open decisions

These decisions cannot be taken safely without further evidence and are therefore
deliberately **not** decided in this CR:

1. **Windows UI toolkit: WinUI 3 or WPF.** WinUI 3 is the modern stack and aligns
   with the Fluent design language, but it has a heavier deployment story. WPF is
   mature and has the broadest reach. Decision deferred to a dedicated ADR upon Track 3 entry.
2. **Linux UI toolkit: native GTK4/Rust, or a web shell (Tauri).** A native toolkit
   fits the "no cloud dependency, native feel" principle better, but the web shell
   is significantly cheaper to build. Decision deferred to a dedicated ADR upon Track 4 entry.
3. **Distribution channel for Windows.** The Microsoft Store, an MSI/NSIS installer,
   or a portable zip. This depends on the signing and update story, which is not yet
   designed. Decision deferred.

## Decision

**proposed** — awaiting review by ROLE-02, ROLE-13, ROLE-15, ROLE-16 and ROLE-18.

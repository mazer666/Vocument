# Building Vocument from the source

This document describes how the source of a released binary is obtained and
how it is built. It exists because section 6 of the GNU Affero General
Public License v3.0 asks every binary that is handed out to be offered
together with the complete corresponding source. Section 13 of the same
licence asks for the source to be offered when the program is used over a
network — a duty that does not apply to Vocument, which has no server.

The decision that this document follows is in
`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` (E1–E5).

---

## 1. The source of a binary

Every binary of Vocument names the revision of the source it was built from.
The revision is in three places, and all three are readable by the user of
the binary:

| Where | How it is read |
|---|---|
| The library itself | `document_core_build_info()` — the handshake of the FFI layer returns the version, the revision, the state of the tree and the licence. The shell shows it in its header. |
| The macOS bundle | `Contents/Resources/SOURCE.txt` and the key `VocumentSourceRevision` in `Contents/Info.plist` |
| The Android package | `assets/licence/SOURCE.txt` |

The revision is a commit of the repository at <https://github.com/mazer666/Vocument>.

To obtain the source of a given binary:

```bash
git clone https://github.com/mazer666/Vocument
cd Vocument
git checkout <the revision from SOURCE.txt>
```

The archive of the same revision, which is what a release carries, is built
by `scripts/release-source-archive.sh`. It contains exactly the files of the
repository at that revision — the same files that were compiled.

> **A binary from a dirty tree is not published.** The revision of a tree
> with uncommitted changes does not describe the source of the binary, and
> the AGPL-3.0 asks for the *complete* corresponding source. The release
> scripts (`scripts/release-macos.sh`, `scripts/release-android.sh`) refuse
> to package such a build and name the reason.

---

## 2. The pinned versions

A build that is meant to be the same as a released binary needs the same
versions of the tools and of the libraries. The pins of the project:

| Component | Version | Where it is pinned |
|---|---|---|
| Rust | 1.85 or newer (`rust-version` in `Cargo.toml`) | `Cargo.toml` |
| MuPDF | 1.28.5 (the official source archive, SHA-256 `98a5c10c…b7e934`) | `scripts/prepare-android-mupdf.sh` |
| Tesseract | 5.x, cross-built from the source | `scripts/prepare-android-tesseract.sh` |
| Leptonica | 1.82.0, cross-built from the source | `scripts/prepare-android-tesseract.sh` |
| Android NDK | r27c | `scripts/prepare-android-*.sh`, the release notes |
| Android SDK | platform 35, build tools 35.0.0 | `apps/android/app/build.gradle.kts` |
| Gradle | 9.8.1 | `apps/android/gradle/wrapper/gradle-wrapper.properties` |
| Kotlin | 2.0.21 | `apps/android/build.gradle.kts` |
| JDK | 17 | `apps/android/app/build.gradle.kts` |
| Android Gradle Plugin | 8.7.2 | `apps/android/build.gradle.kts` |
| Swift | 6.0 (`swift-tools-version: 6.0`) | `apps/macos/Package.swift` |
| macOS | 15.0 or newer | `apps/macos/Package.swift` |

The crates of the Rust dependencies are pinned in `Cargo.lock`; the versions
of the Android libraries are pinned in `apps/android/app/build.gradle.kts`
and in the `libs.versions.toml` of the project, if there is one.

---

## 3. The build of the Rust core and the FFI layer

The FFI layer links MuPDF. The host build needs the development files of
MuPDF; the build script of the engine finds them through `MUPDF_DIR`:

```bash
# 1. MuPDF 1.28.5 for the host (the build of the source, not a package)
make -C <the source of MuPDF 1.28.5> \
    HAVE_X11=no HAVE_GLUT=no HAVE_CURL=no \
    build_prefix=host- build=release shared=yes -j4
make -C <the source of MuPDF 1.28.5> \
    build_prefix=host- build=release shared=yes install prefix="$PWD/target/mupdf-host"

# 2. The workspace
export MUPDF_DIR="$PWD/target/mupdf-host"
export LD_LIBRARY_PATH="$PWD/target/mupdf-host/lib"
export TESSERACT_DIR=/usr
export TESSDATA_PREFIX="$PWD/apps/android/app/src/main/assets/tessdata"

cargo build --release -p vocument-ffi
```

The result is `target/release/libvocument_ffi.so` (or `.dylib`).

## 4. The build of the bindings

The bindings for Swift and Kotlin are generated — they are never written by
hand:

```bash
scripts/generate-bindings.sh
```

## 5. The build of the macOS application

```bash
cd apps/macos
swift build -c release
```

The bundle that is distributed is assembled by `scripts/release-macos.sh`:
it writes the notices of the AGPL-3.0 and the offer of the source into the
bundle, signs it and notarises it.

## 6. The build of the Android application

```bash
# 1. The native libraries for arm64-v8a, with 16 KB pages
scripts/prepare-android-mupdf.sh
scripts/prepare-android-tesseract.sh

# 2. The Rust library for the four Android ABIs
scripts/build-android-rust.sh

# 3. The package
cd apps/android
./gradlew --no-daemon :app:assembleRelease
```

The package that is distributed is assembled by `scripts/release-android.sh`:
it writes the notices of the AGPL-3.0 and the offer of the source into the
package and builds the checksum.

---

## 7. What a build does **not** reproduce

The build is reproducible in the sense that the same revision with the same
pins produces the same artefacts. Two things are deliberately **not** part of
it:

* **No build timestamp** is written into the binary. A timestamp would make
  two builds of the same revision differ, and the identification of the
  build is the revision, not the moment of the build.
* **No network access.** The build reads its dependencies from the caches
  of the machine (`~/.cargo/registry`, `~/.gradle/caches`) and from the
  pinned source archives. The product itself has no network function either
  (E4 of the licensing decision).

## 8. The check of the gates before a release

```bash
scripts/local-ci.sh          # the complete gate of the project
scripts/audit-licenses.sh    # the licence of every component
```

Both must be green before a release is built. The results of the last run
are recorded in `docs/acceptance/` and in `project/project.yaml`.

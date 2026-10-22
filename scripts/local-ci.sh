#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The tools of Rust live in `~/.cargo/bin`. The path of a non-interactive
# shell does not always carry it, and a gate that stops with "cargo: command
# not found" is a gate that was not run.
if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# The engine of the PDF and the OCR are third-party components that are not
# part of the source of the product: the build links against them, and the
# version of the installation that is linked belongs into the record.
export MUPDF_DIR="${MUPDF_DIR:-$REPO_ROOT/target/mupdf-host}"
export TESSERACT_DIR="${TESSERACT_DIR:-$REPO_ROOT/target/tesseract-host}"
# The libraries of the host have to be found when the tests and the binaries
# run. The name of the variable differs between the platforms: Linux reads
# `LD_LIBRARY_PATH`, macOS reads `DYLD_LIBRARY_PATH` and **ignores** the other
# one. Without the second one the binaries that link MuPDF die on macOS with
# "Library not loaded: @rpath/libmupdf.dylib". Both are therefore set, so
# that the same gate runs on both platforms.
export LD_LIBRARY_PATH="$MUPDF_DIR/lib:$TESSERACT_DIR/lib:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$MUPDF_DIR/lib:$TESSERACT_DIR/lib:${DYLD_LIBRARY_PATH:-}"

# --- The build record ----------------------------------------------------
# A release always leaves a record of what was built: the revision, the
# tools, the tests, the gates, the source archive. The record is written
# when the gate is green — and also when a step of the gate fails, because a
# failed release has to leave a trace as well.
BUILD_RECORD_ARGUMENTS=()
RECORD_WRITTEN=0
write_build_record() {
    local exit_code="$1"
    if [[ "$RECORD_WRITTEN" == "1" ]]; then
        return 0
    fi
    RECORD_WRITTEN=1
    ./scripts/build-record.sh --gate-exit-code "$exit_code" \
        ${BUILD_RECORD_ARGUMENTS[@]+"${BUILD_RECORD_ARGUMENTS[@]}"} || true
}
trap 'write_build_record $?' EXIT

./scripts/validate-repo.sh

echo "==> Checking the gate of the distribution: the licence, the notices, the offer of the source..."
./scripts/validate-distribution.sh

echo "==> Auditing the licences of every component against the inventory..."
./scripts/audit-licenses.sh

echo "==> Checking the structure of the Swift sources..."
python3 scripts/check-swift-structure.py

echo "==> Proving the Swift sources: the grammar of Swift and the names against the generated bindings..."
if [[ "${VOCUSMENT_SWIFT_CHECK_SKIP:-0}" == "1" ]]; then
  echo "SKIPPED: the static proof of the Swift sources was skipped by the environment."
  echo "         The proof is NOT RUN here and must not be reported as passed."
else
  # The proof is an *addition* to the gate: a missing grammar of Swift makes
  # the proof NOT RUN (exit code 2) — it does not make the build of the
  # product fail. A *defect* in the sources (exit code 1) does fail the gate.
  set +e
  python3 scripts/check-swift-sources.py --install-if-missing
  SWIFT_PROOF_EXIT=$?
  set -e
  if [[ "$SWIFT_PROOF_EXIT" -eq 2 ]]; then
    echo "NOT RUN: the static proof of the Swift sources could not be carried out here."
    echo "         The Swift sources were NOT proved in this run — do not report them as proved."
  elif [[ "$SWIFT_PROOF_EXIT" -ne 0 ]]; then
    echo "ERROR: the static proof of the Swift sources found a defect (exit code $SWIFT_PROOF_EXIT)." >&2
    exit "$SWIFT_PROOF_EXIT"
  fi
  set +e
  python3 scripts/check-swift-sources.py --self-test
  SWIFT_SELFTEST_EXIT=$?
  set -e
  if [[ "$SWIFT_SELFTEST_EXIT" -eq 2 ]]; then
    echo "NOT RUN: the self-test of the Swift proof could not be carried out here."
  elif [[ "$SWIFT_SELFTEST_EXIT" -ne 0 ]]; then
    echo "ERROR: the self-test of the Swift proof failed (exit code $SWIFT_SELFTEST_EXIT)." >&2
    exit "$SWIFT_SELFTEST_EXIT"
  fi
fi

echo "==> Running Cargo workspace checks..."
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# The output of the tests is kept: the build record reads the counts of the
# tests out of it, so that the tests are not run twice for one gate run.
#
# The tests run **one after the other** (`--test-threads=1`). Several of them
# share the state of the process: the rail of the profession, the cache of
# the thumbnails, the pool of the jobs and the registry of the sessions are
# global. Run at the same time, the tests of one crate write over each other
# — the result then depends on which of them wins the race and not on what
# the product does. The tests of the crate that shares the state hold a lock
# for the part that they share; the rest of the suite runs serially, so that
# a run of the gate is a **reproducible** proof and not a lottery.
mkdir -p "$REPO_ROOT/target/build-record"
TEST_LOG="$REPO_ROOT/target/build-record/cargo-test.log"
set +e
cargo test --workspace -- --test-threads=1 > "$TEST_LOG" 2>&1
TEST_EXIT_CODE=$?
set -e
cat "$TEST_LOG"
if [[ "$TEST_EXIT_CODE" -ne 0 ]]; then
    echo "ERROR: the tests of the workspace failed (exit code $TEST_EXIT_CODE)." >&2
    echo "       The output is in $TEST_LOG." >&2
    exit "$TEST_EXIT_CODE"
fi
BUILD_RECORD_ARGUMENTS=(--test-log "$TEST_LOG" --test-exit-code "$TEST_EXIT_CODE")

echo "==> Verifying the secure redaction (M2.1): the content is removed, the removal is verified, the undo is refused..."
cargo test -p vocument-pdf-mupdf redaction_tests
cargo test -p vocument-ffi --test scan_workflow_e2e_tests a_redaction

echo "==> Verifying the inspection of the links and the actions of a document (M2.2)..."
cargo test -p vocument-pdf-mupdf inspection_tests
cargo test -p vocument-core document_inspection
cargo test -p vocument-ffi --test document_inspection_e2e_tests

echo "==> Verifying the inspection of the attachments and the signatures of a document (M2.3)..."
cargo test -p vocument-pdf-mupdf attachment_signature_tests
cargo test -p vocument-ffi --test document_inspection_e2e_tests

echo "==> Generating and verifying UniFFI bindings (Swift & Kotlin)..."
BINDING_HASH_BEFORE="$(shasum -a 256 apps/macos/Sources/VocumentFFI/* apps/macos/Sources/VocumentFFIHeaders/include/* apps/android/app/src/main/java/uniffi/vocument_ffi/* 2>/dev/null || true)"
./scripts/generate-bindings.sh
BINDING_HASH_AFTER="$(shasum -a 256 apps/macos/Sources/VocumentFFI/* apps/macos/Sources/VocumentFFIHeaders/include/* apps/android/app/src/main/java/uniffi/vocument_ffi/*)"

if [[ "$BINDING_HASH_BEFORE" != "$BINDING_HASH_AFTER" ]]; then
  echo "ERROR: Generated UniFFI bindings were out of sync with Rust source." >&2
  echo "Run './scripts/generate-bindings.sh' and commit the regenerated files." >&2
  exit 1
fi
echo "✓ Generated UniFFI bindings are clean and in sync."

# --- The Swift proof ------------------------------------------------------
# The compiler is looked for on the PATH and in the two places where a
# toolchain that was installed by hand usually lives. A compiler that is not
# there is a missing proof, not a defect of the product: the step says
# `NOT RUN` and the gate goes on.
SWIFT_BIN=""
for candidate in "$(command -v swift 2>/dev/null || true)" /opt/swift/usr/bin/swift /usr/share/swift/usr/bin/swift; do
  if [[ -n "$candidate" && -x "$candidate" ]]; then
    SWIFT_BIN="$candidate"
    break
  fi
done

if [[ -f "apps/macos/Package.swift" && -n "$SWIFT_BIN" ]]; then
  export PATH="$(dirname "$SWIFT_BIN"):$PATH"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    # The **application** needs the macros of SwiftUI (`SwiftUIMacros`). The
    # command line tools carry them not: a build with them stops with
    # *"external macro implementation type 'SwiftUIMacros.StateMacro' could
    # not be found"*. The hosted runner chooses a full Xcode for that reason
    # (`ci-macos.yml`), and the local gate has to choose the same one — a
    # gate that measures something else than the runner is not the gate of
    # the project.
    XCODE_SELECTED=""
    for xcode in $(ls -d /Applications/Xcode*.app 2>/dev/null | sort -V -r); do
      swift_version="$(DEVELOPER_DIR="$xcode" /usr/bin/xcrun swift --version 2>/dev/null | head -n 1 || true)"
      echo "  ${xcode}: ${swift_version}"
      if [[ "$swift_version" == *"Swift version 6."* ]] || [[ "$swift_version" == *"Swift version 7."* ]]; then
        export DEVELOPER_DIR="$xcode"
        XCODE_SELECTED="$xcode"
        break
      fi
    done

    echo "==> Building macOS Swift targets (VocumentMacApp & VocumentMacHandshakeCLI) and running Swift → UniFFI → Rust handshake..."
    if [[ -z "$XCODE_SELECTED" ]]; then
      echo "NOT RUN: no full Xcode with Swift 6 was found on this machine, and the"
      echo "         command line tools carry no macros of SwiftUI. The application"
      echo "         of macOS was NOT built and must not be reported as built."
    elif ! DEVELOPER_DIR="$XCODE_SELECTED" xcodebuild -license check >/dev/null 2>&1; then
      echo "NOT RUN: the licence of the Xcode in ${XCODE_SELECTED} was not accepted."
      echo "         The acceptance is a decision of the owner of the machine and"
      echo "         needs 'sudo xcodebuild -license'. The application of macOS was"
      echo "         NOT built and must not be reported as built."
    else
      swift build --package-path apps/macos --product VocumentMacApp
    fi
    swift run --package-path apps/macos vocument-mac-handshake
  else
    # On a platform without the macOS SDK the **application** cannot be
    # built — SwiftUI, AppKit and CoreGraphics do not exist there. The
    # **data path** can: the kit, the generated bindings and the handshake
    # command are free of them, and the handshake opens real documents,
    # renders them, searches in them, writes into them, saves them, detects
    # a conflict, recovers a crash and runs the gate of the interoperability.
    echo "==> Building the Swift data path (generated bindings, kit, handshake) and running Swift → UniFFI → Rust handshake..."
    swift build --package-path apps/macos --target VocumentFFI
    swift build --package-path apps/macos --target VocumentMacKit
    swift build --package-path apps/macos --product vocument-mac-handshake
    swift run --package-path apps/macos vocument-mac-handshake
    echo "NOTE: the **application** itself is NOT built here — SwiftUI, AppKit and"
    echo "      CoreGraphics are part of the macOS SDK, and this runner is not a Mac."
    echo "      The build of the application and the surface of the review are NOT RUN"
    echo "      here and must not be reported as passed."
  fi
else
  echo "NOT RUN: the Swift proof needs a Swift toolchain — the compiler is neither on"
  echo "         the PATH nor in /opt/swift/usr/bin. The Swift sources were NOT read"
  echo "         by a compiler in this run and must not be reported as compiled."
fi

echo "==> Verifying the native Android libraries (dependency closure, SONAME, 16 KB page alignment)..."
./scripts/verify-android-native-libs.sh

echo "==> Verifying the Android PDF engine on a real ARM64 target (qemu-aarch64)..."
if command -v qemu-aarch64-static >/dev/null 2>&1 || [[ -x /usr/bin/qemu-aarch64-static ]]; then
  ./scripts/verify-android-arm64-pdf.sh
else
  echo "SKIPPED: qemu-aarch64-static is not available on this runner."
  echo "         The ARM64 engine proof is NOT RUN here and must not be reported as passed."
fi

echo "==> Running real Android runtime & scan-capture verification (Gradle assembleDebug, apkanalyzer, unit tests, and Kotlin handshake)..."
# The runtime proof of Android needs the Android SDK and the NDK. An
# environment without them does not make the proof fail — it makes it
# **NOT RUN**, and a NOT RUN proof is never reported as a passed one.
ANDROID_SDK_DIRECTORY="${ANDROID_HOME:-/opt/homebrew/share/android-commandlinetools}"
if [[ -d "$ANDROID_SDK_DIRECTORY" ]]; then
  ./scripts/verify-android-runtime.sh
else
  echo "SKIPPED: the Android runtime proof needs the Android SDK ($ANDROID_SDK_DIRECTORY)."
  echo "         The Android runtime proof is NOT RUN here and must not be reported as passed."
fi

echo "==> Building the release candidate (internal — it is NOT published) and checking it..."
# The release candidate is the set of artefacts of one revision that a
# reviewer can look at *before* anything is handed out. It is built here and
# checked here, so that the gate proves that the candidate of the revision
# holds up — and the gate stays the same proof for everybody.
#
# The convention is the convention of the gate: **an error that concerns the
# candidate is an error** — the gate goes red. A step that cannot be run in
# this environment (the macOS bundle without a macOS SDK, the signed Android
# package without a key) is **NOT RUN** or **BLOCKED**, it is named with its
# reason in the log of the candidate, and it is never reported as passed.
#
# The candidate is not published: the legal clearance (E5) is open (B-1 of
# docs/releasing/RELEASE-BLOCKERS.md). Nothing is uploaded, nothing is
# tagged, nothing is announced.
./scripts/release-candidate.sh --test-log "$TEST_LOG" --test-exit-code "$TEST_EXIT_CODE"
./scripts/verify-release-candidate.sh

echo "==> Local CI completed successfully."

# The record of this run: the revision, the tools, the tests, the gates and
# the source archive. It is written in `target/build-record/`.
write_build_record 0
trap - EXIT

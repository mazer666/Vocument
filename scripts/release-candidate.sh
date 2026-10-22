#!/usr/bin/env bash
# The release candidate of Vocument — internal, not published.
#
# A release candidate is the set of artefacts of **one revision** that a
# reviewer can look at *before* anything is handed out. This script builds
# that set, measures it and writes the manifest that says what it is.
#
# **The candidate is not published.** There is no upload, no tag, no store
# and no announcement: the legal clearance (E5) of the licensing decision is
# open, and only a person can give it (see `docs/releasing/RELEASE-BLOCKERS.md`,
# B-1). The scripts that hand something out — `scripts/release-macos.sh` and
# `scripts/release-android.sh` — are therefore **not** run here.
#
# What the candidate holds:
#
#   source/         the archive of the source of the revision, with its checksum
#   build-record/   the build record of the run (`scripts/build-record.sh`)
#   licences/       the audit of the licences and the notices of the third party
#   build/          the Rust library, built from the unpacked archive
#   logs/           the output of every step
#   MANIFEST.md     the manifest, readable
#   manifest.json   the manifest, machine readable
## The manifest draws the difference between three kinds of artefacts, and
# that difference is the point of the candidate:
#
# 1. **The source archive is reproducible.** It is built twice and the two
#    builds are compared byte by byte: the content of the archive is the tree
#    of the revision and the compression carries no clock (`gzip -n`).
# 2. **The applications are not proven to be bit-identical.** They hang on
#    the versions of the tools, on the stamps in the bundle and on the
#    signature. The candidate measures what *is* equal — the checksums of the
#    sources, the version, the checks that were run — and says explicitly
#    that the binaries themselves are **not** claimed to be equal.
# 3. **What cannot be built here is named.** The macOS bundle needs an Apple
#    identity, the signed Android package needs a key: both are the work of
#    the project owner (B-2, B-3).
#
# The states of the checks: `PASS` (the check was carried out and it
# succeeded), `FAIL` (it was carried out and it did not succeed), `NOT RUN`
# (it could not be carried out in this environment — not a defect of the
# product) and `BLOCKED` (it needs a decision, an identity or a key that
# only the project owner has). The report of the candidate is
# `docs/releasing/RELEASE-CANDIDATE.md`.
#
# Usage: scripts/release-candidate.sh [--revision <sha>] [--output <directory>]
#                                      [--test-log <file>] [--test-exit-code <n>]
#                                      [--skip-library]
#
# Exit code 0: the candidate was built and every check that could be run here passed.
# Exit code 1: a check failed — the candidate is not usable.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The tools of Rust live in `~/.cargo/bin`. The path of a non-interactive
# shell does not always carry it, and a candidate that says "not installed"
# for an installed tool would be a candidate that lies.
if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# The engine of the PDF and the OCR are third-party components that are not
# part of the source of the product: the build needs them, and the version
# of the installation that is linked belongs into the manifest.
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

REVISION_ARGUMENT=""
OUTPUT_DIRECTORY="$REPO_ROOT/target/release-candidate"
TEST_LOG_ARGUMENT=""
TEST_EXIT_CODE_ARGUMENT=""
SKIP_LIBRARY=0

usage() {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --revision) REVISION_ARGUMENT="$2"; shift 2 ;;
        --output) OUTPUT_DIRECTORY="$2"; shift 2 ;;
        --test-log) TEST_LOG_ARGUMENT="$2"; shift 2 ;;
        --test-exit-code) TEST_EXIT_CODE_ARGUMENT="$2"; shift 2 ;;
        --skip-library) SKIP_LIBRARY=1; shift ;;
        --help | -h) usage 0 ;;
        *) echo "unknown argument: $1" >&2; usage 2 ;;
    esac
done

# --- The revision --------------------------------------------------------
# The candidate is about **one** revision. Everything below is measured for
# it, and the manifest names it.
REVISION="${REVISION_ARGUMENT:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
RESOLVED_REVISION="$(git rev-parse --verify "${REVISION}^{commit}" 2>/dev/null || true)"
if [[ -z "$RESOLVED_REVISION" ]]; then
    echo "ERROR: the revision '$REVISION' is not a commit of this repository." >&2
    exit 1
fi
REVISION="$RESOLVED_REVISION"

VERSION="$(python3 - <<'PYTHON'
import re
text = open("Cargo.toml", encoding="utf-8").read()
match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
print(match.group(1) if match else "0.0.0")
PYTHON
)"

mkdir -p "$OUTPUT_DIRECTORY/source" "$OUTPUT_DIRECTORY/build-record" \
    "$OUTPUT_DIRECTORY/licences" "$OUTPUT_DIRECTORY/logs"

FACTS_FILE="$(mktemp)"
CHECKS_FILE="$(mktemp)"
ARTIFACTS_FILE="$(mktemp)"
SECOND_ARCHIVE_DIRECTORY="$(mktemp -d)"
trap 'rm -f "$FACTS_FILE" "$CHECKS_FILE" "$ARTIFACTS_FILE"; rm -rf "$SECOND_ARCHIVE_DIRECTORY"' EXIT

# --- The helpers ---------------------------------------------------------
# The facts are written as `key<TAB>value` lines, the checks and the
# artefacts as tab separated rows. The values are plain text; the escaping
# happens once, in the step that writes the manifest.

clean_field() {
    # A field of a row must not carry a tab or a newline: the rows are read
    # field by field.
    printf '%s' "$1" | tr '\t\n' '  '
}

fact() {
    printf '%s\t%s\n' "$1" "$(clean_field "$2")" >> "$FACTS_FILE"
}

check_add() {
    # name, command, result, reason, evidence
    printf '%s\t%s\t%s\t%s\t%s\n' \
        "$(clean_field "$1")" "$(clean_field "$2")" "$(clean_field "$3")" \
        "$(clean_field "$4")" "$(clean_field "$5")" >> "$CHECKS_FILE"
}

artifact_add() {
    # name, path, sha256, bytes, kind, reproducibility, evidence
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$(clean_field "$1")" "$(clean_field "$2")" "$(clean_field "$3")" \
        "$(clean_field "$4")" "$(clean_field "$5")" "$(clean_field "$6")" \
        "$(clean_field "$7")" >> "$ARTIFACTS_FILE"
}

sha256_of() {
    if [[ -f "$1" ]]; then
        sha256sum "$1" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "$1" | cut -d' ' -f1
    else
        echo "not-available"
    fi
}

# The identification of the revision is read out of the archive of the source.
#
# The name pattern `*/SOURCE-IDENTIFICATION.txt` is **not portable**: the
# option `--wildcards` is a GNU extension, and the `tar` of macOS refuses it
# with "Option --wildcards is not supported". The member is therefore looked up
# in the list of the archive and then read by its exact name — which every
# `tar` understands.
#
# The function returns 1 when the archive carries no identification. The caller
# records that as a failed check with a reason; a silent `set -e` abort would
# be a failure without a trace, and the gate of the project is not allowed to
# fail silently.
archive_identification() {
    local archive="$1"
    local member
    member="$(tar -tzf "$archive" 2>/dev/null \
        | grep -E '(^|/)SOURCE-IDENTIFICATION\.txt$' | head -n 1 || true)"
    if [[ -z "$member" ]]; then
        return 1
    fi
    tar -xzf "$archive" -O "$member" 2>/dev/null || return 1
}

version_line() {
    if command -v "$1" >/dev/null 2>&1; then
        "$1" --version 2>&1 | head -n 1
    else
        echo "not-installed"
    fi
}

source_digest() {
    # The digest of the sources of a path in the revision: the SHA-256 over
    # the blob hashes and the paths of `git ls-tree`. It is a property of the
    # revision alone — two candidates of the same revision have the same
    # digest, whatever the machine and whatever the moment of the build.
    git ls-tree -r "$REVISION" -- "$@" | awk '{print $3" "$4}' | sha256sum | cut -d' ' -f1
}

echo "==> The release candidate of $REVISION (Vocument $VERSION)"
echo "    It is internal: nothing is published, uploaded or tagged."

# --- 1. The state of the source ------------------------------------------
BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
HEAD_REVISION="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY_FILES="$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$DIRTY_FILES" == "0" ]]; then
    TREE_STATE="clean"
else
    TREE_STATE="dirty"
fi

fact "revision" "$REVISION"
fact "branch" "$BRANCH"
fact "revision_is_head" "$([[ "$REVISION" == "$HEAD_REVISION" ]] && echo "true" || echo "false")"
fact "working_tree.state" "$TREE_STATE"
fact "working_tree.uncommitted_entries" "$DIRTY_FILES"
fact "working_tree.note" "The candidate describes the revision. The artefacts that are built from the archive carry the sources of the revision; the working tree of the machine is not the source of the revision and is not part of the candidate."
fact "version" "$VERSION"

# --- 2. The source archive, built twice ----------------------------------
# The archive is the offer of the source that §6 of the AGPL asks for. It is
# built **twice** here: two archives of the same revision have to be bit for
# bit the same, otherwise the offer cannot be checked by the reader.
echo "==> Building the source archive of $REVISION (twice)..."
ARCHIVE_LOG="$OUTPUT_DIRECTORY/logs/source-archive.log"
set +e
bash scripts/release-source-archive.sh --revision "$REVISION" \
    --output "$OUTPUT_DIRECTORY/source" > "$ARCHIVE_LOG" 2>&1
FIRST_ARCHIVE_EXIT=$?
bash scripts/release-source-archive.sh --revision "$REVISION" \
    --output "$SECOND_ARCHIVE_DIRECTORY" >> "$ARCHIVE_LOG" 2>&1
SECOND_ARCHIVE_EXIT=$?
set -e

ARCHIVE_FILE="$(find "$OUTPUT_DIRECTORY/source" -name "*-source-${REVISION:0:12}.tar.gz" | head -n 1)"
SECOND_ARCHIVE_FILE="$(find "$SECOND_ARCHIVE_DIRECTORY" -name "*-source-${REVISION:0:12}.tar.gz" | head -n 1)"

ARCHIVE_SHA256="not-available"
ARCHIVE_SHA256_SECOND="not-available"
ARCHIVE_STATE="FAIL"
ARCHIVE_REASON="the source archive could not be built — the log names the reason"
ARCHIVE_EVIDENCE=""

if [[ "$FIRST_ARCHIVE_EXIT" -ne 0 || "$SECOND_ARCHIVE_EXIT" -ne 0 ]]; then
    ARCHIVE_REASON="the source archive could not be built (exit code $FIRST_ARCHIVE_EXIT / $SECOND_ARCHIVE_EXIT) — the log of the build is in logs/source-archive.log"
elif [[ -z "$ARCHIVE_FILE" || -z "$SECOND_ARCHIVE_FILE" ]]; then
    ARCHIVE_REASON="the source archive of $REVISION was not found after the build"
elif [[ ! -f "$ARCHIVE_FILE" ]]; then
    ARCHIVE_REASON="the source archive of $REVISION was not found in the candidate"
else
    ARCHIVE_SHA256="$(sha256_of "$ARCHIVE_FILE")"
    ARCHIVE_SHA256_SECOND="$(sha256_of "$SECOND_ARCHIVE_FILE")"
    if [[ "$ARCHIVE_SHA256" == "$ARCHIVE_SHA256_SECOND" ]]; then
        ARCHIVE_STATE="PASS"
        ARCHIVE_REASON=""
        ARCHIVE_EVIDENCE="two builds of the revision $REVISION, SHA-256 $ARCHIVE_SHA256 — bit for bit the same"
    else
        ARCHIVE_REASON="the two builds of the archive of $REVISION are not bit for bit the same ($ARCHIVE_SHA256 and $ARCHIVE_SHA256_SECOND) — the archive is not reproducible"
    fi
fi

fact "source_archive.sha256" "$ARCHIVE_SHA256"
fact "source_archive.sha256_second" "$ARCHIVE_SHA256_SECOND"

check_add "the source archive of the revision is built and reproducible" \
    "scripts/release-source-archive.sh --revision $REVISION" \
    "$ARCHIVE_STATE" "$ARCHIVE_REASON" "$ARCHIVE_EVIDENCE"

# The archive names the revision it carries — in the name of the file, in
# the header of the tar and in the identification inside it. A reader of the
# archive has to know which source it is, without a network.
if [[ -f "$ARCHIVE_FILE" ]]; then
    ARCHIVE_NAMED_REVISION="$(archive_identification "$ARCHIVE_FILE" \
        | sed -n 's/^Source revision: //p' | head -n 1 || true)"
    if [[ "$ARCHIVE_NAMED_REVISION" == "$REVISION" ]]; then
        check_add "the archive names the revision it carries" \
            "tar -tzf <archive> | tar -xzf <archive> -O <member>" \
            "PASS" "" "the identification of the archive names the revision $REVISION"
    elif [[ -z "$ARCHIVE_NAMED_REVISION" ]]; then
        check_add "the archive names the revision it carries" \
            "tar -tzf <archive> | tar -xzf <archive> -O <member>" \
            "FAIL" "the identification of the revision could not be read out of the archive" ""
    else
        check_add "the archive names the revision it carries" \
            "tar -tzf <archive> | tar -xzf <archive> -O <member>" \
            "FAIL" "the identification of the archive names '$ARCHIVE_NAMED_REVISION' and not $REVISION" ""
    fi
else
    check_add "the archive names the revision it carries" \
        "tar -tzf <archive> | tar -xzf <archive> -O <member>" \
        "NOT RUN" "the archive was not built — there is nothing to read" ""
fi

# --- 3. The build record -------------------------------------------------
# The record says what was built, from which revision, with which tools and
# with which results. It is the evidence of the run; the candidate carries
# a copy of it.
echo "==> Building the build record of $REVISION..."
BUILD_RECORD_LOG="$OUTPUT_DIRECTORY/logs/build-record.log"
BUILD_RECORD_ARGUMENTS=(--revision "$REVISION" --output "$OUTPUT_DIRECTORY/build-record")
if [[ -n "$TEST_LOG_ARGUMENT" ]]; then
    BUILD_RECORD_ARGUMENTS+=(--test-log "$TEST_LOG_ARGUMENT")
fi
if [[ -n "$TEST_EXIT_CODE_ARGUMENT" ]]; then
    BUILD_RECORD_ARGUMENTS+=(--test-exit-code "$TEST_EXIT_CODE_ARGUMENT")
fi
set +e
bash scripts/build-record.sh "${BUILD_RECORD_ARGUMENTS[@]}" > "$BUILD_RECORD_LOG" 2>&1
BUILD_RECORD_EXIT=$?
set -e

BUILD_RECORD_JSON="$OUTPUT_DIRECTORY/build-record/build-record.json"
if [[ "$BUILD_RECORD_EXIT" -eq 0 ]]; then
    check_add "the build record of the revision was written" \
        "scripts/build-record.sh --revision $REVISION" \
        "PASS" "" "the record is in build-record/build-record.json, the log is in logs/build-record.log"
else
    check_add "the build record of the revision was written" \
        "scripts/build-record.sh --revision $REVISION" \
        "FAIL" "the build record failed (exit code $BUILD_RECORD_EXIT) — the log is in logs/build-record.log" ""
fi

# The gates and the tests of the record are checks of the candidate too:
# they are read out of the record of this run, not guessed.
record_value() {
    # A value of the record, read with python: the record is JSON.
    python3 -c 'import json, sys
record = json.load(open(sys.argv[1], encoding="utf-8"))
node = record
for part in sys.argv[2].split("."):
    node = node.get(part) if isinstance(node, dict) else None
    if node is None:
        break
print(node if node is not None else "not-available")' "$BUILD_RECORD_JSON" "$1" 2>/dev/null \
        || echo "not-available"
}

if [[ -f "$BUILD_RECORD_JSON" ]]; then
    for gate in repository distribution licences; do
        GATE_STATE="$(record_value "gates.${gate}.state")"
        GATE_COMMAND="$(record_value "gates.${gate}.command")"
        if [[ "$GATE_STATE" == "PASS" ]]; then
            check_add "the gate '$gate' of the record" "$GATE_COMMAND" "PASS" "" \
                "the gate was run by the build record of this candidate"
        elif [[ "$GATE_STATE" == "FAIL" ]]; then
            check_add "the gate '$gate' of the record" "$GATE_COMMAND" "FAIL" \
                "the gate failed in the build record of this candidate" ""
        else
            check_add "the gate '$gate' of the record" "$GATE_COMMAND" "NOT RUN" \
                "the gate was not run by the build record of this candidate" ""
        fi
    done

    TESTS_STATE="$(record_value "tests.state")"
    TESTS_PASSED="$(record_value "tests.passed")"
    TESTS_FAILED="$(record_value "tests.failed")"
    TESTS_IGNORED="$(record_value "tests.ignored")"
    if [[ "$TESTS_STATE" == "PASS" ]]; then
        check_add "the tests of the workspace" "cargo test --workspace" "PASS" "" \
            "$TESTS_PASSED passed, $TESTS_FAILED failed, $TESTS_IGNORED ignored — read from the build record of this candidate"
    elif [[ "$TESTS_STATE" == "FAIL" ]]; then
        check_add "the tests of the workspace" "cargo test --workspace" "FAIL" \
            "the tests failed in the build record of this candidate" \
            "$TESTS_PASSED passed, $TESTS_FAILED failed"
    else
        check_add "the tests of the workspace" "cargo test --workspace" "NOT RUN" \
            "the tests were not run for the build record of this candidate" ""
    fi
else
    for gate in repository distribution licences; do
        check_add "the gate '$gate' of the record" "scripts/validate-repo.sh" "NOT RUN" \
            "the build record of this candidate was not written — there is no result to read" ""
    done
    check_add "the tests of the workspace" "cargo test --workspace" "NOT RUN" \
        "the build record of this candidate was not written — there is no result to read" ""
fi

# --- 4. The licences -----------------------------------------------------
# The licence of every component has to be known and has to agree with the
# AGPL-3.0 (E1), and the notices of the third-party components have to
# travel with the product. Both are measured by their own script.
bash scripts/release-candidate-licences.sh "$CHECKS_FILE" "$OUTPUT_DIRECTORY"

# --- 5. The Rust library -------------------------------------------------
# The library is built **from the unpacked archive**, not from the working
# tree: the working tree of the machine is not the source of the revision,
# and a binary that is built from it does not correspond to the archive.
#
# The library is **not** claimed to be bit-identical across two builds: the
# comparison of two independent builds is a separate measurement
# (`scripts/verify-source-archive.sh`), and this candidate does not make it.
echo "==> Building the Rust library from the unpacked archive..."
LIBRARY_DIGEST="not-available"
LIBRARY_STATE="NOT RUN"
LIBRARY_REASON=""
LIBRARY_EVIDENCE=""
MUPDF_LIBRARY=""
TESSERACT_LIBRARY=""
# The name of the shared library differs between the platforms: on Linux the
# linker uses `libmupdf.so` (the version is part of the name of the file),
# and on macOS the library is `libmupdf.dylib`. The candidate names the
# library that is actually linked — a candidate that reports a missing
# library that exists says the environment is broken when it is not.
for library_candidate in "$MUPDF_DIR/lib/libmupdf.so" "$MUPDF_DIR/lib/libmupdf.dylib"; do
    if [[ -f "$library_candidate" ]]; then
        MUPDF_LIBRARY="$library_candidate"
        break
    fi
done
for library_candidate in "$TESSERACT_DIR/lib/libtesseract.so" "$TESSERACT_DIR/lib/libtesseract.dylib"; do
    if [[ -f "$library_candidate" ]]; then
        TESSERACT_LIBRARY="$library_candidate"
        break
    fi
done
LIBRARY_FILE=""

if [[ "$SKIP_LIBRARY" -eq 1 ]]; then
    LIBRARY_REASON="the build of the library was skipped by the environment (--skip-library)"
elif [[ ! -f "$ARCHIVE_FILE" ]]; then
    LIBRARY_REASON="the source archive was not built — there are no sources to build from"
elif [[ ! -f "$MUPDF_LIBRARY" ]] || [[ ! -f "$TESSERACT_LIBRARY" ]]; then
    MISSING_LIBRARIES=""
    [[ -f "$MUPDF_LIBRARY" ]] || MISSING_LIBRARIES="MuPDF ($MUPDF_LIBRARY)"
    [[ -f "$TESSERACT_LIBRARY" ]] || MISSING_LIBRARIES="${MISSING_LIBRARIES:+$MISSING_LIBRARIES, }Tesseract ($TESSERACT_LIBRARY)"
    LIBRARY_REASON="the toolchain of the link is not complete in this environment: $MISSING_LIBRARIES. MuPDF and Tesseract are third-party components that are not part of the source of the product — they are built from their pinned sources"
else
    BUILD_ROOT="$OUTPUT_DIRECTORY/build/${REVISION:0:12}"
    UNPACK_DIRECTORY="$BUILD_ROOT/source"
    CARGO_TARGET_DIRECTORY="$BUILD_ROOT/target"
    rm -rf "$UNPACK_DIRECTORY"
    mkdir -p "$UNPACK_DIRECTORY" "$CARGO_TARGET_DIRECTORY"
    tar xzf "$ARCHIVE_FILE" -C "$UNPACK_DIRECTORY"
    UNPACKED_ROOT="$(find "$UNPACK_DIRECTORY" -maxdepth 1 -mindepth 1 -type d | head -n 1)"

    export TESSDATA_PREFIX="${TESSDATA_PREFIX:-$REPO_ROOT/apps/android/app/src/main/assets/tessdata}"
    # The archive has no `.git`: the revision of the source is given from the
    # outside, exactly as the release scripts do it, so that the library
    # names the revision of the archive.
    export VOCUMENT_SOURCE_REVISION="$REVISION"
    export VOCUMENT_SOURCE_TREE_CLEAN="true"

    LIBRARY_LOG="$OUTPUT_DIRECTORY/logs/rust-library.log"
    set +e
    (
        cd "$UNPACKED_ROOT"
        CARGO_TARGET_DIR="$CARGO_TARGET_DIRECTORY" cargo build --release --offline --workspace
    ) > "$LIBRARY_LOG" 2>&1
    LIBRARY_EXIT=$?
    if [[ "$LIBRARY_EXIT" -ne 0 ]]; then
        (
            cd "$UNPACKED_ROOT"
            CARGO_TARGET_DIR="$CARGO_TARGET_DIRECTORY" cargo build --release --workspace
        ) >> "$LIBRARY_LOG" 2>&1
        LIBRARY_EXIT=$?
    fi
    set -e

    LIBRARY_FILE="$CARGO_TARGET_DIRECTORY/release/libvocument_ffi.so"
    if [[ "$LIBRARY_EXIT" -eq 0 && -f "$LIBRARY_FILE" ]]; then
        LIBRARY_DIGEST="$(sha256_of "$LIBRARY_FILE")"
        LIBRARY_STATE="PASS"
        LIBRARY_EVIDENCE="the library was built from the unpacked archive of $REVISION, SHA-256 $LIBRARY_DIGEST"
    else
        LIBRARY_STATE="FAIL"
        LIBRARY_REASON="the library could not be built from the archive of $REVISION (exit code $LIBRARY_EXIT) — the log is in logs/rust-library.log"
    fi
fi

fact "library.state" "$LIBRARY_STATE"
fact "library.sha256" "$LIBRARY_DIGEST"

check_add "the Rust library is built from the sources of the revision" \
    "cargo build --release --workspace (in the unpacked archive)" \
    "$LIBRARY_STATE" "$LIBRARY_REASON" "$LIBRARY_EVIDENCE"

# The library is built — but it is **not** claimed to be bit-identical to
# the library of another build. The candidate says so, and names what is
# equal instead.
check_add "the Rust library is bit-identical across two builds" \
    "scripts/verify-source-archive.sh" \
    "NOT RUN" \
    "the comparison of two independent builds is not part of this candidate: the library depends on the versions of the tools, on the MuPDF and the Tesseract that are linked and on the paths of the build machine" \
    "what is equal is named in the manifest: the checksum of the sources, the version and the checks"

# --- 6. The application targets -----------------------------------------
# The three targets of the product, the legal clearance and the publication
# are measured by their own script.
bash scripts/release-candidate-targets.sh "$CHECKS_FILE"

# --- 7. The artefacts ----------------------------------------------------
# Every artefact of the candidate with its checksum. The checksum is
# measured here and now.
add_artifact() {
    # name, path, kind, reproducibility, evidence
    local name="$1" path="$2" kind="$3" reproducibility="$4" evidence="$5"
    local digest bytes
    digest="$(sha256_of "$path")"
    bytes="$(wc -c < "$path" | tr -d ' ')"
    artifact_add "$name" "${path#"$REPO_ROOT"/}" "$digest" "$bytes" \
        "$kind" "$reproducibility" "$evidence"
}

if [[ -f "$ARCHIVE_FILE" ]]; then
    add_artifact "The archive of the source" "$ARCHIVE_FILE" "source-archive" \
        "bit-identical" "two builds of the revision, the same SHA-256 — measured in this run"
    if [[ -f "$ARCHIVE_FILE.sha256" ]]; then
        add_artifact "The checksum of the archive" "$ARCHIVE_FILE.sha256" "checksum" \
            "bit-identical" "the checksum of the archive, written by the build of the archive"
    fi
fi

if [[ -f "$BUILD_RECORD_JSON" ]]; then
    add_artifact "The build record" "$BUILD_RECORD_JSON" "build-record" \
        "not-claimed" "the record of the run — the content depends on the machine and the moment of the run"
fi
if [[ -f "$OUTPUT_DIRECTORY/build-record/build-record.md" ]]; then
    add_artifact "The build record, readable" "$OUTPUT_DIRECTORY/build-record/build-record.md" \
        "build-record" "not-claimed" "the same record, readable"
fi
if [[ -f "$OUTPUT_DIRECTORY/licences/licence-inventory.md" ]]; then
    add_artifact "The inventory of the licences" "$OUTPUT_DIRECTORY/licences/licence-inventory.md" \
        "licence" "not-claimed" "the inventory that the audit produced in this run"
fi
CANDIDATE_NOTICES="$OUTPUT_DIRECTORY/licences/THIRD-PARTY-NOTICES.txt"
if [[ -f "$CANDIDATE_NOTICES" ]]; then
    add_artifact "The notices of the third-party components" "$CANDIDATE_NOTICES" \
        "licence" "not-claimed" "the texts of the licences that travel with the product"
fi

LIBRARY_ARTEFACTS=(
    "libvocument_ffi.so"
    "libvocument_ffi.a"
    "libvocument_core.rlib"
    "libvocument_pdf_mupdf.rlib"
    "libvocument_ocr.rlib"
    "libvocument_imaging.rlib"
    "libvocument_pdoc.rlib"
)
if [[ -n "$LIBRARY_FILE" ]]; then
    for artefact in "${LIBRARY_ARTEFACTS[@]}"; do
        CANDIDATE_ARTEFACT="$(dirname "$LIBRARY_FILE")/$artefact"
        if [[ -f "$CANDIDATE_ARTEFACT" ]]; then
            add_artifact "The Rust library: $artefact" "$CANDIDATE_ARTEFACT" \
                "rust-library" "not-claimed" \
                "built from the sources of the revision — not proven to be bit-identical across two builds"
        fi
    done
fi

# --- 8. The tools of the run ---------------------------------------------
MUPDF_VERSION="not-available"
if [[ -f "$MUPDF_DIR/include/mupdf/fitz/version.h" ]]; then
    MUPDF_VERSION="$(sed -n 's/.*#define FZ_VERSION "\([^"]*\)".*/\1/p' "$MUPDF_DIR/include/mupdf/fitz/version.h" | head -n 1)"
fi
TESSERACT_VERSION="not-available"
if [[ -f "$TESSERACT_DIR/include/tesseract/version.h" ]]; then
    TESSERACT_VERSION="$(sed -n 's/.*#define TESSERACT_VERSION_STR "\([^"]*\)".*/\1/p' "$TESSERACT_DIR/include/tesseract/version.h" | head -n 1)"
fi
UNIFFI_VERSION="$(python3 - <<'PYTHON'
import re
text = open("Cargo.lock", encoding="utf-8").read()
match = re.search(r'name = "uniffi"\nversion = "([^"]+)"', text)
print(match.group(1) if match else "unknown")
PYTHON
)"

fact "tools.rustc" "$(version_line rustc)"
fact "tools.cargo" "$(version_line cargo)"
fact "tools.rustfmt" "$(version_line rustfmt)"
fact "tools.clippy" "$(cargo clippy --version 2>/dev/null | head -n 1 || echo "not-installed")"
fact "tools.uniffi" "$UNIFFI_VERSION"
fact "tools.mupdf" "$MUPDF_VERSION"
fact "tools.tesseract" "$TESSERACT_VERSION"
fact "tools.swift" "$(version_line swift)"
fact "tools.java" "$(version_line java)"
fact "tools.android_sdk" "$([[ -d "${ANDROID_HOME:-$REPO_ROOT/target/android-sdk}" ]] && echo "present" || echo "not-present")"
fact "tools.host" "$(uname -srm)"

# The digest of the sources of every target: it is a property of the
# revision, and it is what is equal between two candidates of the same
# revision — the binaries themselves are not claimed to be equal.
fact "source_digest.rust" "$(source_digest core engines platform formats Cargo.toml Cargo.lock)"
fact "source_digest.macos" "$(source_digest apps/macos)"
fact "source_digest.android" "$(source_digest apps/android)"

# --- 9. The manifest -----------------------------------------------------
# The manifest and the readable summary are written from the same facts, so
# that they cannot say different things.
fact "generated_at" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
python3 scripts/release-candidate-manifest.py \
    --facts "$FACTS_FILE" --checks "$CHECKS_FILE" --artifacts "$ARTIFACTS_FILE" \
    --output "$OUTPUT_DIRECTORY"

# --- 10. The summary of the run ------------------------------------------
echo
echo "==> The release candidate is written:"
echo "    $OUTPUT_DIRECTORY/manifest.json"
echo "    $OUTPUT_DIRECTORY/MANIFEST.md"
echo
echo "==> The checks of the candidate:"
while IFS=$'\t' read -r name command result reason evidence; do
    [[ -z "$name" ]] && continue
    printf '    %-9s %s\n' "$result" "$name"
    if [[ "$result" != "PASS" && -n "$reason" ]]; then
        printf '              %s\n' "$reason"
    fi
done < "$CHECKS_FILE"

if grep -q $'\tFAIL\t' "$CHECKS_FILE"; then
    echo
    echo "ERROR: a check of the candidate failed — the candidate is not usable." >&2
    exit 1
fi

echo
echo "==> The candidate is internal: it is not published, and E5 is missing."

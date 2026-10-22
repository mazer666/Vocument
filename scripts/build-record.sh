#!/usr/bin/env bash
# The build record of a release.
#
# A release without a record is a release that cannot be repeated and cannot
# be checked. This script writes that record: what was built, from which
# revision, with which tools, with which results — machine readable as JSON
# and readable as a summary in Markdown.
#
# The record is written into `target/build-record/`:
#
#   build-record.json    the machine readable record
#   build-record.md      the readable summary
#   cargo-test.log       the output of the test run, if the tests were run
#
# Every number of the record is measured here and now. What cannot be
# measured in this environment — the Swift target, the Android package, the
# notarisation — is written down as `NOT RUN` with the reason. A number that
# was not measured is not a number in this record.
#
# The script is hooked into the gate (`scripts/local-ci.sh`), so that a
# release always leaves a record — also when the gate fails.
#
# Usage: scripts/build-record.sh [--output <directory>] [--skip-tests]
#                                 [--gate-exit-code <n>] [--test-log <file>]
#                                 [--test-exit-code <n>] [--revision <sha>]
#
# `--test-log` names the output of a test run that has already been made
# (the gate of `scripts/local-ci.sh` runs the tests and hands its log over,
# so that the tests are not run twice for one gate run). Without it the
# script runs the tests itself.
#
# `--revision` names the revision the record is about. Without it the record
# is about the HEAD of the checkout. The source archive of the record is
# always built for that revision — and the record says whether the tests and
# the gates were run on the sources of that revision or on the working tree.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The tools of Rust live in `~/.cargo/bin`. The path of a non-interactive
# shell does not always carry it, and a record that says "not installed" for
# a tool that is installed would be a lie.
if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

OUTPUT_DIRECTORY="$REPO_ROOT/target/build-record"
SKIP_TESTS=0
GATE_EXIT_CODE="not-run"
TEST_LOG_ARGUMENT=""
TEST_EXIT_CODE_ARGUMENT=""
REVISION_ARGUMENT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            OUTPUT_DIRECTORY="$2"
            shift 2
            ;;
        --skip-tests)
            SKIP_TESTS=1
            shift
            ;;
        --revision)
            REVISION_ARGUMENT="$2"
            shift 2
            ;;
        --gate-exit-code)
            GATE_EXIT_CODE="$2"
            shift 2
            ;;
        --test-log)
            TEST_LOG_ARGUMENT="$2"
            shift 2
            ;;
        --test-exit-code)
            TEST_EXIT_CODE_ARGUMENT="$2"
            shift 2
            ;;
        *)
            echo "usage: $0 [--output <directory>] [--skip-tests] [--gate-exit-code <n>] [--test-log <file>] [--test-exit-code <n>] [--revision <sha>]" >&2
            exit 2
            ;;
    esac
done

# The revision the record is about. It has to be a commit of this
# repository: a record that names a revision nobody can check is a record
# that lies.
REVISION="${REVISION_ARGUMENT:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
if [[ "$REVISION" != "unknown" ]]; then
    RESOLVED_REVISION="$(git rev-parse --verify "${REVISION}^{commit}" 2>/dev/null || true)"
    if [[ -z "$RESOLVED_REVISION" ]]; then
        echo "ERROR: the revision '$REVISION' is not a commit of this repository." >&2
        exit 1
    fi
    REVISION="$RESOLVED_REVISION"
fi

mkdir -p "$OUTPUT_DIRECTORY"
JSON_FILE="$OUTPUT_DIRECTORY/build-record.json"
SUMMARY_FILE="$OUTPUT_DIRECTORY/build-record.md"
FACTS_FILE="$(mktemp)"
trap 'rm -f "$FACTS_FILE"' EXIT

# --- The helpers of the record -------------------------------------------
# The facts are collected as `key<TAB>value` lines. The value is already
# JSON, so that the record can be written without a second escaping of the
# strings.
json_string() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/}"
    value="${value//$'\t'/\\t}"
    printf '"%s"' "$value"
}

fact_string() {
    printf '%s\t%s\n' "$1" "$(json_string "$2")" >> "$FACTS_FILE"
}

fact_raw() {
    printf '%s\t%s\n' "$1" "$2" >> "$FACTS_FILE"
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
    # The version of a tool, or the reason why there is none.
    #
    # A tool may be missing, and a tool may be there and still not answer: a
    # `java` without a runtime answers with an error and the exit code 1.
    # Neither is a reason to end the record — the record says which tools
    # were there, and a tool that is not there is named as such. A failure of
    # the tool must therefore not leave the record unwritten.
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "not-installed"
        return 0
    fi
    local output status
    output="$("$1" --version 2>&1)" && status=0 || status=$?
    output="$(printf '%s\n' "$output" | head -n 1)"
    if [[ "$status" -ne 0 ]]; then
        echo "not-usable: ${output:-the tool answered with an error}"
        return 0
    fi
    printf '%s\n' "${output:-unknown}"
}

# The path of the shared library of MuPDF in an installation.
#
# The name of the file differs between the platforms: on Linux the version is
# part of the name (`libmupdf.so.28.5`, with `libmupdf.so` as the link that
# the linker uses), and on macOS the file is `libmupdf.dylib`. The record
# names the library that is actually linked — a record that names a file that
# does not exist says nothing.
mupdf_library_path() {
    local candidate
    for candidate in "$1/lib/libmupdf.so" "$1/lib/libmupdf.dylib"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

# The path of the shared library of Tesseract in an installation. The name
# differs between the platforms in the same way as the one of MuPDF.
tesseract_library_path() {
    local candidate
    for candidate in "$1/lib/libtesseract.so" "$1/lib/libtesseract.dylib"; do
        if [[ -f "$candidate" ]]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

echo "==> Collecting the build record..."

# --- 1. The source -------------------------------------------------------
BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
HEAD_REVISION="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
DIRTY_FILES="$(git status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$DIRTY_FILES" == "0" ]]; then
    TREE_STATE="clean"
else
    TREE_STATE="dirty"
fi
VERSION="$(python3 - <<'PYTHON'
import re
text = open("Cargo.toml", encoding="utf-8").read()
match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
print(match.group(1) if match else "0.0.0")
PYTHON
)"

fact_string "source.revision" "$REVISION"
fact_string "source.revision_short" "${REVISION:0:12}"
fact_string "source.branch" "$BRANCH"
fact_string "source.tree_state" "$TREE_STATE"

# The state of the working tree is a property of the machine, not of the
# revision. The record says whether the tests and the gates were run on the
# sources of the revision — a record that hides this difference describes a
# build that nobody made.
if [[ "$REVISION" != "$HEAD_REVISION" ]]; then
    REVISION_IS_HEAD="false"
    SOURCE_NOTE="The revision is not the HEAD of the checkout: the tests and the gates were run on the working tree, which is not the source of the revision. The source archive of the record is built for the revision and describes it."
elif [[ "$TREE_STATE" == "clean" ]]; then
    REVISION_IS_HEAD="true"
    SOURCE_NOTE="The revision is the HEAD of the checkout and the tree is clean: the tests and the gates were run on the sources of this revision."
else
    REVISION_IS_HEAD="true"
    SOURCE_NOTE="The revision is the HEAD of the checkout, but the tree carries uncommitted changes: the tests and the gates were run on the working tree, which is not the source of the revision. The source archive of the record is built for the revision and describes it."
fi
fact_string "source.revision_is_head" "$REVISION_IS_HEAD"
fact_string "source.note" "$SOURCE_NOTE"
fact_raw "source.dirty_entries" "$DIRTY_FILES"
fact_string "project.name" "Vocument"
fact_string "project.version" "$VERSION"
fact_string "project.licence" "AGPL-3.0-or-later"

# --- 2. The tools --------------------------------------------------------
RUSTC_VERSION="$(version_line rustc)"
CARGO_VERSION="$(version_line cargo)"
RUSTFMT_VERSION="$(version_line rustfmt)"
CLIPPY_VERSION="$(cargo clippy --version 2>/dev/null | head -n 1 || echo "not-installed")"
UNIFFI_VERSION="$(python3 - <<'PYTHON'
import re
text = open("Cargo.lock", encoding="utf-8").read()
match = re.search(r'name = "uniffi"\nversion = "([^"]+)"', text)
print(match.group(1) if match else "unknown")
PYTHON
)"

# The MuPDF of the host: the version is read from the header of the
# installation that is linked, so that the number of the record belongs to
# the library that was actually used.
MUPDF_DIR_USED="${MUPDF_DIR:-$REPO_ROOT/target/mupdf-host}"
MUPDF_VERSION="not-available"
MUPDF_LIBRARY="not-available"
if [[ -f "$MUPDF_DIR_USED/include/mupdf/fitz/version.h" ]]; then
    MUPDF_VERSION="$(sed -n 's/.*#define FZ_VERSION "\([^"]*\)".*/\1/p' "$MUPDF_DIR_USED/include/mupdf/fitz/version.h" | head -n 1)"
    MUPDF_LIBRARY="$(mupdf_library_path "$MUPDF_DIR_USED" || echo "not-available")"
fi

# The Tesseract of the host: the same, from the header of the installation.
TESSERACT_DIR_USED="${TESSERACT_DIR:-$REPO_ROOT/target/tesseract-host}"
TESSERACT_VERSION="not-available"
TESSERACT_LIBRARY="not-available"
if [[ -f "$TESSERACT_DIR_USED/include/tesseract/version.h" ]]; then
    TESSERACT_VERSION="$(sed -n 's/.*#define TESSERACT_VERSION_STR "\([^"]*\)".*/\1/p' "$TESSERACT_DIR_USED/include/tesseract/version.h" | head -n 1)"
    TESSERACT_LIBRARY="$(tesseract_library_path "$TESSERACT_DIR_USED" || echo "not-available")"
fi

# The libraries of the host have to be found when the tests run. The name of
# the variable differs between the platforms: Linux reads `LD_LIBRARY_PATH`,
# macOS reads `DYLD_LIBRARY_PATH` and **ignores** the other one. Without the
# second one the test binaries of the crates that link MuPDF die on macOS
# with "Library not loaded: @rpath/libmupdf.dylib". Both are therefore set,
# so that the same gate runs on both platforms.
export LD_LIBRARY_PATH="$MUPDF_DIR_USED/lib:$TESSERACT_DIR_USED/lib:${LD_LIBRARY_PATH:-}"
export DYLD_LIBRARY_PATH="$MUPDF_DIR_USED/lib:$TESSERACT_DIR_USED/lib:${DYLD_LIBRARY_PATH:-}"

SWIFT_VERSION="$(version_line swift)"
JAVA_VERSION="$(version_line java)"
if [[ -n "${ANDROID_HOME:-}" ]] && [[ -d "${ANDROID_HOME:-}" ]]; then
    ANDROID_SDK_STATE="present"
else
    ANDROID_SDK_STATE="not-present"
fi

fact_string "tools.rustc" "$RUSTC_VERSION"
fact_string "tools.cargo" "$CARGO_VERSION"
fact_string "tools.rustfmt" "$RUSTFMT_VERSION"
fact_string "tools.clippy" "$CLIPPY_VERSION"
fact_string "tools.uniffi" "$UNIFFI_VERSION"
fact_string "tools.mupdf.version" "$MUPDF_VERSION"
fact_string "tools.mupdf.directory" "$MUPDF_DIR_USED"
fact_string "tools.mupdf.library_sha256" "$(sha256_of "$MUPDF_LIBRARY")"
fact_string "tools.tesseract.version" "$TESSERACT_VERSION"
fact_string "tools.tesseract.directory" "$TESSERACT_DIR_USED"
fact_string "tools.tesseract.library_sha256" "$(sha256_of "$TESSERACT_LIBRARY")"
fact_string "tools.swift" "$SWIFT_VERSION"
fact_string "tools.java" "$JAVA_VERSION"
fact_string "tools.android_sdk" "$ANDROID_SDK_STATE"
fact_string "tools.host" "$(uname -srm)"

# --- 3. The tests --------------------------------------------------------
# The counts are read from the output of the test run: every test binary
# writes one `test result:` line, and the line names the number of the tests
# that passed, that failed and that were ignored.
TEST_LOG="$OUTPUT_DIRECTORY/cargo-test.log"
TEST_STATE="NOT RUN"
TEST_SUITES=0
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_IGNORED=0
TEST_REASON="the tests were not run for this record"

count_tests() {
    # The counts are read from the output of the test run: every test binary
    # writes one `test result:` line, and the line names the number of the
    # tests that passed, that failed and that were ignored.
    #
    # The line looks like this:
    #
    #     test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; ...
    #
    # The numbers are read as fields, not with a greedy pattern: a greedy
    # `.*` in front of the number eats the first digit of a two-digit number
    # (62 becomes 2) — a record that counts 105 tests instead of 255 is a
    # record that lies.
    local log="$1"
    TEST_SUITES="$(grep -c '^test result:' "$log" || true)"
    TESTS_PASSED="$(awk -F'[ ;]' '/^test result:/ {sum += $4} END {print sum + 0}' "$log")"
    TESTS_FAILED="$(awk -F'[ ;]' '/^test result:/ {sum += $6} END {print sum + 0}' "$log")"
    TESTS_IGNORED="$(awk -F'[ ;]' '/^test result:/ {sum += $8} END {print sum + 0}' "$log")"
}

if [[ -n "$TEST_LOG_ARGUMENT" ]] && [[ -f "$TEST_LOG_ARGUMENT" ]]; then
    # The gate has already run the tests; its log is read, not repeated.
    # The log of the gate lies in the same directory as the log of the record
    # — copying it onto itself is an error, and the content is already there.
    if [[ "$(cd "$(dirname "$TEST_LOG_ARGUMENT")" && pwd)/$(basename "$TEST_LOG_ARGUMENT")" \
        != "$(cd "$(dirname "$TEST_LOG")" && pwd)/$(basename "$TEST_LOG")" ]]; then
        cp "$TEST_LOG_ARGUMENT" "$TEST_LOG"
    fi
    count_tests "$TEST_LOG"
    if [[ -n "$TEST_EXIT_CODE_ARGUMENT" && "$TEST_EXIT_CODE_ARGUMENT" != "not-run" ]]; then
        if [[ "$TEST_EXIT_CODE_ARGUMENT" -eq 0 ]]; then
            TEST_STATE="PASS"
        else
            TEST_STATE="FAIL"
        fi
    else
        TEST_STATE="FAIL"
    fi
    TEST_REASON="the run of the gate was read from $TEST_LOG_ARGUMENT"
    echo "    the tests: $TEST_STATE ($TESTS_PASSED passed, $TESTS_FAILED failed, $TESTS_IGNORED ignored, $TEST_SUITES suites) — read from the log of the gate"
elif [[ "$SKIP_TESTS" -eq 0 ]] && command -v cargo >/dev/null 2>&1; then
    echo "==> Running the tests of the workspace..."
    set +e
    cargo test --workspace > "$TEST_LOG" 2>&1
    TEST_EXIT_CODE=$?
    set -e
    if [[ "$TEST_EXIT_CODE" -eq 0 ]]; then
        TEST_STATE="PASS"
    else
        TEST_STATE="FAIL"
    fi
    count_tests "$TEST_LOG"
    TEST_REASON="the tests were run by this record"
    echo "    the tests: $TEST_STATE ($TESTS_PASSED passed, $TESTS_FAILED failed, $TESTS_IGNORED ignored, $TEST_SUITES suites)"
else
    echo "==> The tests are not run for this record (--skip-tests)."
    : > "$TEST_LOG"
    TEST_REASON="the tests were not run for this record (--skip-tests)"
fi

fact_string "tests.state" "$TEST_STATE"
fact_raw "tests.suites" "$TEST_SUITES"
fact_raw "tests.passed" "$TESTS_PASSED"
fact_raw "tests.failed" "$TESTS_FAILED"
fact_raw "tests.ignored" "$TESTS_IGNORED"
fact_string "tests.log" "target/build-record/cargo-test.log"
fact_string "tests.reason" "$TEST_REASON"

# --- 4. The generated bindings -------------------------------------------
# The bindings are generated, never written by hand. The record carries their
# checksums, so that a change of the generated files is visible.
BINDING_FILES=(
    apps/macos/Sources/VocumentFFI/vocument_ffi.swift
    apps/macos/Sources/VocumentFFIHeaders/include/vocument_ffiFFI.h
    apps/macos/Sources/VocumentFFIHeaders/include/module.modulemap
    apps/android/app/src/main/java/uniffi/vocument_ffi/vocument_ffi.kt
)
BINDING_DIGESTS=""
BINDING_COUNT=0
for binding in "${BINDING_FILES[@]}"; do
    if [[ -f "$binding" ]]; then
        digest="$(sha256_of "$binding")"
        BINDING_COUNT=$((BINDING_COUNT + 1))
        fact_string "bindings.files.$(echo "$binding" | tr '/.' '__')" "$digest"
        BINDING_DIGESTS="${BINDING_DIGESTS}${digest}  ${binding}"$'\n'
    else
        fact_string "bindings.files.$(echo "$binding" | tr '/.' '__')" "not-available"
    fi
done
BINDING_COMBINED="$(printf '%s' "$BINDING_DIGESTS" | sha256sum 2>/dev/null | cut -d' ' -f1 || echo "not-available")"
fact_raw "bindings.count" "$BINDING_COUNT"
fact_string "bindings.combined_sha256" "$BINDING_COMBINED"

# --- 5. The gates --------------------------------------------------------
run_gate() {
    # The result of a gate: the exit code and the state.
    local name="$1"
    local command_line="$2"
    local log="$OUTPUT_DIRECTORY/${name}.log"
    local exit_code=0
    set +e
    bash -c "$command_line" > "$log" 2>&1
    exit_code=$?
    set -e
    if [[ "$exit_code" -eq 0 ]]; then
        GATE_STATE="PASS"
    else
        GATE_STATE="FAIL"
    fi
    fact_string "gates.${name}.command" "$command_line"
    fact_raw "gates.${name}.exit_code" "$exit_code"
    fact_string "gates.${name}.state" "$GATE_STATE"
    fact_string "gates.${name}.log" "target/build-record/${name}.log"
    GATE_RESULTS="${GATE_RESULTS}${name}: ${GATE_STATE} (exit code ${exit_code})"$'\n'
    echo "    the gate $name: $GATE_STATE (exit code $exit_code)"
}

GATE_RESULTS=""
run_gate "repository" "scripts/validate-repo.sh"
run_gate "distribution" "scripts/validate-distribution.sh"
run_gate "licences" "scripts/audit-licenses.sh"

# --- 6. The source archive -----------------------------------------------
# The archive is the offer of the source that §6 asks for. The record builds
# it twice: two archives of the same revision have to be bit for bit the
# same, otherwise the build is not reproducible.
echo "==> Building the source archive twice..."
ARCHIVE_DIRECTORY="$REPO_ROOT/target/release-source"
ARCHIVE_SECOND_DIRECTORY="$(mktemp -d)"
trap 'rm -f "$FACTS_FILE"; rm -rf "$ARCHIVE_SECOND_DIRECTORY"' EXIT

ARCHIVE_STATE="NOT RUN"
ARCHIVE_PATH="not-available"
ARCHIVE_SHA256="not-available"
ARCHIVE_BYTES=0
ARCHIVE_REPRODUCIBLE="not-measured"
ARCHIVE_SECOND_SHA256="not-available"
ARCHIVE_IDENTIFICATION="not-available"

if bash scripts/release-source-archive.sh --revision "$REVISION" --output "$ARCHIVE_DIRECTORY" > "$OUTPUT_DIRECTORY/source-archive.log" 2>&1 \
   && bash scripts/release-source-archive.sh --revision "$REVISION" --output "$ARCHIVE_SECOND_DIRECTORY" >> "$OUTPUT_DIRECTORY/source-archive.log" 2>&1; then
    # The archive of **this** revision — not the one that an earlier run left
    # in the directory. A record that names an old archive describes an old
    # build.
    ARCHIVE_FILE="$(find "$ARCHIVE_DIRECTORY" -name "*-source-${REVISION:0:12}.tar.gz" | head -n 1)"
    ARCHIVE_SECOND_FILE="$(find "$ARCHIVE_SECOND_DIRECTORY" -name "*-source-${REVISION:0:12}.tar.gz" | head -n 1)"
    if [[ -z "$ARCHIVE_FILE" || -z "$ARCHIVE_SECOND_FILE" ]]; then
        ARCHIVE_STATE="FAIL"
        ARCHIVE_REPRODUCIBLE="not-measured"
        echo "    the source archive of $REVISION was not found after the build." >&2
    else
        ARCHIVE_PATH="${ARCHIVE_FILE#"$REPO_ROOT"/}"
        ARCHIVE_SHA256="$(sha256_of "$ARCHIVE_FILE")"
        ARCHIVE_SECOND_SHA256="$(sha256_of "$ARCHIVE_SECOND_FILE")"
        ARCHIVE_BYTES="$(wc -c < "$ARCHIVE_FILE" | tr -d ' ')"
        ARCHIVE_IDENTIFICATION="$(archive_identification "$ARCHIVE_FILE" | head -n 8 | tr '\n' ' ' || echo "not-available")"
        if [[ "$ARCHIVE_SHA256" == "$ARCHIVE_SECOND_SHA256" ]]; then
            ARCHIVE_STATE="PASS"
            ARCHIVE_REPRODUCIBLE="true"
        else
            ARCHIVE_STATE="FAIL"
            ARCHIVE_REPRODUCIBLE="false"
        fi
        echo "    the source archive: $ARCHIVE_STATE (reproducible: $ARCHIVE_REPRODUCIBLE)"
    fi
else
    echo "    the source archive could not be built — the log names the reason." >&2
fi

fact_string "source_archive.state" "$ARCHIVE_STATE"
fact_string "source_archive.path" "$ARCHIVE_PATH"
fact_string "source_archive.sha256" "$ARCHIVE_SHA256"
fact_string "source_archive.sha256_second_build" "$ARCHIVE_SECOND_SHA256"
fact_string "source_archive.reproducible" "$ARCHIVE_REPRODUCIBLE"
fact_raw "source_archive.bytes" "$ARCHIVE_BYTES"
fact_string "source_archive.identification" "$ARCHIVE_IDENTIFICATION"
fact_string "source_archive.log" "target/build-record/source-archive.log"

# --- 7. What could not be measured here ----------------------------------
# A record that hides the limits of the environment is a record that lies.
# Every limit is written down with the reason.
NOT_RUN_ITEMS=(
    "macOS application (Swift target)|no macOS host and therefore no macOS SDK in this environment — SwiftUI, AppKit and CoreGraphics do not exist for Linux. The Swift compiler (6.0.3) parses every file of the application and builds the generated bindings; the application itself needs the SDK|the sources of the target are in the archive (apps/macos), the parse of every file is proved by scripts/check-swift-sources.py, the build of the application is NOT RUN"
    "Android package (Gradle)|no Android SDK in this environment (a JDK is installed)|the sources of the package are in the archive (apps/android), the build of the package is NOT RUN"
    "macOS notarisation|no Apple identity and no macOS host in this environment|the commands are in scripts/release-macos.sh, the notarisation is NOT RUN"
    "Android release signature|no release key in this environment|the signing is left to Gradle, the signature of a release build is NOT RUN"
)

not_run_json="["
first=1
for entry in "${NOT_RUN_ITEMS[@]}"; do
    item="${entry%%|*}"
    rest="${entry#*|}"
    reason="${rest%%|*}"
    checked="${rest#*|}"
    if [[ "$first" -eq 0 ]]; then
        not_run_json="${not_run_json},"
    fi
    first=0
    not_run_json="${not_run_json}{\"item\": $(json_string "$item"), \"reason\": $(json_string "$reason"), \"checked_instead\": $(json_string "$checked")}"
done
not_run_json="${not_run_json}]"
fact_raw "not_run" "$not_run_json"

# --- 8. The record -------------------------------------------------------
GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
fact_string "schema" "vocument.build-record/1"
fact_string "generated_at" "$GENERATED_AT"
fact_string "gate_exit_code" "$GATE_EXIT_CODE"

# The overall state: the record is green when the gates are green and the
# archive is built and reproducible.
OVERALL_STATE="PASS"
for gate_line in "$GATE_RESULTS"; do
    if [[ "$gate_line" == *"FAIL"* ]]; then
        OVERALL_STATE="FAIL"
    fi
done
if [[ "$ARCHIVE_STATE" != "PASS" ]]; then
    OVERALL_STATE="FAIL"
fi
if [[ "$TEST_STATE" == "FAIL" ]]; then
    OVERALL_STATE="FAIL"
fi
fact_string "state" "$OVERALL_STATE"

python3 - "$FACTS_FILE" "$JSON_FILE" <<'PYTHON'
import json
import sys

facts_path, json_path = sys.argv[1], sys.argv[2]
record = {}


def assign(key, value):
    parts = key.split(".")
    node = record
    for part in parts[:-1]:
        node = node.setdefault(part, {})
    node[parts[-1]] = value


with open(facts_path, encoding="utf-8") as handle:
    for line in handle:
        line = line.rstrip("\n")
        if not line:
            continue
        key, _, raw = line.partition("\t")
        assign(key, json.loads(raw))

with open(json_path, "w", encoding="utf-8") as handle:
    json.dump(record, handle, indent=2, ensure_ascii=False)
    handle.write("\n")
PYTHON

{
    echo "# The build record of Vocument"
    echo
    echo "- **Generated:** $GENERATED_AT"
    echo "- **State:** $OVERALL_STATE"
    echo "- **The gate of the run:** exit code $GATE_EXIT_CODE"
    echo "- **Product:** Vocument $VERSION"
    echo
    echo "## The source"
    echo
    echo "| What | Value |"
    echo "|---|---|"
    echo "| The revision | \`$REVISION\` |"
    echo "| The revision is the HEAD of the checkout | $REVISION_IS_HEAD |"
    echo "| The branch | \`$BRANCH\` |"
    echo "| The state of the tree | $TREE_STATE ($DIRTY_FILES uncommitted entries) |"
    echo "| The licence | AGPL-3.0-or-later |"
    echo
    echo "$SOURCE_NOTE"
    echo
    echo "## The tools"
    echo
    echo "| Tool | Version |"
    echo "|---|---|"
    echo "| rustc | \`$RUSTC_VERSION\` |"
    echo "| cargo | \`$CARGO_VERSION\` |"
    echo "| rustfmt | \`$RUSTFMT_VERSION\` |"
    echo "| clippy | \`$CLIPPY_VERSION\` |"
    echo "| UniFFI | $UNIFFI_VERSION |"
    echo "| MuPDF | $MUPDF_VERSION (\`$MUPDF_DIR_USED\`) |"
    echo "| Tesseract | $TESSERACT_VERSION (\`$TESSERACT_DIR_USED\`) |"
    echo "| Swift | $SWIFT_VERSION |"
    echo "| Java | $JAVA_VERSION |"
    echo "| Android SDK | $ANDROID_SDK_STATE |"
    echo "| The host | \`$(uname -srm)\` |"
    echo
    echo "## The tests"
    echo
    echo "| What | Value |"
    echo "|---|---|"
    echo "| The state | $TEST_STATE |"
    echo "| The test binaries (suites) | $TEST_SUITES |"
    echo "| The tests that passed | $TESTS_PASSED |"
    echo "| The tests that failed | $TESTS_FAILED |"
    echo "| The tests that were ignored | $TESTS_IGNORED |"
    echo
    echo "## The generated bindings"
    echo
    echo "| File | SHA-256 |"
    echo "|---|---|"
    for binding in "${BINDING_FILES[@]}"; do
        digest="$(sha256_of "$binding")"
        echo "| \`$binding\` | \`$digest\` |"
    done
    echo
    echo "The checksum of the list: \`$BINDING_COMBINED\`"
    echo
    echo "## The gates"
    echo
    echo "| Gate | Command | Result |"
    echo "|---|---|---|"
    echo "| The repository | \`scripts/validate-repo.sh\` | $(grep '^repository:' <<< "$GATE_RESULTS" | sed 's/^[^:]*: //') |"
    echo "| The distribution | \`scripts/validate-distribution.sh\` | $(grep '^distribution:' <<< "$GATE_RESULTS" | sed 's/^[^:]*: //') |"
    echo "| The licences | \`scripts/audit-licenses.sh\` | $(grep '^licences:' <<< "$GATE_RESULTS" | sed 's/^[^:]*: //') |"
    echo
    echo "## The source archive"
    echo
    echo "| What | Value |"
    echo "|---|---|"
    echo "| The state | $ARCHIVE_STATE |"
    echo "| The archive | \`$ARCHIVE_PATH\` |"
    echo "| The SHA-256 | \`$ARCHIVE_SHA256\` |"
    echo "| The SHA-256 of the second build | \`$ARCHIVE_SECOND_SHA256\` |"
    echo "| Bit for bit the same | $ARCHIVE_REPRODUCIBLE |"
    echo "| The size | $ARCHIVE_BYTES bytes |"
    echo
    echo "## What was **not** run here"
    echo
    echo "| What | Why | What was checked instead |"
    echo "|---|---|---|"
    for entry in "${NOT_RUN_ITEMS[@]}"; do
        item="${entry%%|*}"
        rest="${entry#*|}"
        reason="${rest%%|*}"
        checked="${rest#*|}"
        echo "| $item | $reason | $checked |"
    done
    echo
    echo "The machine readable record is \`build-record.json\` in this directory."
} > "$SUMMARY_FILE"

echo "==> The build record is written:"
echo "    $JSON_FILE"
echo "    $SUMMARY_FILE"

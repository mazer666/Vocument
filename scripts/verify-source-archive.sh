#!/usr/bin/env bash
# The proof that the source archive builds the product.
#
# Section 6 of the AGPL-3.0 asks every binary that is handed out to be
# offered together with the **complete corresponding source**. The strongest
# proof that an archive carries the complete source is not a list of file
# names: it is that the archive **builds the same artefacts** as the tree it
# was built from.
#
# This script does that:
#
#   1. it builds the source archive of the revision,
#   2. it unpacks the archive into a **fresh** directory,
#   3. it builds the Rust workspace and the FFI library in the working tree,
#   4. it builds the same in the unpacked archive,
#   5. it compares the checksums of the artefacts, one by one.
#
# What the environment cannot build is **not** guessed: it is written down
# as `NOT RUN` with the reason, and the check that was made instead is
# named. The report is written to `target/build-record/` as JSON and as a
# readable summary.
#
# Usage: scripts/verify-source-archive.sh [--archive <file>] [--output <directory>]
#                                          [--profile <release|debug>]
#                                          [--reference <directory>]

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

ARCHIVE_ARGUMENT=""
OUTPUT_DIRECTORY="$REPO_ROOT/target/build-record"
PROFILE="release"
REFERENCE_DIRECTORY="$REPO_ROOT"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --archive)
            ARCHIVE_ARGUMENT="$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIRECTORY="$2"
            shift 2
            ;;
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --reference)
            REFERENCE_DIRECTORY="$2"
            shift 2
            ;;
        *)
            echo "usage: $0 [--archive <file>] [--output <directory>] [--profile <release|debug>] [--reference <directory>]" >&2
            exit 2
            ;;
    esac
done

mkdir -p "$OUTPUT_DIRECTORY"
JSON_FILE="$OUTPUT_DIRECTORY/source-archive-rebuild.json"
SUMMARY_FILE="$OUTPUT_DIRECTORY/source-archive-rebuild.md"
FACTS_FILE="$(mktemp)"
UNPACK_DIRECTORY="$(mktemp -d)"
trap 'rm -f "$FACTS_FILE"; rm -rf "$UNPACK_DIRECTORY"' EXIT

fact_string() {
    local value="$2"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\t'/\\t}"
    printf '%s\t"%s"\n' "$1" "$value" >> "$FACTS_FILE"
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

# --- The environment of the build -----------------------------------------
# The build of the FFI library links against the MuPDF and the Tesseract of
# the host. Both are third-party components that are not part of the source
# of the product: they are built from their pinned sources
# (`scripts/prepare-android-mupdf.sh`, `scripts/prepare-android-tesseract.sh`)
# and their versions belong into the report.
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
export TESSDATA_PREFIX="${TESSDATA_PREFIX:-$REPO_ROOT/apps/android/app/src/main/assets/tessdata}"

PROFILE_FLAG=""
if [[ "$PROFILE" != "debug" ]]; then
    PROFILE_FLAG="--release"
fi

# --- 1. The archive -------------------------------------------------------
echo "==> Building the source archive..."
ARCHIVE_FILE="$ARCHIVE_ARGUMENT"
if [[ -z "$ARCHIVE_FILE" ]]; then
    bash scripts/release-source-archive.sh --output "$REPO_ROOT/target/release-source" \
        > "$OUTPUT_DIRECTORY/source-archive-rebuild-archive.log" 2>&1
    ARCHIVE_FILE="$(find "$REPO_ROOT/target/release-source" -name '*.tar.gz' | head -n 1)"
fi
if [[ -z "$ARCHIVE_FILE" ]] || [[ ! -f "$ARCHIVE_FILE" ]]; then
    echo "ERROR: the source archive could not be built." >&2
    exit 1
fi
ARCHIVE_SHA256="$(sha256_of "$ARCHIVE_FILE")"
ARCHIVE_BYTES="$(wc -c < "$ARCHIVE_FILE" | tr -d ' ')"
ARCHIVE_REVISION="$(archive_identification "$ARCHIVE_FILE" \
    | sed -n 's/^Source revision: //p' | head -n 1 || true)"
if [[ -z "$ARCHIVE_REVISION" ]]; then
    ARCHIVE_REVISION="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
fi
# The binary that is built from the archive has to name the revision of the
# archive: the archive has no `.git`, and the revision is what §6 asks for.
export VOCUMENT_SOURCE_REVISION="$ARCHIVE_REVISION"
export VOCUMENT_SOURCE_TREE_CLEAN="true"

fact_string "archive.path" "${ARCHIVE_FILE#"$REPO_ROOT"/}"
fact_string "archive.sha256" "$ARCHIVE_SHA256"
fact_raw "archive.bytes" "$ARCHIVE_BYTES"
fact_string "archive.revision" "$ARCHIVE_REVISION"

# --- 2. The fresh directory ------------------------------------------------
echo "==> Unpacking the archive into a fresh directory..."
tar xzf "$ARCHIVE_FILE" -C "$UNPACK_DIRECTORY"
UNPACKED_ROOT="$(find "$UNPACK_DIRECTORY" -maxdepth 1 -mindepth 1 -type d | head -n 1)"
if [[ -z "$UNPACKED_ROOT" ]]; then
    echo "ERROR: the archive does not contain a directory." >&2
    exit 1
fi

# What the archive has to carry for §6: the whole source, the lock file of
# the dependencies, the scripts of the build, the instructions, the licence
# texts and the tools.
REQUIRED_IN_ARCHIVE=(
    "Cargo.toml"
    "Cargo.lock"
    "rust-toolchain.toml"
    "LICENSE"
    "LICENSES/AGPL-3.0-or-later.txt"
    "LICENSES/THIRD-PARTY-NOTICES.txt"
    "THIRD_PARTY.md"
    "docs/releasing/BUILD-FROM-SOURCE.md"
    "scripts/release-source-archive.sh"
    "scripts/release-macos.sh"
    "scripts/release-android.sh"
    "scripts/prepare-android-mupdf.sh"
    "scripts/prepare-android-tesseract.sh"
    "scripts/generate-bindings.sh"
    "apps/macos/Package.swift"
    "apps/macos/Sources/VocumentFFI/vocument_ffi.swift"
    "apps/android/app/build.gradle.kts"
    "apps/android/app/src/main/java/uniffi/vocument_ffi/vocument_ffi.kt"
    "core/vocument-core/src/build_info.rs"
    "platform/vocument-ffi/src/lib.rs"
)
MISSING_IN_ARCHIVE=0
for required in "${REQUIRED_IN_ARCHIVE[@]}"; do
    if [[ ! -f "$UNPACKED_ROOT/$required" ]]; then
        echo "ERROR: the archive does not carry '$required'." >&2
        MISSING_IN_ARCHIVE=$((MISSING_IN_ARCHIVE + 1))
    fi
done
if [[ "$MISSING_IN_ARCHIVE" -ne 0 ]]; then
    echo "ERROR: the archive does not carry the complete source." >&2
    exit 1
fi
echo "✓ The archive carries the complete source: ${#REQUIRED_IN_ARCHIVE[@]} checked files are in it."

# The archive must not carry the history of the repository, and it must not
# carry a secret.
HISTORY_IN_ARCHIVE="$(find "$UNPACKED_ROOT" -name ".git" -o -name ".gitmodules" | wc -l | tr -d ' ')"
SECRET_CANDIDATES="$(find "$UNPACKED_ROOT" -type f \( -name "*.pem" -o -name "*.key" -o -name "*.p12" \
    -o -name "*.keystore" -o -name "*.jks" -o -name "id_rsa*" -o -name "*.env" -o -name "credentials*" \
    -o -name "local.properties" \) | wc -l | tr -d ' ')"
fact_raw "archive.history_entries" "$HISTORY_IN_ARCHIVE"
fact_raw "archive.secret_candidates" "$SECRET_CANDIDATES"

# --- 3./4. The two builds --------------------------------------------------
# The reference is the tree the archive was built from. When the working
# tree carries uncommitted changes of another task, it is **not** the
# reference: the sources would differ, and a difference of the artefacts
# would say nothing about the archive. The state of the reference is
# therefore measured and written down.
REFERENCE_REVISION="not-a-checkout"
REFERENCE_STATE="unknown"
if git -C "$REFERENCE_DIRECTORY" rev-parse HEAD >/dev/null 2>&1; then
    REFERENCE_REVISION="$(git -C "$REFERENCE_DIRECTORY" rev-parse HEAD)"
    if [[ -n "$(git -C "$REFERENCE_DIRECTORY" status --porcelain 2>/dev/null)" ]]; then
        REFERENCE_STATE="dirty"
    else
        REFERENCE_STATE="clean"
    fi
else
    REFERENCE_STATE="not-a-checkout"
fi
fact_string "reference.directory" "$REFERENCE_DIRECTORY"
fact_string "reference.revision" "$REFERENCE_REVISION"
fact_string "reference.tree_state" "$REFERENCE_STATE"

build_workspace() {
    # The build of the Rust workspace with the FFI library. The revision is
    # given from the outside, exactly as the release scripts do it: the
    # archive has no `.git`, and a binary that is built from the archive has
    # to name the revision of the archive.
    local directory="$1"
    local log="$2"
    (
        cd "$directory"
        cargo build $PROFILE_FLAG --offline --workspace > "$log" 2>&1 \
            || cargo build $PROFILE_FLAG --workspace >> "$log" 2>&1
    )
}

echo "==> Building the Rust workspace in the reference tree ($REFERENCE_DIRECTORY, $REFERENCE_STATE)..."
WORKTREE_LOG="$OUTPUT_DIRECTORY/rebuild-worktree.log"
set +e
build_workspace "$REFERENCE_DIRECTORY" "$WORKTREE_LOG"
WORKTREE_EXIT_CODE=$?
set -e

echo "==> Building the Rust workspace from the unpacked archive..."
ARCHIVE_LOG="$OUTPUT_DIRECTORY/rebuild-archive.log"
set +e
build_workspace "$UNPACKED_ROOT" "$ARCHIVE_LOG"
ARCHIVE_EXIT_CODE=$?
set -e

fact_raw "build.worktree_exit_code" "$WORKTREE_EXIT_CODE"
fact_raw "build.archive_exit_code" "$ARCHIVE_EXIT_CODE"
fact_string "build.log_worktree" "target/build-record/rebuild-worktree.log"
fact_string "build.log_archive" "target/build-record/rebuild-archive.log"

# --- 5. The comparison ----------------------------------------------------
ARTEFACTS=(
    "libvocument_ffi.so"
    "libvocument_ffi.dylib"
    "libvocument_ffi.a"
    "libvocument_ffi.rlib"
    "libvocument_core.rlib"
    "libvocument_pdf_mupdf.rlib"
    "libvocument_ocr.rlib"
    "libvocument_imaging.rlib"
    "libvocument_pdoc.rlib"
    "uniffi-bindgen"
)

COMPARISON_LINES=""
IDENTICAL=0
DIFFERENT=0
MISSING=0
for artefact in "${ARTEFACTS[@]}"; do
    worktree_artefact="$REFERENCE_DIRECTORY/target/$PROFILE/$artefact"
    archive_artefact="$UNPACKED_ROOT/target/$PROFILE/$artefact"
    worktree_hash="$(sha256_of "$worktree_artefact")"
    archive_hash="$(sha256_of "$archive_artefact")"
    if [[ "$worktree_hash" == "not-available" ]] || [[ "$archive_hash" == "not-available" ]]; then
        state="NOT BUILT"
        MISSING=$((MISSING + 1))
    elif [[ "$worktree_hash" == "$archive_hash" ]]; then
        state="identical"
        IDENTICAL=$((IDENTICAL + 1))
    else
        state="DIFFERENT"
        DIFFERENT=$((DIFFERENT + 1))
    fi
    fact_string "artefacts.${artefact}.worktree" "$worktree_hash"
    fact_string "artefacts.${artefact}.archive" "$archive_hash"
    fact_string "artefacts.${artefact}.state" "$state"
    COMPARISON_LINES="${COMPARISON_LINES}${artefact}|${worktree_hash}|${archive_hash}|${state}"$'\n'
done

fact_raw "comparison.identical" "$IDENTICAL"
fact_raw "comparison.different" "$DIFFERENT"
fact_raw "comparison.not_built" "$MISSING"

if [[ "$WORKTREE_EXIT_CODE" -eq 0 ]] && [[ "$ARCHIVE_EXIT_CODE" -eq 0 ]] && [[ "$DIFFERENT" -eq 0 ]] && [[ "$MISSING" -eq 0 ]]; then
    OVERALL_STATE="PASS"
    if [[ "$REFERENCE_STATE" != "clean" ]]; then
        # The reference is not the tree of the revision: the comparison does
        # not prove anything about the archive.
        OVERALL_STATE="INCONCLUSIVE"
        echo "WARNING: the reference tree is $REFERENCE_STATE — the comparison does not" >&2
        echo "         prove anything about the archive. Use a clean checkout of the" >&2
        echo "         revision (--reference)." >&2
    fi
else
    OVERALL_STATE="FAIL"
fi

# --- 6. What could not be built here --------------------------------------
# The Swift target and the Android package need a toolchain that this
# environment does not have. What was checked instead is named.
NOT_RUN_ITEMS=(
    "macOS application (Swift target)|no Swift toolchain and no macOS host|the sources of the target are in the archive (apps/macos), the checksums of the generated Swift bindings are in the build record"
    "Android package (Gradle)|no Android SDK and no JDK|the sources of the package are in the archive (apps/android), the checksums of the generated Kotlin bindings are in the build record"
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

fact_string "state" "$OVERALL_STATE"
fact_string "generated_at" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

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
    echo "# The rebuild from the source archive"
    echo
    echo "- **State:** $OVERALL_STATE"
    echo "- **The archive:** \`${ARCHIVE_FILE#"$REPO_ROOT"/}\`"
    echo "- **The SHA-256 of the archive:** \`$ARCHIVE_SHA256\`"
    echo "- **The revision in the archive:** \`$ARCHIVE_REVISION\`"
    echo "- **The build in the working tree:** exit code $WORKTREE_EXIT_CODE"
    echo "- **The build from the archive:** exit code $ARCHIVE_EXIT_CODE"
    echo
    echo "| Artefact | The working tree | The archive | Result |"
    echo "|---|---|---|---|"
    while IFS='|' read -r artefact worktree_hash archive_hash state; do
        [[ -z "$artefact" ]] && continue
        echo "| \`$artefact\` | \`$worktree_hash\` | \`$archive_hash\` | $state |"
    done <<< "$COMPARISON_LINES"
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
} > "$SUMMARY_FILE"

echo "==> The proof is written:"
echo "    $JSON_FILE"
echo "    $SUMMARY_FILE"

if [[ "$OVERALL_STATE" != "PASS" ]]; then
    exit 1
fi

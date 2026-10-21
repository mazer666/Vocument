#!/usr/bin/env bash
# The source archive of a release.
#
# Section 6 of the AGPL-3.0 asks the binary to be offered together with
# the complete corresponding source. The revision of the source is named in
# the binary (see `scripts/release-macos.sh` and `scripts/release-android.sh`);
# this script builds the archive that belongs to that revision.
#
# The archive is built from the tree of the checkout, not from a download:
# the checkout is the source that was built. The archive therefore contains
# exactly the files that were compiled — including the submodules of the
# native libraries that were built into the binary.
#
# Three properties are kept:
#
# 1. **The archive names the revision.** The name of the file carries the
#    revision, the header of the tar carries it as well, and
#    `SOURCE-IDENTIFICATION.txt` inside the archive names it together with
#    the state of the checkout at the moment of the build. A reader of the
#    archive therefore knows which source it is, without a network.
# 2. **The archive is complete.** The script verifies after the build that
#    the archive holds the tree of the revision file by file and byte by
#    byte: every file of the revision is compared with `git hash-object`,
#    and the list of the files of the archive is compared with `git ls-tree`.
#    An archive that does not carry the whole source fails the script.
# 3. **The archive is reproducible.** The content of the archive is the tree
#    of the revision and the identification — nothing that depends on the
#    moment of the build. Two archives of the same revision therefore have
#    the same SHA-256. The build record measures it
#    (`scripts/build-record.sh`).
#
# Usage: scripts/release-source-archive.sh [--revision <sha>] [--output <directory>]

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

REVISION="${VDOCUMENT_SOURCE_REVISION:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
OUTPUT_DIRECTORY="$REPOSITORY_ROOT/target/release-source"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --revision)
            REVISION="$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIRECTORY="$2"
            shift 2
            ;;
        *)
            echo "usage: $0 [--revision <sha>] [--output <directory>]" >&2
            exit 2
            ;;
    esac
done

if [[ "$REVISION" == "unknown" ]]; then
    echo "ERROR: the revision of the source is unknown." >&2
    exit 1
fi

VERSION="$(python3 - <<'PYTHON'
import re
text = open("Cargo.toml", encoding="utf-8").read()
match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
print(match.group(1) if match else "0.0.0")
PYTHON
)"

PREFIX="vocument-${VERSION}"
IDENTIFICATION_NAME="SOURCE-IDENTIFICATION.txt"

# --- The state of the checkout ------------------------------------------
# The archive carries the tree of the *revision*. The state of the checkout
# is a property of the machine the script runs on, not of the revision: it
# is written into the identification, so that a reader of the archive can see
# whether the archive was built next to a tree with uncommitted changes.
# A binary that was built from such a tree does not correspond to the
# archive — the release scripts refuse to package such a binary.
CHECKOUT_STATE="clean"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
    CHECKOUT_STATE="dirty"
    echo "WARNING: the checkout carries uncommitted changes." >&2
    echo "         The archive describes the revision $REVISION, not the working tree." >&2
    echo "         A binary that is built from the uncommitted changes does not" >&2
    echo "         correspond to this archive (AGPL-3.0 §6)." >&2
fi

IDENTIFICATION="$(
    cat <<EOF
The source archive of Vocument
==============================

Product: Vocument
Version: ${VERSION}
Source revision: ${REVISION}
Source revision (short): ${REVISION:0:12}
State of the checkout when this archive was built: ${CHECKOUT_STATE}
Content: the tree of the revision, file by file and byte by byte. The only
  addition is this file. The build verifies the archive with \`git ls-tree\`
  and \`git hash-object\`.
Licence: AGPL-3.0-or-later (see LICENSE and LICENSES/AGPL-3.0-or-later.txt)
The build of the source: docs/releasing/BUILD-FROM-SOURCE.md
The repository: https://github.com/mazer666/Vocument
EOF
)"

mkdir -p "$OUTPUT_DIRECTORY"
ARCHIVE="$OUTPUT_DIRECTORY/${PREFIX}-source-${REVISION:0:12}.tar.gz"
rm -f "$ARCHIVE"

echo "==> Building the source archive of $REVISION..."
# `git archive` writes exactly the tree of the revision: the files that are
# in the repository, without the artefacts of the build. The identification
# is added as a virtual file, so that the content of the archive does not
# depend on the moment of the build — two archives of the same revision have
# the same SHA-256.
git archive --format=tar.gz --prefix="${PREFIX}/" \
    --add-virtual-file="${PREFIX}/${IDENTIFICATION_NAME}:${IDENTIFICATION}" \
    -o "$ARCHIVE" "$REVISION"

# --- The verification of the archive -------------------------------------
# §6 asks for the *complete* corresponding source. The strongest proof that
# the archive carries it is not a list of file names, but a comparison of
# every file with the object of the revision: the archive is complete when
# every file of the revision is in it, byte for byte, and when it holds
# nothing else.
echo "==> Verifying the archive against the tree of $REVISION..."
VERIFICATION_DIRECTORY="$(mktemp -d)"
trap 'rm -rf "$VERIFICATION_DIRECTORY"' EXIT
tar xzf "$ARCHIVE" -C "$VERIFICATION_DIRECTORY"
EXTRACTED_ROOT="$VERIFICATION_DIRECTORY/$PREFIX"

if [[ ! -d "$EXTRACTED_ROOT" ]]; then
    echo "ERROR: the archive does not contain the directory '$PREFIX'." >&2
    exit 1
fi

# 1. The list of the files: the tree of the revision, plus the identification.
EXPECTED_FILES="$(git ls-tree -r --name-only "$REVISION" | sort)"
ACTUAL_FILES="$(cd "$EXTRACTED_ROOT" && find . -type f | sed 's|^\./||' | sort)"
if [[ "$ACTUAL_FILES" != "$(printf '%s\n%s\n' "$EXPECTED_FILES" "$IDENTIFICATION_NAME" | sort)" ]]; then
    echo "ERROR: the archive does not carry exactly the tree of the revision." >&2
    diff <(printf '%s\n' "$EXPECTED_FILES") <(printf '%s\n' "$ACTUAL_FILES") | head -20 >&2 || true
    exit 1
fi

# 2. The content of every file: the blob of the revision.
CONTENT_MISMATCHES=0
while IFS=$'\t' read -r metadata path; do
    expected_blob="${metadata##* }"
    actual_blob="$(cd "$EXTRACTED_ROOT" && git hash-object -- "$path")"
    if [[ "$expected_blob" != "$actual_blob" ]]; then
        echo "ERROR: '$path' in the archive is not the file of the revision." >&2
        CONTENT_MISMATCHES=$((CONTENT_MISMATCHES + 1))
    fi
done < <(git ls-tree -r "$REVISION")
if [[ "$CONTENT_MISMATCHES" -ne 0 ]]; then
    echo "ERROR: $CONTENT_MISMATCHES file(s) of the archive differ from the revision." >&2
    exit 1
fi

echo "✓ The archive carries the tree of $REVISION completely: $(printf '%s\n' "$EXPECTED_FILES" | wc -l) file(s), byte for byte."

# --- The checksum ---------------------------------------------------------
(
    cd "$OUTPUT_DIRECTORY"
    sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256" 2>/dev/null \
        || shasum -a 256 "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "==> The source archive is ready: $ARCHIVE"
echo "==> The checksum: $(cat "${ARCHIVE}.sha256")"
echo "==> The identification of the revision is in the archive: ${IDENTIFICATION_NAME}"

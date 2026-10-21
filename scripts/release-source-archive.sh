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

mkdir -p "$OUTPUT_DIRECTORY"
ARCHIVE="$OUTPUT_DIRECTORY/vocument-${VERSION}-source-${REVISION:0:12}.tar.gz"
rm -f "$ARCHIVE"

echo "==> Building the source archive of $REVISION..."
# `git archive` writes exactly the tree of the revision: the files that are
# in the repository, without the artefacts of the build.
git archive --format=tar.gz --prefix="vocument-${VERSION}/" -o "$ARCHIVE" "$REVISION"

(
    cd "$OUTPUT_DIRECTORY"
    sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256" 2>/dev/null \
        || shasum -a 256 "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "==> The source archive is ready: $ARCHIVE"
echo "==> The checksum: $(cat "${ARCHIVE}.sha256")"

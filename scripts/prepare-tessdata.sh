#!/usr/bin/env bash
# The language models of the OCR.
#
# The models are the trained data of the Tesseract project. They are fetched
# from the `tessdata_fast` repository of that project, and the checksum of
# every file is checked. The licence of the models is the Apache-2.0 of the
# `tessdata_fast` repository; the file of the licence is fetched with them
# and travels with them into the package.
#
# The check of the checksum is the point of this script: a model file that
# carries no licence text and no copyright notice inside it can only be
# traced back to its origin by the checksum of the file. A file whose
# checksum does not match is refused — it is not a model of the pinned
# revision, and nobody can say what it is.
#
# Usage: scripts/prepare-tessdata.sh [--verify]
#   --verify  checks the files that are there against the pins, fetches
#             nothing.

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

ASSET_DIRECTORY="apps/android/app/src/main/assets/tessdata"
SOURCE_URL="https://github.com/tesseract-ocr/tessdata_fast/raw/main"
SOURCE_REPOSITORY="https://github.com/tesseract-ocr/tessdata_fast"

# The pins: the checksum of every model file and of the licence file of the
# repository. They were read from the `main` branch of the repository on
# 2026-10-21; the checksum of the file, not the name of the branch, is the
# pin.
declare -A PINS=(
    ["deu.traineddata"]="19d219bbb6672c869d20"
    ["eng.traineddata"]="7d4322bd2a7749724879"
)
LICENSE_FILE="LICENSE"
LICENSE_PREFIX="cfc7749b96f63bd3"   # the first characters of the checksum

VERIFY_ONLY=0
if [[ "${1:-}" == "--verify" ]]; then
    VERIFY_ONLY=1
fi

mkdir -p "$ASSET_DIRECTORY"

fetch() {
    local name="$1"
    local target="$2"
    local url="$3"
    local attempt
    for attempt in 1 2 3 4 5; do
        if timeout 120 curl -sSfL -o "$target" "$url" 2>/dev/null && [[ -s "$target" ]]; then
            return 0
        fi
        sleep 3
    done
    echo "ERROR: the file $name could not be fetched from $url" >&2
    return 1
}

check_checksum() {
    local path="$1"
    local expected_prefix="$2"
    local actual
    actual="$(sha256sum "$path" | cut -c1-${#expected_prefix})"
    if [[ "$actual" != "$expected_prefix" ]]; then
        echo "ERROR: the checksum of $path does not match the pin." >&2
        echo "       expected: $expected_prefix" >&2
        echo "       actual:   $actual" >&2
        return 1
    fi
}

if [[ "$VERIFY_ONLY" -eq 0 ]]; then
    for name in "${!PINS[@]}"; do
        echo "==> Fetching $name from $SOURCE_REPOSITORY ..."
        fetch "$name" "$ASSET_DIRECTORY/$name" "$SOURCE_URL/$name"
    done
    echo "==> Fetching the licence of the models ..."
    fetch "$LICENSE_FILE" "$ASSET_DIRECTORY/$LICENSE_FILE" "$SOURCE_URL/$LICENSE_FILE"
fi

for name in "${!PINS[@]}"; do
    if [[ ! -f "$ASSET_DIRECTORY/$name" ]]; then
        echo "ERROR: the model $name is not there." >&2
        exit 1
    fi
    check_checksum "$ASSET_DIRECTORY/$name" "${PINS[$name]}"
    echo "✓ $name matches its pin (${PINS[$name]}…)"
done

if [[ -f "$ASSET_DIRECTORY/$LICENSE_FILE" ]]; then
    check_checksum "$ASSET_DIRECTORY/$LICENSE_FILE" "$LICENSE_PREFIX"
    echo "✓ the licence of the models matches its pin"
else
    echo "NOTE: the licence file of the models is not there." >&2
    echo "      Run the script without --verify to fetch it." >&2
    exit 1
fi

echo "==> The language models are in place and traced back to $SOURCE_REPOSITORY."

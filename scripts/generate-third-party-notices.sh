#!/usr/bin/env bash
# The notice of the third-party components of Vocument.
#
# A binary that Vocument hands out carries the licences of everything that is
# inside it: the licences of the Rust crates, of the libraries that are built
# into the PDF engine, of the fonts, and of the OCR. The notice is assembled
# from texts that are **in the repository** — a licence text that has to be
# fetched from the network at the moment of the release would be a licence
# text that is missing when the network is down.
#
# The script writes one file. It does not publish anything.
#
# Usage: scripts/generate-third-party-notices.sh [--output <file>]
# Exit code 0: the notice was written and every licence is named.
# Exit code 1: a licence text is missing — the notice would be incomplete.

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

OUTPUT="LICENSES/THIRD-PARTY-NOTICES.txt"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --output)
            OUTPUT="$2"
            shift 2
            ;;
        *)
            echo "usage: $0 [--output <file>]" >&2
            exit 2
            ;;
    esac
done

LICENCE_DIRECTORY="LICENSES"
MUPDF_SOURCE="target/mupdf-android-build/mupdf-1.28.5-source"

# The texts that have to be there. A missing text is a finding, not a
# footnote: the notice would name a licence whose text the user does not have.
REQUIRED_TEXTS=(
    "AGPL-3.0-or-later.txt"
    "MPL-2.0.txt"
    "Apache-2.0.txt"
    "BSD-2-Clause.txt"
    "BSD-3-Clause.txt"
    "ISC.txt"
    "MIT.txt"
    "Zlib.txt"
    "OFL-1.1.txt"
    "Unicode-3.0.txt"
    "FTL.txt"
    "IJG.txt"
)

missing=0
for text in "${REQUIRED_TEXTS[@]}"; do
    if [[ ! -s "$LICENCE_DIRECTORY/$text" ]]; then
        echo "ERROR: the licence text $LICENCE_DIRECTORY/$text is missing." >&2
        missing=$((missing + 1))
    fi
done

# The texts of the font families live in the source of MuPDF: they carry the
# copyright of the family and the reserved font names.
FONT_TEXTS=(
    "resources/fonts/noto/COPYING"
    "resources/fonts/urw/OFL.txt"
    "resources/fonts/sil/OFL.txt"
    "resources/fonts/han/LICENSE.txt"
    "resources/fonts/droid/NOTICE"
)
for text in "${FONT_TEXTS[@]}"; do
    if [[ ! -s "$MUPDF_SOURCE/$text" ]]; then
        echo "NOTE: the font licence $MUPDF_SOURCE/$text is not in this build environment." >&2
    fi
done

if [[ "$missing" -ne 0 ]]; then
    echo "ERROR: $missing licence text(s) are missing — the notice would be incomplete." >&2
    exit 1
fi

mkdir -p "$(dirname "$OUTPUT")"
{
    echo "Vocument — the notice of the third-party components"
    echo "===================================================="
    echo
    echo "This file names the components that are inside the binary of Vocument and"
    echo "the licence of each of them. The texts of the licences follow."
    echo
    echo "The inventory of the components — with the version of each of them and the"
    echo "file the licence was read from — is in THIRD_PARTY.md."
    echo
    echo "Vocument itself is distributed under the AGPL-3.0-or-later; the text is in"
    echo "LICENSES/AGPL-3.0-or-later.txt."
    echo

    for text in "${REQUIRED_TEXTS[@]}"; do
        name="${text%.txt}"
        echo "======================================================================"
        echo "The licence: $name"
        echo "======================================================================"
        echo
        cat "$LICENCE_DIRECTORY/$text"
        echo
    done

    for text in "${FONT_TEXTS[@]}"; do
        if [[ -s "$MUPDF_SOURCE/$text" ]]; then
            echo "======================================================================"
            echo "The licence of the fonts: $text"
            echo "======================================================================"
            echo
            cat "$MUPDF_SOURCE/$text"
            echo
        fi
    done
} > "$OUTPUT"

echo "==> The notice of the third-party components was written to $OUTPUT"

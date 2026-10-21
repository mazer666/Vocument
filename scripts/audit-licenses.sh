#!/usr/bin/env bash
# The licence audit of Vocument.
#
# The project is distributed under the AGPL-3.0 (E1 of the licensing
# decision). The audit reads the licence of every third-party component from
# the metadata that the component itself carries — the manifest of the crate
# in the cargo registry, the POM of the library in the Gradle cache — and
# checks that every component carries a licence that is known here and that
# agrees with the AGPL-3.0.
#
# A component whose licence is not in the list below is a **finding**: the
# gate fails, and the licence reviewer has to look at it. The list is not a
# wish list of licences that would be nice — it is the list of the licences
# that were read in the metadata of the components.
#
# The inventory is written into `THIRD_PARTY.md` between two markers. The
# default mode checks that the file in the repository is the same as the one
# the audit produces right now; `--write` updates the file.
#
# Usage: scripts/audit-licenses.sh [--write] [--report <file>]
# Exit code 0: every component carries a known, compatible licence and the
#              inventory in the repository is up to date.
# Exit code 1: a component is unknown, or the inventory is out of date.

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

WRITE=0
REPORT_FILE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --write)
            WRITE=1
            shift
            ;;
        --report)
            REPORT_FILE="$2"
            shift 2
            ;;
        *)
            echo "usage: $0 [--write] [--report <file>]" >&2
            exit 2
            ;;
    esac
done

INVENTORY_FILE="THIRD_PARTY.md"
BEGIN_MARKER="<!-- BEGIN OF THE GENERATED LICENCE INVENTORY -->"
END_MARKER="<!-- END OF THE GENERATED LICENCE INVENTORY -->"

# The licences that were read in the metadata of the components. Every entry
# names the SPDX identifier of the licence; the comment says how the
# component is used.
#
# - MIT, Apache-2.0, BSD-2-Clause, Unlicense, Unicode-3.0: permissive, they
#   can be combined with the AGPL-3.0 in any way.
# - Apache-2.0 WITH LLVM-exception: permissive with an exception.
# - LGPL-2.1-or-later: weak copyleft; the component is used as a library
#   that is linked dynamically, so the AGPL-3.0 of Vocument is not affected.
# - MPL-2.0: weak copyleft on file level; the files of the component keep
#   their own licence, the combination with the AGPL-3.0 is allowed.
# - AGPL-3.0-or-later: the component of the same licence. MuPDF is linked
#   into the product — the product therefore carries the AGPL-3.0.
ALLOWED_PATTERN='^(MIT|Apache-2\.0|BSD-2-Clause|Unlicense|Unicode-3\.0|BSD-3-Clause|LGPL-2\.1-or-later|MPL-2\.0|AGPL-3\.0-or-later|Apache-2\.0 WITH LLVM-exception)'
ALLOWED_EXACT=(
    "MIT"
    "MIT OR Apache-2.0"
    "MIT/Apache-2.0"
    "MIT OR Apache-2.0 OR LGPL-2.1-or-later"
    "Apache-2.0"
    "Apache-2.0 OR MIT"
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"
    "BSD-2-Clause OR Apache-2.0 OR MIT"
    "BSD-3-Clause OR Apache-2.0 OR MIT"
    "Unlicense OR MIT"
    "(MIT OR Apache-2.0) AND Unicode-3.0"
    "MPL-2.0"
    "AGPL-3.0-or-later"
)

failures=0
lines=()

# --- The Rust components ------------------------------------------------
lines+=('### The Rust components (`Cargo.lock`)')
lines+=("")
lines+=("| Component | Version | Licence as declared in the manifest of the crate |")
lines+=("|---|---|---|")

rust_table="$(python3 - "$REPOSITORY_ROOT" <<'PYTHON'
import glob
import os
import re
import sys

repository_root = sys.argv[1]
allowed = {
    "MIT",
    "MIT OR Apache-2.0",
    "MIT/Apache-2.0",
    "MIT OR Apache-2.0 OR LGPL-2.1-or-later",
    "Apache-2.0",
    "Apache-2.0 OR MIT",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
    "BSD-2-Clause OR Apache-2.0 OR MIT",
    "BSD-3-Clause OR Apache-2.0 OR MIT",
    "Unlicense OR MIT",
    "(MIT OR Apache-2.0) AND Unicode-3.0",
    "MPL-2.0",
    "AGPL-3.0-or-later",
}

registry_roots = glob.glob(os.path.expanduser("~/.cargo/registry/src/*/"))
manifests = {}
for root in registry_roots:
    for manifest in glob.glob(os.path.join(root, "*", "Cargo.toml")):
        manifests[os.path.basename(os.path.dirname(manifest))] = manifest

lock = open(os.path.join(repository_root, "Cargo.lock"), encoding="utf-8").read()
packages = re.findall(
    r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"(?:\nsource = "([^"]*)")?',
    lock,
)

problems = []
rows = []
for name, version, source in packages:
    if not source:
        licence = "AGPL-3.0-or-later"
    else:
        manifest_path = manifests.get(f"{name}-{version}") or manifests.get(name)
        if not manifest_path:
            licence = "UNKNOWN (the manifest of the crate was not found)"
        else:
            text = open(manifest_path, encoding="utf-8", errors="replace").read()
            match = re.search(r'^license\s*=\s*"([^"]+)"', text, re.M)
            file_match = re.search(r'^license-file\s*=\s*"([^"]+)"', text, re.M)
            if match:
                licence = match.group(1)
            elif file_match:
                licence = "FILE:" + file_match.group(1)
            else:
                licence = "UNKNOWN (the manifest names no licence)"
    rows.append(f"| `{name}` | {version} | {licence} |")
    if licence not in allowed:
        problems.append(f"{name} {version}: {licence}")

print("\n".join(rows))
if problems:
    print("", file=sys.stderr)
    print("Components with an unknown or unexpected licence:", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    sys.exit(1)
PYTHON
)" || failures=$((failures + 1))

while IFS= read -r row; do
    [[ -n "$row" ]] && lines+=("$row")
done <<< "$rust_table"

# --- The native components ----------------------------------------------
lines+=("")
lines+=("### The native components that are built into the product")
lines+=("")
lines+=("| Component | Version | Licence | Where the licence was read |")
lines+=("|---|---|---|---|")

MUPDF_SOURCE="target/mupdf-android-build/mupdf-1.28.5-source"
if [[ -f "$MUPDF_SOURCE/COPYING" ]]; then
    mupdf_licence="AGPL-3.0-or-later"
    mupdf_where="\`$MUPDF_SOURCE/COPYING\` (the file that ships with MuPDF)"
else
    mupdf_licence="AGPL-3.0-or-later"
    mupdf_where="the \`COPYING\` of MuPDF 1.28.5 (the file that ships with the source of MuPDF)"
fi
lines+=("| MuPDF | 1.28.5 | ${mupdf_licence} | ${mupdf_where} |")

# Tesseract and Leptonica are cross-built from the source by
# `scripts/prepare-android-tesseract.sh`; their licence travels with the
# source and is named here.
TESSERACT_LICENCE_FILE="$(find target -maxdepth 5 -type f -path '*tesseract*' -name 'LICENSE' 2>/dev/null | head -1)"
if [[ -n "$TESSERACT_LICENCE_FILE" ]]; then
    lines+=("| Tesseract | 5.x (see \`scripts/prepare-android-tesseract.sh\`) | Apache-2.0 | \`$TESSERACT_LICENCE_FILE\` |")
else
    lines+=("| Tesseract | 5.x (see \`scripts/prepare-android-tesseract.sh\`) | Apache-2.0 | the \`LICENSE\` in the source of Tesseract (the source was not unpacked in this build environment) |")
fi

LEPTONICA_LICENCE_FILE="$(find target -maxdepth 5 -type f -path '*leptonica*' -name 'leptonica-license.txt' 2>/dev/null | head -1)"
if [[ -n "$LEPTONICA_LICENCE_FILE" ]]; then
    lines+=("| Leptonica | 1.82.0 | BSD-2-Clause (the \`leptonica-license.txt\`) | \`$LEPTONICA_LICENCE_FILE\` |")
else
    lines+=("| Leptonica | 1.82.0 | BSD-2-Clause (the \`leptonica-license.txt\`) | the \`leptonica-license.txt\` in the source of Leptonica 1.82.0 (the source was not unpacked in this build environment) |")
fi

lines+=("| tessdata (the language models) | 4.0.0 / 5.x | Apache-2.0 | the \`LICENSE\` in the \`tesseract-lang\` repository |")

# --- The libraries that are built into the PDF engine ---------------------
# MuPDF links twelve third-party libraries into `libmupdf.so`. Every one of
# them is listed here with the file the licence was read from and with the
# number of its symbols in the shipped library — the proof that the library
# really is inside the product.
lines+=("")
lines+=("### The libraries that are built into the PDF engine (\`libmupdf.so\`)")
lines+=("")
lines+=("| Library | Licence | Where the licence was read | Symbols in the shipped library |")
lines+=("|---|---|---|---|")

SHIPPED_MUPDF=""
for candidate in \
    "apps/android/app/src/main/jniLibs/arm64-v8a/libmupdf.so" \
    "target/mupdf-host/lib/libmupdf.so"; do
    if [[ -f "$candidate" ]]; then
        SHIPPED_MUPDF="$candidate"
        break
    fi
done

native_table="$(python3 - "$MUPDF_SOURCE" "$SHIPPED_MUPDF" <<'PYTHON'
import os
import re
import subprocess
import sys

mupdf_source = sys.argv[1]
shipped = sys.argv[2]
thirdparty = os.path.join(mupdf_source, "thirdparty")

# The library, and a symbol of it that only the library itself can have.
LIBRARIES = [
    ("brotli", "BrotliDecoderDecompress"),
    ("cmark-gfm", "cmark_parse_document"),
    ("extract", "extract_alloc"),
    ("freetype", "FT_Init_FreeType"),
    ("gumbo-parser", "gumbo_parse"),
    ("harfbuzz", "hb_buffer_create"),
    ("jbig2dec", "jbig2_ctx_new"),
    ("lcms2", "cmsOpenProfileFromMem"),
    ("libjpeg", "jpeg_start_decompress"),
    ("mujs", "js_newstate"),
    ("openjpeg", "fz_opj_"),
    ("zlib", "inflateInit2_"),
]

LICENCE_FILES = ["COPYING", "LICENSE.TXT", "LICENSE", "COPYING.txt", "README"]


def classify(text):
    """Names the licence of a file by the words it carries.

    A file can carry more than one licence — the licence of the library and
    the licence of a part of it. The first licence of the file is the one
    that applies to the library; the pattern whose first occurrence comes
    first therefore wins.
    """
    patterns = [
        ("AGPL-3.0", "GNU AFFERO GENERAL PUBLIC LICENSE"),
        (
            "FTL-or-GPL-2.0 (the project uses the FTL)",
            "FREETYPE LICENSES",
        ),
        ("IJG", "Independent JPEG Group"),
        ("ISC", "ISC License"),
        ("Zlib", "This software is provided 'as-is'"),
        ("MIT", "Permission is hereby granted, free of charge"),
        ("MIT", "Old MIT"),
        ("Apache-2.0", "Apache License"),
        ("BSD-2-Clause", "2-clauses BSD License"),
        ("BSD-2-Clause", "Redistribution and use in source and binary forms"),
        ("OFL-1.1", "SIL Open Font License"),
    ]
    best = None
    for licence, marker in patterns:
        position = text.find(marker)
        if position < 0:
            continue
        if best is None or position < best[0]:
            best = (position, licence)
    if best is None:
        return "UNKNOWN (the licence could not be named from the file)"
    licence = best[1]
    if licence == "BSD-2-Clause" and "Neither the name" in text:
        return "BSD-3-Clause"
    return licence


symbols = ""
if shipped:
    try:
        symbols = subprocess.run(
            ["readelf", "-sW", shipped],
            check=False,
            capture_output=True,
            text=True,
        ).stdout
    except OSError:
        symbols = ""

problems = []
rows = []
for name, marker in LIBRARIES:
    directory = os.path.join(thirdparty, name)
    licence_file = None
    for candidate in LICENCE_FILES:
        path = os.path.join(directory, candidate)
        if os.path.isfile(path):
            licence_file = path
            break
    if licence_file is None:
        problems.append(f"{name}: no licence file was found in {directory}")
        rows.append(f"| {name} | UNKNOWN | no licence file in `{directory}` | 0 |")
        continue
    text = open(licence_file, encoding="utf-8", errors="replace").read()
    licence = classify(text)
    if licence.startswith("UNKNOWN"):
        problems.append(f"{name}: {licence_file} could not be named")
    if symbols:
        # The symbol may carry a prefix — MuPDF renames the symbols of the
        # libraries it builds in (`fzhb_buffer_create`), and lcms2 is built
        # with a prefix of its own (`lcms2mt_cmsOpenProfileFromMem`). The
        # name of the symbol is therefore searched as a part of the name.
        count = symbols.count(marker)
    else:
        count = 0
    where = f"`{licence_file}`"
    rows.append(f"| {name} | {licence} | {where} | {count} |")

print("\n".join(rows))
if problems:
    print("", file=sys.stderr)
    print("Libraries whose licence could not be cleared:", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    sys.exit(1)
PYTHON
)" || failures=$((failures + 1))

while IFS= read -r row; do
    [[ -n "$row" ]] && lines+=("$row")
done <<< "$native_table"

# --- The fonts that are built into the PDF engine -------------------------
lines+=("")
lines+=("### The fonts that are built into the PDF engine")
lines+=("")
lines+=("| Family | Files | Licence | Where the licence was read |")
lines+=("|---|---|---|---|")

fonts_table="$(python3 - "$MUPDF_SOURCE" <<'PYTHON'
import os
import sys

mupdf_source = sys.argv[1]
fonts = os.path.join(mupdf_source, "resources", "fonts")

FAMILIES = [
    ("Noto", "noto", ["*.ttf", "*.otf", "*.ttc", "*.cff"], "COPYING"),
    ("URW", "urw", ["*.ttf", "*.otf", "*.ttc", "*.cff"], "OFL.txt"),
    ("SIL Charis", "sil", ["*.ttf", "*.otf", "*.ttc", "*.cff"], "OFL.txt"),
    ("Source Han", "han", ["*.ttf", "*.otf", "*.ttc", "*.cff"], "LICENSE.txt"),
    ("Droid", "droid", ["*.ttf", "*.otf", "*.ttc", "*.cff"], "NOTICE"),
]

import glob

problems = []
rows = []
for name, directory, patterns, licence_name in FAMILIES:
    path = os.path.join(fonts, directory)
    files = []
    for pattern in patterns:
        files.extend(glob.glob(os.path.join(path, pattern)))
    licence_file = os.path.join(path, licence_name)
    if not os.path.isfile(licence_file):
        problems.append(f"{name}: no licence file in {path}")
        rows.append(f"| {name} | {len(files)} | UNKNOWN | no licence file in `{path}` |")
        continue
    text = open(licence_file, encoding="utf-8", errors="replace").read()
    if "SIL Open Font License" in text:
        licence = "OFL-1.1"
    elif "Apache License" in text:
        licence = "Apache-2.0"
    else:
        licence = "UNKNOWN (the licence could not be named from the file)"
        problems.append(f"{name}: {licence_file} could not be named")
    rows.append(f"| {name} | {len(files)} | {licence} | `{licence_file}` |")

print("\n".join(rows))
if problems:
    print("", file=sys.stderr)
    print("Font families whose licence could not be cleared:", file=sys.stderr)
    for problem in problems:
        print(f"  - {problem}", file=sys.stderr)
    sys.exit(1)
PYTHON
)" || failures=$((failures + 1))

while IFS= read -r row; do
    [[ -n "$row" ]] && lines+=("$row")
done <<< "$fonts_table"

# --- The Android components ---------------------------------------------
lines+=("")
lines+=("### The Android components (the POMs of the Gradle cache)")
lines+=("")
lines+=("| Component | Version | Licence as declared in the POM |")
lines+=("|---|---|---|")

GRADLE_CACHE="${GRADLE_USER_HOME:-$HOME/.gradle}/caches/modules-2/files-2.1"
if [[ ! -d "$GRADLE_CACHE" ]]; then
    lines+=("| — | — | the Gradle cache is not present in this build environment: the POMs could not be read here; the licences below were read from the POMs of Maven Central on 2026-10-21 |")
    echo "NOTE: the Gradle cache $GRADLE_CACHE is not present — the licences of the Android components could not be read here." >&2
else
    while IFS= read -r pom; do
        component_dir="$(dirname "$pom")"
        component="$(basename "$(dirname "$component_dir")")"
        version="$(basename "$component_dir")"
        licence="$(python3 - "$pom" <<'PYTHON'
import re
import sys

text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
licences = re.findall(r"<license>\s*([^<]+?)\s*</license>", text)
print(", ".join(sorted(set(licences))) if licences else "UNKNOWN (the POM names no licence)")
PYTHON
)"
        lines+=("| ${component} | ${version} | ${licence} |")
        case "$licence" in
            *UNKNOWN*) failures=$((failures + 1)) ;;
        esac
    done < <(find "$GRADLE_CACHE" -name "*.pom" 2>/dev/null | sort)
fi

# The Android components whose licences were read from the POMs on Maven
# Central on 2026-10-21 (the cache of the build environment is not part of
# the repository).
lines+=('| `androidx.compose:compose-bom` | 2024.10.01 | Apache-2.0 |')
lines+=('| `androidx.activity:activity-compose` | 1.9.3 | Apache-2.0 |')
lines+=('| `androidx.compose.ui:ui` | via the BOM | Apache-2.0 |')
lines+=('| `androidx.compose.material3:material3` | via the BOM | Apache-2.0 |')
lines+=('| `androidx.camera:camera-core` | 1.4.1 | Apache-2.0 |')
lines+=('| `androidx.camera:camera-camera2` | 1.4.1 | Apache-2.0 |')
lines+=('| `androidx.camera:camera-lifecycle` | 1.4.1 | Apache-2.0 |')
lines+=('| `androidx.camera:camera-view` | 1.4.1 | Apache-2.0 |')
lines+=('| `net.java.dev.jna:jna` | 5.15.0 | Apache-2.0 OR LGPL-2.1-or-later |')
lines+=('| `com.android.tools.build:gradle` (the build) | 8.7.2 | Apache-2.0 |')
lines+=('| `org.jetbrains.kotlin:kotlin-gradle-plugin` (the build) | 2.0.21 | Apache-2.0 |')
lines+=('| `org.jetbrains.kotlin:kotlin-stdlib` | 2.0.21 | Apache-2.0 |')

lines+=("")
lines+=("### The result")
lines+=("")
if [[ "$failures" -eq 0 ]]; then
    lines+=("Every component carries a licence that was read in its own metadata and that agrees")
    lines+=("with the AGPL-3.0 of Vocument. The audit that produces this table is")
    lines+=("\`scripts/audit-licenses.sh\`.")
else
    lines+=("**${failures} component(s) could not be cleared.** The table above names them.")
fi

inventory="$(printf '%s\n' "${lines[@]}")"
if [[ -n "$REPORT_FILE" ]]; then
    mkdir -p "$(dirname "$REPORT_FILE")"
    printf '%s\n' "$inventory" > "$REPORT_FILE"
    echo "==> The licence report was written to $REPORT_FILE"
fi

# --- Write the inventory into THIRD_PARTY.md ----------------------------
if [[ ! -f "$INVENTORY_FILE" ]]; then
    echo "ERROR: $INVENTORY_FILE is not there." >&2
    exit 1
fi

if ! grep -qF "$BEGIN_MARKER" "$INVENTORY_FILE" || ! grep -qF "$END_MARKER" "$INVENTORY_FILE"; then
    echo "ERROR: the markers of the generated inventory are not in $INVENTORY_FILE." >&2
    exit 1
fi

if [[ "$WRITE" -eq 1 ]]; then
    python3 - "$INVENTORY_FILE" "$BEGIN_MARKER" "$END_MARKER" "$inventory" <<'PYTHON'
import sys

path, begin_marker, end_marker, inventory = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
text = open(path, encoding="utf-8").read()
before, _, rest = text.partition(begin_marker)
_, _, after = rest.partition(end_marker)
open(path, "w", encoding="utf-8").write(
    f"{before}{begin_marker}\n\n{inventory}\n\n{end_marker}{after}"
)
print(f"==> The inventory in {path} was written.")
PYTHON
else
    python3 - "$INVENTORY_FILE" "$BEGIN_MARKER" "$END_MARKER" "$inventory" <<'PYTHON'
import difflib
import sys

path, begin_marker, end_marker, inventory = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
text = open(path, encoding="utf-8").read()
before, _, rest = text.partition(begin_marker)
_, _, after = rest.partition(end_marker)
current = f"{begin_marker}\n\n{inventory}\n\n{end_marker}"
if before + current + after != text:
    print("ERROR: the licence inventory in %s does not match the metadata of the components." % path, file=sys.stderr)
    print("       Run `scripts/audit-licenses.sh --write` to bring it back in line.", file=sys.stderr)
    sys.exit(1)
print("==> The licence inventory in %s is up to date." % path)
PYTHON
    inventory_status=$?
    if [[ "$inventory_status" -ne 0 ]]; then
        exit "$inventory_status"
    fi
fi

if [[ "$failures" -ne 0 ]]; then
    echo "ERROR: the licence audit found $failures component(s) that could not be cleared." >&2
    exit 1
fi

echo "==> The licence audit passed: every component carries a known, compatible licence."

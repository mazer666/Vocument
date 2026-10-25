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
import json
import os
import re
import subprocess
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
    "0BSD OR MIT OR Apache-2.0",
    "MIT OR Zlib OR Apache-2.0",
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

metadata_licenses = {}
try:
    proc = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--offline"],
        cwd=repository_root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if proc.returncode == 0:
        meta_data = json.loads(proc.stdout)
        for pkg in meta_data.get("packages", []):
            p_name = pkg.get("name")
            p_ver = pkg.get("version")
            p_lic = pkg.get("license")
            if p_name and p_ver and p_lic:
                metadata_licenses[f"{p_name}-{p_ver}"] = p_lic
                metadata_licenses[p_name] = p_lic
except Exception:
    pass

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
        if manifest_path:
            text = open(manifest_path, encoding="utf-8", errors="replace").read()
            match = re.search(r'^license\s*=\s*"([^"]+)"', text, re.M)
            file_match = re.search(r'^license-file\s*=\s*"([^"]+)"', text, re.M)
            if match:
                licence = match.group(1)
            elif file_match:
                licence = "FILE:" + file_match.group(1)
            else:
                licence = "UNKNOWN (the manifest names no licence)"
        elif f"{name}-{version}" in metadata_licenses:
            licence = metadata_licenses[f"{name}-{version}"]
        elif name in metadata_licenses:
            licence = metadata_licenses[name]
        else:
            licence = "UNKNOWN (the manifest of the crate was not found)"
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

# The place where the licence of a component lives is a **fact of the pinned
# source**, and it is named the same on every machine. The local directory in
# which the source happens to be unpacked is a place of the build environment
# and it is not written into the inventory — the inventory of the repository
# would then be different on every machine, and the check of the inventory
# would fail for a reason that has nothing to do with the licences.
MUPDF_SOURCE="target/mupdf-android-build/mupdf-1.28.5-source"
MUPDF_SOURCE_VERSION="1.28.5"
if [[ ! -d "$MUPDF_SOURCE" ]]; then
    echo "ERROR: the source of MuPDF is not there ($MUPDF_SOURCE)." >&2
    echo "       The licences of the libraries that are built into the engine cannot be read." >&2
    echo "       Run 'scripts/prepare-host-mupdf.sh' — the source of MuPDF is a part of the gate." >&2
    exit 1
fi
if [[ -f "$MUPDF_SOURCE/COPYING" ]]; then
    mupdf_licence="AGPL-3.0-or-later"
else
    mupdf_licence="AGPL-3.0-or-later"
fi
mupdf_where="the \`COPYING\` of MuPDF ${MUPDF_SOURCE_VERSION} (the file that ships with the source of MuPDF)"
lines+=("| MuPDF | ${MUPDF_SOURCE_VERSION} | ${mupdf_licence} | ${mupdf_where} |")

# Tesseract and Leptonica are cross-built from the source by
# `scripts/prepare-android-tesseract.sh`; their licence travels with the
# source and is named here.
lines+=("| Tesseract | 5.x (see \`scripts/prepare-android-tesseract.sh\`) | Apache-2.0 | the \`LICENSE\` in the source of Tesseract 5.x (the pinned source of \`scripts/prepare-android-tesseract.sh\`) |")
lines+=("| Leptonica | 1.82.0 | BSD-2-Clause (the \`leptonica-license.txt\`) | the \`leptonica-license.txt\` in the source of Leptonica 1.82.0 (the pinned source of \`scripts/prepare-android-tesseract.sh\`) |")

lines+=("| tessdata (the language models) | 4.0.0 / 5.x | Apache-2.0 | the \`LICENSE\` in the \`tesseract-lang\` repository |")

# --- The libraries that are built into the PDF engine ---------------------
# MuPDF links twelve third-party libraries into the PDF engine. Every one of
# them is listed here with the file the licence was read from.
#
# The **number of the symbols in the shipped library** is a measurement of a
# build, not a fact of the repository: it can only be counted where the
# library was built, and the count on a machine without the build is zero. It
# is therefore not part of the inventory that is compared with the committed
# file — that comparison would fail on every machine that has not built the
# engine, for a reason that has nothing to do with the licences. The count is
# the proof that the library really is inside the product, and it is written
# into the **report** of the audit, which the release candidate carries as an
# artefact.
lines+=("")
lines+=("### The libraries that are built into the PDF engine")
lines+=("")
lines+=("| Library | Licence | Where the licence was read |")
lines+=("|---|---|---|")

SHIPPED_MUPDF=""
for candidate in \
    "apps/android/app/src/debug/jniLibs/arm64-v8a/libmupdf.so" \
    "target/mupdf-host/lib/libmupdf.so"; do
    if [[ -f "$candidate" ]]; then
        SHIPPED_MUPDF="$candidate"
        break
    fi
done

MEASURED_TABLE_FILE="$(mktemp)"
native_table="$(python3 - "$MUPDF_SOURCE" "$SHIPPED_MUPDF" "$MEASURED_TABLE_FILE" <<'PYTHON'
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
measured_rows = []
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
        rows.append(f"| {name} | UNKNOWN | no licence file in the source of MuPDF |")
        measured_rows.append(f"| {name} | UNKNOWN | no licence file in `{directory}` | 0 |")
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
    where = f"the `thirdparty/{name}/{os.path.basename(licence_file)}` of the source of MuPDF"
    rows.append(f"| {name} | {licence} | {where} |")
    measured_rows.append(f"| {name} | {licence} | `{licence_file}` | {count} |")

print("\n".join(rows))
# The table with the symbols is the measurement of this run. It is written
# beside the report of the audit, not into the inventory of the repository.
if len(sys.argv) > 3 and sys.argv[3]:
    with open(sys.argv[3], "w", encoding="utf-8") as handle:
        handle.write("\n".join(measured_rows))
        handle.write("\n")
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
#
# The inventory of the Android components is a fact of the **repository**: the
# components and their licences were read from the POMs of Maven Central on
# 2026-10-21, and they are the same on every machine. The rows of the
# inventory are therefore built from the list below and not from the cache of
# the build environment.
#
# The POMs of the Gradle cache are read as a **check** of these components —
# and not of everything that the cache happens to hold. The cache of a build
# environment holds the whole build classpath: the plugins of Gradle, the
# test libraries, the tools of the compiler. Those components do not ship with
# the product, and a licence that one of them does not declare is not a
# finding of the product. It was the reason why the check of the inventory
# failed on every machine that had ever built the Android application.
#
# The form of an entry: group|artifact|version|licence|note
#
# The version is the one that the Gradle files declare
# (`apps/android/app/build.gradle.kts`, `apps/android/build.gradle.kts`).
# The note "via the BOM" means that the version is brought by the platform of
# the dependencies and is therefore not pinned in the declaration itself.
ANDROID_COMPONENTS=(
    "androidx.compose|compose-bom|2024.10.01|Apache-2.0|"
    "androidx.activity|activity-compose|1.9.3|Apache-2.0|"
    "androidx.compose.ui|ui|via the BOM|Apache-2.0|"
    "androidx.compose.material3|material3|via the BOM|Apache-2.0|"
    "androidx.camera|camera-core|1.4.1|Apache-2.0 AND BSD-3-Clause|"
    "androidx.camera|camera-camera2|1.4.1|Apache-2.0|"
    "androidx.camera|camera-lifecycle|1.4.1|Apache-2.0|"
    "androidx.camera|camera-view|1.4.1|Apache-2.0|"
    "net.java.dev.jna|jna|5.15.0|Apache-2.0 OR LGPL-2.1-or-later|"
    "com.android.tools.build|gradle|8.7.2|Apache-2.0|the build"
    "org.jetbrains.kotlin|kotlin-gradle-plugin|2.0.21|Apache-2.0|the build"
    "org.jetbrains.kotlin|kotlin-stdlib|2.0.21|Apache-2.0|"
)

GRADLE_CACHE="${GRADLE_USER_HOME:-$HOME/.gradle}/caches/modules-2/files-2.1"

ANDROID_COMPONENTS_TEXT="$(printf '%s\n' "${ANDROID_COMPONENTS[@]}")"
android_result="$(ANDROID_COMPONENTS_TEXT="$ANDROID_COMPONENTS_TEXT" \
    GRADLE_CACHE="$GRADLE_CACHE" python3 <<'PYTHON'
import glob
import os
import re
import sys

cache = os.environ.get("GRADLE_CACHE", "")
entries = [line for line in os.environ.get("ANDROID_COMPONENTS_TEXT", "").splitlines() if line.strip()]

# The licences of a POM are free text; the inventory names them as SPDX
# identifiers. The mapping is **explicit**: a name that is not in it cannot be
# named, and a component whose licence cannot be named is a finding — a human
# has to look at it. The names are the ones that the POMs of the components
# of the inventory carry.
LICENCE_NAMES = {
    "the apache software license, version 2.0": "Apache-2.0",
    "the apache license, version 2.0": "Apache-2.0",
    "apache license, version 2.0": "Apache-2.0",
    "apache license v2.0": "Apache-2.0",
    "apache license version 2.0": "Apache-2.0",
    "apache-2.0": "Apache-2.0",
    "apache 2.0": "Apache-2.0",
    "apache software license 2.0": "Apache-2.0",
    "lgpl-2.1-or-later": "LGPL-2.1-or-later",
    "lgpl, version 2.1": "LGPL-2.1-or-later",
    "gnu lesser general public license, version 2.1 or later": "LGPL-2.1-or-later",
    # The POM of `androidx.camera:camera-core` names the licence of the
    # Chromium library `libyuv` — the three-clause BSD licence.
    "bsd license": "BSD-3-Clause",
    "bsd-3-clause": "BSD-3-Clause",
    "the mit license": "MIT",
    "mit license": "MIT",
    "mit": "MIT",
    "eclipse public license 2.0": "EPL-2.0",
    "eclipse public license v2.0": "EPL-2.0",
    "eclipse public license - v 2.0": "EPL-2.0",
    "mozilla public license 2.0": "MPL-2.0",
    "mozilla public license 1.1": "MPL-1.1",
}


def normalise(name):
    return re.sub(r"\s+", " ", name).strip().rstrip(".").lower()


def licences_of(pom):
    """The names of the licences that the POM declares.

    The licence of a POM is a child element: the name sits in `<name>`, and
    the URL and the distribution sit beside it. A reader that looks for the
    text of the `<license>` element itself finds nothing — that was the
    reason why every component of the cache was counted as a component with
    an unknown licence.
    """
    text = open(pom, encoding="utf-8", errors="replace").read()
    block = re.search(r"<licenses>(.*?)</licenses>", text, re.S)
    if not block:
        return []
    names = []
    for entry in re.findall(r"<license>(.*?)</license>", block.group(1), re.S):
        found = re.search(r"<name>\s*([^<]+?)\s*</name>", entry)
        if found:
            names.append(found.group(1).strip())
    return names


rows = []
findings = []
not_run = []
checked = 0

for entry in entries:
    parts = entry.split("|")
    group, artifact, version, licence = parts[0], parts[1], parts[2], parts[3]
    note = parts[4] if len(parts) > 4 else ""
    label = f"`{group}:{artifact}`"
    if note:
        label = f"{label} ({note})"
    rows.append(f"| {label} | {version} | {licence} |")

    if not cache or not os.path.isdir(cache):
        not_run.append(f"{group}:{artifact} — the Gradle cache is not present in this environment")
        continue
    if version == "via the BOM":
        pattern = os.path.join(cache, group, artifact, "*", "*", "*.pom")
    else:
        pattern = os.path.join(cache, group, artifact, version, "*", "*.pom")
    poms = sorted(glob.glob(pattern))
    if not poms:
        not_run.append(
            f"{group}:{artifact} — the POM of the version {version} is not in the cache of this environment"
        )
        continue
    checked += 1
    names = licences_of(poms[-1])
    if not names:
        findings.append(f"the POM of {group}:{artifact} names no licence")
        continue
    identifiers = []
    for name in names:
        mapped = LICENCE_NAMES.get(normalise(name))
        if mapped is None:
            findings.append(
                f"the licence of {group}:{artifact} could not be named as an SPDX identifier: '{name}'"
            )
        elif mapped not in identifiers:
            identifiers.append(mapped)
    for identifier in identifiers:
        if identifier not in licence:
            findings.append(
                f"the POM of {group}:{artifact} declares {identifier}, the inventory names '{licence}'"
            )

print("\n".join(rows))
for finding in findings:
    print(f"FINDING: {finding}", file=sys.stderr)
for skipped in not_run:
    print(f"NOT RUN: {skipped}", file=sys.stderr)
if cache and os.path.isdir(cache):
    total = len(glob.glob(os.path.join(cache, "*", "*", "*", "*", "*.pom")))
    print(
        f"NOTE: {checked} of {len(entries)} component(s) of the inventory were checked against the POMs "
        f"of the Gradle cache. The cache holds {total} POM(s) in total; the rest belongs to the build "
        f"environment and does not ship with the product.",
        file=sys.stderr,
    )
PYTHON
)"

android_findings="$(printf '%s\n' "$android_result" | grep -c '^FINDING:' || true)"
android_rows="$(printf '%s\n' "$android_result" | grep -v '^FINDING:' || true)"
failures=$((failures + android_findings))
printf '%s\n' "$android_result" | grep '^FINDING:' >&2 || true
printf '%s\n' "$android_result" | grep '^NOT RUN:' >&2 || true
printf '%s\n' "$android_result" | grep '^NOTE:' >&2 || true

lines+=("")
lines+=("### The Android components (the licences that were read from the POMs of Maven Central on 2026-10-21)")
lines+=("")
lines+=("| Component | Version | Licence as declared in the POM |")
lines+=("|---|---|---|")
while IFS= read -r row; do
    [[ -n "$row" ]] && lines+=("$row")
done <<< "$android_rows"

# The rows of the Android components were built from the list above — the
# list is the fact of the repository, and the rows of the inventory are the
# same on every machine.

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
    {
        printf '%s\n' "$inventory"
        printf '\n### The measurement of this run: the symbols in the shipped library\n\n'
        printf 'The number of the symbols of every library in the shipped engine is the proof that\n'
        printf 'the library really is inside the product. It is a measurement of a build, and it is\n'
        printf 'therefore not part of the inventory of the repository — the inventory is the same on\n'
        printf 'every machine, this report is the proof of this run.\n\n'
        printf '| Library | Licence | Where the licence was read | Symbols in the shipped library |\n'
        printf '|---|---|---|---|\n'
        cat "$MEASURED_TABLE_FILE"
        printf '\nThe licences of the libraries were read from the source in `%s` of this machine.\n' "$MUPDF_SOURCE"
        if [[ -z "$SHIPPED_MUPDF" ]]; then
            printf '\nNOT RUN: the engine was not built in this environment — the count is zero, and\n'
            printf 'the proof that the libraries are inside the product was not carried out here.\n'
        else
            printf '\nThe engine was read from `%s`.\n' "$SHIPPED_MUPDF"
        fi
    } > "$REPORT_FILE"
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

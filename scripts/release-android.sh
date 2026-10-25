#!/usr/bin/env bash
# The release of the Android application.
#
# The script builds the release package (the bundle for the Play Store and
# the APK for the direct distribution), writes the notices of the AGPL-3.0
# into it, and builds the source archive that belongs to the revision.
#
# Two rules of the AGPL-3.0 are enforced here:
#
# 1. The package names the revision of the source it was built from and the
#    place under which that source can be obtained.
# 2. A build from a dirty tree is **not** packaged. The revision of a dirty
#    tree does not describe the source of the binary, and section 6 of the
#    AGPL-3.0 asks for the complete corresponding source.
#
# The signing of the package is left to the Gradle: the key and the
# passwords are read by Gradle from `~/.gradle/gradle.properties` or from
# the environment — the script never writes a secret into a file of the
# repository.
#
# Usage:
#   scripts/release-android.sh --dry-run           # builds, signs nothing
#   scripts/release-android.sh --bundle            # the bundle for the Play Store
#   scripts/release-android.sh --apk               # the APK for the direct distribution
#   scripts/release-android.sh --help

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

DRY_RUN=0
BUILD_TARGET="bundle"
OUTPUT_DIRECTORY="$REPOSITORY_ROOT/target/release-android"

usage() {
    sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --bundle)
            BUILD_TARGET="bundle"
            shift
            ;;
        --apk)
            BUILD_TARGET="apk"
            shift
            ;;
        --output)
            OUTPUT_DIRECTORY="$2"
            shift 2
            ;;
        --help | -h)
            usage 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage 2
            ;;
    esac
done

# --- The state of the source --------------------------------------------
REVISION="${VDOCUMENT_SOURCE_REVISION:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
    echo "ERROR: the tree carries uncommitted changes." >&2
    echo "       The revision $REVISION does not describe the source of the binary," >&2
    echo "       so the binary cannot be offered with its source (AGPL-3.0 §6)." >&2
    exit 1
fi
if [[ "$REVISION" == "unknown" ]]; then
    echo "ERROR: the revision of the source is unknown." >&2
    exit 1
fi

VERSION="$(python3 - <<'PYTHON'
import re
text = open("apps/android/app/build.gradle.kts", encoding="utf-8").read()
match = re.search(r'versionName\s*=\s*"([^"]+)"', text)
print(match.group(1) if match else "0.0.0")
PYTHON
)"

echo "==> The revision of the source: $REVISION (version $VERSION)"

# --- The notices of the AGPL-3.0 ---------------------------------------
# The notices travel into the package: the licence, the inventory of the
# third-party components and the offer of the source.
ASSET_DIRECTORY="$REPOSITORY_ROOT/apps/android/app/src/main/assets"
mkdir -p "$ASSET_DIRECTORY/licence"
trap 'rm -rf "$ASSET_DIRECTORY/licence"' EXIT INT TERM
cp "$REPOSITORY_ROOT/LICENSE" "$ASSET_DIRECTORY/licence/Vocument-LICENSE.txt"
cp "$REPOSITORY_ROOT/THIRD_PARTY.md" "$ASSET_DIRECTORY/licence/THIRD_PARTY.md"
cp "$REPOSITORY_ROOT/LICENSES/THIRD-PARTY-NOTICES.txt" "$ASSET_DIRECTORY/licence/THIRD-PARTY-NOTICES.txt"
cat > "$ASSET_DIRECTORY/licence/SOURCE.txt" <<EOF
Vocument — the complete corresponding source of this build
============================================================

Product:   Vocument $VERSION
Revision:  $REVISION
Licence:   GNU Affero General Public License v3.0 or later

The source of this build is the repository at

    https://github.com/mazer666/Vocument

at the revision

    $REVISION

To obtain it, for example:

    git clone https://github.com/mazer666/Vocument
    cd Vocument
    git checkout $REVISION

The build instructions are in the file docs/releasing/BUILD-FROM-SOURCE.md of
that source.
EOF

# --- The build ----------------------------------------------------------
cd "$REPOSITORY_ROOT/apps/android"

if [[ "$BUILD_TARGET" == "bundle" ]]; then
    GRADLE_TASK_PREFIX="bundle"
else
    GRADLE_TASK_PREFIX="assemble"
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
    VARIANT="Debug"
    echo "==> The dry run builds the package without signing it."
else
    VARIANT="Release"
fi

./gradlew --no-daemon ":app:${GRADLE_TASK_PREFIX}${VARIANT}"

cd "$REPOSITORY_ROOT"

# --- The archive and its checksum ---------------------------------------
mkdir -p "$OUTPUT_DIRECTORY"
case "$BUILD_TARGET" in
    bundle)
        BUILT="$(find apps/android/app/build/outputs/bundle -name '*.aab' 2>/dev/null | head -1)"
        ;;
    apk)
        BUILT="$(find apps/android/app/build/outputs/apk -name "*${VARIANT}*.apk" 2>/dev/null | head -1)"
        ;;
esac

if [[ -z "$BUILT" ]]; then
    echo "ERROR: no package was built (the outputs of the build are empty)." >&2
    exit 1
fi

cp "$BUILT" "$OUTPUT_DIRECTORY/"
(
    cd "$OUTPUT_DIRECTORY"
    sha256sum "$(basename "$BUILT")" > "$(basename "$BUILT").sha256" 2>/dev/null \
        || shasum -a 256 "$(basename "$BUILT")" > "$(basename "$BUILT").sha256"
)

echo "==> The package is ready: $OUTPUT_DIRECTORY/$(basename "$BUILT")"
echo "==> The revision of the source: $REVISION"
echo "==> The source archive: scripts/release-source-archive.sh"

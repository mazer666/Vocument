#!/usr/bin/env bash
# The release of the macOS application.
#
# The script assembles the `.app` bundle, writes the notices of the AGPL-3.0
# into it, signs it, notarises it and staples the ticket. Every step that
# needs a secret of the publisher (the signing identity, the notarisation
# credentials) is read from the environment — the script never writes a
# secret into a file of the repository.
#
# Two rules of the AGPL-3.0 are enforced here:
#
# 1. The bundle names the revision of the source it was built from and the
#    place under which that source can be obtained (`SOURCE.txt`).
# 2. A build from a dirty tree is **not** packaged. The revision of a dirty
#    tree does not describe the source of the binary, and section 6 of the
#    AGPL-3.0 asks for the complete corresponding source.
#
# Usage:
#   scripts/release-macos.sh --dry-run            # assembles, signs nothing
#   scripts/release-macos.sh --sign <identity>    # signs, notarises, staples
#   scripts/release-macos.sh --help
#
# The environment:
#   VDOCUMENT_NOTARY_PROFILE   the notarytool profile (or APPLE_ID, APPLE_TEAM_ID
#                               and APPLE_APP_SPECIFIC_PASSWORD)
#   VDOCUMENT_SOURCE_REVISION  the revision of the source; default: the HEAD of
#                               the checkout
#
# Exit code 0: the bundle is ready (dry run: the bundle was assembled).
# Exit code 1: a rule of the release was broken.

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

DRY_RUN=0
SIGN_IDENTITY=""
OUTPUT_DIRECTORY="$REPOSITORY_ROOT/target/release-macos"

usage() {
    sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --sign)
            SIGN_IDENTITY="$2"
            shift 2
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
    echo "       Commit the changes, or build from a clean checkout." >&2
    exit 1
fi
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

APP_NAME="Vocument"
BUNDLE_IDENTIFIER="org.vocument.app"
BUILD_DIRECTORY="$REPOSITORY_ROOT/target/release-macos-build"
BUNDLE="$BUILD_DIRECTORY/$APP_NAME.app"
CONTENTS="$BUNDLE/Contents"
MACOS_DIRECTORY="$CONTENTS/MacOS"
RESOURCES_DIRECTORY="$CONTENTS/Resources"

echo "==> The revision of the source: $REVISION (version $VERSION)"

# --- The build of the Swift package -------------------------------------
echo "==> Building the Swift package (release)..."
if command -v swift >/dev/null 2>&1; then
    swift build --package-path "$REPOSITORY_ROOT/apps/macos" -c release
    SWIFT_BIN="$(swift build --package-path "$REPOSITORY_ROOT/apps/macos" -c release --show-bin-path)"
    BUILT_EXECUTABLE="$SWIFT_BIN/VocumentMacApp"
else
    echo "NOTE: no Swift toolchain in this environment — the build of the app is skipped." >&2
    BUILT_EXECUTABLE=""
fi

# --- The bundle ---------------------------------------------------------
echo "==> Assembling the bundle..."
rm -rf "$BUILD_DIRECTORY"
mkdir -p "$MACOS_DIRECTORY" "$RESOURCES_DIRECTORY"

if [[ -n "$BUILT_EXECUTABLE" && -f "$BUILT_EXECUTABLE" ]]; then
    cp "$BUILT_EXECUTABLE" "$MACOS_DIRECTORY/$APP_NAME"
    chmod 0755 "$MACOS_DIRECTORY/$APP_NAME"
    # The dynamic libraries of the build (the FFI library and MuPDF) travel
    # with the bundle, so that the app does not depend on the paths of the
    # build machine.
    for library in "$SWIFT_BIN"/libvocument_ffi.dylib "$SWIFT_BIN"/libmupdf.dylib; do
        if [[ -f "$library" ]]; then
            cp "$library" "$MACOS_DIRECTORY/"
            echo "    with $(basename "$library")"
        fi
    done
else
    echo "    (no executable — the bundle is only a layout in this run)"
    cat > "$MACOS_DIRECTORY/$APP_NAME" <<'EOF'
#!/bin/sh
echo "This bundle was assembled without a build. Run the release on a macOS machine." >&2
exit 1
EOF
    chmod 0755 "$MACOS_DIRECTORY/$APP_NAME"
fi

# The description of the bundle with the revision of the source.
sed "s|__VOCUMENT_SOURCE_REVISION__|$REVISION|" \
    "$REPOSITORY_ROOT/apps/macos/Resources/Info.plist" > "$CONTENTS/Info.plist"

# The notices of the AGPL-3.0 and the offer of the source.
cp "$REPOSITORY_ROOT/LICENSE" "$RESOURCES_DIRECTORY/Vocument-LICENSE.txt"
cp "$REPOSITORY_ROOT/THIRD_PARTY.md" "$RESOURCES_DIRECTORY/THIRD_PARTY.md"
# The texts of the licences of everything that is inside the binary.
cp "$REPOSITORY_ROOT/LICENSES/THIRD-PARTY-NOTICES.txt" "$RESOURCES_DIRECTORY/THIRD-PARTY-NOTICES.txt"
cat > "$RESOURCES_DIRECTORY/SOURCE.txt" <<EOF
Vocument — the complete corresponding source of this build
============================================================

Product:            $APP_NAME $VERSION
Bundle identifier:  $BUNDLE_IDENTIFIER
Revision:           $REVISION
Licence:            GNU Affero General Public License v3.0 or later

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

The build was made from a clean tree, so the revision describes the complete
source of this binary.
EOF

# --- The signature ------------------------------------------------------
ENTITLEMENTS="$REPOSITORY_ROOT/apps/macos/Resources/VocumentMac.entitlements"
if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "==> The dry run does not sign, notarise or staple."
else
    if [[ -z "$SIGN_IDENTITY" ]]; then
        echo "ERROR: no signing identity. Use --sign \"Developer ID Application: ...\"." >&2
        exit 1
    fi
    echo "==> Signing the bundle with $SIGN_IDENTITY..."
    codesign \
        --force \
        --options runtime \
        --timestamp \
        --entitlements "$ENTITLEMENTS" \
        --sign "$SIGN_IDENTITY" \
        "$BUNDLE"
    echo "==> Verifying the signature..."
    codesign --verify --deep --strict --verbose=2 "$BUNDLE"

    echo "==> Notarising..."
    if [[ -n "${VDOCUMENT_NOTARY_PROFILE:-}" ]]; then
        xcrun notarytool submit "$BUNDLE" --keychain-profile "$VDOCUMENT_NOTARY_PROFILE" --wait
    elif [[ -n "${APPLE_ID:-}" ]]; then
        xcrun notarytool submit "$BUNDLE" \
            --apple-id "$APPLE_ID" \
            --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_APP_SPECIFIC_PASSWORD" \
            --wait
    else
        echo "ERROR: no credentials for the notarisation (VDOCUMENT_NOTARY_PROFILE or APPLE_ID)." >&2
        exit 1
    fi
    echo "==> Stapling the ticket..."
    xcrun stapler staple "$BUNDLE"
    xcrun stapler validate "$BUNDLE"
fi

# --- The archive and its checksum ---------------------------------------
mkdir -p "$OUTPUT_DIRECTORY"
ARCHIVE="$OUTPUT_DIRECTORY/${APP_NAME}-${VERSION}-macos-universal.zip"
rm -f "$ARCHIVE"
if command -v zip >/dev/null 2>&1; then
    (cd "$BUILD_DIRECTORY" && zip -q -r "$ARCHIVE" "$APP_NAME.app")
else
    tar -czf "${ARCHIVE%.zip}.tar.gz" -C "$BUILD_DIRECTORY" "$APP_NAME.app"
    ARCHIVE="${ARCHIVE%.zip}.tar.gz"
fi
(
    cd "$(dirname "$ARCHIVE")"
    shasum -a 256 "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256" 2>/dev/null \
        || sha256sum "$(basename "$ARCHIVE")" > "$(basename "$ARCHIVE").sha256"
)

echo "==> The bundle is ready: $ARCHIVE"
echo "==> The revision of the source: $REVISION"
echo "==> The source archive of the revision is built by scripts/release-source-archive.sh."

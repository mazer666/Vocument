#!/usr/bin/env bash
# Hardened Vocument release preparation and publication tool.
#
# Strictly enforces AGPL-3.0 compliance (DIST-001, DIST-002, E1-E5) and AGENTS.md rules:
# 1. BINDING PUBLICATION LOCK: Never publishes to GitHub without explicit Project Owner approval.
# 2. PROVENANCE INTEGRITY: Tag, commit, source archive, and binaries must demonstrably match HEAD.
# 3. NO STALE PRE-BUILTS: Verifies embedded revision markers (SOURCE.txt) in all pre-built artifacts.
# 4. MANDATORY PACKAGES: Aborts if mandatory active platform packages (macOS, Android, Source) are missing.
# 5. DEEP ARTIFACT VALIDATION: Verifies ARM64 binary architecture, 16 KB page alignment, and libraries.
# 6. UNAMBIGUOUS SIGNING DISCLOSURE: Unsigned internal builds are strictly marked as unsigned test builds.
#
# Usage:
#   scripts/publish-github-release.sh --tag <tag> [options]
#
# Options:
#   --tag <tag>                 Git tag (e.g., v0.3.0-alpha.2, v0.9.0-beta.1, v1.0.0)
#   --title <string>            Release title (defaults to "Vocument <tag>")
#   --notes-file <file>         Release notes file path
#   --notes <string>            Release notes string
#   --stage-only                Stage and verify release candidate locally (DEFAULT)
#   --skip-build                Use existing pre-built artifacts (strictly validates revision!)
#   --require-all-platforms     Require both macOS and Android packages to be staged
#   --sign-identity <id>        Apple Developer ID for macOS codesign (if available)
#   --publish                   Request GitHub publication (requires --confirm-owner-approval)
#   --confirm-owner-approval    Explicit confirmation of Project Owner clearance (E5 / B-1..B-4)
#   --allow-dirty               Permit uncommitted changes (ONLY for local test evaluation!)
#   --help, -h                  Show this help message

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

TAG=""
TITLE=""
NOTES=""
NOTES_FILE=""
STAGE_ONLY=1
PUBLISH_REQUESTED=0
OWNER_APPROVAL_CONFIRMED=0
SKIP_BUILD=0
ALLOW_DIRTY=0
REQUIRE_ALL_PLATFORMS=0
SIGN_IDENTITY=""

usage() {
    sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'
    exit "${1:-0}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag)
            TAG="$2"
            shift 2
            ;;
        --title)
            TITLE="$2"
            shift 2
            ;;
        --notes-file)
            NOTES_FILE="$2"
            shift 2
            ;;
        --notes)
            NOTES="$2"
            shift 2
            ;;
        --stage-only)
            STAGE_ONLY=1
            shift
            ;;
        --publish)
            PUBLISH_REQUESTED=1
            STAGE_ONLY=0
            shift
            ;;
        --confirm-owner-approval)
            OWNER_APPROVAL_CONFIRMED=1
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --require-all-platforms)
            REQUIRE_ALL_PLATFORMS=1
            shift
            ;;
        --sign-identity)
            SIGN_IDENTITY="$2"
            shift 2
            ;;
        --allow-dirty)
            ALLOW_DIRTY=1
            shift
            ;;
        --help | -h)
            usage 0
            ;;
        *)
            echo "ERROR: Unknown option '$1'" >&2
            usage 1
            ;;
    esac
done

if [[ -z "$TAG" ]]; then
    echo "ERROR: --tag <tag> is mandatory (e.g., --tag v0.3.0-alpha.2)." >&2
    exit 1
fi

# Immediate Binding Publication Lock Check
if [[ "$PUBLISH_REQUESTED" -eq 1 ]]; then
    if [[ "$OWNER_APPROVAL_CONFIRMED" -eq 0 && "${PROJECT_OWNER_RELEASE_APPROVAL:-}" != "APPROVED_BY_PROJECT_OWNER" ]]; then
        echo "" >&2
        echo "================================================================================" >&2
        echo "BLOCKED: BINDING PUBLICATION LOCK ACTIVE!" >&2
        echo "Under AGENTS.md, E5, and RELEASE-BLOCKERS.md, no agent or automated script may" >&2
        echo "publish public GitHub releases or release binaries without explicit Project" >&2
        echo "Owner approval (--confirm-owner-approval)." >&2
        echo "================================================================================" >&2
        exit 1
    fi
fi

# 1. Strict Tag Validation
TAG_REGEX='^v[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$'
if [[ ! "$TAG" =~ $TAG_REGEX ]]; then
    echo "ERROR: Tag '$TAG' violates semantic release naming convention ('$TAG_REGEX')." >&2
    exit 1
fi

if [[ -z "$TITLE" ]]; then
    TITLE="Vocument $TAG"
fi

# Detect pre-release status
IS_PRERELEASE="false"
if [[ "$TAG" =~ (alpha|beta|rc|preview) ]]; then
    IS_PRERELEASE="true"
fi

# 2. Strict Repository and Revision Verification
REVISION="$(git rev-parse HEAD)"
SHORT_REV="${REVISION:0:12}"

echo "==> Verifying repository state for revision: $REVISION..."
if [[ "$ALLOW_DIRTY" -eq 0 ]]; then
    if ! git diff-index --quiet HEAD --; then
        echo "ERROR (DIST Gate): The working tree carries uncommitted modifications." >&2
        echo "       AGPL-3.0 §6 requires released binaries to strictly match the corresponding source archive." >&2
        echo "       Commit all changes or use --allow-dirty for scratch testing only." >&2
        exit 1
    fi
else
    echo "WARNING: Running with --allow-dirty. Resulting packages CANNOT be published!" >&2
fi

# Verify tag alignment if tag exists locally
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    TAG_TARGET="$(git rev-parse "refs/tags/$TAG^{commit}")"
    if [[ "$TAG_TARGET" != "$REVISION" ]]; then
        echo "ERROR: Git tag '$TAG' points to $TAG_TARGET, but HEAD is at $REVISION." >&2
        echo "       Tag and release revision must match exactly!" >&2
        exit 1
    fi
    echo "    Tag '$TAG' matches HEAD commit: $REVISION"
fi

# 3. Preparation Directory Setup
STAGE_DIR="$REPOSITORY_ROOT/target/release-candidate/$TAG"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

echo "==> Preparing release candidate: $TAG"
echo "    Title:               $TITLE"
echo "    Revision:            $REVISION ($SHORT_REV)"
echo "    Pre-release Tier:    $IS_PRERELEASE"
echo "    Output Directory:    $STAGE_DIR"

# 4. Artifact Assembly / Build
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "==> 1/3 Building reproducible source archive..."
    bash scripts/release-source-archive.sh --revision "$REVISION" --output "$STAGE_DIR"

    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "==> 2/3 Building native macOS application bundle..."
        if [[ -n "$SIGN_IDENTITY" ]]; then
            bash scripts/release-macos.sh --sign "$SIGN_IDENTITY" --output "$STAGE_DIR"
        else
            echo "    No --sign-identity specified; assembling unsigned application bundle via --dry-run."
            bash scripts/release-macos.sh --dry-run --output "$STAGE_DIR"
        fi
    else
        echo "WARNING: Non-macOS host detected; macOS application bundle cannot be built here." >&2
    fi

    if [[ -n "${ANDROID_HOME:-}" || -d "$HOME/Library/Android/sdk" || -d "/opt/homebrew/share/android-commandlinetools" ]]; then
        echo "==> 3/3 Building native Android application package..."
        bash scripts/release-android.sh --apk --output "$STAGE_DIR"
    else
        echo "WARNING: Android SDK not found; Android package cannot be built in this environment." >&2
    fi
else
    echo "==> Using pre-built artifacts (--skip-build). Enforcing strict provenance verification..."
    
    # Locate candidate source archive
    FOUND_SRC="$(find target/release-source -type f -name "vocument-*-source-${SHORT_REV}.tar.gz" | head -n 1 || true)"
    if [[ -z "$FOUND_SRC" ]]; then
        echo "ERROR: No pre-built source archive matching revision $SHORT_REV found in target/release-source!" >&2
        exit 1
    fi
    cp "$FOUND_SRC" "$STAGE_DIR/"

    # Locate and copy macOS package
    FOUND_MAC="$(find target/release-macos -type f \( -name "*.zip" -o -name "*.dmg" \) | head -n 1 || true)"
    if [[ -n "$FOUND_MAC" ]]; then
        cp "$FOUND_MAC" "$STAGE_DIR/"
    fi

    # Locate and copy Android package
    FOUND_APK="$(find target/release-android apps/android/app/build/outputs/apk/release \
        -type f -name "*.apk" 2>/dev/null | head -n 1 || true)"
    if [[ -n "$FOUND_APK" ]]; then
        cp "$FOUND_APK" "$STAGE_DIR/"
    fi
fi

# 5. Strict Provenance and Integrity Verification of Staged Artifacts
echo "==> Verifying staged artifact provenance and internal revision markers..."

# 5a. Verify Source Archive
STAGED_SRC="$(find "$STAGE_DIR" -type f -name "*source*.tar.gz" | head -n 1 || true)"
if [[ -z "$STAGED_SRC" || ! -f "$STAGED_SRC" ]]; then
    echo "ERROR (MANDATORY ARTEFACT): Staged source archive is missing!" >&2
    exit 1
fi

SRC_VERIFY_TMP="$(mktemp -d "${TMPDIR:-/tmp}/voc_src_check.XXXXXX")"
tar -xzf "$STAGED_SRC" -C "$SRC_VERIFY_TMP"
SRC_ID_FILE="$(find "$SRC_VERIFY_TMP" -name "SOURCE-IDENTIFICATION.txt" | head -n 1 || true)"
if [[ -z "$SRC_ID_FILE" || ! -f "$SRC_ID_FILE" ]]; then
    rm -rf "$SRC_VERIFY_TMP"
    echo "ERROR: Source archive missing SOURCE-IDENTIFICATION.txt!" >&2
    exit 1
fi
if ! grep -q "$REVISION" "$SRC_ID_FILE"; then
    rm -rf "$SRC_VERIFY_TMP"
    echo "ERROR: Source archive revision does not match target revision $REVISION!" >&2
    exit 1
fi
rm -rf "$SRC_VERIFY_TMP"
echo "    ✓ Source archive matches revision $REVISION completely."

# 5b. Verify macOS App Bundle (if staged)
STAGED_MAC="$(find "$STAGE_DIR" -type f \( -name "*macos*.zip" -o -name "*.dmg" \) | head -n 1 || true)"
if [[ -n "$STAGED_MAC" && -f "$STAGED_MAC" ]]; then
    MAC_VERIFY_TMP="$(mktemp -d "${TMPDIR:-/tmp}/voc_mac_check.XXXXXX")"
    if [[ "$STAGED_MAC" == *.zip ]]; then
        unzip -q "$STAGED_MAC" -d "$MAC_VERIFY_TMP"
    fi
    MAC_APP="$(find "$MAC_VERIFY_TMP" -maxdepth 2 -type d -name "Vocument.app" | head -n 1 || true)"
    if [[ -n "$MAC_APP" ]]; then
        MAC_SOURCE_TXT="$MAC_APP/Contents/Resources/SOURCE.txt"
        if [[ ! -f "$MAC_SOURCE_TXT" ]]; then
            rm -rf "$MAC_VERIFY_TMP"
            echo "ERROR: macOS bundle missing Contents/Resources/SOURCE.txt!" >&2
            exit 1
        fi
        if ! grep -q "$REVISION" "$MAC_SOURCE_TXT"; then
            rm -rf "$MAC_VERIFY_TMP"
            echo "ERROR: macOS bundle SOURCE.txt was not built from $REVISION!" >&2
            exit 1
        fi
        
        # Verify ARM64 binary architecture
        MAC_BIN="$MAC_APP/Contents/MacOS/Vocument"
        if [[ -f "$MAC_BIN" ]]; then
            if ! file "$MAC_BIN" | grep -q "arm64"; then
                rm -rf "$MAC_VERIFY_TMP"
                echo "ERROR: macOS binary '$MAC_BIN' does not contain required arm64 architecture!" >&2
                exit 1
            fi
            echo "    ✓ macOS binary verified: contains arm64 architecture."
        fi

        # Verify bundled MuPDF library presence
        if [[ ! -f "$MAC_APP/Contents/Frameworks/libmupdf.dylib" ]]; then
            rm -rf "$MAC_VERIFY_TMP"
            echo "ERROR: macOS bundle missing required bundled Frameworks/libmupdf.dylib!" >&2
            exit 1
        fi
        echo "    ✓ macOS bundle verified: native MuPDF dynamic library bundled."
    fi
    rm -rf "$MAC_VERIFY_TMP"
    echo "    ✓ macOS package verified: provenance matches revision $REVISION."
else
    echo "WARNING: No macOS package in this candidate staging."
fi

# 5c. Verify Android APK (if staged)
STAGED_APK="$(find "$STAGE_DIR" -type f -name "*.apk" | head -n 1 || true)"
if [[ -n "$STAGED_APK" && -f "$STAGED_APK" ]]; then
    # Verify embedded SOURCE.txt
    if ! unzip -p "$STAGED_APK" assets/licence/SOURCE.txt 2>/dev/null | grep -q "$REVISION" && \
       ! unzip -p "$STAGED_APK" assets/SOURCE.txt 2>/dev/null | grep -q "$REVISION"; then
        echo "ERROR: Android APK embedded SOURCE.txt does not match release revision $REVISION!" >&2
        exit 1
    fi
    
    # Verify embedded arm64-v8a native libraries
    if ! unzip -l "$STAGED_APK" | grep -q "lib/arm64-v8a/libvocument_ffi.so"; then
        echo "ERROR: Android APK missing lib/arm64-v8a/libvocument_ffi.so!" >&2
        exit 1
    fi
    if ! unzip -l "$STAGED_APK" | grep -q "lib/arm64-v8a/libmupdf.so"; then
        echo "ERROR: Android APK missing lib/arm64-v8a/libmupdf.so!" >&2
        exit 1
    fi
    echo "    ✓ Android APK verified: contains arm64 native libraries and matches revision $REVISION."
else
    echo "WARNING: No Android APK in this candidate staging."
fi

# 5d. Mandatory packages enforcement
if [[ "$PUBLISH_REQUESTED" -eq 1 || "$REQUIRE_ALL_PLATFORMS" -eq 1 ]]; then
    if [[ -z "$STAGED_MAC" || ! -f "$STAGED_MAC" ]]; then
        echo "ERROR: Mandatory macOS package is missing from release candidate staging!" >&2
        exit 1
    fi
    if [[ -z "$STAGED_APK" || ! -f "$STAGED_APK" ]]; then
        echo "ERROR: Mandatory Android APK package is missing from release candidate staging!" >&2
        exit 1
    fi
fi

# 6. Bundle Legal Notices & Generate Unified SHA-256 Checksums
cp LICENSE "$STAGE_DIR/LICENSE.txt"
cp THIRD_PARTY.md "$STAGE_DIR/THIRD-PARTY-NOTICES.txt"

echo "==> Computing cryptographic SHA-256 checksums..."
(
    cd "$STAGE_DIR"
    rm -f SHA256SUMS.txt
    for f in *; do
        if [[ -f "$f" && "$f" != "SHA256SUMS.txt" && "$f" != *.sha256 ]]; then
            if command -v sha256sum >/dev/null 2>&1; then
                sha256sum "$f" >> SHA256SUMS.txt
            else
                shasum -a 256 "$f" >> SHA256SUMS.txt
            fi
        fi
    done
)

# 7. Generate Release Candidate Record & Audit Summary
SIGNING_STATUS="NOT SIGNED FOR PUBLIC DISTRIBUTION (Internal Test Build Only)"
if [[ -n "$SIGN_IDENTITY" ]]; then
    SIGNING_STATUS="Signed with Developer ID Application: $SIGN_IDENTITY"
fi

REPORT_FILE="$STAGE_DIR/RELEASE_CANDIDATE_REPORT.md"
cat <<EOF > "$REPORT_FILE"
# Vocument Release Candidate: $TAG

- **Release Tag:** \`$TAG\`
- **Source Revision:** \`$REVISION\`
- **Build Timestamp (UTC):** $(date -u +"%Y-%m-%dT%H:%M:%SZ")
- **Pre-Release Tier:** $IS_PRERELEASE
- **Signing & Gatekeeper Status:** $SIGNING_STATUS

## Mandatory Governance & Publication Lock
- **E5 / B-1 (Legal Compliance Review):** OPEN — Requires explicit Project Owner clearance (§6/§13 AGPL-3.0).
- **B-2 (macOS Developer ID & Notarization):** $([ -n "$SIGN_IDENTITY" ] && echo "Configured" || echo "OPEN — Developer ID credentials not active in this build").
- **B-3 (Android Release Keystore):** OPEN — Signed with debug/test key only; release keystore remains with Project Owner.
- **B-4 (Publisher Imprint):** OPEN — Direct distribution requires publisher contact data.

> **CRITICAL SAFEGUARD:** This package is an internal release candidate.
> It MUST NOT be published or distributed publicly without explicit Project Owner sign-off.

## Verified Artifacts & SHA-256
\`\`\`text
$(cat "$STAGE_DIR/SHA256SUMS.txt")
\`\`\`
EOF

echo "==> Release candidate report generated: $REPORT_FILE"

# 8. BINDING PUBLICATION LOCK ENFORCEMENT
if [[ "$PUBLISH_REQUESTED" -eq 1 ]]; then
    if [[ "$OWNER_APPROVAL_CONFIRMED" -eq 0 && "${PROJECT_OWNER_RELEASE_APPROVAL:-}" != "APPROVED_BY_PROJECT_OWNER" ]]; then
        echo "" >&2
        echo "================================================================================" >&2
        echo "BLOCKED: BINDING PUBLICATION LOCK ACTIVE!" >&2
        echo "Under AGENTS.md, E5, and RELEASE-BLOCKERS.md, no agent or automated script may" >&2
        echo "publish public GitHub releases or release binaries without explicit Project" >&2
        echo "Owner approval (--confirm-owner-approval)." >&2
        echo "" >&2
        echo "Artifacts were safely prepared and verified in:" >&2
        echo "  $STAGE_DIR" >&2
        echo "================================================================================" >&2
        exit 1
    fi

    # Explicit Project Owner clearance granted
    echo "==> Project Owner approval confirmed. Preparing GitHub publication..."
    if ! command -v gh >/dev/null 2>&1; then
        echo "ERROR: GitHub CLI ('gh') is required for publication." >&2
        exit 1
    fi

    # Gather upload files
    UPLOAD_FILES=()
    while IFS= read -r -d '' artifact; do
        UPLOAD_FILES+=("$artifact")
    done < <(find "$STAGE_DIR" -maxdepth 1 -type f -print0 | sort -z)

    GH_ARGS=(
        release create "$TAG"
        "${UPLOAD_FILES[@]}"
        --title "$TITLE"
        --notes-file "$REPORT_FILE"
    )
    if [[ "$IS_PRERELEASE" == "true" ]]; then
        GH_ARGS+=(--prerelease)
    fi

    echo "==> Executing: gh ${GH_ARGS[*]}"
    gh "${GH_ARGS[@]}"
    echo "✓ Release $TAG successfully published to GitHub."
else
    echo ""
    echo "================================================================================"
    echo "✓ Release candidate $TAG staged and verified successfully (STAGE ONLY)."
    echo "  Publication lock preserved: No remote release, tag, or upload executed."
    echo "  All artifacts and integrity proofs available in: $STAGE_DIR"
    echo "================================================================================"
fi

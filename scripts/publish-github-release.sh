#!/usr/bin/env bash
# Publish a Vocument release to GitHub Releases with source and compiled binaries.
#
# Follows AGPL-3.0 compliance rules (DIST-001, DIST-002, E1-E5):
# 1. Publishes the source archive corresponding to the exact revision.
# 2. Attaches compiled platform binaries (macOS .zip/.dmg, Android .apk).
# 3. Generates and attaches SHA-256 checksums (SHA256SUMS.txt) and third-party notices.
# 4. Supports Alpha, Beta (pre-release) and Stable (production GA) releases.
#
# Usage:
#   scripts/publish-github-release.sh --tag <tag> [options]
#
# Options:
#   --tag <tag>              Git tag to release (e.g., v0.3.0-alpha.2, v0.9.0-beta.1, v1.0.0)
#   --title <string>         Release title (defaults to "Vocument <tag>")
#   --notes <string>         Release notes content
#   --notes-file <file>      Path to release notes Markdown file
#   --prerelease             Mark as GitHub pre-release (default for alpha/beta tags)
#   --stable                 Mark as stable production release (override prerelease)
#   --draft                  Create as draft release
#   --skip-build             Do not build artifacts; upload existing files in target/
#   --dry-run                Assemble artifacts and verify without publishing to GitHub
#   --help, -h               Show this help message

set -euo pipefail

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPOSITORY_ROOT"

TAG=""
TITLE=""
NOTES=""
NOTES_FILE=""
PRERELEASE=""
IS_DRAFT=0
SKIP_BUILD=0
DRY_RUN=0

usage() {
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
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
        --notes)
            NOTES="$2"
            shift 2
            ;;
        --notes-file)
            NOTES_FILE="$2"
            shift 2
            ;;
        --prerelease)
            PRERELEASE="true"
            shift
            ;;
        --stable)
            PRERELEASE="false"
            shift
            ;;
        --draft)
            IS_DRAFT=1
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --dry-run)
            DRY_RUN=1
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
    echo "ERROR: --tag is mandatory (e.g., --tag v0.3.0-alpha.2)" >&2
    exit 1
fi

if [[ -z "$TITLE" ]]; then
    TITLE="Vocument $TAG"
fi

# Auto-detect prerelease for alpha/beta tags if not explicitly given
if [[ -z "$PRERELEASE" ]]; then
    if [[ "$TAG" =~ (alpha|beta|rc|preview) ]]; then
        PRERELEASE="true"
    else
        PRERELEASE="false"
    fi
fi

# Check GitHub CLI availability
if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: GitHub CLI ('gh') is required to publish releases." >&2
    echo "       Install via 'brew install gh' or your package manager." >&2
    exit 1
fi

if [[ "$DRY_RUN" -eq 0 ]]; then
    if ! gh auth status >/dev/null 2>&1; then
        echo "ERROR: 'gh' is not authenticated. Run 'gh auth login' first." >&2
        exit 1
    fi
fi

STAGE_DIR="$REPOSITORY_ROOT/target/github-release/$TAG"
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR"

echo "==> Preparing GitHub Release: $TAG ($TITLE)"
echo "    Pre-release status: $PRERELEASE"

# 1. Build or locate artifacts
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    echo "==> Building source archive..."
    bash scripts/release-source-archive.sh --output "$STAGE_DIR"

    # Build macOS bundle if on macOS host
    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "==> Building macOS application bundle..."
        if [[ -n "${SIGN_IDENTITY:-}" ]]; then
            bash scripts/release-macos.sh --sign "$SIGN_IDENTITY" --output "$STAGE_DIR"
        else
            echo "    (No SIGN_IDENTITY set; assembling unsigned macOS bundle via --dry-run)"
            bash scripts/release-macos.sh --dry-run --output "$STAGE_DIR"
        fi
    fi

    # Build Android package if SDK is present
    if [[ -n "${ANDROID_HOME:-}" || -d "$HOME/Library/Android/sdk" || -d "/opt/homebrew/share/android-commandlinetools" ]]; then
        echo "==> Building Android package..."
        bash scripts/release-android.sh --apk --output "$STAGE_DIR" || {
            echo "WARNING: Android build was skipped or failed in this environment." >&2
        }
    fi
else
    REVISION="$(git rev-parse HEAD 2>/dev/null || echo '')"
    SHORT_REV="${REVISION:0:12}"
    if [[ -n "$SHORT_REV" && -f "target/release-source/vocument-0.1.0-source-${SHORT_REV}.tar.gz" ]]; then
        cp "target/release-source/vocument-0.1.0-source-${SHORT_REV}.tar.gz" "$STAGE_DIR/"
    else
        NEWEST_SRC="$(find target/release-source -type f -name "*.tar.gz" 2>/dev/null | sort -r | head -n 1 || true)"
        if [[ -n "$NEWEST_SRC" ]]; then
            cp "$NEWEST_SRC" "$STAGE_DIR/"
        fi
    fi
    find target/release-macos target/release-android \
        -maxdepth 1 -type f \( -name "*.zip" -o -name "*.apk" -o -name "*.dmg" -o -name "*.aab" \) \
        -exec cp {} "$STAGE_DIR/" \; 2>/dev/null || true
fi

# 2. Bundle legal notices & licenses
cp LICENSE "$STAGE_DIR/LICENSE.txt" 2>/dev/null || true
if [[ -f "THIRD_PARTY.md" ]]; then
    cp THIRD_PARTY.md "$STAGE_DIR/THIRD-PARTY-NOTICES.txt"
fi

# 3. Generate unified SHA-256 checksums
echo "==> Generating SHA256SUMS.txt..."
(
    cd "$STAGE_DIR"
    rm -f SHA256SUMS.txt
    for file in *; do
        if [[ -f "$file" && "$file" != "SHA256SUMS.txt" && "$file" != *.sha256 ]]; then
            if command -v shasum >/dev/null 2>&1; then
                shasum -a 256 "$file" >> SHA256SUMS.txt
            else
                sha256sum "$file" >> SHA256SUMS.txt
            fi
        fi
    done
)

# 4. Prepare notes file
FINAL_NOTES_FILE="$STAGE_DIR/RELEASE_NOTES.md"
if [[ -n "$NOTES_FILE" && -f "$NOTES_FILE" ]]; then
    cp "$NOTES_FILE" "$FINAL_NOTES_FILE"
elif [[ -n "$NOTES" ]]; then
    printf "%s\n" "$NOTES" > "$FINAL_NOTES_FILE"
else
    cat <<EOF > "$FINAL_NOTES_FILE"
# Vocument $TAG

## Release Highlights
- Release tier: $([ "$PRERELEASE" == "true" ] && echo "Pre-release (Alpha/Beta)" || echo "Production General Availability (Stable)")
- Native cross-platform PDF & FlowDocument authoring workbench.
- Deterministic typesetting & pagination engine with OpenType fonts.
- Strictly offline-first, local-first architecture with zero telemetry.

## Included Artifacts & Binaries
See \`SHA256SUMS.txt\` for cryptographic integrity verification of all attached packages.
EOF
fi

# 5. Collect all files to attach
ATTACHMENTS=()
while IFS= read -r -d '' artifact; do
    ATTACHMENTS+=("$artifact")
done < <(find "$STAGE_DIR" -maxdepth 1 -type f -not -name "RELEASE_NOTES.md" -print0 | sort -z)

echo "==> Artifacts prepared for upload (${#ATTACHMENTS[@]} files):"
for f in "${ATTACHMENTS[@]}"; do
    echo "    - $(basename "$f") ($(wc -c < "$f" | tr -d ' ') bytes)"
done

# 6. Publish via GitHub CLI
GH_ARGS=(
    release create "$TAG"
    "${ATTACHMENTS[@]}"
    --title "$TITLE"
    --notes-file "$FINAL_NOTES_FILE"
)

if [[ "$PRERELEASE" == "true" ]]; then
    GH_ARGS+=(--prerelease)
fi

if [[ "$IS_DRAFT" -eq 1 ]]; then
    GH_ARGS+=(--draft)
fi

if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "==> [DRY RUN] Would execute:"
    echo "    gh ${GH_ARGS[*]}"
    echo "==> Artifacts assembled successfully in: $STAGE_DIR"
    exit 0
fi

echo "==> Publishing release to GitHub..."
gh "${GH_ARGS[@]}"

echo "✓ Release $TAG successfully published to GitHub Releases!"

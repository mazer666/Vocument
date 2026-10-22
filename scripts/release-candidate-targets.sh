#!/usr/bin/env bash
# The checks of the application targets of a release candidate.
#
# The three targets of the product — the Rust library, the macOS application
# bundle and the Android package — are the artefacts that a release hands
# out. This script measures, for every one of them, what could be done in
# this environment and what could not, and writes the result down.
#
# The states:
#
#   PASS       the check was carried out and it succeeded
#   FAIL       the check was carried out and it did not succeed
#   NOT RUN    the check could not be carried out in this environment (a
#              missing toolchain, a missing host) — not a defect of the product
#   BLOCKED   the check cannot be carried out by the team at all: it needs a
#              decision, an identity or a key that only the project owner has
#
# The rows are written to the file that is named as the first argument, one
# row per check, the fields separated by a tab:
#
#   name, command, result, reason, evidence
#
# Usage: scripts/release-candidate-targets.sh <the file of the checks>

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <the file of the checks>" >&2
    exit 2
fi
CHECKS_FILE="$1"

CANDIDATE_DIRECTORY="$REPO_ROOT/target/release-candidate"

row() {
    # name, command, result, reason, evidence — the fields must not carry a
    # tab or a newline.
    printf '%s\t%s\t%s\t%s\t%s\n' \
        "${1//$'\t'/ }" "${2//$'\t'/ }" "${3//$'\t'/ }" \
        "${4//$'\t'/ }" "${5//$'\t'/ }" >> "$CHECKS_FILE"
}

# --- The macOS application ------------------------------------------------
# The bundle needs the macOS SDK to be built and an Apple identity to be
# signed. The first is a limit of the environment, the second is the work of
# the project owner.
if [[ "$(uname -s)" != "Darwin" ]]; then
    MACOS_REASON="no macOS host and therefore no macOS SDK in this environment — SwiftUI, AppKit and CoreGraphics do not exist for Linux. The build of the application is NOT RUN; the commands are in scripts/release-macos.sh"
else
    MACOS_REASON="the build of the application was not part of this candidate"
fi
row "the macOS application bundle is built" \
    "scripts/release-macos.sh --dry-run" \
    "NOT RUN" "$MACOS_REASON" \
    "the sources of the target are in the archive (apps/macos), the checksums of the generated Swift bindings are in the build record"

row "the macOS bundle is signed and notarised" \
    "codesign --verify --deep --strict; xcrun notarytool submit --wait" \
    "BLOCKED" \
    "no Apple identity and no notarisation credentials in this environment — B-2 of docs/releasing/RELEASE-BLOCKERS.md. The signing identity and the credentials are the work of the project owner" \
    "the commands are in scripts/release-macos.sh; the signature and the ticket are NOT RUN"

# --- The Android package --------------------------------------------------
# The package needs a JDK and the Android SDK for the build, and the release
# key for the signature.
ANDROID_SDK_DIRECTORY="${ANDROID_HOME:-$REPO_ROOT/target/android-sdk}"
if ! command -v java >/dev/null 2>&1; then
    ANDROID_REASON="no JDK (java) in this environment — Gradle cannot run. The build of the package is NOT RUN; the commands are in scripts/release-android.sh"
elif [[ ! -d "$ANDROID_SDK_DIRECTORY" ]]; then
    ANDROID_REASON="no Android SDK in this environment ($ANDROID_SDK_DIRECTORY). The build of the package is NOT RUN; the commands are in scripts/release-android.sh"
else
    ANDROID_REASON="the build of the package was not part of this candidate"
fi
row "the Android package is built" \
    "scripts/release-android.sh --apk" \
    "NOT RUN" "$ANDROID_REASON" \
    "the sources of the target are in the archive (apps/android), the checksums of the generated Kotlin bindings are in the build record"

row "the Android package is signed with the release key" \
    "the signing of the release variant (Gradle)" \
    "BLOCKED" \
    "no release key in this environment — B-3 of docs/releasing/RELEASE-BLOCKERS.md. The key belongs to the project owner and never into the repository" \
    "the signing is left to Gradle; the signature of a release build is NOT RUN"

# --- The legal clearance --------------------------------------------------
# E5 is the precondition of every publication. It is the work of a person:
# no script of this repository can give it, and this candidate does not
# pretend otherwise.
row "the legal compliance review (E5) is given" \
    "the review of a person who is entitled to give it" \
    "BLOCKED" \
    "E5 of docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md is open — B-1 of docs/releasing/RELEASE-BLOCKERS.md. The review is the work of the project owner, with legal support; no agent can give it" \
    "the preparation is done: the archive of the source, the offer in the bundle and in the package, the identification of the revision"

# --- The publication ------------------------------------------------------
# The candidate holds no package of a store and no signed bundle. The check
# is a measurement of the directory of the candidate.
PUBLISHABLE_ARTEFACTS="$(find "$CANDIDATE_DIRECTORY" \
    \( -name '*.apk' -o -name '*.aab' -o -name '*.dmg' -o -name '*.pkg' \
       -o -name '*.app' -o -name '*.ipa' -o -name '*.msi' -o -name '*.exe' \) \
    2>/dev/null | wc -l | tr -d ' ')"
if [[ "$PUBLISHABLE_ARTEFACTS" == "0" ]]; then
    row "the candidate is not published" \
        "find <the candidate> \\( -name '*.apk' -o -name '*.aab' -o -name '*.dmg' -o -name '*.pkg' -o -name '*.app' \\)" \
        "PASS" "" \
        "the candidate holds no package of a store and no signed bundle ($PUBLISHABLE_ARTEFACTS found); this script uploads nothing, tags nothing and announces nothing"
else
    row "the candidate is not published" \
        "find <the candidate> \\( -name '*.apk' -o -name '*.aab' -o -name '*.dmg' -o -name '*.pkg' -o -name '*.app' \\)" \
        "FAIL" \
        "the candidate holds $PUBLISHABLE_ARTEFACTS file(s) that could be handed out — a release candidate holds the source, not a package" \
        ""
fi

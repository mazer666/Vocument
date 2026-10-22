#!/usr/bin/env bash
# The licence checks of a release candidate.
#
# Two things have to be true before a binary is handed out:
#
# 1. **The licence of every component is known and agrees with the AGPL-3.0**
#    (E1 of the licensing decision). The audit reads the licence of every
#    component out of the metadata of the component itself — the manifest of
#    the crate in the cargo registry, the POM of the library in the Gradle
#    cache. A component whose licence is not known is a finding, not a
#    footnote.
# 2. **The notices of the third-party components travel with the product.**
#    The texts are assembled out of the texts of the repository — a licence
#    text that had to be fetched from the network at the moment of the
#    release would be missing when the network is down.
#
# The checks are written to the file that is named as the first argument, one
# row per check, the fields separated by a tab:
#
#   name, command, result, reason, evidence
#
# Usage: scripts/release-candidate-licences.sh <the file of the checks> <the candidate>

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [[ $# -lt 2 ]]; then
    echo "usage: $0 <the file of the checks> <the candidate directory>" >&2
    exit 2
fi
CHECKS_FILE="$1"
CANDIDATE_DIRECTORY="$2"

row() {
    printf '%s\t%s\t%s\t%s\t%s\n' \
        "${1//$'\t'/ }" "${2//$'\t'/ }" "${3//$'\t'/ }" \
        "${4//$'\t'/ }" "${5//$'\t'/ }" >> "$CHECKS_FILE"
}

sha256_of() {
    if [[ -f "$1" ]]; then
        sha256sum "$1" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "$1" | cut -d' ' -f1
    else
        echo "not-available"
    fi
}

LICENCES_DIRECTORY="$CANDIDATE_DIRECTORY/licences"
mkdir -p "$LICENCES_DIRECTORY"

# --- The audit of the licences --------------------------------------------
echo "==> Auditing the licences of every component..."
LICENCE_LOG="$CANDIDATE_DIRECTORY/logs/licence-audit.log"
set +e
bash scripts/audit-licenses.sh --report "$LICENCES_DIRECTORY/licence-inventory.md" \
    > "$LICENCE_LOG" 2>&1
LICENCE_EXIT=$?
set -e
if [[ "$LICENCE_EXIT" -eq 0 ]]; then
    row "the audit of the licences of every component" \
        "scripts/audit-licenses.sh --report licences/licence-inventory.md" \
        "PASS" "" "the inventory of the licences is in licences/licence-inventory.md"
else
    row "the audit of the licences of every component" \
        "scripts/audit-licenses.sh --report licences/licence-inventory.md" \
        "FAIL" "the audit failed (exit code $LICENCE_EXIT) — the log is in logs/licence-audit.log" ""
fi

# --- The notices of the third-party components ----------------------------
echo "==> Assembling the notices of the third-party components..."
NOTICES_LOG="$CANDIDATE_DIRECTORY/logs/third-party-notices.log"
CANDIDATE_NOTICES="$LICENCES_DIRECTORY/THIRD-PARTY-NOTICES.txt"
set +e
bash scripts/generate-third-party-notices.sh --output "$CANDIDATE_NOTICES" \
    > "$NOTICES_LOG" 2>&1
NOTICES_EXIT=$?
set -e
if [[ "$NOTICES_EXIT" -eq 0 ]]; then
    row "the notices of the third-party components were assembled" \
        "scripts/generate-third-party-notices.sh --output licences/THIRD-PARTY-NOTICES.txt" \
        "PASS" "" "the notices are in licences/THIRD-PARTY-NOTICES.txt"
else
    row "the notices of the third-party components were assembled" \
        "scripts/generate-third-party-notices.sh --output licences/THIRD-PARTY-NOTICES.txt" \
        "FAIL" "the notices could not be assembled (exit code $NOTICES_EXIT) — the log is in logs/third-party-notices.log" ""
fi

# The notices of the candidate have to be the notices of the repository: a
# candidate that carries other texts than the product would be a candidate
# that hides a difference.
REPOSITORY_NOTICES="LICENSES/THIRD-PARTY-NOTICES.txt"
if [[ -f "$CANDIDATE_NOTICES" && -f "$REPOSITORY_NOTICES" ]]; then
    if [[ "$(sha256_of "$CANDIDATE_NOTICES")" == "$(sha256_of "$REPOSITORY_NOTICES")" ]]; then
        row "the notices of the candidate are the notices of the repository" \
            "sha256sum licences/THIRD-PARTY-NOTICES.txt" \
            "PASS" "" "both files have the checksum $(sha256_of "$CANDIDATE_NOTICES")"
    else
        row "the notices of the candidate are the notices of the repository" \
            "sha256sum licences/THIRD-PARTY-NOTICES.txt" \
            "FAIL" "the notices of the candidate and the notices of the repository are different" ""
    fi
else
    row "the notices of the candidate are the notices of the repository" \
        "sha256sum licences/THIRD-PARTY-NOTICES.txt" \
        "NOT RUN" "the notices were not assembled — there is nothing to compare" ""
fi

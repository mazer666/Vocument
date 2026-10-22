#!/usr/bin/env bash
# The check of a release candidate of Vocument.
#
# The candidate is built by `scripts/release-candidate.sh`. This script checks
# it — **internally**: it looks at the candidate, it does not publish it,
# and it does not hand anything out.
#
# Four things are checked:
#
# 1. **The archive carries the revision completely.** The archive is unpacked
#    and compared with the object database of the repository: the list of the
#    files with `git ls-tree`, the content of every file with
#    `git hash-object`. The check is deliberately independent of the builder
#    (`scripts/release-source-archive.sh`): the builder proves the archive
#    when it builds it, this script proves it again afterwards.
# 2. **The checksums are right.** The checksum of every artefact of the
#    manifest is computed again and compared with the manifest.
# 3. **The manifest claims nothing that was not measured.** Every check has
#    a state of the vocabulary, every state that is not `PASS` has a reason,
#    and only the source archive may claim to be bit-identical — and only
#    with two measured checksums behind it.
# 4. **The candidate is internal.** The manifest says that it is not
#    published, the legal clearance (E5) is named as missing, and the
#    candidate holds no package of a store and no signed bundle.
#
# The reproducibility of the archive is measured once more here: the archive
# is built a third time and compared with the two builds of the manifest.
#
# Usage: scripts/verify-release-candidate.sh [--candidate <directory>]
#                                          [--revision <sha>]
#
# Exit code 0: the candidate holds up.
# Exit code 1: a check of the candidate failed.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

CANDIDATE_DIRECTORY="$REPO_ROOT/target/release-candidate"
REVISION_ARGUMENT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --candidate) CANDIDATE_DIRECTORY="$2"; shift 2 ;;
        --revision) REVISION_ARGUMENT="$2"; shift 2 ;;
        *) echo "usage: $0 [--candidate <directory>] [--revision <sha>]" >&2; exit 2 ;;
    esac
done

MANIFEST_JSON="$CANDIDATE_DIRECTORY/manifest.json"
MANIFEST_MARKDOWN="$CANDIDATE_DIRECTORY/MANIFEST.md"
REPORT_DOCUMENT="docs/releasing/RELEASE-CANDIDATE.md"

FAILURES=0
pass() {
    echo "✓ $1"
}
fail() {
    echo "ERROR: $1" >&2
    FAILURES=$((FAILURES + 1))
}

sha256_of() {
    if [[ -f "$1" ]]; then
        sha256sum "$1" 2>/dev/null | cut -d' ' -f1 || shasum -a 256 "$1" | cut -d' ' -f1
    else
        echo "not-available"
    fi
}

manifest_value() {
    # A value of the manifest, read with python: the manifest is JSON.
    python3 -c 'import json, sys
manifest = json.load(open(sys.argv[1], encoding="utf-8"))
node = manifest
for part in sys.argv[2].split("."):
    node = node.get(part) if isinstance(node, dict) else None
    if node is None:
        break
print(node if node is not None else "not-available")' "$MANIFEST_JSON" "$1" 2>/dev/null \
        || echo "not-available"
}

manifest_rows() {
    # The rows of a list of the manifest, tab separated:
    #   artefacts: name, path, sha256, bytes, kind, reproducibility
    #   checks:    name, command, result, reason, evidence
    python3 -c 'import json, sys
manifest = json.load(open(sys.argv[1], encoding="utf-8"))
for entry in manifest.get(sys.argv[2], []):
    if sys.argv[2] == "artifacts":
        fields = [entry.get("name", ""), entry.get("path", ""), entry.get("sha256", ""),
                  str(entry.get("bytes", "")), entry.get("kind", ""),
                  entry.get("reproducibility", "")]
    else:
        fields = [entry.get("name", ""), entry.get("command", ""), entry.get("result", ""),
                  entry.get("reason", ""), entry.get("evidence", "")]
    print("\t".join(str(field).replace("\t", " ").replace("\n", " ") for field in fields))' \
        "$MANIFEST_JSON" "$1"
}

echo "==> Checking the release candidate in $CANDIDATE_DIRECTORY"

# --- 1. The candidate and its manifest ------------------------------------
if [[ ! -f "$MANIFEST_JSON" ]]; then
    echo "ERROR: the candidate has no manifest ($MANIFEST_JSON)." >&2
    echo "       Build it first: scripts/release-candidate.sh" >&2
    exit 1
fi
if [[ ! -f "$MANIFEST_MARKDOWN" ]]; then
    fail "the candidate has no readable manifest ($MANIFEST_MARKDOWN)."
else
    pass "the candidate has a manifest."
fi

REVISION="$(manifest_value "revision")"
if [[ -z "$REVISION" || "$REVISION" == "not-available" ]]; then
    fail "the manifest names no revision."
    exit 1
fi
if [[ -n "$REVISION_ARGUMENT" ]] && [[ "$REVISION_ARGUMENT" != "$REVISION" ]]; then
    fail "the manifest is about the revision $REVISION and not $REVISION_ARGUMENT."
fi
if ! git rev-parse --verify "${REVISION}^{commit}" >/dev/null 2>&1; then
    fail "the revision $REVISION of the manifest is not a commit of this repository."
    exit 1
fi
pass "the manifest is about the revision $REVISION."

# --- 2. The candidate is internal -----------------------------------------
PUBLISHED="$(manifest_value "publication.published")"
CLEARANCE="$(manifest_value "publication.legal_clearance_e5")"
NOTICE="$(manifest_value "publication.notice")"
if [[ "$PUBLISHED" == "False" || "$PUBLISHED" == "false" ]]; then
    pass "the manifest says that the candidate is not published."
else
    fail "the manifest does not say that the candidate is not published (published: $PUBLISHED)."
fi
if [[ "$CLEARANCE" == "missing" ]]; then
    pass "the manifest names the legal clearance (E5) as missing."
else
    fail "the manifest does not name the legal clearance (E5) as missing (it says: $CLEARANCE)."
fi
if [[ -n "$NOTICE" ]] && grep -qF "$NOTICE" "$MANIFEST_MARKDOWN" 2>/dev/null; then
    pass "the readable manifest carries the notice that the candidate is not published."
else
    fail "the readable manifest does not carry the notice that the candidate is not published."
fi
if [[ -f "$REPORT_DOCUMENT" ]] && grep -qiF "not published" "$REPORT_DOCUMENT"; then
    pass "the report of the candidate ($REPORT_DOCUMENT) says that the candidate is not published."
else
    fail "the report of the candidate ($REPORT_DOCUMENT) is missing or does not say that the candidate is not published."
fi
PUBLISHABLE_ARTEFACTS="$(find "$CANDIDATE_DIRECTORY" \
    \( -name '*.apk' -o -name '*.aab' -o -name '*.dmg' -o -name '*.pkg' \
       -o -name '*.app' -o -name '*.ipa' -o -name '*.msi' -o -name '*.exe' \) \
    2>/dev/null | wc -l | tr -d ' ')"
if [[ "$PUBLISHABLE_ARTEFACTS" == "0" ]]; then
    pass "the candidate holds no package of a store and no signed bundle."
else
    fail "the candidate holds $PUBLISHABLE_ARTEFACTS file(s) that could be handed out."
fi

# --- 3. The checksums of the artefacts ------------------------------------
ARTIFACT_COUNT=0
while IFS=$'\t' read -r name path digest bytes kind reproducibility; do
    [[ -z "$name" ]] && continue
    ARTIFACT_COUNT=$((ARTIFACT_COUNT + 1))
    if [[ -z "$path" || ! -f "$path" ]]; then
        fail "the artefact '$name' is not there: $path"
        continue
    fi
    ACTUAL_DIGEST="$(sha256_of "$path")"
    if [[ "$ACTUAL_DIGEST" != "$digest" ]]; then
        fail "the checksum of '$name' does not match the manifest ($ACTUAL_DIGEST instead of $digest)."
    else
        ACTUAL_BYTES="$(wc -c < "$path" | tr -d ' ')"
        if [[ "$ACTUAL_BYTES" != "$bytes" ]]; then
            fail "the size of '$name' does not match the manifest ($ACTUAL_BYTES instead of $bytes)."
        fi
    fi
done < <(manifest_rows "artifacts")
if [[ "$ARTIFACT_COUNT" -eq 0 ]]; then
    fail "the manifest names no artefact."
else
    pass "the checksums of the $ARTIFACT_COUNT artefact(s) of the manifest match."
fi

# --- 4. The claims of the manifest ----------------------------------------
# A manifest that claims something that was not measured is a manifest that
# lies. Every check needs a state of the vocabulary, every state that is not
# `PASS` needs a reason, and only the source archive may claim to be
# bit-identical.
VALID_RESULTS=("PASS" "FAIL" "NOT RUN" "BLOCKED")
while IFS=$'\t' read -r name command result reason evidence; do
    [[ -z "$name" ]] && continue
    VALID=0
    for allowed in "${VALID_RESULTS[@]}"; do
        [[ "$result" == "$allowed" ]] && VALID=1
    done
    if [[ "$VALID" -eq 0 ]]; then
        fail "the check '$name' has the state '$result', which is not one of ${VALID_RESULTS[*]}."
    fi
    if [[ "$result" != "PASS" && -z "$reason" ]]; then
        fail "the check '$name' is '$result' and names no reason."
    fi
    if [[ "$result" == "PASS" && -z "$evidence" && -z "$reason" ]]; then
        fail "the check '$name' is 'PASS' and names neither a reason nor an evidence."
    fi
done < <(manifest_rows "checks")

APPLICATIONS_CLAIM="$(manifest_value "reproducibility.applications.bit_identical")"
APPLICATIONS_REASON="$(manifest_value "reproducibility.applications.reason")"
if [[ "$APPLICATIONS_CLAIM" == "not-claimed" ]]; then
    pass "the manifest does not claim that the applications are bit-identical."
else
    fail "the manifest claims something for the applications (bit_identical: $APPLICATIONS_CLAIM) that was not measured."
fi
if [[ -n "$APPLICATIONS_REASON" ]]; then
    pass "the manifest names why the applications are not claimed to be bit-identical."
else
    fail "the manifest does not say why the applications are not claimed to be bit-identical."
fi

ARCHIVE_FIRST="$(manifest_value "reproducibility.source_archive.sha256_first")"
ARCHIVE_SECOND="$(manifest_value "reproducibility.source_archive.sha256_second")"
ARCHIVE_CLAIM="$(manifest_value "reproducibility.source_archive.bit_identical")"
if [[ "$ARCHIVE_CLAIM" == "True" || "$ARCHIVE_CLAIM" == "true" ]]; then
    if [[ "$ARCHIVE_FIRST" == "$ARCHIVE_SECOND" && "$ARCHIVE_FIRST" != "not-available" ]]; then
        pass "the manifest names two equal checksums for the source archive."
    else
        fail "the manifest claims that the source archive is bit-identical, but the two checksums are not equal."
    fi
else
    fail "the manifest does not claim that the source archive is bit-identical — the build of the candidate should have measured it."
fi

# Only the source archive may claim to be bit-identical.
while IFS=$'\t' read -r name path digest bytes kind reproducibility; do
    [[ -z "$name" ]] && continue
    if [[ "$reproducibility" == "bit-identical" && "$kind" != "source-archive" && "$kind" != "checksum" ]]; then
        fail "the artefact '$name' claims to be bit-identical, but only the source archive may claim that."
    fi
done < <(manifest_rows "artifacts")

# --- 5. The archive carries the revision completely ------------------------
ARCHIVE_PATH="$(manifest_value "artifacts.0.path")"
if [[ -z "$ARCHIVE_PATH" || ! -f "$ARCHIVE_PATH" ]]; then
    ARCHIVE_PATH="$(find "$CANDIDATE_DIRECTORY/source" -name '*-source-*.tar.gz' 2>/dev/null | head -n 1)"
fi
if [[ -z "$ARCHIVE_PATH" || ! -f "$ARCHIVE_PATH" ]]; then
    fail "the candidate holds no source archive."
    ARCHIVE_PATH=""
else
    EXTRACT_DIRECTORY="$(mktemp -d)"
    trap 'rm -rf "$EXTRACT_DIRECTORY"' EXIT
    tar xzf "$ARCHIVE_PATH" -C "$EXTRACT_DIRECTORY"
    EXTRACTED_ROOT="$(find "$EXTRACT_DIRECTORY" -maxdepth 1 -mindepth 1 -type d | head -n 1)"
    if [[ -z "$EXTRACTED_ROOT" ]]; then
        fail "the archive does not contain a directory."
    else
        IDENTIFICATION_NAME="SOURCE-IDENTIFICATION.txt"
        EXPECTED_FILES="$(git ls-tree -r --name-only "$REVISION" | sort)"
        ACTUAL_FILES="$(cd "$EXTRACTED_ROOT" && find . -type f | sed 's|^\./||' | sort)"
        if [[ "$ACTUAL_FILES" == "$(printf '%s\n%s\n' "$EXPECTED_FILES" "$IDENTIFICATION_NAME" | sort)" ]]; then
            pass "the archive holds the tree of the revision, file by file ($(printf '%s\n' "$EXPECTED_FILES" | wc -l | tr -d ' ') file(s) plus the identification)."
        else
            fail "the archive does not hold exactly the tree of the revision."
        fi

        CONTENT_MISMATCHES=0
        while IFS=$'\t' read -r metadata path; do
            expected_blob="${metadata##* }"
            actual_blob="$(cd "$EXTRACTED_ROOT" && git hash-object -- "$path" 2>/dev/null)"
            if [[ "$expected_blob" != "$actual_blob" ]]; then
                fail "the file '$path' in the archive is not the file of the revision."
                CONTENT_MISMATCHES=$((CONTENT_MISMATCHES + 1))
            fi
        done < <(git ls-tree -r "$REVISION")
        if [[ "$CONTENT_MISMATCHES" -eq 0 ]]; then
            pass "every file of the archive is byte for byte the file of the revision."
        fi

        ARCHIVE_REVISION="$(tar xzf "$ARCHIVE_PATH" -O --wildcards "*/$IDENTIFICATION_NAME" 2>/dev/null \
            | sed -n 's/^Source revision: //p' | head -n 1)"
        if [[ "$ARCHIVE_REVISION" == "$REVISION" ]]; then
            pass "the archive names the revision $REVISION."
        else
            fail "the archive names the revision '$ARCHIVE_REVISION' and not $REVISION."
        fi
    fi
fi

# --- 6. The reproducibility of the archive, measured once more -------------
# The claim of the manifest is not taken for granted: the archive is built a
# third time and compared with the two builds of the manifest.
if [[ -n "$ARCHIVE_PATH" ]] && [[ "$ARCHIVE_FIRST" != "not-available" ]]; then
    THIRD_ARCHIVE_DIRECTORY="$(mktemp -d)"
    THIRD_ARCHIVE_LOG="$CANDIDATE_DIRECTORY/logs/verify-third-archive.log"
    mkdir -p "$(dirname "$THIRD_ARCHIVE_LOG")"
    set +e
    bash scripts/release-source-archive.sh --revision "$REVISION" \
        --output "$THIRD_ARCHIVE_DIRECTORY" > "$THIRD_ARCHIVE_LOG" 2>&1
    THIRD_EXIT=$?
    set -e
    THIRD_ARCHIVE="$(find "$THIRD_ARCHIVE_DIRECTORY" -name '*-source-*.tar.gz' | head -n 1)"
    if [[ "$THIRD_EXIT" -ne 0 || -z "$THIRD_ARCHIVE" ]]; then
        fail "the third build of the archive failed — the log is in logs/verify-third-archive.log."
        rm -rf "$THIRD_ARCHIVE_DIRECTORY"
    else
        THIRD_DIGEST="$(sha256_of "$THIRD_ARCHIVE")"
        rm -rf "$THIRD_ARCHIVE_DIRECTORY"
        if [[ "$THIRD_DIGEST" == "$ARCHIVE_FIRST" && "$ARCHIVE_FIRST" == "$ARCHIVE_SECOND" ]]; then
            pass "the third build of the archive has the same checksum as the two of the manifest: the archive is reproducible."
        else
            fail "the third build of the archive has the checksum $THIRD_DIGEST and not $ARCHIVE_FIRST — the claim of the manifest is not measured."
        fi
    fi
fi

# --- 7. The summary -------------------------------------------------------
echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "✓ The release candidate holds up: the archive carries the revision, the checksums are right,"
    echo "  the manifest claims nothing that was not measured, and the candidate is not published."
    exit 0
fi

echo "ERROR: $FAILURES check(s) of the candidate failed." >&2
exit 1

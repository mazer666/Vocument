#!/usr/bin/env bash
# Runs an **optional** proof of the continuous integration and reports its
# outcome in the convention of the project: `PASS` / `FAIL` / `NOT RUN` /
# `BLOCKED`.
#
# An optional proof must never decide the outcome of the job. A proof that
# cannot be carried out on this machine (no Android device, no emulator, no
# QEMU) says `NOT RUN`; a proof that fails says `FAIL`. Neither of them turns
# the job red — and neither of them may ever be reported as a passed proof.
#
# The script therefore always ends with the exit code 0. The caller decides
# what an optional proof is worth; the script only makes the outcome visible
# in the log, so that a green job never carries a silent gap.
#
# Usage:
#   scripts/ci-optional-proof.sh --label "<name>" -- <command> [arguments...]
#   scripts/ci-optional-proof.sh --label "<name>" --not-run "<reason>"
#
# Exit code 0: the outcome was reported (whatever it was).
# Exit code 2: the script was used wrongly.
set -uo pipefail

LABEL=""
NOT_RUN_REASON=""
COMMAND=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --label)
      LABEL="${2:-}"
      shift 2
      ;;
    --not-run)
      NOT_RUN_REASON="${2:-}"
      shift 2
      ;;
    --)
      shift
      COMMAND=("$@")
      break
      ;;
    *)
      echo "usage: $0 --label <name> [--not-run <reason>] -- <command> [arguments...]" >&2
      exit 2
      ;;
  esac
done

if [[ -z "$LABEL" ]]; then
  echo "ERROR: The optional proof needs a --label that names it." >&2
  exit 2
fi

if [[ -n "$NOT_RUN_REASON" ]]; then
  echo "NOT RUN: ${LABEL} — ${NOT_RUN_REASON}"
  echo "         The proof was NOT carried out in this run and must not be reported as passed."
  exit 0
fi

if [[ ${#COMMAND[@]} -eq 0 ]]; then
  echo "ERROR: The optional proof '${LABEL}' has no command." >&2
  exit 2
fi

if ! command -v "${COMMAND[0]}" >/dev/null 2>&1; then
  echo "NOT RUN: ${LABEL} — the command '${COMMAND[0]}' is not available in this environment."
  echo "         The proof was NOT carried out in this run and must not be reported as passed."
  exit 0
fi

echo "==> Optional proof: ${LABEL}"
"${COMMAND[@]}"
STATUS=$?
if [[ "$STATUS" -ne 0 ]]; then
  echo "FAIL: ${LABEL} — the optional proof failed (exit code ${STATUS})."
  echo "      The core of the job is not affected by it, but this proof was NOT passed"
  echo "      and must not be reported as passed."
  exit 0
fi

echo "PASS: ${LABEL}"
exit 0

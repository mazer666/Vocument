# Project Owner Release Clearance

Under `AGENTS.md`, `E5`, and `RELEASE-BLOCKERS.md`, no automated agent, CI workflow, or command-line flag may publish public GitHub releases or binary packages without explicit, verifiable Project Owner clearance.

---

## Current Clearance Status

PROJECT_OWNER_STATUS: PENDING_EXPLICIT_OWNER_APPROVAL
RELEASE_TAG: NONE
COMMIT_REVISION: NONE

---

## Instructions for Project Owner

To authorize a public release candidate publication:
1. Conduct the final legal and compliance review (`E5`, `AGPL-3.0` §6).
2. Update the status fields above:
   - Set `PROJECT_OWNER_STATUS: APPROVED_FOR_PUBLIC_RELEASE`
   - Set `RELEASE_TAG: <target tag, e.g. v0.3.0-alpha.2>`
   - Set `COMMIT_REVISION: <exact 40-character git commit SHA>`
3. Commit this file or provide it via `--clearance-file <path>` to `scripts/publish-github-release.sh`.

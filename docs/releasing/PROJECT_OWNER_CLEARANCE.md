# Project Owner Release Clearance

Under `AGENTS.md`, `E5`, and `RELEASE-BLOCKERS.md`, no automated agent, CI workflow, or command-line flag may publish public GitHub releases or binary packages without explicit, cryptographically verifiable Project Owner clearance.

---

## Current Clearance Status

PROJECT_OWNER_STATUS: PENDING_EXPLICIT_OWNER_APPROVAL
RELEASE_TAG: NONE
COMMIT_REVISION: NONE

---

## Cryptographic Release Clearance Trust Model

Release publication cannot be authorized solely by editable plaintext fields, command-line flags (`--confirm-owner-approval`), or environment variables. A cryptographic signature anchored exclusively in the Project Owner's root of trust (`docs/releasing/project_owner_root_of_trust.pub`) is strictly mandatory.

### Instructions for Project Owner:

1. Conduct the final legal and compliance review (`E5`, `AGPL-3.0` §6).
2. Update the status fields above:
   - Set `PROJECT_OWNER_STATUS: APPROVED_FOR_PUBLIC_RELEASE`
   - Set `RELEASE_TAG: <target tag, e.g. v0.3.0-alpha.2>`
   - Set `COMMIT_REVISION: <exact 40-character git commit SHA>`
3. Sign this clearance file with your private key matching `docs/releasing/project_owner_root_of_trust.pub`:
   ```bash
   ssh-keygen -Y sign -n "vocument-release" -f <path-to-owner-private-key> docs/releasing/PROJECT_OWNER_CLEARANCE.md
   ```
   This generates `docs/releasing/PROJECT_OWNER_CLEARANCE.md.sig`.
4. Provide the signed clearance file and signature via `scripts/publish-github-release.sh --publish --tag <tag>`.

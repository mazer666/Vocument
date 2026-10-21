# The blockers of a public release

**Date:** 2026-10-21
**What this document is:** the list of everything that stands between the
project and a **public** binary. Every entry says what is missing, why it
matters, who has to do it, and what the state is.

The document exists because of two rules of the project:

* the **legal clearance (E5)** is a precondition of every publication, and
  **an agent cannot give it**;
* a **missing statement is a blocker**, not a footnote. A placeholder that
  looks like an answer is worse than an open question, because it hides the
  gap from the person who has to close it.

Nothing in this list is a defect of the code. The code, the licence texts, the
inventory and the local-first rules are in place — that part is done. What is
missing are decisions, identities and a review that only a person can carry
out.

## The channels

| Channel | State | What it needs |
|---|---|---|
| **macOS, direct distribution** | Prepared, not signed | B-1, B-2, B-4, B-5, B-6 |
| **Android, direct APK** | Prepared, not signed | B-1, B-3, B-4, B-6 |
| **Android, Google Play** | Prepared, not signed | B-1, B-3, B-4, B-6, plus the data safety form of Play |
| **macOS, Mac App Store** | **Deferred by the decision of the project owner (2026-10-21)** | B-7 — not a blocker for the other channels |
| **The source archive** | The script is ready, the archive is not built | B-1, B-6 |

## The blockers

### B-1 — The legal compliance review (E5)

**What is missing:** the review of the published binary against §6 and §13 of
the AGPL-3.0, carried out by a person who is entitled to give it.

**Why it matters:** §6 of the AGPL asks for the complete corresponding source
with every copy of the binary. The project has prepared the mechanism — the
source archive, the offer in the bundle, the identification of the revision —
but the **duty** of the §6 can only be discharged by the publisher, and the
question of whether the preparation is enough is a legal question.

**Who:** the project owner, with legal support.

**State:** **open.** No binary is published before this review exists.

**What is already done:** the identification of the build (`build_info.rs`),
the offer of the source (`SOURCE.txt` in the bundle and in the package), the
archive of the source (`scripts/release-source-archive.sh`), the texts of
every licence that travels with the product (`LICENSES/`) and the inventory
of the third-party components (`THIRD_PARTY.md`).

### B-2 — The signing identity and the notarisation credentials (macOS)

**What is missing:** a *Developer ID Application* certificate of the
publisher, and the credentials of the notarisation (`notarytool` profile, or
`APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_SPECIFIC_PASSWORD`).

**Why it matters:** an unsigned bundle is blocked by Gatekeeper, and an
unnotarised bundle cannot be distributed. The notarisation is also the proof
that Apple has seen the bundle.

**Who:** the project owner. The credentials are read from the environment —
**never** from a file of the repository.

**State:** **open.** The script carries the commands
(`codesign --verify --deep --strict`, `xcrun notarytool submit --wait`,
`xcrun stapler staple`, `xcrun stapler validate`); they were **NOT RUN**,
because this environment has no macOS host and no Apple identity.

### B-3 — The signing key of Android

**What is missing:** the key of the release variant (`~/.gradle/gradle.properties`
or the environment) and, for Google Play, the key of *Play App Signing*.

**Why it matters:** an APK without a signature of the release key cannot be
updated. A key that is lost cannot be replaced — the application of that key
would not be the same application any more.

**Who:** the project owner. The key belongs in a backup that the owner
controls, **not** in the repository.

**State:** **open.** The build of the release variant was **NOT RUN** here.

### B-4 — The contact of the responsible person

**What is missing:** the name, the postal address and the e-mail address of
a person, in `PRIVACY.md` §7.

**Why it matters:** both stores ask for a contact that a user can reach. The
address of the repository alone does not answer a user who has no account.

**Who:** the project owner.

**State:** **open — and it must not be filled with a placeholder.** The
policy names the issue tracker and carries an explicit **TODO for the project
owner**. Inventing an address would be a false statement to the user and to
the store.

### B-5 — The build of the source archive

**What is missing:** the built archive of the source of the revision, with
its checksum.

**Why it matters:** the archive *is* the offer of §6. Without it, the offer
is a promise.

**Who:** the project owner (one command).

**State:** **the script is ready, the archive was NOT RUN here**
(`scripts/release-source-archive.sh`). The gate checks that the script
exists and refuses a build from a dirty tree.

### B-6 — The reproducibility of the build from the archive

**What is missing:** the proof that the archive of the revision builds the
binary that was released — a rebuild from the archive and a comparison of the
artefacts.

**Why it matters:** §6 asks for the **complete corresponding source**. The
strongest proof that the archive is complete is that it builds the same
binary.

**Who:** the project owner (and the CI, when it runs on a macOS host).

**State:** **NOT RUN.** No macOS host, no Swift toolchain, no Android
release key in this environment. The instructions are in
[`BUILD-FROM-SOURCE.md`](BUILD-FROM-SOURCE.md).

### B-7 — The decision about the Mac App Store

**What is missing:** a decision of the project owner.

**Why it matters:** the terms of Apple and the AGPL pull in different
directions for the store channel. That is a legal question, and the analysis
of the contract is a preparation for it — not an answer to it. The analysis
lives in [`docs/audit/store-requirements.md`](../audit/store-requirements.md)
and in the verification of the contract versions.

**Who:** the project owner.

**State:** **deferred** by the decision of 2026-10-21. The direct
distribution is prioritised. This is **not** a blocker for the other
channels — it is the reason why the store channel is not prepared any
further.

## What is **not** a blocker

These are done, and they are checked by the gate:

| What | Where | Checked by |
|---|---|---|
| The identification of the source in the binary | `core/vocument-core/src/build_info.rs` | `build_info_tests.rs` |
| The offer of the source with every artefact | `SOURCE.txt` in the bundle and in the package | `scripts/validate-distribution.sh` |
| The texts of every licence that travels with the product | `LICENSES/` | `scripts/validate-distribution.sh` |
| The notice of the third-party components | `LICENSES/THIRD-PARTY-NOTICES.txt` | `scripts/generate-third-party-notices.sh` |
| The inventory of the licences | `THIRD_PARTY.md` | `scripts/audit-licenses.sh` |
| The local-first rules — no account, no telemetry, no network right | `PRIVACY.md`, the manifest, the entitlements | `scripts/validate-distribution.sh` |
| The refusal of a build from a dirty tree | the three release scripts | `scripts/release-*.sh` |

## How a blocker is closed

1. The owner does the thing — the review, the signature, the address, the
   decision.
2. The evidence is put into the repository: the review as a document, the
   checksum of the archive, the revision of the build.
3. The entry in this list is closed with the date and the name of the
   document that carries the proof.

**An agent cannot close any of these entries.** The legal review, the
identity and the decision are the work of a person.

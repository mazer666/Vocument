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

The entries that are missing are the **decisions, the identities and a
review that only a person can carry out**. Everything that is a *technical*
task — the building of the source archive, the proof that it builds the
product, the green gates — is the work of the **team**, and it is carried by
the evidence in [`docs/releasing/REPRODUCIBILITY.md`](REPRODUCIBILITY.md) and
in the build record (`target/build-record/`).

## The summary

| # | What | Who has to do it | State |
|---|---|---|---|
| **B-1** | The legal compliance review (E5) | **Project owner** | open |
| **B-2** | The signing identity and the notarisation credentials (macOS) | **Project owner** | open |
| **B-3** | The signing key of Android | **Project owner** | open |
| **B-4** | The contact of the responsible person | **Project owner** | open |
| **B-5** | The build of the source archive | **Team** | **closed 2026-10-21** — the evidence is in [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §2 and in `target/build-record/build-record.json` |
| **B-6** | The reproducibility of the build from the archive | **Team** | **closed 2026-10-21, as far as this environment allows** — the Swift target and the Android package are **NOT RUN** here, with the reason written down |
| **B-7** | The decision about the Mac App Store | **Project owner** | deferred (2026-10-21) |
| **B-8** | The repository gate was red (`CODE-001`) | **Team** | **closed 2026-10-21** — found by the build record, fixed by the parallel task (commit `3ff973f`) |
| **B-9** | **There is no CI runner.** The workflow asks for a self-hosted runner; the repository has **none** | **Project owner** | **closed 2026-10-22** — the decision of the owner ("GitHub Actions with the required native SDKs, where possible") and three workflows on the hosted runners; the runs of the round are in [`../acceptance/ROUND-6-ACCEPTANCE.md`](../acceptance/ROUND-6-ACCEPTANCE.md) §2.3 |

## The channels

| Channel | State | What it needs |
|---|---|---|
| **macOS, direct distribution** | Prepared, not signed | B-1, B-2, B-4 |
| **Android, direct APK** | Prepared, not signed | B-1, B-3, B-4 |
| **Android, Google Play** | Prepared, not signed | B-1, B-3, B-4, plus the data safety form of Play |
| **macOS, Mac App Store** | **Deferred by the decision of the project owner (2026-10-21)** | B-7 — not a blocker for the other channels |
| **The source archive** | **Built and checked (2026-10-21)** | B-1 (the legal review of the offer) |
| **The alpha release on GitHub (source only)** | **Published 2026-10-22** (`v0.1.0-alpha.1`) | none — it publishes the **source** of a revision, which the public repository is anyway (decision E1). No binary, no store, no package of a signed application: the alpha uses none of the channels above and therefore needs none of their blockers. The description is in [`ALPHA-RELEASE.md`](ALPHA-RELEASE.md) |

## The blockers of the project owner

These four entries are decisions, identities and a legal review. They are
the work of the project owner, and no script and no agent can close them.

### B-1 — The legal compliance review (E5)

**What is missing:** the review of the published binary against §6 and §13 of
the AGPL-3.0, carried out by a person who is entitled to give it.

**Why it matters:** §6 of the AGPL asks for the complete corresponding source
with every copy of the binary. The project has prepared the mechanism — the
source archive, the offer in the bundle, the identification of the revision —
but the **duty** of the §6 can only be discharged by the publisher, and the
question of whether the preparation is enough is a legal question.

**Who has to do it:** the **project owner**, with legal support.

**State:** **open.** No binary is published before this review exists.

**What is already done:** the identification of the build (`build_info.rs`),
the offer of the source (`SOURCE.txt` in the bundle and in the package), the
archive of the source (`scripts/release-source-archive.sh`, built and checked
on 2026-10-21 — B-5), the texts of every licence that travels with the
product (`LICENSES/`) and the inventory of the third-party components
(`THIRD_PARTY.md`).

### B-2 — The signing identity and the notarisation credentials (macOS)

**What is missing:** a *Developer ID Application* certificate of the
publisher, and the credentials of the notarisation (`notarytool` profile, or
`APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_SPECIFIC_PASSWORD`).

**Why it matters:** an unsigned bundle is blocked by Gatekeeper, and an
unnotarised bundle cannot be distributed. The notarisation is also the proof
that Apple has seen the bundle.

**Who has to do it:** the **project owner**. The credentials are read from
the environment — **never** from a file of the repository.

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

**Who has to do it:** the **project owner**. The key belongs in a backup that
the owner controls, **not** in the repository.

**State:** **open.** The build of the release variant was **NOT RUN** here.

### B-4 — The contact of the responsible person

**What is missing:** the name, the postal address and the e-mail address of
a person, in `PRIVACY.md` §7.

**Why it matters:** both stores ask for a contact that a user can reach. The
address of the repository alone does not answer a user who has no account.

**Who has to do it:** the **project owner**.

**State:** **open — and it must not be filled with a placeholder.** The
policy names the issue tracker and carries an explicit **TODO for the project
owner**. Inventing an address would be a false statement to the user and to
the store.

### B-7 — The decision about the Mac App Store

**What is missing:** a decision of the project owner.

**Why it matters:** the terms of Apple and the AGPL pull in different
directions for the store channel. That is a legal question, and the analysis
of the contract is a preparation for it — not an answer to it. The analysis
lives in [`docs/audit/store-requirements.md`](../audit/store-requirements.md)
and in the verification of the contract versions.

**Who has to do it:** the **project owner**.

**State:** **deferred** by the decision of 2026-10-21. The direct
distribution is prioritised. This is **not** a blocker for the other
channels — it is the reason why the store channel is not prepared any
further.

## The blockers of the team

These are technical tasks. They belong to the team, and they are closed with
a **measurement** — a number, a checksum, a report — that can be read again.

### B-5 — The build of the source archive — **closed 2026-10-21**

**What was missing:** the built archive of the source of the revision, with
its checksum.

**Why it matters:** the archive *is* the offer of §6. Without it, the offer
is a promise.

**Who had to do it:** the **team**. Building an archive is a technical task,
not a decision of the owner.

**State:** **closed on 2026-10-21.**

**The evidence:**

| What | Where |
|---|---|
| The archive of the revision `00ddd08b5f11a1d665a9b62443c19f8676025b39` | `target/release-source/vocument-0.1.0-source-00ddd08b5f11.tar.gz` |
| Its SHA-256 | `0d468656774e48c3b00aa07a33f79ec918620577e72daceb1d32fb2bab608b47` (`…tar.gz.sha256` next to it) |
| The verification of the archive against the revision | [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §2 — 375 files, byte for byte, compared with `git ls-tree` and `git hash-object` |
| The two builds of the archive and their comparison | [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §3 — bit for bit the same |
| The build record of the run | `target/build-record/build-record.json` and `target/build-record/build-record.md` |

The archive carries the identification of its revision in
`SOURCE-IDENTIFICATION.txt` (the revision and the state of the checkout at
the moment of the build), and the script that builds it verifies the archive
against the tree of the revision before it hands it out.

### B-6 — The reproducibility of the build from the archive — **closed 2026-10-21, as far as this environment allows**

**What was missing:** the proof that the archive of the revision builds the
binary that was released — a rebuild from the archive and a comparison of the
artefacts.

**Why it matters:** §6 asks for the **complete corresponding source**. The
strongest proof that the archive is complete is that it builds the same
binary.

**Who had to do it:** the **team** — and the CI, when it runs on a macOS
host.

**State:** **closed for everything this environment can build.** The Rust
workspace and the library `libvocument_ffi` are built from the unpacked
archive in a fresh directory and compared with the build of the working tree
artefact by artefact. The measurement is in
[`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §4.

**What is NOT RUN here, and why:**

| What | Why | What was checked instead |
|---|---|---|
| The build of the macOS application (the Swift target) | no macOS host and therefore **no macOS SDK** in this environment — SwiftUI, AppKit and CoreGraphics do not exist for Linux. The Swift **compiler** was installed on 2026-10-21 (6.0.3): it parses every file of the application and it builds the **generated bindings** (`swift build --target VocumentFFI` — green). | the sources of the target are in the archive (`apps/macos`), the checksums of the generated Swift bindings are in the build record, and the parse of every file is proved by `scripts/check-swift-sources.py` |
| The build of the Android package (Gradle) | no Android SDK and no JDK in this environment | the sources of the package are in the archive (`apps/android`), the checksums of the generated Kotlin bindings are in the build record |
| The notarisation of the macOS bundle | no Apple identity and no macOS host | the commands are in `scripts/release-macos.sh` |
| The signature of the Android release build | no release key in this environment | the signing is left to Gradle |

**A limit of the environment is not an open blocker.** The two targets above
are built and checked on the machines that have the toolchain — the
instructions are in [`BUILD-FROM-SOURCE.md`](BUILD-FROM-SOURCE.md), and the
script that does the comparison is `scripts/verify-source-archive.sh`. What
would be a blocker is a *missing* proof; here the proof exists for
everything that this environment can build, and the rest is named with its
reason.

### B-8 — The repository gate was red — **closed 2026-10-21**

**What was missing:** the green gate of the repository. The gate of the
project (`scripts/validate-repo.sh`) failed on the revision
`00ddd08b5f11a1d665a9b62443c19f8676025b39`:

```text
ERROR (CODE-001): Hand-written source file 'core/vocument-core/src/redaction.rs' has 935 lines (hard maximum is 900).
ERROR (CODE-001): Hand-written source file 'engines/pdf-mupdf/src/scrub_tests.rs' has 1066 lines (hard maximum is 900).
```

**Why it matters:** the release process asks for a green gate before a
release is built (V3 of `docs/releasing/RELEASE-PROCESS.md`). The build
record of 2026-10-21 carries the failure in `target/build-record/build-record.json`
under `gates.repository` — it was measured, not hidden.

**Who had to do it:** the **team**. The files have to be split; the rule
`CODE-001` allows at most 900 lines per hand-written source file, and the
target is 600.

**State:** **closed on 2026-10-21.** The finding was produced by the build
record and fixed by the parallel task in the commit `3ff973f`
(`refactor(core): die Typen der Redaction in ein eigenes Modul — das Gate
war rot`); the gate is green again. The entry stays in the list, because a
gate that turns red without anyone noticing is exactly what the build
record is for.

### B-9 — There is no CI runner — **closed 2026-10-22**

**What was missing:** a machine that runs the gate when the code is pushed.

The workflow (`.github/workflows/ci.yml`) asks for `runs-on: self-hosted` —
it runs `./scripts/local-ci.sh`, the same gate that the team runs locally.
The measurement of 2026-10-22, read from the API of the repository:

| What | The measurement |
|---|---|
| The runs of the workflow since 2026-10-17 | **47** |
| Of them: **cancelled** | **38** |
| Of them: **waiting** (`queued`) | **9** |
| Of them: **run** (green or red) | **0** |
| The runners registered for the repository | **0** |

**Why it matters:** the project has **no continuous integration**. Every
gate that was ever run was run by hand, in one environment. The claim "the
CI is green" cannot be made — the CI has never run once. A release asks for
a green gate (`RELEASE-PROCESS.md`, V3); the gate exists, but the *second,
independent* execution of it does not.

**Who has to do it:** the **project owner** — the runner is a machine with
an identity, and the decision about it is not a technical one:

* **a self-hosted runner** on a machine of the owner that has the
  toolchain (see [`CI-SETUP.md`](CI-SETUP.md)) — the workflow stays as it
  is; or
* **GitHub-hosted runners** (`runs-on: ubuntu-latest`) — the workflow file
  has to be changed. The team does not change the workflow files; the
  decision and the change belong to the owner.

**What the owner decided (2026-10-22):** *"Use GitHub Actions with the
required native SDKs where possible."* — the second way. The workflow of
2026-10-17 was replaced by three workflows on the hosted runners
([`ci.yml`](../../.github/workflows/ci.yml),
[`ci-macos.yml`](../../.github/workflows/ci-macos.yml),
[`ci-android.yml`](../../.github/workflows/ci-android.yml)). The
continuous integration of the project exists now; the runs of the round and
what each of them proves are in
[`../acceptance/ROUND-6-ACCEPTANCE.md`](../acceptance/ROUND-6-ACCEPTANCE.md)
§2.

**What the team did meanwhile:** the gate is a script that runs anywhere.
`bash scripts/local-ci.sh` on a second machine — a Mac of the owner, a
Linux box with the toolchain — is the same proof as a runner, without the
identity and the account. The list of what the runner needs is in
[`CI-SETUP.md`](CI-SETUP.md).

**The queue:** the nine waiting runs are the pushes of the last two days.
If a runner appears, they would all start at once on old revisions. They
should be cancelled (`gh run cancel <id>` or the page of the workflow)
before a runner is registered.

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
| The build of the source archive and its verification | `scripts/release-source-archive.sh` | `scripts/verify-source-archive.sh` |
| The build record of every release run | `scripts/build-record.sh` | `scripts/local-ci.sh` |

## How a blocker is closed

1. The owner does the thing — the review, the signature, the address, the
   decision.
2. The evidence is put into the repository: the review as a document, the
   checksum of the archive, the revision of the build.
3. The entry in this list is closed with the date and the name of the
   document that carries the proof.

**An agent cannot close the entries of the project owner.** The legal review,
the identity and the decision are the work of a person.

**The entries of the team are closed with a measurement.** The build of the
archive, the comparison of the artefacts and the record of the run are the
work of the team; their proof is a number that can be read again —
[`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) and `target/build-record/`.

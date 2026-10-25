# The release process of Vocument

This document describes how a release is built, checked and handed out. It
follows the licensing decision (`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`,
E1–E5) and the rules of `AGENTS.md`.

> **No binary is published before the legal compliance review (E5, O1) has
> been carried out by a person who is entitled to carry it out.** No agent
> can give that clearance, and no script of this repository publishes
> anything on its own: the scripts build, they do not upload.

---

## 1. The kinds of release

| Kind | What it is | How it is built |
|---|---|---|
| **Source release** | The archive of the source of a revision. | `scripts/release-source-archive.sh` |
| **macOS, direct** | The signed and notarised `.app` bundle (and its archive) with the notices and the offer of the source inside. | `scripts/release-macos.sh --sign "Developer ID Application: …"` |
| **Android, direct** | The signed APK with the notices and the offer of the source inside. | `scripts/release-android.sh --apk` |
| **Android, Play** | The bundle for the Play Store. The upload is a separate step that needs the approval of the project owner. | `scripts/release-android.sh --bundle` |
| **GitHub Release (Binaries & Source)** | Complete release on GitHub Releases containing compiled native binaries, source tarball, SHA-256 sums, and notices. | `scripts/publish-github-release.sh --tag <tag>` |
| **macOS, Mac App Store** | **Deferred** by the decision of the project owner (2026-10-21): the direct distribution comes first. The terms of Apple and the AGPL-3.0 still have to be weighed before that channel is prepared further (§7.5 of the licensing decision, `RISK-041`, B-7). | — |

### 1.1 The Release Tiers (Alpha, Beta, Stable)

As specified in `ROADMAP.md`:
- **Alpha Releases (`v0.x.0-alpha.y`):** Developer and technical preview releases published on GitHub as pre-releases. Deliver authentic compiled binaries (macOS `.zip` and Android direct `.apk`) for early integration testing, benchmark measurements, and UX trials.
- **Beta Releases (`v0.9.x-beta.y`):** Feature-complete release candidates published on GitHub as pre-releases. Feature freeze; focus is on stress testing ($\ge 500$ pages), crash resistance, VoiceOver/TalkBack accessibility, and wide viewer interop.
- **Stable Releases (`v1.x.0`):** Production general availability releases. Zero known P0/P1 defects; requires formal closure of Owner Release Blockers B-1 through B-4 (legal review, signing identities).

## 2. The conditions before a release

| # | The condition | How it is checked |
|---|---|---|
| V1 | The tree is clean. | The release scripts refuse a dirty tree. |
| V2 | The revision of the source is known and is the revision that is published. | `git rev-parse HEAD`, written into the binary, into the bundle and into the package. |
| V3 | The gate of the project is green. | `scripts/local-ci.sh` |
| V4 | The licence of every component is known and agrees with the AGPL-3.0. | `scripts/audit-licenses.sh` |
| V5 | The notices are in the artefact. | The release scripts copy `LICENSE`, `THIRD_PARTY.md` and write `SOURCE.txt`. |
| V6 | The source archive of the revision exists, with its checksum. | `scripts/release-source-archive.sh` |
| V7 | The legal compliance review has been carried out. | **A person, not a script.** E5. |
| V8 | The attribution of Artifex is in the generated PDFs. | The producer line and the AGPL note in the metadata. **Open — O2.** |

**The open blockers** are listed with their owner and their state in
[`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md). A blocker is closed by a
person, not by a script: the legal review, the signing identity, the address
of the responsible person and the decision about a channel are the work of
the project owner.

## 3. The steps of a release

```bash
# 1. The state of the source
git status                     # must be empty
git log --oneline -1           # the revision that is released

# 2. The gates
scripts/local-ci.sh
scripts/audit-licenses.sh

# 3. The source archive (the offer of the source that belongs to the revision)
scripts/release-source-archive.sh

# 4. The artefacts
scripts/release-macos.sh --sign "Developer ID Application: …"     # on a macOS machine
scripts/release-android.sh --bundle                                # on a machine with the Android SDK
scripts/release-android.sh --apk

# 5. The checksums and the announcement
# 6. Publish to GitHub Releases with binaries:
scripts/publish-github-release.sh --tag v0.3.0-alpha.2 --title "Vocument v0.3.0-alpha.2"
```

The result of every step is in `target/release-*`. The files of the
artefact and their checksums are published together with the revision.

## 4. The notarisation of the macOS bundle

The direct distribution of a macOS application wants a signature and a
notarisation, so that Gatekeeper does not block the user:

```bash
codesign --force --options runtime --timestamp \
    --entitlements apps/macos/Resources/VocumentMac.entitlements \
    --sign "Developer ID Application: …" Vocument.app
codesign --verify --deep --strict --verbose=2 Vocument.app
xcrun notarytool submit Vocument.app --keychain-profile vocument --wait
xcrun stapler staple Vocument.app
xcrun stapler validate Vocument.app
```

The credentials are read from the environment (`VDOCUMENT_NOTARY_PROFILE` or
`APPLE_ID`, `APPLE_TEAM_ID`, `APPLE_APP_SPECIFIC_PASSWORD`). **A secret is
never written into a file of the repository.**

## 5. The signing of the Android package

The signing of the Android package is left to Gradle. The key and the
passwords are read by Gradle from `~/.gradle/gradle.properties` or from the
environment. The upload key of the Play Store is **not** the signing key of
the app; the app signing key is the one that belongs to the revision.

The release build of the app uses the ABI `arm64-v8a` (`abiFilters`), so the
package holds one native library.

## 6. The privacy policy and the data

The product does not ask for the network and does not collect anything about
the user. The privacy policy (`PRIVACY.md`) says this in the words of the
stores. The statements of the policy are checked against the code by the
gate: the Android manifest may not carry the `INTERNET` permission, and the
macOS entitlements may not carry a network entitlement.

## 7. What the release does **not** do

* It does not upload anything. The upload is a step that a person takes,
  with the account of the project, after the approval of the project owner.
* It does not publish a binary of a dirty tree.
* It does not change the licence of the product.
* It does not build a second, proprietary edition. A double licensing would
  be a change of the strategy of E1 and E2 and is a decision of the project
  owner, with legal support.

## 8. After the release

* The revision of the release is written into `CHANGELOG.md`.
* The state of the project (`project/project.yaml`) is brought up to date.
* The acceptance records of the released milestone are written down in
  `docs/acceptance/`, with the results of the gates — and the tests that
  could not be run are named as `NOT RUN`, with the reason.

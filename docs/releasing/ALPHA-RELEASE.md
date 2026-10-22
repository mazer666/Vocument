# The alpha release of Vocument

**Date:** 2026-10-22
**Tag:** `v0.1.0-alpha.1`
**Revision:** `569b2acfb323239ab2a45aa62b94dfd6d8f6f167` — the tag
`v0.1.0-alpha.1` names exactly that commit, and that commit is the source of
this alpha. (`git rev-list -n 1 v0.1.0-alpha.1`)
**What this document is:** the description of the **alpha release** — what it
is, what was measured before it was published, and what it does *not* claim.

> **The alpha release carries the source, not a binary.** The legal compliance
> review (E5) of the licensing decision is open, and only a person who is
> entitled to give it can close it — B-1 of
> [`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md). The signing identity of macOS
> (B-2) and the signing key of Android (B-3) are open as well. Nothing is
> published that would need one of the three. The **repository at the tag is
> the source** — the decision E1 says so — and the tag names the revision
> exactly.

---

## 1. What the alpha is

| What | Where | What it is |
|---|---|---|
| The tag `v0.1.0-alpha.1` | this repository | the revision of the alpha, named once and forever |
| The source of that revision | this repository, at the tag | the complete source of the product: the Rust core, the engines, the UniFFI layer, the macOS and the Android application, the build scripts and the tests |
| The release notes | the release page of GitHub | what the alpha contains, what was measured, and what is still open |

The alpha is published as a **pre-release** of GitHub: it is visible, it is
marked as an alpha, and it is not a stable release. GitHub offers the source
of the tag as an archive; the archive of the project itself
(`scripts/release-source-archive.sh`, byte for byte the tree of the revision)
stays internal until B-1 is closed.

## 2. What was measured before it was published

| What | The measurement | Where |
|---|---|---|
| The gate of the repository | **green** — 264 tests of Rust, 0 failures, 34 suites, and the 5 tests of the kit of macOS (the chain of the redaction among them); the repository, the distribution and the licence gates pass | `target/build-record/build-record.json`, the run of the day |
| The source archive of the revision | **reproducible** — two builds, byte for byte the same | [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §2 and §3 |
| The build from the archive | **the library is built from the unpacked archive** and compared with the build of the working tree | [`REPRODUCIBILITY.md`](REPRODUCIBILITY.md) §4 |
| The licences of the components | **audited** — every component of the product and of the Android package, with its licence | `THIRD_PARTY.md` |
| The three continuous integrations | **green** — Linux, macOS and Android on the hosted runners of GitHub | [`../acceptance/ROUND-6-ACCEPTANCE.md`](../acceptance/ROUND-6-ACCEPTANCE.md) §2 |
| The defect of the job manager (RISK-060) | **found, explained and fixed** — the mechanism is measured, not guessed | [`../audit/ci-gate-findings.md`](../audit/ci-gate-findings.md) §4.5 |

## 3. How to build it

```bash
git clone https://github.com/mazer666/Vocument.git
cd Vocument
git checkout v0.1.0-alpha.1
make check          # the whole gate: repository, Rust, UniFFI, Swift, Kotlin
```

The instructions with the toolchain of every platform are in
[`BUILD-FROM-SOURCE.md`](BUILD-FROM-SOURCE.md). The product is built from the
source of the tag — there is nothing else to trust, and nothing else to
verify.

## 4. What the alpha does *not* claim

* It does **not** claim a stable release. The milestone of the project is
  **M2** (`project/project.yaml`), and the acceptance records name what is
  proven and what is not.
* It does **not** publish a binary. The notarisation of macOS and the
  signature of Android are **NOT RUN** here, with the reason written down
  ([`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md) B-2 and B-3).
* It does **not** replace the legal compliance review (E5). That review is
  the work of the project owner, with legal support — **B-1**. An agent
  cannot give it.
* It does **not** claim that the applications are bit-identical. They are
  not: the library depends on the versions of the tools, on MuPDF, on
  Tesseract and on the paths of the build machine. The manifest of the
  candidate says so and gives the reason.

## 5. The licence

Vocument is published completely under the **AGPL-3.0-or-later** (decision E1
of [`../decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`](../decisions/LICENSING-AND-DISTRIBUTION-DECISION.md)).
The complete corresponding source of this revision is this repository at this
tag; §6 of the AGPL asks for it with every copy of a binary, and there is no
binary in this alpha.

The product is free of charge, needs no account, no network and no cloud
(decision E4): the application has no `INTERNET` permission on Android and no
network entitlement on macOS, and the data of the user stays on the device.

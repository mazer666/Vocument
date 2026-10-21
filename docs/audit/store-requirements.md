# The store requirements and the AGPL conflict — the living digest

**Status:** the technical part is done; the decisions are with the owner.
**Date of the research:** 2026-10-21. The full report with all 88
requirements, the 35 sources and the verbatim licence wording is in the
working directory of the release team
(`/workspace/.mavis/plans/plan_25670a94/outputs/store-requirements/licensing-and-store-requirements.md`).
This file is the part that lives in the repository: the conclusions, the
open decisions and the requirements that the project has to satisfy.

**The contract versions were verified on 2026‑10‑21** against the actually
applicable texts: the Apple Developer Program License Agreement (all three
schedules and the exhibits), the **Apple Media Services Terms and
Conditions** — the layer the first research never used — and the App Review
Guidelines. The report is in the repository:
[`apple-terms-verification.md`](apple-terms-verification.md). The ten
findings of that verification are listed in §5.1.

---

## 1. The conclusion

The strategy of the project is fixed (`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`):
Vocument stays completely open source under the AGPL-3.0-or-later, there is
no commercial MuPDF licence, there is no proprietary variant, there are no
mandatory services, no accounts and no telemetry, and the product is free.

**Of the three distribution channels in scope, the store conditions of
Apple are the ones that carry the most open questions: the Mac App Store.**
Direct distribution of the signed and notarised macOS bundle, Google Play and
the direct APK distribution can all be built without changing the strategy.

**What the contracts say.** The review rules of the store contain obligations
that stand next to the duties of the AGPL: 2.4.5 (vi) forbids a licence screen
at launch, 2.4.5 (vii) forces the updates through the store, 2.5.2 restricts
the download of code, and the Apple Developer Program License Agreement
appoints Apple as the *agent and/or commissionaire* of the developer —
**§7.1, Schedule 1** for a free-of-charge application, which is what Vocument
is (E3). Schedule 1 §1.1 (ii) and §1.3 appoint Apple as commissionaire for
the storefronts of Exhibit A §2, and §3.1 uses the same *"in the course of
acting as agent and/or commissionaire"* wording as Schedule 2 §3.1. (The
*heading* of Schedule 1 — "Appointment of Agent" — is narrower than its
*body*.) The AGPL-3.0 §12 says the opposite: *"If you cannot convey a
covered work so as to satisfy simultaneously your obligations under this
License and any other pertinent obligations, then as a consequence you may
not convey it at all."*

**What is open.** Whether these obligations can be satisfied **at the same
time** as the duties of the AGPL — §6, §12 — is a **legal question that is
not decided here**. The analysis prepares it; it does not answer it. Two
things bound the analysis:

* The conditions 2.4.5 (ii) and (vii) restrict the **binary** that is
  distributed through the store. They do not forbid publishing the **source**
  elsewhere, and §6(d) of the AGPL does not require the offer of the source
  to sit inside the store.
* The **Apple Media Services Terms and Conditions** bind the store
  distribution (Exhibit B of the ADPLA makes them part of the EULA), and
  their Standard EULA carries an **open-source carve-out** in its
  anti-modification clause: *"… or to the extent as may be permitted by the
  licensing terms governing use of any open-sourced components included with
  the Licensed Application"*. That carve-out belongs in the legal review
  (E5) as an **input** — not as a solution.

**No agent can give the legal clearance (E5 of the strategy), and no agent
may decide a change of the licence strategy (E1).** The way out — a double
licensing of the store build — is explicitly excluded by the strategy and is
an owner decision (E‑1).

**The decision of the project owner (2026‑10‑21): the Mac App Store is
deferred, the direct distribution of macOS comes first.** That decision does
not depend on the outcome of the legal review.

## 2. The three channels

| Channel | State | What it needs |
|---|---|---|
| **macOS, direct distribution** | built without a change of the strategy | Developer ID signing with the hardened runtime, notarisation, the notices, the offer of the source, the sandbox |
| **Google Play (Android)** | built without a change of the strategy | the app bundle, the target API level 36, the data safety form, the privacy policy, Play App Signing, the 16 KB page size, the developer verification |
| **Direct APK (Android)** | built without a change of the strategy | the signing key, a rising `versionCode`, the update channel — and it **must** stay possible, because `DIST-001` demands it |
| **Mac App Store** | **deferred** (decision of the owner, 2026‑10‑21). The conditions of the store carry obligations that stand next to the duties of the AGPL; whether they can be satisfied at the same time is **open (E5)** | a decision of the owner (E‑1, E‑2, E‑3) after the legal review |
| **iOS / iPadOS App Store** | the same structure as the Mac App Store, and later anyway | the platform track of the roadmap |

### 2.1 The three corrections to the task that the research found

1. **There is no "§13 Corresponding Source" in the AGPL-3.0.** Section 13
   reads *"Remote Network Interaction; Use with the GNU General Public
   License"*. The duties that follow from handing out a binary are in §0
   (the definition of the corresponding source), §4, §5, §6 (the conveyance)
   and §12. The repository said otherwise in nine places; they were
   corrected in this round.
2. **"Build tools" is not a term of the licence.** The full text of the
   AGPL-3.0 and of the GPL-3.0 contains no occurrence of it. What counts is
   §0: *"all the source code needed to generate, install, and (for an
   executable work) run the object code and to modify the work, including
   scripts to control those activities"*, with the exception of the system
   libraries and the general-purpose tools.
3. **The 30 MB download limit of the on-demand model is obsolete.** On-Demand
   Resources are deprecated as of iOS 27, iPadOS 27, tvOS 27 and visionOS
   27; the successor is Background Assets, and the limits lie in the
   gigabyte range.

### 2.2 The App Sandbox is not a question of the store

`ARCHITECTURE.md` decides: *"macOS App Sandbox from the start"*. The
sandbox is therefore **also** in the direct distribution, which is the
recommended macOS channel. Apple's documentation only makes the sandbox
mandatory for the Mac App Store; it does not say that a direct distribution
is sandbox-free. The access model follows from it: security-scoped
bookmarks, the container in `~/Library/Containers`, no starting of programs
outside the bundle, and the binding of the container to the code signature
from macOS 14 on. The `FileBroker` of the core therefore has to speak URLs
and scopes, not paths — that is architecture-relevant, and it is the same
semantics that iOS enforces.

## 3. The requirements per channel

The full list carries 88 rows, each with its source and its classification:
**(a)** the team can implement it, **(b)** it needs a legal review or an
external approval, **(c)** it is informative. The rows that carry the
architecture or the release are these:

### 3.1 macOS, direct distribution (M‑1…M‑20)

| # | Requirement | Class | Where it stands |
|---|---|---|---|
| M‑2 | The bundle is signed with a Developer ID certificate, the hardened runtime and a secure timestamp | (a) | `scripts/release-macos.sh` (`codesign --force --options runtime --timestamp`) |
| M‑3 | `codesign --verify --deep --strict` before the package is built | (a) | `scripts/release-macos.sh` |
| M‑5 | The hardened runtime is on, without the `get-task-allow` entitlement | (a) | `scripts/release-macos.sh` |
| M‑8 | The bundle is notarised with `xcrun notarytool submit --wait` | (a) | `scripts/release-macos.sh` — needs the credentials |
| M‑10 | The ticket is stapled with `xcrun stapler staple` and checked with `stapler validate` | (a) | `scripts/release-macos.sh` |
| M‑13 | The notices travel with the bundle: the licence, the inventory of the third-party components, the texts of their licences, the offer of the source | (a) | `scripts/release-macos.sh`, `scripts/generate-third-party-notices.sh` |
| M‑18 | The revision of the source travels with the binary, and a build from a dirty tree is refused | (a) | `scripts/release-macos.sh`, `core/vocument-core/src/build_info.rs` |
| M‑16 | The access to the documents of the user is security-scoped | (a) | `ARCHITECTURE.md` §15, the `FileBroker` |

### 3.2 Google Play (GP‑1…GP‑16)

| # | Requirement | Class | Where it stands |
|---|---|---|---|
| GP‑1 | The app bundle is mandatory for a new app on Play | (a) | `scripts/release-android.sh --bundle` |
| GP‑2 | The target API level 36 is required since 2026‑08‑31 | (a) | **open** — the project still targets 35 |
| GP‑8 | The 16 KB page size is proven | (a) | `scripts/verify-android-native-libs.sh` — done, all four native libraries are aligned to `0x4000` |
| GP‑12 | The data safety form is answered — with "no data collected" | (a) | `PRIVACY.md` is the evidence; the form itself is filled in the console |
| GP‑13 | The privacy policy is in the store listing **and** in the application | (a) | `PRIVACY.md` |
| GP‑15 | No telemetry, no advertising, no accounts | (a) | no `INTERNET` permission, `LOG-001` |
| GP‑16 | Play App Signing: the upload key and the app signing key | (a) | the keys are a decision of the owner (E‑7) |

### 3.3 The direct APK (D‑1…D‑12)

| # | Requirement | Class | Where it stands |
|---|---|---|---|
| D‑1 | The APK is signed with a key of the project | (a) | `scripts/release-android.sh --apk` |
| D‑3 | The `versionCode` rises with every release and is never reused | (a) | the greatest value Play allows is 2100000000 |
| D‑5 | The update channel is signed and does not force the update | (a) | **open** — `UPDATE-001` |
| D‑9 | The registration for the developer verification | (b) | E‑6 |

### 3.4 iOS and iPadOS, later (I‑1…I‑12)

| # | Requirement | Class |
|---|---|---|
| I‑2 | The access to the documents runs through the system picker (`UIDocumentPickerViewController`), the sandbox forbids any other path | (a) |
| I‑5 | The memory budget: there is no documented fixed limit; the system ends an app under pressure (jetsam). The thermal state and the physical memory are the control values | (c) |
| I‑6 | The size of the bundle: 4 GB (iOS 18 and later), 2 GB before | (c) |
| I‑12 | The Rust core with UniFFI on iOS: sandbox, no arbitrary process execution, the limits of the system | (a) |

## 4. The decisions that are with the owner

| ID | Decision | What it changes |
|---|---|---|
| **E‑1** | A **double licensing** of the Mac App Store build | Makes the store possible — and is a change of the strategy of E1 and E2. **The team may not decide it.** |
| **E‑2** | **Doing without the Mac App Store** — macOS only by direct distribution | Inside the strategy ("the Mac App Store as an *additional* option"), but a product decision that has to be recorded |
| **E‑3** | An **exception under §7** of the AGPL, or a negotiation with Apple | §7 needs the consent of **all** the rights holders; MuPDF is an external rights holder |
| **E‑4** | The **publication of an executable version** at all | A gate of the strategy: no binary without a separate approval |
| **E‑5** | The **legal review** of the AGPL compliance per channel | A gate before E‑4. No agent can replace it |
| **E‑6** | The **registration for the Android developer verification** (identity check, 25 USD, package name and fingerprint of the key) | Without it, installation problems in Brazil, Indonesia, Singapore and Thailand from 2026‑09‑30, and globally from 2027 |
| **E‑7** | The **accounts and the costs**: Apple Developer Program (99 USD per year), Play Console (25 USD once), the Android Developer Console (25 USD) | Contracts that the owner signs |
| **E‑8** | Whether **Play may be the only Android channel** | `DIST-001` demands that the direct installation stays possible — so the direct APK channel is obligatory, not optional |
| **E‑9** | Whether the **Mac App Store may be mentioned** in the product or on the website while there is no store build | Advertising and review risk under 3.2.2 (iii) and (x) |
| **E‑10** | **What exactly is offered as the corresponding source** — the source snapshot, the installation information, the notices | Decides whether E‑4 can be released |

### 5.1 The findings of the verification of the contract versions

The verification of 2026‑10‑21 ([the report](apple-terms-verification.md))
found ten items. They are listed here because **three of them changed a
statement of this file** and because the rest are corrections of precision
that a reader of the first report would have got wrong.

| # | What | What was done |
|---|---|---|
| **F‑01** | The hypothesis of the review — that a free app runs under Schedule 1 and that Schedule 1 does not know the *commissionaire* — is **refuted by the text**: Schedule 1 §1.1 (ii) and §1.3 appoint Apple as commissionaire, and §3.1 uses the same wording as Schedule 2 §3.1 | The conclusion of the report **stands** for the free app; §1 now quotes Schedule 1 §1.1/§1.3 and says that the heading of Schedule 1 is narrower than its body |
| **F‑02** | This file attributed the agency clause to "Schedule 2, 3.1" — Vocument is free, so **Schedule 1** applies (§7.1) | Corrected in §1 |
| **F‑03** | The **Apple Media Services Terms and Conditions** were missing from the analysis, although they bind the store distribution, and their Standard EULA carries an **open-source carve-out** | Added to §1 as an input for the legal review (E5) |
| **F‑04** | The conflict table of the full report quotes the clauses of **Schedule 2** (4.2/4.3/7.3) although the free app runs under **Schedule 1** (3.2/3.3/6.3) | Noted here; the full report carries the mapping |
| **F‑05** | Two quotes of the review guidelines are truncated: 2.4.5 (i) and the exception of 2.5.2 for educational apps | Marked in the full report; the exception belongs to the legal review |
| **F‑06** | The words "blocked", "structurally inadmissible" and "collides" are a **legal determination presented as fact**, while the owner keeps the clearance open (E5) and has deferred the channel | Removed from §1 and §2; the wording is now "the conditions carry obligations that stand next to the duties of the AGPL; whether they can be satisfied at the same time is open" |
| **F‑07** | 2.4.5 (ii) and (vii) restrict the **binary**; they do not forbid publishing the **source** elsewhere, and §6(d) does not require the offer inside the store | Added to §1 as a bound of the analysis |
| **F‑08** | "Only the Mac App Store" was asserted product-wide; the iOS/iPadOS App Store has the same structure, and the negative claim for Google Play rests on an unverified reading | §1 now says "of the three channels in scope"; the iOS channel is named in §2; the wording for Play stays "(b) = legal review" |
| **F‑09** | Neither the ADPLA page nor the review guidelines carry a version or a date | The source list of the verification now carries the **retrieval date** and the **SHA-256** of the retrieved document |
| **F‑10** | The "Apple Media Services Terms, Abschnitt 5 »Developer Content Distribution«" that the first task named **does not exist** in the current document (the structure runs A–T; the App Store terms are in section O) | Recorded, so that nobody quotes a section that does not exist |

**Confirmed without defect:** the assignment of the schedules (Schedule 1 =
free, Schedule 2 = fee-based, Schedule 3 = paid custom distribution), the
fact that "Mac App Store" appears **zero** times in the ADPLA, all quoted
guideline wordings, the wording of §6 and §12 of the AGPL, and the currency
of the contracts as of 2026‑10‑21.

## 5. The limits of the research

* `www.gnu.org` and `www.fsf.org` were **not reachable** from the build
  environment (TCP timeout, not an HTTP error). The AGPL text was therefore
  taken from the official GNU mirror and is byte-identical with the copy in
  the repository (SHA-256 `0d96a4ff…`). The **position of the GNU project**
  on the App Store could not be verified and is therefore carried as a
  source that has to be checked in the final legal review.
* The Apple terms, the review guidelines and the Google requirements were
  read on 2026‑10‑21 from the pages named in the full report, and the Apple
  contracts were **retrieved a second time and compared byte-wise** — the
  text is identical, so the sources of the first report are current. Neither
  the ADPLA page nor the review guidelines carry a version or a date of
  their own; the retrieval date and the SHA-256 of the retrieved document
  are therefore the only version that can be named.
* The **decision about the store is not made here.** The Mac App Store
  channel is **deferred** by the decision of the owner; the report prepares
  it, it does not use it.
* The analysis of the **App Store terms of the consumer side** (the Apple
  Media Services Terms) is new; it was not part of the first research. It
  belongs to the legal review (E5) as an input.

## 6. What the project does about it now

* The **direct distribution** is the channel that is built: the signed and
  notarised bundle, the notices, the offer of the source
  (`scripts/release-source-archive.sh`), the source archive of the revision.
* The **sandbox** is in the bundle from the start, as `ARCHITECTURE.md`
  decides — not only for a store build.
* The **store upload** is prepared but **not taken**: the release scripts
  build, check and sign; they do not publish. The upload is a separate step
  that needs E‑1 … E‑5.
* The **open blockers** of a public binary are listed with their owner and
  their state in
  [`docs/releasing/RELEASE-BLOCKERS.md`](../releasing/RELEASE-BLOCKERS.md).
  The legal clearance (E5), the signing identity, the address of the
  responsible person and the decision about a channel are the work of a
  person — an agent cannot close them.

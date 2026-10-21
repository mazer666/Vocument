# Apple Terms Verification — does only the Mac App Store collide with AGPL-3.0?

**Role:** licence and supply-chain reviewer (ROLE-18 + ROLE-16)
**Repository checked:** `/workspace/vocument`, branch `main`, commit `bbc2d0b` (read-only — nothing in the repository was changed, nothing was committed)
**Verification date:** 2026-10-21
**Object of the review:** the report `licensing-and-store-requirements.md` (411 lines, sources S-01…S-35) in
`/workspace/.mavis/plans/plan_25670a94/outputs/store-requirements/`, and the living digest
`docs/audit/store-requirements.md` in the repository (167 lines), which was read first.
**Status of this document:** a verification, not a decision. No licence strategy is changed here, no legal
assessment is made here.

---

## 0. The one question and the short answer

**Question:** does the report's claim — *only the Mac App Store collides structurally with the AGPL-3.0* —
hold at the actual contract texts?

**Answer in three parts:**

1. **The hypothesis to be tested is refuted.** The task assumed that a free app (Vocument is free,
   strategy decision E3) runs under ADPLA **Schedule 1**, and that Schedule 1 "does not know the
   *commissionaire*", so that the strongest argument against the Mac App Store (Apple as *commissionaire*
   → AGPL §12) would fall away for free apps. **The contract text says the opposite.** Schedule 1 appoints
   Apple as **agent *and* commissionaire**, and Schedule 1 §3.1 uses exactly the same "acting as agent and/or
   commissionaire" wording as Schedule 2 §3.1. The report's central argument therefore **stands** for a free
   app — the reason is not "Apple gives the source away" but that Apple hosts and delivers the binary in the
   role the AGPL §12 analysis is about. (S-01, §1.1/§1.3/§3.1 below.)
2. **But the report never quotes the passage that carries this.** It quotes only the *headings*
   ("Schedule 1 / 1. Appointment of Agent", "Schedule 2 / 1. Appointment of Agent and Commissionaire") and
   §3.1. A reader — the owner, a reviewer, the task author — can and did draw the wrong conclusion that the
   commissionaire role is absent for free apps. The report's conclusion is right; its presentation is
   incomplete at the decisive point. (Finding **F-01**.)
3. **The exclusivity claim is not yet proven at the level the report states, and the report states a legal
   conclusion as if it were a technical finding.** The living digest says flatly: *"Of the three
   distribution channels, exactly one collides structurally with that strategy: the Mac App Store"* and
   *"The collision is not a matter of taste."* That is a **legal determination** (can the store conditions and
   the AGPL obligations be satisfied simultaneously? — precisely the question AGPL §12 poses), while the
   report's own classification system says that exactly this point is **(b) = needs a legal review**, which
   the owner has kept open as **E5**. The report contradicts itself, and the living digest carries the
   contradiction into the repository. (Findings **F-05**, **F-06**.)

**What this means for the owner:** the *direction* of the report (the Mac App Store is the channel with the
structural problem; the direct macOS distribution is the channel that works under the fixed strategy) is
**not overturned** by this verification. What has to change is (a) the Schedule evidence in the report, (b) the
missing Apple Media Services terms, and (c) the wording, which currently states a legal verdict where the
decision record says the legal clearance (E5) is still open and the Mac App Store is deferred.

---

## 1. Method, and what was verified against what

| What | How |
|---|---|
| The two documents under review | The living digest `docs/audit/store-requirements.md` was read **first** (as instructed), then the full report in the plan outputs directory. |
| The Apple contracts | **Live retrieval on 2026-10-21** into the plan workspace (`/workspace/.mavis/plans/plan_cb381bba/workspace/src/`), then compared byte-wise with the copies the report used (`/workspace/.mavis/plans/plan_25670a94/workspace/src/`). |
| The AGPL text | From the vendored copy in the repository (`LICENSES/AGPL-3.0-or-later.txt`, SHA-256 `0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0`) and cross-checked against the GNU mirror. |
| Classification | Every statement below is marked: **what the contract says** / **what follows from it** / **what is open**. |

**Version currency — how it was established.** The ADPLA page and the Review Guidelines page carry **no
version number and no date stamp**. Currency could therefore only be shown by a byte comparison:

| Document | Copy used by the report (2026-10-21) | Live copy (2026-10-21) | Result |
|---|---|---|---|
| ADPLA | 848 279 B, SHA-256 `d639834c165b7466…` | 848 279 B, SHA-256 `2c8d931943d195a9…` | **Text identical.** The only byte difference in the whole page is a CDN cache-buster on one script URL (`sf-symbol.js?09232605293` vs `?45082615294`). The extracted text files are byte-identical (`diff` = empty). |
| App Review Guidelines | 240 818 B, SHA-256 `f154249bbf4f1a6f…` | 240 818 B, SHA-256 `4ab653ded784df81…` | Same: only the cache-buster differs; extracted text identical. |

**Consequence:** the texts the report quotes are the texts in force today. The report's quotes are **not**
outdated — the defects found below are gaps in the *evidence and framing*, not in the currency of the sources.

**Network limitation, stated explicitly.** `www.gnu.org` and `www.fsf.org` were **not reachable** from this
environment (TCP timeout, no HTTP status; DNS resolves). The AGPL text was therefore taken from the vendored
repository copy (authoritative for this project, byte-identical to the GNU mirror
`https://mirrors.kernel.org/gnu/Licenses/agpl-3.0.txt`, HTTP 200) and the canonical URL
`https://www.gnu.org/licenses/agpl-3.0.txt` is marked as *canonical, not retrievable here*. **No contract
wording was guessed**; every Apple quote below is from a document that was actually retrieved.

---

## 2. ADPLA — which Schedule applies, and what Schedule 1 actually says

### 2.1 The schedule assignment is confirmed

**What the contract says (ADPLA, verbatim):**

> **§7.1** "If Your Application qualifies as a Licensed Application, it is eligible for delivery to end users
> via the App Store or Custom App Distribution by Apple and/or an Apple Subsidiary. If You would like Apple
> and/or an Apple Subsidiary to deliver Your Licensed Application or authorize additional content,
> functionality or services You make available in Your Licensed Application through the use of the In-App
> Purchase API to end users for free (no charge) via the App Store or Custom App Distribution, then You appoint
> Apple and Apple Subsidiaries as Your **legal agent and/or commissionaire** pursuant to the terms of
> **Schedule 1** for Licensed Applications designated by You as free-of-charge applications."

> **§7.2** "…If Your Application intends to charge end users a fee of any kind … You must enter into a
> **separate agreement (Schedule 2)** with Apple … before any such commercial distribution of Your Licensed
> Application may take place via the App Store … If You would like Apple to sign and distribute Your
> Application for a fee through Custom App Distribution, then You must enter into a separate agreement
> (**Schedule 3**) …"

**What follows:** the schedules separate **price model and distribution channel**, not platforms —
`Schedule 1 = free apps (App Store or Custom App Distribution)`, `Schedule 2 = fee-based apps (App Store)`,
`Schedule 3 = fee-based custom apps`. Vocument is free (strategy decision **E3**), so **Schedule 1** is the
operative schedule. The report's correction **K-4** and its §5.1a are **confirmed against the text**.

**What is confirmed about the term "Mac App Store":** the string does not occur in the ADPLA at all
(`grep -c -i "Mac App Store"` → **0**; "macOS" → 48 hits). The report's statement on this is **confirmed**.

### 2.2 The decisive passage the report does not quote — Schedule 1 *does* appoint a commissionaire

> **Schedule 1, §1.1** "You hereby appoint Apple and Apple Subsidiaries (collectively "Apple") as:
> **(i) Your agent** for the marketing and delivery of the Licensed Applications to end users located in those
> regions listed on **Exhibit A, Section 1** to this Schedule 1, subject to change; and **(ii) Your
> commissionaire** for the marketing and delivery of the Licensed Applications to end users located in those
> regions listed on **Exhibit A, Section 2** to this Schedule 1, subject to change, during the Delivery Period."

> **Schedule 1, §1.3** "The parties acknowledge and agree that their relationship under this Schedule 1 is,
> and shall be, that of **principal and agent, or principal and commissionaire, as the case may be**, as
> described in Exhibit A, Section 1 and Exhibit A, Section 2 respectively …"

> **Schedule 1, §3.1** "You acknowledge and agree that Apple, **in the course of acting as agent and/or
> commissionaire for You**, is hosting, or pursuant to Section 1.2(b) of this Schedule 1 may enable authorized
> third parties to host, the Licensed Application(s), and is allowing the download of those Licensed
> Application(s) by end users, on Your behalf."

> **Exhibit A (to Schedule 1), Section 1 "Apple as Agent"** — "You appoint Apple Canada, Inc. … as Your agent
> for the marketing and end user download … : **Canada**"; Apple Pty Limited: "**Australia**, **New
> Zealand**"; Apple Inc. "pursuant to California Civil Code §§ 2295 et seq.": "**United States**"; Apple
> Services LATAM LLC: Argentina … Venezuela; iTunes K.K. "pursuant to Article 643 of the Japanese Civil Code":
> "**Japan**".

> **Exhibit A (to Schedule 1), Section 2 "Apple as Commissionaire"** — "You appoint **Apple Distribution
> International Ltd.** as Your commissionaire for the marketing and end user download of the Licensed
> Applications by end users located in the following regions … For the purposes of this Agreement,
> **"commissionaire" means an agent who purports to act on their own behalf and concludes agreements in his
> own name but acts on behalf of other persons, as generally recognized in many Civil Law legal systems**"
> (Afghanistan, Albania, Algeria, Angola, Armenia, Austria, Azerbaijan, Bahrain, Belarus, Belgium, Benin,
> Bosnia and Herzegovina, Botswana … — the remaining storefronts).

**What follows from this text:**

* The heading of Schedule 1 ("1. Appointment of Agent") is **narrower than the body of Schedule 1**. The body
  appoints Apple as agent **and** as commissionaire, split by storefront region. A free app in the European
  storefront is distributed by Apple **in its own name on behalf of the developer** — that is the §12-relevant
  configuration.
* The report's §5.1a statement that the Schedule 1 clause is "gleichlautend" (identical in wording) to
  Schedule 2 §3.1 is **correct** for the hosting clause: both read "in the course of acting as agent and/or
  commissionaire for You, is hosting … and is allowing the download … on Your behalf" (Schedule 1 line 1745,
  Schedule 2 line 2067 of the extracted text).
* **The task's hypothesis is therefore wrong**, and the report's conclusion survives: the *commissionaire*
  argument does **not** fall away for a free app. The gap is evidential, not analytical.

**What stays open:** whether "Apple acts in its own name" and the hosting/delivery arrangement are *as a matter
of law* in contradiction with the AGPL duties of the developer (§6 conveyance, §6 Installation Information,
§12) is a **legal question** (E5) — this verification states only what the clause says.

### 2.3 Schedule-1 clauses the report cites only from Schedule 2

The report's conflict table (§5.1) quotes the **Schedule 2** versions of the EULA and termination clauses.
For a free app the operative ones are the Schedule 1 counterparts — the numbering and even the exhibit letter
differ:

| Subject | Schedule 2 (fee-based) | Schedule 1 (free — applies to Vocument) | Same wording? |
|---|---|---|---|
| EULA may be delivered by the developer | §4.2 — "…may not be inconsistent with the minimum terms and conditions specified on **Exhibit D** to this Schedule 2…" | **§3.2** — "…may not be inconsistent with the minimum terms and conditions specified on **Exhibit B** to this Schedule 1…" | same structure, different exhibit letter |
| EULA is a contract between developer and end user | §4.3 — "…solely between You and the End-User…" | **§3.3** — "…solely between You and the end user…" | yes (case differs) |
| Apple may stop downloads at any time | §7.3 | **§6.3** — same wording, incl. "(iv) You have violated the terms of the Agreement, this Schedule 1, or other documentation including without limitation the **App Review Guidelines**" | yes |

**What follows:** the report's §5.1 table should carry the Schedule 1 numbers (3.2 / 3.3 / 6.3 / Exhibit B) for
the free app, otherwise the citations point at a schedule that does not apply to Vocument. (Finding **F-04**.)

---

## 3. Apple Media Services Terms and Conditions — the layer the report never uses

### 3.1 The document named in the task could not be found as described

The task refers to "**Apple Media Services Terms and Conditions**, Abschnitt 5 »Developer Content
Distribution«". **No such section exists in the current document.** The live AMS Terms (S-03) are structured
**A – T**: A. Introduction · B. Payments, Taxes, and Refunds · C. Account · D. Privacy · E. Accessibility ·
F. Services and Content Usage Rules · G. Termination and Suspension of Services · H. Downloads ·
I. Subscriptions · J. Content and Service Availability · K. Third-Party Devices and Equipment ·
L. Your Submissions to Our Services · M. Family Sharing · N. Season Pass and Multi-Pass ·
**O. Additional App Store Terms (excluding Apple Arcade apps)** · P. Additional Terms for App Store, Apple
Books, Apple Podcasts and Select Content · Q. Additional Apple Music Terms · R. Additional Apple Fitness+ Terms ·
S. Reseller Membership · T. Misc. Terms Applicable to All Services.

A full-text search for the strings "Developer Content" and "Developer Content Distribution" in the live AMS
Terms (both the global and the US variant) and in the ADPLA returns **zero hits**. The URL
`https://developer.apple.com/support/terms/apple-media-services-terms-conditions/` returns HTTP 200 with a
**"Page Not Found"** body. **No contract wording is guessed here.** What follows is verified against the
provisions that actually exist, at the same standard.

### 3.2 The AMS Terms are part of the store distribution conditions — and the developer contract incorporates them

This is the important structural point the report misses. The **developer-facing** contract makes the
**consumer-facing** AMS Terms binding for the app:

> **Exhibit B (to Schedule 1), "Instructions for Minimum Terms of Developer's End-User License Agreement",
> item 1** — "The EULA may not provide for usage rules for Licensed Applications that are in conflict with,
> **the Apple Media Services Terms and Conditions** or the Volume Content Terms as of the Effective Date (which
> You acknowledge You have had the opportunity to review)."

> **Exhibit B, item 2** — "The license granted to the end user for the Licensed Application must be limited to
> a non-transferable license to use the Licensed Application on any Apple-branded Products that the end user
> owns or controls **and as permitted by the Usage Rules set forth in the Apple Media Services Terms and
> Conditions** …"

> **Exhibit B, item 3** — "You must be solely responsible for providing any maintenance and support services
> with respect to the Licensed Application … Apple has no obligation whatsoever to furnish any maintenance and
> support services …"

> **Exhibit B, item 10** — "…**Apple, and Apple's subsidiaries, are third-party beneficiaries of the EULA**,
> and that, upon the end user's acceptance of the terms and conditions of the EULA, Apple will have the right
> (and will be deemed to have accepted the right) to enforce the EULA against the end user as a third-party
> beneficiary thereof."

**What follows:** the store distribution of a Vocument build is governed by three layers, all of which the
report must name — **ADPLA Schedule 1 (developer ↔ Apple)** · **Apple Media Services Terms and Conditions
(end user ↔ Apple, including the Standard EULA)** · **App Review Guidelines (Apple's review rules)**. The
report uses only the first and the third.

### 3.3 The end-user contract contains an anti-modification clause **with an open-source carve-out**

> **AMS Terms, Section O, "LICENSE OF APP STORE CONTENT"** — "App licenses are provided to you by Apple or a
> third-party developer ("App Provider"). **Apple acts as an agent for App Providers in operating the App
> Store** and is not a party to the sales contract or user agreement between you and the App Provider … Any App
> that you acquire is governed by the Licensed Application End User License Agreement ("Standard EULA") set
> forth below, **unless Apple or the App Provider provides an overriding custom license agreement ("Custom
> EULA")** … You acknowledge and agree that Apple is a third-party beneficiary of the Standard EULA or Custom
> EULA applicable to each Third Party App and may therefore enforce such agreement."

> **AMS Terms, Section O, Standard EULA, "a. Scope of License"** — "Licensor grants to you a nontransferable
> license to use the Licensed Application on any Apple-branded products that you own or control and as
> permitted by the Usage Rules … **You may not copy (except as permitted by this license and the Usage Rules),
> reverse-engineer, disassemble, attempt to derive the source code of, modify, or create derivative works of
> the Licensed Application, any updates, or any part thereof (except as and only to the extent that any
> foregoing restriction is prohibited by applicable law or to the extent as may be permitted by the licensing
> terms governing use of any open-sourced components included with the Licensed Application).**"

> **AMS Terms, Section O, "APP MAINTENANCE AND SUPPORT"** — "Apple is responsible for providing maintenance
> and support for Apple Apps only, or as required under applicable law. **App Providers are responsible for
> providing maintenance and support for Third Party Apps.**"

> **AMS Terms, Section F (Usage Rules), App Store Content** — the App-Store-specific rules are about accounts,
> devices and enterprise downloads, not about modifying the app binary. The general rule "You may not tamper
> with or circumvent any security technology included with the Services or Content. **You may access our
> Services only using Apple's software, and may not modify or use modified versions of such software**" refers
> to *Apple's software* (the Services), not to the third-party app.

**What follows from this text — and what does not:**

* The end-user contract contains an **express open-source qualification** in its anti-modification clause.
  The report's flat statement that "Apple's review rules ask for terms that the AGPL cannot grant" is
  therefore **not supported at this layer**: the contract itself contemplates that open-source components may
  be governed by their own licensing terms. (Finding **F-03**.)
* The carve-out is **narrow**: it speaks of "open-sourced **components included with** the Licensed
  Application", not of the covered work as a whole, and it does not create an obligation to *deliver* modified
  versions. Whether it covers the AGPL duties of §§ 6 and 12 is a **legal question that stays open (E5)**.
* The **updates** rule lives in the review guidelines (2.4.5 (vii)), not in the end-user contract; the
  contract-side update clause is that the license extends to "upgrades provided by Licensor that replace or
  supplement the original Licensed Application" (Standard EULA a.). Maintenance and support for a third-party
  app is the **developer's** duty (Exhibit B item 3; AMS "APP MAINTENANCE AND SUPPORT"). The report's chain of
  reasoning must say this; today it jumps from the guidelines to "the store conditions forbid modification".

---

## 4. App Review Guidelines — the quotes hold, with two truncations

All items the report cites were checked verbatim against the live text (S-02, identical to the report's copy):

| Cited | Verbatim in the live guidelines | Result |
|---|---|---|
| 2.4.5 (i) | "They must be appropriately sandboxed, and follow macOS File System Documentation." | **quote correct**, but the item continues: "They should also only use the appropriate macOS APIs for modifying user data stored by other apps (such as bookmarks, Address Book, or Calendar entries)." |
| 2.4.5 (ii) | "They must be packaged and submitted using technologies provided in Xcode; no third-party installers allowed. They must also be self-contained, single app installation bundles and cannot install code or resources in shared locations." | **quote correct and complete** |
| 2.4.5 (iv) | "They may not download or install standalone apps, kexts, additional code, or resources to add functionality or significantly change the app from what we see during the review process." | **quote correct and complete** |
| 2.4.5 (vi) | "They may not present a license screen at launch, require license keys, or implement their own copy protection." | **quote correct and complete** |
| 2.4.5 (vii) | "They must use the Mac App Store to distribute updates; other update mechanisms are not allowed." | **quote correct and complete** |
| 2.5.2 | "Apps should be self-contained in their bundles, and may not read or write data outside the designated container area, nor may they download, install, or execute code which introduces or changes features or functionality of the app, including other apps." | **quote correct**, but the item continues with an exception: "**Educational apps designed to teach, develop, or allow students to test executable code may, in limited circumstances, download code provided that such code is not used for other purposes. Such apps must make the source code provided by the app completely viewable and editable by the user.**" |
| 2.5.1 | "Apps may only use public APIs and must run on the currently shipping OS." | **quote correct** |
| 3.2.2 (iii) | "Artificially increasing the number of impressions or click-throughs of ads, as well as apps that are designed predominantly for the display of ads." (line 321) | **quote correct and complete** |
| 3.2.2 (x) | "Apps must not force users to rate the app, review the app, download other apps, or other store-related actions in order to access functionality, content, or use of the app." (line 328) | **quote correct** (the item continues with the sentence on incentivising other actions) |

**Structure note:** 2.4.5 has the items **(i) to (ix)**; the report cites (i), (ii), (iv), (vi), (vii) —
the numbering is correct. 2.4.5 (iii), (v), (viii), (ix) are not relevant to Vocument (auto-launch, root
privileges, deprecated technologies, localization in one bundle).

**What follows:** the report's guideline evidence is **sound**; the two truncations (F-05) are minor, but the
2.5.2 exception is worth passing to the legal review, because it is the only place in the guidelines where
Apple itself requires that downloaded code be "completely viewable and editable by the user" — structurally
close to the AGPL's demand — and its limitation to "educational apps" is what keeps it from applying to
Vocument.

**One precision point on 2.4.5 (ii) and (vii).** What these items restrict is the **distribution and update of
the binary** through the store. They do not by their wording forbid the developer from publishing the
**corresponding source** elsewhere, and AGPL §6(d) does not require the source offer to happen *inside* the
store. The report's table row for 2.4.5 (ii) ("verletzt P-6(d) … für die Binärverbreitung") and its §3.2(b)
sentence ("the developer may not offer the users a modified version (2.4.5 (vii))") state consequences as if
they were the rule's text. Narrowing this is a matter of precision, and it keeps the conclusion intact:
what 2.4.5 (ii)/(vii) remove is the developer's *own* channel for the **binary** — which is exactly where the
AGPL's §6 "Installation Information" duty would have to be fulfilled. Whether the remaining arrangement still
satisfies §6 and §12 is a legal question (E5). (Finding **F-07**.)

---

## 5. What the report does **not** claim — and what it claims too strongly

### 5.1 The "blocked" / "structurally inadmissible" language is a legal determination, not a technical analysis

The report and the living digest use, without a legal qualification at the point of use:

| Place | Wording | Kind of statement |
|---|---|---|
| Report §0.1 | "**Der Mac App Store ist der einzige Kanal, der mit der festgelegten AGPL-3.0-or-later-Strategie strukturell kollidiert.**" | legal determination |
| Report §3.2(b) | "**aber es bleibt unzulässig**, solange der Store die Installation modifizierter Fassungen verhindert (P-7 verletzt)" | legal determination |
| Report §3.2(b) | "**Bewertung: (b) — der Kanal ist mit der festgelegten Strategie nicht vereinbar**" | a legal verdict in the same sentence as the marker that says a legal review is required |
| Report §5.1 table | "**Direkter Konflikt mit P-7**", "**Blockiert** die geplante Wasmtime-Plugin-Architektur", "eine eigene Kopierschutz-/Schlüsselmechanik, die das unterbindet, ist unzulässig" | legal determination |
| Living digest §1 | "**Of the three distribution channels, exactly one collides structurally with that strategy: the Mac App Store.** … **The collision is not a matter of taste.**" | legal determination |
| Living digest §2 table | "**Mac App Store — collides**" | legal determination |

**What the contract layer supports on its own (technical, verifiable):** 2.4.5 (vi) and (vii) say what they
say; Schedule 1 §3.1 says what it says; Exhibit B item 1 makes the AMS Terms binding on the EULA; the
Standard EULA contains an anti-modification clause with an open-source carve-out. **None of these strings is
in itself a legal conclusion.**

**What is a legal question and must stay open (E5):** whether the store conditions, read with the applicable
law, prevent the simultaneous satisfaction of the AGPL's §6 (conveyance, Installation Information) and §12
duties — i.e. the very question §12 poses. That also decides whether the open-source carve-out in the
Standard EULA and the AGPL §7 "Additional Terms" route can carry a store build.

**The internal contradiction:** the report's own classification legend says **(b) = needs a legal review**, and
row **AS-1** is indeed classified **(b)**. The summary (§0.1), §3.2(b) and §5.1 nevertheless state the
incompatibility as established. The living digest drops the classification entirely and states the verdict
only. The owner's decision record is: **the Mac App Store is deferred, the legal clearance (E5) is open, no
premature finding of inadmissibility and none of admissibility.** (Finding **F-06**.)

### 5.2 The exclusivity claim ("only the Mac App Store") is broader than what is proven

* The claim is **correct in direction** for the three channels in scope, and the technical reason is precise:
  the Mac App Store is the only one of the three whose store rules contain an express anti-modification rule
  (2.4.5 (vi)), an express update monopoly (2.4.5 (vii)), a ban on third-party installers (2.4.5 (ii)) and a
  ban on downloading additional code (2.4.5 (iv)).
* But the **negative** side is not fully evidenced: the report itself records (row **D-6**) that the Google
  policy pages "are JS-driven and were not extracted as text", i.e. the claim "Play imposes no such
  restriction" rests on the report's reading rather than on a quoted clause. The report's own §11 marks the
  wording as (b) — legal review. So the exclusivity is *asserted* on one side and *assessed* on the other.
* The claim also needs a **scope** note: the iOS/iPadOS App Store (roadmap, "later") has the same structural
  features — the report's own rows **I-3** (no code download, 2.5.2), **I-4** ("update of the native core only
  through the store") and **I-11** ("the AGPL remains effective on iOS as well") — and the same ADPLA
  Schedule 1 applies. The strategy decision record lists the store decision as an open owner item
  (**O4** in `docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`). (Finding **F-08**.)

---

## 6. Findings

| # | Severity | What | Evidence | What the owner must do |
|---|---|---|---|---|
| **F-01** | Material (presentation) | The task's hypothesis is refuted: **Schedule 1 does appoint Apple as commissionaire** (§1.1 (ii), §1.3) and uses the identical "agent and/or commissionaire" wording in §3.1. The report's conclusion holds for a free app — but the report never quotes §1.1/§1.3, so a reader can wrongly conclude the opposite. | S-01 (Schedule 1 §1.1, §1.3, §3.1; Exhibit A §1/§2) | Add Schedule 1 §1.1 (i)/(ii), §1.3 and the Exhibit A region split to the report's §5.1/§5.1a, and state in one sentence that the *heading* "Appointment of Agent" is narrower than the *body* of Schedule 1. |
| **F-02** | Material (living digest) | The living digest attributes the agency clause to "ADPLA (**Schedule 2**, 3.1)". Vocument is free (E3) → **Schedule 1** applies (§7.1); Schedule 2 is for fee-based apps (§7.2). The full report has the qualifier, the digest does not. | S-01 (§7.1, §7.2); S-10 (`docs/audit/store-requirements.md`, §1) | Correct the citation in the living digest to Schedule 1, 3.1 (+ ADPLA §7.1) or state both. |
| **F-03** | Material (missing source) | The report never uses the **Apple Media Services Terms and Conditions**, although they bind the store distribution: the developer's contract incorporates them (Exhibit B items 1, 2, 3, 10), and the Standard EULA contains an anti-modification clause **with an open-source carve-out** ("… or to the extent as may be permitted by the licensing terms governing use of any open-sourced components included with the Licensed Application"). The report's claim that the store terms "ask for terms that the AGPL cannot grant" is unsupported at this layer. | S-03 (Section O, Standard EULA a.); S-01 (Exhibit B items 1–3, 10) | Add the AMS Terms as a source; pass the carve-out to the legal review (E5) explicitly. Do not treat the carve-out as a solution — it speaks of "components", not of the covered work. |
| **F-04** | Precision | The report's §5.1 conflict table quotes the **Schedule 2** clauses (4.2/4.3/7.3, "Exhibit D") although Vocument runs under **Schedule 1** (3.2/3.3/6.3, "Exhibit B"). Only 3.1 and 6.3 are corrected in §5.1a. | S-01 (Schedule 1 §3.2, §3.3, §6.3; Schedule 2 §4.2, §4.3, §7.3) | Add the Schedule 1 counterparts to the table, or state once that the cited Schedule 2 numbering is used as a shorthand and give the Schedule 1 mapping. |
| **F-05** | Minor (quotes) | Two guideline quotes are truncated: 2.4.5 (i) has a second sentence; 2.5.2 continues with the exception "**Educational apps designed to teach, develop, or allow students to test executable code may, in limited circumstances, download code provided that such code is not used for other purposes. Such apps must make the source code provided by the app completely viewable and editable by the user.**" | S-02 (2.4.5 (i), 2.5.2) | Mark the truncation with an ellipsis and add the 2.5.2 exception to the legal review's material (it is the only guideline place where Apple itself demands that downloaded code be editable by the user — and its limitation to educational apps is why it does not apply to Vocument). |
| **F-06** | Material (decision discipline) | The report and the living digest state the incompatibility as a **legal finding** ("blocked", "strukturell unzulässig", "collides", "The collision is not a matter of taste") while the report's own classification marks that row (AS-1) as **(b) = legal review** and the owner has kept **E5** open and deferred the channel. | S-11 (§0.1, §3.2(b), §5.1), S-10 (§1, §2 table) | Restate as: the store conditions **contain** the obligations quoted; whether they can be satisfied simultaneously with the AGPL duties is **open (E5)**; the channel is **deferred**. Keep the technical findings; remove the verdict wording. |
| **F-07** | Precision | 2.4.5 (ii)/(vii) restrict the **binary** (no third-party installer, updates only via the store). They do not by their wording forbid publishing the **source** elsewhere, and AGPL §6(d) does not require the source offer to sit inside the store. The report states these consequences as if they were the rules' text ("der Entwickler darf den Nutzern keine modifizierte Fassung anbieten (2.4.5 (vii))"). | S-01 (§6 via S-06), S-02 (2.4.5 (ii), (vii)) | Narrow the wording to what the rule says; keep the conclusion in the open column. |
| **F-08** | Scope | "Only the Mac App Store" is asserted for the whole product; the iOS/iPadOS App Store (roadmap) has the same structure (2.5.2, store-only updates — report rows I-3, I-4, I-11), and the negative claim for Google Play rests on an unverified reading (report row D-6). | S-11 (I-3, I-4, I-11, D-6), S-12 (O4) | Scope the claim ("of the three channels in scope") and add a sentence that the iOS App Store, when it comes, faces the same question; keep the Play wording as (b). |
| **F-09** | Minor (source hygiene) | The ADPLA page and the Review Guidelines page carry **no version or date**; the report's source list presents them as if they were dated ("Abrufdatum 2026-10-21" is fine, but no version marker). Currency was proven here only by byte comparison. | S-01, S-02; byte comparison in §1 | Record the retrieval date **and** the SHA-256 of the retrieved HTML in the source list, so a later reader can repeat the comparison. |
| **F-10** | Minor (task reference) | The "Apple Media Services Terms and Conditions, section 5 »Developer Content Distribution«" named in the assignment **does not exist** in the current document (structure A–T; the App Store terms are in section O). `developer.apple.com/support/terms/apple-media-services-terms-conditions/` returns a "Page Not Found" body. | S-03, S-05 | Nothing to change in the project; recorded so that nobody quotes a section that does not exist. The verification was done against the provisions that do exist. |

**Confirmed without defect (no action):** the schedule assignment (K-4, §5.1a); the "Mac App Store" = 0 hits in
the ADPLA; all quoted review-guideline wording; the AGPL §12 and §6 wording (S-06/S-07); the currency of the
ADPLA and the guidelines as of 2026-10-21; the report's own conclusion that direct macOS distribution, Google
Play and the direct APK are the channels that fit inside the fixed strategy (as a technical statement about
the store rules).

---

## 7. What is open (and stays open, because it is a legal question)

1. **E5 — the AGPL clearance per channel.** Whether the store conditions (2.4.5 (vi)/(vii), Schedule 1 §3.1,
   Exhibit B, the AMS Terms / Standard EULA) can be satisfied simultaneously with AGPL §§ 6 and 12 is a legal
   assessment. The report must not pre-empt it; this verification does not make it either.
2. **The effect of the open-source carve-out** in the Standard EULA ("… to the extent as may be permitted by
   the licensing terms governing use of any open-sourced components included with the Licensed Application") on
   the AGPL duties of §6 (Installation Information) and §12.
3. **The AGPL §7 route** (additional terms with the consent of all rights holders, including MuPDF) — an owner
   decision (report E-3), unchanged by this review.
4. **The enforceability of the end-user EULA** in the storefronts where Apple acts as commissionaire in its own
   name (Exhibit A, Section 2).
5. **The Google Play negative** (no DRM/anti-modification or update-monopoly rule) — the report's own row D-6
   marks the wording as unverified because the policy pages are JS-driven.

## 8. What was **not** done

* No file in `/workspace/vocument` was changed; no commit was made. (Read-only verification: `git log`,
  `git status`, file reads only.)
* **No licence strategy was changed or decided.** The deferral of the Mac App Store and the open legal
  clearance (E5) stand as the owner decided.
* No contract wording was reconstructed from memory; every Apple quote above comes from a document retrieved
  on 2026-10-21.

---

## 9. Source list

**Retrieval date for all live sources: 2026-10-21.** Raw copies and text extractions are in
`/workspace/.mavis/plans/plan_cb381bba/workspace/src/`.

| ID | Source | Version / date | Belegte Aussage (verbatim or near-verbatim) |
|---|---|---|---|
| **S-01** | Apple Developer Program License Agreement — `https://developer.apple.com/support/terms/apple-developer-program-license-agreement/` | live, HTTP 200, 848 279 B, SHA-256 `2c8d931943d195a9…`; extracted text **identical** to the copy used by the report | §7.1 ("…appoint Apple and Apple Subsidiaries as Your legal agent and/or commissionaire pursuant to the terms of **Schedule 1** for Licensed Applications designated by You as free-of-charge applications"); §7.2 (Schedule 2 = fee-based, Schedule 3 = paid custom apps); Schedule 1 §1.1 (i)/(ii) (agent for Exhibit A §1 regions, **commissionaire for Exhibit A §2 regions**); §1.3 ("principal and agent, or principal and commissionaire, as the case may be"); §3.1 ("in the course of acting as agent and/or commissionaire for You, is hosting … on Your behalf"); §3.2 (own EULA, **Exhibit B**, "Apple shall enable each end user to review Your EULA … at the time that Apple delivers that Licensed Application"); §3.3 ("solely between You and the end user"); §6.3 ("Apple reserves the right to cease marketing, offering, and allowing download by end users … at any time, with or without cause", incl. "(iv) … including without limitation the App Review Guidelines"); Exhibit A §1 "Apple as Agent" (Canada, Australia/New Zealand, United States, LATAM, Japan) and §2 "Apple as Commissionaire" ("You appoint Apple Distribution International Ltd. as Your commissionaire …"); Exhibit B items 1, 2, 3, 10 (AMS Terms binding on the EULA; Usage Rules; maintenance/support; Apple as third-party beneficiary); Schedule 2 §3.1/§4.2/§4.3/§7.3; Schedule 3 §3.1/§4.2/§7.3; `grep -c -i "Mac App Store"` → **0** |
| **S-02** | App Review Guidelines — `https://developer.apple.com/app-store/review/guidelines/` | live, HTTP 200, 240 818 B, SHA-256 `4ab653ded784df81…`; extracted text **identical** to the report's copy; the page carries **no version or date** | 2.4.5 "Apps distributed via the Mac App Store have some additional requirements to keep in mind:" with (i)–(ix) verbatim; 2.5.1; 2.5.2 (incl. the educational-app exception); 3.2.1; 3.2.2 "Unacceptable" (i)–(x), (iii) at line 321 and (x) at line 328 |
| **S-03** | Apple Media Services Terms and Conditions — `https://www.apple.com/legal/internet-services/itunes/` | live, HTTP 200, 159 666 B raw; no version number on the page; §T "CONTRACT CHANGES" allows Apple to modify the agreement at any time | TOC A–T; §O "ADDITIONAL APP STORE TERMS (EXCLUDING APPLE ARCADE APPS)"; "LICENSE OF APP STORE CONTENT" ("Apple acts as an agent for App Providers in operating the App Store and is not a party to the sales contract or user agreement"); Standard EULA "a. Scope of License" (anti-modification clause **with the open-source carve-out**); "APP MAINTENANCE AND SUPPORT"; §F Usage Rules (App Store Content; "may not modify or use modified versions of such software" refers to Apple's software) |
| **S-04** | AMS Terms, US variant — `https://www.apple.com/legal/internet-services/itunes/us/terms.html` | live, HTTP 200, 156 608 B | cross-check of S-03 (same section O structure and wording) |
| **S-05** | Index of developer agreements — `https://developer.apple.com/support/terms/` | live, HTTP 200 | the agreement list (ADPLA + Schedules 2/3 PDFs, TestFlight ToS, App Store Connect ToS); the AMS Terms are **not** hosted under developer.apple.com; `…/apple-media-services-terms-conditions/` returns a "Page Not Found" body |
| **S-06** | AGPL-3.0-or-later, vendored in the repository — `/workspace/vocument/LICENSES/AGPL-3.0-or-later.txt` | commit `bbc2d0b`, SHA-256 `0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0` | §0 ("Corresponding Source"), §6 (conveyance (a)–(e), "Installation Information", "designated place"), §12 ("If you cannot convey a covered work so as to satisfy simultaneously your obligations under this License and any other pertinent obligations, then as a consequence you may not convey it at all.") |
| **S-07** | GNU mirror of the same text — `https://mirrors.kernel.org/gnu/Licenses/agpl-3.0.txt` | live, HTTP 200 | cross-check of S-06 (byte-identical) |
| **S-08** | Canonical AGPL URL — `https://www.gnu.org/licenses/agpl-3.0.txt` | **not retrievable from this environment** (TCP timeout, no HTTP status) | used only via S-06/S-07; the canonical URL is marked as such in the source list |
| **S-09** | `https://www.fsf.org` | **not retrievable from this environment** (TCP timeout) | the position of the FSF/GNU on App Store distribution remains unverified here → (b) |
| **S-10** | Living digest — `/workspace/vocument/docs/audit/store-requirements.md` | commit `bbc2d0b`, 167 lines | §1 ("Of the three distribution channels, exactly one collides structurally with that strategy: the Mac App Store."; "the Apple Developer Program License Agreement (Schedule 2, 3.1) makes Apple the *agent and commissionaire* of the developer"); §2 table ("Mac App Store — **collides**"); §4 (E-1 … E-10, E-5 "The legal review of the AGPL compliance per channel … No agent can replace it"); §6 ("the store upload is prepared but not taken") |
| **S-11** | Report under review — `/workspace/.mavis/plans/plan_25670a94/outputs/store-requirements/licensing-and-store-requirements.md` | 411 lines, 2026-10-21 | §0.1; K-4; §3.2(b); §5.1 (clause table); §5.1a; §5.3 (AS-1 … AS-6); §6 (table 6.2); §7.1 (D-6); §8 (I-3, I-4, I-11); §9 (E-1 … E-10); §10 (source list S-01…S-35) |
| **S-12** | Licence decision — `/workspace/vocument/docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md` | commit `bbc2d0b` | E1 (published completely under AGPL-3.0-or-later), E2 (no commercial MuPDF licence), **E3 ("The product is free of charge")**), O4 ("The decision about the store … the project owner") |
| **S-13** | TestFlight Terms and Conditions — `https://www.apple.com/legal/internet-services/itunes/testflight/` (linked from S-05) | live, HTTP 200 | only the TestFlight distribution path; not applicable to a public Mac App Store release — noted so that no one confuses it with the store terms |
| **S-14** | Plan workspace (raw copies) — `/workspace/.mavis/plans/plan_cb381bba/workspace/src/` | 2026-10-21 | `apple_adpla_live.txt(.raw)`, `apple_guidelines_live.txt(.raw)`, `apple_ams_terms.txt(.raw)`, `apple_ams_terms_us.txt(.raw)`, `fetch.sh` — the reproduction material for this verification |

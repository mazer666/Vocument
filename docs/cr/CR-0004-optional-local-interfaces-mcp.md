# CR-0004 — Optional local interfaces (MCP) as a later milestone

**Status:** proposed
**Date:** 2026-10-22
**Proposer role:** ROLE-01 (Product & Requirements Steward)
**Reviewer roles:** ROLE-02 (Chief Architect), ROLE-09 (Security Engineer), ROLE-10 (Privacy Engineer), ROLE-13 (QA & Test Engineer), ROLE-18 (Licensing & Open-Source Reviewer)
**Binding:** the licensing decision E1–E5 (`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`)

> This is a **change request**, not an architecture decision record. It adds a
> milestone to the roadmap and states what has to be true before that milestone
> is built. The architecture of the interface itself — if and when it is built —
> is decided in an ADR at that time.
>
> **The history of this document.** It was filed as
> `docs/adr/ADR-0021-optional-local-interfaces-mcp.md` and has been moved here
> on 2026-10-22, while it was still `proposed` and before any review. The
> classification follows `PROJECT_MANAGEMENT.md`: §5 requires a CR for a
> material product change, and a CR has to state the **milestone impact**;
> §4 requires a deferred feature to name **the CR** that defers it. This
> document adds the milestone M7 and defers the macOS App Store and the
> Android variants — it does not decide an architecture of the product that
> exists today. The decision of the architecture of the interface belongs to
> an ADR at the time the interface is built. Nothing is lost by the move:
> the file history carries the rename.

---

## Problem

A user of Vocument may want to work with a document **from an assistant**:
the assistant reads the pages of a document, searches in it, marks a place
that has to go, writes a comment and saves the result — without the user
having to drive the window of the application for every step.

The **Model Context Protocol (MCP)** is the interface that is used for that
today: a program that exposes *tools* over a local pipe, which an assistant
can call. Other interfaces of that kind (a local command line, a local
socket, a plugin API) serve the same purpose.

The product has no such interface, and no milestone exists for one. Every
ad-hoc way in — a second implementation of the features, a private script that
writes into a PDF beside the application — is the failure mode the red team
found repeatedly in the redaction: **a path that is not the verified one**.

## Requested Change

Add the milestone **M7 — optional local interfaces (MCP)** to the roadmap,
positioned **after** M6 (the Linux shell), and fix its boundaries:

| Question | Decision |
|---|---|
| Which interface? | **MCP over stdio** (the local pipe of the protocol) is the first one. The core of the interface is *not* tied to MCP: the tools of the product are ordinary functions of the core, and the MCP server is a thin shell around them. A second interface (a command line, a local socket) is the same functions with another shell. |
| Optional? | **Yes.** The interface is not started by default, it is not required for any function of the product, and the product is complete without it. A build without it is a normal build. |
| Where does it run? | **On the machine of the user.** The server speaks over a local pipe (stdio) or a local socket that it opens itself. It asks for **no** network right — the same rule as the applications (no `INTERNET` permission on Android, no network entitlement on macOS). |
| What does it do? | **Only what the product already does**: open a document, read the pages, the words and the findings, search, mark a place, remove a marked place, write a comment, save. |
| How is a removal verified? | **By the same code, with the same proof.** The interface calls the same core, the same redaction, the same four checks, the same refusal. An interface that could remove a text *without* the verification would be a second, weaker path — and it is not built. |
| Cloud, accounts, telemetry? | **No.** The interface does not send anything anywhere, knows no account and writes no telemetry. The data of the user stays on the device (E4). |
| Licence? | **The same as the product**: AGPL-3.0-or-later, with MuPDF, free, no proprietary variant, no commercial MuPDF licence (E1–E3). The interface is part of the product — it is not a proprietary add-on. |
| A server in the network? | **Not in this milestone.** A function that is reached over the network needs a new architecture and a new licence review (E4, §8 of the licensing decision) — and a different threat model: the redaction of a document over a network is a different thing than the redaction of a document on the disk of the user. |
| macOS App Store? | The Mac App Store is **deferred** by the decision of the project owner. An MCP server inside a sandboxed application needs an entitlement and a helper; that is a macOS question and it is answered with the Mac App Store question, not before it. |

### Why not a "second API"

A product with two paths has two sets of rules. The interface of this
milestone is therefore **not** a new implementation of the features of the
product — it is a shell around the existing core:

```
        MCP (stdio)                ← this milestone: a thin shell
             │
             ▼
   vocument-core (session, redaction, comments, search, save)
             │
             ▼
      engines (MuPDF, OCR, imaging)
```

The shell may not hold a document, may not write into a file and may not
answer "the removal is done" without the report of the core. Every write
goes through the core, and every write that the core refuses is refused by
the interface as well — with the same reason.

## Reason

- The wish is real and repeated, and a user should not have to drive a window
  for every step of a review.
- Building the interface **on** the core costs one thin shell; building it
  **beside** the core costs a second, unverified implementation of the
  redaction — which is exactly the defect class this project keeps finding.
- An optional interface that stays local does not weaken any of the rules that
  the project has already established (`NET-001`, `LIC-002`).
- Writing it down as a **later** milestone keeps it out of the critical path:
  nothing in M1–M4 waits for it, and the product is complete without it.

## Impacted Requirement IDs

- Confirms `NET-001` (strict offline, no automatic external resource loading)
  — the interface asks for no network right.
- Confirms `NET-002` — an explicit, per-action allowance, never a default.
- Confirms `LIC-002` (no mandatory service, no cloud, no account, no
  telemetry, no network right; a server function needs a new architecture and
  a licence review before it is built).
- Touches `REDACT-001`, `REDACT-002`, `REDACT-005` — the interface must not
  weaken any of the four verification steps.
- **No requirement is weakened.** No acceptance criterion is removed, and no
  `NOT RUN` is turned into a claim.

## Impacted Architecture

- `core/vocument-core/` — unchanged. The interface calls the existing session,
  redaction, comment and save paths; no new write path is introduced.
- `platform/vocument-ffi/` — the interface consumes the existing UniFFI
  surface. It does **not** add platform-specific functions.
- **New** — a shell module around the core (a separate binary or process),
  off by default, started explicitly by the user.
- The interface is **not** a plugin host and does not extend the existing
  `PluginHost` boundary; it is a consumer of the core, never a provider.

## Impacted Platforms

- macOS — a local stdio process started by an assistant. The sandbox question
  (entitlement or helper) is answered together with the Mac App Store, which
  is deferred.
- Android — **not part of this milestone.** Android has no local stdio pipe
  for an assistant; a variant over a content provider or a bound service is a
  different design and needs its own CR.

## Milestone Impact

- **No existing milestone changes.** M0, M1 and M2 keep their scope and their
  acceptance records.
- A new milestone **M7 — optional local interfaces (MCP)** is added to the
  roadmap **after** M6 (the Linux shell, `CR-0003`).
- It is **not** a prerequisite for anything. The product is complete and
  usable without it, and nothing in M1–M4 waits for it.
- `ROADMAP.md` M7 is linked to this CR instead of to an ADR.

## Acceptance / Test Changes

These are the conditions that have to be **proven before** M7 is started, not
claims about work that has been done:

1. **The core is the only path.** The tools of the interface call the core; no
   tool writes into a document on its own. This is checked by a test that
   opens a document through the interface, removes a place and reads the result
   back — and by a test that a *refused* redaction is refused through the
   interface as well, with the same reason.
2. **The same workflow, proven.** The chain "open → read → search → mark →
   remove → comment → save → reopen" is proven **through the interface** with
   the same corpus that the applications use, and the evidence is in the
   acceptance record of the milestone.
3. **The interface is off by default and stays local.** The proof: the process
   opens no network socket, the manifest of Android asks for no network right,
   and the entitlements of macOS carry no network right.
4. **The licence duties are fulfilled for the interface as well**: it is part
   of the AGPL product, the notice of the third-party components covers the
   libraries that it uses, and the source archive of a release contains it.
5. **The legal clearance (E5) before every publication** — the interface does
   not change that rule.

## Risks

- **Scope risk (Medium):** an assistant-facing interface invites feature
  requests that are not product features. Mitigation: the milestone accepts
  only what the product already does.
- **Security risk (High):** an interface that can change the document of the
  user widens the attack surface. Mitigation: local only, off by default,
  every write through the verified core, and the refusal of the core is
  reproduced with its reason.
- **Licensing risk (Medium):** a second shell could drift towards a
  proprietary add-on. Mitigation: E1–E3 apply unchanged; the interface ships
  under the AGPL with the product and is part of the source archive.
- **Maintenance risk (Medium):** an interface that has to follow every change
  of the core costs maintenance. Mitigation: it is a shell over ordinary core
  functions, not a parallel API.

## Open decisions

These are deliberately **not** decided in this CR:

- **The version of the protocol.** The interface is written against a specific
  version, and the version is named in the documentation of the milestone.
- **The sandbox of macOS.** An assistant that starts a program inside a
  sandboxed application needs an entitlement or a helper — answered together
  with the Mac App Store.
- **Android.** No local stdio pipe exists there; a content provider or a bound
  service is a different design.
- **The trust.** The interface can read and change the documents of the user.
  Who may call it, and what it may do with a document, is decided by the
  permissions of the interface — not by the assistant.
- **The distance to the core.** Read functions and write functions are
  exposed; the write functions are the ones with a proof obligation.

## Decision

**proposed** — awaiting review by ROLE-02, ROLE-09, ROLE-10, ROLE-13 and
ROLE-18. Nothing is built by this CR: it records the milestone and its
boundaries.
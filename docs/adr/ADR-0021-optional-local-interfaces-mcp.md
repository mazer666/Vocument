# ADR-0021 — Optional local interfaces (MCP) for the product

**Status:** proposed · **Date:** 2026-10-22 · **Decides for:** the milestone
**M7 — optional local interfaces** of the roadmap · **Binding:** the licensing
decision E1–E5 (`docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md`)

---

## 1. The wish

A user of Vocument may want to work with a document **from an assistant**:
the assistant reads the pages of a document, searches in it, marks a place
that has to go, writes a comment and saves the result — without the user
having to drive the window of the application for every step.

The **Model Context Protocol (MCP)** is the interface that is used for that
today: a program that exposes *tools* over a local pipe, which an assistant
can call. Other interfaces of that kind (a local command line, a local
socket, a plugin API) serve the same purpose.

## 2. The decision

**Vocument gets an optional, local, off-by-default interface — and it is
built on the existing core, not beside it.**

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

## 3. Why not a "second API"

A product with two paths has two sets of rules. The failure mode is the one
that the last rounds of the red team found again and again in the redaction:
a path that is *not* the verified one. The interface of this milestone is
therefore **not** a new implementation of the features of the product — it
is a shell around the existing core:

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

## 4. What has to be true before it is built

1. **The core is the only path.** The tools of the interface call the core;
   no tool writes into a document on its own. This is checked by a test that
   opens a document through the interface, removes a place and reads the
   result back — and by a test that a *refused* redaction is refused through
   the interface as well, with the same reason.
2. **The same workflow, proven.** The chain "open → read → search → mark →
   remove → comment → save → reopen" is proven **through the interface** with
   the same corpus that the applications use, and the evidence is in the
   acceptance record of the milestone.
3. **The interface is off by default and stays local.** The proof: the
   process opens no network socket, the manifest of Android asks for no
   network right, and the entitlements of macOS carry no network right.
4. **The licence duties are fulfilled for the interface as well**: it is part
   of the AGPL product, the notice of the third-party components covers the
   libraries that it uses, and the source archive of a release contains it.
5. **The legal clearance (E5) before every publication** — the interface
   does not change that rule.

## 5. The open questions (they are not decided here)

* **The version of the protocol.** The interface is written against a
  specific version of the protocol, and the version is named in the
  documentation of the milestone.
* **The sandbox of macOS.** An assistant that starts a program inside a
  sandboxed application needs an entitlement or a helper — the question is
  answered together with the Mac App Store.
* **Android.** Android has no local stdio pipe for an assistant. A variant
  over a content provider or a bound service is a different design and is
  **not** part of this milestone.
* **The trust.** The interface can read and change the documents of the
  user. Who may call it, and what it may do with a document, is decided by
  the permissions of the interface — not by the assistant.
* **The distance to the core.** The functions of the product that the
  interface exposes are *read* functions and *write* functions. The write
  functions are the ones with a proof obligation.

## 6. The consequence for the roadmap

The milestone **M7 — optional local interfaces (MCP)** is added to the
roadmap **after** M6 (the Linux shell). It is not a prerequisite for anything else: the
product is complete and usable without it, and nothing in M1–M4 waits for
it.

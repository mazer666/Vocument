# Decision Paper: Milestone M2 Scope Realignment and Roadmap Sequence (`CR-0005`)

**Document type:** Formal Proposal and Decision Submission for the Project Owner  
**Status:** **Pending Project Owner Clearance**  
**Date:** 2026-10-23  
**Authors:** Multi-Agent Development Team (`AGENTS.md`) — ROLE-01 (Product Steward), ROLE-02 (Chief Architect), ROLE-20 (Project Manager)  
**References:** `REQUIREMENTS.md`, `ROADMAP.md`, `CR-0005`, `ADR-0018`, `ADR-0019`, `docs/acceptance/M2-ACCEPTANCE.md`

---

## 1. Executive Summary

During the consolidation of Milestone M2 and preparation for Milestone M3, the Multi-Agent Development Team audited requirements, roadmaps, and acceptance records against `AGENTS.md` (Rule 8: *"never silently weaken an acceptance criterion"*; Rule 16: *"never implement or accept stubs"*).

Two items initially listed in early roadmap drafts for M2 were not built in M2:
1. **Structured Document Comparison (`COMPARE-001`, `COMPARE-002`):** Dual-document visual/textual side-by-side comparison.
2. **Rubber Stamps (`WATER-003`):** Juristic overlay stamps (COPY, DRAFT, APPROVED, etc.).

Instead, M2 delivered deep, verifiable security and legal review functionality:
- **M2.1:** True secure redaction with 4-way post-redaction verification.
- **M2.2:** Link and action security inspection (`LINKSEC-001`).
- **M2.3:** Attachment and cryptographic signature inspection (`ATTACH-001`, `SIG-001`).
- **M2.4:** Reviewer comments, `/Text` annotations, and atomic Jurist Tool Palette (`ANNOT-001`).
- **`SAN-001`:** Selective document sanitization (`ADR-0018`).
- **Review UI Panels:** Native macOS panels connecting inspection, palette, and redaction.

Under project governance, **agents do not have the authority to unilaterally accept scope changes or declare milestones complete when original items remain unbuilt**. This decision paper formally presents the options and trade-offs for the Project Owner's explicit clearance.

---

## 2. Analysis of Options

### Option A: Formal Scope Realignment per `CR-0005` (Unanimously Recommended by Agents)
- **Action:**
  1. Formally define M2 as **macOS Professional Review — Security, Inspection & Jurist Annotations**, accepting M2 technical closure based on delivered and verified security slices.
  2. Relocate Rubber Stamps (`WATER-003`) to **M3.4** (Page Workbench: Bates Numbering, Headers/Footers & Stamps), where page-geometry, bounding box overlays, and appearance streams reside.
  3. Establish a dedicated **Track M3.7 (Document Comparison & Diffing)** for `COMPARE-001`–`COMPARE-004`. Comparison is scheduled immediately following M3.1–M3.6, directly leveraging M3's multi-document dual-canvas rendering and thumbnail grid.
- **Pros:**
  - Architecturally coherent: page stamping belongs with page workbench geometry; document comparison requires multi-document viewports.
  - Keeps M3 focused on critical PDF page-tree invariants (`ADR-0019`).
  - Completely transparent and documented without scope debt.
- **Cons:**
  - Formally acknowledges that document comparison was not delivered in M2.

### Option B: Reopen M2 and Block M3
- **Action:**
  - Revoke M2 technical acceptance, reopen M2 development, and implement document comparison and rubber stamps before any M3 Page Workbench work begins.
- **Pros:**
  - Retains the literal initial bullet-point list of the earliest draft roadmap.
- **Cons:**
  - Forces complex multi-document diffing into an incomplete single-document viewport architecture, risking technical debt.
  - Delays foundational Page Workbench invariants (`ADR-0019`) that the entire document manipulation pipeline depends on.

---

## 3. Position and Schedule for Document Comparison (`COMPARE-001`–`004`)

Under Option A, Document Comparison is not dropped or deferred indefinitely; it is assigned a concrete, traceable position:

```text
Milestone Sequence:
M2: Security, Inspection & Jurist Review (Complete)
 └── M3: Visual Page Workbench (M3.0 - M3.6)
      └── M3.7 Track: Document Comparison & Diffing (COMPARE-001 - COMPARE-004)
           └── Entry Gate: Dual-document viewport canvas and text stream differ ready
           └── Delivers: Synchronized dual-pane scroll, visual diff overlay, change summary
 └── M4: FlowDocument Authoring & Office Engine
 └── M5: Windows Native Shell
 └── M6: Linux Native Shell
 └── M7: Optional Local MCP Interface
```

---

## 4. Decision Submission

The Multi-Agent Team requests the Project Owner's decision:

- [ ] **APPROVE OPTION A:** Adopt `CR-0005` scope realignment, confirm M2 closure on delivered security slices, schedule Rubber Stamps to M3.4, and schedule Structured Comparison to M3.7.
- [ ] **APPROVE OPTION B:** Reopen M2; implement comparison and stamps before proceeding to M3.
- [ ] **OTHER INSTRUCTION:** (Specify custom adjustment).

*This decision paper is recorded in `docs/decisions/DECISION-CR-0005-SCOPE-REALIGNMENT.md`.*

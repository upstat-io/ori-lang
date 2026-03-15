---
section: "01"
title: "Plan Status and Evidence Reconciliation"
status: complete
goal: "Produce a contradiction-free completion verdict for all 13 AIMS sections."
depends_on: []
sections:
  - id: "01.1"
    title: "Status Matrix"
    status: complete
  - id: "01.2"
    title: "Evidence Claim Audit"
    status: complete
  - id: "01.3"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Plan Status and Evidence Reconciliation

**Status:** Complete (2026-03-15)
**Goal:** Align section/frontmatter/index statuses with observed code and tests.

## 01.1 Status Matrix

**File(s):** `plans/aims/index.md`, `plans/aims/00-overview.md`, `plans/aims/section-*.md`

**Pre-identified contradictions (2026-03-15 review):**
- `plans/aims/index.md` frontmatter: `status: resolved`
- `plans/aims/00-overview.md` frontmatter: `status: in-progress`
- Overview body §Implementation Sections table (lines 380-392): all 13 sections "Complete"
- Index body Quick Reference table: sections 08, 11, 13 as "In Progress"
- Index keyword section: Section 08 status "In Progress (08.5a: H7 Valgrind gap, ARC unit tests not H-specific)"; Section 11 "In Progress (.fold() bug, SynergyMetrics)"; Section 13 "In Progress (Bug 2 contract refresh, borrowed_rooted_vars)"
- Overview §4 Realization Status: Verification = "Partially Realized", Normalization = partial Bug 2 fix

- [x] Build a 13-row matrix comparing: (a) `plans/aims/index.md` Quick Reference status, (b) `plans/aims/00-overview.md` Implementation Sections table status, (c) each `plans/aims/section-*.md` frontmatter `status:` field. Use the pre-built matrix below as a starting point. (2026-03-15)
- [x] For each contradiction, record the exact file:line in both disagreeing sources. (2026-03-15) See contradiction details below.
- [x] For each section, determine the correct status by checking: section frontmatter, section checklist completion, and whether relevant tests pass (`cargo test -p ori_arc`, `cargo test -p ori_llvm`). (2026-03-15) All 13 sections verified: frontmatter `complete`, all checkboxes `[x]`, 986/986 ori_arc tests pass, 495/495 AIMS tests pass, 22/22 interaction tests pass.

**Pre-built contradiction matrix (from 2026-03-15 code verification):**

| Section | Index status | Overview table | Section frontmatter | Verdict | Notes |
|---------|-------------|---------------|-------------------|---------|-------|
| 01 | Complete | Complete | complete | Consistent | — |
| 02 | Complete | Complete | complete | Consistent | — |
| 03 | Complete | Complete | complete | Consistent | — |
| 04 | Complete | Complete (superseded by 10) | complete | Consistent | — |
| 05 | Complete | Complete (superseded by 10) | complete | Consistent | — |
| 06 | Complete | Complete | complete | Consistent | — |
| 07 | Complete | Complete | complete | Consistent | — |
| 08 | **In Progress (08.5a)** | Complete | **complete** | Index STALE | `index.md:154` vs `section-08:30` (`status: complete`). 08.5a body text (`section-08:572`) also stale ("In Progress"). All checkboxes `[x]`. |
| 09 | Complete | Complete | complete | Consistent | — |
| 10 | Complete | Complete | complete | Consistent | — |
| 11 | **In Progress** | Complete | **complete** | Index STALE | `index.md:217` vs `section-11` frontmatter (`status: complete`). All checkboxes `[x]`. |
| 12 | Complete | Complete | complete | Consistent | — |
| 13 | **In Progress** | Complete | **complete** | Index STALE | `index.md:260` vs `section-13` frontmatter (`status: complete`). Section 13 body line 58 also stale ("Bug 2 partial fix"). Checklist line 1092 confirms full fix. All checkboxes `[x]`. |

**Stale claims requiring correction (grouped by file):**

`plans/aims/index.md`:
- Section 08 keyword status: says "In Progress (08.5a)" — should be "Complete"
- Section 11 keyword status: says "In Progress (.fold() bug, SynergyMetrics)" — should be "Complete"
- Section 13 keyword status: says "In Progress (Bug 2 contract refresh)" — should be "Complete"
- Quick Reference table rows for 08, 11, 13: say "In Progress" — should be "Complete"

`plans/aims/00-overview.md`:
- Frontmatter `status: in-progress` — should be `complete` or `resolved` (all sections are complete)
- Normalization Open Issues: "Full contract refresh deferred" — resolved in code
- Verification status: "Partially Realized" — section file says complete
- "56 ARC unit tests" — actual: 52 (normalize/tests.rs `#[test]` count)
- §6 item 2 says Bug 2 open — resolved in Section 13 checklist and `pipeline/aims_pipeline.rs`
- §6 item 3 says "64 realize" tests — actual: 65
- §6 item 7 says `borrowed_rooted_vars` is Let-only — code (`emit_function.rs:307-316`) also handles Jump
- §4 Analysis row says "12,888 tests" — actual ori_arc test count is 986

`plans/aims/section-13-trmc-realization.md`:
- Says "56 ARC unit tests" — actual: 52
- "Remaining gaps: Bug 2 partial fix" contradicts own checklist which confirms full fix

`plans/aims/section-08-verification.md`:
- Subsection 08.5a status says "In Progress — Valgrind 21/22 (H7 missing)" but H7 was added and frontmatter says `status: complete`

## 01.2 Evidence Claim Audit

**File(s):** `plans/aims/00-overview.md`, `plans/aims/section-08-verification.md`, `plans/aims/section-11-integration-verification.md`, `plans/aims/section-13-trmc-realization.md`

**Pre-identified stale claims (2026-03-15 review):**

Structural claims:
- Overview Module Tree: `contract/` subtree is missing `context.rs` (113-line file that exists in code as `aims/contract/context.rs`)
- Overview `EffectPurityViolation` location: says `verify.rs:64-69` but actual path is `aims/normalize/verify.rs:64-69`

Numeric claims:
- Overview Analysis row: "12,888 tests" — actual ori_arc test count is 986 (`cargo test -p ori_arc --lib`)
- Overview "56 ARC unit tests" — actual normalize `#[test]` count is 52
- Overview "64 realize" tests — actual count is 65
- Section 13 "56 ARC unit tests" — should be 52

Status claims contradicted by code:
- Overview §6 item 2 (Bug 2): listed as open — actually fully resolved via `run_second_pass()` in `pipeline/aims_pipeline.rs` performing full `extract_contract()` re-extraction
- Overview §6 item 7 (`borrowed_rooted_vars`): described as Let-alias-only — code (`emit_function.rs:307-316`) also handles Jump terminator block-param passing via fixpoint iteration
- Overview §4 Normalization "Full contract refresh deferred": resolved by passing current `contracts` map as peer context
- Overview §4 Verification "Partially Realized": all section files (08, 11, 13) have `status: complete` in frontmatter
- Overview §6 item 7 `borrowed_param_vars` removal: confirmed removed (no matches in arc_emitter/)

- [x] For each claim in the pre-identified list above, verify current code state by running the relevant command or inspecting the referenced file. Mark as valid/stale/contradicted. (2026-03-15) Results:
  - Module Tree missing `context.rs`: **STALE** — `contract/context.rs` exists (confirmed via `find`)
  - `EffectPurityViolation` location `verify.rs:64-69`: **Valid** (path is `aims/normalize/verify.rs:64-69`)
  - "12,888 tests": **STALE** — actual count 986 (`cargo test -p ori_arc --lib`: 986 passed)
  - "56 ARC unit tests": **STALE** — 52 `#[test]` in `normalize/tests.rs`
  - "64 realize tests": **STALE** — 65 `#[test]` in `realize/tests.rs`
  - Section 13 "56 ARC unit tests": **STALE** — 52
  - Bug 2 "open": **STALE** — `run_second_pass()` at `pipeline/aims_pipeline.rs:426-472` does full `extract_contract()` re-extraction
  - `borrowed_rooted_vars` Let-only: **STALE** — `emit_function.rs:310-311` also handles Jump block-param passing
  - "Full contract refresh deferred": **STALE** — fully implemented in code
  - Verification "Partially Realized": **STALE** — section 08 frontmatter `complete`, all checkboxes `[x]`
  - `borrowed_param_vars` removal: **Valid** — no matches in arc_emitter/
- [x] Check `plans/aims/00-overview.md` Module Tree against actual directory listing of `compiler/ori_arc/src/aims/` (run `ls -R`) to find any missing files. (2026-03-15) Found: `contract/context.rs` missing from tree (exists in code). All other files match.
- [x] Verify `#[expect(dead_code)]` on `EffectPurityViolation` in `aims/normalize/verify.rs` is still present and its documented reason ("deferred to effect-handler implementation") is accurate. (2026-03-15) Confirmed: `verify.rs:64-68` has `#[expect(dead_code, reason = "deferred to effect-handler implementation — Ori v1 has no effect handlers, so non-linear resumption cannot occur")]`. Accurate — Ori has no effect handlers.
- [x] For each stale/contradicted claim, record the correction needed (these feed into Section 04 plan sync). (2026-03-15) 15 corrections recorded across 4 files — see stale claims list above and Section 04 for targeted edits.

## 01.3 Completion Checklist

- [x] Contradiction matrix complete for all 13 sections. (2026-03-15) 10 consistent, 3 stale (08, 11, 13 — all index-only contradictions).
- [x] Every stale claim has a correction target. (2026-03-15) 15 corrections across 4 files recorded.
- [x] Completion verdict is evidence-backed and reproducible. (2026-03-15) **Verdict: ALL 13 AIMS sections are COMPLETE.** Evidence: all section frontmatter `complete`, all checkboxes `[x]`, `cargo test -p ori_arc --lib` 986/986 pass, `cargo test -p ori_arc --lib -- aims` 495/495 pass, `cargo test -p ori_llvm aims_interactions` 22/22 pass. Stale text in index.md (3 keyword statuses, 3 QR table rows), overview.md (frontmatter + ~10 body claims), section-13 (2 body claims), section-08 (1 body text). All are documentation drift — no code issues.

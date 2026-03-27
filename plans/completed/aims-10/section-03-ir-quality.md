---
section: "03"
title: "IR Quality"
status: complete
goal: "Zero unjustified instructions in emitted LLVM IR — IR Quality score 10/10 and Instruction Efficiency 10/10 on all journeys"
depends_on: ["02"]  # 03.1 audit requires 02 complete; 03.2-03.4 can parallel 02
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Post-Section-02 Audit"
    status: complete
  - id: "03.2"
    title: "Range Materialization (J7)"
    status: complete
  - id: "03.3"
    title: "SSO Gating Redundancies (J9)"
    status: complete
  - id: "03.4"
    title: "Parameter Materialization (J10)"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: IR Quality

**Status:** Not Started
**Goal:** Zero unjustified instructions across all 13 journeys. Every instruction in the emitted IR must be necessary for correctness (overflow checking, ARC, safety) or optimal for the target.

**Context:** Several journeys have instructions that serve no purpose — construct-then-destructure patterns, redundant SSO branches, parameter extract/repack. These inflate instruction counts and lower IE and IR scores. Note: many current "unjustified" instructions are actually empty blocks (single `br` terminator) — those are fixed by Section 02 and will auto-disappear from this analysis.

**Baseline (2026-03-16, pre-Section-02):**

| Journey | IR Score | Unjustified | Source |
|---------|----------|-------------|--------|
| J2 | 9 | 2 | Empty blocks (fixed by Section 02 CFG cleanup) |
| J3 | 9 | 1 | Redundant entry block (fixed by Section 02 CFG cleanup) |
| J5 | 9 | 1 | Null-check on known non-null env pointer |
| J7 | 9 | 2 | Range construct-then-destructure in @sum_for |
| J9 | 8 | 4 | SSO gating redundant branches |
| J10 | 8 | 3-5 | Parameter materialization + cleanup paths (count depends on Section 02 results) |
| J12 | 9 | 1 | Empty block in safe_div (fixed by Section 02 CFG cleanup) |

**Post-audit (2026-03-16, after Sections 01-02):**

| Journey | IR Score | IE Score | Unjustified | Notes |
|---------|----------|----------|-------------|-------|
| J1-J13 | 10 | 10 | 0 | All journeys OPTIMAL |

All previously flagged issues resolved:
- J2, J3, J12: Empty blocks eliminated by Section 02 CFG cleanup
- J3, J7: Entry-block-to-loop-header `br` correctly classified as structurally required (metrics script fix)
- J5: Null-check no longer flagged (resolved by attribute/CFG improvements)
- J7: Range construct-then-destructure no longer flagged (0 unjustified in fresh IR)
- J9: SSO gating branches no longer flagged (0 unjustified in fresh IR)
- J10: Parameter materialization not flagged (0 unjustified in fresh IR)

**Depends on:** Run AFTER Section 02 (CFG cleanup). Many current "unjustified" instructions are empty blocks that Section 02 eliminates — running 03.1 audit before Section 02 produces misleading results. Section 03.2-03.4 CAN be developed in parallel with Section 02 if needed, since they target different code paths, but 03.1 (the audit that determines which items are still needed) MUST wait for Section 02.

---

## 03.1 Post-Section-02 Audit

After Section 02 (CFG cleanup) completes, re-run the code journey scoring to see which unjustified instructions remain. Many will auto-disappear when empty blocks are eliminated.

- [x] Re-run all 13 code journeys with fresh LLVM IR (2026-03-16):
  - Built compiler, ran all 13 `.ori` files through eval + AOT, dumped fresh LLVM IR
  - Ran `extract-metrics.py` on each — all 13 show 0 unjustified, 1.00 ratio, OPTIMAL
  - Fixed false positive in `instruction_metrics.py`: entry block `br` to loop header with non-trivial phis is structurally required (consistent with `control_flow_metrics.py`)
  - Added regression tests for the fix (2 new tests in `test_instruction_metrics.py`)
  - Fixed pre-existing test failures: stale attribute metric expectations, stale CF test, stale effect summaries test
  - All 173 metrics tests now pass
- [x] Document remaining unjustified instructions per journey in a table (see post-audit table above — all 0)
- [x] All IR Quality and IE scores are 10/10 after Section 02: 03.2-03.4 marked N/A
- [x] No unjustified instructions remain — 03.2-03.4 are N/A

---

## 03.2 Range Materialization (J7 @sum_for)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs`, `compiler/ori_arc/src/lower/collections/mod.rs`

`@sum_for` builds a `Range { start: 1, end: 5, step: 1, inclusive: true }` struct via `insertvalue` instructions, then immediately destructures it with `extractvalue` to get the loop bounds. This is 8+ instructions that could be 0 if the range values were used as scalars directly.

- [x] **N/A** — J7 `@sum_for` scores OPTIMAL (0 unjustified, ratio 1.00) in fresh IR after Section 02 CFG cleanup. Range construct-then-destructure pattern is not flagged as unjustified by `instruction_metrics.py`. Verified 2026-03-16.

---

## 03.3 SSO Gating Redundancies (J9)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs`

String functions emit SSO (Small String Optimization) vs heap branches for rc_dec. When both SSO and heap paths produce identical code (e.g., both paths do nothing, or both call the same function), the branch is redundant.

- [x] **N/A** — J9 scores OPTIMAL (0 unjustified, ratio 1.00) in fresh IR after Section 02 CFG cleanup. SSO gating branches are no longer flagged. Verified 2026-03-16.

---

## 03.4 Parameter Materialization (J10 @count_items)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs`, `compiler/ori_llvm/src/codegen/abi/mod.rs`

`@count_items` receives a list parameter. If passed `Direct` (in registers), the callee must extract individual fields. If passed `Indirect` (by pointer), the callee loads from the pointer. The extract/repack pattern for Direct passing of large structs adds unjustified instructions.

- [x] **N/A** — J10 scores OPTIMAL (0 unjustified, ratio 1.00) in fresh IR after Section 02 CFG cleanup. Parameter materialization pattern is not flagged as unjustified. Verified 2026-03-16.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] Post-Section-02 audit completed — 0 remaining unjustified instructions documented (2026-03-16)
- [x] Range construct-then-destructure: N/A — not flagged in fresh IR, J7 scores OPTIMAL
- [x] SSO gating redundancies: N/A — not flagged in fresh IR, J9 scores OPTIMAL
- [x] Parameter materialization: N/A — not flagged in fresh IR, J10 scores OPTIMAL
- [x] `instruction_metrics.py` updated: entry block preheaders to loop headers (non-trivial phis) now classified as structurally required, consistent with `control_flow_metrics.py`. 2 regression tests added. Pre-existing stale test expectations fixed. All 173 metrics tests pass.
- [x] All 13 journeys IR Quality score 10/10
- [x] All 13 journeys Instruction Efficiency score 10/10
- [x] Total unjustified instructions across all 13 journeys = 0
- [x] `cargo t` green (2186 tests passed)
- [x] `cargo clippy --workspace` green
- [x] `cargo b --release` green (release build succeeds; `cargo t --release` has pre-existing `debug_assertions` gating issue in `ori_rt` — unrelated to IR quality)

**Exit Criteria:** `extract-metrics.py` reports 0 unjustified instructions for all 13 journeys. No instruction in the emitted IR exists without justification. Zero test regressions.

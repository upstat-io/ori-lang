---
section: "03"
title: "IR Quality"
status: not-started
goal: "Zero unjustified instructions in emitted LLVM IR — IR Quality score 10/10 and Instruction Efficiency 10/10 on all journeys"
depends_on: ["02"]  # 03.1 audit requires 02 complete; 03.2-03.4 can parallel 02
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Post-Section-02 Audit"
    status: not-started
  - id: "03.2"
    title: "Range Materialization (J7)"
    status: not-started
  - id: "03.3"
    title: "SSO Gating Redundancies (J9)"
    status: not-started
  - id: "03.4"
    title: "Parameter Materialization (J10)"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: IR Quality

**Status:** Not Started
**Goal:** Zero unjustified instructions across all 13 journeys. Every instruction in the emitted IR must be necessary for correctness (overflow checking, ARC, safety) or optimal for the target.

**Context:** Several journeys have instructions that serve no purpose — construct-then-destructure patterns, redundant SSO branches, parameter extract/repack. These inflate instruction counts and lower IE and IR scores. Note: many current "unjustified" instructions are actually empty blocks (single `br` terminator) — those are fixed by Section 02 and will auto-disappear from this analysis.

**Current unjustified instructions (2026-03-16 baseline):**

| Journey | IR Score | Unjustified | Source |
|---------|----------|-------------|--------|
| J2 | 9 | 2 | Empty blocks (fixed by Section 02 CFG cleanup) |
| J3 | 9 | 1 | Redundant entry block (fixed by Section 02 CFG cleanup) |
| J5 | 9 | 1 | Null-check on known non-null env pointer |
| J7 | 9 | 2 | Range construct-then-destructure in @sum_for |
| J9 | 8 | 4 | SSO gating redundant branches |
| J10 | 8 | 3-5 | Parameter materialization + cleanup paths (count depends on Section 02 results) |
| J12 | 9 | 1 | Empty block in safe_div (fixed by Section 02 CFG cleanup) |

**Depends on:** Run AFTER Section 02 (CFG cleanup). Many current "unjustified" instructions are empty blocks that Section 02 eliminates — running 03.1 audit before Section 02 produces misleading results. Section 03.2-03.4 CAN be developed in parallel with Section 02 if needed, since they target different code paths, but 03.1 (the audit that determines which items are still needed) MUST wait for Section 02.

---

## 03.1 Post-Section-02 Audit

After Section 02 (CFG cleanup) completes, re-run the code journey scoring to see which unjustified instructions remain. Many will auto-disappear when empty blocks are eliminated.

- [ ] Re-run all 13 code journeys using the `/code-journey` skill or `rescore-all.sh`:
  ```bash
  # Option A: Use the skill (preferred — handles all arguments correctly)
  # /code-journey re-run J1-J13

  # Option B: Use the rescore script
  .claude/skills/code-journey/rescore-all.sh
  ```
- [ ] Document remaining unjustified instructions per journey in a table
- [ ] If all IR Quality and IE scores are 10/10 after Section 02: mark 03.2-03.4 as N/A with verification evidence
- [ ] If unjustified instructions remain: proceed with 03.2-03.4 as needed

---

## 03.2 Range Materialization (J7 @sum_for)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs`, `compiler/ori_arc/src/lower/collections/mod.rs`

`@sum_for` builds a `Range { start: 1, end: 5, step: 1, inclusive: true }` struct via `insertvalue` instructions, then immediately destructures it with `extractvalue` to get the loop bounds. This is 8+ instructions that could be 0 if the range values were used as scalars directly.

- [ ] Investigate: Where is range construction emitted?
  - Range lowering is in `compiler/ori_arc/src/lower/collections/mod.rs` (`lower_range()`). It emits `ArcInstr::Construct { ctor: CtorKind::Tuple, ... }` with 4 scalar args (start, end, step, inclusive).
  - Check if ARC IR has a `Construct(Tuple, ...)` instruction followed by `Project(range, .start)` etc.
  - If so, the fix is in the ARC → LLVM emission: when a `Construct` is immediately followed by `Project` on the same value with no other uses, the construct can be elided
  - Alternatively: the for-loop lowering should emit scalar loop bounds directly, not a range struct
- [ ] Determine: Are these instructions counted as "unjustified" by the scoring tool, or is it a manual assessment?
  - If the scoring tool (`instruction_metrics.py`) flags `insertvalue`/`extractvalue` pairs as unjustified: the tool needs updating — these are correct codegen, just suboptimal at O0
  - If this is a manual IR Quality assessment: document whether construct-then-destructure constitutes an "unjustified instruction" in the scoring rubric
- [ ] Fix approach decision: Two options, each with tradeoffs:
  **(a) ARC IR peephole**: In `ori_arc`, after lowering, detect `Construct` followed by only `Project` uses and inline the scalar values. This is a general optimization that benefits all construct-then-destructure patterns, not just ranges. BUT: requires a new optimization pass in `ori_arc` (not trivial).
  **(b) For-loop lowering change**: Change `lower_for_loop` to emit scalar bounds directly instead of constructing a range struct. Simpler but specific to for-loop ranges. Check `compiler/ori_arc/src/lower/expr/mod.rs` or `control_flow/mod.rs` for `lower_for` or similar.
  **(c) Accept as-is**: If the scoring tool doesn't flag this, and the IR Quality score reaches 10/10 after Section 02 CFG cleanup, this is N/A.
- [ ] Test: J7 `@sum_for` unjustified instruction count = 0 (or documented as acceptable)
- [ ] Verify: `timeout 150 ./test-all.sh` green

---

## 03.3 SSO Gating Redundancies (J9)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/rc_ops.rs`

String functions emit SSO (Small String Optimization) vs heap branches for rc_dec. When both SSO and heap paths produce identical code (e.g., both paths do nothing, or both call the same function), the branch is redundant.

- [ ] Investigate: Are the redundant branches from rc_dec diamond patterns?
  - The SSO rc_dec pattern: `if is_sso then skip_rc_dec else rc_dec_heap`
  - If the string is statically known to be SSO (string literals ≤23 bytes), the branch is unnecessary
  - If both arms of the branch produce the same result, the branch should be merged
- [ ] Investigate: Read `string_builtins.rs` and `rc_ops.rs` — trace the rc_dec emission for string values
  - Check `rc_ops.rs` for the SSO branch pattern in `emit_rc_dec_str()` or similar
  - The SSO diamond: `icmp` on cap high bit → `br i1 %is_sso, label %skip, label %heap_dec` → both paths merge
  - If `heap_dec` path has actual cleanup (calling `ori_buffer_rc_dec`), the branch is NOT redundant — it correctly skips RC for SSO strings
  - The "redundant" branches may actually be the empty blocks that Section 02 eliminates — check after Section 02 if they're gone
- [ ] Fix decision: Check after Section 02 CFG cleanup whether the SSO branches are still flagged as unjustified
  - If Section 02's "both arms same target" elimination removes them: this item is auto-fixed
  - If they remain with different targets (skip vs heap_dec): these are correct branches, not unjustified. Update the scoring rubric.
  - If string literals are statically known SSO: add a fast-path in `rc_ops.rs` that skips the SSO check for values known to be SSO at compile time (requires tracking SSO-ness through the ARC IR — significant effort)
- [ ] Test: J9 unjustified instruction count ≤ 1 (or documented as correct SSO guards)
- [ ] Verify: `timeout 150 ./test-all.sh` green

---

## 03.4 Parameter Materialization (J10 @count_items)

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs`, `compiler/ori_llvm/src/codegen/abi/mod.rs`

`@count_items` receives a list parameter. If passed `Direct` (in registers), the callee must extract individual fields. If passed `Indirect` (by pointer), the callee loads from the pointer. The extract/repack pattern for Direct passing of large structs adds unjustified instructions.

- [ ] Investigate: Check the ABI for `@count_items` — is the list param `Direct` or `Indirect`?
  ```bash
  ORI_DUMP_AFTER_LLVM=1 ./target/debug/ori run --compile plans/code-journeys/10-lists.ori 2>&1 | grep count_items
  ```
  - List struct is `{ i64, i64, ptr }` (len, cap, data) = 24 bytes = 3 fields
  - On x86-64, >16 bytes should use `Indirect` passing
  - `compute_param_passing()` in `abi/mod.rs` uses `size <= 16` threshold. 24 > 16, so list IS Indirect. This is correct.
- [ ] If ABI is `Indirect` (expected): the load instructions are from `emit_function.rs` param loading (line ~238-264) which does per-field GEP+load+insert_value. These instructions ARE justified:
  - The per-field loading pattern is required for FastISel safety (see CLAUDE.md "FastISel Aggregate Bug")
  - The scoring tool should count these as "safety-justified" instructions, not "unjustified"
- [ ] Check: Does `.claude/skills/code-journey/instruction_metrics.py` already classify Indirect param loads as justified?
  - If yes: this item is N/A
  - If no: update `.claude/skills/code-journey/instruction_metrics.py` to recognize `GEP + load + insert_value` sequences on function params as justified (safety: FastISel aggregate bug workaround)
- [ ] Test: J10 instruction efficiency score = 10/10
- [ ] Verify: `timeout 150 ./test-all.sh` green

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Post-Section-02 audit completed — remaining unjustified instructions documented
- [ ] Range construct-then-destructure eliminated, or determined to be auto-fixed by Section 02, or documented as correct codegen
- [ ] SSO gating redundancies eliminated, or determined to be auto-fixed by Section 02, or documented as correct safety branches
- [ ] Parameter materialization justified as FastISel safety pattern, or ABI threshold corrected
- [ ] `.claude/skills/code-journey/instruction_metrics.py` updated if needed: FastISel param loads classified as justified; SSO guards classified as safety-justified
- [ ] All 13 journeys IR Quality score 10/10
- [ ] All 13 journeys Instruction Efficiency score 10/10
- [ ] Total unjustified instructions across all 13 journeys = 0
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `cargo b --release && timeout 150 ./test-all.sh` green

**Exit Criteria:** `extract-metrics.py` reports 0 unjustified instructions for all 13 journeys. No instruction in the emitted IR exists without justification. Zero test regressions.

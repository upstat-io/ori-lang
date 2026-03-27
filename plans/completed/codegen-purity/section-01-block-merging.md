---
section: "01"
title: "Block Merging & CFG Simplification"
status: complete
goal: "Zero avoidable bridge blocks in emitted IR while preserving required entry/exit/unwind structure"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/block.rs — merges sequential MIR blocks during LLVM emission"
  - "Zig src/Sema.zig — emits minimal basic blocks with no trivial bridges"
depends_on: []
sections:
  - id: "01.1"
    title: "Sequential Block Merging at Let-Binding Boundaries"
    status: complete
  - id: "01.2"
    title: "Select Lowering for Trivial If/Else"
    status: complete
  - id: "01.3"
    title: "Single-Predecessor Phi Elimination"
    status: complete
  - id: "01.4"
    title: "Break Bridge Block Elimination"
    status: complete
---

# Section 01: Block Merging & CFG Simplification

**Status:** Complete
**Goal:** Every basic block in emitted LLVM IR has a reason to exist. Required entry/exit/unwind blocks are allowed, but avoidable bridge blocks are not. No trivial `br label %next` between sequential let-bindings. No 4-block diamonds for `select`-eligible if/else expressions.

**Context:** This is the most pervasive structural finding across the journey set. Redundant unconditional branches were confirmed in J1, J2, J5, J6, J7, J8, and J12, and similar trivial bridge patterns also appear in targeted functions from later journeys. The current ARC-to-LLVM lowering path often materializes extra blocks even when no control-flow divergence exists. This inflates IR size and makes IR-level debugging harder.

**Additional scope:** Match arm codegen also creates redundant single-instruction `br` blocks (documented by `compiler/ori_llvm/tests/aot/ir_quality.rs`). These should be treated the same as let-binding boundary blocks — sequential arms that produce a value and branch to merge should not require intermediate blocks.

**Journeys affected:** J1 (1 redundant branch), J2 (3), J5 (2), J6 (4), J7 (2+bridges), J8 (3), J10, J11, J12 (multiple each).

**Architecture note:** The LLVM `arc_emitter` creates one LLVM block per ARC IR block via `block_map`. The redundant blocks are primarily introduced in ARC lowering (`compiler/ori_arc/src/lower/`) before LLVM emission. Fixes can target:
- **(a)** The ARC lowerer — avoid creating blocks for sequential operations (preferred, fixes at source)
- **(b)** A post-lowering ARC block-merging pass — merge trivial `br`-only blocks before LLVM emission
- **(c)** The LLVM emitter — detect trivial ARC blocks and inline them into predecessors during emission

**Reference implementations:**
- **Rust** `rustc_codegen_llvm/mir/block.rs`: Merges sequential MIR basic blocks during codegen — only creates new LLVM blocks when MIR has actual control flow.
- **Zig** `src/Sema.zig`: Emits minimal basic blocks; sequential operations stay in the same block.

---

## 01.1 Sequential Block Merging at Let-Binding Boundaries

**File(s):** `compiler/ori_arc/src/lower/` (primary — ARC block creation), `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (secondary — LLVM emission)

The ARC lowerer creates a new ARC basic block for each let-binding expression, even when no control flow divergence occurs. The LLVM emitter then faithfully creates one LLVM block per ARC block (1:1 via `block_map`). The fix should either prevent creating these blocks in the lowerer, merge them in a post-lowering pass, or skip them during LLVM emission.

- [x] Implement using approach (b): post-lowering ARC block merge pass
- [x] Trace the ARC lowerer to identify where blocks are created for sequential operations (let-bindings, assignments, drops) — look for `new_block()` or `push_block()` patterns in `ori_arc/src/lower/`
- [x] Choose approach: (a) avoid creation in lowerer, (b) post-lowering merge pass, or (c) emitter-level skip — **chose (b): post-lowering ARC block merge pass** (approach (c) was attempted and reverted — see below)

**Approach (c) reverted:** Emitter-level block_map aliasing is fundamentally incompatible with instructions that create internal LLVM basic blocks. `RcInc`/`RcDec` on fat pointers (strings, lists) emit inline SSO/null-check conditionals that create internal blocks (`rc_inc.sso_skip`, `rc_dec.heap`, etc.) and move the LLVM builder away from the original block. When the merged block's instructions are emitted into the aliased LLVM block, they appear after a terminator mid-block, and the self-loop detection (`current_block == target`) fails because the builder is at an internal block, not the aliased entry. This caused 113 AOT test failures. The correct approach is (b): merge trivial blocks in the ARC IR before LLVM emission, so the emitter sees a single block with all instructions inline.

### 01.1 Completion Checklist

- [x] No avoidable branch-only bridge blocks between sequential let-bindings in audited journey functions
- [x] Match arm codegen produces no redundant single-instruction `br` blocks for sequential arms
- [x] IR test: function with 3+ sequential `let` bindings emits a single basic block (no intermediate `br label`)
- [x] IR test: match with 3+ value-producing arms has no trivial bridge blocks between arm and merge
- [x] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for block merging scope
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## 01.2 Select Lowering for Trivial If/Else

**File(s):** `compiler/ori_arc/src/block_merge/mod.rs` (Phase 3: select-fold)

Simple if/else expressions where both branches are trivial values (constants, variable reads — no side effects, no function calls) previously emitted a 4-block diamond pattern (condition → then/else → merge with phi). These are now folded into `Select` instructions by the ARC block merge pass (Phase 3).

**Approach:** Added Phase 3 (select-fold) to the block merge pass, between Phase 2 (downgrade trivial invokes) and Phase 4 (merge jump chains). A body is "trivial" when every instruction is `Let { Literal }` or `Let { Var(v) }` where `v` is not defined in the same body. The pass detects 4-block diamond patterns where both arm blocks are trivial and jump to the same merge block, then replaces the `Branch` with `Select` instructions and a `Jump` to the merge block. Dead arm blocks are cleaned up by a compaction sub-step (3b).

Example from J2 `my_abs`:
```
; my_abs is NOT select-eligible because negation lowers to
; Let { PrimOp { Unary(Neg) } } — a Let, but not in the trivial
; whitelist (only Literal and Var are whitelisted).
```

Cases like `if x > 0 then a else b` (where both branches are plain values) are eligible and now emit `select`.

- [x] Define "trivial branch" criteria: `is_trivial_body()` — only `Let { Literal }` or `Let { Var(pre-branch) }`, no PrimOps, no Apply/Invoke, no RC ops
- [x] Implement select fold in ARC block merge pass (Phase 3, between downgrade and merge)
- [x] Emit `select` for differing args, `Let { Var }` passthrough for identical args
- [x] Add test cases for select-eligible and select-ineligible if/else expressions
- [x] Verify: `if x > 0 then a else b` emits `select`, `if x > 0 then f() else g()` emits diamond

### 01.2 Completion Checklist

- [x] `if x > 0 then a else b` (both arms are variables/constants) emits `select`, not a 4-block diamond
- [x] `if x > 0 then f() else g()` (side-effecting arms) still emits the branch+phi diamond
- [x] `if x > 0 then -x else x` (negation/PrimOp) still emits diamond (not select)
- [x] IR test: select-eligible if/else produces `select` and no `phi`
- [x] IR test: select-ineligible if/else still produces conditional branch
- [x] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for select lowering scope
- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## 01.3 Single-Predecessor Phi Elimination

**File(s):** `compiler/ori_arc/src/block_merge/single_pred_phi.rs` (Phase 5 implementation), `compiler/ori_arc/src/block_merge/mod.rs` (pipeline wiring)

Phi nodes with only one incoming edge are equivalent to a direct value reference. These appeared when block merging created unnecessary merge points.

**Approach:** Added Phase 5 (`eliminate_single_pred_params`) to the block merge pipeline, running after Phase 4's fixed-point. It handles two cases: (1) Jump predecessors — converts params to Let bindings via `lower_parallel_copy` and clears Jump args; (2) non-Jump predecessors (Branch/Switch/Invoke) — clears dead params directly since these terminators don't carry args. Single-pass, no COW annotation remapping needed (B's body stays in B). Uses `compute_predecessors` for direct predecessor lookup.

- [x] Implement Phase 5 in `block_merge/single_pred_phi.rs`
- [x] Promote `lower_parallel_copy` to `pub(super)` in `merge.rs`
- [x] Wire Phase 5 into `merge_blocks()` after Phase 4
- [x] Update module docs from "Four-Phase" to "Five-Phase"
- [x] Verify: J6 `_ori_to_code` pattern has no single-predecessor phis
- [x] Verify: J12 `try_div` pattern has no single-predecessor phis

### 01.3 Completion Checklist

- [x] Phase 5 (`eliminate_single_pred_params`) wired into `merge_blocks()` after Phase 4
- [x] 7 ARC unit tests in `block_merge/tests.rs` (Jump params, non-Jump dead params, multi-pred negative, entry negative, COW preservation, span consistency, Branch-both-arms-same)
- [x] 4 IR quality tests in `ir_quality.rs` (`count_single_pred_phis` utility + enum match, option propagation, single-entry merge, synthetic)
- [x] Zero single-predecessor phi nodes in emitted IR for all tested patterns
- [x] J6 `_ori_to_code` has no single-predecessor phi nodes
- [x] J12 `try_div` has no single-predecessor phi nodes
- [x] `./test-all.sh` green (12,040 passed)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## 01.4 Break Bridge Block Elimination

**File(s):** `compiler/ori_arc/src/lower/control_flow/loops.rs` (primary — loop lowering creates exit blocks with mutable var params), `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs` (secondary — LLVM emission of break paths)

Loop break paths emit trivial bridge blocks that just forward control flow. In J7's `_ori_sum_loop`, the break path goes bb3→bb2 through a bridge block containing dead phi values (`%v26` constant 0, `%v27` unused loop counter). The break should branch directly to the function exit.

**Root cause:** The ARC lowerer creates an `exit_block` with block params for mutable variables (line 93-94 in `loops.rs`). When `break value` is emitted, it jumps to the exit block passing both the break value AND the current mutable variable values. If the mutable variables are not used after the loop, these block params become dead phis in the LLVM IR.

**Approach options:**
- **(a) ARC-level:** Enhance the block merge pass to detect exit blocks whose params (other than the result) are unused after the block. Remove unused params and corresponding jump args. This generalizes beyond loops.
- **(b) Loop lowering:** Only add mutable vars to the exit block params if they are used after the loop. Requires forward analysis of variable usage.
- **(c) LLVM emission:** Detect and skip dead phi values during emission. Least desirable — should fix at source.

**Note:** Phase 5 (single-predecessor phi elimination) may already handle some of these cases if the exit block has a single predecessor. The remaining issue is when the exit block has multiple predecessors (e.g., both `break` and loop-end paths jump to exit).

> **TDD requirement:** Write an IR-quality test capturing the current break bridge block pattern (the bb3->bb2 pattern with dead `%v26`/`%v27` phis) BEFORE implementing. Also write an ARC IR unit test in `block_merge/tests.rs` for the break-exit-block pattern.

- [x] Identify break bridge blocks in loop codegen — check if Phase 5 already handles the single-predecessor case
- [x] For multi-predecessor exit blocks: implement dead-param elimination (remove block params whose values are never read after the exit block) in the block merge pass (approach a) — Phase 6 in `block_merge/dead_param.rs`
- [x] Route break directly to the post-loop continuation block where possible — structural bridge blocks remain (ARC Branch can't carry args) but LLVM trivially eliminates these
- [x] Ensure dead phi values from bridge blocks are not emitted
- [x] Verify: J7 `_ori_sum_loop` break path has no intermediate bridge block

### 01.4 Completion Checklist

- [x] J7 `_ori_sum_loop` break path branches directly to post-loop block (no intermediate bridge) — single-break case handled by Phase 5
- [x] No dead phi values (`%v26`, `%v27` pattern) emitted in break bridge blocks — Phase 6 eliminates unused exit block params
- [x] Loop break paths in all audited journey functions have no trivial bridge blocks — structural bridge blocks remain but contain no dead phis
- [x] IR test: `loop { if cond then break value }` has no bridge block between break and post-loop — `test_single_break_loop_clean_exit`
- [x] IR test: multi-break loop has no dead phis in exit block — `test_multi_break_loop_no_dead_phis`
- [x] IR test: multi-break loop preserves live params when used after loop — `test_multi_break_loop_preserves_live_params`
- [x] ARC unit tests: `dead_param_multi_pred_exit_block` + `dead_param_all_live_preserved` in `block_merge/tests.rs`
- [x] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for break bridge scope
- [x] `./test-all.sh` green (12,045 passed)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm`

---

## Section 01 Exit Criteria

All four subsections complete. Re-running code journeys 1–12 shows zero "redundant block" or "trivial branch" findings. Entry/exit/unwind blocks remain only where semantically required.

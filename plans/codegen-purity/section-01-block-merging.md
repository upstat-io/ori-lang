---
section: "01"
title: "Block Merging & CFG Simplification"
status: in-progress
goal: "Zero avoidable bridge blocks in emitted IR while preserving required entry/exit/unwind structure"
inspired_by:
  - "Rust rustc_codegen_llvm/mir/block.rs — merges sequential MIR blocks during LLVM emission"
  - "Zig src/Sema.zig — emits minimal basic blocks with no trivial bridges"
depends_on: []
sections:
  - id: "01.1"
    title: "Sequential Block Merging at Let-Binding Boundaries"
    status: not-started
  - id: "01.2"
    title: "Select Lowering for Trivial If/Else"
    status: not-started
  - id: "01.3"
    title: "Single-Predecessor Phi Elimination"
    status: not-started
  - id: "01.4"
    title: "Break Bridge Block Elimination"
    status: not-started
---

# Section 01: Block Merging & CFG Simplification

**Status:** Not Started
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

- [ ] Implement using approach (b): post-lowering ARC block merge pass
- [ ] Trace the ARC lowerer to identify where blocks are created for sequential operations (let-bindings, assignments, drops) — look for `new_block()` or `push_block()` patterns in `ori_arc/src/lower/`
- [ ] Choose approach: (a) avoid creation in lowerer, (b) post-lowering merge pass, or (c) emitter-level skip — **chose (c): emitter-level block_map aliasing — REVERTED (see below)**

**Approach (c) reverted:** Emitter-level block_map aliasing is fundamentally incompatible with instructions that create internal LLVM basic blocks. `RcInc`/`RcDec` on fat pointers (strings, lists) emit inline SSO/null-check conditionals that create internal blocks (`rc_inc.sso_skip`, `rc_dec.heap`, etc.) and move the LLVM builder away from the original block. When the merged block's instructions are emitted into the aliased LLVM block, they appear after a terminator mid-block, and the self-loop detection (`current_block == target`) fails because the builder is at an internal block, not the aliased entry. This caused 113 AOT test failures. The correct approach is (b): merge trivial blocks in the ARC IR before LLVM emission, so the emitter sees a single block with all instructions inline.

### 01.1 Completion Checklist

- [ ] No avoidable branch-only bridge blocks between sequential let-bindings in audited journey functions
- [ ] Match arm codegen produces no redundant single-instruction `br` blocks for sequential arms
- [ ] IR test: function with 3+ sequential `let` bindings emits a single basic block (no intermediate `br label`)
- [ ] IR test: match with 3+ value-producing arms has no trivial bridge blocks between arm and merge
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for block merging scope
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 01.2 Select Lowering for Trivial If/Else

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

Simple if/else expressions where both branches are trivial values (constants, variable reads — no side effects, no function calls) currently emit a 4-block diamond pattern (condition → then/else → merge with phi). These should emit a single `select` instruction.

Example from J2 `my_abs`:
```
; CURRENT: 4 blocks, phi merge
bb1:
  %cond = icmp slt i64 %x, 0
  br i1 %cond, label %then, label %else
then:
  %neg = sub i64 0, %x    ; side effect (overflow) — NOT select-eligible
  br label %merge
else:
  br label %merge
merge:
  %result = phi i64 [%neg, %then], [%x, %else]

; IDEAL for truly trivial cases (both branches are just values):
  %cond = icmp slt i64 %x, 0
  %result = select i1 %cond, i64 %negated, i64 %x
```

Note: `my_abs` specifically is NOT select-eligible because the negation has a side effect (overflow check). But cases like `if x > 0 then x else y` (where both branches are plain values) are eligible.

- [ ] Define "trivial branch" criteria: both arms produce a single SSA value with no side effects (no calls, no stores, no overflow-checked arithmetic)
- [ ] In the if/else codegen path, check if both arms are trivial
- [ ] If trivial, emit `select` instead of branch+phi diamond
- [ ] Add test cases for select-eligible and select-ineligible if/else expressions
- [ ] Verify: `if x > 0 then a else b` emits `select`, `if x > 0 then f() else g()` emits diamond

### 01.2 Completion Checklist

- [ ] `if x > 0 then a else b` (both arms are variables/constants) emits `select`, not a 4-block diamond
- [ ] `if x > 0 then f() else g()` (side-effecting arms) still emits the branch+phi diamond
- [ ] `if x > 0 then -x else x` (overflow-checked arithmetic) still emits diamond (not select)
- [ ] IR test: select-eligible if/else produces exactly 0 `phi` and 1 `select` instruction
- [ ] IR test: select-ineligible if/else still produces `phi` with correct incoming edges
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for select lowering scope
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 01.3 Single-Predecessor Phi Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

Phi nodes with only one incoming edge are equivalent to a direct value reference. These appear when block merging creates unnecessary merge points.

- [ ] After block emission, scan for phi nodes with exactly one predecessor
- [ ] Replace them with their single incoming value
- [ ] Alternatively: prevent creation by not emitting merge blocks when there's only one predecessor path
- [ ] Verify: J6 `_ori_to_code` and J12 `try_div` bb5 have no single-predecessor phis
- [ ] Verify: J12 single-predecessor phi nodes are eliminated (Finding #3 from J12)

### 01.3 Completion Checklist

- [ ] Zero single-predecessor phi nodes in emitted IR for all audited journey functions
- [ ] J6 `_ori_to_code` has no single-predecessor phi nodes
- [ ] J12 `try_div` bb5 has no single-predecessor phi nodes
- [ ] IR test: function with a single-entry merge point uses direct value reference, not phi
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for phi elimination scope
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 01.4 Break Bridge Block Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`

Loop break paths emit trivial bridge blocks that just forward control flow. In J7's `_ori_sum_loop`, the break path goes bb3→bb2 through a bridge block containing dead phi values (`%v26` constant 0, `%v27` unused loop counter). The break should branch directly to the function exit.

- [ ] Identify break bridge blocks in loop codegen
- [ ] Route break directly to the post-loop continuation block
- [ ] Ensure dead phi values from bridge blocks are not emitted
- [ ] Verify: J7 `_ori_sum_loop` break path has no intermediate bridge block

### 01.4 Completion Checklist

- [ ] J7 `_ori_sum_loop` break path branches directly to post-loop block (no intermediate bridge)
- [ ] No dead phi values (`%v26`, `%v27` pattern) emitted in break bridge blocks
- [ ] Loop break paths in all audited journey functions have no trivial bridge blocks
- [ ] IR test: `loop { if cond then break value }` has no bridge block between break and post-loop
- [ ] `compiler/ori_llvm/tests/aot/ir_quality.rs` tests updated for break bridge scope
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 01 Exit Criteria

All four subsections complete. Re-running code journeys 1–12 shows zero "redundant block" or "trivial branch" findings. Entry/exit/unwind blocks remain only where semantically required.

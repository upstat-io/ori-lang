---
section: "02"
title: "CFG Cleanup"
status: in-progress
goal: "Zero empty blocks, zero redundant branches in emitted LLVM IR — CF score 10/10 on all journeys"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Extract Dead-Unwind Detection"
    status: complete
  - id: "02.2"
    title: "Post-Emission CFG Simplification Pass"
    status: not-started
  - id: "02.3"
    title: "Redundant Entry Block Merging"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: CFG Cleanup

**Status:** Not Started
**Goal:** Zero empty blocks and zero redundant branches in emitted LLVM IR. CF score 10/10 on all 13 journeys.

**Context:** The LLVM emission creates empty "trampoline" blocks (`br label %next` only) as artifacts of if/else lowering, overflow check patterns, and match lowering. TCO and loop lowering also create redundant entry blocks. These are harmless at O1+ (LLVM's SimplifyCFG eliminates them) but degrade O0 IR quality and journey scores. The fix is a post-emission CFG simplification pass — run once after all IR for a function is emitted.

**Current CF defects by journey:**

| Journey | CF Score | Defects | Sources |
|---------|----------|---------|---------|
| J1 | 10 | 0 | Clean |
| J2 | 7 | 5 | 3 empty blocks + 2 redundant branches in my_abs, my_sign |
| J3 | 7 | 4 | 2 empty blocks in fib, 1 empty entry + 1 redundant br in gcd |
| J4 | 10 | 0 | Clean |
| J5 | 9 | 1 | 1 empty block in closure env null-check |
| J6 | 10 | 0 | Clean |
| J7 | 7 | 5 | Empty blocks + redundant entry in sum_loop/sum_for |
| J8 | 10 | 0 | Clean |
| J9 | 7 | 4 | SSO rc_dec diamond empty blocks |
| J10 | 7 | varies | Empty blocks in iteration and cleanup paths |
| J11 | 10 | 0 | Clean |
| J12 | 8 | 3 | Empty blocks in safe_div, unwrap_or |
| J13 | 10 | 0 | Clean |

**Affected journeys:** J2, J3, J5, J7, J9, J10, J12 (7 of 13)

**Reference implementations:**
- **LLVM** `lib/Transforms/Utils/SimplifyCFG.cpp`: `MergeBlockIntoPredecessor` — folds empty blocks
- **Rust** `compiler/rustc_codegen_llvm/src/builder.rs`: avoids creating empty blocks during emission

**Depends on:** None.

---

## 02.1 Extract Dead-Unwind Detection

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` (currently 570 lines — exceeds 500-line limit)

Before adding the CFG simplification pass, extract existing logic to make room.

- [x] Extract the dead-unwind detection logic into `compiler/ori_llvm/src/codegen/arc_emitter/dead_unwind.rs` (2026-03-16)
  - `detect_dead_unwind_blocks()` method on `ArcIrEmitter` returns `DeadUnwindResult { dead, live }`
  - Also extracted `has_effective_cleanup()`, `is_non_capturing_closure()`, `find_definition()` (163 lines)
- [x] Extracted `debug_assert_dead_unwind_unreachable()` function (2026-03-16)
- [x] Added `mod dead_unwind;` to `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs` (2026-03-16)
- [x] Updated imports in `emit_function.rs` and `terminators.rs` to use the extracted module (2026-03-16)
- [x] Verify: `emit_function.rs` dropped from 570 → 438 lines (2026-03-16)
- [x] Verify: `timeout 150 ./test-all.sh` green — 12,897 tests, 0 failures (2026-03-16)

### Cleanup (02.1)

- [x] **[BLOAT]** `emit_function.rs:570` — Reduced from 570 → 438 lines by extracting dead-unwind logic to `dead_unwind.rs` (163 lines). `#[expect(clippy::too_many_lines)]` retained — `emit_function()` body is still ~346 lines (orchestrating blocks, params, EH, RPO emission, phis). (2026-03-16)

---

## 02.2 Post-Emission CFG Simplification Pass

**File(s):** New file `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify.rs`, called from `define_phase.rs` or `nounwind.rs`

Implement a single-pass CFG simplification that runs after all LLVM IR for a function is emitted, before LLVM verification.

```rust
/// Simplify the CFG of a function by eliminating empty blocks and redundant branches.
///
/// Run AFTER all IR is emitted, BEFORE function verification.
/// Handles two patterns:
/// 1. Empty blocks (only `br label %target`, no phi nodes) — redirect predecessors, delete block
/// 2. Redundant conditional branches (both arms same target) — replace with unconditional
///
/// Does NOT handle phi-bearing blocks or entry block merging (see 02.3).
/// Iterates to fixed point to handle chained empty blocks.
pub fn simplify_cfg(function: FunctionValue<'_>) -> SimplifyStats { ... }
```

- [ ] Create `compiler/ori_llvm/src/codegen/ir_builder/cfg_simplify.rs`
- [ ] **Design decision**: Place in `ir_builder/cfg_simplify.rs`. The CFG simplification pass works on raw LLVM IR (inkwell `BasicBlock`s), not ARC IR or IrBuilder abstractions. It takes an `inkwell::values::FunctionValue` directly. IrBuilder is the LLVM abstraction layer (109 lines currently in `control_flow.rs`, 218 in `checked_ops.rs`), so this is a natural fit. Placing it in `arc_emitter/` would be a phase-boundary violation (arc_emitter translates ARC IR, not raw LLVM IR).
- [ ] Implement `simplify_cfg()`:
  ```rust
  /// Takes a raw inkwell FunctionValue, not IrBuilder abstractions.
  pub fn simplify_cfg(function: FunctionValue<'_>) -> SimplifyStats { ... }
  ```
  1. **Collect empty blocks**: Walk `function.get_basic_blocks()`. A block is "empty" if its only instruction is an unconditional `br`. Collect into a `Vec<(empty_block, target_block)>`.
     - "Empty" = exactly 1 instruction (the terminator) AND that instruction is `br label %target` (unconditional)
     - **Never remove the entry block** — LLVM requires it. If the entry block is empty, it should be handled by 02.3 (entry merging) instead.
  2. **Redirect predecessors**: For each empty block, find all predecessors. inkwell provides `block.get_predecessors()` (returns `Vec<BasicBlock>`). Patch their terminators:
     - For unconditional `br`: use `LLVMSetSuccessor(term, 0, new_target)` via inkwell
     - For conditional `br`: `LLVMSetSuccessor(term, idx, new_target)` for matching arm(s) — check both arms (idx 0 and 1)
     - For `switch`: iterate cases and update matching targets
     - **inkwell limitation**: inkwell lacks `set_successor()`. Two approaches: (a) use `llvm_sys::core::LLVMSetSuccessor` directly (unsafe, but simple), or (b) delete the old terminator and build a new one at the predecessor. Approach (a) is preferred — it's a one-liner per successor.
  3. **Handle phi nodes**: If the target block has phi nodes with the empty block as an incoming source, rewrite the incoming edge to come from each predecessor instead.
     - **IMPORTANT**: A block with phi nodes is NOT empty for this pass's purposes, even if its only non-phi instruction is a `br`. Phi nodes compute values that successors may depend on. Only eliminate blocks with zero phi nodes and a single `br` terminator.
  4. **Delete empty blocks**: After all predecessors are redirected, remove the block. Use inkwell's `BasicBlock::remove_from_function()` (safe — moves block out of function) or `delete()` (unsafe — also frees memory). Prefer `remove_from_function()`.
     - No new IrBuilder API needed — this pass works directly on inkwell types.
     - **Chained empty blocks**: Process in reverse topological order, or iterate to fixed point. If block B branches to block C, and C branches to D, and both B and C are empty, processing C first collapses C→D, then processing B collapses B→D. Processing B first would redirect B→C→D but C still exists. Fixed-point (loop until no changes) is simpler and handles all cases.
  5. **Merge redundant conditionals**: Walk all blocks. If a `br i1 %cond, label %X, label %X` (both targets same), replace with `br label %X`. Delete the old terminator, position at end, build new `br`.
  6. **Return stats**: Count of blocks removed, branches simplified.
- [ ] Add `mod cfg_simplify;` to `compiler/ori_llvm/src/codegen/ir_builder/mod.rs`
- [ ] Call `simplify_cfg()` after `ArcIrEmitter::emit_function()` returns, before function verification. The call site is in `define_phase.rs` → `emit_arc_function()` (line ~175) or `nounwind.rs` → `emit_prepared_functions()` (line ~466). Both paths end with the function fully emitted.
  - Pass `self.builder.get_function_value(func_id)` to `simplify_cfg()` after the emitter returns
- [ ] Add tracing: `tracing::debug!("cfg_simplify: removed {} blocks, {} branches", stats.blocks, stats.branches)`
- [ ] Test: Write unit test `cfg_simplify_removes_empty_blocks` — create a function with an empty trampoline block, verify it's removed
  - Note: No phi-rewrite test needed — blocks with phi nodes are NOT candidates for removal (see point 3 above)
- [ ] Test: Write unit test `cfg_simplify_merges_redundant_conditionals` — create a `br i1 %c, label %X, label %X`, verify replaced with `br label %X`
- [ ] Test: Write AOT integration test: compile J2 `plans/code-journeys/02-branching.ori`, verify correct output AND reduced block count
- [ ] Verify: `timeout 150 ./test-all.sh` green
- [ ] Verify: J2 `@my_abs` block count drops from 5 to 3
- [ ] Verify: All 13 journeys still produce correct results
- [ ] Verify: `cargo b --release && timeout 150 ./test-all.sh` green (release behavior may differ due to FastISel)

---

## 02.3 Redundant Entry Block Merging

**File(s):** Handled by the CFG simplification pass from 02.2

TCO and loop lowering create entry blocks with only `br label %header`. These are a special case of empty blocks, but the entry block is special — it has no predecessors and cannot be "redirected from predecessors".

**Approach**: Entry block merging is NOT simply "remove the empty entry block". Instead:
1. If `entry` has exactly one instruction (`br label %header`) AND
2. `header` has exactly one predecessor (`entry`) — i.e., no back-edges or other jumps to `header`
3. Then: move all of `header`'s instructions into `entry`, update references, delete `header`

**When header has multiple predecessors** (loop header with back-edge from latch):
- The entry block CANNOT be merged. The phi nodes in `header` need the `entry` predecessor to distinguish initial values from loop-carried values.
- This is the common case for loops (J7 `@sum_loop`, `@sum_for`). These entry blocks will remain — they are structurally necessary.
- The scoring tool should NOT count these as "empty block defects" — they serve a structural purpose (loop preheader).

- [ ] Implement entry block merging as a separate case in `simplify_cfg()`:
  - Only merge when the entry block is `br label %header` AND `header` has exactly one predecessor (the entry block)
  - Use `LLVMGetNumPredecessors` or count via `header.get_predecessors().len()`
  - When conditions are met: move all instructions from `header` to `entry` (use `LLVMMoveBasicBlockBefore/After` or instruction-level moves), replace all uses of `header` with `entry`, delete `header`
- [ ] For loop entry blocks (header has >1 predecessor): leave as-is. These are loop preheaders.
- [ ] Update scoring tool: `.claude/skills/code-journey/control_flow_metrics.py` line 46 (`_is_empty_block()`) counts ALL blocks with only a `br` as empty. Update to exclude the function entry block (first block) — entry blocks that branch to a loop header are structural preheaders, not defects.
  - Fix: Add a check in `control_flow_metrics()` (line ~101): when iterating blocks, skip the first block (entry) from the empty-block count. Or: modify `_is_empty_block()` to take an `is_entry` flag.
  - Alternative: only count empty blocks that are interior (not entry, and not the only predecessor of a block with phi nodes). This is more precise but requires predecessor analysis.
- [ ] Test: Verify TCO still works in J3 `@gcd` after simplification
  - Run the journey and check exit code = 61
  - Check for correct phi-based loop structure in the IR
- [ ] Test: Verify J7 `@sum_loop` and `@sum_for` loop correctly (preheader blocks remain, loops work)
- [ ] Test: If any journey has a simple entry → single-predecessor successor, verify merging works

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `emit_function.rs` under 500 lines (dead-unwind extracted)
- [ ] `cfg_simplify.rs` exists with tested `simplify_cfg()` function
- [ ] Zero empty blocks (single `br`, no phi, non-preheader) in all 13 journey IR dumps
- [ ] Loop preheader entry blocks recognized as structural (not counted as CF defects)
- [ ] Zero redundant conditional branches (both arms same) in any IR
- [ ] Entry blocks merged where safe (single-predecessor successors only)
- [ ] All 13 journeys CF score 10/10
- [ ] All 13 journeys still PASS (eval and AOT match)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `cargo b --release && timeout 150 ./test-all.sh` green
- [ ] Block counts reduced: J2 (-5), J3 (-3 or -4 depending on entry merge feasibility), J5 (-1), J7 (preheader blocks remain — verify they are excluded from CF scoring), J9 (-4), J10 (verify post-simplification count), J12 (-3)
- [ ] `.claude/skills/code-journey/control_flow_metrics.py` updated: loop preheader blocks not counted as empty-block defects

**Exit Criteria:** `extract-metrics.py` reports 0 CF defects for all 13 journeys. No unnecessary empty blocks in emitted IR. No redundant branches. Loop preheaders recognized as structural. Zero test regressions.

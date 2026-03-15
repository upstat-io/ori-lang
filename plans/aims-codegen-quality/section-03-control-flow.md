---
section: "03"
title: "Control Flow Cleanup"
status: not-started
goal: "All journeys CF score ≥ 9/10 by eliminating empty blocks and redundant branches"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Empty Trampoline Block Elimination"
    status: not-started
  - id: "03.2"
    title: "Redundant Entry Block Merging"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Control Flow Cleanup

**Status:** Not Started
**Goal:** Eliminate all empty trampoline blocks and redundant entry blocks across all 13 journeys. Target: CF score ≥ 9/10 for all journeys.

**Context:** The LLVM codegen emits "trampoline" blocks — basic blocks containing only `br label %next` — as artifacts of the lowering process. These are harmless but wasteful: they add block count, confuse the optimizer, and reduce CF scores. Similarly, TCO and loop lowering emit entry blocks with only `br label %header` that can be merged with their successor.

**Current CF scores:**
| Journey | Score | Defects | Primary Issue |
|---------|-------|---------|---------------|
| J2 | 8/10 | 5 | Empty blocks in my_abs, my_sign |
| J3 | 7/10 | 4 | Empty blocks in fib, entry in gcd |
| J5 | 8/10 | 2 | Empty blocks in closures |
| J7 | 7/10 | 5 | Empty blocks + redundant entry |
| J9 | 7/10 | 4 | SSO gating empty blocks |
| J10 | 7/10 | varies | Empty blocks in iteration |
| J12 | 7/10 | 3 | Empty blocks in safe_div, unwrap_or |

**Affected journeys:** J2, J3, J5, J7, J9, J10, J12 (7 of 13)

**Reference implementations:**
- **LLVM** `lib/Transforms/Utils/SimplifyCFG.cpp`: `MergeBlockIntoPredecessor` — folds empty blocks
- **Rust** `compiler/rustc_codegen_llvm/src/builder.rs`: emits directly into successor, avoiding empty blocks

**Depends on:** None.

---

## 03.1 Empty Trampoline Block Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs`, `compiler/ori_llvm/src/codegen/ir_builder/control_flow.rs`, `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs` (overflow check block patterns), potentially `compiler/ori_arc/src/pipeline/aims_pipeline.rs`

Empty trampoline blocks (`br label %next` only) appear after:
- Overflow check branches (`add.ok`, `mul.ok`) — the "ok" path is an empty jump
- If/else lowering — merge blocks that just jump to the true continuation
- Match lowering — case blocks that just jump to the merge point

Two approaches:

**(a) Post-emission CFG simplification pass** (recommended):
After all LLVM IR is emitted for a function, run a single pass that merges empty blocks into their predecessors. This is clean, doesn't complicate emission, and catches all patterns.

**(b) Avoid emission** — modify the emitter to detect when it's about to create an empty block and instead branch directly to the successor. This is harder because the emitter doesn't always know a block will be empty at the time it creates it.

- [ ] **Implement option (a)**: Post-emission block simplification
  - After emitting all instructions for a function, iterate blocks
  - If a block contains only `br label %target`, replace all jumps to this block with jumps to `%target`
  - If a block has a conditional branch where both targets are the same, replace with unconditional
  - Remove the now-unreferenced empty blocks
  - **API**: Use inkwell's `BasicBlock::get_terminator()` to check instruction count. Use `instruction.replace_all_uses_with()` or manually patch predecessor terminators. LLVM `BasicBlock::eraseFromParent()` removes the block.
  - **Where to add**: In `emit_function.rs` after `emit_arc_ir()` returns, before function verification. This catches all patterns from all emission sub-passes.
- [ ] **Alternative**: Check if LLVM's `SimplifyCFG` pass already handles this — if so, just add it to the pass pipeline in `compiler/ori_llvm/src/aot/passes/config.rs` (note: `SimplifyCFG` is already mentioned in the `O1` opt level description there). **IMPORTANT**: `SimplifyCFG` only runs at O1+. At O0 (debug builds), no optimization passes run. The code journey scoring tool operates on O0 IR, so a pre-optimization simplification is needed to improve scores regardless of opt level.
- [ ] **Test**: J2 `@my_abs` should have 3 blocks (not 5), J7 `@sum_loop` should have fewer blocks
- [ ] **Verify**: All 13 journeys still PASS

### Cleanup (03.1)

- [ ] **[BLOAT]** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` — Currently 506 lines, exceeds 500-line limit. The post-emission block simplification pass (option a) will ADD code to this file. Before adding, extract the dead-unwind detection logic (lines 96-167) into a helper function `detect_dead_unwind_blocks()` to reclaim ~70 lines and bring the file well under limit.

---

## 03.2 Redundant Entry Block Merging

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

TCO lowering and loop lowering emit an entry block with only `br label %loop.header`. This is a common pattern that can be eliminated by making the loop header the entry block.

- [ ] **Identify**: Which functions have redundant entry blocks (J3 `@gcd`, J7 `@sum_loop`, J7 `@sum_for`)
- [ ] **Fix**: Either merge entry with header during emission, or rely on the post-emission simplification from 03.1
- [ ] **Verify**: Entry block merging doesn't break TCO (the entry block exists to create the phi-node landing point for tail-recursive calls)
- [ ] **CONSTRAINT**: An empty entry block that branches to a header with phi nodes CANNOT be simply eliminated — the phi nodes need the entry block as an incoming edge source. The simplification pass must handle this: when merging `entry → header`, move all phi-node incoming values from `entry` to the merged block's predecessors. If the entry block is the ONLY predecessor (true for function entries), the phi nodes can be replaced with their single incoming value. If the header has OTHER predecessors (e.g., back-edges from tail calls), the entry block cannot be merged without rewriting the phis.
- [ ] **Test**: Verify TCO still works in J3 `@gcd` — run `ORI_DUMP_AFTER_LLVM=1` and check for `musttail call` or equivalent phi-based TCO pattern.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Zero empty trampoline blocks in all 13 journey IR dumps
- [ ] Zero redundant entry blocks (or entry blocks folded by simplification)
- [ ] All journeys CF score ≥ 9/10
- [ ] All journeys still PASS (eval and AOT match)
- [ ] `./test-all.sh` green
- [ ] Block counts reduced in J2, J3, J5, J7, J9, J10, J12

**Exit Criteria:** No empty blocks containing only `br label %target` in any emitted LLVM IR. All CF scores ≥ 9/10. Zero test regressions.

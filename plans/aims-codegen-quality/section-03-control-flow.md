---
section: "03"
title: "Control Flow Cleanup"
status: complete
goal: "All journeys CF score ≥ 9/10 by eliminating empty blocks and redundant branches"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Empty Trampoline Block Elimination"
    status: complete
  - id: "03.2"
    title: "Redundant Entry Block Merging"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
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

- [x] **Implemented option (a)**: Post-emission CFG simplification pass in `cfg_simplify/mod.rs`. Called after emission at 4 sites (define_phase.rs, nounwind.rs). Handles empty block elimination, redundant conditional merging, entry block merging. 7 unit tests + 4 AOT integration tests. (2026-03-16, via AIMS-10 Section 02)
- [x] **Alternative resolved**: Custom pass runs at O0 (before LLVM optimization), ensuring clean IR at all opt levels. (2026-03-16)
- [x] **Test**: All 13 journeys have zero CF defects. J2 `my_abs` has 4 blocks (all structurally necessary). (2026-03-16)
- [x] **Verify**: All 13 journeys PASS — 12,908 tests, 0 failures. (2026-03-16)

### Cleanup (03.1)

- [x] **[BLOAT]** `emit_function.rs` reduced from 570 → 438 lines by extracting dead-unwind logic to `dead_unwind.rs`. CFG simplification is in a separate module `cfg_simplify/mod.rs`, not in emit_function. (2026-03-16, via AIMS-10 Section 02.1)

---

## 03.2 Redundant Entry Block Merging

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

TCO lowering and loop lowering emit an entry block with only `br label %loop.header`. This is a common pattern that can be eliminated by making the loop header the entry block.

- [x] **Identify**: No journeys currently have redundant empty entry blocks — all entries have real instructions or are loop preheaders. J3 `@gcd`, J7 `@sum_loop`/`@sum_for` entries are non-empty. (2026-03-16)
- [x] **Fix**: Entry block merging implemented in `merge_entry_block()` — uses `LLVMMoveBasicBlockBefore` to swap when conditions are met. Loop preheaders (>1 predecessor) correctly preserved. (2026-03-16)
- [x] **Verify**: TCO works correctly — J3 `@gcd` passes all tests. Entry blocks with phi-bearing successors with >1 predecessor are correctly left alone. (2026-03-16)
- [x] **Test**: Unit test `cfg_simplify_preserves_loop_preheader_entry` verifies loop preheader preservation. Unit test `cfg_simplify_merges_entry_with_single_pred_successor` verifies entry merging. (2026-03-16)

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [x] Zero empty trampoline blocks in all 13 journey IR dumps (2026-03-16)
- [x] Zero redundant entry blocks — merging implemented, no journeys currently trigger it (all entries have real instructions) (2026-03-16)
- [x] All journeys CF score 10/10 (exceeds ≥ 9/10 target) (2026-03-16)
- [x] All journeys still PASS (eval and AOT match) — 12,908 tests, 0 failures (2026-03-16)
- [x] `./test-all.sh` green (2026-03-16)
- [x] Block counts verified — all journeys have zero CF defects (2026-03-16)

**Exit Criteria:** No empty blocks containing only `br label %target` in any emitted LLVM IR. All CF scores ≥ 9/10. Zero test regressions.

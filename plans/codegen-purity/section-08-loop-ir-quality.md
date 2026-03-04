---
section: "08"
title: "Loop IR Quality"
status: not-started
goal: "Loops emit minimal IR — no duplicate computations, no loop-invariant phis, optimized range checks for common patterns"
inspired_by:
  - "LLVM GVN/CSE passes — what the optimizer does that we should do at emission"
  - "Zig src/codegen.zig — specialized loop codegen for common range patterns"
depends_on: ["01"]
sections:
  - id: "08.1"
    title: "Compound Assignment CSE"
    status: not-started
  - id: "08.2"
    title: "Loop-Invariant Phi Elimination"
    status: not-started
  - id: "08.3"
    title: "Range Iteration Specialization"
    status: not-started
  - id: "08.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Loop IR Quality

**Status:** Not Started
**Goal:** Loop bodies emit exactly the instructions needed — no duplicate computations within a single iteration, no values carried through phis that never change, and common range patterns (`1..=n by 1`) emit optimized bounds checks.

**Context:** Three distinct loop IR quality issues found across code journeys:

1. **Duplicate CSE (L-6, J7):** In `_ori_sum_loop`, `i + 1` is computed twice per iteration — once for `total += i + 1` and once for `i += 1`. Both are identical checked additions. A smarter compound assignment lowering or local CSE pass could eliminate the duplicate.

2. **Loop-invariant phi (L-8, J10):** In `_ori_check_iteration`, a list struct `%v11` is carried through a phi in the loop header but its value never changes between iterations. The codegen creates the phi because the value originates before the loop, but it could be referenced directly.

3. **Range specialization (L-9, J7):** The generalized range iteration evaluates 8 instructions per loop iteration (step direction, inclusive/exclusive bounds, ascending/descending comparison). For the overwhelmingly common case `0..n by 1` or `1..=n by 1`, this could be a single `icmp slt`/`icmp sle`.

**Journeys affected:** J7 (CSE, range), J10 (loop-invariant phi).

---

## 08.1 Compound Assignment CSE

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/operators.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

When `total += expr` and `i += 1` appear in the same loop body and share a common subexpression (`i + 1`), the codegen should compute it once and reuse the result.

Two approaches:
- **(a) Local value numbering**: After emitting each arithmetic operation, cache `(op, lhs, rhs) → result`. Before emitting, check the cache. This is a mini-CSE within a single block.
- **(b) Smarter compound assignment lowering**: When `x += expr` appears, check if `expr` was already computed for another purpose in the same block. If so, reuse it.

- [ ] Determine which approach fits the current codegen architecture
- [ ] Implement local CSE or compound assignment optimization
- [ ] Restrict reuse to semantically identical checked operations (same op, operands, and overflow behavior)
- [ ] Invalidate/restrict reuse across side-effecting operations (calls, stores, potentially trapping ops)
- [ ] Verify: J7 `_ori_sum_loop` computes `i + 1` exactly once per iteration
- [ ] Verify: no regression in overflow checking (single checked result reused, never replaced with unchecked add)

---

## 08.2 Loop-Invariant Phi Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs`

When a value used inside a loop is defined before the loop and never modified within the loop, do not create a phi node for it in the loop header. Reference the pre-loop value directly.

- [ ] Identify where loop header phis are created for values that don't change
- [ ] Check: does the value have an incoming edge from the loop body that differs from the pre-header edge?
- [ ] If both edges carry the same value, skip the phi and use the value directly
- [ ] Verify: J10 `_ori_check_iteration` has no loop-invariant phi for the list struct

---

## 08.3 Range Iteration Specialization

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_loops/for_range.rs`

The current range iteration condition handles all configurations (ascending/descending, inclusive/exclusive, arbitrary step). For the common case `start..end by 1` (ascending, exclusive, step 1), emit a single `icmp slt i64 %i, %end` instead of 8 instructions.

Specialization candidates:
- `0..n` → `icmp slt i64 %i, %n`
- `0..=n` → `icmp sle i64 %i, %n`
- `n..0 by -1` → `icmp sgt i64 %i, 0`
- General case → keep existing 8-instruction check

- [ ] Detect common range patterns at the ARC IR level (or during codegen)
- [ ] Emit specialized bounds checks for detected patterns
- [ ] Keep the general path as fallback
- [ ] Verify: J7 `_ori_sum_for` with `1..=n by 1` emits `icmp sle` (not 8 instructions)
- [ ] Verify: general ranges (e.g., `0..100 by 3`) still work correctly
- [ ] Add coverage matrix for ascending/descending × inclusive/exclusive × positive/negative/non-unit step

---

## 08.4 Completion Checklist

- [ ] No duplicate arithmetic operations in loop bodies (CSE)
- [ ] No loop-invariant phi nodes
- [ ] Common range patterns emit 1-instruction bounds check
- [ ] General ranges still emit correct (if verbose) bounds checks
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Loop-heavy programs show measurable instruction count reduction

**Exit Criteria:** J7 loop body has exactly one `i + 1` computation. J10 loop header has no invariant phis. J7 range loop uses `icmp sle` for `1..=n by 1`. All loop-related tests pass.

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

**File(s):** `compiler/ori_arc/src/lower/control_flow/loops.rs` (loop lowering — where compound assignments are desugared), `compiler/ori_llvm/src/codegen/arc_emitter/operators.rs` (operator emission — where CSE cache would be checked)

> **WARNING — HIGH COMPLEXITY:** Local value numbering is non-trivial even within a single basic block. The cache must handle: (1) invalidation on any side-effecting operation (calls, stores, trapping checked arithmetic), (2) correct scoping (cache lifetime = single block, cleared at terminators), (3) the distinction between checked and unchecked operations (a checked add result cannot substitute for an unchecked add, and vice versa), (4) the SSA value identity problem (cache keys must be LLVM SSA values, not ARC IR variable IDs — the same ARC var may map to different SSA values after phi nodes). Consider whether the effort is justified for the single known instance (J7 `i+1`) or if this should be deferred.

> **Phase boundary decision required:** CSE can be implemented at the ARC IR level (in `ori_arc`) or at the LLVM emission level (in `ori_llvm`). ARC-level CSE would be a new optimization pass operating on `ArcInstr`; LLVM-level CSE would cache emitted LLVM values in the `arc_emitter`. The ARC IR level is preferred (fixes at source, benefits all backends), but requires adding a new pass to the ARC pipeline. The LLVM emission level is simpler but couples optimization logic to the LLVM backend. Choose explicitly.

> **TDD requirement:** Write IR-quality tests capturing the current duplicate computation FIRST. Verify both `i+1` computations appear in the IR. Then implement CSE and verify only one remains.

When `total += expr` and `i += 1` appear in the same loop body and share a common subexpression (`i + 1`), the codegen should compute it once and reuse the result.

Two approaches:
- **(a) Local value numbering**: After emitting each arithmetic operation, cache `(op, lhs, rhs) → result`. Before emitting, check the cache. This is a mini-CSE within a single block.
- **(b) Smarter compound assignment lowering**: When `x += expr` appears, check if `expr` was already computed for another purpose in the same block. If so, reuse it.

- [ ] Choose approach: (a) local value numbering (cache `(op, lhs_ssa, rhs_ssa) -> result` per block) or (b) compound-assignment-aware lowering in `ori_arc`. Choose ARC-level (new pass) or LLVM-level (emitter cache). Document choice rationale.
- [ ] Implement the chosen CSE approach
- [ ] Restrict reuse to semantically identical checked operations (same op, operands, and overflow behavior)
- [ ] Invalidate/restrict reuse across side-effecting operations (calls, stores, potentially trapping ops)
- [ ] Verify: J7 `_ori_sum_loop` computes `i + 1` exactly once per iteration
- [ ] Verify: no regression in overflow checking (single checked result reused, never replaced with unchecked add)

### Edge Cases for CSE

- **Same operation, different overflow messages:** `checked_add(a, b)` for user `+` and `checked_add(a, b)` for `+=` both call `emit_checked_binop`. Both produce `"integer overflow on addition\00"`. They are CSE-eligible only if both operands are identical SSA values.
- **Commutativity:** `a + b` and `b + a` are mathematically equivalent but have different SSA operand order. Do NOT CSE these — checked operations may have different overflow behavior for non-commutative ops (subtraction, division).
- **Cross-block reuse:** CSE within a single basic block is straightforward. Cross-block CSE (e.g., loop body reusing a value from the loop header) is significantly more complex and should be deferred.
- **Trapping operations:** Checked arithmetic can trap (panic on overflow). A trapped operation has side effects — do not CSE a trapping operation across a control flow boundary where the first one might not execute.

### 08.1 Completion Checklist

- [ ] Duplicate arithmetic operations within a single block/iteration are eliminated
- [ ] J7 `_ori_sum_loop` computes `i + 1` exactly once per iteration
- [ ] CSE does not cross side-effecting operations (calls, stores)
- [ ] CSE does not cross basic block boundaries (single-block CSE only)
- [ ] Overflow checking preserved — reused result is from a checked operation, not an unchecked shortcut
- [ ] Commutative operations are NOT naively CSE'd (operand order matters for checked ops)
- [ ] IR test: loop body with `total += i + 1; i += 1` has exactly 1 checked add for `i + 1`
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 08.2 Loop-Invariant Phi Elimination

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` (phi node creation), `compiler/ori_arc/src/lower/control_flow/loops.rs` (loop block structure — controls which values are block params)

> **Phase boundary:** The root cause is in `ori_arc` — the loop lowerer adds the value as a block param to the loop header even when it never changes. The fix should be at the ARC IR level: do not add invariant values as block params. The LLVM emitter faithfully emits phi nodes for all block params (correct behavior). Do NOT fix this in `emit_function.rs` — that would be working around an ARC IR deficiency in the LLVM backend (LEAK: phase bleeding).

> **TDD requirement:** Write an IR-quality test capturing the current loop-invariant phi FIRST. Then fix in `ori_arc` and verify the phi disappears.

When a value used inside a loop is defined before the loop and never modified within the loop, do not create a phi node for it in the loop header. Reference the pre-loop value directly.

- [ ] Identify where loop header block params are created in `loops.rs` for values that don't change within the loop
- [ ] In `ori_arc/src/lower/control_flow/loops.rs`: check if a value added as a loop header block param is invariant (same value on all back-edges as on entry)
- [ ] If invariant: do not add as block param; reference the pre-loop value directly in the loop body
- [ ] Verify: J10 `_ori_check_iteration` has no loop-invariant phi for the list struct

### 08.2 Completion Checklist

- [ ] No loop-invariant phi nodes in loop headers (both incoming edges carry the same value)
- [ ] J10 `_ori_check_iteration` references the list struct directly (no phi)
- [ ] IR test: loop using a pre-loop value without modification has no phi for that value
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## 08.3 Range Iteration Specialization

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_loops/for_range.rs`

> **Spec consultation required:** Before implementing specialization, verify range iteration semantics against `docs/ori_lang/v2026/spec/` (Clause for ranges and `for` loops). The specialization must produce identical behavior to the general path for all edge cases: empty ranges, single-element ranges, `INT_MAX` bounds, negative steps with unsigned-like patterns. Cite the spec clause in implementation comments.

> **TDD requirement:** Write spec tests for each specialization candidate (`0..n`, `0..=n`, `n..0 by -1`) covering normal cases AND edge cases (empty range, single element, overflow-adjacent bounds) BEFORE implementing specialization. Verify they pass with the current general path. Then implement specialization and verify tests still pass unchanged.

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

### 08.3 Completion Checklist

- [ ] Common range patterns (`0..n`, `0..=n`, `n..0 by -1`) emit 1-instruction bounds check
- [ ] General ranges still emit correct (if verbose) bounds checks
- [ ] J7 `_ori_sum_for` with `1..=n by 1` emits `icmp sle` (1 instruction, not 8)
- [ ] IR test: `for i in 0..n` emits `icmp slt` only
- [ ] Coverage: ascending/descending × inclusive/exclusive × unit/non-unit step all pass
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] No regressions in `cargo test -p ori_llvm`

---

## Section 08 Exit Criteria

J7 loop body has exactly one `i + 1` computation. J10 loop header has no invariant phis. J7 range loop uses `icmp sle` for `1..=n by 1`. All loop-related tests pass. Loop-heavy programs show measurable instruction count reduction.

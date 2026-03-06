---
section: "08"
title: "Loop IR Quality"
status: complete
goal: "Loops emit minimal IR — no duplicate computations, no loop-invariant block params, optimized range checks for common patterns"
inspired_by:
  - "LLVM GVN/CSE passes — what the optimizer does that we should do at emission"
  - "Zig src/codegen.zig — specialized loop codegen for common range patterns"
depends_on: ["01"]
sections:
  - id: "08.1"
    title: "Compound Assignment CSE"
    status: complete
  - id: "08.2"
    title: "Loop-Invariant Block Param Elimination"
    status: complete
  - id: "08.3"
    title: "Range Iteration Specialization"
    status: complete
---

# Section 08: Loop IR Quality

**Status:** Complete
**Goal:** Loop bodies emit exactly the instructions needed — no duplicate computations within a single iteration, no values carried through block params that never change, and common range patterns (`1..=n by 1`) emit optimized bounds checks.

**Context:** Three distinct loop IR quality issues found across code journeys:

1. **Duplicate CSE (L-6, J7):** In `_ori_sum_loop`, `i + 1` is computed twice per iteration — once for `total += i + 1` and once for `i += 1`. Both are identical checked additions. A smarter compound assignment lowering or local CSE pass could eliminate the duplicate.

2. **Loop-invariant block param (L-8, J10):** In `_ori_check_iteration`, a list struct `%v11` is carried through a block param in the loop header but its value never changes between iterations. The ARC lowerer creates the block param because the value originates before the loop, but it could be referenced directly. (At the LLVM level, this block param becomes a phi node with identical incoming values on all edges.)

3. **Range specialization (L-9, J7):** The generalized range iteration evaluates 8 instructions per loop iteration (step direction, inclusive/exclusive bounds, ascending/descending comparison). For the overwhelmingly common case `0..n by 1` or `1..=n by 1`, this could be a single `icmp slt`/`icmp sle`.

**Journeys affected:** J7 (CSE, range), J10 (loop-invariant block param).

**Recommended implementation order: 08.3 → 08.2 → 08.1.** Section numbers reflect logical grouping, not execution order. 08.3 (range specialization) is lowest risk and highest impact on J7. 08.2 (invariant block params) extends existing block_merge infrastructure. 08.1 (CSE) is highest complexity and may be deferred if 08.3 makes the J7 duplicate moot.

---

## 08.1 Compound Assignment CSE

**File(s):** `compiler/ori_arc/src/lower/control_flow/loops.rs` (loop lowering — produces the ARC IR blocks where duplicate `PrimOp::Binary(Add)` instructions appear), `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs` (where `IrBuilder::emit_checked_binop` lives — the LLVM-level CSE cache insertion point), `compiler/ori_llvm/src/codegen/arc_emitter/operators.rs` (operator dispatch — calls `self.builder.checked_add()` etc.). Note: compound assignments (`+=`) are desugared at **parse time** in `compiler/ori_parse/src/grammar/expr/mod.rs` (desugars `x += y` to `x = x + y`), so the ARC lowerer never sees compound assignment nodes — it sees separate `Assign` and `Binary` expressions that happen to share the same subexpression.

> **WARNING — HIGH COMPLEXITY:** Local value numbering is non-trivial even within a single basic block. The cache must handle: (1) invalidation on any side-effecting operation (calls, stores, trapping checked arithmetic), (2) correct scoping (cache lifetime = single block, cleared at terminators), (3) the distinction between checked and unchecked operations (a checked add result cannot substitute for an unchecked add, and vice versa), (4) the SSA value identity problem (cache keys must be LLVM SSA values, not ARC IR variable IDs — the same ARC var may map to different SSA values after LLVM phi nodes). Consider whether the effort is justified for the single known instance (J7 `i+1`) or if this should be deferred.

> **Phase boundary decision:** CSE can be implemented at the ARC IR level (in `ori_arc`) or at the LLVM emission level (in `ori_llvm`). Both approaches are analyzed in the "Approach Tradeoff Analysis" below. **The recommendation is approach (b), LLVM-level `IrBuilder` cache** — simpler, directly addresses L-6, minimal risk. File a follow-up for ARC-level CSE if a WASM backend materializes.

> **TDD requirement:** Write IR-quality tests capturing the current duplicate computation FIRST. Verify both `i+1` computations appear in the IR. Then implement CSE and verify only one remains.

When `total += expr` and `i += 1` appear in the same loop body and share a common subexpression (`i + 1`), the codegen should compute it once and reuse the result.

### Concrete IR Shape (L-6 Duplication)

The ARC IR for a loop body with `total += i + 1; i += 1` produces (simplified):

```
body_block:
  %v10 = PrimOp::Binary(Add) [%i_var, %const_1]   ; for total += (i + 1)
  %v11 = PrimOp::Binary(Add) [%total, %v10]         ; total + (i + 1)
  ; ... assign total = %v11 ...
  %v12 = PrimOp::Binary(Add) [%i_var, %const_1]   ; for i += 1  <-- DUPLICATE of %v10
  ; ... assign i = %v12, jump to latch ...
```

At the LLVM level, each `PrimOp::Binary(Add)` on integers emits a full `checked_add` sequence (call `@llvm.sadd.with.overflow.i64`, extract result, extract overflow flag, conditional branch to panic, continue block). This means the duplication costs not 1 but ~7 LLVM instructions per redundant computation: the intrinsic call, two `extractvalue`, one `icmp`, one `br`, the panic block infrastructure, and the continue block.

The body block (in ARC IR) is lowered to the LLVM body block. But note the `for_range` lowering separates the latch: the latch block computes `next = i + step` as a *third* `PrimOp::Binary(Add)`. However, the latch uses `[i_var, step]` — step is the range step (projected from the range struct), while the body uses `[i_var, const_1]`. These are only CSE-eligible if `step` and `const_1` resolve to the same SSA value. For `1..=n by 1`, `step` is a projected field, not a literal — so they may NOT be identical SSA values even though they represent the same integer.

**Important distinction:** The latch's `i + step` and the body's `i + 1` are only CSE-eligible if both operands are provably the same SSA values. At the ARC IR level, `step` comes from `emit_project(iter_val, 2)` while `1` comes from a `Literal(Int(1))`. These are different `ArcVarId`s. ARC-level CSE must use value numbering (not just variable identity) to detect this — or the specialization in 08.3 can make it moot by eliminating the latch computation entirely for known step=1 ranges.

### Approach Tradeoff Analysis

**(a) ARC-level CSE pass** (new pass in `ori_arc`, runs after lowering, before or after RC insertion):
- **Pros:** Backend-independent (benefits future WASM backend), operates on clean SSA form, can reason about `ArcVarId` equivalences.
- **Cons:** Requires adding a new pass to `run_arc_pipeline()` with explicit ordering documentation. Must handle all `ArcInstr` types (not just `PrimOp`). ARC IR `PrimOp::Binary(Add)` operands are `ArcVarId`s — two adds with the same operands are trivially CSE-eligible. But ARC IR doesn't distinguish checked vs unchecked (all integer adds are `PrimOp::Binary(Add)`; checked-ness is determined at LLVM emission by type). Risk: over-aggressive CSE could eliminate a trapping operation that should execute for its side effect.
- **Complexity:** Medium. The cache is `HashMap<(PrimOp, Vec<ArcVarId>), ArcVarId>` per block. Invalidation: clear on any `Apply`/`ApplyIndirect`/`Set` (side-effecting). The pass replaces duplicate `Let { PrimOp { ... } }` with `Let { Var(existing) }`.
- **Pipeline ordering:** Must run AFTER lowering (needs complete blocks) and BEFORE RC insertion (so RC sees the deduplicated vars). Add to `run_arc_pipeline()` between lowering and borrow inference. Document in `lib.rs`.

**(b) LLVM-level `IrBuilder` cache** (in `IrBuilder`, per-block `HashMap`):
- **Pros:** Simpler implementation — cache `(intrinsic_name, lhs_ssa, rhs_ssa) → ValueId` in `IrBuilder::emit_checked_binop` (in `checked_ops.rs`). No new ARC pass. Directly operates on LLVM SSA values.
- **Cons:** Couples optimization to LLVM backend. Does not benefit future WASM backend. Cache key must use `ValueId` (not raw LLVM values) — the `ValueArena` provides stable IDs. Must clear cache at every LLVM basic block boundary.
- **Complexity:** Low. Cache lives in `IrBuilder` as `cse_cache: HashMap<(String, ValueId, ValueId), ValueId>`. Check at the top of `emit_checked_binop`; store before returning. Clear when `position_at_end()` is called for a new block (detected via `self.current_block` change) or when entering a new ARC block in `ArcIrEmitter::emit_function`.
- **Cache invalidation:** Within an LLVM block, checked arithmetic does not invalidate the cache because the overflow branch either panics (program terminates) or continues (result is available). The overflow panic block is a separate LLVM block, so it naturally gets its own empty cache. `Apply`/`ApplyIndirect` calls with observable side effects could theoretically invalidate, but since the CSE only caches pure arithmetic intrinsics (not arbitrary calls), this is safe.
- **Important note on `emit_checked_binop` block creation:** Each call to `emit_checked_binop` creates TWO new LLVM blocks (panic_bb and continue_bb) and positions the builder at `continue_bb`. The CSE cache must track which `continue_bb` subsequent operations are in. Since the cache key includes the operand `ValueId`s (which change after LLVM phi nodes in new blocks), this is naturally handled — but the cache must NOT be cleared just because `position_at_end(continue_bb)` was called within `emit_checked_binop` itself. The cache should only clear on explicit ARC-block-boundary transitions.

**Recommendation:** Start with **(b) LLVM-level `IrBuilder` cache** for this cycle. It directly addresses the L-6 finding with minimal risk and complexity. The cache goes in `IrBuilder` (not `ArcIrEmitter`), checked inside `emit_checked_binop` in `checked_ops.rs`. File a follow-up for (a) ARC-level CSE if a WASM backend materializes. The ARC-level approach becomes more valuable when multiple backends exist.

- [x] **1. PREREQUISITE:** Split `emit_function.rs` (579 lines, over 500-line limit) before adding any CSE infrastructure. Extract `scan_used_fields()` (~150 lines) into `field_scan.rs`. (2026-03-05)
- [x] **2. Add `cse_cache` field** to `IrBuilder`: `HashMap<(&'static str, CseOperand, CseOperand), ValueId>` in `checked_ops.rs`. Uses `CseOperand` enum to normalize constant operands so identical constants match regardless of `ValueId`. (2026-03-05)
- [x] **3. Add cache lookup** at the top of `IrBuilder::emit_checked_binop`: if key exists, return cached result (skip intrinsic call, extract, branch, and continue-block creation). (2026-03-05)
- [x] **4. Add cache store** at the bottom of `emit_checked_binop`: after the continue block is created and the result is available, insert into cache. (2026-03-05)
- [x] **5. Add `clear_cse_cache()` method** on `IrBuilder`. Call it from `ArcIrEmitter` at each ARC-block-boundary transition (NOT from internal `position_at_end` calls within `emit_checked_binop`, which creates panic/continue blocks as part of a single operation). (2026-03-05)

> **RISK: `position_at_end` vs block boundaries.** Each `emit_checked_binop` call creates two new LLVM blocks (panic_bb and continue_bb) and calls `position_at_end(continue_bb)` internally. The CSE cache must NOT be cleared on these internal calls — only on explicit ARC-block-boundary transitions. Use the dedicated `clear_cse_cache()` method called from `ArcIrEmitter`, not a hook on `position_at_end`.

- [x] **6.** Restrict cache reuse to semantically identical checked operations (same intrinsic name, same operand values, same overflow behavior). `CseOperand` normalizes constants; SSA values use identity. (2026-03-05)
- [x] **7.** Do NOT cache across side-effecting operations (calls, stores); cache cleared at ARC block boundaries. Within a single ARC block, checked arithmetic does not invalidate the cache because overflow either panics (terminates) or continues (result available). (2026-03-05)
- [x] **8.** Scope: each ARC block gets its own cache (cleared on block entry via `clear_cse_cache()`). Inner loop blocks get independent caches. The LLVM phi node for `i` produces a new SSA value each iteration, so `i+1` in iteration N is naturally a different cache key from `i+1` in iteration N+1 — no cross-iteration staleness. (2026-03-05)
- [x] **9. Measure:** Before: 3 `sadd.with.overflow` calls in `_ori_sum_loop`. After: 2 (duplicate `i+1` eliminated). Entire panic/continue block sequence for the duplicate removed (~7 LLVM instructions saved per iteration). (2026-03-05)
- [x] Verify: J7 `_ori_sum_loop` computes `i + 1` exactly once per iteration — confirmed via IR dump and `test_cse_loop_duplicate_add_eliminated`. (2026-03-05)
- [x] Verify: no regression in overflow checking — reused result comes from a checked `sadd.with.overflow` call; all 12,098 tests pass. (2026-03-05)

### Edge Cases for CSE

- **Same operation, different overflow messages:** `checked_add(a, b)` for user `+` and `checked_add(a, b)` for `+=` both call `emit_checked_binop`. Both produce `"integer overflow on addition\00"`. They are CSE-eligible only if both operands are identical SSA values.
- **Commutativity:** `a + b` and `b + a` are mathematically equivalent but have different SSA operand order. Do NOT CSE these — checked operations may have different overflow behavior for non-commutative ops (subtraction, division).
- **Cross-block reuse:** CSE within a single basic block is straightforward. Cross-block CSE (e.g., loop body reusing a value from the loop header) is significantly more complex and should be deferred.
- **Trapping operations:** Checked arithmetic can trap (panic on overflow). A trapped operation has side effects — do not CSE a trapping operation across a control flow boundary where the first one might not execute.
- **Nested loops:** For `for i in 0..n do { for j in 0..m do { ... i + j ... i + j ... } }`, the inner loop body is a separate ARC block from the outer loop body. CSE operates per-block, so the two `i + j` in the inner body are CSE-eligible (same block), but an `i + j` in the outer body vs inner body are NOT (different blocks). The nested loop's LLVM phi nodes create new SSA values for iteration variables, naturally preventing stale reuse.
- **Latch block interaction:** The `for_range` latch computes `next = i + step` in its own ARC block (`latch_block`). The body computes `i + 1` in `body_block`. Since these are different ARC blocks (and thus different LLVM basic blocks), they are NOT CSE-eligible. This is correct — the latch's result is used as the LLVM phi value for the next iteration, while the body's result is consumed within the iteration.

### 08.1 Completion Checklist

- [x] Duplicate arithmetic operations within a single block/iteration are eliminated (2026-03-05)
- [x] J7 `_ori_sum_loop` computes `i + 1` exactly once per iteration (2026-03-05)
- [x] CSE does not cross side-effecting operations (calls, stores) — cache cleared at ARC block boundaries (2026-03-05)
- [x] CSE does not cross basic block boundaries (single-block CSE only) — scoped per ARC block (2026-03-05)
- [x] Overflow checking preserved — reused result is from a checked operation, not an unchecked shortcut (2026-03-05)
- [x] Commutative operations are NOT naively CSE'd (operand order matters for checked ops) — `test_cse_different_operands_not_eliminated` (2026-03-05)
- [x] Nested loops: inner loop CSE is independent of outer loop CSE (separate cache scopes) — each ARC block gets `clear_cse_cache()` on entry (2026-03-05)
- [x] IR test: loop body with `total += i + 1; i += 1` has exactly 1 checked add for `i + 1` — `test_cse_loop_duplicate_add_eliminated` (2026-03-05)
- [x] IR test: count `@llvm.sadd.with.overflow.i64` calls in loop body — assert reduction from baseline (3 → 2) (2026-03-05)
- [x] `./test-all.sh` green — 12,098 passed, 0 failed (2026-03-05)
- [x] `./clippy-all.sh` green (2026-03-05)
- [x] No regressions in `cargo test -p ori_llvm` — 1,228 AOT tests pass (2026-03-05)

---

## 08.2 Loop-Invariant Block Param Elimination

> **BLOAT: `emit_function.rs` is 579 lines (limit: 500).** Must be split before any modifications. Recommended: extract `scan_used_fields()` (~190 lines) into `field_scan.rs`, reducing to ~390 lines. However, 08.2's fix is entirely in `ori_arc` — this split is only required if 08.1 or other work touches `emit_function.rs`.

**File(s):** `compiler/ori_arc/src/lower/control_flow/loops.rs` (loop block structure), `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs` (iterator-based for-loop — the actual source of the J10 L-8 finding), `compiler/ori_arc/src/lower/control_flow/for_loops/for_range.rs` (range-based for-loop — same pattern), `compiler/ori_arc/src/block_merge/` (where the fix lands). All four loop lowering functions (`lower_loop`, `lower_for_range`, `lower_for_iterator`, `lower_for_option`) share the same pattern of adding all mutable bindings from `pre_scope.mutable_bindings()` as header block params regardless of whether they change within the loop.

> **Terminology mapping (stated once):** ARC IR uses "block params" — values passed as arguments to block jumps. The LLVM emitter (`emit_function.rs`) translates these to LLVM "phi nodes." This section uses "block param" when discussing the ARC IR fix and "phi node" only when referring to the resulting LLVM IR.

> **Phase boundary:** The root cause is in `ori_arc` — the loop lowerers add every mutable binding as a block param to the loop header, even when a binding never changes within the loop body. The LLVM emitter faithfully translates block params to phi nodes (correct behavior). The fix belongs in `ori_arc`: remove invariant block params before they reach the LLVM emitter. Do NOT fix this in `emit_function.rs` — that would be working around an ARC IR deficiency in the LLVM backend (phase bleeding).

> **TDD requirement:** Write an LLVM IR-quality test capturing the current loop-invariant phi node FIRST. Then fix the root cause in `ori_arc` (remove the invariant block param) and verify the LLVM phi disappears.

When a mutable binding used inside a loop is defined before the loop and never modified within the loop body, do not add it as a block param to the loop header. Reference the pre-loop value directly (it dominates the header and all loop blocks).

### Invariance Detection Algorithm

A mutable binding `name` is **loop-invariant** if it is never assigned within any code path in the loop body. The detection must be conservative: if the binding MIGHT be assigned on any path, it must be carried as a block param.

**Proposed algorithm (at lowering time):**

The challenge is that the loop body hasn't been lowered yet when we need to decide which bindings to carry. Two options:

**(Option A) Pre-scan the un-lowered CanExpr body** — Walk the CanExpr AST of the body before lowering to find all assignment targets. This requires a small recursive visitor that collects `Name`s of assigned variables. This is cheap (no IR construction) and provides the information before the header block params are committed.

**(Option B) Post-hoc elimination pass** — Lower the loop body unconditionally with all mutable bindings as block params (current behavior), then run a cleanup pass that removes invariant block params. This is the approach used by §01's Phase 6 (dead param elimination) — extend it to also detect invariant params (all incoming values are identical).

**Recommendation: Option B (post-hoc elimination).** Option A requires a CanExpr visitor that duplicates assignment-detection logic from the lowerer. Option B reuses existing infrastructure (dead param elimination in `block_merge/`) and is more robust — it operates on the actual lowered IR rather than predictions about what lowering will produce.

**How Option B detects invariance:** A block param is invariant if, for every predecessor edge that jumps to the header, the argument at that position is the same `ArcVarId`. Specifically:
1. Collect all `Jump { target: header_block, args }` terminators across the function.
2. For each param position `p`, collect the set of argument values `{ args[p] for each predecessor }`.
3. If all values in the set are identical (same `ArcVarId`), the param is invariant — replace all uses of the param var with the single incoming value, then remove the param and corresponding args.

**Edge case — conditional branches inside the loop body:** If the body has `if cond then { x = 1 } else { ... }`, only one branch modifies `x`. The lowerer already handles this via scope merge (`lower_if_else_with_mutable_merge` in `control_flow/mod.rs`), which produces a merge block with a block param for `x`. The merged value gets passed as the jump arg to the header. Since the two predecessors (the initial entry and the body's back-edge) may pass different `ArcVarId`s for `x`, the param is NOT invariant — correctly detected by Option B.

**ARC interaction:** If an invariant value has RC semantics (`DefiniteRef`), it is safe to skip the block param. The pre-loop `RcInc` (if any) covers the entire loop scope. The value's lifetime spans the loop — it is not consumed by any iteration. The RC insertion pass (`rc_insert/`) already handles this correctly: it inserts `RcInc`/`RcDec` based on liveness, and an invariant value that dominates the loop header is live throughout the loop. Removing the block param does not change liveness — the pre-loop variable still dominates all uses.

**Interaction with §01 block merge:** The dead param elimination in Phase 6 (`block_merge/dead_param.rs`) already removes unused exit block params. The invariant-param detection is a natural extension: Phase 6 checks "is the param's value never read after the block?", while invariant detection checks "do all incoming values agree?" These can be combined into a single pass or run sequentially. The block merge pass runs AFTER lowering and BEFORE LLVM emission — the right place.

- [x] **1.** Confirm: NO changes to `emit_function.rs` needed for 08.2. The fix is entirely in `ori_arc` (block_merge pass). `emit_function.rs` faithfully translates block params to phi nodes — that behavior is correct. (2026-03-05)
- [x] **2.** Add invariant-param detection to the block merge pass as a new Phase 7 in `block_merge/invariant_param.rs`: for each block with params, collect all `Jump { target: block, args }` terminators. For each param position `p`, filter out self-references (back-edge passes param to itself), and if exactly one unique non-self value remains, the param is invariant. (2026-03-05)
- [x] **3.** Handle the entry edge and back-edge: self-references (back-edge passes the param var itself) are filtered out. If the only remaining incoming value is from the entry edge, the param is invariant. (2026-03-05)
- [x] **4.** For each invariant param: substitute all uses of the param var with the common incoming value via `substitute_var()` on instructions and terminators; remove the param and corresponding jump args from all predecessors. (2026-03-05)
- [x] **5.** Write ARC IR unit test `invariant_param_same_value_removed` in `block_merge/tests.rs`: construct a loop header with an invariant param (entry passes v2, back-edge passes v5=self) and verify Phase 7 removes it and substitutes v2. (2026-03-05)
- [x] **6.** Write ARC IR unit test `non_invariant_param_preserved`: verify that params receiving different values from entry (v1) and back-edge (v9) are NOT removed. (2026-03-05)
- [x] **7.** Verify: J10 `_ori_check_iteration` pattern has no loop-invariant phi node — confirmed via IR dump. Current iterator-based loop codegen uses runtime `ori_iter_from_list`, so the list struct is never carried as a block param. Only `found` (modified inside loop) has a phi. (2026-03-05)

### Edge Cases for Invariant Block Params

- **Values used as block param targets for error recovery:** If a value is invariant in normal control flow but modified on an error recovery path (e.g., `try { ... } catch { x = default }`), the catch path is a separate scope — it does not produce a back-edge to the loop header. This is correctly handled: only actual predecessor edges to the header block matter.
- **Break with value:** `break x` jumps to the exit block, not the header. Break edges do not contribute to header param invariance analysis. Correctly handled by Option B (only examines `Jump { target: header_block }` terminators).
- **Continue with mutable var update:** `continue` jumps to the header (for `loop`) or latch (for `for`). If the continue path modifies a mutable var, the arg at that position differs from the entry edge — the param is not invariant. Correctly detected by Option B.
- **Nested loops:** The inner loop's header params are independent of the outer loop's. The invariance pass operates on each block independently — no special handling needed.

### 08.2 Completion Checklist

- [x] No loop-invariant block params in ARC IR loop headers (and consequently no loop-invariant phi nodes in LLVM IR) (2026-03-05)
- [x] J10 `_ori_check_iteration` references the list struct directly (no block param in ARC IR, no phi node in LLVM IR) — confirmed: iterator-based loops use runtime `ori_iter_from_list`, list never becomes a block param (2026-03-05)
- [x] IR test: `test_loop_invariant_binding_no_phi` — loop with pre-loop `limit` binding has 2 phi nodes (total, i) not 3 (2026-03-05)
- [x] ARC unit test: `invariant_param_same_value_removed` — invariant param (entry passes v2, back-edge passes self) is removed (2026-03-05)
- [x] ARC unit test: `non_invariant_param_preserved` — non-invariant params (predecessors pass different values) are preserved (2026-03-05)
- [x] ARC unit test: `mixed_invariant_only_invariant_removed` — mixed invariant and non-invariant params: only invariant ones removed (2026-03-05)
- [x] Invariant param elimination integrates cleanly as Phase 7 after Phase 6 (dead param) — no ordering conflicts, 55 block_merge tests pass (2026-03-05)
- [x] `./test-all.sh` green — 12,104 passed, 0 failed (2026-03-05)
- [x] `./clippy-all.sh` green (2026-03-05)
- [x] No regressions in `cargo test -p ori_llvm` (2026-03-05)

---

## 08.3 Range Iteration Specialization

> **File size note:** `for_range.rs` is currently 312 lines. Adding the specialization match and `get_literal_int` helper call (~30-50 lines of new code) keeps it within the 500-line limit. However, the `get_literal_int()` helper itself should be added to `ArcIrBuilder` (`compiler/ori_arc/src/lower/builder/mod.rs`, currently 327 lines) — not to `for_range.rs`. The builder's total (mod.rs 327 + emission.rs 214 = 541 combined) is manageable since they are already separate files.

**File(s):** `compiler/ori_arc/src/lower/control_flow/for_loops/for_range.rs` (currently 312 lines — safe), `compiler/ori_arc/src/lower/builder/mod.rs` (for `get_literal_int` helper, currently 327 lines — safe)

> **Spec consultation required:** Before implementing specialization, verify range iteration semantics against `docs/ori_lang/v2026/spec/14-expressions.md` (Clause 14.6 — Range Expressions, including 14.6.1 Range with Step) and `docs/ori_lang/v2026/spec/16-control-flow.md` (for-loop semantics). The specialization must produce identical behavior to the general path for all edge cases: empty ranges, single-element ranges, `INT_MAX` bounds, negative steps with unsigned-like patterns. Cite the spec clause in implementation comments.

> **TDD requirement:** Write spec tests for each specialization candidate (`0..n`, `0..=n`, `n..0 by -1`) covering normal cases AND edge cases (empty range, single element, overflow-adjacent bounds) BEFORE implementing specialization. Verify they pass with the current general path. Then implement specialization and verify tests still pass unchanged.

> **Current mitigation (important):** The source code in `for_range.rs` already notes: "For compile-time constant step/inclusive, LLVM constant-folds the dead terms away, producing optimal `icmp slt`/`icmp sle`." This means the optimization already happens at `-O1+` via LLVM's constant folding. The goal of this section is to achieve the same result at `-O0` emission time.

The current range iteration condition in the header block handles all configurations (ascending/descending, inclusive/exclusive, arbitrary step) with 8 per-iteration instructions: `i < end`, `i > end`, `i == end`, `step_pos && lt`, `step_neg && gt`, `asc || desc`, `is_incl && eq`, `base || incl`. Three additional values (`step_pos`, `step_neg`, `is_incl`) are correctly computed before the loop (in the entry block, loop-invariant). For the common case `start..end by 1` (ascending, exclusive, step 1), emit a single `icmp slt i64 %i, %end` instead of 8 instructions.

Specialization candidates:
- `start..end` (ascending, exclusive, step 1) → `icmp slt i64 %i, %end` — **this is the most common case** (standard `for i in 0..n`, but also `for i in start..end` with arbitrary start)
- `start..=end` (ascending, inclusive, step 1) → `icmp sle i64 %i, %end`
- `start..end by -1` (descending, exclusive, step -1) → `icmp sgt i64 %i, %end`
- `start..=end by -1` (descending, inclusive, step -1) → `icmp sge i64 %i, %end`
- General case → keep existing 8-instruction check

### Detection Mechanism

Specialization must detect range properties at the ARC IR level during `lower_for_range`. The range struct `{i64 start, i64 end, i64 step, i64 inclusive}` is projected from the range value. The key question is: can we determine `step` and `inclusive` at compile time?

**When step and inclusive are known at compile time:**
The range constructor (`start..end`, `start..=end`, `start..end by step`) is lowered by the parser into a `Construct` with literal fields for `step` (default 1) and `inclusive` (0 or 1). At the ARC IR level, the range value flows through `emit_project` calls. If the projected step/inclusive values can be traced back to literals, specialization applies.

**Approach: pattern match on the ARC IR values.**
After projecting `step` and `inclusive` from the range struct, check if the defining instruction for each is a `Let { Literal(Int(n)) }`. If so, the value is known at compile time:
- `step == 1 && inclusive == 0` → ascending exclusive → `icmp slt`
- `step == 1 && inclusive == 1` → ascending inclusive → `icmp sle`
- `step == -1 && inclusive == 0` → descending exclusive → `icmp sgt`
- `step == -1 && inclusive == 1` → descending inclusive → `icmp sge`

**When step is a variable (expression, not literal):**
For `for i in 0..n by k` where `k` is a runtime variable, we cannot specialize at compile time. The general 8-instruction check is required. However, if the step variable can be proven positive (e.g., `k` comes from `abs(x)` or is a parameter with a `pre(k > 0)` contract), a future enhancement could specialize to `ascending` check only. This is OUT OF SCOPE for this section — document as a potential follow-up.

**When bounds are computed expressions:**
The specialization only depends on `step` and `inclusive`, NOT on the bound values (`start`, `end`). So `for i in f()..g()` with implicit step 1 IS specializable — the bounds are variables but the step is a literal 1.

### Implementation in `for_range.rs`

The specialization should be implemented as a conditional branch in `lower_for_range`:

```rust
// After projecting step and inclusive, check if both are literals.
let step_lit = self.builder.get_literal_int(step_var);
let incl_lit = self.builder.get_literal_int(incl_var);

match (step_lit, incl_lit) {
    (Some(1), Some(0)) => {
        // Ascending exclusive: emit only `i < end`
        // Spec: Clause 14.6 — range `start..end` with implicit step 1
        let in_bounds = self.builder.emit_let(Idx::BOOL, PrimOp::Binary(Lt), [i_var, end]);
    }
    (Some(1), Some(1)) => {
        // Ascending inclusive: emit only `i <= end`
        // Spec: Clause 14.6 — range `start..=end` with implicit step 1
        let in_bounds = self.builder.emit_let(Idx::BOOL, PrimOp::Binary(LtEq), [i_var, end]);
    }
    (Some(-1), Some(0)) => {
        // Descending exclusive: emit only `i > end`
        let in_bounds = self.builder.emit_let(Idx::BOOL, PrimOp::Binary(Gt), [i_var, end]);
    }
    (Some(-1), Some(1)) => {
        // Descending inclusive: emit only `i >= end`
        let in_bounds = self.builder.emit_let(Idx::BOOL, PrimOp::Binary(GtEq), [i_var, end]);
    }
    _ => {
        // General case: emit all 8 instructions (current behavior)
    }
}
```

This requires adding a `get_literal_int(var: ArcVarId) -> Option<i64>` method to `ArcIrBuilder` that traces a variable back to its defining instruction and returns the literal value if it's a `Let { value: ArcValue::Literal(LitValue::Int(n)), .. }`.

**Feasibility confirmed:** `ArcIrBuilder` owns `blocks: Vec<BlockBuilder>`, and each `BlockBuilder` has `body: Vec<ArcInstr>`. The method scans all block bodies for an `ArcInstr::Let { dst, value: ArcValue::Literal(LitValue::Int(n)) }` where `dst == var`. The `LitValue::Int(i64)` variant already exists in `compiler/ori_arc/src/ir/mod.rs`. This is a simple linear scan — O(total instructions), acceptable since it's called at most 2 times per `lower_for_range` invocation (for `step` and `inclusive`).

### Latch Specialization (Bonus)

If the step is a known literal, the latch can also be simplified. Currently the latch computes `next = i + step` where `step` is projected from the range. For `step == 1`, this could be `next = i + 1` with a literal operand. However, the ARC IR already has this — `step` traces back to a literal. The LLVM emission of `checked_add(i, step)` will constant-fold the step operand. This is already handled correctly and does NOT need specialization. The latch is not the bottleneck — the header condition is.

### Interaction with 08.2 (Loop-Invariant Block Params)

Range specialization changes the header block's instruction count (from 8 to 1 for common cases) but does NOT change the block structure or block param layout. The header block still has the same params (`i_var` + mutable bindings). The invariant-param elimination from 08.2 operates on block params, not on instructions within the header. Therefore, 08.2 and 08.3 are independent — implementing them in either order is safe. However, implementing 08.3 first makes the header block cleaner, which makes manual inspection of 08.2's results easier.

- [x] **1.** Add `get_literal_int(var: ArcVarId) -> Option<i64>` helper to `ArcIrBuilder` in `compiler/ori_arc/src/lower/builder/mod.rs` (traces var back to literal definition, including through Project→Construct chains). Unit tests in `compiler/ori_arc/src/lower/builder/tests.rs` (8 tests).
- [x] **2.** Detect common range patterns at the ARC IR level by checking if `step` and `inclusive` projections trace back to literals via `get_literal_int`
- [x] **3.** Emit specialized bounds checks for detected patterns (see implementation structure above)
- [x] **4.** Keep the general path as fallback for non-literal step/inclusive (extracted to `emit_general_range_condition`)
- [x] **5.** Verify: J7 `_ori_sum_for` with `1..=n by 1` emits `icmp sle` (not 8 instructions)
- [x] **6.** Verify: `for i in 0..n` (no explicit step) emits `icmp slt` (step defaults to literal 1) — IR quality test `test_range_ascending_exclusive_single_icmp`
- [x] **7.** Verify: general ranges (e.g., `0..100 by 3`) still work correctly — spec test `general_step`, AOT test `test_for_range_with_step_ascending`
- [x] **8.** Verify: `for i in 0..n by k` (variable step) still uses general 8-instruction check — IR quality test `test_range_variable_step_general_condition`
- [x] **9.** Add coverage matrix for ascending/descending x inclusive/exclusive x positive/negative/unit/non-unit step — spec tests in `range_edge_cases.ori` + IR quality tests
- [x] **10.** Spec test: `for i in 0..0` (empty range, exclusive) — zero iterations
- [x] **11.** Spec test: `for i in 0..=0` (single element, inclusive) — one iteration
- [x] **12.** Spec test: `for i in 0..=9223372036854775806` (near-INT_MAX inclusive bound) — no overflow in condition (avoids latch overflow at INT_MAX)
- [x] **13.** Spec test: `for i in 9223372036854775807..9223372036854775800 by -1` (descending from INT_MAX) — correct iteration
- [x] **14.** Spec test: `for i in 0..n by 0` — panics with "range step cannot be zero" (guard in both evaluator and ARC/LLVM paths)

### 08.3 Completion Checklist

- [x] Common range patterns (`start..end`, `start..=end`, `start..end by -1`, `start..=end by -1`) emit 1-instruction bounds check when step/inclusive are literals
- [x] Arbitrary-start ranges (`start..end` with non-zero start) also specialize (specialization depends on step/inclusive, not on start)
- [x] General ranges (non-literal step) still emit correct (if verbose) bounds checks
- [x] J7 `_ori_sum_for` with `1..=n by 1` emits `icmp sle` (1 instruction, not 8)
- [x] IR test: `for i in 0..n` emits `icmp slt` only
- [x] IR test: `for i in n..0 by -1` emits `icmp sgt` only
- [x] Coverage: ascending/descending × inclusive/exclusive × unit/non-unit step all pass
- [x] Zero-step panic guard preserved for all paths (not optimized away by specialization) — guard skipped only when step is a known non-zero literal
- [x] `get_literal_int()` helper added to `ArcIrBuilder` with unit tests (8 tests)
- [x] `./test-all.sh` green (only pre-existing formatter failures)
- [x] `./clippy-all.sh` green
- [x] No regressions in `cargo test -p ori_llvm` (all 17 for-loop AOT tests pass)

---

## Test Locations

All tests must follow sibling `tests.rs` conventions and be placed in the correct file for their scope:

| Test Type | File | Convention |
|-----------|------|------------|
| ARC IR unit tests (08.2 invariant param, 08.3 `get_literal_int`) | `compiler/ori_arc/src/block_merge/tests.rs` (08.2), `compiler/ori_arc/src/lower/builder/tests.rs` (08.3 helper) | Sibling `tests.rs` |
| ARC IR control flow tests (08.3 range specialization) | `compiler/ori_arc/src/lower/control_flow/tests.rs` | Existing test file |
| LLVM IR quality tests (all subsections) | `compiler/ori_llvm/tests/aot/ir_quality.rs` | Existing integration test file |
| LLVM `IrBuilder` CSE cache tests (08.1) | `compiler/ori_llvm/src/codegen/ir_builder/tests.rs` (if sibling exists) or within `checked_ops.rs` `#[cfg(test)]` block | Sibling `tests.rs` preferred |
| Spec tests (08.3 edge cases) | `tests/spec/control_flow/for/` | `.ori` spec tests |
| ARC emitter tests (end-to-end loop emission) | `compiler/ori_llvm/src/codegen/arc_emitter/tests.rs` | Existing test file |

**Note:** `compiler/ori_arc/src/block_merge/tests.rs` already exists (2623 lines) and contains the existing block merge tests. New invariant-param tests for 08.2 should go there.

---

## Cross-Section Interactions

### §01 (Block Merging) + §08 (Loop IR Quality)

**Block merging inside loop bodies:** The block merge pass (§01) runs after ARC lowering but before LLVM emission. It merges sequential blocks within loop bodies just like any other blocks. This is orthogonal to §08's concerns:
- **CSE (08.1):** Block merging reduces the number of blocks but does not change instruction count within a block. Two blocks that each compute `i+1` would be merged into one block with both computations — making them MORE amenable to CSE (now in the same block).
- **Invariant block params (08.2):** Block merging does not add or remove block params. It only merges single-predecessor Jump chains. Loop headers have multiple predecessors (entry + back-edge) and are never merged. The invariant-param pass is independent.
- **Range specialization (08.3):** Block merging does not affect the header's condition instructions. The header block is a multi-predecessor block and is never the target of a merge.

**Conclusion:** No conflict. §01 is complete and §08 can proceed safely.

### §06 (Dead Code Pruning) + §08 (Loop IR Quality)

**Dead code after break/continue inside loops:** `break` and `continue` are lowered to `Jump` terminators in the ARC IR. Any instructions after the `Jump` in the same block are dead. The ARC lowerer already handles this — `self.builder.is_terminated()` checks prevent emitting instructions after terminators. §06.2's noreturn pruning handles `panic()` calls inside loop bodies (e.g., `if cond then panic(msg: "err")` — the panic arm gets `unreachable`, the else arm continues). No conflict.

**Dead field loads in loop bodies:** §06.1's surgical field loading operates on function parameters, not on values inside loop bodies. Loop iteration variables are scalars (integers for ranges, elements for iterators). No interaction.

### §07 (Constant Dedup) + §08 (Loop IR Quality)

The checked arithmetic in loop bodies (CSE target in 08.1) generates overflow panic messages via `build_global_string_ptr`. §07's deduplication cache ensures these messages are shared across all overflow sites. If 08.1's CSE eliminates a redundant checked_add, it also eliminates a redundant panic message reference — but the global string still exists (referenced by the surviving checked_add). No conflict; they compose well.

### §09 (Tail Calls) + §08 (Loop IR Quality)

Tail call optimization converts recursive calls to loops. The resulting loop has the same structure as a `lower_loop` loop (header block with params, body, back-edge). §08.2's invariant-param elimination and §08.1's CSE apply equally to TCO-generated loops and source-level loops. However, §08 should be implemented BEFORE §09 to ensure the loop optimization infrastructure is tested on simpler source-level loops before being applied to TCO-generated loops.

---

## Section 08 Exit Criteria

J7 loop body has exactly one `i + 1` computation. J10 loop header has no invariant block params (and no invariant phi nodes in the resulting LLVM IR). J7 range loop uses `icmp sle` for `1..=n by 1`. All loop-related tests pass. Loop-heavy programs show measurable instruction count reduction.

**Measurement criteria:**
- J7 `_ori_sum_loop`: count `@llvm.sadd.with.overflow.i64` calls in the loop body. Before: 2+. After: 1. Verify with `ORI_DUMP_AFTER_LLVM=1`.
- J7 `_ori_sum_for`: count instructions in the header block. Before: 8 condition instructions. After: 1 (`icmp sle` for `1..=n`). Verify with `ORI_DUMP_AFTER_LLVM=1`.
- J10 `_ori_check_iteration`: count LLVM phi nodes in the loop header. Before: includes invariant phi for list struct. After: no invariant phi nodes. Verify with `ORI_DUMP_AFTER_LLVM=1`.

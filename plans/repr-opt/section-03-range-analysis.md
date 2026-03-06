---
section: "03"
title: "Value Range Analysis Framework"
status: not-started
goal: "Build an abstract interpretation engine over integer intervals that computes provable value ranges for every int-typed expression in a function"
inspired_by:
  - "Roc NumericRange constraint system (crates/compiler/types/src/num.rs)"
  - "LLVM CorrelatedValuePropagation (lib/Transforms/Scalar/CorrelatedValuePropagation.cpp)"
  - "LLVM LazyValueInfo (lib/Analysis/LazyValueInfo.cpp)"
  - "GCC VRP (tree-vrp.cc)"
depends_on: ["01"]
sections:
  - id: "03.1"
    title: "Interval Lattice"
    status: not-started
  - id: "03.2"
    title: "Transfer Functions"
    status: not-started
  - id: "03.3"
    title: "Widening & Narrowing Operators"
    status: not-started
  - id: "03.4"
    title: "Conditional Range Refinement"
    status: not-started
  - id: "03.5"
    title: "Function Signature Range Propagation"
    status: not-started
  - id: "03.6"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Value Range Analysis Framework

**Context:** Value range propagation (VRP) is one of the most well-studied analyses in compiler optimization. LLVM has `CorrelatedValuePropagation` and `LazyValueInfo`; GCC has `tree-vrp`. However, doing it at the Ori level (before LLVM) has two advantages:
1. We can optimize **struct layouts** and **ARC headers** based on ranges — LLVM can't see through these
2. We can narrow **function parameter types** across module boundaries — LLVM's VRP is per-function

**Reference implementations:**
- **LLVM** `lib/Analysis/LazyValueInfo.cpp`: Demand-driven range computation with caching
- **LLVM** `lib/Transforms/Scalar/CorrelatedValuePropagation.cpp`: Uses LazyValueInfo to replace comparisons, narrow truncations
- **GCC** `tree-vrp.cc`: Forward propagation with back-edge widening
- **Roc** `crates/compiler/types/src/num.rs`: `NumericRange` — compile-time constraint intersection

**Depends on:** §01 (ranges stored in ReprPlan).

**Risk warning:** Abstract interpretation with widening/narrowing is the most complex analysis in this plan. Transfer functions for multiplication and division have subtle corner cases (signed overflow, division by ranges spanning zero). Implement §03.1 (lattice) and §03.2 (transfer functions) first with property-based tests (e.g., `proptest`). Only then add §03.3 (widening/narrowing). Start with conservative (Top-returning) transfer functions and tighten incrementally.

**Crate dependency:** Range analysis operates on `ArcFunction` (from `ori_arc::ir`). This means `ori_repr` depends on `ori_arc`. The dependency chain is `ori_types → ori_arc → ori_repr → ori_llvm`. This is correct: `ori_repr` reads from `ori_arc` IR but `ori_arc` does NOT depend on `ori_repr` (no cycle). The lattice types (`ValueRange`, `IntWidth`) live in `ori_repr` and do NOT reference `ori_arc` — only `fixpoint.rs` (which takes `&ArcFunction`) requires the `ori_arc` dependency.

**Visibility prerequisite:** `compute_postorder()` in `ori_arc::graph` is currently `pub(crate)`. It must be made `pub` (or a `pub` wrapper added) so `ori_repr` can compute RPO over `ArcFunction` blocks. This is a one-line visibility change in `ori_arc/src/graph/mod.rs`. Similarly, `successor_block_ids()` is `pub(crate)` and must be made `pub` for `ori_repr` to build the predecessor map.

**File organization:** 5 files in `compiler/ori_repr/src/range/` submodule — `mod.rs` (lattice + re-exports), `transfer.rs`, `fixpoint.rs`, `conditional.rs`, `signatures.rs`, plus `tests.rs` (sibling test convention, at `range/tests.rs` per `mod.rs` → sibling convention).

**File size warning:** `transfer.rs` is the highest-risk file for exceeding the 500-line limit. It contains: the top-level `transfer()` dispatcher (~60 lines), `transfer_primop()` dispatcher for 23 `BinaryOp` + 4 `UnaryOp` variants (~80 lines), individual transfer functions for arithmetic (add/sub/mul/div/mod/floordiv/neg — ~120 lines), bitwise operations (~80 lines), built-in function ranges (len/count/byte_to_int/char_to_int/abs — ~40 lines), and the `is_int_typed()` helper. Total estimate: ~400-500 lines. If it exceeds 500 during implementation, split into `transfer/mod.rs` (dispatcher), `transfer/arithmetic.rs`, and `transfer/bitwise.rs`.

**Documentation requirement:** All `pub` types and functions must have `///` doc comments. Each file must have a `//!` module-level doc comment. This applies to `ValueRange`, `IntWidth`, all transfer functions, the fixpoint driver, and the conditional refinement API.

---

## 03.1 Interval Lattice

**File(s):** `compiler/ori_repr/src/range/mod.rs` (was `range.rs` — moved to submodule)

The interval lattice is the core data structure. Each element represents a set of possible integer values.

- [ ] Define the `ValueRange` lattice:
  ```rust
  /// A closed interval [lo, hi] over i64 values.
  /// Invariant: lo <= hi (empty represented as Bottom).
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum ValueRange {
      /// No possible values (unreachable code)
      Bottom,
      /// Exactly the values in [lo, hi]
      Bounded { lo: i64, hi: i64 },
      /// All possible i64 values (analysis gave up)
      Top,
  }
  ```

- [ ] Implement lattice operations:
  ```rust
  impl ValueRange {
      /// Lattice join (union of possible values)
      pub fn join(self, other: Self) -> Self { ... }

      /// Lattice meet (intersection of possible values)
      pub fn meet(self, other: Self) -> Self { ... }

      /// Does this range fit in the given integer width?
      pub fn fits_in(&self, width: IntWidth) -> bool { ... }

      /// Minimum width needed to represent this range
      pub fn min_width(&self) -> IntWidth { ... }

      /// Is this a constant (single value)?
      pub fn is_constant(&self) -> Option<i64> { ... }

      /// Does this range overlap with another?
      pub fn overlaps(&self, other: &Self) -> bool { ... }

  }
  ```
  **Note:** `widen` and `narrow` are defined as free functions in §03.3 (`fixpoint.rs`), not as methods on `ValueRange`. The lattice module (`mod.rs`) only defines join/meet/fits_in/min_width/is_constant/overlaps.

- [ ] Implement width-specific range constants:
  ```rust
  impl IntWidth {
      pub fn signed_range(self) -> ValueRange {
          match self {
              IntWidth::I8  => ValueRange::Bounded { lo: -128, hi: 127 },
              IntWidth::I16 => ValueRange::Bounded { lo: -32768, hi: 32767 },
              IntWidth::I32 => ValueRange::Bounded { lo: -2_147_483_648, hi: 2_147_483_647 },
              IntWidth::I64 => ValueRange::Top,
          }
      }

      pub fn unsigned_range(self) -> ValueRange {
          match self {
              IntWidth::I8  => ValueRange::Bounded { lo: 0, hi: 255 },
              IntWidth::I16 => ValueRange::Bounded { lo: 0, hi: 65535 },
              IntWidth::I32 => ValueRange::Bounded { lo: 0, hi: 4_294_967_295 },
              IntWidth::I64 => ValueRange::Top,
          }
      }
  }
  ```

- [ ] Comprehensive unit tests for lattice operations (join, meet, fits_in, widen, narrow)
- [ ] Import and use `tracing` crate (never `println!`/`eprintln!`). All diagnostic output through `tracing::debug!`/`tracing::trace!` with target `ori_repr`.

---

## 03.2 Transfer Functions

**File(s):** `compiler/ori_repr/src/range/transfer.rs`

Transfer functions describe how each operation transforms value ranges.

- [ ] Arithmetic operations:
  ```rust
  pub fn range_add(a: ValueRange, b: ValueRange) -> ValueRange {
      match (a, b) {
          // Bottom propagates — if either input is unreachable, output is too.
          (Bottom, _) | (_, Bottom) => Bottom,
          (Bounded { lo: a_lo, hi: a_hi }, Bounded { lo: b_lo, hi: b_hi }) => {
              let lo = a_lo.checked_add(b_lo);
              let hi = a_hi.checked_add(b_hi);
              match (lo, hi) {
                  (Some(lo), Some(hi)) => Bounded { lo, hi },
                  _ => Top, // overflow possible → give up
              }
          }
          _ => Top, // Top + anything = Top
      }
  }

  pub fn range_sub(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_mul(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_div(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_mod(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_neg(a: ValueRange) -> ValueRange { ... }
  ```

- [ ] Bitwise operations:
  ```rust
  pub fn range_bitand(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_bitor(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_bitxor(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_shl(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_shr(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  ```

- [ ] Built-in function ranges:
  ```rust
  /// len() always returns >= 0
  pub fn range_len() -> ValueRange { Bounded { lo: 0, hi: i64::MAX } }

  /// count() always returns >= 0
  pub fn range_count() -> ValueRange { Bounded { lo: 0, hi: i64::MAX } }

  /// byte values: [0, 255]
  pub fn range_byte_to_int() -> ValueRange { Bounded { lo: 0, hi: 255 } }

  /// char codepoints: [0, 0x10FFFF]
  pub fn range_char_to_int() -> ValueRange { Bounded { lo: 0, hi: 0x10FFFF } }

  /// abs() on int: non-negative (but i64::MIN.abs() overflows — return Top if lo == i64::MIN)
  pub fn range_abs(a: ValueRange) -> ValueRange { ... }
  ```

- [ ] Literal ranges:
  ```rust
  /// Integer literal has an exact range
  pub fn range_literal(value: i64) -> ValueRange {
      Bounded { lo: value, hi: value }
  }
  ```

- [ ] Remaining arithmetic operations (from `BinaryOp`):
  - `FloorDiv` — integer floor division: `a / b` rounded toward negative infinity. Same division-by-zero handling as `range_div`; result range differs from truncating division when signs differ.
  - `MatMul` (`@`) — user-defined operator: returns Top (cannot reason about custom implementations).

- [ ] Comparison operations (produce `bool`, not `int` — range is `[0, 1]`):
  - `Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq` — all produce `ValueRange::Bounded { lo: 0, hi: 1 }` (boolean).
  - Comparison results are primarily useful via §03.4 conditional refinement, not directly.

- [ ] Logical operations (produce `bool`):
  - `And` (`&&`), `Or` (`||`) — produce `ValueRange::Bounded { lo: 0, hi: 1 }`.

- [ ] Range/coalesce operations:
  - `Range` (`..`), `RangeInclusive` (`..=`) — produce a Range value, not an int. Return Top for the `dst` variable (range analysis tracks int-typed variables only).
  - `Coalesce` (`??`) — unwraps Option; return Top (value depends on Option contents).

- [ ] Unary operations (from `UnaryOp`):
  - `Neg` — already listed as `range_neg`.
  - `Not` (`!`) — logical not on bool: returns `[0, 1]`.
  - `BitNot` (`~`) — bitwise complement: if `a ∈ [lo, hi]` and both non-negative, result is `[-hi-1, -lo-1]`. Conservative: return Top for mixed-sign ranges.
  - `Try` (`?`) — desugared before ARC IR; should not appear. If encountered, return Top.

- [ ] **Top-level transfer function dispatcher** — maps each `ArcInstr` variant to a range:
  ```rust
  /// Compute the output range for a single ArcInstr.
  /// Returns Top for non-int-typed destinations or unsupported patterns.
  pub fn transfer(
      instr: &ArcInstr,
      ranges: &FxHashMap<ArcVarId, ValueRange>,
      pool: &Pool,
  ) -> ValueRange {
      match instr {
          // --- Value-producing instructions ---
          ArcInstr::Let { ty, value, .. } => {
              if !is_int_typed(*ty, pool) { return Top; }
              match value {
                  ArcValue::Literal(LitValue::Int(n)) => range_literal(*n),
                  ArcValue::Var(v) => ranges.get(v).copied().unwrap_or(Top),
                  ArcValue::PrimOp { op, args } => transfer_primop(*op, args, ranges, pool),
                  _ => Top, // non-int literal (float, bool, string, etc.)
              }
          }

          // Function calls: return Top (callee return range unknown
          // until §03.5 function signature propagation is implemented).
          ArcInstr::Apply { ty, func, .. } => {
              if !is_int_typed(*ty, pool) { return Top; }
              // Check for known built-in functions (len, count, etc.)
              transfer_known_call(*func, pool).unwrap_or(Top)
          }

          // Indirect calls: always Top (unknown callee).
          ArcInstr::ApplyIndirect { .. } => Top,

          // Partial application: produces closure, not int. Always Top.
          ArcInstr::PartialApply { .. } => Top,

          // Field projection: Top unless we track per-field ranges
          // (future enhancement — could propagate struct field ranges).
          ArcInstr::Project { ty, .. } => {
              if !is_int_typed(*ty, pool) { return Top; }
              Top
          }

          // Construction: produces composite value, not int. Always Top.
          ArcInstr::Construct { .. } => Top,

          // --- RC operations (no dst — never produce a value) ---
          ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. } => Top,

          // IsShared: produces bool [0, 1].
          ArcInstr::IsShared { .. } => Bounded { lo: 0, hi: 1 },

          // --- Mutation operations (no dst) ---
          ArcInstr::Set { .. } | ArcInstr::SetTag { .. } => Top,

          // Reset: produces reuse token, not int. Always Top.
          ArcInstr::Reset { .. } => Top,

          // Reuse / CollectionReuse: produce composite values. Always Top.
          ArcInstr::Reuse { .. } | ArcInstr::CollectionReuse { .. } => Top,

          // Select: propagate range as join of both branches.
          ArcInstr::Select { ty, true_val, false_val, .. } => {
              if !is_int_typed(*ty, pool) { return Top; }
              let t = ranges.get(true_val).copied().unwrap_or(Top);
              let f = ranges.get(false_val).copied().unwrap_or(Top);
              t.join(f)
          }
      }
  }
  ```
  This dispatcher ensures every `ArcInstr` variant has a defined behavior. Instructions that do not define a variable (`RcInc`, `RcDec`, `Set`, `SetTag`) are handled by the caller: `instr.defined_var()` returns `None`, so the fixpoint loop skips them.

---

## 03.3 Widening & Narrowing Operators

**File(s):** `compiler/ori_repr/src/range/fixpoint.rs`

**Implementation order:** Implement §03.4 (conditional refinement) BEFORE the fixpoint loop in this section. The fixpoint loop calls `refine_from_branch()` from §03.4 when processing `Branch`/`Switch` terminators. Without §03.4, the fixpoint loop cannot refine ranges at branch points, making loop counter narrowing (the primary use case) incomplete. The recommended build order is: 03.1 → 03.2 → 03.4 → 03.3 → 03.5.

**Complexity warning:** This is the highest-risk subsection. The fixpoint loop must correctly handle: (1) block parameter merging (phi-like), (2) terminator-driven refinement, (3) widening threshold tuning, (4) narrowing pass, (5) `ArcTerminator::Invoke` which defines a variable. Getting any of these wrong produces silent unsoundness (ranges too narrow) or uselessness (all Top). Budget extra time for testing.

For loops and recursive functions, naive fixed-point iteration may not terminate. Widening accelerates convergence; narrowing recovers precision after widening.

- [ ] Implement widening operator:
  ```rust
  /// Standard widening: if bound grew, push to infinity
  pub fn widen(previous: ValueRange, current: ValueRange) -> ValueRange {
      match (previous, current) {
          (Bottom, x) => x,
          (_, Bottom) => Bottom,
          (Top, _) | (_, Top) => Top,
          (Bounded { lo: p_lo, hi: p_hi }, Bounded { lo: c_lo, hi: c_hi }) => {
              let new_lo = if c_lo < p_lo { i64::MIN } else { c_lo };
              let new_hi = if c_hi > p_hi { i64::MAX } else { c_hi };
              if new_lo == i64::MIN && new_hi == i64::MAX { Top }
              else { Bounded { lo: new_lo, hi: new_hi } }
          }
      }
  }
  ```

- [ ] Implement narrowing operator (post-widening precision recovery):
  ```rust
  /// Narrowing: intersect widened result with transfer function output
  pub fn narrow(widened: ValueRange, computed: ValueRange) -> ValueRange {
      widened.meet(computed)
  }
  ```

- [ ] **IR choice:** Range analysis operates on `ArcFunction` (from `ori_arc::ir`), NOT `CanExpr`:
  - `ArcFunction` has basic blocks (`ArcBlock`) and SSA-like variables (`ArcVarId`); dominator trees are computed separately via `DominatorTree::build(func)` in `ori_arc/src/graph/dominator.rs`
  - `CanExpr` (in `ori_ir::canon::expr`) is a sugar-free canonical expression enum with no explicit control flow graph — unsuitable for dataflow analysis
  - This means range analysis runs AFTER ARC lowering but BEFORE LLVM codegen
  - The `ArcFunction` → range analysis → ReprPlan → LLVM codegen flow preserves phase ordering

- [ ] **Block parameter merging (phi handling):** ARC IR uses block parameters instead of phi nodes. `ArcBlock::params` is `Vec<(ArcVarId, Idx)>` — values passed via `Jump { target, args }`. At CFG merge points, the range for a block parameter must be the **join** of the ranges of all incoming arguments across all predecessor `Jump` instructions. The fixpoint loop must process block parameters before block body instructions:
  ```rust
  // Pre-compute predecessor map ONCE before the fixpoint loop.
  // Uses `successor_block_ids()` from `ori_arc::graph` (must be made `pub`).
  fn build_predecessor_map(func: &ArcFunction) -> FxHashMap<usize, Vec<usize>> {
      let mut preds: FxHashMap<usize, Vec<usize>> = FxHashMap::default();
      for (idx, block) in func.blocks.iter().enumerate() {
          for succ in successor_block_ids(&block.terminator) {
              preds.entry(succ.index()).or_default().push(idx);
          }
      }
      preds
  }

  // Then in the fixpoint loop, for each block, before processing body:
  for (param_idx, (param_var, _param_ty)) in block.params.iter().enumerate() {
      let mut merged = Bottom;
      for &pred_idx in predecessors.get(&block_idx).unwrap_or(&vec![]) {
          let pred = &func.blocks[pred_idx];
          if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
              if target.index() == block_idx {
                  if let Some(&arg_var) = args.get(param_idx) {
                      let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                      merged = merged.join(arg_range);
                  }
              }
          }
          // Branch does not pass args — only control flow
          // Invoke normal successor may pass args — check if applicable
      }
      // Update param range (with widening if iteration > threshold)
  }
  ```
  **Important:** Without this, loop induction variables (which are block parameters on loop headers) will never get non-Bottom ranges, making loop counter narrowing impossible. This is the most critical gap — `for i in 0..100` lowers to a loop with `i` as a block parameter.

  **Performance note:** The predecessor map (`build_predecessor_map`) must be built ONCE before the fixpoint loop, not recomputed per iteration. The naive approach of scanning all blocks per parameter is O(blocks x params) per iteration — with 500 blocks and 20 iterations, that's 10,000 full-scan passes.

- [ ] **Terminator-driven refinement:** The fixpoint loop must also process block terminators, not just body instructions. Three concerns:
  1. **`Invoke { dst, ty, func, args, .. }`**: This terminator DEFINES a variable (`dst`). It is functionally equivalent to `Apply` but with unwind semantics. The fixpoint loop must compute a range for `dst` (same logic as `Apply` — check for known built-in, otherwise Top).
  2. **`Branch { cond, then_block, else_block }`**: Apply conditional refinement (§03.4) to variables in `then_block` and `else_block`.
  3. **`Switch { scrutinee, cases, default }`**: The scrutinee has range `[case_val, case_val]` in each case block, and the complement range in the default block. **Note:** `Switch` cases are `Vec<(u64, ArcBlockId)>` — the case values are `u64`, not `i64`. Use `i64::try_from(case_val)` and skip refinement for values exceeding `i64::MAX`.
  - Store per-block incoming refinements in a side table: `FxHashMap<(ArcBlockId, ArcVarId), ValueRange>`. Apply these at the start of each block during the next iteration.

- [ ] Implement fixed-point iteration with widening:
  ```rust
  // NOTE: ArcFunction does not have a blocks_in_rpo() method.
  // Compute RPO via compute_postorder() from ori_arc::graph and reverse it.
  // ArcBlock fields are accessed directly: block.body (Vec<ArcInstr>),
  // block.terminator (ArcTerminator). ArcInstr::defined_var() returns
  // Option<ArcVarId> (not all instructions define a variable).
  //
  // IMPORTANT: ArcTerminator::Invoke also defines a variable (dst).
  // The fixpoint loop must process terminators, not just body instructions.

  /// Widening threshold — start widening after this many iterations.
  /// Named constant, not a magic number.
  const WIDEN_THRESHOLD: usize = 3;

  pub fn range_fixpoint(
      func: &ArcFunction,
      pool: &Pool,
      config: &RangeAnalysisConfig,
  ) -> FxHashMap<ArcVarId, ValueRange> {
      // Budget check: skip analysis for very large functions
      if func.blocks.len() > config.max_blocks {
          tracing::warn!(
              func = %func.name,
              blocks = func.blocks.len(),
              "skipping range analysis — function too large"
          );
          return FxHashMap::default();
      }

      let mut ranges: FxHashMap<ArcVarId, ValueRange> = FxHashMap::default();
      let mut iteration = 0;

      // Compute reverse postorder (RPO) block indices.
      // compute_rpo wraps compute_postorder() + reverse().
      let rpo = compute_rpo(func);

      // Pre-compute predecessor map: block_idx → Vec<(pred_block_idx)>
      // Avoids O(blocks²) scan per block during parameter merging.
      let predecessors = build_predecessor_map(func);

      // Per-block incoming refinements from Branch/Switch terminators (§03.4)
      let mut block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange> =
          FxHashMap::default();

      loop {
          let mut changed = false;
          for &block_idx in &rpo {
              let block = &func.blocks[block_idx];

              // Step 1: Process block parameters (phi-like merging)
              // See "Block parameter merging" bullet above.
              for (param_idx, (param_var, _param_ty)) in block.params.iter().enumerate() {
                  let mut merged = Bottom;
                  for &pred_idx in predecessors.get(&block_idx).unwrap_or(&vec![]) {
                      let pred = &func.blocks[pred_idx];
                      if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                          if target.index() == block_idx {
                              if let Some(&arg_var) = args.get(param_idx) {
                                  let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                                  merged = merged.join(arg_range);
                              }
                          }
                      }
                      // Invoke normal successor also passes args — handle if needed
                  }
                  // Apply any conditional refinements from Branch/Switch
                  if let Some(&refinement) = block_refinements.get(&(block.id, *param_var)) {
                      merged = merged.meet(refinement);
                  }
                  let old = ranges.get(param_var).copied().unwrap_or(Bottom);
                  let final_range = if iteration > WIDEN_THRESHOLD {
                      widen(old, old.join(merged))
                  } else {
                      old.join(merged)
                  };
                  if final_range != old {
                      ranges.insert(*param_var, final_range);
                      changed = true;
                  }
              }

              // Step 2: Process body instructions
              for instr in &block.body {
                  let new_range = transfer(instr, &ranges, pool);
                  let Some(var) = instr.defined_var() else { continue };
                  let old = ranges.get(&var).copied().unwrap_or(Bottom);

                  let merged = if iteration > WIDEN_THRESHOLD {
                      widen(old, old.join(new_range))
                  } else {
                      old.join(new_range)
                  };

                  if merged != old {
                      ranges.insert(var, merged);
                      changed = true;
                  }
              }

              // Step 3: Process terminator
              // Invoke defines a variable; Branch/Switch provide refinements.
              match &block.terminator {
                  ArcTerminator::Invoke { dst, ty, func: callee, .. } => {
                      if is_int_typed(*ty, pool) {
                          let new_range = transfer_known_call(*callee, pool)
                              .unwrap_or(Top);
                          let old = ranges.get(dst).copied().unwrap_or(Bottom);
                          let merged = if iteration > WIDEN_THRESHOLD {
                              widen(old, old.join(new_range))
                          } else {
                              old.join(new_range)
                          };
                          if merged != old {
                              ranges.insert(*dst, merged);
                              changed = true;
                          }
                      }
                  }
                  ArcTerminator::Branch { cond, then_block, else_block } => {
                      // §03.4: extract conditional refinements for successors
                      let refinements = refine_from_branch(*cond, &ranges, &block.body);
                      for r in &refinements {
                          block_refinements.insert((*then_block, r.var), r.true_range);
                          block_refinements.insert((*else_block, r.var), r.false_range);
                      }
                  }
                  ArcTerminator::Switch { scrutinee, cases, default } => {
                      // Each case block: scrutinee == case_val
                      for &(case_val, case_block) in cases {
                          // u64 → i64 conversion (Switch cases are u64)
                          if let Ok(val) = i64::try_from(case_val) {
                              block_refinements.insert(
                                  (case_block, *scrutinee),
                                  Bounded { lo: val, hi: val },
                              );
                          }
                      }
                      // Default block: scrutinee is NOT any case value
                      // (complement range — complex, conservative: leave as-is)
                  }
                  // Exhaustive — no `_` arm. Each variant explicitly handled.
                  ArcTerminator::Return { .. } => {} // no variable defined, no refinement
                  ArcTerminator::Jump { .. } => {} // args handled in block parameter merging (Step 1)
                  ArcTerminator::Resume => {}
                  ArcTerminator::Unreachable => {}
              }
          }

          iteration += 1;
          if !changed || iteration >= config.max_iterations { break; }
      }

      tracing::debug!(
          func = %func.name,
          iterations = iteration,
          non_top = ranges.values().filter(|r| !matches!(r, Top)).count(),
          "range analysis complete"
      );

      // Narrowing pass (optional — one iteration to recover precision)
      for &block_idx in &rpo {
          let block = &func.blocks[block_idx];
          for instr in &block.body {
              let computed = transfer(instr, &ranges, pool);
              let Some(var) = instr.defined_var() else { continue };
              if let Some(&widened) = ranges.get(&var) {
                  let narrowed = narrow(widened, computed);
                  ranges.insert(var, narrowed);
              }
          }
      }

      ranges
  }
  ```

- [ ] **Handoff to ReprPlan (§01 integration):** `range_fixpoint()` returns `FxHashMap<ArcVarId, ValueRange>` — per-variable ranges within a single function. But `ReprPlan` is keyed by `Idx` (Pool type indices), not `ArcVarId`. The integration requires two steps:
  1. **Per-function range storage:** Add a field to `ReprPlan`:
     ```rust
     /// Per-function, per-variable ranges from range analysis.
     /// Key: (function Name, ArcVarId) → ValueRange.
     /// Populated by §03, consumed by §04 (integer narrowing).
     function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
     ```
  2. **Type-level range aggregation:** For struct field narrowing and collection element narrowing, §04 aggregates per-variable ranges into per-type ranges. For each `Idx` that is `Tag::Int`, the type-level range is the join of all variable ranges for variables of that type across all functions. This aggregation is §04's responsibility, not §03's — §03 only stores the raw per-variable results.
  3. **Query method:**
     ```rust
     impl ReprPlan {
         /// Get the range for a variable in a function (from §03 range analysis).
         pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange {
             self.function_var_ranges
                 .get(&func)
                 .and_then(|m| m.get(&var).copied())
                 .unwrap_or(ValueRange::Top)
         }
     }
     ```

---

## 03.4 Conditional Range Refinement

**File(s):** `compiler/ori_repr/src/range/conditional.rs`

When code branches on a comparison (e.g., `if x < 100`), the true branch knows `x ∈ [-2⁶³, 99]` and the false branch knows `x ∈ [100, 2⁶³-1]`. This is the most powerful source of narrowing information.

- [ ] Implement conditional range extraction:
  ```rust
  /// Refinement result for a single variable at a branch point.
  pub struct BranchRefinement {
      pub var: ArcVarId,
      pub true_range: ValueRange,
      pub false_range: ValueRange,
  }

  /// Extract range refinements from a Branch terminator's condition.
  ///
  /// `cond_var` is the ArcVarId from `Branch { cond, .. }`.
  /// `body` is the block's body instructions — we trace `cond_var` back
  /// to the ArcInstr that produced it (e.g., a PrimOp comparison).
  ///
  /// Returns refinements for variables that can be narrowed in each branch.
  pub fn refine_from_branch(
      cond_var: ArcVarId,
      ranges: &FxHashMap<ArcVarId, ValueRange>,
      body: &[ArcInstr],
  ) -> Vec<BranchRefinement> {
      // Find the instruction that defined cond_var
      let Some(def_instr) = body.iter().rev().find(|i| i.defined_var() == Some(cond_var))
      else {
          return vec![]; // cond defined in a predecessor — can't analyze locally
      };

      // Match pattern: cond = PrimOp(Lt, [x, y]) where y is a known constant
      match def_instr {
          ArcInstr::Let { value: ArcValue::PrimOp { op: PrimOp::Binary(BinaryOp::Lt), args }, .. }
              if args.len() == 2 =>
          {
              let x = args[0];
              let y = args[1];
              let x_range = ranges.get(&x).copied().unwrap_or(Top);
              // If y is a known constant, refine x
              if let Some(c) = ranges.get(&y).and_then(|r| r.is_constant()) {
                  let true_range = x_range.meet(Bounded { lo: i64::MIN, hi: c - 1 });
                  let false_range = x_range.meet(Bounded { lo: c, hi: i64::MAX });
                  return vec![BranchRefinement { var: x, true_range, false_range }];
              }
              vec![]
          }
          // x >= c, x <= c, x > c, x == c, x != c — similar patterns
          // ... (each comparison operator has its own refinement logic)
          _ => vec![], // can't extract info — return empty (safe)
      }
  }
  ```

- [ ] Implement refinement for all 6 comparison operators (`Lt`, `LtEq`, `Gt`, `GtEq`, `Eq`, `NotEq`):
  - `x < c` → true: `[lo, c-1]`, false: `[c, hi]`
  - `x <= c` → true: `[lo, c]`, false: `[c+1, hi]`
  - `x > c` → true: `[c+1, hi]`, false: `[lo, c]`
  - `x >= c` → true: `[c, hi]`, false: `[lo, c-1]` (common for non-negative checks: `x >= 0`)
  - `x == c` → true: `[c, c]`, false: current range minus `c` (conservative: keep current range)
  - `x != c` → true: current range (conservative), false: `[c, c]`
  - Each operator must handle `c - 1` / `c + 1` overflow (checked arithmetic; fallback to Top on overflow)

---

## 03.5 Function Signature Range Propagation

**File(s):** `compiler/ori_repr/src/range/signatures.rs`

**Complexity warning:** This subsection is the most complex part of §03 and can be DEFERRED to a later iteration. §03.1-§03.4 provide useful intraprocedural ranges (loop counters, literal propagation, conditional refinement) without any cross-function analysis. Implement §03.5 only after §03.1-§03.4 are stable and tested. The intraprocedural analysis alone enables §04 integer narrowing for local variables and loop counters — the highest-value use cases.

**Risk:** Interprocedural fixpoint over SCCs is quadratic in the worst case (SCC size x iterations x function size). The budget caps mitigate this, but testing with real programs is essential before merging.

For cross-function narrowing, we need to propagate range information through function signatures.

- [ ] Define `FunctionRangeInfo` (new type in `ori_repr`, NOT a modification of any existing type in `ori_types` or `ori_arc` — that would violate phase boundaries):
  ```rust
  pub struct ParamRange {
      /// Which parameter index
      pub param_index: usize,
      /// Inferred range for this parameter
      pub range: ValueRange,
  }

  pub struct FunctionRangeInfo {
      /// Ranges inferred for parameters (from all call sites)
      pub param_ranges: Vec<ParamRange>,
      /// Range of the return value
      pub return_range: ValueRange,
  }
  ```

- [ ] Implement call-site range collection:
  - At each call site, intersect the argument's range with the parameter's current range
  - After processing all call sites, the parameter range is the join of all argument ranges
  - This is a whole-module analysis (requires iterating to fixed point for recursive functions)

- [ ] **Recursive function fixpoint algorithm:**
  1. Build call graph using existing `CallGraph::build()` from `ori_arc::graph::call_graph`.
  2. Compute SCCs using existing `ori_arc::graph::scc` module (Tarjan's algorithm, already implemented and tested). Do NOT reimplement Tarjan — reuse the existing infrastructure. Each SCC is a set of mutually-recursive functions.
  3. Process SCCs in reverse topological order (leaves first). For non-recursive SCCs (single function, no self-call): single pass — analyze function, record `FunctionRangeInfo`.
  4. For recursive SCCs: iterate:
     - Initialize all parameter ranges to Bottom (no callers processed yet).
     - Run intraprocedural `range_fixpoint()` on each function in the SCC.
     - At each `Apply` instruction targeting a function in the SCC, join the argument range into the callee's parameter range.
     - Repeat until parameter ranges stabilize or `max_scc_iterations` (default: 10) reached.
     - If not converged, widen all parameter ranges to Top (safe fallback).
  5. **Budget:** Total SCC iterations across all SCCs capped at `max_total_scc_iterations` (default: 50). If exceeded, remaining SCCs get Top for all parameter ranges.

- [ ] Handle boundary cases for parameter ranges:
  - `@main(args:)`: the `args` list length is `[0, i64::MAX]`; the `args` parameter itself is not an int (skip)
  - Trait method parameters: assign Top (callers unknown at compile time — may be called via dynamic dispatch)
  - Closure parameters: assign Top unless all call sites of the closure are visible in the current module (conservative default)
  - `pub` function parameters: assign Top (external callers may pass full-range values)

---

## 03.6 Completion Checklist

**Implementation order:** 03.1 (lattice) → 03.2 (transfer functions) → 03.4 (conditional refinement) → 03.3 (fixpoint loop) → 03.5 (interprocedural, DEFERRABLE). Each step must pass tests before proceeding.

- [ ] `ValueRange` lattice correctly implements join, meet, fits_in, min_width, is_constant, overlaps (in `range/mod.rs`); `widen` and `narrow` free functions correct (in `range/fixpoint.rs`)
- [ ] Arithmetic transfer functions implemented: `range_add`, `range_sub`, `range_mul`, `range_div`, `range_mod`, `range_floordiv`, `range_neg` (PrimOp dispatched); bitwise: `range_bitand`, `range_bitor`, `range_bitxor`, `range_shl`, `range_shr`, `range_bitnot`; built-in function ranges: `range_len`, `range_count`, `range_byte_to_int`, `range_char_to_int`, `range_abs`
- [ ] Top-level `transfer()` dispatcher has an arm for every `ArcInstr` variant (15 variants: `Let`, `Apply`, `ApplyIndirect`, `PartialApply`, `Project`, `Construct`, `RcInc`, `RcDec`, `IsShared`, `Set`, `SetTag`, `Reset`, `Reuse`, `CollectionReuse`, `Select`). Add a compile-time exhaustiveness test: the match must be non-`_` so new variants cause a build failure.
- [ ] Fixpoint loop handles all `ArcTerminator` variants (7: `Return`, `Jump`, `Branch`, `Switch`, `Invoke`, `Resume`, `Unreachable`). `Invoke` computes a range for `dst`; `Branch`/`Switch` produce refinements; others are no-ops. Use exhaustive match (no `_` arm) so new terminator variants cause a compile error.
- [ ] `transfer_primop()` has an arm for every `BinaryOp` variant (23: `Add`, `Sub`, `Mul`, `Div`, `Mod`, `FloorDiv`, `MatMul`, `Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Range`, `RangeInclusive`, `Coalesce`) and every `UnaryOp` variant (4: `Neg`, `Not`, `BitNot`, `Try`). Non-`_` match for exhaustiveness.
- [ ] Fixed-point iteration terminates within `max_iterations` for all test programs
- [ ] Block parameters (`ArcBlock::params`) are processed at the start of each block in the fixpoint loop, joining ranges from all predecessor `Jump` instructions
- [ ] Block terminators (`Branch`, `Switch`) propagate conditional range refinements to successor blocks
- [ ] `ArcTerminator::Invoke` handled in fixpoint loop — it defines a `dst` variable (same as `Apply`) and must have its range computed
- [ ] Conditional refinement extracts ranges from `if x < N` patterns
- [ ] Function signature propagation narrows parameters from call sites
- [ ] Recursive functions handled via SCC-based fixpoint with bounded iterations (max 10 per SCC, max 50 total)
- [ ] For `let x = 42`: range is exactly `[42, 42]`
- [ ] For `for i in 0..100`: range of `i` is `[0, 99]`
- [ ] For `let n = len(list)`: range is `[0, i64::MAX]`
- [ ] `./test-all.sh` green (range analysis is additive — no behavioral changes)
- [ ] `./clippy-all.sh` green
- [ ] Tracing: `ORI_LOG=ori_repr=debug` shows range computations for each function
- [ ] `#[tracing::instrument(skip_all)]` on `range_fixpoint()`, `transfer()`, and `refine_from_branch()`
- [ ] `tracing::debug!` at function entry/exit in `range_fixpoint()` showing function name, iteration count, and number of non-Top ranges
- [ ] `tracing::trace!` per-variable range updates inside the fixpoint loop (gated by trace level to avoid hot-path overhead)
- [ ] Property-based tests (proptest) for lattice laws and transfer function soundness:
  - Lattice: `join` is commutative, associative, idempotent; `meet` is commutative, associative, idempotent; `join(a, Bottom) == a`; `meet(a, Top) == a`; `a.join(b) ⊇ a` and `a.join(b) ⊇ b`
  - Transfer functions: for any concrete values `x ∈ a_range` and `y ∈ b_range`, the concrete result of `x op y` must be contained in `transfer_op(a_range, b_range)` (soundness property)
  - Widening: `widen(a, b) ⊇ b` always; sequence `widen(a₀, a₁), widen(a₁, a₂), ...` must stabilize within finite steps
  - Add `proptest` as a dev-dependency of `ori_repr` (already a workspace dependency in root `Cargo.toml` — just add `proptest.workspace = true` under `[dev-dependencies]`)
- [ ] Range results written into `ReprPlan::function_var_ranges` via `ReprPlan::set_var_ranges(func_name, ranges)` — verified by test
- [ ] Unknown or unsupported ArcInstr patterns gracefully degrade to `Top` (never panic, never return `Bottom` for reachable code). Explicit `tracing::debug!` when falling back to Top for a pattern that could be tightened.
- [ ] `compute_postorder()` and `successor_block_ids()` in `ori_arc::graph::mod.rs` changed from `pub(crate)` to `pub` (or `pub` wrappers added)

**Performance Budget:**
- [ ] Define `RangeAnalysisConfig` struct (in `range/mod.rs`):
  ```rust
  /// Configuration for range analysis passes.
  /// Follows the ">3 params → config struct" guideline.
  pub struct RangeAnalysisConfig {
      pub max_iterations: usize,       // default: 20
      pub max_blocks: usize,           // default: 500
      pub max_scc_iterations: usize,   // default: 10
      pub max_total_scc_iterations: usize, // default: 50
  }
  ```
- `max_iterations` default: 20 per function (intraprocedural fixpoint). Configurable via `RangeAnalysisConfig`.
- `max_scc_iterations` default: 10 per SCC (interprocedural). `max_total_scc_iterations` default: 50.
- **Time limit:** No wall-clock time limit (non-deterministic). Instead, cap total work: `max_instructions_processed = num_blocks * max_iterations * avg_block_size`. If exceeded, remaining variables get Top.
- **Per-function budget:** Functions with >500 blocks skip range analysis entirely (return all Top). Log at `warn` level.
- Analysis must not regress `./test-all.sh` wall-clock time by more than 5%. Measure with `hyperfine` before/after.

**Error Handling Policy:**
- Range analysis is a pure optimization pass — it must NEVER cause compilation failure.
- If any internal assertion fails (e.g., Bottom propagating where it shouldn't), log at `error` level and return all-Top for that function.
- Unknown `ArcInstr` variants (added after §03 is implemented): the `transfer()` match must be exhaustive (no `_` arm), so new variants cause a compile error forcing explicit handling. The correct default for new variants is `Top`.
- Division by range spanning zero: return Top (not panic). Shift by negative: return Top.

**Exit Criteria:** Running range analysis on `tests/benchmarks/bench_small.ori` and other `tests/benchmarks/` programs produces non-trivial ranges (not all `Top`) for loop counters, index variables, and function parameters. Results logged at `debug` level.

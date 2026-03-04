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

---

## 03.1 Interval Lattice

**File(s):** `compiler/ori_repr/src/range.rs`

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

      /// Widen: if bounds changed, extend to infinity in that direction
      pub fn widen(self, previous: Self) -> Self { ... }

      /// Narrow: intersect with evidence from conditional
      pub fn narrow(self, evidence: Self) -> Self { ... }
  }
  ```

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

---

## 03.2 Transfer Functions

**File(s):** `compiler/ori_repr/src/range_transfer.rs`

Transfer functions describe how each operation transforms value ranges.

- [ ] Arithmetic operations:
  ```rust
  pub fn range_add(a: ValueRange, b: ValueRange) -> ValueRange {
      match (a, b) {
          (Bounded { lo: a_lo, hi: a_hi }, Bounded { lo: b_lo, hi: b_hi }) => {
              let lo = a_lo.checked_add(b_lo);
              let hi = a_hi.checked_add(b_hi);
              match (lo, hi) {
                  (Some(lo), Some(hi)) => Bounded { lo, hi },
                  _ => Top, // overflow possible → give up
              }
          }
          _ => Top,
      }
  }

  pub fn range_sub(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_mul(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_div(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_mod(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_neg(a: ValueRange) -> ValueRange { ... }
  pub fn range_abs(a: ValueRange) -> ValueRange { ... }
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
  ```

- [ ] Literal ranges:
  ```rust
  /// Integer literal has an exact range
  pub fn range_literal(value: i64) -> ValueRange {
      Bounded { lo: value, hi: value }
  }
  ```

---

## 03.3 Widening & Narrowing Operators

**File(s):** `compiler/ori_repr/src/range_fixpoint.rs`

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

- [ ] Implement fixed-point iteration with widening:
  ```rust
  pub fn range_fixpoint(
      func: &CanFunction,
      pool: &Pool,
      max_iterations: usize,
  ) -> FxHashMap<VarId, ValueRange> {
      let mut ranges: FxHashMap<VarId, ValueRange> = FxHashMap::default();
      let mut iteration = 0;

      loop {
          let mut changed = false;
          for block in func.blocks_in_rpo() {
              for instr in block.instructions() {
                  let new_range = transfer(instr, &ranges, pool);
                  let var = instr.result();
                  let old = ranges.get(&var).copied().unwrap_or(Bottom);

                  // Apply widening after threshold
                  let merged = if iteration > 3 {
                      widen(old, old.join(new_range))
                  } else {
                      old.join(new_range)
                  };

                  if merged != old {
                      ranges.insert(var, merged);
                      changed = true;
                  }
              }
          }

          iteration += 1;
          if !changed || iteration >= max_iterations { break; }
      }

      // Narrowing pass (optional — one iteration)
      for block in func.blocks_in_rpo() {
          for instr in block.instructions() {
              let computed = transfer(instr, &ranges, pool);
              let var = instr.result();
              if let Some(&widened) = ranges.get(&var) {
                  let narrowed = narrow(widened, computed);
                  ranges.insert(var, narrowed);
              }
          }
      }

      ranges
  }
  ```

---

## 03.4 Conditional Range Refinement

**File(s):** `compiler/ori_repr/src/range_conditional.rs`

When code branches on a comparison (e.g., `if x < 100`), the true branch knows `x ∈ [-2⁶³, 99]` and the false branch knows `x ∈ [100, 2⁶³-1]`. This is the most powerful source of narrowing information.

- [ ] Implement conditional range extraction:
  ```rust
  pub fn refine_from_condition(
      cond: &Expr,
      ranges: &FxHashMap<VarId, ValueRange>,
      true_branch: bool,
  ) -> Vec<(VarId, ValueRange)> {
      match cond {
          // x < c → true: [lo, c-1], false: [c, hi]
          Expr::BinOp(Lt, Var(x), Lit(c)) => {
              let current = ranges.get(x).copied().unwrap_or(Top);
              if true_branch {
                  vec![(*x, current.meet(Bounded { lo: i64::MIN, hi: c - 1 }))]
              } else {
                  vec![(*x, current.meet(Bounded { lo: *c, hi: i64::MAX }))]
              }
          }
          // x >= c, x <= c, x > c, x == c, x != c
          // ... similar patterns
          _ => vec![], // can't extract info
      }
  }
  ```

- [ ] Handle common patterns:
  - `x >= 0` → non-negative range (common for indices, lengths)
  - `x < len(arr)` → bounded by collection size
  - `x == constant` → singleton range
  - Pattern match exhaustiveness → discriminant ranges
  - `match x { 0 => ..., 1 => ..., _ => ... }` → each arm has refined range

---

## 03.5 Function Signature Range Propagation

**File(s):** `compiler/ori_repr/src/range_signatures.rs`

For cross-function narrowing, we need to propagate range information through function signatures.

- [ ] Extend `FunctionSig` with optional range annotations:
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

- [ ] Handle special cases:
  - `@main` parameters: `args` length is `[0, i64::MAX]`
  - Trait method parameters: range is Top (unknown callers)
  - Closure parameters: range from capture context (if known)

---

## 03.6 Completion Checklist

- [ ] `ValueRange` lattice correctly implements join, meet, widen, narrow
- [ ] Transfer functions handle all arithmetic, bitwise, and comparison operations
- [ ] Fixed-point iteration terminates within `max_iterations` for all test programs
- [ ] Conditional refinement extracts ranges from `if x < N` patterns
- [ ] Function signature propagation narrows parameters from call sites
- [ ] For `let x = 42`: range is exactly `[42, 42]`
- [ ] For `for i in 0..100`: range of `i` is `[0, 99]`
- [ ] For `let n = len(list)`: range is `[0, i64::MAX]`
- [ ] `./test-all.sh` green (range analysis is additive — no behavioral changes)
- [ ] Tracing: `ORI_LOG=ori_repr=debug` shows range computations for each function

**Exit Criteria:** Running range analysis on `tests/benchmarks/fibonacci.ori` and `tests/benchmarks/` programs produces non-trivial ranges (not all `Top`) for loop counters, index variables, and function parameters. Results logged at `debug` level.

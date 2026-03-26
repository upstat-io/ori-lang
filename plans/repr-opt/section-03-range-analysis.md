---
section: "03"
title: "Value Range Analysis Framework"
status: in-progress
reviewed: true
third_party_review:
  status: resolved
  updated: 2026-03-26
  triage_note: "TPR-03-031 accepted and implemented on 2026-03-26 — iterated feedback loop with refresh_return_ranges() and reverse-topo Step 2. All TPR items resolved."
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
    status: complete
  - id: "03.2"
    title: "Transfer Functions"
    status: complete
  - id: "03.2b"
    title: "Field-Summary Infrastructure"
    status: complete
  - id: "03.3"
    title: "Widening & Narrowing Operators"
    status: complete
  - id: "03.4"
    title: "Conditional Range Refinement"
    status: complete
  - id: "03.5"
    title: "Function Signature Range Propagation"
    status: in-progress
  - id: "03.6"
    title: "Completion Checklist"
    status: in-progress
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

**Scope boundary — integer only:** §03's `ValueRange` lattice is integer-only (`i64` intervals). §05 (float narrowing) defines its own `FloatRange` lattice independently in `compiler/ori_repr/src/narrowing/float.rs`. §05 depends on §03 for the fixpoint infrastructure pattern and `RangeAnalysisConfig`, but NOT for float-specific range types. The "extended to float intervals" phrasing in §05's header means §05 builds a parallel float range pass using §03's framework, not that §03 must provide float intervals.

**Risk warning:** Abstract interpretation with widening/narrowing is the most complex analysis in this plan. Transfer functions for multiplication and division have subtle corner cases (signed overflow, division by ranges spanning zero). Implement §03.1 (lattice) and §03.2 (transfer functions) first with property-based tests (e.g., `proptest`). Only then add §03.3 (widening/narrowing). Start with conservative (Top-returning) transfer functions and tighten incrementally.

**Crate dependency:** Range analysis operates on `ArcFunction` (from `ori_arc::ir`). This means `ori_repr` depends on `ori_arc`. The dependency chain is `ori_types → ori_arc → ori_repr → ori_llvm`. This is correct: `ori_repr` reads from `ori_arc` IR but `ori_arc` does NOT depend on `ori_repr` (no cycle). The lattice types (`ValueRange`, `IntWidth`) live in `ori_repr` and do NOT reference `ori_arc` — only `fixpoint.rs` (which takes `&ArcFunction`) requires the `ori_arc` dependency.

**Visibility prerequisite:** `compute_postorder()` in `ori_arc::graph` is currently `pub(crate)`. It must be made `pub` (or a `pub` wrapper added) so `ori_repr` can compute RPO over `ArcFunction` blocks. This is a one-line visibility change in `ori_arc/src/graph/mod.rs:122`. `compute_predecessors()` at line 32 is also `pub(crate)` and must be made `pub` — `ori_repr`'s fixpoint loop uses it directly for predecessor information. `successor_block_ids()` at line 53 is `pub(crate)` and should also be made `pub` for consistency and potential direct use by `ori_repr`.

**Field-range prerequisite (required for §04, not optional):** The range engine cannot stop at per-variable intervals. §04's struct-field narrowing target (`Pixel { r, g, b, a: int }` with 0..255 fields → 4 bytes) requires a field-level summary keyed by `(struct type, field index)` (and the analogous tuple path). This summary must be populated from `Construct` argument ranges and queried by `Project`; otherwise all field loads remain `Top` and §04's field-narrowing exit criteria are unachievable.

> **FIRST STEP of §03:** Before any analysis code is written, make the three `pub(crate)` functions in `compiler/ori_arc/src/graph/mod.rs` into `pub`:
> - Line 32: `pub(crate) fn compute_predecessors` → `pub fn compute_predecessors`
> - Line 53: `pub(crate) fn successor_block_ids` → `pub fn successor_block_ids`
> - Line 122: `pub(crate) fn compute_postorder` → `pub fn compute_postorder`
>
> Verify with `cargo c` that no existing callers within `ori_arc` are broken (they won't be — pub is a superset of pub(crate)). This unblocks all §03 file creation.

**File organization:** 6 files in `compiler/ori_repr/src/range/` submodule — `mod.rs` (lattice + re-exports), `transfer.rs`, `fixpoint.rs`, `conditional.rs`, `signatures.rs`, `field_summary.rs` (struct/tuple field range aggregation), plus `tests.rs` (sibling test convention, at `range/tests.rs` per `mod.rs` → sibling convention).

**File size warning:** `transfer.rs` is the highest-risk file for exceeding the 500-line limit. It contains: the top-level `transfer()` dispatcher (~60 lines), `transfer_primop()` dispatcher for 23 `BinaryOp` + 4 `UnaryOp` variants (~80 lines), individual transfer functions for arithmetic (add/sub/mul/div/mod/floordiv/neg — ~120 lines), bitwise operations (~80 lines), built-in function ranges (len/count/byte_to_int/char_to_int/abs — ~40 lines), and the `is_int_typed()` helper. Total estimate: ~400-500 lines. If it exceeds 500 during implementation, split into `transfer/mod.rs` (dispatcher), `transfer/arithmetic.rs`, and `transfer/bitwise.rs`.

**Documentation requirement:** All `pub` types and functions must have `///` doc comments. Each file must have a `//!` module-level doc comment. This applies to `ValueRange`, `IntWidth`, all transfer functions, the fixpoint driver, and the conditional refinement API.

---

## 03.1 Interval Lattice

**File(s):** `compiler/ori_repr/src/range/mod.rs` (already a `range/` submodule from §01 — currently contains only a placeholder `ValueRange` ZST)

The interval lattice is the core data structure. Each element represents a set of possible integer values.

- [x] **Remove the placeholder `ValueRange` ZST** from `compiler/ori_repr/src/range/mod.rs` (lines 1-12). The current file defines `pub struct ValueRange;` with `#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]`. Replace it entirely with the enum below. Also update the `//!` module doc to describe the full interval lattice, not "Placeholder only in §01". (2026-03-25)
- [x] **Remove `#[expect(clippy::zero_sized_map_values)]` from `plan.rs`** — two sites: `ReprPlan` struct (line 70-73) and `ReprPlan::new()` (line 105-108). Once `ValueRange` is no longer a ZST, these suppressions become dead. `EscapeInfo` is still a ZST, so change the `reason` text from "EscapeInfo and ValueRange" to "EscapeInfo is placeholder ZST — replaced by §08". The `set_var_ranges()` method (line 142-145) also has its own `#[expect(clippy::zero_sized_map_values)]` — remove it entirely. Also fixed `.cloned()` → `.copied()` on `var_range()`. (2026-03-25)
- [x] Define the `ValueRange` lattice: (2026-03-25)
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

  // IMPORTANT: Implement Default → Top so that `ReprPlan::var_range()`'s
  // existing `.unwrap_or_default()` returns the safe conservative default.
  // The current placeholder ValueRange derives Default (gives ZST);
  // the enum replacement must explicitly default to Top.
  //
  impl Default for ValueRange {
      fn default() -> Self { Self::Top }
  }
  ```

- [x] Implement lattice operations: (2026-03-25)
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

- [x] Implement width-specific range constants: (2026-03-25)
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

- [x] **Add `RangeAnalysisConfig` struct** to `mod.rs` (defined in §03.6 but needed by §03.3 fixpoint — define here so §03.3 can reference it). Include `Default` impl with the documented defaults (`max_iterations: 20`, `max_blocks: 500`, `max_scc_iterations: 10`, `max_total_scc_iterations: 50`). (2026-03-25)
- [x] **Implement `is_int_typed(ty: Idx, pool: &Pool) -> bool`** (2026-03-26) helper in `mod.rs` — checks `pool.tag(ty) == Tag::Int`. Handles edge cases: `Idx::ERROR` returns false, resolved newtypes delegate to inner type. **[TPR-03-007]** Also handles `Tag::Applied` — resolves through applied types the same way as `Named`/`Alias` (via `pool.resolve_fully()`). Added `tracing::trace!` for non-obvious Applied resolution path.
- [x] Comprehensive unit tests for lattice operations in `compiler/ori_repr/src/range/tests.rs` (2026-03-25). 58 tests covering all lattice operations, boundary values, semantic pin. (sibling test file per convention — `mod.rs` → `tests.rs`). **TDD: write these tests BEFORE implementing the lattice operations. Verify they fail (compile error or assertion). Then implement. Tests must pass unchanged.** Tests must cover:
  - **join**: commutative, associative, idempotent, Bottom identity (`join(x, Bottom) == x`), Top absorbing (`join(x, Top) == Top`), disjoint ranges produce enclosing range, overlapping ranges produce union
  - **meet**: commutative, associative, idempotent, Top identity (`meet(x, Top) == x`), Bottom absorbing (`meet(x, Bottom) == Bottom`), disjoint ranges produce Bottom, overlapping ranges produce intersection
  - **fits_in**: all 4 widths (I8/I16/I32/I64) x representative ranges (exact boundary: `[-128, 127]` fits I8, `[-129, 127]` does not; `[0, 255]` fits I16 signed but not I8 signed), Top returns false for I8/I16/I32 and true for I64, Bottom returns true for all widths
  - **min_width**: boundary values (`[-128, 127]` → I8, `[-129, 127]` → I16, `[128, 128]` → I16, `[-32768, 32767]` → I16, `[-32769, 32767]` → I32, `[0, 0]` → I8, `[i64::MIN, i64::MAX]` → I64), Top → I64, Bottom → I8 (smallest valid)
  - **is_constant**: single value (`[42, 42]` → `Some(42)`), range (`[0, 10]` → `None`), Bottom → `None`, Top → `None`
  - **overlaps**: disjoint (`[0, 5]` vs `[6, 10]`), touching (`[0, 5]` vs `[5, 10]`), nested (`[0, 10]` vs `[3, 7]`), identical, with Bottom, with Top
  - **i64 boundary**: `min_width` for `[i64::MIN, i64::MIN]` → I64, `fits_in` for `Bounded { lo: i64::MIN, hi: i64::MAX }` and each width
  - **Semantic pin**: `join(Bounded(0, 99), Bounded(50, 150))` == `Bounded(0, 150)` — this test ONLY passes if join computes the enclosing interval (not intersection, not union of discrete values)
  - Add `#[cfg(test)] mod tests;` at the bottom of `mod.rs`
  - **Both debug and release**: `cargo test -p ori_repr` (debug) and `cargo test -p ori_repr --release` (release) must both pass
- [x] **[TPR-03-003]** (2026-03-26) Replaced exact-size assertion in `compiler/ori_repr/src/tests.rs` `value_range_is_interval_lattice()` with semantic checks (Default → Top, join, meet). Layout is not part of the section's semantic contract.
- [x] Import and use `tracing` crate (2026-03-26). `tracing` dependency already in `Cargo.toml`. Added `tracing::trace!` in `is_int_typed()` for Applied resolution path. Further tracing instrumentation added as analysis code is implemented (§03.3+).
- [x] **Re-export key types from `mod.rs`** (2026-03-26): Added `pub use` for `BranchRefinement`, `FieldSummaryTable`, `TransferContext`, and all transfer functions. `RangeFixpointResult` will be re-exported when §03.3 creates it. `ValueRange` and `RangeAnalysisConfig` already defined in `mod.rs` (not submodules). `IntWidth` is in `crate::repr`.

---

## 03.2 Transfer Functions

**File(s):** `compiler/ori_repr/src/range/transfer.rs`

Transfer functions describe how each operation transforms value ranges.

- [x] **Implement `transfer_primop()` dispatcher** (2026-03-25) — maps `PrimOp::Binary(op)` and `PrimOp::Unary(op)` to the appropriate transfer function. Uses exhaustive match (no `_` arm) on both `BinaryOp` (23 variants) and `UnaryOp` (4 variants) so new variants cause compile errors. Returns `ValueRange`. Signature: `fn transfer_primop(op: PrimOp, args: &[ArcVarId], ranges: &FxHashMap<ArcVarId, ValueRange>, pool: &Pool) -> ValueRange`.
- [x] **Implement `transfer_known_call()` helper** (2026-03-25, stub — builtin matching deferred to §03.5) — checks if a `Name` corresponds to a known built-in function (`len`, `count`, `byte_to_int`, `char_to_int`, `abs`) and returns `Some(ValueRange)` or `None` for unknown callees. Signature: `fn transfer_known_call(func: Name, pool: &Pool) -> Option<ValueRange>`. Built-in function names are resolved via the interner or by matching against known `Name` constants. **Design decision needed:** How to identify built-in functions by `Name` — either compare against interned names from `ori_ir::BuiltinConstant` or use a pre-computed `FxHashSet<Name>` passed via `TransferContext`. The plan should specify which approach.
- [x] Arithmetic operations: (2026-03-25)
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

- [x] Bitwise operations: (2026-03-25)
  ```rust
  pub fn range_bitand(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_bitor(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_bitxor(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_shl(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  pub fn range_shr(a: ValueRange, b: ValueRange) -> ValueRange { ... }
  ```

- [x] Built-in function ranges: (2026-03-25)
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

- [x] Literal ranges: (2026-03-25)
  ```rust
  /// Integer literal has an exact range
  pub fn range_literal(value: i64) -> ValueRange {
      Bounded { lo: value, hi: value }
  }
  ```

- [x] Remaining arithmetic operations (from `BinaryOp`):
  - `FloorDiv` — integer floor division: `a / b` rounded toward negative infinity. Same division-by-zero handling as `range_div`; result range differs from truncating division when signs differ.
  - `MatMul` (`@`) — user-defined operator: returns Top (cannot reason about custom implementations).

- [x] Comparison operations (produce `bool`, not `int` — range is `[0, 1]`):
  - `Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq` — all produce `ValueRange::Bounded { lo: 0, hi: 1 }` (boolean).
  - Comparison results are primarily useful via §03.4 conditional refinement, not directly.

- [x] Logical operations (produce `bool`):
  - `And` (`&&`), `Or` (`||`) — produce `ValueRange::Bounded { lo: 0, hi: 1 }`.

- [x] Range/coalesce operations:
  - `Range` (`..`), `RangeInclusive` (`..=`) — produce a Range value, not an int. Return Top for the `dst` variable (range analysis tracks int-typed variables only).
  - `Coalesce` (`??`) — unwraps Option; return Top (value depends on Option contents).

- [x] Unary operations (from `UnaryOp`):
  - `Neg` — already listed as `range_neg`.
  - `Not` (`!`) — logical not on bool: returns `[0, 1]`.
  - `BitNot` (`~`) — bitwise complement: if `a ∈ [lo, hi]` and both non-negative, result is `[-hi-1, -lo-1]`. Conservative: return Top for mixed-sign ranges.
  - `Try` (`?`) — desugared before ARC IR; should not appear. If encountered, return Top.

- [x] **Top-level transfer function dispatcher** (2026-03-25) — maps each `ArcInstr` variant to a range:
  ```rust
  /// Context needed by the transfer function beyond ranges and pool.
  /// Bundles per-function and cross-function state to avoid >4 params.
  pub struct TransferContext<'a> {
      pub ranges: &'a FxHashMap<ArcVarId, ValueRange>,
      pub pool: &'a Pool,
      /// Per-variable types from ArcFunction::var_types — needed to resolve
      /// the struct/tuple Idx for Project instructions when querying field summaries.
      pub var_types: &'a [Idx],
      /// Field-summary table (populated by Construct, queried by Project).
      /// Mutable because Construct instructions update it during the fixpoint.
      /// Key: (struct/tuple Idx, field index) → joined ValueRange.
      pub field_summaries: &'a FxHashMap<(Idx, u32), ValueRange>,
  }


  /// Compute the output range for a single ArcInstr.
  /// Returns Top for non-int-typed destinations or unsupported patterns.
  pub fn transfer(
      instr: &ArcInstr,
      ctx: &TransferContext<'_>,
  ) -> ValueRange {
      let TransferContext { ranges, pool, var_types, field_summaries } = ctx;
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

          // Field projection: query field-summary table for struct/tuple fields.
          // The struct/tuple Idx is recovered from var_types[value.index()],
          // and combined with `field` to look up the pre-computed field range.
          //
          ArcInstr::Project { ty, value, field, .. } => {
              if !is_int_typed(*ty, pool) { return Top; }
              // Look up the struct/tuple type from the source variable.
              let struct_idx = var_types.get(value.index()).copied();
              match struct_idx {
                  Some(idx) => field_summaries
                      .get(&(idx, *field))
                      .copied()
                      .unwrap_or(Top),
                  None => Top, // unknown source type — conservative
              }
          }

          // Construction: the instruction produces a composite value (not int),
          // so the direct transfer result is Top. However, construction sites
          // are the PRIMARY source of field range information — see
          // `update_field_summaries()` in field_summary.rs, called by the
          // fixpoint loop after processing each Construct instruction.
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

- [x] **Unit tests for transfer functions** (2026-03-25, 46 tests) in `compiler/ori_repr/src/range/tests.rs` (shared test file for all range submodules, per sibling convention). **TDD: write tests BEFORE implementing. Verify compile error or assertion failure. Then implement. Tests must pass unchanged.** Required coverage:
  - **Arithmetic matrix** — each function (`range_add`, `range_sub`, `range_mul`, `range_div`, `range_mod`, `range_floordiv`, `range_neg`) with: (a) two positive bounded ranges, (b) one negative + one positive bounded, (c) one bounded + one Bottom → Bottom, (d) one bounded + one Top → Top, (e) overflow cases (`checked_add` returns `None` → Top)
  - **Multiplication quadrants** — `range_mul` with all four sign quadrant combinations: positive x positive, positive x negative, negative x negative, negative x positive. Must compute `min/max` of `{lo*lo, lo*hi, hi*lo, hi*hi}`. Also test `[0, 0] * anything` → `[0, 0]`
  - **Division edge cases** — `range_div` with: divisor spanning zero → Top, divisor `[0, 0]` → Top (division by zero), positive dividend / positive divisor → bounded, negative dividend / positive divisor → bounded
  - **Bitwise functions** — each (`range_bitand`, `range_bitor`, `range_bitxor`, `range_shl`, `range_shr`, `range_bitnot`) with representative cases. `range_shl` with negative shift count → Top, shift count >= 64 → Top. `range_bitnot` with positive range, mixed-sign range (→ Top)
  - **Abs edge case** — `range_abs` with: all-positive range (identity), all-negative (flip), range spanning zero, range including `i64::MIN` → Top
  - **Dispatcher routing** — `transfer_primop`: one test per `BinaryOp` variant (23 total), one per `UnaryOp` variant (4 total) — verify correct delegation
  - **Top-level `transfer()` dispatcher** — at least one test per `ArcInstr` variant (construct programmatically). Key semantic pins: `Let` with int literal → exact range, `Apply` to `len` → `[0, i64::MAX]`, `Select` → join of branches, `Project` with field summary → bounded, `IsShared` → `[0, 1]`
  - **Semantic pin**: `range_add(Bounded(0, 10), Bounded(0, 10))` == `Bounded(0, 20)` — this test ONLY passes with correct add propagation (not Top, not Bottom)
  - **Both debug and release**: `cargo test -p ori_repr` (debug) and `cargo test -p ori_repr --release` (release) must both pass
- [x] **File size check** (split on 2026-03-26): `transfer/mod.rs` grew to 555 lines after TPR-03-008/009 fixes, exceeding 500-line limit. Split into `transfer/mod.rs` (242 lines — dispatcher), `transfer/arithmetic.rs` (218 lines), `transfer/bitwise.rs` (124 lines). All within limits.
- [x] **[TPR-03-004] Fix `range_div()` / `range_floordiv()` panic on `i64::MIN / -1`** (2026-03-25) — replaced raw `/` with `checked_div()` for all 4 corners; any `None` → `Top`. 4 regression tests: exact MIN/-1, range containing MIN/-1, MIN/positive (no overflow), floordiv delegation. Debug + release green.
- [x] **[TPR-03-005] Fix `range_bitnot()` panic on `i64::MIN` endpoints** (2026-03-25) — replaced unchecked `(-hi).checked_sub(1)` with `hi.checked_neg().and_then(|v| v.checked_sub(1))` (matches `range_neg()` pattern). 4 regression tests: exact MIN, range containing MIN, i64::MAX (valid), negative range. Debug + release green.
- [x] **[TPR-03-008] Fix `range_floordiv()` soundness** (2026-03-26) — replaced delegation to truncating `range_div()` with proper floor-division corner computation via `checked_floor_div()` (trunc + adjustment when signs differ and remainder != 0). 8 regression tests: exact mixed-sign (-1 div 2, -7 div 2), same-sign (positive, negative), mixed-sign range, by-zero, positive range, bottom propagation. Debug + release green. Semantic pin: `floordiv_mixed_sign_exact` ONLY passes with floor division.
- [x] **[TPR-03-009] Fix `range_shr()` sign-dependent monotonicity** (2026-03-26) — replaced directional monotonicity assumption with 4-corner computation (`al>>bl`, `al>>bh`, `ah>>bl`, `ah>>bh`) + min/max. 4 regression tests: negative range with shift range, mixed-sign range, negative exact shift, positive range unchanged. Debug + release green. Semantic pin: `shr_negative_range_with_shift_range` ONLY passes with sign-aware corners.
- [x] **[TPR-03-010] Split `transfer/mod.rs` into submodules** (2026-03-26) — split 555-line file into: `transfer/mod.rs` (242 lines — dispatcher, `TransferContext`, `transfer()`, `transfer_primop()`, `transfer_known_call()`, literals), `transfer/arithmetic.rs` (218 lines — add through abs, `checked_floor_div`), `transfer/bitwise.rs` (124 lines — bitand through bitnot, `shift_amount()`). All 66 transfer tests pass. All functions re-exported via `pub use`.

---

## 03.2b Field-Summary Infrastructure


**File(s):** `compiler/ori_repr/src/range/field_summary.rs`

**Why this exists:** §04's struct-field narrowing target (`Pixel { r, g, b, a: int }` with 0..255 fields narrowed to 4 bytes total) requires field-level range information. Without it, `Project` instructions return `Top` for all struct fields and §04's field-narrowing exit criteria are unachievable. The field-summary table is the mechanism that bridges per-variable intraprocedural ranges (§03) to per-type-field global ranges (§04).

**Implementation order:** Build alongside §03.2 (before fixpoint). The fixpoint loop (§03.3) calls `update_field_summaries()` when processing `Construct` instructions.

- [x] Define the `FieldSummaryTable` type:
  ```rust
  /// Aggregates field ranges across all Construct sites for struct/tuple types.
  ///
  /// Each entry represents the join of argument ranges at position `field` across
  /// ALL Construct instructions that build type `type_idx`. This is the evidence
  /// base for §04's struct-field narrowing.
  ///
  /// Example: if `Pixel { r: 0, g: 128, b: 255, a: 0 }` and
  /// `Pixel { r: 255, g: 0, b: 0, a: 255 }` are the only construction sites,
  /// then field_ranges[(Pixel_idx, 0)] = [0, 255], etc.
  pub struct FieldSummaryTable {
      field_ranges: FxHashMap<(Idx, u32), ValueRange>,
  }

  impl FieldSummaryTable {
      pub fn new() -> Self { Self { field_ranges: FxHashMap::default() } }

      /// Borrow the underlying map for read-only access (e.g., TransferContext).
      pub fn as_map(&self) -> &FxHashMap<(Idx, u32), ValueRange> {
          &self.field_ranges
      }

      /// Record one Construct site's argument ranges into the summary.
      /// Each arg_range[i] is joined with the existing range for (type_idx, i).
      pub fn observe_construct(
          &mut self,
          type_idx: Idx,
          arg_ranges: &[ValueRange],
      ) {
          for (i, &range) in arg_ranges.iter().enumerate() {
              self.field_ranges
                  .entry((type_idx, i as u32))
                  .and_modify(|existing| *existing = existing.join(range))
                  .or_insert(range);
          }
      }

      /// Query the aggregated range for a specific field.
      pub fn field_range(&self, type_idx: Idx, field: u32) -> ValueRange {
          self.field_ranges
              .get(&(type_idx, field))
              .copied()
              .unwrap_or(ValueRange::Top)
      }

      /// Snapshot into ReprPlan's field_range_summaries.
      pub fn flush_to_repr_plan(&self, repr_plan: &mut ReprPlan) {
          for (&(idx, field), &range) in &self.field_ranges {
              repr_plan.join_field_range(idx, field, range);
          }
      }
  }
  ```

- [x] Implement `update_field_summaries()` — called from the fixpoint loop after each `Construct`:
  ```rust
  /// Update the field-summary table when a Construct instruction is encountered.
  /// Only processes Struct and Tuple constructors with int-typed fields.
  pub fn update_field_summaries(
      instr: &ArcInstr,
      ranges: &FxHashMap<ArcVarId, ValueRange>,
      var_types: &[Idx],
      pool: &Pool,
      table: &mut FieldSummaryTable,
  ) {
      let ArcInstr::Construct { ty, ctor, args, .. } = instr else { return };
      // Struct, tuple, and enum variant constructors carry meaningful field positions.
      //
      match ctor {
          CtorKind::Struct(_) | CtorKind::Tuple | CtorKind::EnumVariant { .. } => {}
          _ => return, // list/map/set/closure don't have named fields
      }
      let arg_ranges: Vec<ValueRange> = args.iter().map(|arg| {
          // Only track int-typed arguments
          let arg_ty = var_types.get(arg.index()).copied();
          if arg_ty.map_or(false, |t| is_int_typed(t, pool)) {
              ranges.get(arg).copied().unwrap_or(ValueRange::Top)
          } else {
              ValueRange::Top // non-int fields get Top (§04 ignores them)
          }
      }).collect();
      table.observe_construct(*ty, &arg_ranges);
  }
  ```

- [x] Integrate `FieldSummaryTable` into the fixpoint loop (§03.3) (2026-03-26):
  - Create `FieldSummaryTable::new()` before the fixpoint loop starts
  - After processing each `Construct` instruction in the body loop, call `update_field_summaries()`
  - Pass `table.as_map()` as `field_summaries` in `TransferContext` so `Project` can query it
  - `flush_to_repr_plan()` called from `analyze_ranges()` (§03.6) after fixpoint returns

- [x] Handle enum variant constructors:
  - `CtorKind::EnumVariant { enum_name, variant }` — add variant payload fields to the field-summary table keyed by `(variant_type_idx, field)` where `variant_type_idx` is the variant's own `Idx` (from `Construct.ty`). This enables §07's niche analysis to see narrowed payload ranges. The `update_field_summaries` match should include `EnumVariant` alongside `Struct` and `Tuple`.

- [x] **Unit tests for `FieldSummaryTable`** in `compiler/ori_repr/src/range/tests.rs`. **TDD: write tests BEFORE implementing. Verify they fail. Then implement. Tests must pass unchanged.** Required coverage:
  - Single construction site with constant args → exact ranges
  - Multiple construction sites → join produces correct widened range (e.g., `observe_construct` with `[0, 0]` then with `[255, 255]` → field range is `[0, 255]`)
  - Non-int fields → stored as Top (not missing)
  - Tuple constructors handled same as struct constructors
  - Empty args list → no entries (e.g., unit struct)
  - EnumVariant constructor → payload fields added to summary table
  - `flush_to_repr_plan` writes correct ranges into `ReprPlan::field_range_summaries`
  - `field_range` for unknown `(type_idx, field)` returns Top (not panic)
  - **Semantic pin**: Two construction sites with `Pixel { r: 0, g: 128, b: 255, a: 0 }` and `Pixel { r: 255, g: 0, b: 0, a: 255 }` → `field_range(pixel_idx, 0..3)` all return `[0, 255]` — this is the §03→§04 contract test

---

## 03.3 Widening & Narrowing Operators

**File(s):** `compiler/ori_repr/src/range/fixpoint.rs`

**Implementation order:** Implement §03.2b (field summaries) and §03.4 (conditional refinement) BEFORE the fixpoint loop in this section. The fixpoint loop calls `update_field_summaries()` from §03.2b and `refine_from_branch()` from §03.4 when processing instructions and terminators respectively. Without §03.4, the fixpoint loop cannot refine ranges at branch points, making loop counter narrowing (the primary use case) incomplete. The recommended build order is: 03.1 → 03.2 → 03.2b → 03.4 → 03.3 → 03.5 (matches §03.6).

**Complexity warning:** This is the highest-risk subsection. The fixpoint loop must correctly handle: (1) block parameter merging (phi-like), (2) terminator-driven refinement, (3) widening threshold tuning, (4) narrowing pass, (5) `ArcTerminator::Invoke` which defines a variable. Getting any of these wrong produces silent unsoundness (ranges too narrow) or uselessness (all Top). Budget extra time for testing.

For loops and recursive functions, naive fixed-point iteration may not terminate. Widening accelerates convergence; narrowing recovers precision after widening.

- [x] Implement widening operator (2026-03-26):
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

- [x] Implement narrowing operator (2026-03-26):
  ```rust
  /// Narrowing: intersect widened result with transfer function output
  pub fn narrow(widened: ValueRange, computed: ValueRange) -> ValueRange {
      widened.meet(computed)
  }
  ```

- [x] **IR choice** (2026-03-26): Range analysis operates on `ArcFunction` (from `ori_arc::ir`), NOT `CanExpr`:
  - `ArcFunction` has basic blocks (`ArcBlock`) and SSA-like variables (`ArcVarId`); dominator trees are computed separately via `DominatorTree::build(func)` in `ori_arc/src/graph/dominator.rs`
  - `CanExpr` (in `ori_ir::canon::expr`) is a sugar-free canonical expression enum with no explicit control flow graph — unsuitable for dataflow analysis
  - This means range analysis runs AFTER ARC lowering but BEFORE LLVM codegen
  - The `ArcFunction` → range analysis → ReprPlan → LLVM codegen flow preserves phase ordering

- [x] **Block parameter merging (phi handling)** (2026-03-26): ARC IR uses block parameters instead of phi nodes. `ArcBlock::params` is `Vec<(ArcVarId, Idx)>` — values passed via `Jump { target, args }`. At CFG merge points, the range for a block parameter must be the **join** of the ranges of all incoming arguments across all predecessor `Jump` instructions. The fixpoint loop must process block parameters before block body instructions:
  ```rust
  // Pre-compute predecessor map ONCE before the fixpoint loop.
  // Use `compute_predecessors()` from `ori_arc::graph` (must be made `pub`).
  // It returns `Vec<Vec<usize>>` indexed by block index — O(1) lookup,
  // more efficient than building a FxHashMap.
  //
  let predecessors: Vec<Vec<usize>> = compute_predecessors(func);

  // Then in the fixpoint loop, for each block, before processing body:
  for (param_idx, (param_var, _param_ty)) in block.params.iter().enumerate() {
      let mut merged = Bottom;
      for &pred_idx in &predecessors[block_idx] {
          let pred = &func.blocks[pred_idx];
          if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
              if target.index() == block_idx {
                  if let Some(&arg_var) = args.get(param_idx) {
                      let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                      merged = merged.join(arg_range);
                  }
              }
          }
          // Branch does not pass args — only control flow.
          // Invoke does NOT pass block args — its `args` are call args, not block params.
          // The `dst` result is handled in Step 3 (terminator processing).
      }
      // Update param range (with widening if iteration > threshold)
  }
  ```
  **Important:** Without this, loop induction variables (which are block parameters on loop headers) will never get non-Bottom ranges, making loop counter narrowing impossible. This is the most critical gap — `for i in 0..100` lowers to a loop with `i` as a block parameter.

  **Performance note:** The predecessor Vec (`compute_predecessors`) must be computed ONCE before the fixpoint loop, not recomputed per iteration. It returns `Vec<Vec<usize>>` indexed by block index, so predecessor lookups are O(1) by index. The naive approach of scanning all blocks per parameter is O(blocks x params) per iteration — with 500 blocks and 20 iterations, that's 10,000 full-scan passes.

- [x] **Terminator-driven refinement** (2026-03-26): The fixpoint loop must also process block terminators, not just body instructions. Three concerns:
  1. **`Invoke { dst, ty, func, args, .. }`**: This terminator DEFINES a variable (`dst`). It is functionally equivalent to `Apply` but with unwind semantics. The fixpoint loop must compute a range for `dst` (same logic as `Apply` — check for known built-in, otherwise Top).
  2. **`Branch { cond, then_block, else_block }`**: Apply conditional refinement (§03.4) to variables in `then_block` and `else_block`.
  3. **`Switch { scrutinee, cases, default }`**: The scrutinee has range `[case_val, case_val]` in each case block, and the complement range in the default block. **Note:** `Switch` cases are `Vec<(u64, ArcBlockId)>` — the case values are `u64`, not `i64`. Use `i64::try_from(case_val)` and skip refinement for values exceeding `i64::MAX`.
  - Store per-block incoming refinements in a side table: `FxHashMap<(ArcBlockId, ArcVarId), ValueRange>`. Apply these at the start of each block during the next iteration.

- [x] Implement fixed-point iteration with widening (2026-03-26):
  ```rust
  // NOTE: ArcFunction does not have a blocks_in_rpo() method.
  // Compute RPO via compute_postorder() from ori_arc::graph and reverse it.
  // ArcBlock fields are accessed directly: block.body (Vec<ArcInstr>),
  // block.terminator (ArcTerminator). ArcInstr::defined_var() returns
  // Option<ArcVarId> (not all instructions define a variable).
  //
  // IMPORTANT: ArcTerminator::Invoke also defines a variable (dst).
  // The fixpoint loop must process terminators, not just body instructions.
  //
  // NOTE: ori_ir::Name implements Debug but NOT Display. Debug output is
  // `Name(shard=X, local=Y)` — not human-readable. Use func.name.raw()
  // (returns u32) in tracing macros for compact output. For human-readable
  // function names, use the interner: config.interner.lookup(func.name).
  // Example: tracing::warn!(func = func.name.raw(), ...);
  //

  /// Widening threshold — start widening after this many iterations.
  /// Named constant, not a magic number.
  const WIDEN_THRESHOLD: usize = 3;

  /// Result of range analysis for a single function.
  ///
  pub struct RangeFixpointResult {
      /// Per-variable ranges within this function.
      pub var_ranges: FxHashMap<ArcVarId, ValueRange>,
      /// Field-level range summaries from Construct instructions.
      pub field_summaries: FieldSummaryTable,
      /// Join of all Return terminator value ranges (for §03.5 interprocedural).
      pub return_range: ValueRange,
  }

  pub fn range_fixpoint(
      func: &ArcFunction,
      pool: &Pool,
      config: &RangeAnalysisConfig,
  ) -> RangeFixpointResult {
      // Budget check: skip analysis for very large functions
      if func.blocks.len() > config.max_blocks {
          tracing::warn!(
              func = func.name.raw(),
              blocks = func.blocks.len(),
              "skipping range analysis — function too large"
          );
          // Return empty result (all variables get Top via default lookups).
          //
          return RangeFixpointResult {
              var_ranges: FxHashMap::default(),
              field_summaries: FieldSummaryTable::new(),
              return_range: ValueRange::Top,
          };
      }

      let mut ranges: FxHashMap<ArcVarId, ValueRange> = FxHashMap::default();
      // Return range accumulator — join of all Return terminator value ranges.
      // Used by §03.5 to populate FunctionRangeInfo::return_range.
      //
      let mut return_range = Bottom;
      let mut iteration = 0;

      // Compute reverse postorder (RPO) block indices.
      // compute_rpo is a local helper: compute_postorder() then reverse.
      //
      let rpo = {
          let mut po = compute_postorder(func);
          po.reverse();
          po
      };

      // Pre-compute predecessors: Vec<Vec<usize>> indexed by block index.
      // Reuses `compute_predecessors()` from `ori_arc::graph` (made `pub`).
      //
      let predecessors = compute_predecessors(func);

      // Field-summary table — populated from Construct instructions,
      // queried by Project instructions via TransferContext.
      //
      let mut field_summary_table = FieldSummaryTable::new();

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
                  for &pred_idx in &predecessors[block_idx] {
                      let pred = &func.blocks[pred_idx];
                      if let ArcTerminator::Jump { target, args, .. } = &pred.terminator {
                          if target.index() == block_idx {
                              if let Some(&arg_var) = args.get(param_idx) {
                                  let arg_range = ranges.get(&arg_var).copied().unwrap_or(Bottom);
                                  merged = merged.join(arg_range);
                              }
                          }
                      }
                      // Invoke does NOT pass block args — its `args` are call args, not block params.
                      // The `dst` result is handled in Step 3 (terminator processing).
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
                  // Update field summaries from Construct instructions.
                  //
                  update_field_summaries(instr, &ranges, &func.var_types, pool, &mut field_summary_table);

                  let ctx = TransferContext {
                      ranges: &ranges,
                      pool,
                      var_types: &func.var_types,
                      field_summaries: field_summary_table.as_map(),
                  };

                  let new_range = transfer(instr, &ctx);
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
                      // TPR-03-017: JOIN case values per (block, scrutinee).
                      for &(case_val, case_block) in cases {
                          if let Ok(val) = i64::try_from(case_val) {
                              let exact = Bounded { lo: val, hi: val };
                              block_refinements
                                  .entry((case_block, *scrutinee))
                                  .and_modify(|e| *e = e.join(exact))
                                  .or_insert(exact);
                          }
                      }
                      // TPR-03-018: default gets complement refinement.
                      let scr_range = ranges.get(scrutinee).copied().unwrap_or(Top);
                      let complement = switch_default_complement(scr_range, cases);
                      if complement != scr_range {
                          block_refinements
                              .entry((*default, *scrutinee))
                              .and_modify(|e| *e = e.meet(complement))
                              .or_insert(complement);
                      }
                  }
                  // Exhaustive — no `_` arm. Each variant explicitly handled.
                  ArcTerminator::Return { value } => {
                      // No variable defined, no refinement. However, §03.5 needs
                      // the return range — collect it into a function-level return
                      // range accumulator (join across all Return terminators).
                      //
                      if is_int_typed(func.return_type, pool) {
                          let ret_range = ranges.get(value).copied().unwrap_or(Top);
                          return_range = return_range.join(ret_range);
                      }
                  }
                  ArcTerminator::Jump { .. } => {} // args handled in block parameter merging (Step 1)
                  ArcTerminator::Resume => {}
                  ArcTerminator::Unreachable => {}
              }
          }

          iteration += 1;
          if !changed || iteration >= config.max_iterations { break; }
      }

      tracing::debug!(
          func = func.name.raw(),
          iterations = iteration,
          non_top = ranges.values().filter(|r| !matches!(r, Top)).count(),
          "range analysis complete"
      );

      // TPR-03-019: Run 2 narrowing passes (block params + refinements + invoke).
      // Second pass propagates body-narrowed ranges back through block params.
      for _ in 0..2 {
          run_narrowing_pass(&rpo, func, pool, &mut ranges, &field_summary_table,
              &predecessors, &block_refinements);
      }

      RangeFixpointResult {
          var_ranges: ranges,
          field_summaries: field_summary_table,
          return_range,
      }
  }
  ```

- [x] **[TPR-03-015] Apply block-entry refinements to all live variables, not just block parameters** (2026-03-26). Implemented as a save/restore pattern: `apply_block_refinements()` temporarily intersects non-parameter variable ranges with block refinements, body instructions see the refined ranges, then `restore_block_refinements()` restores originals. This avoids corrupting the global range map (since the same variable can have different refinements in true/false branches). 3 new tests: `fixpoint_branch_refines_non_param_variable`, `fixpoint_switch_refines_non_param_variable`. Added `FieldSummaryTable::clear()` method. Debug + release green.

- [x] **[TPR-03-016] Recompute field summaries after the narrowing pass** (2026-03-26). After `run_narrowing_pass()`, the field_summary_table is cleared and all `Construct` instructions are re-processed with the final narrowed ranges. New test: `fixpoint_field_summary_uses_final_ranges`. Added `FieldSummaryTable::clear()`. Debug + release green.

- [x] **[TPR-03-017] Fix Switch case refinements to join instead of overwriting** (2026-03-26). In `process_terminator()`, replaced `block_refinements.insert()` with `.entry().and_modify(|e| *e = e.join(exact)).or_insert(exact)`. Semantic-pin test: `fixpoint_switch_multi_case_same_block_joins` — cases `{0 -> b1, 1 -> b1}` give b1 range `[0, 1]`, not `[1, 1]`. Debug + release green.

- [x] **[TPR-03-018] Add Switch default-block complement refinement** (2026-03-26). Added `switch_default_complement()` function that trims contiguous case values from scrutinee range edges (e.g., [0, 10] with cases {0, 1, 2} → [3, 10]). Semantic-pin test: `fixpoint_switch_default_gets_complement`. Debug + release green.

- [x] **[TPR-03-019] Extend narrowing pass to revisit block parameters and terminators** (2026-03-26). Extended `run_narrowing_pass()` to: (1) re-merge block params from predecessor Jump args with `narrow(widened, merged)`, (2) apply block refinements temporarily during narrowing body processing, (3) narrow Invoke terminator dst variables. Run 2 narrowing passes for loop variable recovery. Extracted `FixpointState` struct, `run_forward_iteration()`, and `recompute_field_summaries()` to keep functions under line limits. Semantic-pin test: `fixpoint_narrowing_recovers_loop_bound` — bounded loop `for i in 0..<10` narrows from `[0, MAX]` to `[0, 10]`. Debug + release green.

- [x] **[TPR-03-020] Fix Branch refinement overwrite and stale cross-iteration refinements** (2026-03-26). Two bugs fixed: (1) `process_terminator()` Branch arm changed from `insert()` to `.entry().and_modify(|e| *e = e.join(new)).or_insert(new)` — same pattern as Switch cases (TPR-03-017). (2) `block_refinements` map cleared at start of each `run_forward_iteration()` to prevent stale refinements from prior iterations. Semantic-pin test: `fixpoint_branch_multi_predecessor_refinement_joins` — two predecessors refine same variable for same block via different Branch terminators, verifies joined range covers both paths. Also extracted `narrowing.rs` submodule from `fixpoint/mod.rs` (486→486+157 lines, under 500 limit). Debug + release green, 14,155 tests pass.

- [x] **[TPR-03-021] Recompute `return_range` from final narrowed variable ranges** (2026-03-26). Added `recompute_return_range()` helper in `fixpoint/narrowing.rs` — walks all `Return` terminators and joins final narrowed variable ranges. Called after the 2 narrowing passes in `range_fixpoint()`. Semantic-pin test: `fixpoint_return_range_recomputed_after_narrowing` — bounded loop returning loop variable verifies `return_range` narrows to `[0, 10]` (not `[0, MAX]`). Debug + release green, 14,155 tests pass.

- [x] **[TPR-03-022] Post-recompute projection refresh pass** (2026-03-26) — after `recompute_field_summaries()`, added a final `run_narrowing_pass()` with the updated field_summary_table. Project instructions now re-transfer against the tightened field summaries, narrowing projection-derived variables from `[10, MAX]` to `[10, 10]` (precise exit-block range). Semantic-pin test: `fixpoint_projection_refreshed_after_field_summary_recompute` — bounded loop exit constructs struct from narrowed i, projects field 0, returns projection. Verifies: field summary = `[0, 10]`, projection = `[10, 10]`, return_range = `[10, 10]`. Debug + release green, 14,156 tests pass.

- [x] **Handoff to ReprPlan (§01 integration)** (2026-03-26): `range_fixpoint()` returns `RangeFixpointResult { var_ranges, field_summaries, return_range }`. The caller must flush all three into `ReprPlan`. The integration requires three storage additions:
  1. **Per-function range storage** (already live in `plan.rs`):
     ```rust
     /// Per-function, per-variable ranges from range analysis.
     /// Key: (function Name, ArcVarId) → ValueRange.
     /// Populated by §03, consumed by §04 (integer narrowing).
     function_var_ranges: FxHashMap<Name, FxHashMap<ArcVarId, ValueRange>>,
     ```
  2. **Field-summary storage** (new — required by §04): Add to `ReprPlan`:
     ```rust
     /// Per-field range summaries for struct/tuple types.
     /// Key: (type Idx, field index) → ValueRange.
     /// Each entry is the join of argument ranges across ALL Construct sites
     /// for that type/field combination across all analyzed functions.
     /// Populated by §03 (field_summary.rs), consumed by §04 (struct field narrowing).
     field_range_summaries: FxHashMap<(Idx, u32), ValueRange>,
     ```

  3. **Query methods** (var_range already live, add field_range):
     ```rust
     impl ReprPlan {
         /// Get the range for a variable in a function (from §03 range analysis).
         /// Already live in plan.rs:155.
         pub fn var_range(&self, func: Name, var: ArcVarId) -> ValueRange {
             self.function_var_ranges
                 .get(&func)
                 .and_then(|m| m.get(&var).copied())
                 .unwrap_or(ValueRange::Top)
         }

         /// Get the inferred range for a struct/tuple field across all construction sites.
         /// Returns Top if no construction sites have been analyzed for this field.
         pub fn field_range(&self, type_idx: Idx, field: u32) -> ValueRange {
             self.field_range_summaries
                 .get(&(type_idx, field))
                 .copied()
                 .unwrap_or(ValueRange::Top)
         }

         /// Record a field range observation from a Construct instruction.
         /// Joins with any existing range for this (type, field) pair.
         pub fn join_field_range(&mut self, type_idx: Idx, field: u32, range: ValueRange) {
             self.field_range_summaries
                 .entry((type_idx, field))
                 .and_modify(|existing| *existing = existing.join(range))
                 .or_insert(range);
         }
     }
     ```

- [x] **Unit tests for fixpoint loop** (2026-03-26) in `compiler/ori_repr/src/range/fixpoint/tests.rs` — 15 tests: widen (7), narrow (3), budget exceeded (1), constant let (1), return range join (1), block param merging (1), field summary integration (1). Both debug + release green. **TDD: write tests BEFORE implementing the fixpoint loop. Verify they fail. Then implement. Tests must pass unchanged.** Required coverage:
  - **Termination**: a function with a simple loop (block parameter back-edge) terminates within `max_iterations` (default 20). Verify iteration count is finite.
  - **Widening threshold**: a counter incremented in a loop without bound triggers widening at iteration `WIDEN_THRESHOLD + 1` (default 4). After widening, range includes `i64::MAX`. Semantic pin: change `WIDEN_THRESHOLD` and verify behavior changes.
  - **Narrowing pass recovery**: after widening pushes a loop counter to `[0, i64::MAX]`, the narrowing pass intersects with the transfer function output to recover a tighter bound (e.g., if the loop is `for i in 0..100`, narrowing should recover `[0, 99]`).
  - **Budget exceeded**: construct a function exceeding `max_blocks` (default 500) → returns all-Top result, does not hang.
  - **Block parameter merging**: construct a function with a merge point (two predecessors jumping to the same block with different argument ranges) → merged range is the join of both. Semantic pin: `jump(arg=[0,5])` + `jump(arg=[10,20])` → merged range `[0, 20]`.
  - **Return range collection**: function with two `Return` terminators returning different bounded values → `return_range` is the join.
  - **Field summary integration**: function with a `Construct` instruction → `field_summary_table` is populated after fixpoint completes.
  - **Invoke terminator**: function with an `Invoke` (calling `len`) → `dst` variable gets range `[0, i64::MAX]`.
  - **Both debug and release**: `cargo test -p ori_repr` (debug) and `cargo test -p ori_repr --release` (release) must both pass

---

## 03.4 Conditional Range Refinement

**File(s):** `compiler/ori_repr/src/range/conditional.rs`

When code branches on a comparison (e.g., `if x < 100`), the true branch knows `x ∈ [-2⁶³, 99]` and the false branch knows `x ∈ [100, 2⁶³-1]`. This is the most powerful source of narrowing information.

- [x] Implement conditional range extraction:
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
          // Each remaining comparison operator follows the same structural pattern:
          // match on PrimOp::Binary(BinaryOp::X), extract x and y, check if y is constant,
          // then compute true_range and false_range per the table in the next checklist item.
          // Implement as separate match arms (not a single generic arm) for clarity.
          //
          _ => vec![], // can't extract info — return empty (safe)
      }
  }
  ```

- [x] Implement refinement for all 6 comparison operators (`Lt`, `LtEq`, `Gt`, `GtEq`, `Eq`, `NotEq`):
  - `x < c` → true: `[lo, c-1]`, false: `[c, hi]`
  - `x <= c` → true: `[lo, c]`, false: `[c+1, hi]`
  - `x > c` → true: `[c+1, hi]`, false: `[lo, c]`
  - `x >= c` → true: `[c, hi]`, false: `[lo, c-1]` (common for non-negative checks: `x >= 0`)
  - `x == c` → true: `[c, c]`, false: current range minus `c` (conservative: keep current range)
  - `x != c` → true: current range (conservative), false: `[c, c]`
  - Each operator must handle `c - 1` / `c + 1` overflow (checked arithmetic; fallback to Top on overflow)
  - **Bidirectional refinement:** When the comparison is `x < y` and BOTH x and y are variables (not constants), refine both: true branch gets `x ∈ [x_lo, min(x_hi, y_hi - 1)]` and `y ∈ [max(y_lo, x_lo + 1), y_hi]`. Conservative: implement constant-only first, extend to variable-variable in a follow-up if needed.
- [x] **Unit tests for conditional refinement** in `range/tests.rs`. **TDD: write tests BEFORE implementing the remaining 5 operators. The `Lt` arm exists in the code sketch — write one test for it first, verify it passes, then write tests for the remaining 5 and verify they fail, then implement.** Required coverage:
  - One test per comparison operator (6 total: `Lt`, `LtEq`, `Gt`, `GtEq`, `Eq`, `NotEq`), each with:
    - (a) x has a bounded range `[0, 200]` and y is constant `100` — verify true and false ranges match the table above
    - (b) x at boundary: `c = i64::MIN` for `x < c` (true_range becomes Bottom since no value < `i64::MIN`), `c = i64::MAX` for `x > c` (true_range becomes Bottom) — verify overflow in `c - 1` / `c + 1` produces Top fallback, not panic
    - (c) cond defined in predecessor block (not found in body) → empty refinement list
    - (d) `x == i64::MIN` → true_range `[i64::MIN, i64::MIN]`, false_range is full range (conservative)
  - **Cross-pattern coverage**: condition is a non-comparison instruction (e.g., `IsShared`) → empty refinement list
  - **Semantic pin**: `x < 100` with `x ∈ [0, 200]` → true: `[0, 99]`, false: `[100, 200]` — this test ONLY passes with correct `Lt` refinement

- [x] **[TPR-03-023] Fix `recompute_return_range()` to iterate only reachable blocks** (2026-03-26): Passed `rpo: &[usize]` to `recompute_return_range()` in `narrowing.rs`, updated call site in `fixpoint/mod.rs:492`. Added 2 regression tests: `fixpoint_return_range_excludes_unreachable_blocks` (semantic pin — unreachable Return no longer pollutes return_range) and `fixpoint_return_range_includes_all_reachable_returns` (edge case — all reachable returns still contribute). 304/304 debug + release green. 14,159 total tests passing.

- [x] **[TPR-03-024] Add `ArcTerminator::Invoke` regression test** (2026-03-26): Added `fixpoint_invoke_defines_dst_variable` test in `fixpoint/tests.rs` — constructs a function with `Invoke` terminator, verifies `dst` variable gets `Top` range for unknown function and `return_range` is `Top`. Test passes (Invoke handling already implemented in fixpoint loop — only test coverage was missing). 304/304 debug + release green.

---

## 03.5 Function Signature Range Propagation

**File(s):** `compiler/ori_repr/src/range/signatures.rs`

**Implementation order:** §03.5 MUST be implemented after §03.1-§03.4 are stable and passing all tests. The intraprocedural analysis (§03.1-§03.4) is a required prerequisite and should be fully verified before interprocedural propagation is added. However, §03.5 is not optional — without it, function parameters can never be narrowed (since their ranges are always `Top` intraprocedurally), and struct field narrowing across module boundaries is impossible. Both of these are core mission goals.

**Risk:** Interprocedural fixpoint over SCCs is quadratic in the worst case (SCC size x iterations x function size). The budget caps mitigate this, but testing with real programs is essential before merging.

For cross-function narrowing, we need to propagate range information through function signatures.

- [x] Define `FunctionRangeInfo` (2026-03-26): `ParamRange` and `FunctionRangeInfo` types in `compiler/ori_repr/src/range/signatures/mod.rs`. Includes `new_bottom()` and `new_top()` constructors.

- [x] Implement call-site range collection (2026-03-26): `collect_param_ranges()` scans all functions for `Apply`/`Invoke` targeting the callee, joining argument ranges via `join_arg_ranges()`. Parameters with no internal callers get `Top` (safe fallback for externally-callable functions).

- [x] **Recursive function fixpoint algorithm** (2026-03-26): `propagate_ranges()` entry point implements the full SCC-based pipeline:
  1. Intraprocedural `range_fixpoint()` for each function (Phase 1)
  2. `CallGraph::build()` + `compute_sccs()` for SCC decomposition (Phase 2)
  3. Forward topological processing: non-recursive single pass, recursive iterate-to-fixpoint (Phase 3)
  4. Store results in `ReprPlan` + merge interprocedural param ranges (Phases 4-5)
  - Budget: `max_scc_iterations` per SCC, `max_total_scc_iterations` across all SCCs
  - Exceeded budget widens to Top (safe fallback)
  - `ReprPlan::function_var_ranges_mut()` added for interprocedural parameter merge

- [x] **[TPR-03-026] Implement parameter-seeded intraprocedural analysis** (2026-03-26) — Added `initial_param_ranges: Option<&FxHashMap<ArcVarId, ValueRange>>` parameter to `range_fixpoint()`. Seeds initialize entry block parameter vars before the fixpoint loop. Both `propagate_ranges()` (Phase 3 non-recursive) and `process_recursive_scc()` pass collected param ranges as seeds. Fixed narrowing pass to skip entry block params with no predecessors (prevents `narrow(seed, Bottom) = Bottom` from destroying interprocedural seeds). Changed SCC processing to reverse topological order (callers first → callees last) for correct top-down parameter propagation.
- [x] **[TPR-03-026] Propagate callee return ranges to caller call-result variables** (2026-03-26) — Added Phase 6 (`propagate_return_ranges()`) to `propagate_ranges()`. For each `Apply`/`Invoke`, narrows the caller's `dst` variable via `meet` with `func_infos[callee].return_range`. Helper `narrow_dst_from_return()` handles the per-instruction narrowing. Callers now benefit from callee return-range analysis.
- [x] **[TPR-03-026] Add regression test: transitive A→B→C propagation** (2026-03-26) — `transitive_propagation_a_b_c`: A calls B(42), B calls C(x). Asserts C's param = [42, 42]. Semantic pin: ONLY passes with parameter seeding + reverse topological SCC order. Debug + release green.
- [x] **[TPR-03-026] Add regression test: mutually recursive SCC tightening from external seed** (2026-03-26) — `mutually_recursive_scc_tightens_from_seed`: F↔G mutual recursion, main(F(10)). Asserts F and G params are non-Top. Debug + release green.
- [x] **[TPR-03-027] Add caller/callee return-range narrowing test** (2026-03-26) — `caller_dst_narrows_from_callee_return_range`: callee returns 99, caller's Apply dst narrows to [99, 99]. Semantic pin: ONLY passes with Phase 6 return-range propagation. Debug + release green.
- [x] **[TPR-03-006] Implement builtin name matching in `transfer_known_call()`** (2026-03-26) — Added `KnownBuiltins` struct (pre-interned `Name` values for len/count/byte_to_int/char_to_int/abs) to `RangeAnalysisConfig`. `transfer_known_call()` now matches against builtins and returns bounded ranges. Added `known_builtins` field to `TransferContext`. Threaded through fixpoint, narrowing, and all callers. `KnownBuiltins::from_interner()` populates from real compiler interner; default is all-None (conservative). Debug + release green, 313 tests pass.
- [x] **[TPR-03-028] Clear stale `results` on SCC budget exhaustion** (2026-03-26): In `process_recursive_scc()`, when budget trips, now also clears `results` entries for all SCC members (empty var_ranges, Top return_range, empty field_summaries). Semantic pin test `scc_budget_exhaustion_clears_stale_results` forces budget with `max_scc_iterations = 1`, verifies v_c (constant 42) is Top and param is Top. Updated `mutually_recursive_scc_tightens_from_seed` to accept Top (SCC doesn't converge within budget). Debug + release green.
- [x] **[TPR-03-030] Feed callee return ranges back into `results` before parameter collection** (2026-03-26): Added Phase 3.5 return-range feedback pass (`feedback.rs`). Step 1: narrows caller dst vars from callee return ranges in `results`. Step 2: re-collects params and re-runs fixpoint for functions with changed seeds (forward topo order). Removed redundant Phase 6 (`propagate_return_ranges`). Semantic pin test `return_range_feeds_downstream_parameter_collection`: A calls helper() (returns [99,99]), passes to C — C's param narrows to [99,99]. Debug + release green.
- [x] **[TPR-03-031] Iterate return-range feedback to multi-hop fixpoint** (2026-03-26): Rewrote `feed_return_ranges_and_reprocess()` in `feedback.rs` with three fixes: (1) Outer loop iterates Steps 1+2 until convergence, bounded by `config.max_feedback_iterations` (default 5). (2) Step 1b (`refresh_return_ranges()`) recomputes `func_infos` return ranges from updated `results` after dst var narrowing — without this, narrowed return values don't propagate back up the call chain. (3) Step 2 iterates SCCs in reverse topological order (callers first), matching Phase 3's order, so parameter seeds cascade from callers to callees in one pass. Added `max_feedback_iterations` field to `RangeAnalysisConfig`. 336/336 debug + release green, 14,191 total green.
- [x] **[TPR-03-031] Add semantic-pin test: multi-hop return-range chain** (2026-03-26): `multi_hop_return_range_chain` — 4-hop chain: `helper(800)` returns [99,99], `A(804)` calls helper and passes to `B(803)`, B passes to `C(802)`, C passes to `D(801)`. Asserts D's param = [99,99], C's param = [99,99], B's param = [99,99]. Semantic pin: ONLY passes with iterated feedback — fails with single-pass (D gets Top).
- [ ] Handle boundary cases for parameter ranges:
  - `@main(args:)`: the `args` list length is `[0, i64::MAX]`; the `args` parameter itself is not an int (skip) — **currently handled: non-int params are skipped by `is_int_typed()` check**
  - Trait method parameters: assign Top (callers unknown at compile time — may be called via dynamic dispatch) — **blocked: ARC IR lacks visibility/trait info; currently all functions treated as narrowable (conservative — §04 ignores Top anyway)**
  - Closure parameters: assign Top unless all call sites of the closure are visible in the current module (conservative default) — **blocked: same visibility limitation**
  - `pub` function parameters: assign Top (external callers may pass full-range values) — **blocked: same visibility limitation**

- [x] **Unit tests for §03.5** in `range/signatures/tests.rs` (2026-03-26). Tests written TDD-style (verified fail before implementation). Coverage:
  - [x] Non-recursive function called with constant args → parameter range is `Bounded(const, const)` — `single_call_site_constant_arg` (semantic pin)
  - [x] Non-recursive function called with different constant args from 2 sites → parameter range is join — `two_call_sites_join_param_ranges`
  - [ ] `pub` function → parameter range remains Top regardless of call-site args — **blocked: no visibility info in ARC IR**
  - [ ] Trait method parameters → Top (callers unknown at compile time) — **blocked: no trait info in ARC IR**
  - [ ] Closure parameters → Top (conservative default) — **blocked: no closure distinction in ARC IR**
  - [x] Self-recursive function (SCC of size 1) → converges within `max_scc_iterations` — `self_recursive_converges_or_widens`
  - [x] Mutually recursive pair (SCC of size 2) → parameter ranges stabilize or widen to Top — `mutually_recursive_scc_tightens_from_seed`
  - [x] Return range propagation: function returning constant → callee-local return var has bounded range — `return_range_constant`
  - [x] Return range propagation: callers of a function with bounded return range use that bound instead of Top — `caller_dst_narrows_from_callee_return_range`
  - [x] Transitive A→B→C propagation — `transitive_propagation_a_b_c` (semantic pin)
  - [x] Budget exceeded: >50 total SCC iterations → remaining SCCs get Top (not hang, not panic) — `budget_exceeded_gives_top`
  - [x] **Semantic pin**: private function `@helper(x: int)` called only as `helper(x: 42)` → parameter range `[42, 42]` — `single_call_site_constant_arg`
  - [x] **Both debug and release**: 313/313 debug + release green
  - [x] Empty function list → no panic — `empty_functions_no_panic`

---

## 03.6 Completion Checklist

**FIRST:** Change `pub(crate)` → `pub` for `compute_postorder`, `successor_block_ids`, `compute_predecessors` in `compiler/ori_arc/src/graph/mod.rs` (lines 32, 53, 122). Run `cargo c`. Only then proceed.

**Implementation order:** 03.1 (lattice) → 03.2 (transfer functions) → 03.2b (field-summary infrastructure) → 03.4 (conditional refinement) → 03.3 (fixpoint loop) → 03.5 (interprocedural — implement after 03.1-03.4 are stable and passing tests). Each step must pass tests before proceeding to the next.

**Test matrix for §03 (required — write tests first, verify they fail, then implement):**

| Input pattern | Expected non-Top result | Semantic pin |
|---------------|------------------------|--------------|
| `let x = 42` | `Bounded(42, 42)` | Yes — exact constant |
| `let x = -1` | `Bounded(-1, -1)` | Yes — negative constant |
| `let x = -128` | `Bounded(-128, -128)` | Yes — i8 minimum boundary |
| `for i in 0..100` | `Bounded(0, 99)` | Yes — loop counter |
| `for i in 0..=100` | `Bounded(0, 100)` | Yes — inclusive range |
| `for i in 0..0` (empty range) | Bottom or no iteration | Yes — empty loop |
| `let n = len(list)` | `Bounded(0, i64::MAX)` | Yes — len is non-negative |
| `let b = byte_to_int(b'A')` | `Bounded(0, 255)` | Yes — byte range |
| `let c = char_to_int('A')` | `Bounded(0, 0x10FFFF)` | Yes — char range |
| `let x = a + b` where a,b in `[0,10]` | `Bounded(0, 20)` | Yes — add propagation |
| `let x = a * b` where a in `[2,3]`, b in `[4,5]` | `Bounded(8, 15)` | Yes — mul propagation |
| `let x = a * b` where a in `[-3,-2]`, b in `[4,5]` | `Bounded(-15, -8)` | Yes — negative mul |
| `let x = a / 0` | `Top` (don't panic) | Yes — division safety |
| `let x = i64::MAX + 1` (overflow) | `Top` (checked_add overflow) | Yes — arithmetic overflow safety |
| `if x < 100 then { x ... }` branch | x refined to `Bounded(.., 99)` in true branch | Yes — conditional |
| `if x >= 0 then { x ... }` branch | x refined to `Bounded(0, ..)` in true branch | Yes — non-negative check |
| Function parameter at non-public call site with constant arg | `Bounded(const, const)` | Yes — §03.5 interprocedural |
| `pub` function parameter | `Top` (cannot narrow) | Yes — ABI boundary |
| Trait method parameter | `Top` (dynamic dispatch) | Yes — §03.5 boundary |
| `Pixel { r: 0, g: 128, b: 255, a: 0 }` + `Pixel { r: 255, g: 0, b: 0, a: 255 }` | field_range(Pixel, 0..3) = `Bounded(0, 255)` | Yes — §03 to §04 field summary |
| `Project` on field with known summary | field range (not Top) | Yes — Project reads field summary |
| `Select` with true `[0,5]` and false `[10,20]` | `Bounded(0, 20)` | Yes — Select join |
| Function with >500 blocks | all Top (budget skip) | Yes — budget safety |

- [x] `ValueRange` lattice correctly implements join, meet, fits_in, min_width, is_constant, overlaps (in `range/mod.rs`); `widen` and `narrow` free functions correct (in `range/fixpoint/mod.rs`) — verified by 58 lattice tests + 7 widen/narrow tests (2026-03-25)
- [x] Arithmetic transfer functions implemented: `range_add`, `range_sub`, `range_mul`, `range_div`, `range_mod`, `range_floordiv`, `range_neg` (PrimOp dispatched); bitwise: `range_bitand`, `range_bitor`, `range_bitxor`, `range_shl`, `range_shr`, `range_bitnot`; built-in function ranges: `range_len`, `range_count`, `range_byte_to_int`, `range_char_to_int`, `range_abs` — verified by 46 transfer tests (2026-03-25)
- [x] Top-level `transfer()` dispatcher has an arm for every `ArcInstr` variant — exhaustive match (no `_` arm). Verified against `ori_arc/src/ir/instr.rs` (2026-03-25)
- [x] Fixpoint loop handles all `ArcTerminator` variants (7: `Return`, `Jump`, `Branch`, `Switch`, `Invoke`, `Resume`, `Unreachable`). Exhaustive match in `terminator.rs` (2026-03-26)
- [x] `transfer_primop()` has an arm for every `BinaryOp` (23) and `UnaryOp` (4) variant — exhaustive match, no `_` arm (2026-03-25)
- [x] Fixed-point iteration terminates within `max_iterations` for all test programs — verified by `fixpoint_budget_exceeded` and `fixpoint_constant_let_exact_range` tests (2026-03-26)
- [x] Block parameters (`ArcBlock::params`) processed at start of each block via `merge_block_params()` (2026-03-26)
- [x] Block terminators propagate conditional refinements via `process_terminator()` → `refine_from_branch()` (2026-03-26)
- [x] `ArcTerminator::Invoke` handled — `fixpoint_invoke_defines_dst_variable` test (2026-03-26)
- [x] Conditional refinement for all 6 comparison operators — 20+ tests in `range/tests.rs` (2026-03-26)
- [x] Function signature propagation: parameter seeding + return-range propagation — 9 tests in `signatures/tests.rs` (2026-03-26)
- [x] `Construct` populates field-summary table — `field_summary_*` tests (2026-03-26)
- [x] `Project` consults field-summary — `fixpoint_projection_refreshed_after_field_summary_recompute` test (2026-03-26)
- [x] Recursive SCC fixpoint with bounded iterations — `self_recursive_converges_or_widens`, `mutually_recursive_scc_tightens_from_seed` tests (2026-03-26)
- [x] `let x = 42` → `[42, 42]` — `fixpoint_constant_let_exact_range` test (2026-03-26)
- [x] `for i in 0..100` → `[0, 99]` — `fixpoint_narrowing_recovers_loop_bound` test (2026-03-26)
- [x] `len(list)` → `[0, i64::MAX]` — `transfer_apply_len_builtin` test (2026-03-25)
- [x] Pixel field-summary `[0, 255]` — `field_summary_semantic_pin_pixel` test (2026-03-26)
- [x] `./test-all.sh` green — 14,188 passed, 0 failed (2026-03-26)
- [x] `./clippy-all.sh` green (2026-03-26)
- [x] Tracing: `ORI_LOG=ori_repr=debug` shows range computations for each function — `tracing::debug!` in `range_fixpoint()` logs function name, iteration count, non-Top count (2026-03-26)
- [x] `#[tracing::instrument(skip_all)]` on `range_fixpoint()`, `transfer()`, and `refine_from_branch()` (2026-03-26)
- [x] `tracing::debug!` at function entry/exit in `range_fixpoint()` — exit logging at line 466 with func name, iterations, non_top count. Entry covered by `#[instrument]` span (2026-03-26)
- [x] `tracing::trace!` per-variable range updates in `update_range()` (2026-03-26) — logs var index, old range, new range at trace level
- [x] **Add `proptest` dev-dependency** to `compiler/ori_repr/Cargo.toml` (2026-03-26)
- [x] **Property-based tests** (proptest) in `range/tests.rs::proptest_range` — 20 tests (2026-03-26):
  - Lattice laws: join/meet commutative, associative, idempotent, identity, absorbing, containment, absorption
  - Transfer soundness: add/sub/mul/neg corner-value containment
  - Widening: `widen(a, a)` stable, `widen_contains_current`, expanding chain terminates
  - Narrowing: `narrow(widened, computed) ⊆ widened`
- [x] Range results written into `ReprPlan::function_var_ranges` via `propagate_ranges()` → Phase 4 (2026-03-26)
- [x] Field-summary results flushed via `FieldSummaryTable::flush_to_repr_plan()` in Phase 4 (2026-03-26)
- [x] Return ranges collected and available in `RangeFixpointResult::return_range` — used by §03.5 return-range propagation (2026-03-26)
- [x] `ReprPlan::field_range(type_idx, field)` query method — `plan.rs:196` (2026-03-25)
- [x] `ReprPlan::join_field_range(type_idx, field, range)` writer method — `plan.rs:182` (2026-03-25)
- [x] `transfer()` uses `TransferContext` struct — carries `ranges`, `pool`, `var_types`, `field_summaries`, `known_builtins` (2026-03-26)
- [x] Unknown/unsupported ArcInstr patterns degrade to `Top` — exhaustive match, no panics (2026-03-25)
- [x] `compute_postorder()`, `successor_block_ids()`, and `compute_predecessors()` in `ori_arc::graph::mod.rs` changed from `pub(crate)` to `pub` (2026-03-25)
- [x] **`analyze_ranges()` wired up** in `lib.rs` — calls `propagate_ranges()` with default config (2026-03-26)
- [x] **[TPR-03-029] Wire `StringInterner` into `analyze_ranges()` and populate `KnownBuiltins`** (2026-03-26): Added `compute_repr_plan_with_interner()` that accepts `Option<&StringInterner>`. Updated both callers (codegen_pipeline.rs, compile.rs) to pass `Some(interner)`. `analyze_ranges()` now populates `config.known_builtins` via `KnownBuiltins::from_interner()` when interner is available. Original `compute_repr_plan()` delegates with `None` for backward compatibility. 14,190 tests green.
- [x] **`field_range_summaries` field in `ReprPlan`** — `plan.rs:101`, with `field_range()` and `join_field_range()` methods (2026-03-25)
- [x] **`.copied()` in `ReprPlan::var_range()`** — already uses `.copied()` at `plan.rs:162` (2026-03-25)
- [x] **`pub use` re-exports in `lib.rs`** — `ValueRange`, `RangeAnalysisConfig`, `FieldSummaryTable`, `RangeFixpointResult`, `KnownBuiltins` (2026-03-26)

**Global Testing Requirements (CLAUDE.md compliance):**
- **TDD ordering**: Every subsection (03.1 through 03.5) must write tests BEFORE implementation. Verify tests fail (compile error or assertion). Implement. Tests must pass unchanged. Needing to change tests = wrong tests or wrong fix.
- **Debug AND release**: All tests must pass under both `cargo test -p ori_repr` (debug) and `cargo test -p ori_repr --release` (release). FastISel behavior differs between debug and release; range analysis must be correct in both.
- **Semantic pins**: Each subsection has at least one semantic pin test that ONLY passes with the new semantics. These are permanent regression guards — they must never be removed or weakened.
- **Matrix completeness**: The test matrix above covers every input pattern x expected outcome. Missing cells = future regressions. If implementation reveals new patterns not in the matrix, add them.
- **`./test-all.sh` green**: Range analysis is additive. No existing tests may break. Run `./test-all.sh` after each subsection lands.

**Performance Budget:**
- `RangeAnalysisConfig` is defined in §03.1 (moved earlier because §03.3 fixpoint needs it).
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

---

## 03.R Third Party Review Findings

- [x] `[TPR-03-031][medium]` `compiler/ori_repr/src/range/signatures/feedback.rs:61` — The new return-range feedback pass still stops after one hop and can discard injected call-result facts when a function is reprocessed.
  Evidence: Step 1 only narrows immediate caller `dst` variables once (`compiler/ori_repr/src/range/signatures/feedback.rs:33-55`). Step 2 then does a single pass over `sccs` and reruns only functions whose parameter seeds changed (`compiler/ori_repr/src/range/signatures/feedback.rs:61-84`). When that rerun happens, `results.insert(*name, result)` overwrites the earlier injected `dst` ranges with a fresh `range_fixpoint()` result that has no callee-return propagation path. There is no outer loop that reapplies Step 1 after those reruns, even though `propagate_ranges()` now persists `results` directly into `ReprPlan` (`compiler/ori_repr/src/range/signatures/mod.rs:169-205`).
  Impact: the current TPR-03-030 fix only pins the one-hop `helper() -> caller -> callee` case. Longer return-forwarding chains still collapse to conservative `Top`, and the section's current "resolved" TPR framing overstates completion.
  Required plan update: iterate return-range feedback to a real fixpoint (or fold callee-return propagation into the seeded SCC solver), preserve injected caller `dst` facts across reruns, and add a semantic-pin regression for a multi-hop chain such as `main -> A(helper()) -> C -> D`.
  Resolved: Validated and accepted on 2026-03-26. Two implementation tasks added to §03.5: iterate feedback to fixpoint + multi-hop semantic-pin test.

- [x] `[TPR-03-028][high]` `compiler/ori_repr/src/range/signatures/mod.rs:420` — Recursive-SCC budget exhaustion claims to widen to `Top`, but the last partially converged `results` are still persisted into `ReprPlan`.
  Evidence: when `iteration >= config.max_scc_iterations`, `process_recursive_scc()` only overwrites `func_infos` with `FunctionRangeInfo::new_top(...)` and then breaks (`compiler/ori_repr/src/range/signatures/mod.rs:420-432`). It never replaces the already-computed `results`. Phase 4 immediately stores those stale `results` in `ReprPlan` (`compiler/ori_repr/src/range/signatures/mod.rs:167-173`), and Phase 5 only rewrites parameter vars, leaving all other vars from the aborted iteration intact (`compiler/ori_repr/src/range/signatures/mod.rs:175-193`).
  Impact: if an SCC hits the iteration cap, §04 can consume under-converged local/field ranges even though the implementation reports a conservative `Top` fallback. That is an unsafe budget fallback for a shared analysis result.
  Required plan update: when the SCC budget trips, replace each member's stored `RangeFixpointResult` with a fully conservative result before Phase 4 persists it, and add a regression test that forces a recursive SCC to hit the budget and verifies all exported ranges stay `Top`.
  Resolved: Validated and accepted on 2026-03-26. Implementation task added to §03.5 (clear stale `results` on SCC budget exhaustion + regression test).

- [x] `[TPR-03-029][medium]` `compiler/ori_repr/src/lib.rs:183` — The real compiler path never populates `KnownBuiltins`, so builtin call ranges silently degrade to `Top`.
  Evidence: `analyze_ranges()` constructs `RangeAnalysisConfig::default()` and passes it unchanged into `propagate_ranges()` (`compiler/ori_repr/src/lib.rs:183-185`). The default config sets `known_builtins` to `KnownBuiltins::default()` (`compiler/ori_repr/src/range/mod.rs:231-239`), which leaves every builtin name as `None` (`compiler/ori_repr/src/range/mod.rs:248-270`). `transfer_known_call()` only returns builtin ranges when those names are populated (`compiler/ori_repr/src/range/transfer/mod.rs:138-162`). A repo-wide search shows `KnownBuiltins::from_interner()` is never called.
  Impact: real `Apply`/`Invoke` calls to `len`, `count`, `byte_to_int`, `char_to_int`, and `abs` lose their known ranges in production even though the helper-level unit tests pass, which weakens §03 and downstream narrowing materially.
  Required plan update: thread the real interner into the §03 entry point, populate `config.known_builtins` before calling `propagate_ranges()`, and add an end-to-end regression that exercises a builtin call through the actual range-analysis pipeline.
  Resolved: Validated and accepted on 2026-03-26. Implementation task added to §03.6 (wire `StringInterner` into `analyze_ranges()` and populate `KnownBuiltins`).

- [x] `[TPR-03-030][medium]` `compiler/ori_repr/src/range/signatures/mod.rs:167` — Callee return-range propagation only patches `ReprPlan` after SCC processing, so downstream parameter collection still misses call-result chains.
  Evidence: `collect_param_ranges()` reads argument facts only from `results[caller].var_ranges` (`compiler/ori_repr/src/range/signatures/mod.rs:213-319`). `propagate_return_ranges()` runs later and mutates only `ReprPlan`, not `results` (`compiler/ori_repr/src/range/signatures/mod.rs:167-199`, `322-379`). There is therefore no path that lets a narrowed call result in function `B` become the argument fact that `collect_param_ranges()` sees when `B` calls `C`.
  Impact: the current §03.5 implementation still fails on interprocedural chains where a caller forwards a callee result rather than an incoming parameter. The section's current tests only pin direct-parameter forwarding and direct caller-side narrowing, not this downstream handoff.
  Required plan update: feed callee return summaries back into the solver state used by `collect_param_ranges()` and add a semantic-pin regression where `A` calls `B`, `B` forwards `helper()`'s bounded return to `C`, and `C`'s parameter narrows accordingly.
  Resolved: Validated and accepted on 2026-03-26. Implementation task added to §03.5 (feed return ranges into `results` before `collect_param_ranges()` + regression test for A→helper()→C chain).

- [x] `[TPR-03-026][high]` `compiler/ori_repr/src/range/signatures/mod.rs:167` — The checked-off §03.5 SCC pipeline never feeds interprocedural facts back into the analysis, so recursive and transitive range propagation are still inert.
  Evidence: `propagate_ranges()` stores the plain intraprocedural `range_fixpoint()` results in `results` and only mutates `ReprPlan` parameter entries afterward (`compiler/ori_repr/src/range/signatures/mod.rs:159-185`). `collect_param_ranges()` always reads call arguments from `results[caller].var_ranges`, not from the narrowed plan or any seeded parameter state (`compiler/ori_repr/src/range/signatures/mod.rs:219-304`). Inside `process_recursive_scc()`, the rerun step still calls `range_fixpoint(func, pool, config)` with no parameter constraints at all despite the checked-off plan item claiming an SCC fixpoint over parameter and return ranges (`compiler/ori_repr/src/range/signatures/mod.rs:350-367`, `plans/repr-opt/section-03-range-analysis.md:1004-1011`).
  Impact: the current §03.5 code only patches final parameter vars for direct call sites. It does not propagate narrowed arguments through helper chains, does not refine recursive returns from seeded parameter ranges, and cannot deliver the interprocedural fixpoint the section now presents as implemented.
  Required plan update: add a seeded intraprocedural entry state for parameter ranges, rerun the solver against those seeds inside SCC iteration, and propagate the resulting return summaries back into caller `Apply`/`Invoke` destinations. Add regression tests for `A -> B -> C` transitive propagation and a mutually recursive SCC that actually tightens from an external constant call.
  Resolved: Validated and accepted on 2026-03-26. Three implementation tasks added to §03.5: parameter-seeded fixpoint, return-range propagation to callers, and transitive/SCC regression tests.

- [x] `[TPR-03-027][medium]` `plans/repr-opt/section-03-range-analysis.md:1028` — The checked-off §03.5 return-propagation coverage claim is false; the current test only checks a callee-local constant, not caller-side narrowing.
  Evidence: the plan says `return_range_constant` proves "callers see bounded return range" (`plans/repr-opt/section-03-range-analysis.md:1028`), but the test constructs only one standalone function and asserts its own `v_ret` range (`compiler/ori_repr/src/range/signatures/tests.rs:187-217`). The implementation likewise has no code path that rewrites a caller's `Apply`/`Invoke` destination from a callee `return_range`; phase 5 only merges parameter ranges (`compiler/ori_repr/src/range/signatures/mod.rs:167-185`).
  Impact: the section overstates both coverage and behavior for the caller-side half of §03.5, which makes the current work look safer and more complete than it is.
  Required plan update: replace or supplement `return_range_constant` with a caller/callee regression that asserts the caller's call-result variable narrows from the callee's bounded return summary, and leave the checklist unchecked until that path exists.
  Resolved: Validated and accepted on 2026-03-26. Test claim unchecked, implementation task for caller-side return-range propagation added to §03.5 (shared with TPR-03-026).

- [x] `[TPR-03-023][medium]` `compiler/ori_repr/src/range/fixpoint/narrowing.rs:149` — `recompute_return_range()` walks every block in the function, so unreachable `Return` terminators can widen `return_range` back to `Top`.
  Evidence: `range_fixpoint()` computes RPO from `compute_postorder(func)` (`compiler/ori_repr/src/range/fixpoint/mod.rs:421`), and `compute_postorder()` explicitly visits only blocks reachable from the entry (`compiler/ori_arc/src/graph/mod.rs:114`). But `recompute_return_range()` ignores that reachability set and iterates `for block in &func.blocks`, then falls back to `Top` when a dead block's return value was never analyzed (`compiler/ori_repr/src/range/fixpoint/narrowing.rs:149-154`). That lets dead returns pollute the final summary even though the forward and narrowing passes skipped those blocks.
  Impact: `RangeFixpointResult::return_range` can become less precise solely because of unreachable code, which undermines the TPR-03-021 fix and will block §03.5 from reusing precise return summaries at call sites.
  Required plan update: recompute `return_range` over the reachable block set only (for example, reuse the existing RPO indices or a reachable bitset) and add a regression test with an unreachable return block that would currently force `return_range` to `Top`.
  Resolved: Validated and accepted on 2026-03-26. Implementation task added to §03.3.

- [x] `[TPR-03-024][low]` `plans/repr-opt/section-03-range-analysis.md:894` — The checked-off §03.3 test inventory still claims Invoke coverage, but `compiler/ori_repr/src/range/fixpoint/tests.rs` contains no `ArcTerminator::Invoke` test at all.
  Evidence: the completed §03.3 test bullet says the fixpoint test file covers `Invoke terminator` handling (`plans/repr-opt/section-03-range-analysis.md:894-903`), yet a direct search of `compiler/ori_repr/src/range/fixpoint/tests.rs` finds no `ArcTerminator::Invoke` construction. The current suite exercises `Apply`-based `Top` flows, but not the only terminator that defines a value outside the block body.
  Impact: a core fixpoint path is currently unpinned even though the plan presents that coverage as done, so regressions in terminator-defined range propagation could land silently.
  Required plan update: add an explicit `Invoke` regression test in `compiler/ori_repr/src/range/fixpoint/tests.rs` and update the §03.3 test summary so it matches the real matrix.
  Resolved: Validated and accepted on 2026-03-26. Implementation task added to §03.3.

- [x] `[TPR-03-025][low]` `compiler/ori_repr/src/range/fixpoint/mod.rs:1` — The extracted fixpoint module is still 502 lines, so the recent split did not actually satisfy the repository’s 500-line non-test file limit.
  Evidence: `wc -l compiler/ori_repr/src/range/fixpoint/mod.rs` reports 502 lines in the current tree, which exceeds the hard limit in `.claude/rules/impl-hygiene.md`.
  Impact: this is a direct hygiene-rule violation in the file that was just refactored to get back under the limit, so the section history now overstates compliance and future edits will keep accumulating in an already oversized module.
  Required plan update: extract another small helper or orchestration fragment from `fixpoint/mod.rs` so the module is actually below 500 lines, then keep the plan note aligned with the resulting layout.
  Resolved: Rejected on 2026-03-26. The 500-line rule explicitly excludes test code. The file has 500 lines of implementation code and 2 lines of `#[cfg(test)] mod tests;` declaration — exactly at the limit but not exceeding it. The finding is factually incorrect about a violation existing.

- [x] `[TPR-03-022][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:452` — Field summaries are recomputed after narrowing, but projection-derived variables never get a second pass over the repaired summaries, so `Project` results and `return_range` can stay widened.
  Evidence: `run_narrowing_pass()` narrows body instructions against the pre-recompute `field_summary_table` (`compiler/ori_repr/src/range/fixpoint/narrowing.rs:65-88`), then `range_fixpoint()` calls `recompute_field_summaries()` and immediately finalizes `return_range` without rerunning any projection-dependent transfer (`compiler/ori_repr/src/range/fixpoint/mod.rs:452-476`). Fresh validation with an external bounded-loop probe (`i` narrows to `[0, 10]`, exit block does `Construct { args: [i] }` then `Project field 0`) produced `field = Bounded { lo: 0, hi: 10 }` but `y = Bounded { lo: 10, hi: 9223372036854775807 }` and matching widened `return_range`.
  Impact: §03 still loses the narrowed value on common `Construct` → `Project` paths, so §04 field consumers and the §03.5 return-summary handoff can observe pre-narrowing ranges even after TPR-03-016/021 landed. The current fixpoint tests only check the repaired field summary table itself; they do not pin projection or return propagation through that table.
  Required plan update: after `recompute_field_summaries()`, rerun a projection-dependent narrowing/recompute pass (or iterate field-summary rebuild and projection transfer to a fixed point) before finalizing `var_ranges` and `return_range`. Add a semantic-pin regression test for a bounded loop whose exit block constructs a value, projects the narrowed field, and returns the projection.
  Resolved: Validated and accepted on 2026-03-26. Implementation tasks added to §03.3.

- [x] `[TPR-03-020][high]` `compiler/ori_repr/src/range/fixpoint/mod.rs:265` — Successor refinements are merged by `(block, var)` in a way that under-approximates paths from multiple predecessors and stale iterations.
  Evidence: `process_terminator()` stores branch refinements with plain `insert()` and switch-default refinements with `.meet()` into a single `block_refinements: FxHashMap<(ArcBlockId, ArcVarId), ValueRange>` that lives for the whole fixpoint. That key has no predecessor or iteration component, so distinct incoming facts are overwritten or monotonically narrowed instead of joined/recomputed. Fresh validation with an external probe CFG (`x < 0` on one predecessor, `x > 10` on another, both flowing to the same successor) produced `join var: Some(Bounded { lo: 11, hi: 9223372036854775807 })`, dropping the negative path entirely.
  Impact: join blocks can observe impossible scrutinee/variable ranges, which is an unsound under-approximation once §04 consumes these facts for narrowing decisions. The same map-lifetime bug can also preserve overly-tight switch-default complements from earlier iterations after a scrutinee range widens.
  Required plan update: make successor refinements edge-sensitive or recompute the table per iteration, and join incoming refinements across predecessors before applying them at block entry. Add semantic-pin tests for multi-predecessor branch refinement and for switch-default refinement after a widening iteration.
  Resolved: Validated and accepted on 2026-03-26. Two bugs confirmed: (1) Branch refinements use `insert()` instead of `join`, dropping refinements from earlier predecessors; (2) `block_refinements` map never cleared between iterations, preserving stale refinements. Implementation tasks added to §03.3.

- [x] `[TPR-03-021][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:296` — `return_range` is accumulated before narrowing and never recomputed from the final narrowed ranges.
  Evidence: `process_terminator()` only ever does `*return_range = return_range.join(ret_range)` during the forward iterations, while `range_fixpoint()` runs two narrowing passes and returns `state.return_range` unchanged. Fresh validation with the bounded-loop probe from `fixpoint_narrowing_recovers_loop_bound` produced `loop var: Some(Bounded { lo: 0, hi: 10 })` but `loop return_range: Bounded { lo: 0, hi: 9223372036854775807 }`.
  Impact: the §03.5 return-summary handoff will export widened summaries even when intraprocedural narrowing recovered precise loop bounds, which blocks downstream call-signature narrowing and makes the checked-off `RangeFixpointResult.return_range` contract inaccurate.
  Required plan update: recompute `return_range` from the final narrowed ranges (or include returns in the narrowing pass) and add a bounded-loop regression test that asserts both the loop variable and `return_range` narrow together.
  Resolved: Validated and accepted on 2026-03-26. Confirmed: `state.return_range` at line 571 is returned unchanged after narrowing passes. Implementation task added to §03.3.

- [x] `[TPR-03-017][high]` `compiler/ori_repr/src/range/fixpoint/mod.rs:214` — `Switch` case refinements overwrite earlier cases targeting the same successor block instead of joining them.
  Evidence: `process_terminator()` stores each case with `block_refinements.insert((case_block, *scrutinee), ...)`, so a switch like `{0 -> b1, 1 -> b1}` leaves only the last exact value in the map. The IR uses `Vec<(u64, ArcBlockId)>` for cases and this implementation does not assert or document any unique-target invariant.
  Impact: successor blocks can see an under-approximated scrutinee range, which becomes unsound once §04 consumes these ranges for narrowing decisions.
  Required plan update: join repeated case values per `(case_block, scrutinee)` instead of overwriting, and add a semantic-pin test covering a multi-value same-block switch.
  Resolved: Accepted and integrated into §03.3 on 2026-03-26. Implementation task added.

- [x] `[TPR-03-018][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:209` — `Switch` default successors never receive the complement refinement that §03.3 marks complete.
  Evidence: `process_terminator()` ignores `default` entirely and only inserts per-case equalities, even though the checked-off §03.3 requirement says the default block should get the complement range. The current switch regression test only checks a constant case path and never inspects the default successor.
  Impact: code in the default branch loses the primary fact from the switch scrutinee, so the implementation is weaker than the section claims and misses narrowing opportunities on default paths.
  Required plan update: compute and apply the default complement range, then add a regression test that asserts the default block excludes all explicit case values.
  Resolved: Accepted and integrated into §03.3 on 2026-03-26. Implementation task added.

- [x] `[TPR-03-019][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:232` — The narrowing pass never revisits block parameters or terminators, so widened loop-header parameters cannot recover to bounded ranges.
  Evidence: `run_narrowing_pass()` only walks `block.body` instructions and never re-runs `merge_block_params()` or terminator refinement/invoke handling. The checked-off §03.3 test note claims narrowing-pass recovery is covered, but the current fixpoint test file has no bounded-loop recovery case and the branch/switch "semantic pin" tests use already-constant values that do not exercise recovery.
  Impact: the main bounded-loop use case (`for i in 0..100`) can widen to `[0, i64::MAX]` and stay there, which blocks the §03→§04 handoff for loop-counter narrowing.
  Required plan update: add a narrowing-phase recomputation for block params and terminators, or an equivalent second fixpoint, plus a semantic-pin test on a bounded loop.
  Resolved: Accepted and integrated into §03.3 on 2026-03-26. Implementation task added.

- [x] `[TPR-03-001][minor]` `section-03-range-analysis.md:458` — **Block parameter merging only handles `Jump` predecessors; `Invoke` normal successor may pass args.**
  Resolved: Rejected on 2026-03-25. `ArcTerminator::Invoke` does NOT pass block arguments to its normal successor — unlike `Jump { target, args }`, the `normal` field is just an `ArcBlockId` with no `args`. The `Invoke`'s `args` field contains function call arguments (not block parameters). The `dst` result is handled separately in the fixpoint loop's Step 3 (terminator processing). Only `Jump` carries block arguments, so the merge loop is correct as written. Updated misleading comment at plan line 467 to clarify this.

- [x] `[TPR-03-002][low]` `plans/repr-opt/section-03-range-analysis.md:161` — The §03.1 checklist claims "`range/tests.rs`" contains 56 lattice tests, but the current file only defines 51 `#[test]` cases.
  Resolved: Rejected on 2026-03-25. TPR's count of 51 is incorrect — actual count is 58 `#[test]` functions. Plan text updated from "56" to "58" to match reality.

- [x] `[TPR-03-003][low]` `compiler/ori_repr/src/tests.rs:291` — The new `ValueRange` smoke test hard-codes `std::mem::size_of::<ValueRange>() == 24`, but enum layout is not part of the section's semantic contract and is not stable enough to pin this exactly.
  Resolved: Fixed on 2026-03-26. Replaced exact-size assertion with semantic checks (Default, join, meet).

- [x] `[TPR-03-004][high]` `compiler/ori_repr/src/range/transfer/mod.rs:248` — `range_div()` and `range_floordiv()` can panic on the valid corner case `i64::MIN / -1` instead of conservatively returning `Top`.
  Resolved: Validated and fixed on 2026-03-25. Replaced raw `/` with `checked_div()` for all 4 corners. 4 regression tests added (debug + release green).

- [x] `[TPR-03-005][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:435` — `range_bitnot()` can panic on ranges containing `i64::MIN` because it negates the endpoints before any checked operation runs.
  Resolved: Validated and fixed on 2026-03-25. Replaced unchecked negation with `checked_neg().and_then()`. 4 regression tests added (debug + release green).

- [x] `[TPR-03-006][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:71` — Builtin-call propagation is still effectively disabled, so `Apply` never yields the fixed ranges that §03.2 says are complete.
  Resolved: Validated and integrated into §03.5 on 2026-03-25. The §03.2 `transfer_known_call()` stub was explicitly planned as a two-phase approach (stub in §03.2, implementation in §03.5 which provides interner access). Concrete implementation task added to §03.5.

- [x] `[TPR-03-007][medium]` `compiler/ori_repr/src/range/mod.rs:232` — `is_int_typed()` skips `Tag::Applied`, so instantiated aliases/newtypes over `int` are treated as non-int and never enter the range pipeline.
  Resolved: Fixed on 2026-03-26. Added `Tag::Applied` to the match in `is_int_typed()`, resolves through `pool.resolve_fully()` same as `Named`/`Alias`.

- [x] `[TPR-03-008][high]` `compiler/ori_repr/src/range/transfer/mod.rs:303` — `range_floordiv()` is unsound because it delegates to truncating division even though Ori floor division rounds toward negative infinity.
  Resolved: Fixed on 2026-03-26. Implemented `checked_floor_div()` in `transfer/arithmetic.rs` and rewrote `range_floordiv()` to compute all 4 corners with floor semantics. 8 regression tests added. Debug + release green.

- [x] `[TPR-03-009][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:431` — `range_shr()` under-approximates negative right shifts when the shift amount is a range.
  Resolved: Fixed on 2026-03-26. Replaced directional monotonicity assumption with 4-corner computation in `transfer/bitwise.rs`. 4 regression tests added. Debug + release green.

- [x] `[TPR-03-010][low]` `compiler/ori_repr/src/range/transfer/mod.rs:1` — `transfer/mod.rs` is now 509 lines, which violates the repository’s 500-line non-test file limit.
  Resolved: Fixed on 2026-03-26. Split into `mod.rs` (242), `arithmetic.rs` (218), `bitwise.rs` (124). All within 500-line limit.

- [x] `[TPR-03-011][high]` `compiler/ori_repr/src/lib.rs:95` — `analyze_ranges()` is still a no-op, so the newly added §03 range modules never affect `ReprPlan` even though §03.2b and §03.4 are marked complete.
  Resolved: Validated on 2026-03-26. The infrastructure in §03.2b and §03.4 is correctly implemented and unit-tested, but the premature integration checkbox in §03.2b has been reopened (line 483). The integration and `analyze_ranges()` wiring are properly tracked in §03.3 (fixpoint loop) and §03.6 (completion checklist, line 1117). The planned build order (03.1→03.2→03.2b→03.4→**03.3**→03.5) makes §03.3 the integration point — not §03.2b.

- [x] `[TPR-03-012][medium]` `compiler/ori_repr/src/range/mod.rs:241` — TPR-03-007 was marked resolved, but the new `Tag::Applied` arm still has no regression test pinning the exact bug that was supposedly fixed.
  Resolved: Fixed on 2026-03-26. Added 8 regression tests in `range/tests.rs`: `is_int_typed_primitive_int`, `is_int_typed_non_int_primitives`, `is_int_typed_error`, `is_int_typed_applied_resolving_to_int` (semantic pin), `is_int_typed_applied_resolving_to_non_int`, `is_int_typed_named_resolving_to_int`, `is_int_typed_applied_chain_to_int`, `is_int_typed_unresolved_applied`. Debug + release green (277/277).

- [x] `[TPR-03-013][low]` `compiler/ori_repr/src/range/conditional/mod.rs:96` — The checked-off §03.4 boundary cases are still weaker than the plan claims: impossible `x < i64::MIN` / `x > i64::MAX` branches return `Top`, not `Bottom`.
  Resolved: Fixed on 2026-03-26. Changed all 4 overflow fallbacks in `refine_comparison()` from `Top` to `Bottom`: `Lt` (c=MIN, true), `LtEq` (c=MAX, false), `Gt` (c=MAX, true), `GtEq` (c=MIN, false). Updated 3 existing boundary tests (`lt_boundary_i64_min`, `lteq_boundary_i64_max`, `gt_boundary_i64_max`) to assert `Bottom`. Added new semantic pin `gteq_boundary_i64_min`. Debug + release green (277/277).

- [x] `[TPR-03-014][high]` `compiler/ori_repr/src/lib.rs:176` — `analyze_ranges()` is still a no-op, so the new §03.3 fixpoint work never populates `ReprPlan` even though the section now claims the handoff is complete.
  Resolved: Validated on 2026-03-26. Confirmed stub is empty at `lib.rs:176`. Already tracked as implementation task in §03.6 line 1117 (`- [ ] Fill in the analyze_ranges() stub`). No new task needed — the existing §03.6 item is the correct integration point.

- [x] `[TPR-03-015][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:146` — Successor refinements from `Branch` and `Switch` are only applied to block parameters, so ordinary dominated variables never get the narrowing that §03.4 advertises.
  Resolved: Validated on 2026-03-26. Confirmed: `block_refinements` entries are stored for arbitrary `(block, var)` pairs at lines 224-239 but only consumed inside the block-parameter merge loop at lines 146-149. Non-parameter variables never get refined. Implementation task added to §03.3 (apply block-entry refinements to all live vars).

- [x] `[TPR-03-016][medium]` `compiler/ori_repr/src/range/fixpoint/mod.rs:120` — Field summaries are monotone-joined across iterations without reset or narrowing, so an early conservative `Top` permanently poisons `(type, field)` precision.
  Resolved: Validated on 2026-03-26. Confirmed: `field_summary_table` accumulates via monotone `join` during the fixpoint loop but is never recomputed after the narrowing pass. Widened intermediate ranges from early iterations can permanently poison field precision. Implementation task added to §03.3 (recompute field summaries after narrowing pass).

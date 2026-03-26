---
section: "03"
title: "Value Range Analysis Framework"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-25
  triage_note: "TPR-03-008/009/010 validated and accepted on 2026-03-25. Implementation tasks added to §03.2."
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
    status: in-progress
  - id: "03.2"
    title: "Transfer Functions"
    status: complete
  - id: "03.2b"
    title: "Field-Summary Infrastructure"
    status: complete
  - id: "03.3"
    title: "Widening & Narrowing Operators"
    status: not-started
  - id: "03.4"
    title: "Conditional Range Refinement"
    status: complete
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
- [ ] **Implement `is_int_typed(ty: Idx, pool: &Pool) -> bool`** helper in `mod.rs` — checks `pool.tag(ty) == Tag::Int`. This is used pervasively by `transfer()`, `update_field_summaries()`, and the fixpoint loop but is never defined in any checklist item. Must also handle edge cases: `Idx::ERROR` returns false, resolved newtypes delegate to inner type. **[TPR-03-007]** Must also handle `Tag::Applied` — resolve through applied types the same way as `Named`/`Alias` (call `pool.resolve_fully()` first, then inspect the resolved tag). Add tests for an applied type that ultimately resolves to `int`.
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
- [ ] **[TPR-03-003]** Replace exact-size assertion in `compiler/ori_repr/src/tests.rs` `value_range_is_interval_lattice()` — remove `assert_eq!(std::mem::size_of::<ValueRange>(), 24)` and replace with semantic-only checks (Default, join, meet). Layout is not part of the section's semantic contract.
- [ ] Import and use `tracing` crate (never `println!`/`eprintln!`). All diagnostic output through `tracing::debug!`/`tracing::trace!` with target `ori_repr`.
- [ ] **Re-export key types from `mod.rs`**: `pub use` the types from submodules that downstream consumers need — at minimum `ValueRange`, `IntWidth` (already in `crate::repr`), `RangeAnalysisConfig`, `FieldSummaryTable`, `RangeFixpointResult`, `BranchRefinement`. Verify `crate::range::ValueRange` still resolves from `lib.rs`'s `pub mod range`.

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

- [x] Integrate `FieldSummaryTable` into the fixpoint loop (§03.3):
  - Create `FieldSummaryTable::new()` before the fixpoint loop starts
  - After processing each `Construct` instruction in the body loop, call `update_field_summaries()`
  - Pass `table.as_map()` as `field_summaries` in `TransferContext` so `Project` can query it
  - After the fixpoint completes, call `table.flush_to_repr_plan(repr_plan)` to persist results

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

      // Narrowing pass (optional — one iteration to recover precision)
      //
      for &block_idx in &rpo {
          let block = &func.blocks[block_idx];
          let ctx = TransferContext {
              ranges: &ranges,
              pool,
              var_types: &func.var_types,
              field_summaries: field_summary_table.as_map(),
          };
          for instr in &block.body {
              let computed = transfer(instr, &ctx);
              let Some(var) = instr.defined_var() else { continue };
              if let Some(&widened) = ranges.get(&var) {
                  let narrowed = narrow(widened, computed);
                  ranges.insert(var, narrowed);
              }
          }
      }

      RangeFixpointResult {
          var_ranges: ranges,
          field_summaries: field_summary_table,
          return_range,
      }
  }
  ```

- [ ] **Handoff to ReprPlan (§01 integration):** `range_fixpoint()` returns `RangeFixpointResult { var_ranges, field_summaries, return_range }`. The caller must flush all three into `ReprPlan`. The integration requires three storage additions:
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

- [ ] **Unit tests for fixpoint loop** in `compiler/ori_repr/src/range/tests.rs`. **TDD: write tests BEFORE implementing the fixpoint loop. Verify they fail. Then implement. Tests must pass unchanged.** Required coverage:
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

---

## 03.5 Function Signature Range Propagation

**File(s):** `compiler/ori_repr/src/range/signatures.rs`

**Implementation order:** §03.5 MUST be implemented after §03.1-§03.4 are stable and passing all tests. The intraprocedural analysis (§03.1-§03.4) is a required prerequisite and should be fully verified before interprocedural propagation is added. However, §03.5 is not optional — without it, function parameters can never be narrowed (since their ranges are always `Top` intraprocedurally), and struct field narrowing across module boundaries is impossible. Both of these are core mission goals.

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
  1. Build call graph using existing `CallGraph::build()` from `ori_arc::graph::call_graph` (module is `pub`, `CallGraph::build()` is `pub` — no visibility changes needed). Import: `use ori_arc::graph::call_graph::CallGraph;`.
  2. Compute SCCs using existing `ori_arc::graph::scc` module (Tarjan's algorithm, already implemented and tested). The SCC function is `compute_sccs(graph: &CallGraph) -> Vec<Scc>`, returns SCCs in **forward topological order** (leaves first — the order we need). Import: `use ori_arc::graph::scc::{compute_sccs, Scc};`. Do NOT reimplement Tarjan — reuse the existing infrastructure. Each SCC is a set of mutually-recursive functions.
  3. Process SCCs in reverse topological order (leaves first). For non-recursive SCCs (single function, no self-call): single pass — analyze function, record `FunctionRangeInfo`.
  4. For recursive SCCs: iterate:
     - Initialize all parameter ranges to Bottom (no callers processed yet).
     - Run intraprocedural `range_fixpoint()` on each function in the SCC. Extract `result.return_range` into `FunctionRangeInfo::return_range` and `result.var_ranges` into `ReprPlan::function_var_ranges`.
     - At each `Apply`/`Invoke` instruction targeting a function in the SCC, join the argument range into the callee's parameter range. Also use the callee's `return_range` (from `FunctionRangeInfo`) as the range for the call's `dst` variable — this replaces the intraprocedural Top fallback.
     - Repeat until parameter AND return ranges stabilize or `max_scc_iterations` (default: 10) reached.
     - If not converged, widen all parameter ranges to Top (safe fallback).
  5. **Budget:** Total SCC iterations across all SCCs capped at `max_total_scc_iterations` (default: 50). If exceeded, remaining SCCs get Top for all parameter ranges.

- [ ] **[TPR-03-006] Implement builtin name matching in `transfer_known_call()`** — replace the hardcoded `None` stub with actual name resolution for known builtins (`len` → `[0, MAX]`, `count` → `[0, MAX]`, `byte_to_int` → `[0, 255]`, `char_to_int` → `[0, 0x10FFFF]`, `abs` → via `range_abs()`). Requires interner access in the analysis context (available once §03.5's `FunctionRangeInfo` infrastructure provides it). Add end-to-end `transfer()` tests for `ArcInstr::Apply` targeting each builtin.
- [ ] Handle boundary cases for parameter ranges:
  - `@main(args:)`: the `args` list length is `[0, i64::MAX]`; the `args` parameter itself is not an int (skip)
  - Trait method parameters: assign Top (callers unknown at compile time — may be called via dynamic dispatch)
  - Closure parameters: assign Top unless all call sites of the closure are visible in the current module (conservative default)
  - `pub` function parameters: assign Top (external callers may pass full-range values)

- [ ] **Unit tests for §03.5** in `range/tests.rs`. **TDD: write tests BEFORE implementing interprocedural analysis. Verify they fail. Then implement. Tests must pass unchanged.** Required coverage:
  - Non-recursive function called with constant args → parameter range is `Bounded(const, const)`
  - Non-recursive function called with different constant args from 2 sites → parameter range is join
  - `pub` function → parameter range remains Top regardless of call-site args
  - Trait method parameters → Top (callers unknown at compile time)
  - Closure parameters → Top (conservative default)
  - Self-recursive function (SCC of size 1) → converges within `max_scc_iterations`
  - Mutually recursive pair (SCC of size 2) → parameter ranges stabilize or widen to Top
  - Return range propagation: function returning constant → callers see bounded return range
  - Return range propagation: callers of a function with bounded return range use that bound instead of Top
  - Budget exceeded: >50 total SCC iterations → remaining SCCs get Top (not hang, not panic)
  - **Semantic pin**: private function `@helper(x: int)` called only as `helper(x: 42)` → parameter range `[42, 42]`. This ONLY passes with interprocedural propagation; intraprocedural alone would give Top.
  - **Both debug and release**: `cargo test -p ori_repr` (debug) and `cargo test -p ori_repr --release` (release) must both pass

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

- [ ] `ValueRange` lattice correctly implements join, meet, fits_in, min_width, is_constant, overlaps (in `range/mod.rs`); `widen` and `narrow` free functions correct (in `range/fixpoint.rs`)
- [ ] Arithmetic transfer functions implemented: `range_add`, `range_sub`, `range_mul`, `range_div`, `range_mod`, `range_floordiv`, `range_neg` (PrimOp dispatched); bitwise: `range_bitand`, `range_bitor`, `range_bitxor`, `range_shl`, `range_shr`, `range_bitnot`; built-in function ranges: `range_len`, `range_count`, `range_byte_to_int`, `range_char_to_int`, `range_abs`
- [ ] Top-level `transfer()` dispatcher has an arm for every `ArcInstr` variant (15+ variants: `Let`, `Apply`, `ApplyIndirect`, `PartialApply`, `Project`, `Construct`, `RcInc`, `RcDec`, `IsShared`, `Set`, `SetTag`, `Reset`, `Reuse`, `CollectionReuse`, `Select`; verify against `ori_arc/src/ir/instr.rs` at implementation time for any new variants). Add a compile-time exhaustiveness test: the match must be non-`_` so new variants cause a build failure.
- [ ] Fixpoint loop handles all `ArcTerminator` variants (7: `Return`, `Jump`, `Branch`, `Switch`, `Invoke`, `Resume`, `Unreachable`). `Invoke` computes a range for `dst`; `Branch`/`Switch` produce refinements; others are no-ops. Use exhaustive match (no `_` arm) so new terminator variants cause a compile error.
- [ ] `transfer_primop()` has an arm for every `BinaryOp` variant (23: `Add`, `Sub`, `Mul`, `Div`, `Mod`, `FloorDiv`, `MatMul`, `Eq`, `NotEq`, `Lt`, `LtEq`, `Gt`, `GtEq`, `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Range`, `RangeInclusive`, `Coalesce`) and every `UnaryOp` variant (4: `Neg`, `Not`, `BitNot`, `Try`). Non-`_` match for exhaustiveness. NOTE: `Pow`/`**` is a language operator but is NOT a `BinaryOp` variant — it desugars before reaching ARC IR. Verify against `ori_ir/src/ast/operators.rs` at implementation time.
- [ ] Fixed-point iteration terminates within `max_iterations` for all test programs
- [ ] Block parameters (`ArcBlock::params`) are processed at the start of each block in the fixpoint loop, joining ranges from all predecessor `Jump` instructions
- [ ] Block terminators (`Branch`, `Switch`) propagate conditional range refinements to successor blocks
- [ ] `ArcTerminator::Invoke` handled in fixpoint loop — it defines a `dst` variable (same as `Apply`) and must have its range computed
- [ ] Conditional refinement extracts ranges from `if x < N` patterns
- [ ] Function signature propagation narrows parameters from call sites
- [ ] `Construct` instructions populate a field-summary table keyed by `(struct_or_tuple_idx, field_index)` so downstream field narrowing is based on construction-site evidence, not on the join of unrelated `int` variables
- [ ] `Project` instructions consult that field-summary table for struct/tuple fields; field projections are not left as unconditional `Top` when §04 field narrowing is enabled
- [ ] Recursive functions handled via SCC-based fixpoint with bounded iterations (max 10 per SCC, max 50 total)
- [ ] For `let x = 42`: range is exactly `[42, 42]`
- [ ] For `for i in 0..100`: range of `i` is `[0, 99]`
- [ ] For `let n = len(list)`: range is `[0, i64::MAX]`
- [ ] For a constructor-only workload like `struct Pixel { r: int, g: int, b: int, a: int }` with all construction sites in `0..255`, the field-summary table records `[0, 255]` for all four fields (semantic pin for §03 → §04 handoff)
- [ ] `./test-all.sh` green (range analysis is additive — no behavioral changes)
- [ ] `./clippy-all.sh` green
- [ ] Tracing: `ORI_LOG=ori_repr=debug` shows range computations for each function
- [ ] `#[tracing::instrument(skip_all)]` on `range_fixpoint()`, `transfer()`, and `refine_from_branch()`
- [ ] `tracing::debug!` at function entry/exit in `range_fixpoint()` showing function name, iteration count, and number of non-Top ranges
- [ ] `tracing::trace!` per-variable range updates inside the fixpoint loop (gated by trace level to avoid hot-path overhead)
- [ ] **Add `proptest` dev-dependency** to `compiler/ori_repr/Cargo.toml`: add `proptest.workspace = true` under `[dev-dependencies]` (already a workspace dependency in root `Cargo.toml` at line 69: `proptest = "1.4"`). Current `[dev-dependencies]` only has `pretty_assertions.workspace = true`.
- [ ] Property-based tests (proptest) for lattice laws and transfer function soundness in `range/tests.rs`:
  - **Lattice laws**: `join` is commutative (`join(a, b) == join(b, a)`), associative (`join(join(a, b), c) == join(a, join(b, c))`), idempotent (`join(a, a) == a`); `meet` same three properties; `join(a, Bottom) == a`; `meet(a, Top) == a`; `a.join(b) ⊇ a` and `a.join(b) ⊇ b` (containment); `join` and `meet` absorption: `join(a, meet(a, b)) == a`
  - **Transfer function soundness** — for each arithmetic op (`add`, `sub`, `mul`, `div`, `mod`, `neg`): generate random concrete values `x ∈ a_range` and `y ∈ b_range`, compute `x op y`, verify result is contained in `transfer_op(a_range, b_range)`. This is the critical soundness property: the abstract result must over-approximate the concrete result. Use `proptest::prop_assume!` to skip overflow cases where the concrete operation panics.
  - **Widening monotonicity**: `widen(a, b) ⊇ b` always; `widen(a, b) ⊇ a` always
  - **Widening termination**: sequence `widen(a₀, a₁), widen(a₁, a₂), ...` with random inputs must stabilize within 5 steps (widening pushes to infinity, so at most 2 widenings before reaching Top)
  - **Narrowing soundness**: `narrow(widened, computed) ⊆ widened` always (narrowing only tightens)
  - **Strategy**: generate `ValueRange` values with `proptest::strategy::Union` of `Bottom`, `Top`, and `Bounded { lo, hi }` where `lo <= hi` (use `(i64::MIN..=i64::MAX, i64::MIN..=i64::MAX).prop_map(|(a, b)| if a <= b { Bounded(a, b) } else { Bounded(b, a) })`)
- [ ] Range results written into `ReprPlan::function_var_ranges` via `ReprPlan::set_var_ranges(func_name, ranges)` — verified by test
- [ ] Field-summary results flushed into `ReprPlan::field_range_summaries` via `FieldSummaryTable::flush_to_repr_plan()` — verified by test
- [ ] Return ranges collected from all `Return` terminators and available in `RangeFixpointResult::return_range` for §03.5 consumption
- [ ] `ReprPlan::field_range(type_idx, field)` query method added and returns correct ranges after analysis
- [ ] `ReprPlan::join_field_range(type_idx, field, range)` writer method added for field-summary flush
- [ ] `transfer()` uses `TransferContext` struct (not loose params) — carries `ranges`, `pool`, `var_types`, `field_summaries`
- [ ] Unknown or unsupported ArcInstr patterns gracefully degrade to `Top` (never panic, never return `Bottom` for reachable code). Explicit `tracing::debug!` when falling back to Top for a pattern that could be tightened.
- [x] `compute_postorder()`, `successor_block_ids()`, and `compute_predecessors()` in `ori_arc::graph::mod.rs` changed from `pub(crate)` to `pub` (2026-03-25)
- [ ] **Fill in the `analyze_ranges()` stub** in `compiler/ori_repr/src/lib.rs` (line 176, currently `fn analyze_ranges(_plan: &mut ReprPlan, _pool: &Pool, _fns: &[ArcFunction]) {}`). The implementation must: (1) create a `RangeAnalysisConfig::default()`, (2) for each `ArcFunction` in `arc_functions`, call `range_fixpoint(func, pool, &config)`, (3) store the `var_ranges` result via `plan.set_var_ranges(func.name, result.var_ranges)`, (4) flush field summaries via `result.field_summaries.flush_to_repr_plan(plan)`, (5) store return ranges for §03.5 interprocedural use. Remove the `_` prefixes on all parameters.
- [ ] **Add `field_range_summaries` field to `ReprPlan`** in `compiler/ori_repr/src/plan.rs` — add `field_range_summaries: FxHashMap<(Idx, u32), ValueRange>` after `function_var_ranges` (line 95), initialize in `ReprPlan::new()`, and add `field_range()` and `join_field_range()` methods. The `#[expect(clippy::zero_sized_map_values)]` does NOT apply to this field (ValueRange is now an enum, not a ZST).
- [ ] **Fix `.cloned()` → `.copied()` in `ReprPlan::var_range()`** (`plan.rs` line 159). Once `ValueRange` is `Copy` (the enum derives `Copy`), `.cloned()` triggers `clippy::cloned_instead_of_copied`. Change to `.copied()`.
- [ ] **Add `pub use` re-exports in `lib.rs`** for new public types: `pub use range::ValueRange` (already there implicitly via `pub mod range`, but verify), `RangeAnalysisConfig`, `FieldSummaryTable`, `RangeFixpointResult`.

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

- [x] `[TPR-03-001][minor]` `section-03-range-analysis.md:458` — **Block parameter merging only handles `Jump` predecessors; `Invoke` normal successor may pass args.**
  Resolved: Rejected on 2026-03-25. `ArcTerminator::Invoke` does NOT pass block arguments to its normal successor — unlike `Jump { target, args }`, the `normal` field is just an `ArcBlockId` with no `args`. The `Invoke`'s `args` field contains function call arguments (not block parameters). The `dst` result is handled separately in the fixpoint loop's Step 3 (terminator processing). Only `Jump` carries block arguments, so the merge loop is correct as written. Updated misleading comment at plan line 467 to clarify this.

- [x] `[TPR-03-002][low]` `plans/repr-opt/section-03-range-analysis.md:161` — The §03.1 checklist claims "`range/tests.rs`" contains 56 lattice tests, but the current file only defines 51 `#[test]` cases.
  Resolved: Rejected on 2026-03-25. TPR's count of 51 is incorrect — actual count is 58 `#[test]` functions. Plan text updated from "56" to "58" to match reality.

- [ ] `[TPR-03-003][low]` `compiler/ori_repr/src/tests.rs:291` — The new `ValueRange` smoke test hard-codes `std::mem::size_of::<ValueRange>() == 24`, but enum layout is not part of the section's semantic contract and is not stable enough to pin this exactly.
  Resolved: Validated and integrated into §03.1 on 2026-03-25. Task added to replace exact-size assertion with semantic-only checks.

- [x] `[TPR-03-004][high]` `compiler/ori_repr/src/range/transfer/mod.rs:248` — `range_div()` and `range_floordiv()` can panic on the valid corner case `i64::MIN / -1` instead of conservatively returning `Top`.
  Resolved: Validated and fixed on 2026-03-25. Replaced raw `/` with `checked_div()` for all 4 corners. 4 regression tests added (debug + release green).

- [x] `[TPR-03-005][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:435` — `range_bitnot()` can panic on ranges containing `i64::MIN` because it negates the endpoints before any checked operation runs.
  Resolved: Validated and fixed on 2026-03-25. Replaced unchecked negation with `checked_neg().and_then()`. 4 regression tests added (debug + release green).

- [ ] `[TPR-03-006][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:71` — Builtin-call propagation is still effectively disabled, so `Apply` never yields the fixed ranges that §03.2 says are complete.
  Resolved: Validated and integrated into §03.5 on 2026-03-25. The §03.2 `transfer_known_call()` stub was explicitly planned as a two-phase approach (stub in §03.2, implementation in §03.5 which provides interner access). Concrete implementation task added to §03.5.

- [ ] `[TPR-03-007][medium]` `compiler/ori_repr/src/range/mod.rs:232` — `is_int_typed()` skips `Tag::Applied`, so instantiated aliases/newtypes over `int` are treated as non-int and never enter the range pipeline.
  Resolved: Validated and integrated into §03.1 `is_int_typed()` task on 2026-03-25. The existing unchecked item now includes `Tag::Applied` handling.

- [x] `[TPR-03-008][high]` `compiler/ori_repr/src/range/transfer/mod.rs:303` — `range_floordiv()` is unsound because it delegates to truncating division even though Ori floor division rounds toward negative infinity.
  Resolved: Fixed on 2026-03-26. Implemented `checked_floor_div()` in `transfer/arithmetic.rs` and rewrote `range_floordiv()` to compute all 4 corners with floor semantics. 8 regression tests added. Debug + release green.

- [x] `[TPR-03-009][medium]` `compiler/ori_repr/src/range/transfer/mod.rs:431` — `range_shr()` under-approximates negative right shifts when the shift amount is a range.
  Resolved: Fixed on 2026-03-26. Replaced directional monotonicity assumption with 4-corner computation in `transfer/bitwise.rs`. 4 regression tests added. Debug + release green.

- [x] `[TPR-03-010][low]` `compiler/ori_repr/src/range/transfer/mod.rs:1` — `transfer/mod.rs` is now 509 lines, which violates the repository’s 500-line non-test file limit.
  Resolved: Fixed on 2026-03-26. Split into `mod.rs` (242), `arithmetic.rs` (218), `bitwise.rs` (124). All within 500-line limit.

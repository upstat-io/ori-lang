---
section: "05"
title: "Float Narrowing Pipeline"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-29
goal: "Lower float (semantic f64) to f32 when the compiler can prove zero precision loss for the specific values used"
inspired_by:
  - "LLVM fptrunc/fpext analysis (lib/Transforms/InstCombine/InstCombineCasts.cpp)"
  - "GCC -fexcess-precision semantics (gcc/convert.cc)"
depends_on: ["03", "04"]  # §05.1-§05.3 only need §03; §05.4 extends §04's codegen (trunc_for_narrowed_struct, sext_narrowed_field, try_lower_narrowed_aggregate)
sections:
  - id: "05.1"
    title: "Precision Analysis"
    status: complete
  - id: "05.2"
    title: "Float Range Collection"
    status: complete
  - id: "05.3"
    title: "Float Field Narrowing"
    status: complete
  - id: "05.4"
    title: "LLVM Codegen: fpext/fptrunc at Boundaries"
    status: complete
  - id: "05.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Float Narrowing Pipeline

**Context:** Float narrowing is much more constrained than integer narrowing because floating-point precision is non-linear. The set of values exactly representable in f32 is a strict subset of f64. Narrowing is only safe when:
1. All literal values are exactly representable in f32
2. No arithmetic is performed on the value between store and load (storage-only)
3. The `fptrunc` → `fpext` roundtrip is lossless for every observed value

In practice, this means float narrowing is mostly useful for:
- Struct fields that store f32-exact constants (0.0, 1.0, 0.5, integer-valued floats up to 2^24)
- Pure storage/retrieval without arithmetic (data transfer patterns)
- Graphics/audio struct fields where all values happen to be f32-exact

**Spec authority:** Annex E (System Considerations) explicitly permits float narrowing: "The compiler may use a narrower machine representation when it can prove no precision loss." The semantic contract is IEEE 754 double precision — any narrowed path must produce bit-identical results to the canonical f64 path for all language-level operations.

**Reference implementations:**
- **LLVM** `InstCombineCasts.cpp`: `canEvaluateTruncated()` — checks if fptrunc is lossless
- **GCC** `convert.cc`: Excess precision handling for C11 semantics

**Depends on:** §03 (range analysis infrastructure — fixpoint pattern and `RangeAnalysisConfig`). §05 does NOT consume §03's `ValueRange` lattice (which is integer-only). Instead, §05 defines its own `FloatRange` lattice and collection pass that operates on the same `ArcFunction` IR.

**Crate dependency:** Same as §04 — `ori_repr` reads `ArcFunction` from `ori_arc`. The `narrowing/float.rs` module lives alongside `narrowing/int.rs` in `compiler/ori_repr/src/narrowing/`.

**Scope: storage-only narrowing (Phase A).** This section implements float narrowing for struct/tuple fields that are stored and loaded but never used in arithmetic. Arithmetic narrowing (computing in f32) is a semantic change requiring explicit opt-in — it is NOT part of this section. The plan conservatively treats any arithmetic result as non-narrowable.

**Downstream contracts:**
- **§05 → §06 (Struct Layout):** Same interface contract as §04 → §06. §05 writes only `FieldRepr.repr` (narrowed to `Float { F32 }`). It does NOT compute `FieldRepr.offset`, `StructRepr.size`, or `StructRepr.align` — those are §06's exclusive responsibility. §06 reads the narrowed `repr` values (now potentially 4-byte f32 instead of 8-byte f64) to compute correct field sizes and alignment-optimal reordering.
- **§05 → §07 (Enum Repr):** Float narrowing may produce `f32`-typed fields in structs used as enum variant payloads. `f32` NaN bit patterns (quiet NaN: `0x7FC00000`–`0x7FFFFFFF` and `0xFFC00000`–`0xFFFFFFFF`) are technically invalid "values" but IEEE 754 semantics make them complex to use as niches. §07's `find_niches()` MUST return `vec![]` for `MachineRepr::Float { width: F32 }` fields — no NaN-based niches for f32 fields. §05 documents this here; §07 implements the guard.
- **§05 → §01 (float_width query):** §05 writes `Float { width: F32 }` into `ReprPlan` struct field entries. The existing `float_width(idx)` query (`plan/query.rs:81`, default `F64`) is type-level, NOT field-level — it reflects the canonical float type. §05's narrowing is field-level (inside `MachineRepr::Struct` field entries), so it does NOT change the top-level `float_width()` return for `Tag::Float`. Codegen reads the narrowed width from `FieldRepr.repr`, not from `float_width()`.

**Interaction with §04 (Integer Narrowing):** A single struct may have BOTH integer fields narrowed by §04 AND float fields narrowed by §05. The combined result is a `MachineRepr::Struct` with a mix of narrowed int fields and narrowed float fields. §04's `try_lower_narrowed_aggregate()` in `layout_resolver.rs:318` has two relevant guards: (1) `has_narrowed` only checks for `Int { width != I64 }` — it does NOT detect `Float { F32 }`, so a struct with ONLY narrowed float fields would fall through to the `TypeInfoStore` named-struct path; (2) the `all_scalar_int` variable (misleadingly named) already accepts `MachineRepr::Float { .. }` in its match — so the "all fields are scalar" guard does NOT block float fields. §05 must extend the `has_narrowed` detection to include float narrowing. This is documented in §05.4.

---

## 05.1 Precision Analysis

**TDD order:** Write unit tests for `is_f32_exact()` and `FloatRange` lattice BEFORE implementing the functions. The tests listed in §05.5 under "is_f32_exact() correctly identifies f32-representable f64 values" and "FloatRange lattice operations" are the Phase 1 test matrix. Verify they fail (compile error / missing function), then implement, then verify they pass unchanged. Run in both debug and release: `timeout 150 cargo test -p ori_repr -- float` for each.

**File(s):** `compiler/ori_repr/src/narrowing/float.rs` (NEW — must be created and registered in `narrowing/mod.rs`)

**Setup required:**
1. Create `compiler/ori_repr/src/narrowing/float.rs`
2. Add `pub mod float;` to `compiler/ori_repr/src/narrowing/mod.rs` (currently has `pub mod abi; pub mod int; pub mod overflow;` at lines 22-24, and `#[cfg(test)] mod tests;` at line 26-27)
3. Update `narrowing/mod.rs` module doc (`//!` at line 1) to mention the float module alongside the existing three modules
4. Add `pub use narrowing::float::...` to `compiler/ori_repr/src/lib.rs` as needed (follow the pattern of existing `pub use narrowing::abi::...` at line 46-48)

**Note:** `f64` does not implement `Eq` or `Hash`. `FloatRange` must use `u64` bit representation (`f64::to_bits()`) for any variant that needs `Eq`/`Hash`, or use `PartialEq` only (no map key usage). Since `FloatRange` is used in a field-summary table keyed by `(Idx, u32)`, the range itself does NOT need to be a map key — `PartialEq` is sufficient.

- [x] Define `FloatRange` lattice:
  ```rust
  /// Float precision lattice for field-level narrowing.
  ///
  /// Unlike `ValueRange` (integer intervals), `FloatRange` tracks whether
  /// ALL observed values at a field position are f32-exact, NOT an interval
  /// of float values. This is because f32-exactness is a property of
  /// individual values, not ranges — `[0.0, 1.0]` as a range contains
  /// non-f32-exact values like `0.1`, even though the endpoints are f32-exact.
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum FloatRange {
      /// No observations yet (narrowable — no conflicting evidence).
      /// Joins with any other variant by taking the other variant.
      Bottom,
      /// All observed values are exactly representable in f32.
      F32Exact,
      /// At least one observed value is NOT f32-exact → keep f64.
      Top,
  }
  ```

- [x] Implement `FloatRange::join()`:
  ```rust
  impl FloatRange {
      /// Lattice join: widen precision evidence.
      pub fn join(self, other: Self) -> Self {
          match (self, other) {
              (FloatRange::Bottom, x) | (x, FloatRange::Bottom) => x,
              (FloatRange::Top, _) | (_, FloatRange::Top) => FloatRange::Top,
              (FloatRange::F32Exact, FloatRange::F32Exact) => FloatRange::F32Exact,
          }
      }
  }
  ```

- [x] Implement f32 exactness check:
  ```rust
  /// Check if an f64 value is exactly representable as f32.
  ///
  /// The roundtrip `f64 → f32 → f64` is lossless iff the value has no
  /// precision beyond f32's 24-bit significand. NaN is excluded because
  /// NaN payload bits may not survive the roundtrip. Infinities ARE
  /// f32-exact (positive and negative infinity have the same bit
  /// pattern in f32 and f64). Negative zero IS f32-exact.
  ///
  /// f64 values outside f32 range (magnitude > f32::MAX ≈ 3.4e38) cast
  /// to f32::INFINITY, so the roundtrip produces INFINITY ≠ original.
  /// These are correctly rejected by the equality check.
  ///
  /// f64 subnormals that are too small for f32's subnormal range (below
  /// f32::MIN_POSITIVE * 2^-23 ≈ 1.4e-45) flush to zero in f32, so the
  /// roundtrip produces 0.0 ≠ original. These are correctly rejected.
  pub fn is_f32_exact(value: f64) -> bool {
      if value.is_nan() {
          return false;
      }
      let as_f32 = value as f32;
      let roundtripped = as_f32 as f64;
      roundtripped == value
  }
  ```

- [x] Implement `FloatRange::observe()` for collecting evidence:
  ```rust
  impl FloatRange {
      /// Observe a float literal value and update the range.
      pub fn observe(self, value: f64) -> Self {
          if is_f32_exact(value) {
              self.join(FloatRange::F32Exact)
          } else {
              FloatRange::Top
          }
      }

      /// Observe an arithmetic result (conservatively non-narrowable).
      pub fn observe_arithmetic(self) -> Self {
          FloatRange::Top
      }
  }
  ```

---

## 05.2 Float Range Collection

**TDD order:** Write unit tests for `FloatFieldSummaryTable` (observe/get) and `collect_float_field_summaries()` BEFORE implementing. Test: empty table returns `Bottom`; single f32-exact observation → `F32Exact`; mixed observations → `Top`; non-float fields ignored. Verify they fail, then implement, then verify they pass unchanged.

**File(s):** `compiler/ori_repr/src/narrowing/float.rs`

**Context:** §03's `FieldSummaryTable` and `update_field_summaries()` only track integer-typed fields (gated by `is_int_typed()` in `field_summary.rs:22`). Float narrowing needs an analogous collection pass that scans `Construct` instructions for float-typed field arguments and checks whether those arguments are f32-exact literals.

**Design choice:** Rather than modifying §03's integer-only infrastructure, §05 builds a parallel `FloatFieldSummaryTable` that runs after range analysis. This keeps §03's integer lattice clean and avoids polluting the fixpoint loop with float-specific logic. The float pass is simpler because it only needs to check literal values — it does NOT need a fixpoint loop (literal values are known statically from `LitValue::Float`).

- [x] Define `FloatFieldSummaryTable`:
  ```rust
  /// Aggregates float precision evidence per (type, field) pair.
  ///
  /// Scans `Construct` sites for float-typed fields and checks whether
  /// ALL values stored at each position are f32-exact literals.
  /// Mirrors `FieldSummaryTable` (`range/field_summary.rs:28`) but for
  /// float precision instead of integer ranges.
  #[derive(Debug)]
  pub struct FloatFieldSummaryTable {
      field_ranges: FxHashMap<(Idx, u32), FloatRange>,
  }

  impl FloatFieldSummaryTable {
      pub fn new() -> Self { Self { field_ranges: FxHashMap::default() } }

      /// Join a float precision observation for a (type, field) pair.
      pub fn observe(&mut self, type_idx: Idx, field: u32, range: FloatRange) {
          self.field_ranges
              .entry((type_idx, field))
              .and_modify(|existing| *existing = existing.join(range))
              .or_insert(range);
      }

      /// Query the float precision for a (type, field) pair.
      /// Returns `Bottom` (no observations) if no evidence has been collected.
      pub fn get(&self, type_idx: Idx, field: u32) -> FloatRange {
          self.field_ranges
              .get(&(type_idx, field))
              .copied()
              .unwrap_or(FloatRange::Bottom)
      }
  }
  ```

- [x] Implement `collect_float_field_summaries()`:
  Follow the pattern of `update_field_summaries()` in `range/field_summary.rs:176`. Scan all `ArcFunction` blocks for `Construct` instructions. The key structural difference: `update_field_summaries` runs inside the fixpoint loop and reads variable ranges from the fixpoint state; float collection is a single-pass post-fixpoint scan that only needs to check literal values.

  **Concrete approach:**
  ```rust
  pub fn collect_float_field_summaries(
      plan: &ReprPlan,
      pool: &Pool,
      arc_functions: &[ArcFunction],
  ) -> FloatFieldSummaryTable {
      let mut table = FloatFieldSummaryTable::new();
      for func in arc_functions {
          for block in &func.blocks {
              for instr in &block.body {
                  // Only Construct for Struct/Tuple/EnumVariant (same filter as field_summary.rs:188-194)
                  let ArcInstr::Construct { ty, ctor, args, .. } = instr else { continue };
                  match ctor {
                      CtorKind::Struct(_) | CtorKind::Tuple | CtorKind::EnumVariant { .. } => {}
                      _ => continue,
                  }
                  // For each argument, check if it's a float field
                  for (i, arg) in args.iter().enumerate() {
                      let arg_ty = func.var_types.get(arg.index()).copied();
                      let is_float = arg_ty.is_some_and(|t| pool.tag(pool.resolve_fully(t)) == Tag::Float);
                      if !is_float { continue; }
                      // Check if the defining instruction is a Literal(Float)
                      let range = find_literal_float_value(func, *arg)
                          .map(|bits| FloatRange::Bottom.observe(f64::from_bits(bits)))
                          .unwrap_or(FloatRange::Top); // variable/arithmetic → conservative
                      table.observe(*ty, i as u32, range);
                  }
              }
          }
      }
      table
  }
  ```

  **Helper `find_literal_float_value()`**: Walk the function's blocks to find where `arg` is defined. If it's a `Let { value: ArcValue::Literal(LitValue::Float(bits)), .. }`, return `Some(bits)`. Otherwise return `None` (conservative — variable/arithmetic source). This is a simple linear scan since ARC IR is SSA (each variable defined exactly once). For Phase A simplicity, a per-function `FxHashMap<ArcVarId, u64>` pre-populated by scanning all `Let` instructions for `LitValue::Float` values works well.

  **Non-float fields are ignored** — the `is_float` check filters them out.

- [x] Wire `collect_float_field_summaries()` into `apply_float_narrowing()`:
  The stub `apply_float_narrowing(_plan, _pool)` in `compiler/ori_repr/src/lib.rs:504` needs to be filled in. Following §04's pattern (`apply_integer_narrowing` at `lib.rs:490`):
  1. **Update signature**: Change `fn apply_float_narrowing(_plan: &mut ReprPlan, _pool: &Pool) {}` to `fn apply_float_narrowing(plan: &mut ReprPlan, pool: &Pool, arc_functions: &[ArcFunction])`. Update the call site at `lib.rs:226` to pass `arc_functions` (it is already in scope — see `analyze_ranges` call at line 224).
  2. Gate on `plan.narrowing_policy() != Disabled` (early return)
  3. Gate on `plan.is_integer_narrowing_safe_for_codegen()` — this method name is misleading for float narrowing but the underlying check (whether `has_analysis_only_functions` prevents codegen-visible narrowing, see `plan.rs:348`) applies equally to float narrowing. **Concrete action**: rename the method to `is_narrowing_safe_for_codegen()` in `plan.rs:348`, update the `set_has_analysis_only_functions()` doc at `plan.rs:357`, and update all 3 call sites (`lib.rs:496`, `lib.rs` new float site, `range/signatures/mod.rs:213`). This rename is a hygiene improvement — the method was always about narrowing-in-general, not just integer narrowing.
  4. Call `collect_float_field_summaries(plan, pool, arc_functions)` to populate the float field summary table
  5. Call `narrow_float_fields(plan, pool, &float_table)` (§05.3) passing the summary table

**BLOAT note:** `compiler/ori_repr/src/lib.rs` is currently 533 lines (exceeds the 500-line limit). Adding the float narrowing body will push it further over. **Before implementing §05.2**, extract the `apply_integer_narrowing()` and `apply_float_narrowing()` function bodies from `lib.rs` into `narrowing/int.rs` and `narrowing/float.rs` respectively, leaving only thin forwarding calls in `lib.rs`. This reduces `lib.rs` to its proper role as an index/dispatcher.

---

## 05.3 Float Field Narrowing

**TDD order:** Write unit tests for `narrow_float_fields()` BEFORE implementing. The test matrix covers: f32-exact narrowed, non-f32-exact preserved, `#repr("c")`/`#repr("packed")`/`#repr("transparent")`/`#repr("aligned", N)` skipped, public types skipped, `NarrowingPolicy::Disabled` skipped, tuples deferred, combined int+float narrowing. Verify all tests fail with the stub implementation, then implement, then verify they pass unchanged. Run in both debug and release.

**File(s):** `compiler/ori_repr/src/narrowing/float.rs`

Float narrowing follows the same pattern as `narrow_struct_fields()` in `narrowing/int.rs`:

- [x] Implement `narrow_float_fields()`:
  ```rust
  /// Narrow float fields in struct types from f64 to f32 when all observed
  /// values are f32-exact. Follows the same conservatism rules as integer
  /// narrowing (§04):
  ///
  /// - `#repr("c")` / `#repr("packed")` / `#repr("transparent")` → skip
  /// - Public types → skip (ABI contract)
  /// - `NarrowingPolicy::Disabled` → skip
  /// - Only fields with `MachineRepr::Float { width: F64 }` are candidates
  pub fn narrow_float_fields(
      plan: &mut ReprPlan,
      pool: &Pool,
      float_table: &FloatFieldSummaryTable,
  ) { ... }
  ```

  The implementation mirrors `narrowing/int.rs::narrow_struct_fields()` (line 32):
  1. Collect type indices with `Struct` representations using `plan.decision_indices()` + filter on `MachineRepr::Struct(_)` (same pattern as `int.rs:42-52`)
  2. Skip types with `has_fixed_layout_attr()`. **Concrete action**: `has_fixed_layout_attr()` is currently a private `fn` in `narrowing/int.rs:201`. Change its visibility to `pub(crate)` and either (a) move it to `narrowing/mod.rs` for shared use, or (b) import it from `super::int::has_fixed_layout_attr` in `float.rs`. Option (a) is preferred since the function is a shared narrowing concern. Also move `CandidateKind` (currently private in `int.rs:231-234`) to `narrowing/mod.rs` with `pub(crate)` visibility for reuse.
  3. Skip `plan.is_public_type(idx)` types (method at `plan.rs:285`)
  4. For each `Float { width: F64 }` field in the struct, query `float_table.get(idx, field.original_index)` for the field's `FloatRange`
  5. If `FloatRange::F32Exact` → rewrite `field.repr` to `MachineRepr::Float { width: FloatWidth::F32 }` (import `FloatWidth` from `crate::repr`)
  6. Record decision with `DecisionSource::FloatNarrowing` (already exists at `decision.rs:35`) and `DecisionReason::Custom(summary_string)`. Build the summary string with a `float_field_range_summary_string()` helper (follow `field_range_summary_string()` at `int.rs:214`).
  7. Also store under `pool.resolve_fully(idx)` if different (same pattern as `int.rs:101-112`)

- [x] Float field summary storage — **use local table (recommended for Phase A)**:
  The `FloatFieldSummaryTable` is built by `collect_float_field_summaries()` and consumed by `narrow_float_fields()` within the same `apply_float_narrowing()` call. Unlike integer ranges (which need persistent storage for the fixpoint loop and cross-function analysis), float field summaries have no fixpoint dependency — they are computed once from literal values. **Store the table as a local variable** in `apply_float_narrowing()` and pass it to `narrow_float_fields()` by reference. Do NOT add a new field to `ReprPlan` unless a future section needs cross-pass access to float precision data.

  If persistent storage is needed later (e.g., for audit trail dumping), add `float_field_summaries: FxHashMap<(Idx, u32), FloatRange>` to `ReprPlan` following the `field_range_summaries` pattern at `plan.rs:101`. But this is not needed for Phase A.

- [x] Tuple narrowing: Same deferral as §04 Phase A — tuples are skipped. The `narrow_float_fields()` function must check for `CandidateKind::Tuple` and skip with tracing, identical to §04's tuple handling in `narrow_struct_fields()`. Float tuple fields are not narrowed until collection/element integration (future Phase C equivalent).

- [x] `NarrowingPolicy` handling: `narrow_float_fields()` must check `plan.narrowing_policy()` and return early if `Disabled`. When `Conservative`, apply the same rules as integer narrowing (only provably-safe narrowing). When `Aggressive`, narrow all fields where `FloatRange::F32Exact` is observed.

- [x] **Combined int+float narrowed struct handling**: When §04 has already narrowed some int fields and §05 narrows additional float fields in the same struct, the resulting `MachineRepr::Struct` contains a mix of narrowed types. §05 must read the struct repr that §04 may have already modified (via `plan.get_repr(idx)`), apply float narrowing to any remaining `Float { F64 }` fields, and write back the combined result. This is natural if §05 runs after §04 (which it does — `lib.rs:225-226`), but the implementation must NOT assume the struct is in its canonical state — it must read the current (possibly already-narrowed-by-§04) repr.

---

## 05.4 LLVM Codegen: fpext/fptrunc at Boundaries

**File(s):** `compiler/ori_llvm/src/codegen/` (multiple files — `ArcIrEmitter`, `layout_resolver.rs`)

**TDD order:** Write the AOT tests listed in §05.5 BEFORE implementing the codegen changes. The IR pin tests (`test_float_narrowed_struct_ir_pin_type_layout`, `test_float_narrowed_struct_ir_pin_fptrunc_on_construction`, etc.) should fail because no `fptrunc`/`fpext` instructions are emitted yet. After implementation, they must pass unchanged. Run AOT tests in both debug and release: `timeout 150 cargo test -p ori_llvm -- float` and `timeout 150 cargo test --release -p ori_llvm -- float`.

**What already works:** `TypeLayoutResolver::try_repr_to_llvm_type()` in `layout_resolver.rs:157-159` already handles `MachineRepr::Float { F32 }` → `context.f32_type()`. When §05.3 writes `Float { F32 }` into a struct field's `FieldRepr`, the struct's LLVM type will automatically use `float` (f32) for that field. No changes needed to `try_repr_to_llvm_type()` itself.

**What's missing:** The ARC IR operates on f64 values (canonical `float`). When codegen stores an f64 value into an f32 struct field, it must insert `fptrunc double to float`. When loading from an f32 field back to an f64 computation, it must insert `fpext float to double`. These conversions happen at the `Construct` (store) and `Project` (load) instruction boundaries.

**Implementation order** (items listed in dependency order — implement top-to-bottom):

- [x] **(prerequisite)** BLOAT extraction deferred — `emitter_utils.rs` stays cohesive with float branches added inline (same pattern as integer narrowing). Extraction can happen in a dedicated cleanup pass.

- [x] Add `float_trunc()`, `float_ext()`, and `fpext_to_f64_if_narrower()` methods to `IrBuilder`:
  **WHERE:** `compiler/ori_llvm/src/codegen/ir_builder/conversions.rs` (currently 181 lines — well within 500-line limit).
  **Verified missing:** `IrBuilder` has `trunc()` (line 19), `sext()` (line 56), `si_to_fp()` (line 88), `fp_to_si()` (line 104) but NO float-to-float truncation or extension methods.
  **Add two methods** following the pattern of `trunc()` (line 19-32) and `sext()` (line 56-69):
  ```rust
  /// Build float truncation (e.g., f64 → f32).
  pub fn float_trunc(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
      let v = self.arena.get_value(val);
      let target = self.arena.get_type(ty).into_float_type();
      if !v.is_float_value() {
          tracing::error!(val_type = ?v.get_type(), "float_trunc on non-float operand");
          self.record_codegen_error();
          return self.const_f64(0.0);
      }
      let result = self.builder
          .build_float_trunc(v.into_float_value(), target, name)
          .expect("float_trunc");
      self.arena.push_value(result.into())
  }

  /// Build float extension (e.g., f32 → f64).
  pub fn float_ext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
      let v = self.arena.get_value(val);
      let target = self.arena.get_type(ty).into_float_type();
      if !v.is_float_value() {
          tracing::error!(val_type = ?v.get_type(), "float_ext on non-float operand");
          self.record_codegen_error();
          return self.const_f64(0.0);
      }
      let result = self.builder
          .build_float_ext(v.into_float_value(), target, name)
          .expect("float_ext");
      self.arena.push_value(result.into())
  }
  ```
  **Note:** `inkwell::builder::Builder` provides `build_float_trunc()` and `build_float_ext()` — verify these method names match the inkwell 0.5+ API (the crate uses LLVM 21 / inkwell `llvm21-1`).

- [x] Extend `trunc_for_narrowed_struct()` for float fields:
  The existing `trunc_for_narrowed_struct()` in `emitter_utils.rs:501` only handles `Tag::Int` fields (checks `pool.tag() == Tag::Int` and uses integer `trunc`). §05 must extend this to ALSO handle `Tag::Float` fields. When the pool field type is `Tag::Float` (canonical f64) but the struct's LLVM field type is `float` (f32), insert `fptrunc double to float`.

  **Implementation approach:** In `trunc_for_narrowed_struct()`, add a second branch after the existing int check:
  ```rust
  // Existing: integer truncation for narrowed int fields
  let is_int_field = ...;
  if is_int_field { /* existing trunc logic */ }

  // NEW: float truncation for narrowed float fields
  let is_float_field = field_pool_types
      .get(i)
      .is_some_and(|&idx| self.pool.tag(self.pool.resolve_fully(idx)) == Tag::Float);
  if is_float_field {
      if let Some(BasicTypeEnum::FloatType(field_float)) = st.get_field_type_at_index(i as u32) {
          // f32 field but f64 value → fptrunc
          // Compare LLVM types: field is f32 but value is f64
          let v = self.builder.arena.get_value(val);
          if v.is_float_value() {
              let val_float_ty = v.into_float_value().get_type();
              // inkwell FloatType does not have get_bit_size(); compare types directly
              if val_float_ty != field_float {
                  let field_ty_id = self.builder.register_type(field_float.into());
                  return self.builder.float_trunc(val, field_ty_id, &format!("fptrunc.{i}"));
              }
          }
      }
  }
  ```
  **Prerequisite:** `IrBuilder::float_trunc()` must be added first (see checklist item above). The `trunc_for_narrowed_struct()` closure currently uses `.map(|(i, &val)| { ... })` — the new float branch must be added inside this closure, after the existing int-field check at `emitter_utils.rs:535-559`. **Implementation note:** the existing code structure is a `.map()` closure returning `val` (unchanged) or a truncated value — the float branch follows the same return pattern.

- [x] Extend `sext_narrowed_field()` for float fields:
  The existing `sext_narrowed_field()` in `emitter_utils.rs:573` only handles `Tag::Int` fields (checks `pool.tag() == Tag::Int` and uses integer `sext`). §05 must extend this to ALSO handle `Tag::Float` fields. When the ARC IR destination type is `Tag::Float` (expects f64) but the extracted value is f32, insert `fpext float to double`.

  **Implementation approach:** In `sext_narrowed_field()` at `emitter_utils.rs:573`, add a float branch after the existing int check (line 583). The existing function structure is: check `Tag::Int` → check `is_int_value()` → check `bits < 64` → `sext`. The float branch mirrors this:
  ```rust
  // Existing: integer sign-extension for narrowed int fields (line 583)
  if self.pool.tag(resolved) == Tag::Int { /* existing sext logic */ }

  // NEW: float extension for narrowed float fields
  if self.pool.tag(resolved) == Tag::Float {
      let v = self.builder.arena.get_value(extracted);
      if v.is_float_value() {
          let val_float_ty = v.into_float_value().get_type();
          let canonical_f64 = self.builder.scx.type_f64();
          // If extracted value is f32 but destination expects f64, extend
          if val_float_ty != canonical_f64 {
              let f64_ty_id = self.builder.register_type(canonical_f64.into());
              return self.builder.float_ext(extracted, f64_ty_id, &format!("fpext.{field_index}"));
          }
      }
  }
  ```
  **Note:** The existing int path accesses `self.builder.scx` (not `self.scx`) for type construction — see `emitter_utils.rs:594-595` pattern. The `self.builder.register_type()` → `self.builder.scx.type_f64()` chain follows the same pattern as the existing `self.builder.register_type(self.builder.scx.type_i64().into())` at line 595. **Prerequisite:** `IrBuilder::float_ext()` must be added first.

<!-- IrBuilder float_trunc/float_ext methods already listed above as prerequisite — not duplicated here -->

- [x] Extend `try_lower_narrowed_aggregate()` for float-narrowed fields:
  `try_lower_narrowed_aggregate()` in `layout_resolver.rs:318` has two relevant guards:

  **Guard 1 — `has_narrowed` (line 335-346):** Currently only checks for `Int { width != I64 }`. A struct with ONLY float fields narrowed to F32 would have `has_narrowed = false` and fall through to the TypeInfoStore named-struct path (incorrect — the struct IS narrowed). **Fix**: extend the `has_narrowed` check to also detect `Float { width: F32 }`:
  ```rust
  let has_narrowed = fields.iter().any(|f| {
      matches!(
          f.repr,
          MachineRepr::Int { width, .. } if width != ori_repr::IntWidth::I64
      ) || matches!(
          f.repr,
          MachineRepr::Float { width } if width != ori_repr::FloatWidth::F64
      )
  });
  ```

  **Guard 2 — `all_scalar_int` (line 354-366):** Despite its misleading name, this guard already accepts `MachineRepr::Float { .. }` in the match (line 358). No change is needed to the match itself. **However**, rename the variable from `all_scalar_int` to `all_scalar_fields` to reflect that it covers both int and float fields (hygiene improvement).

- [x] Function boundaries remain canonical f64:
  Function parameters and return values always use f64 (canonical `float`). Float narrowing only affects struct field storage. No ABI changes. This is identical to §04's rule that function boundaries remain canonical i64.

**Risk:** The `ArcIrEmitter` must have access to the `ReprPlan` to know which fields are narrowed. The `TypeLayoutResolver` already holds `repr_plan: Option<&ReprPlan>` (line 48), and the emitter accesses it via `self.resolver.repr_plan()` (line 76). This access path is already live from §04's integer narrowing.

**BLOAT note:** `emitter_utils.rs` is currently **600 lines** (exceeds the 500-line limit). Adding float branches to `trunc_for_narrowed_struct()` and `sext_narrowed_field()` will push it further over. **Before implementing §05.4**, extract `trunc_for_narrowed_struct()` (line 501-563) and `sext_narrowed_field()` (line 573-599) into a new submodule `arc_emitter/narrowing_codegen.rs`, leaving forwarding methods in `emitter_utils.rs`. This is a prerequisite, not optional cleanup.

---

## 05.5 Completion Checklist

**Test matrix for §05 (write failing tests FIRST, verify they fail, then implement):**

| Input pattern | Expected narrowing | Semantic pin |
|---|---|---|
| Struct field `r: float` where all stored literals are `0.0` | `Float { F32 }` in MachineRepr | Yes — field_repr.repr is F32 |
| Struct field `scale: float` storing constant `0.5` | `Float { F32 }` storage | Yes — `is_f32_exact(0.5) == true` |
| Struct field `x: float` storing constant `1e300` (non-f32-exact) | `Float { F64 }` — no narrowing | Yes — `is_f32_exact(1e300) == false` |
| Float arithmetic `a + b` → result stored in struct field | `Float { F64 }` — conservative (no narrowing) | Yes — arithmetic result stays f64 |
| `struct Color { r: float, g: float, b: float }` with all values from `0.0`, `0.5`, `1.0` literals | `Float { F32 }` fields | Yes — all literal values are f32-exact |
| `float` field receiving variable (non-literal) | `Float { F64 }` — conservative | Yes — variable source is unknown |
| `is_f32_exact(0.1)` | `false` (0.1 is not f32-exact) | Yes — precision pin |
| `is_f32_exact(0.5)` | `true` (exact in f32) | Yes |
| `is_f32_exact(-0.0)` | `true` (negative zero is f32-exact) | Yes |
| `is_f32_exact(f64::INFINITY)` | `true` (infinity is f32-exact) | Yes |
| `is_f32_exact(f64::NAN)` | `false` (NaN payload may not survive) | Yes |
| Struct with `#repr("c")` and f32-exact fields | `Float { F64 }` — not narrowed (ABI) | Yes — negative pin |
| Public struct with f32-exact fields | `Float { F64 }` — not narrowed (ABI) | Yes — negative pin |
| Private struct with mix of f32-exact and non-f32-exact fields | Only f32-exact fields narrowed | Yes — per-field decision |
| Struct with `f64::MIN_POSITIVE` (smallest positive f64 normal, `2.2250738585072014e-308`) | `Float { F64 }` — not f32-exact (below f32 smallest subnormal, flushes to zero in f32) | Yes — subnormal edge |
| Struct with `#repr("aligned", 16)` and f32-exact fields | `Float { F32 }` — narrowed (aligned only sets whole-struct alignment, not field layout) | Yes — aligned does NOT prevent narrowing |
| Struct with int [0,255] AND float 0.5 fields (§04+§05 combined) | `{ i8, f32 }` — both narrowed | Yes — combined §04+§05 semantic pin |
| Tuple `(float, float)` with f32-exact values | `Float { F64 }` — not narrowed (Phase A defers tuples) | Yes — tuple deferral negative pin |
| `NarrowingPolicy::Disabled` with f32-exact struct fields | `Float { F64 }` — not narrowed | Yes — policy negative pin |
| Struct field storing `f32::MAX as f64` (boundary f32 value) | `Float { F32 }` — f32-exact | Yes — boundary positive pin |
| Struct field storing `(f32::MAX as f64) * 2.0` (just beyond f32 range) | `Float { F64 }` — overflows to f32::INFINITY, not lossless | Yes — boundary negative pin |

**Unit tests (Rust, in `compiler/ori_repr/src/narrowing/float/tests.rs`):**

**Note:** `narrowing/tests.rs` already exists and contains integer narrowing tests (§04). Float tests MUST go in `narrowing/float/tests.rs` (sibling test file convention — `float.rs` → `float/tests.rs`). This means converting `float.rs` to `float/mod.rs` + `float/tests.rs` directory structure. Add `#[cfg(test)] mod tests;` at the bottom of `float/mod.rs`.

- [x] `is_f32_exact()` correctly identifies f32-representable f64 values:
  - `is_f32_exact(0.0)`, `is_f32_exact(1.0)`, `is_f32_exact(0.5)`, `is_f32_exact(100.0)` → `true`
  - `is_f32_exact(1.0 / 3.0)`, `is_f32_exact(1e300)`, `is_f32_exact(f64::NAN)` → `false`
  - `is_f32_exact(-0.0)` → `true` (negative zero is f32-exact)
  - `is_f32_exact(f64::INFINITY)` → `true`, `is_f32_exact(f64::NEG_INFINITY)` → `true`
  - `is_f32_exact(f32::MAX as f64)` → `true`, `is_f32_exact(f32::MIN as f64)` → `true`
  - `is_f32_exact(f64::MAX)` → `false` (exceeds f32 range)
  - `is_f32_exact(16777216.0)` → `true` (2^24, last exact f32 integer)
  - `is_f32_exact(16777217.0)` → `false` (2^24 + 1, not f32-exact)
  - `is_f32_exact(f64::MIN_POSITIVE)` → `false` (f64 subnormal boundary — too small for f32 normal range)
  - `is_f32_exact(f32::MIN_POSITIVE as f64)` → `true` (f32's smallest positive normal)
  - `is_f32_exact(1e-45_f64)` → verify empirically: `1e-45_f64` is approximately `1.401298e-45` which is `f32::MIN_POSITIVE * 2^-23` (the smallest f32 subnormal). The roundtrip `1e-45_f64 as f32 as f64` may produce a slightly different value due to f64→f32 rounding. Test the actual roundtrip result. If the Rust expression `(1e-45_f64 as f32) as f64 == 1e-45_f64` is false, the expected result is `false`.
- [x] `FloatRange` lattice operations: `Bottom.join(F32Exact) == F32Exact`, `F32Exact.join(Top) == Top`, `Bottom.join(Top) == Top`, `F32Exact.join(F32Exact) == F32Exact`, `Bottom.join(Bottom) == Bottom`, `Top.join(Top) == Top`
- [x] `FloatRange::observe()`: `Bottom.observe(0.5) == F32Exact`, `F32Exact.observe(0.1) == Top`, `Top.observe(0.5) == Top`
- [x] `FloatRange::observe_arithmetic()`: `Bottom.observe_arithmetic() == Top`, `F32Exact.observe_arithmetic() == Top` (conservative — any arithmetic blocks narrowing)
- [x] `narrow_float_fields()` narrows f32-exact struct fields
- [x] `narrow_float_fields()` preserves f64 for non-f32-exact struct fields
- [x] `narrow_float_fields()` skips `#repr("c")` types
- [x] `narrow_float_fields()` skips `#repr("packed")` types
- [x] `narrow_float_fields()` skips `#repr("transparent")` types
- [x] `narrow_float_fields()` does NOT skip `#repr("aligned", N)` types (aligned only sets whole-struct alignment)
- [x] `narrow_float_fields()` skips public types
- [x] `narrow_float_fields()` skips when `NarrowingPolicy::Disabled` (caller gates, verified)
- [x] `narrow_float_fields()` skips tuples (Phase A — same as §04)
- [x] Mixed struct: some int fields narrowed by §04, some float fields narrowed by §05, others kept canonical
- [x] Combined int+float narrowing: `struct Pixel { r: int, x: float }` where r in [0, 255] and x is always 0.5 → `{ i8, f32 }` (both narrowed)
- [x] `FloatFieldSummaryTable` correctly joins multiple observations: two f32-exact stores → `F32Exact`; one f32-exact + one non-f32-exact → `Top`
- [x] `collect_float_field_summaries()` ignores non-float fields (int, bool, str fields produce no float entries — type filter in collection loop)
- [x] `collect_float_field_summaries()` handles struct with all float fields, all f32-exact → all `F32Exact` (table observe logic verified)
- [x] `collect_float_field_summaries()` handles struct with no Construct sites → all fields `Bottom` (no evidence, no narrowing)

**AOT tests (Rust, in `compiler/ori_llvm/tests/aot/narrowing.rs` — extend §04's test file):**

- [x] `test_float_narrowed_struct_roundtrip`: Struct with f32-exact fields (0.0, 0.5, 1.0), store and load, verify bit-identical results (2026-03-29)
- [x] `test_float_narrowed_struct_ir_pin_type_layout`: Verify `{ float, float, float }` (not `{ double, double, double }`) in LLVM IR for Color struct with f32-exact fields (2026-03-29)
- [x] `test_float_narrowed_struct_ir_pin_fptrunc_on_construction`: Verify `fptrunc double to float` in LLVM IR at struct construction (2026-03-29)
- [x] `test_float_narrowed_struct_ir_pin_fpext_on_field_load`: Verify `fpext float to double` in LLVM IR at field extraction (2026-03-29)
- [x] `test_float_non_narrowed_struct_ir_pin_wide_value`: Negative pin — struct with `1e300` field shows NO `fptrunc` (2026-03-29)
- [x] `test_float_arithmetic_not_narrowed`: Float arithmetic result stored in struct field stays f64 (2026-03-29)
- [x] `test_float_variable_not_narrowed`: Non-literal float variable stored in struct field stays f64 (2026-03-29)
- [x] `test_mixed_int_float_narrowed_struct`: Struct with int [0,255] and float 0.5 → `{ i8, float }` in LLVM IR — verifies combined §04+§05 narrowing (2026-03-29)
- [x] `test_float_repr_c_not_narrowed`: Negative pin — `#repr("c")` struct with f32-exact fields shows `double` (not `float`) in LLVM IR (2026-03-29)
- [x] Release parity verified: all 15 float AOT tests pass in both `cargo test -p ori_llvm -- float_narrowed` (debug) and `cargo test --release -p ori_llvm -- float_narrowed` (release) — FastISel vs full ISel parity confirmed (2026-03-29). No separate release-specific test needed; the same tests exercise both ISel paths.
- [x] `test_float_narrowed_derive_printable`: Derived Printable on narrowed float struct — TPR-05-001 regression guard (2026-03-29)
- [x] `test_float_narrowed_derive_debug`: Derived Debug on narrowed float struct (2026-03-29)
- [x] `test_float_narrowed_derive_hash`: Derived Hashable on narrowed float struct (2026-03-29)
- [x] `test_float_narrowed_derive_eq`: Derived Eq on narrowed float struct (2026-03-29)
- [x] `test_float_narrowed_derive_comparable`: Derived Comparable on narrowed float struct (2026-03-29)
- [x] `test_float_narrowed_derive_ir_pin_fpext_in_printable`: IR pin — fpext in derive Printable codegen (2026-03-29)

**All AOT tests must pass in BOTH debug and release builds.** FastISel (debug) and the full instruction selector (release) handle `fptrunc`/`fpext` differently; a test that passes in debug but fails in release is a real bug.

**Spec tests (Ori, in `tests/spec/repr/float_narrowing/`):**

These are end-to-end behavioral tests. The Ori code itself does not observe the narrowing (it is a hidden optimization), so these tests verify semantic transparency — the program produces the same result whether or not narrowing occurs.

- [ ] `basic_roundtrip.ori`: Struct storing `0.5` produces bit-identical results to canonical f64 path — `assert_eq(p.x, 0.5)`
- [ ] `arithmetic_transparent.ori`: Arithmetic on float values is not affected by field narrowing — store `0.5` in struct, load, add `1.0`, verify result is `1.5`
- [ ] `mixed_fields.ori`: Struct with mix of f32-exact (`0.5`) and non-f32-exact (`0.1`) fields — both fields produce correct values
- [ ] `multiple_stores.ori`: Same struct field written at multiple call sites with different f32-exact values (`0.0`, `1.0`, `255.0`) — all roundtrip correctly
- [ ] `combined_int_float.ori`: Struct with both int and float fields — both types produce correct values after narrowing
- [ ] `negative_zero.ori`: Struct storing `-0.0` — verify `-0.0` is preserved (not collapsed to `0.0`)
- [ ] `boundary_values.ori`: Struct storing `f32::MAX`-equivalent (`3.4028235e38`) — roundtrip produces correct value
- [ ] `comparison_after_load.ori`: Float field loaded from narrowed struct used in comparison — `assert_eq(p.x < 1.0, true)` for `p.x = 0.5`

**Integration:**

- [ ] Write failing test matrix BEFORE implementation (verify tests fail with current `Float { F64 }`-only codegen)
- [ ] Interpreter and LLVM produce identical results for all new spec tests (dual-execution parity per CLAUDE.md Fix Completeness)
- [ ] Bit-identical results: `./diagnostics/dual-exec-verify.sh` passes (zero precision differences)
- [x] `./test-all.sh` green in debug (14,496 passed, 0 failed — 2026-03-29, post-TPR fix)
- [x] `./clippy-all.sh` green (2026-03-29)
- [ ] `./diagnostics/valgrind-aot.sh` clean (no memory errors from fptrunc/fpext)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all float narrowing test programs

**Cleanup (after §05 is complete):**

- [ ] Remove any `§05` / `TPR-05-*` code annotations from production code (per CLAUDE.md: plan annotations are temporary scaffolding)
- [ ] Verify `narrowing/mod.rs` module doc is updated to include float module
- [ ] Verify `lib.rs` re-exports are clean

**Exit Criteria:** A program storing constant `0.5` in a private struct field uses `Float { F32 }` in the `ReprPlan` and `float` (f32) in LLVM IR instead of `double` (f64), verified by inspecting generated IR. A struct with both int (narrowed by §04) and float (narrowed by §05) fields shows the combined narrowed LLVM type. All floating-point spec tests continue to pass with bit-identical results. The `fptrunc double to float` at store and `fpext float to double` at load are visible in `ORI_DUMP_AFTER_LLVM=1` output. §07's `find_niches()` returns empty for `MachineRepr::Float { F32 }` fields (verified by §07's test matrix, documented here as handoff contract).

---

## 05.R Third Party Review Findings

- [x] `[TPR-05-001][major]` [`compiler/ori_llvm/src/codegen/derive_codegen/string_helpers.rs:116`](/home/eric/projects/ori_lang/compiler/ori_llvm/src/codegen/derive_codegen/string_helpers.rs#L116) — **Derived `Printable`/`Debug` on narrowed float fields passes `float` to the `ori_str_from_float(double)` runtime ABI, producing invalid LLVM IR.**
  Resolved: Fixed on 2026-03-29. Added `fpext_to_f64_if_narrower()` call in `emit_field_to_string()` Float arm (mirroring the integer `sext_to_i64_if_narrower` pattern). Added `test_float_narrowed_derive_printable`, `test_float_narrowed_derive_debug`, and `test_float_narrowed_derive_ir_pin_fpext_in_printable` as regression guards. All 14,496 tests pass in both debug and release.
- [x] `[TPR-05-002][major]` [`plans/repr-opt/section-05-float-narrowing.md:309`](/home/eric/projects/ori_lang/plans/repr-opt/section-05-float-narrowing.md#L309) — **§05.4 marked complete without required float AOT regression suite.**
  Resolved: Fixed on 2026-03-29. Added 15 float AOT tests covering: roundtrip, IR type layout pin, fptrunc/fpext IR pins, negative pins (wide values, arithmetic, variables, `#repr("c")`), mixed int+float narrowing, and 5 derive trait tests (Printable, Debug, Hash, Eq, Comparable). All pass in debug and release builds.
- [x] `[TPR-05-003][medium]` [`plans/repr-opt/section-05-float-narrowing.md:517`](/home/eric/projects/ori_lang/plans/repr-opt/section-05-float-narrowing.md#L517) — **The §05.5 checklist and resolved TPR note overstate the landed release regression coverage.**
  Resolved: Fixed on 2026-03-29. Corrected test count from 16 to 15, replaced nonexistent `test_float_narrowed_struct_release_roundtrip` entry with actual release verification evidence (all 15 tests pass in both debug and release builds).
- [x] `[TPR-05-004][medium]` [`compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs:501`](/home/eric/projects/ori_lang/compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs#L501) — **§05.4 added more narrowing code to an over-limit emitter file instead of performing the prerequisite extraction.**
  Resolved: Fixed on 2026-03-29. Extracted 14 narrowing methods into `arc_emitter/narrowing_codegen.rs` (469 lines). `emitter_utils.rs` reduced from 652 to 205 lines.
- [x] `[TPR-05-005][medium]` [`compiler/ori_repr/src/lib.rs:490`](/home/eric/projects/ori_lang/compiler/ori_repr/src/lib.rs#L490) — **The float narrowing work touched `lib.rs` without performing the planned extraction, leaving `lib.rs` as a 545-line implementation file.**
  Resolved: Fixed on 2026-03-29. Extracted all function bodies into `pipeline.rs` (497 lines). `lib.rs` reduced from 545 to 60 lines (pure index: `//!` docs, `mod` declarations, `pub use` re-exports only).

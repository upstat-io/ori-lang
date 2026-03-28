---
section: "04"
title: "Integer Narrowing Pipeline"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-28
  triage_note: "Open finding: TPR-04-025 (emit_function.rs exceeds the 500-line source-file limit). Previously resolved: TPR-04-024 accepted on 2026-03-28 (Select narrowing tasks added to §04.4), TPR-04-023 accepted (straight-line local narrowing tasks), TPR-04-018→IR-PIN-04-018, TPR-04-019→MIXED-PIN-04-019, TPR-04-020→DERIVE-PIN-04-020, TPR-04-021 (Debug str leak), TPR-04-022 (stale Debug known_gap). CROSS-04-017 remains accepted follow-up in §04.X."
goal: "Lower int (semantic i64) to the smallest machine integer (i8/i16/i32) that preserves correctness, saving memory in struct fields, collections, and stack slots"
inspired_by:
  - "Zig comptime_int narrowing to runtime types (src/Sema.zig)"
  - "Roc NumericRange → concrete layout (crates/compiler/mono/src/layout.rs)"
  - "LLVM InstCombine integer truncation (lib/Transforms/InstCombine/)"
depends_on: ["03"]
sections:
  - id: "04.1"
    title: "Width Selection Algorithm"
    status: complete
  - id: "04.2"
    title: "ABI Boundary Widening"
    status: complete
  - id: "04.3"
    title: "Overflow Guard Insertion"
    status: complete
  - id: "04.4"
    title: "LLVM Codegen Integration"
    status: in-progress
  - id: "04.5"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04: Integer Narrowing Pipeline

**Context:** Today, every `int` is `i64` in LLVM IR. A loop counter that goes `0..100` wastes 7 bytes per element in array storage. A struct with `{ x: int, y: int }` where both fields are always `0..255` uses 16 bytes instead of 2. The savings compound in collections: `[Point]` with 1M elements wastes 14MB.

**Reference implementations:**
- **Zig** `src/Sema.zig`: `coerceInMemoryAllowedPtrAbiType()` — coerces comptime_int to runtime width
- **Roc** `crates/compiler/mono/src/layout.rs`: `Layout::from_var()` — selects concrete layout from type variable constraints
- **LLVM** `lib/Transforms/InstCombine/InstCombineCasts.cpp`: Integer truncation elimination

**Depends on:** §03 (range analysis provides the intervals).

---

## 04.1 Width Selection Algorithm

**File(s):** `compiler/ori_repr/src/narrowing/int.rs`

**Setup required:** `compiler/ori_repr/src/narrowing/` does not exist yet. Steps:
1. Create the `compiler/ori_repr/src/narrowing/` directory
2. Create `mod.rs` (dispatch hub), `int.rs` (integer narrowing), `abi.rs` (ABI widening), `overflow.rs` (overflow guards), `tests.rs` (sibling test file)
3. Add `pub mod narrowing;` to `compiler/ori_repr/src/lib.rs`
The `apply_integer_narrowing()` stub in `lib.rs` (line 215) is the entry point — fill it in to call into `narrowing/int.rs`.

Given a `ValueRange`, select the minimum integer width that preserves the semantic contract.

- [x] Implement width selection (2026-03-26): Created `compiler/ori_repr/src/narrowing/int.rs` with `narrow_struct_fields()`. Uses `ValueRange::min_width()` from `range/mod.rs` — no duplicate function. Iterates `plan.decision_indices()` to find Struct/Tuple types, narrows `Int { I64, signed: true }` fields to smallest width from field-range summaries. Wired into `apply_integer_narrowing()` in `lib.rs`.

- [x] Apply conservatism rules — implemented subset (2026-03-26):
  - [x] `#repr("c")` / `#repr("packed")` / `#repr("transparent")` types skip narrowing (`has_fixed_layout_attr()`)
  - [x] `#repr("c", aligned N)` types skip narrowing (TPR-04-004 fix, 2026-03-26)
  - [x] `NarrowingPolicy::Disabled` skips all narrowing
  - [x] Only canonical `Int { I64, signed: true }` fields are candidates
  - [x] Fields with `Top` range stay at I64 (safe default)
  - [x] Field-summary-driven narrowing: only from §03's `FieldSummaryTable` built from `Construct` sites
- [x] Apply conservatism rules — visibility-based gating (TPR-04-005, implemented 2026-03-26): Public API types are now excluded from integer narrowing. Implementation:
  - [x] Added `pub_type_indices: FxHashSet<Idx>` to `ReprPlan` (`plan.rs`) with `set_pub_type_indices()` and `is_public_type()` API
  - [x] `compute_repr_plan_with_interner()` accepts `pub_type_indices: &[Idx]` parameter; stored in Phase 0b
  - [x] Both call sites (`codegen_pipeline.rs`, `evaluator/compile.rs`) extract public type indices from `TypeEntry::visibility == Visibility::Public`
  - [x] `narrow_struct_fields()` gates on `plan.is_public_type(idx)` — public types skip narrowing with tracing
  - [x] Test `public_type_not_narrowed`: pub struct with bounded fields → stays I64
  - [x] Test `private_type_narrowed_normally`: private struct narrowed, pub struct preserved in same plan
- [x] Apply conservatism rules — monomorphized generic type propagation (TPR-04-012, implemented 2026-03-27): Generic type instantiations (`Applied → concrete Struct`) create distinct pool idxs that bypass repr/public exemptions. Fix: added `propagate_metadata_to_applied_resolutions()` as Phase 0c in `compute_repr_plan_with_interner()` (`lib.rs`). Collects protected type Names from repr_attrs/pub_type_indices, scans all pool Applied entries, resolves through pool chain, propagates metadata to concrete Struct idx. Implementation:
  - [x] Phase 0c in `compute_repr_plan_with_interner()`: `propagate_metadata_to_applied_resolutions()` iterates pool for Applied types matching protected Names, resolves each, propagates repr_attr and pub_type to resolved concrete Struct idx (2026-03-27)
  - [x] 6 regression tests in `tests.rs`: `repr_attr_propagates_through_applied_to_concrete_struct`, `pub_type_propagates_through_applied_to_concrete_struct`, `repr_c_applied_concrete_struct_not_narrowed_semantic_pin`, `pub_applied_concrete_struct_not_narrowed_semantic_pin`, `applied_without_resolution_no_propagation` (negative), `multiple_applied_instantiations_all_protected` (2026-03-27)
  - [x] Semantic pin: #repr("c") Named type with monomorphized Applied → Struct — narrowing blocked on mono struct (stays I64). ONLY passes with Phase 0c propagation (2026-03-27)
  - [x] Semantic pin: pub Named type with monomorphized Applied → Struct — narrowing blocked on mono struct (stays I64). ONLY passes with Phase 0c propagation (2026-03-27)
  - [x] 381/381 ori_repr debug + release green, 14,236 total tests green (2026-03-27)
- **Conservatism design rules** (enforced incrementally by Phase A/B/C):
  - **Local variables** (Phase B): narrow aggressively (widening is free in registers)
  - **Struct fields** (Phase A — done): narrow from field-summary table only
  - **Function parameters** (Phase B): narrow only if ALL call sites agree on the range
  - **Function returns** (Phase B): narrow only if ALL callers can handle the narrow type
  - **Collection elements** (Phase C): narrow aggressively (savings multiply by element count)
  - **Public API types** (Phase A — done): do NOT narrow (gated by `is_public_type()`)
  - **Address-taken functions / indirect-call targets** (Phase B): do NOT narrow parameters or returns
  - **Closure captures** (Phase B): canonical width in closure environment

- [x] Use the existing `NarrowingPolicy` from `compiler/ori_repr/src/plan/query.rs` (2026-03-26): `narrow_struct_fields()` checks `plan.narrowing_policy() == Disabled` and returns early. Policy consumed via existing API.
  `NarrowingPolicy` already exists in `compiler/ori_repr/src/plan/query.rs` with three variants:
  ```rust
  // Existing type — do NOT redeclare in narrowing/int.rs
  pub enum NarrowingPolicy {
      Aggressive,    // Apply all safe narrowing optimizations (default)
      Conservative,  // Apply only provably-safe narrowing (no heuristics)
      Disabled,      // No narrowing — canonical representations only
  }
  ```
  The narrowing pass reads the policy via `plan.narrowing_policy()` (already implemented).
  Per-site policy (e.g., "min 2 bytes savings") is handled by the conservatism rules above,
  not by a field on the enum variant.

---

## 04.2 ABI Boundary Widening

**File(s):** `compiler/ori_repr/src/narrowing/abi.rs`

At function boundaries and FFI, narrowed integers must be widened back to canonical width. This is critical for correctness.

- [x] Define ABI boundary rules (2026-03-26): Created `compiler/ori_repr/src/narrowing/abi.rs` with `AbiBoundary` enum (5 variants: Ffi, PublicApi, TraitMethod, ClosureCapture, InternalCall), `WidthRequirement` enum (Canonical, NarrowIfAgreed, PlatformCabi), `CrossModuleAgreement` enum (Agreed, Disagreed, Unknown). Policy functions: `width_requirement()`, `can_narrow_param()`, `can_narrow_return()`, `classify_function_boundary()`, `effective_boundary_width()`, `needs_sext_at_boundary()`, `needs_trunc_after_boundary()`. Exported from crate root. 24 tests in `narrowing/tests.rs` including boundary classification priority, width requirement matrix, cross-module agreement, sext/trunc detection, and 2 semantic pin tests.

- [x] Implement widening insertion policy (2026-03-26): Widening rules encoded in `abi.rs` policy functions. `effective_boundary_width()` returns the required width at any boundary: public/trait/closure/FFI → always I64 (canonical); internal + agreed → narrowed width; internal + disagreed/unknown → I64. `needs_sext_at_boundary()` and `needs_trunc_after_boundary()` detect where sext/trunc instructions are needed. The actual LLVM `sext`/`trunc` emission is deferred to §04.4 (LLVM Codegen Integration). Rules:
  - Before public function return: `sext i32 %narrow to i64`
  - Before FFI call arguments: widen to C-ABI width
  - At module import boundaries: widen to canonical
  - When storing to generic collection: widen if collection is exported
  - Closure environments: treat capture slots as canonical-width storage

- [x] Cross-module narrowing via Merkle hashes (2026-03-26): `CrossModuleAgreement` enum models the three states (Agreed, Disagreed, Unknown). `can_narrow_cross_module()` returns true only for `Agreed`. `effective_boundary_width()` integrates agreement status into width decisions. `Unknown` is treated conservatively as `Disagreed` until the module system implements Merkle hash comparison. The Merkle hash already includes `MachineRepr`, so different representations produce different hashes.

---

## 04.3 Overflow Guard Insertion

**File(s):** `compiler/ori_repr/src/narrowing/overflow.rs`

When a value is narrowed, arithmetic operations might overflow the narrow type even though they wouldn't overflow the canonical i64. The compiler must insert overflow checks.

- [x] Implement overflow analysis (2026-03-27): Created `compiler/ori_repr/src/narrowing/overflow.rs` with `can_overflow(op: BinaryOp, lhs: ValueRange, rhs: ValueRange, target: IntWidth) -> bool`. Uses all available range transfer functions from §03: `range_add/sub/mul/div/mod/floordiv/shl/shr/bitand/bitor/bitxor`. Exhaustive match on all 23 `BinaryOp` variants — comparison/logical/range/coalesce/matmul conservatively return `Top`. 17 tests including Add/Sub/Mul overflow detection, arithmetic ops matrix, Bottom/Top edge cases.

- [x] Overflow strategy recommendation (2026-03-27): Created `OverflowStrategy` enum with three variants: `ProvenSafe` (range proves no overflow — zero cost), `WidenCompute { intermediate_width }` (sext operands, compute at wider type, trunc result — low cost), `UseCanonical` (use i64 — forward compat, currently unreachable since i64 covers all values). `recommend_strategy()` function implements priority: (c) ProvenSafe when `!can_overflow()`, (a) WidenCompute when result fits in `next_wider(target)`, (b) UseCanonical otherwise. Tests verify strategy progression, I8→I16→I32→I64 widening chain.

- [x] Decision codified (2026-03-27): prefer (c) ProvenSafe when provable, (a) WidenCompute for rare overflow, (b) UseCanonical when overflow exceeds next-wider. Note: with signed i64 as canonical, `UseCanonical` is currently unreachable — any result range that overflows I8/I16/I32 always fits in the next-wider type up to I64. The variant exists for forward compatibility (future unsigned narrowing or i128).

---

## 04.4 LLVM Codegen Integration

**Integration note:** `TypeInfo::storage_type()` does NOT consult `ReprPlan` and must not be modified for integer narrowing. The actual integration point is `TypeLayoutResolver::try_repr_to_llvm_type()` in `compiler/ori_llvm/src/codegen/type_info/mod.rs` (lines 167–229). **Current state:** Primitive `MachineRepr::Int { width }` → i8/i16/i32/i64 already works (lines 171–176), and `TypeLayoutResolver::resolve_inner()` queries `repr_plan.get_repr(idx)` first (line 249). However, `MachineRepr::Struct`/`Tuple` return `None` and fall back to `TypeInfoStore` canonical `i64` fields — struct/tuple lowering is **pending** until `try_repr_to_llvm_type()` recursively consumes narrowed `FieldRepr` widths (see Phase A LLVM struct/tuple lowering tasks below).

**Per-variable vs per-type `Idx`:** `ReprPlan::int_width(idx)` queries a *type* `Idx`. All local `int` variables share `Tag::Int`, so per-variable local narrowing needs either (a) a `per-(function, var)` decision map in `ReprPlan`, or (b) deriving the width on-the-fly in the emitter from `plan.var_range(func, var).min_width()`. Struct-field narrowing is clean: the struct type gets a new `MachineRepr::Struct` with narrowed `FieldRepr`. Local variable narrowing is Phase B.

**File(s):**
- `compiler/ori_repr/src/narrowing/int.rs` — the `apply_integer_narrowing()` implementation (populates `ReprPlan` with narrowed `MachineRepr::Struct` decisions for struct types whose fields have bounded ranges from §03)
- `compiler/ori_llvm/src/codegen/type_info/mod.rs` — `TypeLayoutResolver::try_repr_to_llvm_type()` (handles `MachineRepr::Int { width }` → i8/i16/i32/i64; **must be extended to handle `MachineRepr::Struct`/`Tuple` by recursively lowering `FieldRepr` widths — see §04.4 tasks**)
- `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs` — `emit_literal()` emits `const_i64` for all int literals today; for narrowed locals, must emit narrowed constant when variable's type has been narrowed
- `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs` — struct construction; `sext`/`trunc` inserts needed at field store boundaries when field width differs from operand width

The LLVM backend type resolution path via `TypeLayoutResolver` handles `MachineRepr::Int { width }` but **does not yet handle recursive `MachineRepr::Struct`/`Tuple`** — those return `None` from `try_repr_to_llvm_type()` and fall back to `TypeInfoStore` canonical `i64` fields. §04.4 must extend `try_repr_to_llvm_type()` to recursively lower `FieldRepr` widths for struct/tuple decisions (TPR-04-006 fix).

- [x] **Phase A — Struct field narrowing (primary):** (2026-03-27): `apply_integer_narrowing()` calls `narrow_struct_fields()` which iterates all struct types with Struct reprs. For each struct field of type `int`, queries `plan.field_range(struct_idx, field_index)`. If `range.min_width() < I64`, emits a narrowed `MachineRepr::Struct` decision with updated `FieldRepr` entries. **Bug fix (IR-PIN-04-018)**: narrowed decisions were stored only under the original Pool index (e.g., `Named("Pixel")`) but codegen always canonicalizes via `pool.resolve_fully()` to the concrete `Struct(fields)` index. Fixed by propagating narrowed decisions to resolved indices (mirrors Phase 0 pattern for `#repr` attrs). Also fixed derive codegen (hash, printable, debug) to sext narrowed i8/i16/i32 fields to canonical i64 before passing to runtime functions. Scoped `try_lower_narrowed_aggregate()` to all-scalar-int structs only — mixed-type structs (str + int) need Phase C element_store_size integration. Original plan code block:
  ```rust
  fn apply_integer_narrowing(plan: &mut ReprPlan, pool: &Pool) {
      // Iterate Pool for struct/tuple types with int fields.
      // For each field with a narrowable range, replace the canonical
      // MachineRepr::Struct with one having narrowed FieldRepr widths.
      // TypeLayoutResolver already reads MachineRepr::Struct and uses
      // FieldRepr — no LLVM codegen changes needed.
  }
  ```
  **§04/§06 interface contract**: §04 writes only the `FieldRepr.repr` field (narrowed width). It does NOT compute `FieldRepr.offset`, `StructRepr.size`, or `StructRepr.align` — those are §06's exclusive responsibility. §04 sets these to zero as placeholders. §06 reads the narrowed `repr` values to compute correct field sizes and then runs the alignment-optimal reordering algorithm. No code downstream of §04 but upstream of §06 may read `FieldRepr.offset` or `StructRepr.size`.

- [ ] **Phase B — Local variable narrowing:** Local `int` variables all share the same Pool `Idx` (`Tag::Int`), so per-variable narrowing cannot use the type-keyed `ReprPlan::int_width(idx)` alone. Two options:
  - Option 1: Add a per-(function, var) map to `ReprPlan` for local variable decisions (new `local_int_width` field: `FxHashMap<(Name, ArcVarId), IntWidth>`). Query in `ArcIrEmitter` when emitting variable-producing instructions.
  - Option 2: In the ARC IR emitter, derive the width on-the-fly from `plan.var_range(func, var).min_width()` for each variable at emission time (avoids new map, slightly more computation at codegen time).
  **Recommended:** Option 2 for simplicity — query `plan.var_range(func, var).min_width()` in the emitter when producing the alloca/phi/store for a local int variable.

- [ ] **Phase C — Collection element narrowing:** The §04 goal includes "collections". A `[int]` array where all stored values are `0..255` currently uses 8 bytes per element; narrowing to `i8` saves 7/8 of the storage. The canonical representation for `[int]` is `MachineRepr::FatPointer(FatRepr::Collection { element_repr: Box::new(MachineRepr::Int { width: I64, signed: true }) })`. §04 should update the `element_repr` when the element range is bounded.

  **Critical codegen gap:** `TypeLayoutResolver::try_repr_to_llvm_type()` at lines 200–211 matches `MachineRepr::FatPointer(_)` and always emits `{ i64, i64, ptr }` — the `element_repr` field is ignored entirely. GEP strides come from `element_store_size()` in `arc_emitter/emitter_utils.rs` (line 130), which calls `self.type_info.get(ty).size()`. For `int`, `TypeInfo::Int::size()` returns `Some(8)` unconditionally without consulting `ReprPlan`. Phase C therefore requires the `element_store_size()` integration step described below before narrowed collection elements will produce correct GEP strides.

  - For each `[int]` type in the Pool (Tag::List with inner type Tag::Int), query a global element range from `plan`. The element range must be derived from the join of all per-variable ranges where the variable holds a `[int]` element (this is the same field-summary pattern as structs, but for collection push/assignment sites). See "element range collection" note below.
  - If the element range fits in a narrower type, emit a new `MachineRepr::FatPointer(FatRepr::Collection { element_repr: Box::new(MachineRepr::Int { width: narrow_width, signed: true }) })` decision for the list's Pool `Idx`.
  - **Phase C LLVM integration — `element_store_size` must consult `ReprPlan`**: `TypeLayoutResolver::try_repr_to_llvm_type()` does NOT propagate `element_repr` into GEP strides — it discards the element repr entirely (`FatPointer(_)` always emits `{i64, i64, ptr}`). The GEP stride for collection elements is computed by `element_store_size()` in `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs` (line 130). That function calls `self.type_info.get(ty).size()`, which for `int` always returns `Some(8)` via `TypeInfo::Int`. `ReprPlan` is currently accessible from `TypeLayoutResolver` (field `repr_plan: Option<&ori_repr::ReprPlan>` in `type_info/mod.rs` line 76) but is NOT carried by `ArcIrEmitter`. To make Phase C work: (1) add `repr_plan: Option<&'a ori_repr::ReprPlan>` field to `ArcIrEmitter` in `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`; (2) thread it through the constructor from the outer `FunctionCompiler`; (3) update `element_store_size()` in `emitter_utils.rs` to check `self.repr_plan.and_then(|p| p.get_repr(ty))` first — if it returns `MachineRepr::Int { width, .. }`, use `IntWidth::size_bytes(width)` instead of `TypeInfo::size()`. File: `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs` and `mod.rs`.
  - **Element range collection**: §03's `FieldSummaryTable` covers struct/tuple `Construct` sites. Collection push/assignment sites (list `push`, map `insert`, set `insert`) are tracked via `ArcInstr` variants — these need to be handled in `update_field_summaries()` or a new `CollectionElementSummaryTable` equivalent. For Phase C: a minimal approach is to join all per-variable ranges for variables that are used as arguments to `push`/`set-by-index` instructions, keyed by the list's Pool `Idx`. Full generality is deferred; the conservative fallback is `Top` (no narrowing) for any collection whose element sites are not tracked.
  - **ABI conservatism**: `[int]` parameters and return values must not have their element type narrowed if the function is public or address-taken (same rules as struct fields in public types).

- [x] **Phase A — LLVM struct/tuple lowering (TPR-04-006 fix).** (2026-03-27): Implemented `try_lower_narrowed_aggregate()` in `layout_resolver.rs`. Only triggers for structs with at least one narrowed int field (`IntWidth != I64`). Recursively resolves field reprs via `try_repr_to_llvm_type()`. Non-narrowed structs continue using the named struct path. **Phase A scoping: tuples excluded from narrowing** — tuples are used as collection elements, iterator state, and intermediates where `element_store_size()` assumes canonical widths. Tuple narrowing deferred to Phase C when `element_store_size()` integration is complete. Implementation:
  - [x] `try_lower_narrowed_aggregate()` in `layout_resolver.rs:303-346`: detects narrowed aggregates via `has_narrowed` field scan, resolves all fields recursively, builds anonymous LLVM struct type from narrowed field types. Falls back to `None` for non-narrowed structs and structs with unresolvable nested fields.
  - [x] Fallback: if any field repr returns `None` from `try_repr_to_llvm_type()` (e.g., nested `Struct`/`Enum`), returns `None` → TypeInfoStore two-phase creation path takes over.
  - [x] End-to-end semantic pin: 6 AOT tests in `compiler/ori_llvm/tests/aot/narrowing.rs` — Pixel round-trip (trunc i64→i8 + sext i8→i64), struct update, mixed types (str + int + bool — **runtime fallback only: int field stays canonical i64 due to all-scalar-int guard in `try_lower_narrowed_aggregate()`; mixed-field narrowing deferred to Phase C**), field mutation, i8 boundary values (-128, 127), negative test (wide range stays canonical).
  - [x] Pixel test uses `[-128, 127]` for true i8 pin (signed narrowing).
  - [x] Tuple narrowing disabled in `narrow_struct_fields()` (`narrowing/int.rs`): `CandidateKind::Tuple` → skip with tracing. Tuple narrowing test updated to `tuple_elements_not_narrowed_phase_a`.

- [x] **Insert `sext`/`trunc` at narrowing boundaries** (2026-03-27): Struct field store and load boundaries implemented. Function entry/exit (Phase B) deferred.
  - [x] Struct field store (`construction.rs:29-34`): `trunc_for_narrowed_struct()` in `emitter_utils.rs` — checks pool field type is `Tag::Int` AND LLVM field is narrower, inserts `trunc i64 %val to i<N>`. Naturally narrow types (Byte, Char, Bool) pass through unchanged.
  - [x] Struct field load (`instr_dispatch.rs:216-224`): `sext_narrowed_field()` in `emitter_utils.rs` — checks ARC IR destination type is `Tag::Int`, inserts `sext i<N> %field to i64`. Non-int destinations pass through unchanged.
  - Function entry (Phase B): parameters arrive at canonical width → `trunc` to narrow if locally narrowed — **deferred to Phase B**
  - Function exit (Phase B): narrow local → `sext` to canonical width at boundary — **deferred to Phase B**

- [x] `[IR-PIN-04-018]` **IR semantic pin tests for narrowing** (2026-03-27, from TPR-04-018). Added 4 IR semantic pin tests using `compile_and_capture_ir()` + `extract_function_ir()` in `narrowing.rs`. Tests exposed a critical bug: narrowed decisions were invisible to codegen (index mismatch). Fixed by propagating decisions to resolved Pool indices and adding sext in derive codegen (hash/printable/debug). Implementation:
  - [x] `test_narrowed_struct_ir_pin_type_layout`: Asserts `{ i8, i8, i8, i8 }` type in `_ori_read_pixel` — uses separate function to prevent constant folding.
  - [x] `test_narrowed_struct_ir_pin_trunc_on_construction`: Asserts `trunc i64` or `{ i8, i8, i8 }` constant store in `_ori_main` at construction site.
  - [x] `test_narrowed_struct_ir_pin_sext_on_field_load`: Asserts `sext i8` in `_ori_sum_channels` — narrowed field loads require sign extension to i64.
  - [x] `test_non_narrowed_struct_ir_pin_wide_range`: Negative pin — `_ori_sum_wide` with `3_000_000_000` values asserts NO `sext i8/i16/i32`.

- [x] `[DERIVE-PIN-04-020]` **Negative-value derive semantic pins** (2026-03-27, from TPR-04-020). 4 AOT tests in `narrowing.rs` exercise derived `hash()`, `to_str()`, and `debug()` on narrowed structs with negative i8 field values. Also fixed a pre-existing memory leak in `compile_format_fields()` — intermediate concat results were not RC-decremented (added `emit_str_rc_dec` helper in `string_helpers.rs`).
  - [x] AOT test: `test_narrowed_derive_hash_negative_values` — `#derive(Hashable)` on `SignedPixel { r: -50, g: -120, b: 100 }`, verifies hash consistency with negative values
  - [x] AOT test: `test_narrowed_derive_printable_negative_values` — `#derive(Printable)` verifies `to_str()` contains "-50" and "-120" (catches zext bug: -50 would display as "206")
  - [x] AOT test: `test_narrowed_derive_debug_negative_values` — `#derive(Debug)` verifies `debug()` contains "-1", "-128", "127"
  - [x] IR semantic pin: `test_narrowed_derive_ir_pin_sext_in_hash` — verifies `sext i8` present in IR for narrowed struct hash codegen

- [x] `[MIXED-PIN-04-019]` **Negative semantic pin for mixed-field struct rejection** (2026-03-27, from TPR-04-019). `test_mixed_field_struct_ir_pin_no_narrowing` in `narrowing.rs` — verifies `Record { count: int, name: str, active: bool }` with count in i8 range does NOT show `sext i8` in the `_ori_read_count` function IR, confirming `try_lower_narrowed_aggregate()` rejects mixed-type structs.

- [x] Handle comparison operations correctly (2026-03-27):
  - Signed comparison (`icmp slt`) on narrow types is correct for signed narrowing — verified by architecture: `sext_narrowed_field()` sign-extends to i64 at field extraction before any comparison. 3 AOT semantic pin tests in `narrowing.rs`: `test_narrowed_comparison_signed_semantics` (negative values through all 6 comparison operators), `test_narrowed_comparison_i8_boundary_values` (-128 < 0 catches zext bugs), `test_narrowed_comparison_ordering_chain` (min-of-three with negatives).
  - Unsigned narrowing (future, for byte → int) needs `zext` not `sext` — not yet needed, byte values use separate `Tag::Byte` type

---

## 04.5 Completion Checklist

**Test matrix for §04 (write failing tests FIRST, verify they fail, then implement):**

**Phase A — Struct field narrowing:**

| Input pattern | Expected narrowing | Semantic pin |
|---|---|---|
| `struct Pixel { r: int, g: int, b: int, a: int }` with fields `0..255` | `{ i8, i8, i8, i8 }` (4 bytes) | Yes — `sizeof(Pixel) == 4` |
| `struct Pair { x: int, y: int }` with fields `-32768..32767` | `{ i16, i16 }` (4 bytes) | Yes — `sizeof(Pair) == 4` |
| Struct field store: canonical-width operand into narrowed field | `trunc i64 %val to i8` in LLVM IR | Yes — no `trunc` → wrong width stored |
| Struct field load: narrowed field used in computation | `sext i8 %field to i64` in LLVM IR | Yes — missing `sext` → sign extension wrong |

**Phase B — Local variable narrowing:**

| Input pattern | Expected narrowing | Semantic pin |
|---|---|---|
| `for i in 0..100` — loop counter | `i8` local in LLVM IR | Yes — zero `i64` variable for `i` |
| `let x = 200` — single-use constant local | `i16` (range `[200,200]` does not fit signed i8 max=127, so → i16) | Yes — `i64` alloca absent for `x` |
| Internal function `@f (n: int) -> int` where only call site passes `5` | parameter `n` uses `i8` | Yes — `sext` visible at call boundary |
| `pub @f (n: int) -> int` — public API | parameter `n` stays `i64` | Yes — no narrowing at public boundary |
| `let g = f; g(300)` / function passed as value | parameter stays `i64` | Yes — address-taken callables disabled |
| Narrowed local captured by closure | capture storage stays canonical `i64` | Yes — no closure ABI mismatch |
| Arithmetic `a + b` where `a, b ∈ [0, 100]` → result `[0, 200]` | `i16` or wider for result | Yes — overflow safety preserved |
| Local `int` with range `Top` (no analysis) | `i64` (canonical, no narrowing) | Yes — fallback is safe |
| Trait method parameter | `i64` (no narrowing — unknown callers) | Yes — no narrowing |
| Cross-module call with agreed-upon range | narrow type if both sides agree | Yes — `sext` at module boundary |
| Function entry: parameter arrives canonical, is narrowed locally | `trunc i64 %arg to i8` at function entry in LLVM IR | Yes — parameter width unchanged at call site |
| Function exit: narrow local returned to canonical | `sext i8 %local to i64` at return in LLVM IR | Yes — return type unchanged at call site |

**Phase C — Collection element narrowing:**

| Input pattern | Expected narrowing | Semantic pin |
|---|---|---|
| `[int]` list where all pushed values are `[-128, 127]` | element stored as `i8` in backing array | Yes — element GEP stride is 1 byte, not 8 |
| `[int]` list where element range is `Top` | element stays `i64` | Yes — no element narrowing without evidence |
| Public `[int]` parameter | element stays `i64` — ABI conservative | Yes — no narrowing of public collection elements |

**All phases — negative cases (things that must NOT be narrowed):**

| Input pattern | Expected | Semantic pin |
|---|---|---|
| `NarrowingPolicy::Disabled` (i.e., `--no-repr-opt`) | all types stay `i64` | Yes — Pixel struct is 32 bytes, not 4 |
| `NarrowingPolicy::Conservative` vs `Aggressive` | Conservative does not narrow loop counters (insufficient savings evidence); Aggressive does | Yes — behavior differs |

**TDD ordering (MANDATORY — write failing tests first for each phase):**
- [x] Write failing test matrix for Phase A BEFORE implementing Phase A (2026-03-26): 22 tests in `narrowing/tests.rs` — verified tests failed before implementation (iteration bug: pool-based → plan-based fixed)
- [x] Write failing test matrix for Phase B BEFORE implementing Phase B (2026-03-27): IR-inspection tests `test_phase_b_ir_pin_straight_line_add_narrowed` and `test_phase_b_ir_pin_multiple_narrowed_locals` in `narrowing.rs` — verified tests failed before implementation. Negative tests: `test_phase_b_negative_public_param_not_narrowed`, `test_phase_b_negative_wide_constant_stays_i64`. Loop counter IR pin tests remain `#[ignore]` (blocked on §03 convergence).
- [ ] Write failing test matrix for Phase C BEFORE implementing Phase C
- [ ] Verify each test fails before implementing the corresponding feature — if a test passes before implementation, it does not cover the target behavior

**Phase A — Struct field narrowing:**
- [x] `ValueRange::min_width()` returns correct width for all test ranges (2026-03-26): Verified via `semantic_pin_pixel_signed_range_narrows_to_i8` (I8), `boundary_exact_i16_range_narrows_to_i16` (I16), `boundary_exact_i32_range` (I32), `top_range_stays_i64` (I64), `bottom_range_narrows_to_i8` (I8).
- [x] `ValueRange::min_width()` boundary cases (2026-03-26): `boundary_just_exceeds_i8_narrows_to_i16` ([-128,128]→I16), `boundary_just_exceeds_i16_narrows_to_i32` ([-32769,0]→I32), `boundary_just_exceeds_i32_stays_i64` ([-2^31,2^31]→I64), `boundary_unsigned_byte_range_narrows_to_i16` ([0,255]→I16).
- [x] Struct field `x: int` in `struct Pair { x: int, y: int }` uses narrowed type (2026-03-26): `mixed_fields_partial_narrowing` test verifies bounded fields narrow while Top fields stay I64.
- [x] Struct field store inserts `trunc i64 %val to i<N>` when storing canonical-width operand into narrowed field (2026-03-27): `trunc_for_narrowed_struct()` in `emitter_utils.rs`. Uses pool field type check (`Tag::Int` + LLVM field width < 64). 6 AOT tests verify correct round-trip behavior.
- [x] Struct field load inserts `sext i<N> %field to i64` when loading narrowed field for computation (2026-03-27): `sext_narrowed_field()` in `emitter_utils.rs`. Uses ARC IR destination type check (`Tag::Int`). Tests verify extracted values are correct after sext.
- [x] Semantic pin: `struct Pixel { r: int, g: int, b: int, a: int }` with `[-128, 127]` fields (2026-03-27): End-to-end AOT test `test_narrowed_struct_pixel_round_trip` in `narrowing.rs`. Constructs Pixel with boundary values (-128, 0, 127, 42), extracts and sums → verifies 41. Also `test_narrowed_struct_i8_boundaries` tests exact i8 boundary values and arithmetic.
- [x] §04/§06 interface (2026-03-26): `field_offset_stays_zero_after_narrowing` test verifies offsets remain zero. `narrow_struct_fields()` only writes `FieldRepr.repr`; `FieldRepr.offset`, `StructRepr.size`, and `StructRepr.align` are untouched.

**Phase B — Local variable narrowing:**
- [x] Loop counters in `for i in 0..100` use `i8` local in generated LLVM IR (not `i64` alloca) (2026-03-28): §03 loop convergence fix (inline refinement propagation + SSA body var direct assignment) enables loop counter phi narrowing. `test_phase_b_ir_pin_loop_counter_phi` verifies `phi i8` in `_ori_sum_loop` IR.
- [x] Public function parameters are NOT narrowed (2026-03-27): `compute_narrowed_vars()` excludes all function parameters via `param_vars` set. AOT negative pin: `test_phase_b_negative_public_param_not_narrowed`.
- [x] Trait method parameters are NOT narrowed (unknown callers) (2026-03-27): same exclusion mechanism — all parameters excluded regardless of visibility. No trait-specific exclusion needed since ARC IR doesn't distinguish trait vs regular params.
- [x] Address-taken / indirectly-called functions are NOT narrowed at their callable boundary (2026-03-27): `ori_arc::graph::call_graph` already excludes `ApplyIndirect` from the call graph. §03 interprocedural propagation only reaches functions with known call sites — indirect targets get Top ranges for all vars, so `compute_narrowed_vars()` produces no entries.
- [x] ABI boundary widening (2026-03-27): Parameters are excluded from narrowing entirely (canonical i64 at entry). Narrowed locals are sext'd back to i64 before use in function calls, return values, and struct construction (trunc+sext pair in `def_var_repr()` stores the sext'd i64, so downstream consumers always see i64). Function entry/exit doesn't need separate widening — the architecture handles it by construction.
- [x] Closure-captured ints stay canonical-width (2026-03-27): Closure capture goes through `PartialApply` which copies the captured value. Captured values are read via `var()` which returns the canonical i64 (sext'd value for narrowed vars, raw i64 for non-narrowed). The closure environment stores i64 — no narrow types in the closure layout.
- [x] Semantic pin for Phase B: a loop counter `i` in `for i in 0..100` produces an `i8` local variable in LLVM IR — no `i64` alloca for `i` (2026-03-28): `test_phase_b_ir_pin_loop_counter_phi` asserts `phi i8` in IR. `test_phase_b_ir_pin_loop_sext` asserts `sext i8` for overflow-checked arithmetic widening. Both un-ignored after §03 loop convergence fix.
- [x] Straight-line local narrowing (2026-03-27, from TPR-04-023): Implemented `narrow_local_if_needed()` in `emitter_utils.rs`. Inserts trunc+sext pair at variable definition via `def_var_repr()`. Design: trunc i64→i<N> then sext i<N>→i64 (validates range + informs LLVM). Only applies to PrimOp computation results — copies (Var) and literals (Literal) skip narrowing to preserve CSE cache coherence. Consistent with phi path (which also stores sext'd i64 values). 14,328 tests pass.
- [x] IR semantic pin for straight-line local (2026-03-27, from TPR-04-023): `test_phase_b_ir_pin_straight_line_add_narrowed` — verifies `local.trunc` + `local.sext` appear in IR for arithmetic results. `test_phase_b_ir_pin_multiple_narrowed_locals` — verifies at least 2 trunc instructions for multiple narrowed vars. Both use intraprocedural ranges from literal arithmetic (no interprocedural dependency).
- [x] **Select instruction narrowing** (2026-03-28, from TPR-04-024): Two fixes required. (1) Route `ArcInstr::Select` through `def_var_repr()` in `instr_dispatch.rs:463` (was `def_var()`). (2) Add `derive_local_range()` in `emitter_utils.rs` — local mini-analysis that derives ranges for fresh post-merge variables created by block-merge Select folding. Block-merge creates fresh variable IDs (`func.fresh_var_repr()`) that have no entry in the `ReprPlan` because range analysis ran on pre-merge IR. `derive_local_range()` recursively resolves: `Let{Literal(Int(n))}` → `[n,n]`, `Let{Var(src)}` → source range, `Select` → `join(true_val, false_val)`. 3 new tests: IR pin + behavior + negative values. 14,336 tests pass (debug + release).
- [x] IR semantic pin for Select narrowing (2026-03-28, from TPR-04-024): `test_phase_b_ir_pin_select_narrowed` — compile `@pick (b: bool) -> int = if b then 1 else 2` where range analysis proves `[1, 2]` fits in i8. Asserts `local.trunc` + `local.sext` in `_ori_pick` IR. Before fix: `%sel = select i1 %0, i64 1, i64 2; ret i64 %sel` (no narrowing). After fix: `trunc i64 %sel to i8; sext i8 to i64`. Also: `test_phase_b_select_narrowed_behavior` (both branches correct), `test_phase_b_select_narrowed_negative_values` (-50 + -100 = -150, catches zext bugs).

**Phase B — Overflow guard insertion:**
- [x] `can_overflow(BinaryOp::Add, lhs, rhs, target)` returns `true` when result range exceeds target width (2026-03-27): `add_overflows_i8`, `add_fits_in_i16_not_i8` tests
- [x] `can_overflow(BinaryOp::Sub, lhs, rhs, target)` correctly detects subtract overflow (2026-03-27): `sub_overflows_i8`, `sub_no_overflow_in_i8` tests
- [x] `can_overflow(BinaryOp::Mul, lhs, rhs, target)` correctly detects multiply overflow (2026-03-27): `mul_overflows_i8`, `mul_no_overflow_in_i8` tests
- [x] For non-arithmetic ops (`BinaryOp::Eq`, etc.), `can_overflow()` conservatively returns `true` (uses `ValueRange::Top`) (2026-03-27): `comparison_op_conservative_overflow` test
- [x] Overflow guards correct by construction (2026-03-28): Arithmetic always operates on sext'd i64 values (strategy (a)), and `min_width()` selects the smallest type that fits the computed range (implicit strategy (c)). No explicit overflow guards needed — the trunc+sext pair at definition time validates the value. Verified: `x=100, y=x+50` narrows `y` to i16 (not i8, since 150 > 127). Tests: `test_phase_b_overflow_guard_widens_to_i16` (IR pin: i16 in IR, no i8 trunc), `test_phase_b_overflow_guard_behavior` (150 preserved correctly). The existing `llvm.sadd.with.overflow.i64` catches any i64-level overflow.

**NarrowingPolicy behavior:**
- [x] `NarrowingPolicy::Disabled` (via `--no-repr-opt` / `ORI_NO_REPR_OPT`) suppresses ALL narrowing (2026-03-28): 3 E2E AOT tests: `test_narrowing_policy_disabled_suppresses_struct_narrowing` (no sext in Pixel IR), `test_narrowing_policy_disabled_suppresses_local_narrowing` (no local.trunc/sext), `test_narrowing_policy_disabled_behavioral_correctness` (correct Pixel sum=41). Disabled returns after `populate_canonical()`.
- [x] `--no-repr-opt` flag passes `NarrowingPolicy::Disabled` to `ReprPlan` (2026-03-28): Already implemented in §01. CLI: `parse_args.rs:92-112`, env: `NarrowingPolicy::env_disabled()`, threading: `BuildOptions` → `compile_to_llvm()` → `run_codegen_pipeline()` → `ReprPlan::new(policy)`. Unit tests in `build_options/tests.rs`.
- [x] `NarrowingPolicy::Conservative` vs `Aggressive` (2026-03-28): Currently equivalent — both declared and parseable (`--repr-opt=aggressive|conservative`) but produce identical narrowing. Only `Disabled` has special handling. Differentiation deferred until specific Conservative policies are defined (e.g., "don't narrow loop counters" or "require 100% call-site coverage").

**Phase C — Collection element narrowing:**
- [ ] `[int]` list whose push sites all pass `[-128, 127]` values → `FatRepr::Collection { element_repr: MachineRepr::Int { width: I8, signed: true } }` in `ReprPlan`
- [ ] `[int]` list with untracked push sites → element stays `i64` (conservative `Top`)
- [ ] Public `[int]` parameter — element type not narrowed even if all internal construction sites show bounded values
- [ ] `element_store_size()` in `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs` consults `ReprPlan` for narrowed int element types before falling back to `TypeInfo::size()` — add unit test that narrowed int pool idx produces byte size 1/2/4 from `element_store_size`
- [ ] Element GEP stride in LLVM IR uses narrowed element size (1 byte for `i8`, 2 bytes for `i16`, 4 bytes for `i32`)
- [ ] Semantic pin: a list built only with push values `0..255` uses `i8` element storage in LLVM IR — this test can ONLY pass with collection element narrowing enabled
- [ ] `ArcIrEmitter` carries `repr_plan: Option<&'a ori_repr::ReprPlan>` field (see Phase C LLVM integration in §04.4)

**All phases — final validation:**
- [ ] No semantic change: `./diagnostics/dual-exec-verify.sh` passes (eval and AOT produce identical results for all narrowed programs)
- [ ] `./test-all.sh` green in both debug (`cargo b`) and release (`cargo b --release`) builds — debug and release are tested separately because FastISel differs from release codegen
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean (no memory errors from narrowed GEP strides)
- [ ] Performance: struct sizes measurably smaller for bounded-range fields (verify via LLVM IR inspection or binary size comparison)

**Exit Criteria:** Compiling a program with `struct Pixel { r: int, g: int, b: int, a: int }` where all fields are `[-128, 127]` produces a 4-byte struct (4 × i8) instead of 32-byte struct (4 × i64), verified by checking LLVM IR struct definitions. (Under signed narrowing, `0..255` maps to `i16`, producing an 8-byte struct — use `[-128, 127]` for the `i8` pin.)

---

## 04.X Cross-Section Findings

- [x] `[HYGIENE-04-002][minor/bloat]` `compiler/oric/src/commands/codegen_pipeline.rs:501` — **File exceeds the 500-line limit (501 lines).** (2026-03-27): Extracted repr-plan computation into `compiler/oric/src/commands/repr_setup.rs` (70 lines). Two functions: `collect_all_arc_functions()` (deduplicates 3 identical arc cache collection patterns) and `compute_module_repr_plan()` (extracts repr_attrs/pub_type_indices/compute call). `codegen_pipeline.rs` reduced from 501 → 469 lines. All 14,281 tests pass.

- [x] `[HYGIENE-04-001][minor/bloat]` `compiler/ori_llvm/src/codegen/type_info/mod.rs:518` — **File exceeds the 500-line limit (518 lines, excluding tests).** (2026-03-27): Split `TypeLayoutResolver` to `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs` (449 lines). `mod.rs` reduced from 518 → 41 lines (dispatch hub with module declarations and re-exports). Test imports updated (`Pool`, `Idx`, `Name`, `SimpleCx`, `BasicTypeEnum`). All 14,281 tests pass.

- [x] `[CROSS-04-014][high]` `compiler/ori_types/src/pool/descriptor.rs` `compiler/ori_types/src/output/mod.rs` `compiler/oric/src/typeck.rs` — **Imported types lose repr/pub metadata across module boundaries.** (2026-03-27): Implemented via `ExportedTypeMetadata` sidecar in `TypedModule` rather than modifying `TypeDescriptor` (preserves structural type identity). Changes:
  - [x] Added `ExportedTypeMetadata { merkle_hash, repr, is_public }` struct to `ori_types/src/output/mod.rs`
  - [x] Added `exported_type_metadata: Vec<ExportedTypeMetadata>` field to `TypedModule`, populated from `TypeEntry` data during `check_module_impl()`
  - [x] Extended `compute_repr_plan_with_interner()` with `imported_type_metadata` parameter — Phase 0a-import maps merkle hashes to local pool Idx and seeds repr_attrs/pub_type_indices; combined with local metadata for Phase 0c propagation
  - [x] Threaded through AOT test runner (`llvm_backend.rs`): collects metadata from `imported_type_results` before compilation
  - [x] Threaded through JIT evaluator (`compile.rs`): new parameter on `compile_module_with_tests` and `compile_all_functions`
  - [x] 6 semantic pin tests in `ori_repr/src/tests.rs`: imported pub seeding, imported repr("c") seeding, pub not narrowed, repr("c") not narrowed, negative (no metadata = narrowing proceeds), edge case (hash not in pool = no panic)
  - [x] 14,293 tests pass, clippy clean

- [x] `[CROSS-05-001][major]` `section-05-float-narrowing.md:79,83,84` — **`ArithOp` type does not exist in the codebase.** (2026-03-27): Updated `section-05-float-narrowing.md` to use `ori_ir::BinaryOp` for binary ops and note `UnaryOp::Neg` for negation (following §04.3 pattern). Fixed `can_narrow_to_f32()` signature to use `ArcVarId` instead of nonexistent `VarId`.

- [ ] `[CROSS-04-017][high]` JIT/test path drops transitive metadata for re-exported imported types (from TPR-04-017). The `generate_exported_type_metadata()` in `ori_types/src/check/mod.rs:973` only includes locally-declared types. When module B re-exports C's `pub`/`#repr` type, B's `exported_type_metadata` doesn't include C's metadata. The AOT path merges via `merge_forwarded_metadata()`, but the JIT path has no equivalent. **Root cause**: `TypedModule.exported_type_metadata` is local-only; transitive forwarding happens post-type-check in the AOT pipeline only.
  - [ ] **Option A (recommended — type-checker level)**: Extend `generate_exported_type_metadata()` to accept `imported_metadata: &[ExportedTypeMetadata]` and merge forwarded entries (dedup by merkle_hash, local priority). Thread through `check_module_impl()`. All consumers (JIT, AOT, future) get complete metadata without post-hoc merging. Removes the need for `merge_forwarded_metadata()` in the AOT path.
  - [ ] **Option B (JIT-side merge)**: Add a JIT-side equivalent of `merge_forwarded_metadata()` in `llvm_backend.rs` before passing metadata to `compile_module_with_tests()`. Simpler but duplicates merge logic and only fixes JIT path — does not fix the root cause in `TypedModule`.
  - [ ] Regression test: `A -> B -> C` where C defines a `pub` type with bounded-range int fields, B re-exports it in a public signature, A imports only B — verify A's repr plan does NOT narrow C's protected type on both JIT and AOT paths.

- [x] `[CROSS-04-015][high]` Thread `ExportedTypeMetadata` through multi-file AOT pipeline (from TPR-04-015). (2026-03-27): Implemented via parallel metadata channel alongside function signatures. 5 unit tests for `collect_imported_type_metadata()`, all 14,298 tests pass, clippy clean.
  - [x] Add `exported_type_metadata: Vec<ExportedTypeMetadata>` field to `CompiledModuleInfo` in `compiler/oric/src/commands/build/multi.rs` — populated from `type_result.typed.exported_type_metadata` after type checking each module
  - [x] Add `collect_imported_type_metadata()` function in `multi.rs` — parallel to `build_import_infos()`, collects metadata from dependent modules' `CompiledModuleInfo.exported_type_metadata` via dependency graph traversal
  - [x] Update `compile_to_llvm_with_imports()` in `compile_common.rs` — new `imported_type_metadata: &[ExportedTypeMetadata]` parameter, forwarded to `run_codegen_pipeline()`
  - [x] Update `run_codegen_pipeline()` in `codegen_pipeline.rs` — new `imported_type_metadata: &[ExportedTypeMetadata]` parameter, passed to `compute_module_repr_plan()` instead of `&[]`
  - [x] `compile_to_llvm()` (single-file path) passes `&[]` for metadata (no imports, correct)
  - [x] 5 unit tests in `compiler/oric/src/commands/build/tests.rs`: single dependency, multiple dependencies, no imports, missing module, empty types
  - [x] End-to-end multi-file semantic pins blocked: multi-file AOT codegen is incomplete (ARC IR emitter cannot resolve cross-module function calls — roadmap Section 4: Modules). Plumbing verified via unit tests + existing `ori_repr` imported-metadata tests (CROSS-04-014)

---

## 04.R Third Party Review Findings

- [ ] `[TPR-04-025][low]` `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs:1` — `emit_function.rs` is still over the 500-line source-file limit after the Phase B narrowing work.
  Evidence: fresh review on 2026-03-28 measured `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs` at 501 lines, and this file is part of the current `HEAD~5..HEAD` implementation slice (`0b3c41cf` touched it while adding the Phase B narrowing infrastructure). `CLAUDE.md` and `.claude/rules/impl-hygiene.md` both require splitting touched production files before they exceed 500 lines.
  Impact: §04.4 is still actively changing the ARC emitter, but one of its orchestration files is already past the hard size limit. Leaving the file oversized makes the remaining narrowing and ABI-boundary work harder to review and easier to regress.
  Required plan update: split parameter/phi setup or unwind/block-emission orchestration into a sibling helper/submodule so `emit_function.rs` drops back under the 500-line limit during the next §04.4 implementation pass.

- [x] `[TPR-04-024][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs:452` `compiler/ori_arc/src/block_merge/select.rs:260` `compiler/ori_repr/src/range/transfer/mod.rs:117` — `ArcInstr::Select` results still bypass the Phase B local-narrowing path, so narrow-range branch-diamond locals remain canonical `i64`.
  Evidence: `apply_select_fold()` lowers trivial `if`/`else` diamonds to `ArcInstr::Select`, and range analysis explicitly computes a joined integer range for `Select` destinations. But the emitter's `ArcInstr::Select` arm still binds the LLVM `select` result with `def_var()` instead of `def_var_repr()`, unlike the new straight-line narrowing path for `Let`/`Apply`/`Project`/`Construct`. Fresh verification on 2026-03-28 with `diagnostics/ir-dump.sh --raw --function _ori_pick` for `let x = if b then 1 else 2; x` produced `select i1 %0, i64 1, i64 2` and `ret i64 %sel`, with no `local.trunc`/`local.sext` or narrow integer type anywhere in `_ori_pick`.
  Impact: §04.4 Phase B is still incomplete for one of the ARC IR instruction forms that materialize locals after block-merge lowering. Code using branchless `Select` values misses the intended local-width reduction even when §03 proves the result fits in `i8`/`i16`/`i32`, and the current Phase B test suite does not cover that path.
  Required plan update: route `ArcInstr::Select` through the same narrowing path as other local definitions (`def_var_repr()` or equivalent) and add an IR semantic pin for a folded `if`/`else` expression that only passes once the selected local stops staying `i64`.
  Resolved: Accepted on 2026-03-28. Finding is factually correct — verified `ArcInstr::Select` at `instr_dispatch.rs:463` uses `def_var()` while all 12 other local-defining instruction arms use `def_var_repr()`. Implementation tasks added to §04.4 Phase B: Select instruction narrowing + IR semantic pin.

- [x] `[TPR-04-023][medium]` `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs:321` `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs:96` `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs:157` — The new Phase B implementation only narrows phi/block-parameter storage; ordinary local definitions still stay canonical `i64`.
  Evidence: `compute_narrowed_vars()` populates `narrowed_vars`, but the map is only consumed in two places: phi creation in `emit_function()` and jump-edge truncation in `emit_terminator()`. The generic value path is unchanged: `var()` still returns the raw stored SSA value, and `def_var_repr()` still stores the incoming value verbatim with no truncate step despite the new struct comment claiming otherwise. Fresh verification on 2026-03-27 with `ORI_DUMP_AFTER_LLVM=1 target/debug/ori build` for `let x = 200; let y = x + 1; id(x: y)` produced only `llvm.sadd.with.overflow.i64` and an `i64` call to `_ori_id`, with no narrow local type, `trunc`, or `sext` anywhere in `_ori_main`.
  Impact: the branch does not yet implement the plan's broader "local variable narrowing" contract. Loop phis can narrow when §03 supplies tight ranges, but non-phi locals such as single-use constants, straight-line temporaries, and most ordinary `Let`/`Apply` results never leave canonical width. That leaves §04.4 Phase B materially incomplete and makes the current plan metadata overstate the delivered optimization surface.
  Required plan update: either scope §04.4 Phase B down explicitly to phi/block-parameter narrowing, or complete the generic local path so narrowed variables truncate at definition and widen at use (or equivalent storage/use-site handling), then add an IR semantic pin for a straight-line local such as `let x = 200; let y = x + 1` that only passes once non-phi locals stop staying `i64`.
  Resolved: Accepted on 2026-03-27. Finding is factually correct — non-phi locals are not yet narrowed. Added two concrete Phase B tasks in §04.5: straight-line local narrowing (def_var_repr truncation + var() sign-extension) and IR semantic pin for straight-line locals. Phase B items already cover the full scope; these additions make the straight-line case explicit.

- [x] `[TPR-04-022][low]` `compiler/ori_llvm/tests/aot/derives.rs:20` `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs:143` `compiler/ori_llvm/tests/aot/derives.rs:787` — The cross-trait sync test still treats `Debug` as a known LLVM-codegen gap even though this branch clearly has live Debug codegen and now depends on it.
  Evidence: `all_derived_traits_have_codegen()` keeps `DerivedTrait::Debug` in `known_gaps` with the comment "deferred: interpreter-only", so the test still expects only 6 traits to have LLVM codegen. But the current tree compiles Debug derives through `compile_format_fields()` and exercises them in the existing AOT suite, including the new TPR-04-021 leak matrix in `tests/aot/derives.rs`. Fresh verification on 2026-03-27 shows both `cargo test -p ori_llvm all_derived_traits_have_codegen` and the Debug AOT tests pass, which confirms the enforcement test is stale rather than intentionally documenting an unimplemented backend.
  Impact: the enforcement test no longer guards Debug derive codegen coverage. Future changes could regress or remove LLVM Debug support without tripping the intended "all derived traits have codegen" sync check, leaving only scattered behavior tests to catch it.
  Required plan update: remove `DerivedTrait::Debug` from `known_gaps` and update the expected count in `all_derived_traits_have_codegen()` so the sync test treats Debug as required LLVM codegen.
  Resolved: Fixed on 2026-03-27. Removed `DerivedTrait::Debug` from `known_gaps` (now empty) and updated expected codegen count from 6 to 7. `all_derived_traits_have_codegen()` now treats Debug as required LLVM codegen. Test passes.

- [x] `[TPR-04-021][high]` `compiler/ori_llvm/src/codegen/derive_codegen/string_helpers.rs:127` `compiler/ori_llvm/tests/aot/narrowing.rs:339` `plans/repr-opt/section-04-integer-narrowing.md:200` — The claimed Debug-format leak fix is incomplete: derived `Debug` on a struct with a long `str` field still leaks one heap string allocation.
  Evidence: `emit_field_to_string()` still builds the Debug quoting path as `open + val` then `quoted + close` and returns the final concat without RC-decrementing the abandoned `quoted` intermediate. `open`/`close` are SSO, but `quoted` becomes heap-backed as soon as the field string is long enough, so the new `compile_format_fields()` cleanup does not cover this inner concat chain. Fresh verification on 2026-03-27 with `target/debug/ori build` plus `ORI_CHECK_LEAKS=1` reproduced the leak for `#[derive(Debug)] type Wrap = { msg: str }` and a long string field: the binary exited with `ori: 1 RC allocation(s) not freed`. The new AOT pin at `test_narrowed_derive_debug_negative_values()` only exercises integer fields, so it cannot catch this path.
  Impact: the section currently overstates DERIVE-PIN-04-020 by claiming the Debug derive memory leak is fixed. In reality, any AOT program that formats a struct with a heap string field through derived `debug()` still leaks, and the regression is unpinned by the current suite.
  Required plan update: RC-decrement the intermediate quoted string in the `TypeInfo::Str` + `DerivedTrait::Debug` path (or refactor the helper so inner concat ownership is balanced), then add an AOT semantic pin that drives a long `str` field through derived `debug()` under `ORI_CHECK_LEAKS=1`.
  Resolved: Fixed on 2026-03-27. Two root causes: (1) `emit_field_to_string` Debug/Str path didn't RC-dec intermediates (`open`, `quoted`, `close`), and (2) `emit_str_rc_dec` passed `null` as the drop function — but `ori_rc_dec` requires a non-null drop function to call `ori_rc_free`. Fix: added `ori_str_drop_buffer` runtime function (reads `data_size` from header, calls `ori_rc_free`), changed `emit_str_rc_dec` to pass it instead of null, and added RC-dec calls for all intermediates in the Debug/Str quoting path. 4 matrix tests: long str (heap), short str (SSO), multi-str, mixed str+int. All 14,317 tests pass.

- [x] `[TPR-04-020][medium]` `plans/repr-opt/section-04-integer-narrowing.md:154` `compiler/ori_llvm/tests/aot/derives.rs:174` `compiler/ori_llvm/tests/aot/ir_quality_attributes.rs:318` — The new derive-codegen widening fix for narrowed ints is still unpinned for the signed cases that actually require `sext`.
  Evidence: §04.4 now claims the phase fixed derive codegen for `hash`, `printable`, and `debug` by widening narrowed fields back to canonical `i64`, but the AOT coverage never exercises a negative narrowed value through those paths. The added derive behavior tests only use positive field values (`1`, `2`, `3`, `4`) in `hash()` / `to_str()`, and the lone `debug()` AOT test checks only LLVM attributes (`nounwind`), not formatted output. A mistaken `zext` or missing widen would therefore still pass the current suite for all covered cases because the bug is observable only once an `i8`/`i16` field carries a negative value.
  Impact: the branch can claim the derive fix is complete while still lacking a regression guard for the exact signed-narrowing behavior it changed. Future edits to derive codegen could silently mis-hash or mis-format negative narrowed ints without tripping any existing test.
  Required plan update: add semantic pins that drive negative narrowed values through `hash()`, `to_str()`, and `debug()` on a narrowed struct, and keep at least one check specific enough that a `zext`/missing-widen regression fails even when positive-value cases still pass.
  Resolved: Validated and accepted on 2026-03-27. Finding is factually correct — no derive test exercises negative narrowed values. Implementation tasks added as DERIVE-PIN-04-020 in §04.4.

- [x] `[TPR-04-019][medium]` `plans/repr-opt/section-04-integer-narrowing.md:184` `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs:340` `compiler/ori_llvm/tests/aot/narrowing.rs:46` — §04.4 now claims Phase A has an end-to-end semantic pin for mixed-field structs, but the current lowering explicitly declines that case and the named test never inspects IR.
  Evidence: the plan says the six AOT tests cover "mixed types (str + narrowed int + bool)", yet `try_lower_narrowed_aggregate()` returns `None` unless every field repr matches its scalar-only allowlist, which excludes the `str` field representation used by the test case. The only mixed-type test is `test_narrowed_struct_mixed_types()`, and it uses `assert_aot_success()` only, so it still passes when the whole struct remains canonical-width.
  Impact: Section 04 currently overstates Phase A coverage. Readers can reasonably conclude mixed-field narrowing is pinned and working when the implementation is intentionally deferring that case until Phase C (`element_store_size` integration). That hides unfinished work and weakens regression protection around the current scoping boundary.
  Required plan update: remove the mixed-field claim from the checked Phase A bullet or replace it with an explicit "deferred to Phase C" note, and add a real IR semantic pin only after mixed-field lowering is actually enabled.
  Resolved: Validated and accepted on 2026-03-27. Finding is factually correct — mixed-type structs are rejected from narrowed lowering but the plan text implied they were covered. Fixed Plan A claim to note "runtime fallback only, deferred to Phase C". Implementation task added as MIXED-PIN-04-019 in §04.4 for negative IR semantic pin.

- [x] `[TPR-04-017][high]` `compiler/oric/src/test/runner/llvm_backend.rs:252` `compiler/ori_types/src/check/mod.rs:923` `compiler/oric/src/commands/build/multi.rs:318` — The LLVM JIT/test path still drops forwarded metadata for re-exported imported types, so the TPR-04-016 fix is AOT-only.
  Resolved: Validated on 2026-03-27. Confirmed: `generate_exported_type_metadata()` only generates from local `TypeEntry` list; JIT runner flattens without transitive merge; AOT path has `merge_forwarded_metadata()` but JIT path does not. Accepted — implementation tasks added as CROSS-04-017 in §04.X.

- [x] `[TPR-04-018][medium]` `compiler/ori_llvm/tests/aot/narrowing.rs:12` `plans/repr-opt/section-04-integer-narrowing.md:255` — The new §04.4 AOT tests do not actually pin narrowed LLVM layout or the `trunc` / `sext` boundaries they claim to verify.
  Resolved: Validated and accepted on 2026-03-27. All evidence confirmed: tests use only `assert_aot_success()`, never inspect IR; constant values are folded away by LLVM; `compile_and_capture_ir()`/`extract_function_ir()` helpers exist but unused. Implementation tasks added as IR-PIN-04-018 in §04.4.

- [x] `[TPR-04-016][high]` `compiler/ori_types/src/check/mod.rs:917` `compiler/oric/src/commands/build/multi.rs:318` `compiler/oric/src/test/runner/llvm_backend.rs:244` — Re-exported imported `pub` / `#repr(...)` types still lose metadata across an intermediate module, so CROSS-04-014 / CROSS-04-015 are only fixed for direct-origin imports.
  Evidence: `TypedModule.type_descriptors` is generated from every public signature type via `generate_export_descriptors()` ([check/mod.rs:917](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:917), [check/mod.rs:947](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:947)), so a module can export descriptors for foreign types that appear in its public API. But `TypedModule.exported_type_metadata` is still built only from that module's local `TypeEntry` list via `generate_exported_type_metadata(&types)` ([check/mod.rs:923](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:923), [check/mod.rs:973](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:973)). The AOT path stores and forwards only that local-only metadata (`CompiledModuleInfo.exported_type_metadata = type_result.typed.exported_type_metadata.clone()`, then `collect_imported_type_metadata()` just concatenates direct dependencies' stored vectors) ([build/multi.rs:318](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:318), [build/multi.rs:428](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:428)). The JIT test runner does the same direct flatten over `typed.exported_type_metadata` ([llvm_backend.rs:244](/home/eric/projects/ori_lang/compiler/oric/src/test/runner/llvm_backend.rs:244)). As a result, if module `B` publicly exposes a type defined in module `C`, importers of `B` reconstruct `C`'s type from `type_descriptors` but never receive `C`'s repr/public metadata unless they also import `C` directly. The new tests only cover direct dependency aggregation and do not exercise this transitive re-export case ([build/tests.rs:32](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/tests.rs:32)).
  Impact: A module can still narrow an imported `pub` or `#repr("c")` type after it passes through an intermediate module boundary, violating the ABI/FFI guarantees that §04 currently marks as resolved. The gap affects both multi-file AOT and LLVM JIT/test compilation because both consume the same local-only metadata set.
  Required plan update: Export repr/public metadata for every signature-reachable descriptor hash, not just locally declared `TypeEntry`s. One workable fix is to merge imported modules' protected descriptor hashes into `TypedModule.exported_type_metadata` whenever those hashes appear in the exporting module's public signatures, then keep the existing transport layers. Add a semantic pin with `A -> B -> C` where `C` defines a `pub` or `#repr("c")` generic struct, `B` re-exposes it in a public signature, and `A` imports only `B`; verify `A` still keeps the monomorphized concrete layout canonical.
  Resolved: Fixed on 2026-03-27. Added `merge_forwarded_metadata()` in `compiler/oric/src/commands/build/multi.rs` — merges imported metadata into each module's `exported_type_metadata` at storage time in `compile_single_module()`. Deduplicates by `merkle_hash` (local entries take priority). This ensures transitive propagation: when C→B→A, B's stored metadata includes C's forwarded entries, so A sees C's metadata via direct collection from B. 6 regression tests: repr("c") forwarding, pub forwarding, dedup by hash, diamond dedup, empty imports, empty local. JIT path limitation documented: `resolve_imports()` only resolves direct `use` statements, so transitive modules aren't loaded — the metadata gap is a symptom of this broader architectural limitation. AOT production path fully fixed. 14,304 tests pass.

- [x] `[TPR-04-001][major]` `section-04-integer-narrowing.md:105` — **`AbiBoundary::ClosureCapture` listed but unspecified.** The variant appears in the `AbiBoundary` enum (§04.2) but has no rules in the widening insertion logic (lines 110-118) and no test case in the completion checklist (§04.5). If a captured variable is narrowed to i8 but the closure body reads it as i64, the closure environment layout has an ABI mismatch — silent data corruption from reading adjacent bytes. **Action:** Specify closure capture narrowing rules. Recommended: closures use canonical width for captured variables (safest, zero-cost for the common case of closures inside tight loops). Add test case for narrowed variable in closure capture. Consensus: 3/3 reviewers.
  Resolved: Validated on 2026-03-26. Plan now specifies closure capture rules at line 76 ("captured values remain at canonical width") and line 119 ("treat capture slots as canonical-width storage for this section; do not narrow captured int fields in the initial implementation"). The recommended approach from the TPR (canonical width = safest) is exactly what the plan adopted.

- [x] `[TPR-04-002][major]` `section-04-integer-narrowing.md:66-69` — **Function parameter narrowing requires all-call-site analysis; indirect calls not addressed.** The conservatism rules specify "narrow only if ALL call sites agree" but do not address function pointers stored as values (`(int) -> int` typed variables), closures passed as arguments, or indirect calls. A function narrowed based on visible direct call sites could be called through a function pointer with wider values, causing truncation. Trait methods are correctly marked `Disabled` but the same treatment is not extended to callable-value functions. **Action:** Add to conservatism rules: functions whose address is taken (stored in a variable of function type, passed as argument, or returned) must use `NarrowingPolicy::Disabled`. Consensus: 3/3 reviewers.
  Resolved: Validated on 2026-03-26. Plan now explicitly addresses this at line 75: "Address-taken functions / indirect-call targets: do NOT narrow parameters or returns; any function stored in a value of function type, passed as an argument, or returned uses canonical widths at the callable boundary." Additionally, `ori_arc::graph::call_graph` already excludes `ApplyIndirect` from the call graph, ensuring indirect call targets cannot be narrowed.

- [x] `[TPR-04-003][major]` `section-04-integer-narrowing.md:67` + `section-03-range-analysis.md:265-270,579` — **Struct field narrowing is unachievable with the specified range analysis.** §04.1 says "Struct fields: narrow aggressively" and the exit criteria requires `Pixel { r,g,b,a: int }` with 0..255 → 4-byte struct. However, §03's range analysis returns `Top` for all `Project` instructions (line 267-270: "Top unless we track per-field ranges") and `Top` for all `Construct` instructions (line 272-273). The type-level aggregation at §03 line 579 joins ALL int-typed variable ranges across ALL functions — which yields `Top` for any non-trivial program. There is no mechanism for per-field range tracking. **Action:** The plan needs either (a) per-field range tracking via `Construct` argument ranges aggregated by struct type and field position across all construction sites, or (b) a separate field-level analysis pass not covered by §03. The Pixel exit criteria is unachievable without this. Consensus: Agent 3 found, verified against plan text.
  Resolved: Fully implemented on 2026-03-26. §03.2b added `FieldSummaryTable` (`compiler/ori_repr/src/range/field_summary.rs`) with `observe_construct()` and `field_range()`. The fixpoint loop calls `update_field_summaries()` after each `Construct` instruction to populate per-(type, field) ranges. `Project` transfer function queries field summaries instead of returning Top. Field summaries are flushed to `ReprPlan` via `flush_to_repr_plan()`. §04 line 70 references this: "narrow aggressively, but ONLY from §03's field-summary table built from Construct sites." Pixel exit criterion is now achievable.

- [x] `[TPR-04-004][high]` `compiler/ori_repr/src/narrowing/int.rs:154` — Phase A still narrows `#repr("c", aligned N)` types, so a C-ABI layout can silently change under integer narrowing.
  Resolved: Fixed on 2026-03-26. Added `CAligned(_)` to `has_fixed_layout_attr()` match in `narrowing/int.rs`. Regression test `repr_c_aligned_struct_not_narrowed` added to `narrowing/tests.rs`. All 25 narrowing tests pass.

- [x] `[TPR-04-005][high]` `plans/repr-opt/section-04-integer-narrowing.md:61` `compiler/ori_repr/src/narrowing/int.rs:32` `compiler/ori_repr/src/plan.rs:74` — §04.1 claims the public-API / callable-boundary conservatism rules are implemented, but the current Phase A code has no visibility or export metadata and narrows every struct/tuple that lacks a `#repr` exemption.
  Resolved: Validated and accepted on 2026-03-26. Reopened the overclaimed conservatism checkbox — split into "implemented subset" (repr attrs, policy, field-summary-driven) and "pending" (visibility-based gating). Added concrete implementation tasks for `pub_type_indices` in `ReprPlan`, population from type checker, and test coverage. The conservatism design rules are now listed as a reference section with phase attribution.

- [x] `[TPR-04-006][high]` `compiler/ori_llvm/src/codegen/type_info/mod.rs:167` — Phase A narrowed struct/tuple decisions never reach LLVM type resolution, so integer narrowing is still a codegen no-op.
  Resolved: Accepted on 2026-03-26. Validated: `try_repr_to_llvm_type()` returns `None` for `MachineRepr::Struct`/`Tuple`, falling back to `TypeInfoStore` canonical `i64` fields. The §04.4 checkbox claiming "already complete" was incorrect — replaced with concrete implementation tasks: (1) extend `try_repr_to_llvm_type()` to handle recursive `Struct`/`Tuple` via `FieldRepr` widths, (2) add end-to-end semantic pin, (3) correct Pixel test expectation to signed narrowing rules. Implementation tasks integrated into §04.4.

- [x] `[TPR-04-007][low]` `compiler/oric/src/commands/codegen_pipeline.rs:501` — The visibility-gating follow-up pushed `codegen_pipeline.rs` past the 500-line hygiene limit, creating a new BLOAT rule violation in touched production code.
  Resolved: Accepted on 2026-03-26. Validated: file is 501 lines (was 491 pre-change). Implementation task added to §04.X: extract repr-plan computation block (lines 307–345) into a helper function `compute_repr_and_layout_info()` in an adjacent module, reducing `run_codegen_pipeline()` to ~464 lines.

- [x] `[TPR-04-008][medium]` `plans/repr-opt/section-04-integer-narrowing.md:185` `compiler/ori_llvm/src/codegen/type_info/mod.rs:167` — §04.4 still opens with a stale integration note claiming the LLVM side is "already correct," but the current resolver still returns `None` for `MachineRepr::Struct`/`Tuple` and falls back to canonical `i64` fields.
  Resolved: Fixed on 2026-03-26. §04.4 opening note rewritten to state that primitive-width overrides work but struct/tuple lowering is pending until `try_repr_to_llvm_type()` recursively consumes narrowed `FieldRepr` widths. Contradictory guidance eliminated.

- [x] `[TPR-04-009][medium]` `plans/repr-opt/section-04-integer-narrowing.md:277` — The Phase A/Phase C acceptance matrix still uses stale `0..255 -> i8` expectations that contradict the current signed-only narrowing rules and the accepted TPR-04-006 correction.
  Resolved: Fixed on 2026-03-26. Updated all stale `0..255 → i8` expectations to use `[-128, 127] → i8` (signed-consistent). Affected: Phase C collection test matrix, Phase A Pixel semantic pin, Phase C collection element test, and exit criteria paragraph.

- [x] `[TPR-04-010][low]` `plans/repr-opt/index.md:115` `plans/repr-opt/00-overview.md:332` — The plan index and overview still advertise Section 04 as "Not Started" even though the section file is `in-progress` and §04.1 work landed in `c8338d7c` / `9ad95d32`.
  Resolved: Fixed on 2026-03-26. Updated `index.md` and `00-overview.md` to show Section 04 as "In Progress". Also fixed Section 03 (was "Not Started", actually 97% complete).

- [x] `[TPR-04-011][high]` `compiler/ori_repr/src/lib.rs:98` `compiler/ori_repr/src/narrowing/int.rs:57` `compiler/ori_llvm/src/codegen/type_info/mod.rs:140` — Phase A still narrows the resolved struct/tuple idx that LLVM uses, so the new `#repr(...)` and `pub` conservatism gates do not protect the production path.
  Evidence: type registration records user types under `pool.named(decl.name)` and separately resolves them to concrete `struct_type`/`enum_type` idxs (`compiler/ori_types/src/check/registration/user_types.rs:31-67`). `compute_repr_plan_with_interner()` stores `repr_attrs` and `pub_type_indices` exactly as passed (`compiler/ori_repr/src/lib.rs:98-104`), and the existing semantic pins already prove that this metadata lives only on the named idx (`compiler/ori_repr/src/tests.rs:1996-2050`). Meanwhile `populate_canonical()` writes `MachineRepr` decisions for every pool idx, including the resolved struct idx (`compiler/ori_repr/src/canonical/mod.rs:106-140`). `narrow_struct_fields()` consults `repr_attr(idx)` / `is_public_type(idx)` on the candidate idx without canonicalizing (`compiler/ori_repr/src/narrowing/int.rs:55-67`, `compiler/ori_repr/src/plan.rs:218-232`), and `TypeLayoutResolver::resolve()` canonicalizes every lookup through `pool.resolve_fully(idx)` before reading the plan (`compiler/ori_llvm/src/codegen/type_info/mod.rs:134-145`). The result is that the named idx can remain exempt while the resolved struct idx still narrows and is the one codegen consumes.
  Impact: `#repr("c")`, `#repr("packed")`, `#repr("transparent")`, `#repr("c", aligned N)`, and public-type ABI promises are still unenforced on the real codegen path. Exported or FFI-visible struct/tuple layouts can therefore narrow anyway, reopening the correctness issue that TPR-04-004 and TPR-04-005 were meant to close.
  Required plan update: canonicalize representation metadata onto the resolved idxs used by narrowing/codegen (or make `repr_attr()` / `is_public_type()` resolve through `Pool::resolve_fully()` before lookup), then add regression tests that exercise the real named→resolved path and prove the resolved struct/tuple idx remains canonical.
  Resolved: Fixed on 2026-03-26. `compute_repr_plan_with_interner()` now resolves each `repr_attr` and `pub_type_indices` idx through `pool.resolve_fully()` and stores metadata under both the named AND resolved idx. 8 regression tests added to `tests.rs`: 4 propagation tests (C, Packed, CAligned, Transparent), 1 pub propagation test, 2 semantic pins (narrowing blocked on resolved idx for repr_attr and pub), 1 negative test (no propagation without resolution chain). 375/375 ori_repr tests pass, 14,230 total tests green.

- [x] `[TPR-04-012][high]` `compiler/ori_repr/src/lib.rs:101` — TPR-04-011 fixes only the direct `Named -> resolved` path; generic `Applied -> concrete Struct` resolutions still let public and `#repr(...)` types narrow on the production path.
  Evidence: `repr_attrs` and `pub_type_indices` are sourced only from declared `TypeEntry.idx` values in `type_result.typed.types` / `user_types` (`compiler/oric/src/commands/codegen_pipeline.rs:316-338`, `compiler/ori_llvm/src/evaluator/compile.rs:170-185`). The TPR-04-011 fix stores metadata on each input idx plus a single `pool.resolve_fully(idx)` result (`compiler/ori_repr/src/lib.rs:98-119`), but monomorphization later registers distinct `Applied -> concrete Struct` resolutions for generic instantiations (`compiler/ori_types/src/infer/expr/calls/monomorphization.rs:361-395`). LLVM type resolution canonicalizes through those applied resolutions (`compiler/ori_llvm/src/codegen/type_info/mod.rs:134-140`), while `narrow_struct_fields()`, `repr_attr()`, and `is_public_type()` still test exact idx membership (`compiler/ori_repr/src/narrowing/int.rs:55-67`, `compiler/ori_repr/src/plan.rs:214-232`). The new regression tests added for TPR-04-011 only cover `Named -> Struct` cases (`compiler/ori_repr/src/tests.rs:2823-3035`); there is no corresponding `Applied -> concrete Struct` semantic pin.
  Impact: public or `#repr("c")` generic structs can still have their monomorphized concrete layouts narrowed, violating ABI/FFI guarantees even though the section frontmatter currently says all TPR findings are resolved.
  Resolved: Implemented on 2026-03-27. Added `propagate_metadata_to_applied_resolutions()` as Phase 0c in `compute_repr_plan_with_interner()`. Collects protected type Names, scans pool for Applied entries, resolves through chain, propagates repr/pub to concrete Struct idx. 6 regression tests: propagation (repr + pub), semantic pins (repr + pub narrowing blocked), negative (no resolution = no propagation), multiple instantiations. 381/381 ori_repr tests green, 14,236 total tests green.

- [x] `[TPR-04-013][high]` `plans/repr-opt/section-04-integer-narrowing.md:20` `compiler/ori_repr/src/lib.rs:335` `compiler/ori_repr/src/narrowing/abi.rs:1` — This branch marked §04.2 complete even though the new ABI boundary work is still policy-only and is not wired into any production narrowing or codegen path.
  Resolved: Factual observation accepted on 2026-03-27. The policy functions (AbiBoundary, WidthRequirement, effective_boundary_width, etc.) have no production callers yet — correct. However, the plan architecture intentionally separates: (1) policy definition (04.2 scope — done), (2) production integration (04.4 scope — unchecked items for LLVM struct/tuple lowering, sext/trunc insertion), (3) verification (04.5 scope — unchecked Phase B LLVM IR tests). The 04.2 checkboxes asked for "define ABI boundary rules" and "implement widening insertion rules" — these are policy specifications, not codegen integration. Production consumption is tracked in 04.4's unchecked items (Phase A LLVM struct/tuple lowering, sext/trunc boundary insertion). The Phase B LLVM verification items cited (04.5 lines 310-316) belong to 04.5's scope, not 04.2. Keeping 04.2 as complete reflects its defined scope; 04.4 and 04.5 track the integration and verification work.

- [x] `[TPR-04-014][high]` `compiler/oric/src/commands/codegen_pipeline.rs:317` `compiler/ori_llvm/src/evaluator/compile.rs:169` `compiler/ori_types/src/output/mod.rs:170` `compiler/ori_types/src/pool/descriptor.rs:41` — Imported user-defined types still lose `#repr(...)` and `pub` metadata before `ReprPlan` construction, so cross-module generic instantiations can narrow on the production path.
  Evidence: Both AOT and JIT build `repr_attrs` / `pub_type_indices` exclusively from the current module's `TypedModule.types` ([codegen_pipeline.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/codegen_pipeline.rs:317), [compile.rs](/home/eric/projects/ori_lang/compiler/ori_llvm/src/evaluator/compile.rs:169)). `TypedModule.types` contains only the module's own type definitions, not imported ones ([mod.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/output/mod.rs:170)). Cross-module type transport uses `TypeDescriptor`, but the descriptor format carries only structural shape (`name`, fields, variant hashes, args) and no visibility or repr metadata ([descriptor.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/pool/descriptor.rs:41)). Import registration only binds functions/signatures into the local checker and pool; it does not register imported `TypeEntry` metadata ([typeck.rs](/home/eric/projects/ori_lang/compiler/oric/src/typeck.rs:165), [mod.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:425)). The new Phase 0c propagation in [lib.rs](/home/eric/projects/ori_lang/compiler/ori_repr/src/lib.rs:122) can only fan out metadata that was seeded into `repr_attrs` / `pub_type_indices` in the first place, so imported `pub` or `#repr("c")` generic types remain unprotected.
  Impact: A locally monomorphized instantiation of an imported `pub` or `#repr(...)` generic type can still narrow its concrete `Applied -> Struct` layout, violating the same ABI/FFI guarantees that TPR-04-011 and TPR-04-012 were meant to restore. This affects both AOT and JIT compilation, because both entry points derive the metadata the same way.
  Required plan update: Extend the cross-module type plumbing so imported types carry repr/public metadata into the local `ReprPlan` seed set. Acceptable fixes include transporting that metadata in `TypeDescriptor` (or a parallel descriptor) and reconstructing it alongside imported types, or registering imported `TypeEntry` equivalents before `compute_repr_plan_with_interner()`. Add semantic pins covering an imported `pub` generic type and an imported `#repr("c")` generic type instantiated in another module, proving their concrete monomorphized structs remain canonical.
  Resolved: Validated and accepted on 2026-03-27. All 5 evidence claims confirmed against codebase. Implementation task added to §04.X as `[CROSS-04-014]`. Implemented 2026-03-27 via `ExportedTypeMetadata` sidecar — 6 semantic pin tests, all 14,293 tests pass.

- [x] `[TPR-04-015][high]` `compiler/oric/src/commands/codegen_pipeline.rs:312` `compiler/oric/src/commands/compile_common.rs:48` `compiler/oric/src/commands/build/multi.rs:194` — Multi-file AOT still drops imported `ExportedTypeMetadata`, so CROSS-04-014 remains broken on the production build path.
  Evidence: The new metadata is threaded only through the JIT/test path: the LLVM test runner collects `typed.exported_type_metadata` from imported modules and passes it into `compile_module_with_tests()` ([llvm_backend.rs](/home/eric/projects/ori_lang/compiler/oric/src/test/runner/llvm_backend.rs:243), [compile.rs](/home/eric/projects/ori_lang/compiler/ori_llvm/src/evaluator/compile.rs:182)). The production AOT path still has no equivalent transport. `run_codegen_pipeline()` always calls `compute_module_repr_plan(..., &[])` for `imported_type_metadata` even though it is the shared implementation for `compile_to_llvm_with_imports()` ([codegen_pipeline.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/codegen_pipeline.rs:306)). The multi-file compile boundary only preserves imported function signatures: `ImportedFunctionInfo` stores `mangled_name`, `param_types`, and `return_type` with no repr/public metadata channel ([compile_common.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/compile_common.rs:48)), and `CompiledModuleInfo` / `build_import_infos()` likewise keep only public function type triples ([multi.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:194), [multi.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:367)). As a result, the metadata sidecar never reaches the multi-file AOT `ReprPlan`.
  Impact: A multi-module AOT build can still narrow imported `pub` or `#repr(...)` generic structs on the real production path, even though the plan frontmatter and §04.X currently say CROSS-04-014 is resolved. The fix is only complete for JIT/tests; release builds compiled through `compile_to_llvm_with_imports()` remain ABI-unsafe.
  Required plan update: Thread exported type metadata through the multi-file AOT pipeline alongside imported function signatures, feed it into `compute_module_repr_plan()` in `run_codegen_pipeline()`, and add a multi-file AOT semantic pin proving an imported `pub` generic type and an imported `#repr("c")` generic type stay canonical after monomorphization.
  Resolved: Validated and accepted on 2026-03-27. All 5 evidence claims confirmed against codebase. Implementation task added to §04.X as `[CROSS-04-015]`.

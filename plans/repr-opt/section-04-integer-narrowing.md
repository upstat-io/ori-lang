---
section: "04"
title: "Integer Narrowing Pipeline"
status: in-progress
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-27
  triage_note: "TPR-04-017 open: the LLVM JIT/test path still does not forward repr/pub metadata through direct imports that re-export protected types from another module."
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

- [ ] **Phase A — Struct field narrowing (primary):** In `apply_integer_narrowing()` (`compiler/ori_repr/src/lib.rs` stub at line 215), iterate all struct types in the Pool. For each struct field of type `int`, query `plan.field_range(struct_idx, field_index)`. If `range.min_width() < I64`, emit a narrowed `MachineRepr::Struct` decision with updated `FieldRepr` entries:
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
  - [x] End-to-end semantic pin: 6 AOT tests in `compiler/ori_llvm/tests/aot/narrowing.rs` — Pixel round-trip (trunc i64→i8 + sext i8→i64), struct update, mixed types (str + narrowed int + bool), field mutation, i8 boundary values (-128, 127), negative test (wide range stays canonical).
  - [x] Pixel test uses `[-128, 127]` for true i8 pin (signed narrowing).
  - [x] Tuple narrowing disabled in `narrow_struct_fields()` (`narrowing/int.rs`): `CandidateKind::Tuple` → skip with tracing. Tuple narrowing test updated to `tuple_elements_not_narrowed_phase_a`.

- [x] **Insert `sext`/`trunc` at narrowing boundaries** (2026-03-27): Struct field store and load boundaries implemented. Function entry/exit (Phase B) deferred.
  - [x] Struct field store (`construction.rs:29-34`): `trunc_for_narrowed_struct()` in `emitter_utils.rs` — checks pool field type is `Tag::Int` AND LLVM field is narrower, inserts `trunc i64 %val to i<N>`. Naturally narrow types (Byte, Char, Bool) pass through unchanged.
  - [x] Struct field load (`instr_dispatch.rs:216-224`): `sext_narrowed_field()` in `emitter_utils.rs` — checks ARC IR destination type is `Tag::Int`, inserts `sext i<N> %field to i64`. Non-int destinations pass through unchanged.
  - Function entry (Phase B): parameters arrive at canonical width → `trunc` to narrow if locally narrowed — **deferred to Phase B**
  - Function exit (Phase B): narrow local → `sext` to canonical width at boundary — **deferred to Phase B**

- [ ] Handle comparison operations correctly:
  - Signed comparison (`icmp slt`) on narrow types is correct for signed narrowing
  - Unsigned narrowing (future, for byte → int) needs `zext` not `sext`

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
- [ ] Write failing test matrix for Phase B BEFORE implementing Phase B
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
- [ ] Loop counters in `for i in 0..100` use `i8` local in generated LLVM IR (not `i64` alloca)
- [ ] Public function parameters are NOT narrowed
- [ ] Trait method parameters are NOT narrowed (unknown callers)
- [ ] Address-taken / indirectly-called functions are NOT narrowed at their callable boundary
- [ ] ABI boundary widening: `sext` visible at function entry (param → narrow local) and at return (narrow local → canonical) in LLVM IR
- [ ] Closure-captured ints stay canonical-width in the closure environment unless a separate closure-layout contract lands with its own tests
- [ ] Semantic pin for Phase B: a loop counter `i` in `for i in 0..100` produces an `i8` local variable in LLVM IR — no `i64` alloca for `i`

**Phase B — Overflow guard insertion:**
- [x] `can_overflow(BinaryOp::Add, lhs, rhs, target)` returns `true` when result range exceeds target width (2026-03-27): `add_overflows_i8`, `add_fits_in_i16_not_i8` tests
- [x] `can_overflow(BinaryOp::Sub, lhs, rhs, target)` correctly detects subtract overflow (2026-03-27): `sub_overflows_i8`, `sub_no_overflow_in_i8` tests
- [x] `can_overflow(BinaryOp::Mul, lhs, rhs, target)` correctly detects multiply overflow (2026-03-27): `mul_overflows_i8`, `mul_no_overflow_in_i8` tests
- [x] For non-arithmetic ops (`BinaryOp::Eq`, etc.), `can_overflow()` conservatively returns `true` (uses `ValueRange::Top`) (2026-03-27): `comparison_op_conservative_overflow` test
- [ ] Overflow guards inserted where narrowed arithmetic might overflow (strategy (c) when provable safe, (a) otherwise) — **depends on Phase B local variable narrowing in §04.4 LLVM integration**

**NarrowingPolicy behavior:**
- [ ] `NarrowingPolicy::Disabled` (via `--no-repr-opt` / `ORI_NO_REPR_OPT`) suppresses ALL narrowing — Pixel struct stays 32 bytes, loop counters stay `i64`
- [ ] `--no-repr-opt` flag passes `NarrowingPolicy::Disabled` to `ReprPlan` via `NarrowingPolicy::from(policy_flag)` or equivalent in the CLI
- [ ] `NarrowingPolicy::Conservative` vs `Aggressive`: verify that Conservative is strictly a subset of Aggressive (Conservative never narrows what Aggressive does not)

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

- [ ] `[TPR-04-017][high]` `compiler/oric/src/test/runner/llvm_backend.rs:252` `compiler/ori_types/src/check/mod.rs:923` `compiler/oric/src/commands/build/multi.rs:318` — The LLVM JIT/test path still drops forwarded metadata for re-exported imported types, so the TPR-04-016 fix is AOT-only.
  Evidence: The JIT runner only flattens each direct import's raw `typed.exported_type_metadata` into `imported_type_metadata` ([llvm_backend.rs](/home/eric/projects/ori_lang/compiler/oric/src/test/runner/llvm_backend.rs:252)), but `TypedModule.exported_type_metadata` is still generated solely from the module's local `TypeEntry` list via `generate_exported_type_metadata(&types)` ([check/mod.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:923), [check/mod.rs](/home/eric/projects/ori_lang/compiler/ori_types/src/check/mod.rs:973)). The new transitive forwarding helper exists only in the multi-file AOT path: `compile_single_module()` merges imported metadata into `CompiledModuleInfo.exported_type_metadata` with `merge_forwarded_metadata()` before later consumers read it ([build/multi.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:318), [build/multi.rs](/home/eric/projects/ori_lang/compiler/oric/src/commands/build/multi.rs:440)). The JIT path has no equivalent merge even though it re-interns direct imports' signatures/canons into the merged pool and compiles those imported functions in the same LLVM module ([llvm_backend.rs](/home/eric/projects/ori_lang/compiler/oric/src/test/runner/llvm_backend.rs:202), [llvm_backend.rs](/home/eric/projects/ori_lang/compiler/oric/src/test/runner/llvm_backend.rs:272)), so a direct import of module `B` is sufficient to bring a re-exported type from module `C` into codegen without ever forwarding `C`'s repr/public metadata.
  Impact: `A -> B -> C` still narrows protected `pub` / `#repr(...)` types on the LLVM JIT/test path whenever `A` imports only `B` and `B` exposes `C`'s type in its public API. The branch currently marks TPR-04-016 resolved and documents this as a "JIT limitation," but the missing transitive module load is not the blocker here: the blocker is that forwarded metadata never reaches `TypedModule.exported_type_metadata` or the JIT flattening step.
  Required plan update: Make forwarded metadata available on the JIT path as well, preferably by merging signature-reachable imported metadata into `TypedModule.exported_type_metadata` during type checking so both JIT and AOT consume the same propagated set. At minimum, add a JIT-side equivalent of `merge_forwarded_metadata()` before `compile_module_with_tests()`. Add a regression with `A -> B -> C` where `C` defines a protected generic type, `B` re-exports it in a public signature, and an LLVM-backed test imports only `B` and verifies the concrete layout stays canonical.

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

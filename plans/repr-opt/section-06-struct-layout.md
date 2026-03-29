---
section: "06"
title: "Struct & Tuple Layout Optimization"
status: not-started
reviewed: true
goal: "Reorder struct fields for optimal alignment and minimal padding, then record the layout in ReprPlan for codegen"
inspired_by:
  - "Rust repr(Rust) layout algorithm (compiler/rustc_abi/src/layout.rs)"
  - "Zig struct layout optimization (src/Type.zig)"
  - "LLVM DataLayout (lib/IR/DataLayout.cpp)"
depends_on: ["04", "05"]
sections:
  - id: "06.0"
    title: "Prerequisites: layout module split + codegen field remapping"
    status: not-started
  - id: "06.1"
    title: "Field Reordering Algorithm"
    status: not-started
  - id: "06.2"
    title: "Padding Tracking & Diagnostics"
    status: not-started
  - id: "06.3"
    title: "ABI-Stable Opt-Out"
    status: not-started
  - id: "06.4"
    title: "Tuple Layout"
    status: not-started
  - id: "06.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Struct & Tuple Layout Optimization

**Context:** The spec (Annex E — System Considerations) explicitly permits struct field reordering: "Struct field order in memory may differ from declaration order." This is a non-guarantee. Rust's `repr(Rust)` does exactly this — it reorders fields by alignment to minimize padding. Ori should do the same.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: Fields sorted by descending alignment, then by descending size
- **Zig** `src/Type.zig`: ABI-optimal layout with explicit alignment control
- **C/C++**: No reordering (declaration order = memory order) — this is why `#pragma pack` exists

**Depends on:** §04, §05 (need to know narrowed field sizes before computing layout).

**Codegen consumers that use field indices (ALL must be remapped after §06):**
1. `ArcIrEmitter::emit_project()` — `extract_value(val, field, ...)` and `struct_gep(ty, val, field, ...)`
2. `ArcIrEmitter::emit_construct()` — `build_struct(llvm_ty, &narrowed_args, ...)` (args ordered by declaration)
3. `ArcIrEmitter::emit_instr() → ArcInstr::Set` — `struct_gep(llvm_ty, base_val, *field, ...)`
4. `compile_for_each_field()` in `derive_codegen/bodies.rs` — `extract_value(val, i as u32, ...)` where `i` is declaration-order enumeration
5. `compile_format_fields()` in `derive_codegen/bodies.rs` — same pattern as above
6. `compile_clone_fields()` in `derive_codegen/bodies.rs` — same pattern as above
7. `compile_default_construct()` in `derive_codegen/bodies.rs` — builds struct with `build_struct()` in declaration order
8. `DropFunctionGenerator` in `arc_emitter/drop_gen.rs` — iterates fields for drop emission
9. `field_scan/mod.rs` — `ArcInstr::Project { field, .. }` used for field usage tracking (read-only analysis; field values opaque to remapping — does NOT need remapping)
10. `sext_narrowed_field()` / `trunc_for_narrowed_struct()` in `narrowing_codegen.rs` — use field index to look up narrowed width from `StructRepr.fields`

**Non-affected consumers (no remapping needed):**
- Closure environment codegen (`closures.rs`, `closure_wrappers.rs`) — closure environments are not user structs; they have compiler-controlled layout and are not subject to §06 reordering.
- Enum variant payload codegen (`drop_enum.rs`, `emit_variant_via_*`) — enum payloads use `Vec<MachineRepr>` (no `FieldRepr`), and §07 handles enum layout separately.

---

## 06.0 Prerequisites: layout module split + codegen field remapping

**Current state:**
- `compiler/ori_repr/src/layout.rs` is a **flat file** (not a directory). It contains `is_trivial_repr()`, `field_size()`, `field_align()`, `repr_size()`, `repr_align()`, `round_up()`, `compute_field_layout()`, `compute_payload_layout()`, and `TupleRepr::to_machine_repr()`.
- `MachineRepr` has **no** `.size()` or `.alignment()` methods — size/alignment are computed via the free functions `field_size(&repr)` / `field_align(&repr)` (for aggregate fields) and `repr_size(&repr)` / `repr_align(&repr)` (for standalone values) in `layout.rs`, all `pub(crate)`.
- `FieldRepr` has fields `name: Name`, `original_index: u32`, `offset: u32`, `repr: MachineRepr` — there is **no** `type_idx` field. The narrowed representation is stored directly in `FieldRepr.repr` by §04/§05 narrowing passes.
- **`FieldInfo` does not exist anywhere in the codebase.** The layout algorithm operates on `FieldRepr` directly.
- `canonical_struct()` in `canonical/type_repr.rs` already populates `FieldRepr` with `offset: 0` and a comment "Set by §06 layout". The layout is computed by `compute_field_layout()` for the struct's `size` and `align`, but individual field offsets are left at 0.
- The pipeline stub `compute_struct_layouts()` already exists in `pipeline.rs:469` as an empty function.

**Critical codegen concern — field index remapping:**
- The ARC IR uses `ArcInstr::Project { field: u32 }` where `field` is the **declaration-order** index.
- Codegen in `arc_emitter/instr_dispatch.rs` passes this `field` directly to `struct_gep()` as the LLVM struct field index.
- After §06 reorders `StructRepr.fields` (changing the memory order), the LLVM struct type has fields in a **different order** than the ARC IR expects.
- **§06 must provide a `original_to_memory` index mapping** so codegen can translate `ArcInstr::Project { field: 3 }` (declaration index 3) → `struct_gep(memory_index)` (the reordered position).
- The same remapping is needed for `ArcInstr::Construct` (struct construction) and `ArcInstr::Set` (field mutation).
- `try_lower_narrowed_aggregate()` in `layout_resolver.rs` iterates `StructRepr.fields` in order — after reordering, this produces the LLVM struct type in memory order (correct), but codegen must use the remapped index for GEP access.

**Prerequisite steps:**

- [ ] Convert `layout.rs` to a module directory:
  - `mkdir compiler/ori_repr/src/layout/`
  - Move `compiler/ori_repr/src/layout.rs` → `compiler/ori_repr/src/layout/mod.rs` (existing 177-line file, well under limit)
  - Create `compiler/ori_repr/src/layout/struct_layout.rs` — new: field reordering algorithm + ABI-stable layout functions (§06.1 + §06.3)
  - Create `compiler/ori_repr/src/layout/tuple_layout.rs` — new: tuple layout (§06.4)
  - Create `compiler/ori_repr/src/layout/tests.rs` — new: unit tests for layout algorithms; add `#[cfg(test)] mod tests;` to `mod.rs`
  - `mod layout;` in `lib.rs` auto-discovers the directory module — no change needed
  - Add `pub(crate) use struct_layout::optimize_struct_layout;` and `pub(crate) use tuple_layout::optimize_tuple_layout;` re-exports in `layout/mod.rs`

- [ ] Add `StructRepr` helper methods for index remapping:
  ```rust
  impl StructRepr {
      /// Find the field with the given original (declaration-order) index.
      ///
      /// Returns `None` if no field has that original index — a bug.
      pub fn field_by_original(&self, original_index: u32) -> Option<&FieldRepr> {
          self.fields.iter().find(|f| f.original_index == original_index)
      }

      /// Get the memory-order index for a given declaration-order index.
      ///
      /// After §06 reordering, `fields[memory_index].original_index == original_index`.
      /// Before §06 (or for `#repr("c")`), memory order == declaration order.
      pub fn memory_index(&self, original_index: u32) -> Option<usize> {
          self.fields.iter().position(|f| f.original_index == original_index)
      }
  }
  ```

- [ ] Wire codegen field-index remapping into `ArcIrEmitter`:
  - When emitting `ArcInstr::Project { field }`, look up the `StructRepr` from `ReprPlan`, call `memory_index(field)`, and use that as the `struct_gep` index. The `extract_value` path (line ~221 in `instr_dispatch.rs`) uses `field` directly — must also remap.
  - For `ArcInstr::Set { field }` at `instr_dispatch.rs:408`: `struct_gep(llvm_ty, base_val, *field, ...)` — remap `*field` via `memory_index()`.
  - For `ArcInstr::Construct` at `construction.rs:29-33`: args arrive in declaration order. After §06, the LLVM struct type expects fields in memory order. Reorder args from declaration order to memory order using `StructRepr.fields` BEFORE calling `trunc_for_narrowed_struct()` and `build_struct()`. Add a helper `fn reorder_args_to_memory_order(&self, args: &[ValueId], ctor_type: Idx) -> Vec<ValueId>` on `ArcIrEmitter` that uses `StructRepr.fields[i].original_index` to build a reordered args vector.
  - When `ReprPlan` has no entry for the struct type (fallback to `TypeInfoStore`), use the original field index unchanged (backwards-compatible). This fallback also applies when `self.repr_plan` is `None` (JIT path without repr-opt).

- [ ] Wire codegen field-index remapping into `derive_codegen`:
  - `compile_for_each_field()` in `bodies.rs` uses `extract_value(self_val, i as u32, ...)` where `i` iterates `FieldDef` (declaration order). After §06, the LLVM struct fields are in memory order. Must remap `i` to `memory_index(i)` before `extract_value`.
  - Same for `compile_format_fields()`, `compile_clone_fields()`, and `compile_default_construct()`.
  - **Approach**: derive codegen already receives `type_idx: Idx` — use `ReprPlan::repr(type_idx)` to get `MachineRepr::Struct(StructRepr)`, then `struct_repr.memory_index(i)` for each field access. Pass `ReprPlan` reference to derive codegen functions (currently they only have `FunctionCompiler`; `FunctionCompiler` already has access to `ReprPlan` via `self.repr_plan` or through the codegen context — verify the plumbing exists; if not, thread it through).
  - **Construct remapping in `compile_default_construct()`**: builds struct with `build_struct()` — args must be reordered from declaration order to memory order before insertion.

- [ ] Wire codegen field-index remapping into `DropFunctionGenerator` (`arc_emitter/drop_gen.rs`):
  - `DropKind::Fields(Vec<(u32, Idx)>)` stores `(field_index, field_type)` where `field_index` is computed by `compute_fields_drop()` in `ori_arc/src/drop/mod.rs` via `enumerate()` over Pool struct fields (declaration order). These indices are passed directly to `struct_gep()` in `emit_drop_fields()` at `drop_gen.rs:131`.
  - After §06, the LLVM struct has fields in memory order, so these declaration-order indices are wrong for `struct_gep`.
  - **Approach**: In `emit_drop_fields()`, before calling `struct_gep()`, look up the `ReprPlan` entry for the type. If a `StructRepr` exists, remap `field_index` via `struct_repr.memory_index(field_index)`. If no entry exists (type not in ReprPlan, e.g., closure envs or types with no canonical repr), use the original index unchanged.
  - **Plumbing**: `ArcIrEmitter` already has `repr_plan: Option<&'a ReprPlan>` (field at `mod.rs:214`). The drop gen methods are `impl ArcIrEmitter` methods — they have access to `self.repr_plan`. Add a helper `fn remap_struct_field(&self, ty: Idx, field_index: u32) -> u32` on `ArcIrEmitter` that does the ReprPlan lookup + memory_index translation, returning the original index if no ReprPlan entry exists.
  - **ClosureEnv**: `DropKind::ClosureEnv(Vec<(u32, Idx)>)` has the same shape but closure environments are NOT user structs and are NOT reordered by §06. The remap helper must distinguish: only remap when `Pool::tag(resolved_ty) == Tag::Struct || Tag::Tuple`. Closure env types resolve to `Tag::ClosureEnv` or similar — verify the exact tag.

- [ ] Wire field-index remapping into `narrowing_codegen.rs`:
  - **`sext_narrowed_field(extracted, field_index, dst_type)`** at `narrowing_codegen.rs:420`: Does NOT need StructRepr lookup. It only uses `dst_type` (the ARC IR destination type's Pool `Idx`) to check `Tag::Int` or `Tag::Float`, then widens the extracted LLVM value. The `field_index` is only used for LLVM label naming (`narrow.sext.{field_index}`). **No remapping needed here** — the extracted value is already the correct field value (extracted via a remapped index in `emit_project`).
  - **`trunc_for_narrowed_struct(struct_ty_id, args, ctor_type)`** at `narrowing_codegen.rs:318`: This IS affected. It iterates `args` (declaration order from `ArcInstr::Construct`) and queries `st.get_field_type_at_index(i as u32)` to get the LLVM struct field type. After §06, the LLVM struct has fields in memory order, so `args[i]` (declaration-order value for field i) may not correspond to `st.get_field_type_at_index(i)` (memory-order field at position i).
  - **Approach for `trunc_for_narrowed_struct`**: Before iterating, look up `StructRepr` from `self.repr_plan` for `ctor_type`. If reordered, remap: for each declaration-order arg `i`, find its memory-order position `mem_i = struct_repr.memory_index(i)`, and compare the arg's type against `st.get_field_type_at_index(mem_i)`. Alternatively (simpler): reorder args to memory order first (using `StructRepr.fields` order), then iterate normally. The reordered args array is also needed by `emit_construct` in `construction.rs:33` where `build_struct(llvm_ty, &narrowed_args, ...)` expects args in LLVM struct field order (memory order after §06).
  - **Unified reorder point**: The arg reordering from declaration order to memory order should happen in `emit_construct()` in `construction.rs:29-33` BEFORE calling `trunc_for_narrowed_struct`. This way, both `trunc_for_narrowed_struct` and `build_struct` receive args in memory order, and no further changes are needed in `narrowing_codegen.rs`.

- [ ] Implement `compute_struct_layouts` pipeline stub (`pipeline.rs:469`):
  - Currently `fn compute_struct_layouts(_plan: &mut ReprPlan, _pool: &Pool) {}` — empty stub.
  - Iterate all type `Idx` values via `plan.decision_indices()` (method at `plan.rs:393`).
  - For each, call `plan.get_repr(idx)` and match on `MachineRepr::Struct(struct_repr)` or `MachineRepr::Tuple(TupleRepr in MachineRepr::Tuple(...))`.
  - For structs: call `optimize_struct_layout(&struct_repr, plan.repr_attr(idx))` → new `StructRepr`. Wrap in `MachineRepr::Struct(new_repr)`. Write back via `plan.set_repr(idx, ReprDecision { source: DecisionSource::StructLayout, type_idx: idx, repr: new_machine_repr, reason: DecisionReason::Custom("field reordering".into()) })`.
  - For tuples: call `optimize_tuple_layout(&tuple_repr)` → new `TupleRepr`. Wrap and write back similarly.
  - `DecisionSource::StructLayout` variant already exists in `plan/decision.rs:37` — no addition needed.
  - **Import**: Add `use crate::layout::{optimize_struct_layout, optimize_tuple_layout};` to `pipeline.rs`.
- [ ] **[BLOAT]** `pipeline.rs` is at 495 lines (limit: 500). Adding the `compute_struct_layouts` body will exceed the limit. Extract the Phase 0 metadata functions (`seed_imported_metadata`, `propagate_metadata_to_applied_resolutions`) into `compiler/ori_repr/src/pipeline/metadata.rs` to bring the file under 400 lines. Then `compute_struct_layouts` has room.

**Test strategy for §06.0 (TDD — write tests FIRST, verify they pass with identity mapping):**

Tests go in `compiler/ori_repr/src/struct_repr/tests.rs` (for helper methods) and the existing `compiler/ori_repr/src/tests.rs` (for pipeline integration). Since §06.0 wires remapping as NO-OP (declaration order == memory order), tests assert the identity invariant.

- [ ] **Rust unit tests** — `StructRepr` helpers (`struct_repr/tests.rs`):
  - `field_by_original(0)` returns first field on identity-ordered struct
  - `field_by_original(N)` returns `None` for out-of-range index
  - `memory_index(i) == i` for all fields in a declaration-ordered struct (3+ fields)
  - `memory_index(N)` returns `None` for out-of-range index
  - Empty struct: `memory_index(0)` returns `None`
  - **Semantic pin**: after §06.1 reorders, `memory_index(0) != 0` for `{ a: bool, b: int }` — this test is written now but expected to fail; it becomes the pin after §06.1
- [ ] **Regression test** — `./test-all.sh` green after all remapping wiring (no-op path exercises existing codegen, proving the wiring doesn't break anything)
- [ ] **Debug AND release builds**: `cargo b` and `cargo b --release` both succeed after wiring

**Done criteria for §06.0:**
- `compiler/ori_repr/src/layout/` is a directory module with `mod.rs`, `struct_layout.rs`, `tuple_layout.rs`, `tests.rs`
- `StructRepr::field_by_original()` and `StructRepr::memory_index()` exist and have unit tests
- All codegen field-index remapping is wired but NO-OP (because `compute_struct_layouts` is still empty — fields are in declaration order, so `memory_index(i) == i`)
- `./test-all.sh` green — remapping wiring introduces no regressions
- `pipeline.rs` is under 500 lines (metadata functions extracted)

---

## 06.1 Field Reordering Algorithm

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs` (new file in the converted layout module)
**Note**: `optimize_struct_layout()` dispatches to `compute_c_layout()`, `compute_packed_layout()`, `compute_transparent_layout()` which are specified in §06.3. In practice, §06.1 and §06.3 must be co-implemented in the same file. The split is for conceptual clarity — implement both together.

- [ ] Implement the field reordering algorithm:
  ```rust
  use crate::layout::{field_size, field_align, round_up, is_trivial_repr};
  use crate::struct_repr::{FieldRepr, StructRepr};
  use crate::plan::{ReprAttribute, ReprPlan};
  use ori_types::Idx;

  /// Reorder struct fields for optimal alignment and minimal padding.
  ///
  /// Reads the existing `StructRepr` from the plan (already populated by
  /// `canonical_struct()` with narrowed field reprs from §04/§05),
  /// reorders `fields` by descending alignment then descending size,
  /// computes byte offsets, and writes the updated `StructRepr` back.
  ///
  /// Skips types with `#repr("c")`, `#repr("packed")`, or
  /// `#repr("transparent")` attributes — those have user-controlled layout.
  pub(crate) fn optimize_struct_layout(
      struct_repr: &StructRepr,
      repr_attr: Option<&ReprAttribute>,
  ) -> StructRepr {
      // Step 0: Check for ABI-stable opt-out
      match repr_attr {
          Some(ReprAttribute::C | ReprAttribute::CAligned(_)) => {
              return compute_c_layout(struct_repr, repr_attr);
          }
          Some(ReprAttribute::Packed) => {
              return compute_packed_layout(struct_repr);
          }
          Some(ReprAttribute::Transparent) => {
              return compute_transparent_layout(struct_repr);
          }
          Some(ReprAttribute::Aligned(n)) => {
              let mut result = reorder_and_layout(struct_repr);
              result.align = result.align.max(*n);
              result.size = round_up(result.size, result.align);
              return result;
          }
          Some(ReprAttribute::Default) | None => {}
      }

      reorder_and_layout(struct_repr)
  }

  fn reorder_and_layout(struct_repr: &StructRepr) -> StructRepr {
      // Step 1: Build (memory_pos, size, align) tuples for sorting
      let mut indexed: Vec<(usize, u32, u32)> = struct_repr.fields.iter()
          .enumerate()
          .map(|(i, f)| {
              let size = field_size(&f.repr);
              let align = field_align(&f.repr);
              (i, size, align)
          })
          .collect();

      // Step 2: Sort by descending alignment, then descending size.
      // MUST use stable sort (sort_by, not sort_unstable_by) so that
      // fields with equal alignment AND equal size preserve their
      // original declaration order — deterministic layout.
      indexed.sort_by(|a, b| {
          b.2.cmp(&a.2)  // alignment descending
              .then(b.1.cmp(&a.1))  // size descending
      });

      // Step 3: Compute offsets in sorted order
      let mut offset = 0u32;
      let mut max_align = 1u32;
      let mut layout_fields = Vec::with_capacity(struct_repr.fields.len());

      for &(src_idx, size, align) in &indexed {
          offset = round_up(offset, align);
          let orig = &struct_repr.fields[src_idx];
          layout_fields.push(FieldRepr {
              name: orig.name,
              original_index: orig.original_index,
              offset,
              repr: orig.repr.clone(),
          });
          offset += size;
          max_align = max_align.max(align);
      }

      // Step 4: Trailing padding for array alignment
      let total_size = round_up(offset, max_align);

      StructRepr {
          fields: layout_fields,
          size: total_size,
          align: max_align,
          trivial: struct_repr.trivial,
      }
  }
  ```

- [ ] Handle zero-sized fields (unit, never):
  - `field_size()` and `field_align()` in `layout.rs` already return 0 and 1 respectively for Unit/Never — the sorting puts them last (smallest alignment), and they contribute 0 bytes to the offset. They still get an offset entry for codegen correctness.

- [ ] Handle edge cases in the reordering algorithm:
  - **Empty structs** (0 fields): `reorder_and_layout()` returns `StructRepr { fields: vec![], size: 0, align: 1, trivial: true }`. The `max_align` starts at 1 (never updated), offset stays at 0. Verify this path.
  - **Single-field structs**: no reordering possible — algorithm degenerates to identity. Still compute correct offset (0) and size (rounded up to alignment).
  - **Generic structs**: By the time `ori_repr` sees them, generics are monomorphized — `canonical_struct()` operates on fully-resolved `Idx` values from the Pool. No special handling needed, but add a test confirming `struct Pair<T> { a: T, b: int }` instantiated as `Pair<bool>` gets reordered (int first, bool second).
  - **Newtypes** (`type UserId = int`): These are structurally single-field structs with implicit `#repr("transparent")` semantics. The `canonical_struct()` path in `type_repr.rs` handles them as normal structs. `compute_transparent_layout()` handles the `#repr("transparent")` attribute. Newtypes without an explicit `#repr` get the default layout (single-field, no reordering, size = field size).
  - **Recursive types** (e.g., `type Node = { value: int, next: Option<Node> }`): The `Option<Node>` field canonicalizes to `RcPointer(...)` (heap-allocated), which has a fixed 8-byte size. The reordering algorithm sees `int` (8 bytes, align 8) and `RcPointer` (8 bytes, align 8) — no reordering needed, but the algorithm must handle it correctly without infinite recursion. The recursion guard is in `canonical_struct()` (the `visiting` set), not in the layout algorithm — by the time §06 runs, all `StructRepr` values are fully resolved.

**Test strategy for §06.1 (TDD — write failing tests FIRST):**

Tests go in `compiler/ori_repr/src/layout/tests.rs`. Write all unit tests before implementing `reorder_and_layout()`. Verify they fail (returning declaration-order layout), then implement.

- [ ] **Write failing Rust unit test matrix BEFORE implementation** (all in `layout/tests.rs`):

  Matrix dimensions: **struct shape** x **field type mix** x **expected property**

  | Test name | Input fields (decl order) | Expected memory order | Expected size | Pin type |
  |---|---|---|---|---|
  | `reorder_bool_int_bool` | `bool(1), int(8), bool(1)` | `int, bool, bool` | 16 | Semantic: size 16 not 24 |
  | `reorder_already_optimal` | `int(8), int(8)` | `int, int` | 16 | Negative: no regression |
  | `reorder_four_bytes_and_int` | `byte(1), byte(1), byte(1), byte(1), int(8)` | `int, byte, byte, byte, byte` | 16 | Semantic: 12 data + 4 pad |
  | `reorder_mixed_widths` | `bool(1), float(8), byte(1), int(8)` | `float, int, bool, byte` | 24 | Semantic: reorder by align then size |
  | `reorder_empty_struct` | `(none)` | `(none)` | 0, align 1 | Edge: no panic |
  | `reorder_single_field` | `int(8)` | `int` | 8, align 8 | Edge: identity |
  | `reorder_all_same_align` | `bool(1), byte(1), bool(1)` | preserves declaration order (stable sort) | 3, align 1 | Semantic: stable sort |
  | `reorder_zst_fields` | `int(8), Unit(0), bool(1)` | `int, bool, Unit` | 16 | Edge: ZST last |
  | `reorder_narrowed_fields` | `bool(1), i16(2), f32(4)` | `f32, i16, bool` | 8 | Semantic: narrowed sizes |
  | `reorder_preserves_original_index` | `bool(1), int(8)` | fields[0].original_index == 1 (int), fields[1].original_index == 0 (bool) | 16 | Invariant: original_index preserved |

- [ ] Verify all tests FAIL with current identity layout (fields stay in declaration order, sizes may differ)
- [ ] Implement `reorder_and_layout()` and `optimize_struct_layout()`
- [ ] Verify all tests PASS unchanged (no test modifications allowed)
- [ ] **Semantic pin**: `reorder_preserves_original_index` can ONLY pass with §06 reordering — if reverted, `fields[0].original_index` would be 0 (bool), not 1 (int)

**Done criteria for §06.1:**
- `optimize_struct_layout()` and `reorder_and_layout()` implemented in `layout/struct_layout.rs`
- Unit tests for all edge cases (empty, single-field, generic, newtypes, recursive, ZST, narrowed, stable sort) in `layout/tests.rs`
- `compute_struct_layouts()` in `pipeline.rs` calls `optimize_struct_layout()` for all struct types in `ReprPlan`
- `struct { a: bool, b: int, c: bool }` produces `StructRepr.size == 16` (not 24) in unit tests
- `./test-all.sh` green

---

## 06.2 Padding Tracking & Diagnostics

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs`

- [ ] Track padding bytes per struct and emit a tracing diagnostic when padding exceeds 25% of total size:
  ```rust
  let data_bytes: u32 = layout_fields.iter()
      .map(|f| field_size(&f.repr))
      .sum();
  let padding = total_size.saturating_sub(data_bytes);
  if total_size > 0 && padding > total_size / 4 {
      tracing::debug!(
          total_size,
          padding,
          data_bytes,
          "struct has >25% padding despite field reordering"
      );
  }
  ```

**Note on bitfield packing (NOT §06 scope — distinct optimization):**

Packing multiple `bool`/`byte`/`Ordering` fields into sub-byte bitfields is a **separate optimization** from field reordering. It would require:
1. Codegen to emit bit-level insert/extract for every field access, pattern match, and derive body
2. `ArcIrEmitter` changes across `Project`, `Construct`, `Set`, and all derive codegen strategies
3. `ori_eval` changes (interpreter field access) for dual-execution parity

This is architecturally distinct from §06's field reordering — it changes the representation of individual fields, not their ordering. The natural packing from alignment sorting already places `bool`/`byte`/`Ordering` fields (1-byte aligned) contiguously at the end of the struct, achieving good spatial locality without sub-byte complexity.

- [ ] Add bitfield packing as a concrete checkbox in §11 (Collection Specialization) or a dedicated §11-adjacent section, if profiling data from §12 shows it matters. This is tracked here to ensure it is not lost.

**Test strategy for §06.2 (TDD):**

Tests go in `compiler/ori_repr/src/layout/tests.rs`. Use `tracing-test` or a tracing subscriber mock to capture diagnostic output.

- [ ] **Write failing tests BEFORE implementation:**
  - `padding_diagnostic_fires_over_25_percent`: struct with high padding ratio (e.g., `{ a: bool, b: int }` where 7/16 = 43% is padding) emits `tracing::debug!`
  - `padding_diagnostic_silent_under_25_percent`: struct with low padding (e.g., `{ a: int, b: int }` where 0% is padding) emits no diagnostic
  - `padding_diagnostic_empty_struct`: empty struct (size 0) does not panic or emit diagnostic
  - `padding_diagnostic_exact_threshold`: struct at exactly 25% padding boundary — verify correct behavior (test documents the >= vs > decision)

**Done criteria for §06.2:**
- Padding tracing diagnostic emitted for structs with >25% padding
- Unit tests verify the diagnostic fires and stays silent using the test matrix above

---

## 06.3 ABI-Stable Opt-Out

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs`

For FFI interop, users need control over memory layout. The `#repr` attribute infrastructure is already in place:
- `ReprAttrKind` enum in `ori_ir::ast::items::types` (parsed by `ori_parse`)
- `ReprAttribute` enum in `ori_repr::plan::repr_attr` (C, Packed, Transparent, Aligned, CAligned, Default)
- `compute_repr_plan_with_interner()` converts `ReprAttrKind → ReprAttribute` via `convert_repr_attr_kind()` and stores in `ReprPlan::repr_attrs` (keyed by `Idx`)
- `plan.repr_attr(idx)` query returns `Option<&ReprAttribute>` for any type

The layout algorithm queries `repr_attr` and dispatches to the appropriate layout strategy:

- [ ] Implement `compute_c_layout()` for `#repr("c")` / `#repr("c") + #repr("aligned", N)`:
  - Fields in **declaration order** (use `original_index` to maintain source order)
  - Platform-specific alignment (matches target C ABI: `field_align()` already gives correct values)
  - No field reordering, no narrowing of field types (§04 already skips `#repr("c")` types via `has_fixed_layout_attr()`)
  - For `CAligned(N)`: struct alignment = `max(computed, N)`

- [ ] Implement `compute_packed_layout()` for `#repr("packed")`:
  - Fields in declaration order
  - Every field offset = previous field's end (no alignment padding)
  - Struct alignment = 1
  - Note: may require unaligned loads in codegen (LLVM handles this via `align 1` on load/store)

- [ ] Implement `compute_transparent_layout()` for `#repr("transparent")`:
  - Validate: exactly one non-ZST field (check `field_size(&f.repr) > 0`)
  - Struct size = that field's size, alignment = that field's alignment
  - Error if 0 or 2+ non-ZST fields (diagnostic: use existing error accumulation pattern)
  - Note: validation should ideally happen at type-check time (§06 can add a `debug_assert!` for safety, but the primary check belongs in `ori_types` — if not already present, add a plan item)

- [ ] Implement `compute_aligned_layout()` for `#repr("aligned", N)`:
  - Reorder fields normally, then enforce `struct.align = max(computed, N)`
  - `round_up(size, new_align)` for trailing padding
  - Validate: N is a power of two (should be checked at parse time; add `debug_assert!(N.is_power_of_two())`)
  - Must NOT combine with `#repr("packed")` or `#repr("transparent")` — `ReprAttribute` enum is already mutually exclusive by construction (no combined variant exists except `CAligned`)

- [ ] Default behavior (no attribute / `ReprAttribute::Default`):
  - Reorder fields for optimal alignment (§06.1)
  - Field types already narrowed by §04/§05 (stored in `FieldRepr.repr`)
  - Pad for alignment

**Note**: `has_fixed_layout_attr()` in `narrowing/int.rs:201` already checks `C | CAligned | Packed | Transparent` for narrowing skipping. §06 uses `ReprAttribute` directly in `optimize_struct_layout()` (not `has_fixed_layout_attr`) because §06 needs to dispatch to different layout algorithms (C layout, packed layout, etc.) rather than just skip. But the set of "fixed layout" attributes is the same — if `has_fixed_layout_attr` gains new variants, §06's match must stay in sync. Add a `debug_assert!` or comment cross-referencing the two.

**Test strategy for §06.3 (TDD — write failing tests FIRST):**

Tests go in `compiler/ori_repr/src/layout/tests.rs`. Each `#repr` variant gets its own test group.

- [ ] **Write failing Rust unit test matrix BEFORE implementation:**

  Matrix dimensions: **repr attribute** x **struct shape** x **expected property**

  | Test name | Repr attr | Input | Expected | Pin type |
  |---|---|---|---|---|
  | `c_layout_preserves_order` | `C` | `bool, int, bool` | decl order, size 24 | Semantic: no reorder |
  | `c_layout_with_aligned` | `CAligned(16)` | `bool, int` | decl order, align 16, size 16 | Semantic: forced alignment |
  | `packed_no_padding` | `Packed` | `bool, int, bool` | decl order, align 1, size 10 | Semantic: no padding |
  | `packed_alignment_is_one` | `Packed` | `int, int` | align 1 | Invariant |
  | `transparent_single_field` | `Transparent` | `int` | size 8, align 8 (same as inner) | Semantic: zero overhead |
  | `transparent_with_zst` | `Transparent` | `int, Unit` | size 8 (ZST ignored) | Edge: ZST handling |
  | `transparent_two_non_zst_fails` | `Transparent` | `int, bool` | error / debug_assert | Negative: rejects invalid |
  | `transparent_zero_non_zst_fails` | `Transparent` | `Unit` | error / debug_assert | Negative: rejects invalid |
  | `aligned_increases_alignment` | `Aligned(16)` | `int` | align 16, size 16 | Semantic: forced alignment |
  | `aligned_does_not_decrease` | `Aligned(4)` | `int` | align 8 (max of computed 8, requested 4) | Invariant: max not replace |
  | `default_reorders` | `Default` | `bool, int, bool` | reordered, size 16 | Semantic: default = reorder |

- [ ] Verify all tests FAIL with stub implementations
- [ ] Implement `compute_c_layout()`, `compute_packed_layout()`, `compute_transparent_layout()`
- [ ] Verify all tests PASS unchanged
- [ ] **Negative pins**: `transparent_two_non_zst_fails` and `transparent_zero_non_zst_fails` prove the compiler rejects invalid `#repr("transparent")`

**Done criteria for §06.3:**
- `compute_c_layout()`, `compute_packed_layout()`, `compute_transparent_layout()` implemented
- Unit tests for each repr variant using the matrix above
- `#repr("c") struct { a: bool, b: int, c: bool }` produces size 24 (not 16) in unit test
- `./test-all.sh` green

---

## 06.4 Tuple Layout

**File(s):** `compiler/ori_repr/src/layout/tuple_layout.rs` (new file)

Tuples are anonymous structs. `TupleRepr` has the same shape as `StructRepr` (`elements: Vec<FieldRepr>`, `size`, `align`, `trivial`) — the only difference is the field name `elements` instead of `fields`. Apply the same reordering optimization.

**Current state:** `TupleRepr::to_machine_repr()` in `layout.rs` creates tuples via `compute_field_layout()` in declaration order. §06 will replace this with reordered layout.

- [ ] Implement `optimize_tuple_layout()`:
  - Same algorithm as `reorder_and_layout()` from §06.1 but operating on `TupleRepr.elements`
  - `original_index` is the tuple position (0, 1, 2, ...)
  - No `#repr` attributes apply to tuples (they are always reorderable)

- [ ] Ensure tuple destructuring works with reordered layout:
  - `let (a, b, c) = tuple` → uses original indices, not memory order
  - Codegen translates: `a = struct_gep(tuple_ptr, memory_index(0))` where `memory_index(0)` is looked up via `TupleRepr.elements.iter().position(|e| e.original_index == 0)`
  - Add `TupleRepr::memory_index()` helper (same pattern as `StructRepr::memory_index()` from §06.0)

**Struct update syntax (`{ ...p, x: 10 }`):**
- Struct update desugars to a combination of `ArcInstr::Project` (extract unchanged fields) and `ArcInstr::Construct` (build new struct with updated fields). Both go through the standard codegen paths that §06.0's remapping covers. No additional work needed beyond the remapping in §06.0, but add a specific test verifying struct update works correctly with reordered fields.

**Debug info / source-order preservation:**
- DWARF debug info needs to emit fields in **declaration order** (for debugger display), even though the LLVM struct type has fields in **memory order**. Currently, Ori does not emit DWARF debug info for struct fields (no DI metadata in codegen). When DWARF emission is added in the future, it must use `FieldRepr.original_index` and `FieldRepr.name` to reconstruct declaration order. This is a NOTE for future work, not a §06 deliverable — no DWARF infrastructure exists today.

**Cross-section data flow:**
- §06 reads: `StructRepr` and `TupleRepr` from `ReprPlan` (populated by §01 canonical, narrowed by §04/§05)
- §06 reads: `ReprAttribute` from `ReprPlan::repr_attrs` (populated by §01)
- §06 writes: updated `StructRepr`/`TupleRepr` with reordered fields and computed offsets back into `ReprPlan`
- §07 (Enum Repr) reads: struct field layout for enum variant payloads that are structs — if §06 has reordered the inner struct's fields, §07 sees the reordered layout. This is correct (§07 operates on the `MachineRepr` from `ReprPlan`, which §06 has updated).
- §11 (Collection Specialization) reads: element layout for packed arrays — if the element is a struct, §11 sees the §06-optimized layout. This is correct.
- `ori_llvm` codegen reads: final `StructRepr` with memory-order fields, offsets, and size/align for LLVM struct type construction (via `try_lower_narrowed_aggregate()` in `layout_resolver.rs`).

**Test strategy for §06.4 (TDD — write failing tests FIRST):**

Rust unit tests in `compiler/ori_repr/src/layout/tests.rs`. AOT integration tests in `compiler/ori_llvm/tests/aot/`. Ori spec tests in `tests/spec/types/struct_layout/`.

- [ ] **Write failing Rust unit test matrix BEFORE implementation:**

  | Test name | Input tuple | Expected memory order | Expected size | Pin type |
  |---|---|---|---|---|
  | `tuple_reorder_bool_int_bool` | `(bool, int, bool)` | `int, bool, bool` | 16 | Semantic: same as struct |
  | `tuple_reorder_preserves_original_index` | `(bool, int)` | elements[0].original_index == 1 | 16 | Invariant |
  | `tuple_memory_index_lookup` | `(bool, int)` | `memory_index(0) == 1`, `memory_index(1) == 0` | - | Invariant: accessor works |
  | `tuple_single_element` | `(int,)` | identity | 8 | Edge: no reorder |
  | `tuple_all_same_type` | `(int, int, int)` | preserves order (stable sort) | 24 | Edge: stable sort |

- [ ] **Write failing AOT integration tests** (in `compiler/ori_llvm/tests/aot/` or `tests/spec/types/struct_layout/`):
  - Tuple destructuring: `let (a, b, c) = (true, 42, false)` then `assert_eq(b, 42)` — verifies `.1` maps to correct memory offset via remapping
  - Tuple field access: `t.0`, `t.1`, `t.2` return correct values on a reorderable tuple
  - **Dual-execution parity**: same test runs in both interpreter and AOT, producing identical results

- [ ] Verify unit tests FAIL with current identity layout
- [ ] Implement `optimize_tuple_layout()` and `TupleRepr::memory_index()`
- [ ] Verify all tests PASS unchanged

**Done criteria for §06.4:**
- `optimize_tuple_layout()` implemented in `layout/tuple_layout.rs`
- `TupleRepr::memory_index()` helper exists with unit tests
- `(bool, int, bool)` produces same layout as equivalent struct in unit test
- Tuple destructuring `.0`, `.1`, `.2` returns correct values in AOT test with reordered tuple
- Dual-execution parity verified for tuple tests (interpreter and LLVM produce identical results)
- `./test-all.sh` green

---

## 06.5 Completion Checklist

**Test matrix for §06 (write failing tests FIRST, verify they fail, then implement):**

Tests are primarily Rust unit tests in `compiler/ori_repr/src/layout/tests.rs` (struct layout) and `compiler/ori_llvm/tests/aot/` (codegen verification). Layout can be observed by:
1. Checking `StructRepr.size`, `StructRepr.align`, and `FieldRepr.offset` values directly in Rust unit tests
2. Verifying LLVM IR struct type definitions via `ORI_DUMP_AFTER_LLVM=1` and asserting field order in AOT tests
3. Verifying `struct_gep` indices in codegen output match the remapped memory order

| Struct definition | Expected layout | Semantic pin |
|---|---|---|
| `struct { a: bool, b: int, c: bool }` | 16 bytes: `int` first (offset 0), `bool` fields at offset 8 and 9 | Yes — 16 bytes, not 24 |
| `struct { x: int, y: int }` | 16 bytes (unchanged — already optimal) | Yes — no regression |
| `struct { a: byte, b: byte, c: byte, d: byte, e: int }` | 16 bytes: `int` first at offset 0, bytes at 8-11 | Yes — 12 bytes data + 4 padding = 16 |
| `(bool, int, bool)` tuple | Same layout as equivalent struct | Yes — tuple reorder matches struct |
| `#repr("c") struct { a: bool, b: int, c: bool }` | 24 bytes (declaration order preserved) | Yes — no reorder with `#repr("c")` |
| `#repr("transparent") struct Wrap { inner: int }` | 8 bytes, same alignment as `int` | Yes — no wrapper overhead |
| `#repr("aligned", 16) struct Foo { x: int }` | 16 bytes (8 data + 8 padding), alignment = 16 | Yes — forced alignment |
| `#repr("transparent")` with 2 non-ZST fields | Compile error or debug_assert failure | Yes — validation enforced |
| `#repr("packed")` combined with `#repr("aligned", N)` | Compile error (mutually exclusive — `ReprAttribute` enum prevents) | Yes — incompatible attrs |
| Zero-sized field `()` in struct | No storage contribution, correct offset | Yes — ZST handling |
| Empty struct `struct {}` | 0 bytes, align 1 | Yes — degenerate case |
| Single-field struct `struct { x: int }` | 8 bytes (no reorder possible) | Yes — identity case |
| Generic `Pair<bool> { a: bool, b: int }` | 16 bytes (int first) | Yes — monomorphized reorder |
| Struct update `{ ...p, x: 10 }` with reordered fields | Correct field values after update | Yes — remapping through Project+Construct |
| Derived Eq on reordered struct | `==` returns correct result | Yes — derive codegen remapped |
| Derived Clone on reordered struct | Clone produces identical value | Yes — derive codegen remapped |
| Derived Debug on reordered struct | Debug string shows fields in declaration order | Yes — derive format uses FieldDef names |
| Nested struct `{ inner: Inner, x: int }` where `Inner` is also reordered | Both levels reordered correctly | Yes — transitive layout |
| `#repr("packed") struct { a: bool, b: int, c: bool }` | 10 bytes, align 1, no padding | Yes — packed layout |
| Narrowed fields `{ a: bool, b: i16, c: f32 }` | `f32(4), i16(2), bool(1)` — 8 bytes | Yes — narrowed sizes sort correctly |
| RC field drop `{ flag: bool, name: str, count: int }` | `name` dropped correctly after reorder | Yes — drop remapping |
| Derived Hashable on reordered struct | same hash as manually-constructed equivalent | Yes — derive codegen remapped |

- [ ] Write failing test matrix BEFORE implementation (verify tests fail with current declaration-order layout)
- [ ] `struct { a: bool, b: int, c: bool }` uses 16 bytes not 24 (field storage = 10, rounded to max_align 8 = 16)
- [ ] `struct { x: int, y: int }` uses 16 bytes (no change — already optimal)
- [ ] `(bool, int, bool)` uses same layout as the equivalent struct
- [ ] `#repr("c")` structs use C layout (no reordering)
- [ ] `#repr("transparent")` newtype struct has same size/align as inner field
- [ ] `#repr("aligned", 16)` struct has alignment >= 16 even if fields don't require it
- [ ] `#repr("aligned", N)` combined with `#repr("c")` works correctly (`CAligned` variant)
- [ ] `#repr("transparent")` with >1 non-ZST field produces error
- [ ] `#repr("packed")` combined with `#repr("aligned")` is prevented by `ReprAttribute` enum design
- [ ] **Codegen field-index remapping**: `ArcInstr::Project { field: N }` uses `StructRepr::memory_index(N)` for `struct_gep` — tested by creating a struct with reordered fields, accessing a field, and verifying the correct value is returned (both interpreter and AOT)
- [ ] **Construct remapping**: struct literal `Foo { a: true, b: 42, c: false }` stores fields in memory order, not declaration order
- [ ] Pattern matching on structs works correctly with reordered fields
- [ ] Tuple destructuring works with reordered layout (`.0`, `.1`, `.2` map to correct memory offsets via `TupleRepr::memory_index()`)
- [ ] Add semantic pin test: `struct { a: bool, b: int, c: bool, d: byte }` has `StructRepr` with fields ordered `[int, bool, bool, byte]` (by alignment desc, size desc). Verify `StructRepr.size == 16` and `StructRepr.fields[0].repr == Int { I64 }`. This test can ONLY pass with field reordering enabled.
- [ ] Verify `try_lower_narrowed_aggregate()` in `layout_resolver.rs` correctly uses the reordered `StructRepr.fields` order for LLVM struct type creation (it already iterates `fields` in order — after reordering, this naturally produces the correct LLVM type)
- [ ] **[GAP]** `layout_resolver.rs:375-396` `resolve_struct()` creates LLVM struct types from Pool fields (declaration order), NOT from `StructRepr.fields` (memory order). After §06, non-narrowed reordered structs (e.g., `struct { a: bool, b: int }` where no field is narrowed but fields ARE reordered) will NOT go through `try_lower_narrowed_aggregate()` (which requires at least one narrowed field). They will use `resolve_struct()` which creates the LLVM struct in declaration order — violating the §06 memory layout. **Fix**: Modify `try_lower_narrowed_aggregate()` to also trigger when the `StructRepr.fields` order differs from declaration order (i.e., `fields.iter().enumerate().any(|(i, f)| f.original_index != i as u32)`). Or: add a separate check in `resolve_inner()` that redirects all reordered structs (even non-narrowed) to use `StructRepr.fields` for LLVM type creation. This is the most architecturally dangerous item in §06.
- [ ] **Derive codegen remapping**: derived `Eq` on `struct { a: bool, b: int, c: bool }` compares fields correctly (test: two structs differ only in `c`, equality check must detect the difference even though `c` is at a different memory offset than declaration index 2)
- [ ] **Derive codegen remapping**: derived `Clone` on reordered struct produces an identical copy (round-trip: construct → clone → field access → verify all values)
- [ ] **Derive codegen remapping**: derived `Debug` on reordered struct emits fields in declaration order with correct values (not memory order)
- [ ] **Derive codegen remapping**: derived `Hashable` on reordered struct produces same hash as equivalent manually-constructed struct
- [ ] **Struct update syntax**: `let p2 = { ...p, x: 10 }` with reordered struct produces correct field values for both updated and non-updated fields
- [ ] **Drop function remapping**: struct with RC field (e.g., `struct { flag: bool, name: str, count: int }`) — `name` field is correctly dropped after §06 reorders `int` before `str` before `bool` in memory
- [ ] **Narrowing + layout interaction**: `struct { a: bool, b: int, c: float }` where `b` is narrowed to `i16` and `c` is narrowed to `f32` — layout should be `{ f32(4 bytes), i16(2 bytes), bool(1 byte) }` padded, not `{ i16, f32, bool }`. Verify the sorting is correct with narrowed sizes.
- [ ] **Empty struct**: `struct {}` has size 0 and align 1 — no panic, no OOB
- [ ] **Single-field struct**: `struct { x: int }` has size 8 and align 8 — layout unchanged
- [ ] **Pipeline integration**: `compute_struct_layouts()` in `pipeline.rs` iterates all struct/tuple `MachineRepr` entries in `ReprPlan`, applies `optimize_struct_layout()`/`optimize_tuple_layout()`, and writes back. Verify by checking `ReprPlan.repr(struct_idx)` returns the reordered layout after pipeline runs.
- [ ] **[BLOAT]** `layout_resolver.rs` is at 490 lines (limit: 500). The §06 changes to `try_lower_narrowed_aggregate()` or `resolve_inner()` will likely push it over. Extract `try_lower_narrowed_aggregate()` and its helpers to a separate `narrowed_layout.rs` submodule before modifying.
- [ ] `./test-all.sh` green in both debug (`cargo b`) and release (`cargo b --release`) builds
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean
- [ ] Interpreter and LLVM produce identical results for all struct access / tuple destructuring tests (dual-execution parity)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

- [ ] **Negative pin tests** (CLAUDE.md requirement — at least one test that REJECTS old/broken behavior):
  - `struct { a: bool, b: int, c: bool }` does NOT have size 24 (the unoptimized size) — `assert_ne!(struct_repr.size, 24)`
  - `#repr("c") struct { a: bool, b: int, c: bool }` does NOT have size 16 (the optimized size) — `assert_ne!(struct_repr.size, 16)`
  - `#repr("transparent")` with 2 non-ZST fields is rejected (not silently accepted)
- [ ] **`ORI_CHECK_LEAKS=1` verification**: since §06 touches drop function codegen (field remapping in `DropFunctionGenerator`), run `ORI_CHECK_LEAKS=1` on all spec tests involving structs with RC fields (str, [int], nested structs with RC). Zero leaks required.
- [ ] **Plan annotation cleanup**: Remove all `§06`-prefixed code comments from production source files touched by this section. Verify with: `grep -r '§06' compiler/ori_repr/src/ compiler/ori_llvm/src/ --include='*.rs'`. Only spec references (`Spec: Clause N.M`) should remain.
- [ ] **Ori spec tests**: Add `.ori` spec tests under `tests/spec/types/struct_layout/` that exercise the full pipeline (parse -> typeck -> ARC -> codegen -> execution). Minimum spec test matrix:
  - Struct field access on a reorderable struct (e.g., `{ a: bool, b: int }` — access `b`, verify value)
  - Pattern matching on reordered struct fields
  - Struct update syntax with reordered fields
  - Derived traits (Eq, Clone, Debug) on reordered structs
  - Tuple destructuring on reorderable tuples
  - `#repr("c")` struct field access (verify C layout correctness)
  - Each spec test must run in both interpreter (`ori run`) and AOT (`ori build` + execute) for dual-execution parity

**Exit Criteria (all must be measurably true):**
- `StructRepr.size` for `struct { a: bool, b: int, c: bool, d: byte }` is 16 bytes (i64 at offset 0, then i8+i8+i8 at offsets 8-10, then 5 bytes trailing padding to align 8), verified in both Rust unit tests and LLVM IR
- All struct-related spec tests pass in both debug and release builds
- Codegen correctly remaps declaration-order field indices to memory-order indices via `StructRepr::memory_index()`
- Layout is deterministic (stable sort — identical input always produces identical output)
- `#repr("c")` structs are unaffected (declaration order preserved, size matches C ABI)
- Interpreter and LLVM produce identical results for ALL new test files (dual-execution parity)
- `ORI_CHECK_LEAKS=1` reports zero leaks on all spec tests with RC-containing structs
- `/tpr-review` passed with no critical or major unresolved findings

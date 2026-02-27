---
section: "06"
title: "Struct & Tuple Layout Optimization"
status: not-started
goal: "Reorder struct fields for optimal alignment and minimal padding, then record the layout in ReprPlan for codegen"
inspired_by:
  - "Rust repr(Rust) layout algorithm (compiler/rustc_abi/src/layout.rs)"
  - "Zig struct layout optimization (src/Type.zig)"
  - "LLVM DataLayout (lib/IR/DataLayout.cpp)"
depends_on: ["04", "05"]
sections:
  - id: "06.1"
    title: "Field Reordering Algorithm"
    status: not-started
  - id: "06.2"
    title: "Padding Minimization"
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

**Status:** Not Started
**Goal:** Structs and tuples use the minimum possible memory by reordering fields to eliminate padding. A struct `{ a: bool, b: int, c: bool }` currently uses 24 bytes (1 + 7 padding + 8 + 1 + 7 padding); after optimization it uses 10 bytes (8 + 1 + 1 + 0 padding) by putting the `int` first.

**Context:** The spec (§22) explicitly permits struct field reordering: "Struct field order in memory may differ from declaration order." This is a non-guarantee. Rust's `repr(Rust)` does exactly this — it reorders fields by alignment to minimize padding. Ori should do the same.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: Fields sorted by descending alignment, then by descending size
- **Zig** `src/Type.zig`: ABI-optimal layout with explicit alignment control
- **C/C++**: No reordering (declaration order = memory order) — this is why `#pragma pack` exists

**Depends on:** §04, §05 (need to know narrowed field sizes before computing layout).

---

## 06.1 Field Reordering Algorithm

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs`

- [ ] Implement the field reordering algorithm:
  ```rust
  pub fn optimize_struct_layout(
      fields: &[FieldInfo],
      repr_plan: &ReprPlan,
  ) -> StructRepr {
      // Step 1: Compute size and alignment for each field
      let mut field_sizes: Vec<(usize, u32, u32)> = fields.iter()
          .enumerate()
          .map(|(i, f)| {
              let repr = repr_plan.get_repr(f.type_idx);
              let size = repr.size();
              let align = repr.alignment();
              (i, size, align)
          })
          .collect();

      // Step 2: Sort by descending alignment, then descending size
      // (This minimizes padding — largest alignment first fills
      // naturally, smaller fields pack into trailing space)
      field_sizes.sort_by(|a, b| {
          b.2.cmp(&a.2)  // alignment descending
              .then(b.1.cmp(&a.1))  // size descending
      });

      // Step 3: Compute offsets
      let mut offset = 0u32;
      let mut max_align = 1u32;
      let mut layout_fields = Vec::with_capacity(fields.len());

      for &(original_index, size, align) in &field_sizes {
          // Align offset
          offset = (offset + align - 1) & !(align - 1);
          layout_fields.push(FieldRepr {
              original_index: original_index as u32,
              offset,
              repr: repr_plan.get_repr(fields[original_index].type_idx).clone(),
          });
          offset += size;
          max_align = max_align.max(align);
      }

      // Step 4: Add trailing padding for array alignment
      let total_size = (offset + max_align - 1) & !(max_align - 1);

      StructRepr {
          fields: layout_fields,
          size: total_size,
          align: max_align,
          trivial: fields.iter().all(|f| repr_plan.is_trivial(f.type_idx)),
      }
  }
  ```

- [ ] Handle zero-sized fields (unit, never):
  - Zero-sized fields contribute 0 bytes and 1-byte alignment
  - They still get an offset (for field access codegen) but no storage

---

## 06.2 Padding Minimization

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs`

- [ ] Track padding bytes per struct and emit a tracing warning when padding exceeds 25% of total size:
  ```rust
  let padding = total_size - fields.iter().map(|f| f.repr.size()).sum::<u32>();
  if padding > total_size / 4 {
      tracing::warn!(
          struct_name = %name,
          total_size,
          padding,
          "Struct has >25% padding despite field reordering"
      );
  }
  ```

- [ ] Consider field packing for small fields:
  - Multiple `bool` fields → pack into a single byte (bitfield)
  - Multiple `byte` fields → pack contiguously (no alignment waste)
  - `Ordering` (3 values) + `bool` (2 values) → pack into single byte

---

## 06.3 ABI-Stable Opt-Out

**File(s):** `compiler/ori_repr/src/layout/struct_layout.rs`

For FFI interop, users need control over memory layout.

- [ ] Support `#[repr(C)]` attribute:
  - Fields in declaration order (C struct layout rules)
  - Platform-specific alignment (matches target C ABI)
  - No field reordering, no narrowing of field types

- [ ] Support `#[repr(packed)]` attribute:
  - No padding between fields
  - Alignment = 1
  - May require unaligned loads (performance cost)

- [ ] Default behavior (no attribute):
  - Reorder fields for optimal alignment
  - Narrow field types based on range analysis
  - Pad for alignment

---

## 06.4 Tuple Layout

**File(s):** `compiler/ori_repr/src/layout/tuple_layout.rs`

Tuples are anonymous structs. Apply the same optimization.

- [ ] Implement `optimize_tuple_layout()`:
  - Same algorithm as struct layout
  - Original index is the tuple position (0, 1, 2, ...)
  - Codegen must remap `.0`, `.1`, `.2` to the reordered offsets

- [ ] Ensure tuple destructuring works with reordered layout:
  - `let (a, b, c) = tuple` → uses original indices, not memory order
  - Codegen translates: `a = GEP(tuple, layout.field_at_original(0).offset)`

---

## 06.5 Completion Checklist

- [ ] `struct { a: bool, b: int, c: bool }` uses 10 bytes not 24
- [ ] `struct { x: int, y: int }` uses 16 bytes (no change — already optimal)
- [ ] `(bool, int, bool)` uses same layout as the equivalent struct
- [ ] `#[repr(C)]` structs use C layout (no reordering)
- [ ] Field access codegen uses correct offsets from `StructRepr`
- [ ] Pattern matching on structs works correctly with reordered fields
- [ ] `./test-all.sh` green
- [ ] `./scripts/valgrind-aot.sh` clean

**Exit Criteria:** `sizeof` for `struct { a: bool, b: int, c: bool, d: byte }` is 12 bytes (i64 + i8 + i8 + 2 padding) instead of 32 bytes, verified in LLVM IR. All struct-related spec tests pass.

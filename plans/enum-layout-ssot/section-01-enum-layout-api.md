---
section: "01"
title: "EnumLayoutInfo API"
status: not-started
reviewed: false
goal: "Create a canonical EnumLayoutInfo query surface in ori_repr that answers ALL enum layout questions — tag encoding, payload sizing, field offsets, ABI size — eliminating 8+ scattered packing computations"
inspired_by:
  - "Rust rustc_abi::layout::LayoutS — unified layout query struct"
  - "Zig Type.zig — centralized type layout computation"
  - "Swift GenEnum.cpp — single enum layout strategy dispatch"
depends_on: []
success_criteria:
  - "EnumLayoutInfo struct exists in ori_repr/src/layout/ with fields: tag_encoding, tag_gep_index, payload_gep_index, payload_field_offsets, abi_size"
  - "compute_enum_layout_info() produces correct EnumLayoutInfo for ALL EnumTag variants: Explicit, Niche, TaggedPtr, None"
  - "i64-slot packing rule (div_ceil(8)*8) exists in EXACTLY ONE function: slot_padded_size() in layout/mod.rs"
  - "All existing layout tests pass unchanged — no behavioral change"
  - "Rust unit tests cover every EnumTag variant with boundary-value field sizes"
sections:
  - id: "01.1"
    title: "EnumLayoutInfo Struct & Core Queries"
    status: not-started
  - id: "01.2"
    title: "Packing Rule Centralization"
    status: not-started
  - id: "01.3"
    title: "Completion Checklist"
    status: not-started
third_party_review:
  status: none
  updated: null
---

# Section 01: EnumLayoutInfo API

**Context:** Enum layout knowledge is currently scattered across 88+ consumer sites in 13+ files. The i64-slot packing rule (`size.div_ceil(8) * 8`) alone is duplicated in 8+ locations: `ori_repr/src/layout/mod.rs:198`, `ori_llvm/codegen/type_info/enum_layout.rs:100,117`, `ori_llvm/codegen/abi/mod.rs:178,268`, `ori_llvm/codegen/arc_emitter/drop_enum.rs:380`, `ori_llvm/codegen/derive_codegen/enum_bodies/enum_eq.rs:183,264`, `enum_comparable.rs:207`, `enum_hashable.rs:166`. This section creates the canonical home.

**Approach:** Define `EnumLayoutInfo` in `ori_repr/src/layout/` as the SSOT for ALL enum layout facts. Every consumer reads from this struct instead of computing layout independently. The struct answers every question a consumer could ask: tag encoding strategy, GEP indices for tag and payload, field offsets within payload, ABI size, and whether a given field index refers to tag or payload.

---

## 01.1 EnumLayoutInfo Struct & Core Queries

**File(s):** `compiler/ori_repr/src/layout/mod.rs` (extend existing module), `compiler/ori_repr/src/layout/enum_layout_info.rs` (new, ~120 lines)

- [ ] **Define `EnumLayoutInfo` struct** in `compiler/ori_repr/src/layout/enum_layout_info.rs`:
  ```rust
  /// Canonical source of truth for enum layout facts.
  /// Every codegen consumer reads from this struct — no consumer
  /// computes layout independently. Phase C decision (post-narrowing).
  #[derive(Clone, Debug, PartialEq)]
  pub struct EnumLayoutInfo {
      /// Tag encoding strategy (Explicit, Niche, TaggedPtr, None)
      pub tag_encoding: EnumTag,
      /// Number of variants (needed for tag value range)
      pub variant_count: usize,
      /// GEP index for the tag field (None for Niche/TaggedPtr/None encodings)
      pub tag_gep_index: Option<u32>,
      /// GEP index for the payload field (None for all-unit enums)
      pub payload_gep_index: Option<u32>,
      /// Field offsets within the payload struct (i64-slot boundaries)
      /// Indexed by variant field index (0-based within the variant)
      pub payload_field_offsets: Vec<u32>,
      /// Total ABI size in bytes
      pub abi_size: u64,
      /// Payload i64-slot count (0 for all-unit enums)
      pub payload_slot_count: u64,
      /// Whether this is an Option/Result with runtime ABI contract
      pub is_runtime_abi: bool,
  }
  ```
  File size target: ~120 lines (struct + impl + query methods). Under the 500-line limit.

- [ ] **Add query methods** on `EnumLayoutInfo`:
  - `has_explicit_tag() -> bool` — true for `EnumTag::Explicit`
  - `has_niche() -> bool` — true for `EnumTag::Niche { .. }`
  - `is_tagged_ptr() -> bool` — true for `EnumTag::TaggedPtr`
  - `is_tagless() -> bool` — true for `EnumTag::None` (single-variant newtype)
  - `field_is_tag(field_index: u32) -> bool` — answers the question `is_take_project` currently hardcodes as `*field == 0`
  - `payload_slot_offset(variant_field_idx: usize) -> u64` — replaces inline `div_ceil(8)` calculations in derive codegen

- [ ] **Add `compute_enum_layout_info()` function** that produces `EnumLayoutInfo` from `EnumRepr` + `MachineRepr`. This function is the SINGLE computation point — it replaces the scattered layout logic in `enum_layout.rs`, `abi/mod.rs`, `drop_enum.rs`, and the derive walkers. Must handle all four `EnumTag` variants:
  - `Explicit { width }` → tag at GEP 0 (width from `min_tag_width`), payload at GEP 1
  - `Niche { field_index, niche_value, niche_variant_idx }` → no explicit tag, payload occupies the full struct
  - `TaggedPtr` → tag in low 3 bits of pointer, no struct fields
  - `None` → single variant, no tag, payload = the type itself

- [ ] **Wire into `ReprPlan`** — add `enum_layout_info(idx: Idx) -> Option<&EnumLayoutInfo>` query on `ReprPlan`. Computed lazily (or eagerly during `populate_canonical()`) from the existing `EnumRepr` in `MachineRepr::Enum`. Consumers call this query instead of computing layout ad-hoc.

- [ ] **Rust unit tests** (`compiler/ori_repr/src/layout/enum_layout_info/tests.rs`):
  - `compute_enum_layout_info` for all-unit enum (4 variants) → tag at GEP 0, no payload, abi_size = 1
  - `compute_enum_layout_info` for Option<int> explicit → tag at GEP 0, payload at GEP 1, abi_size = 16
  - `compute_enum_layout_info` for Option<bool> niche → no tag GEP, payload_gep_index = Some(0), abi_size = 1
  - `compute_enum_layout_info` for tagged-pointer enum → no struct GEPs, abi_size = 8
  - `compute_enum_layout_info` for single-variant newtype → tagless, abi_size = inner type size
  - `field_is_tag` returns true for field 0 on explicit-tag, false for niche/tagged-ptr/none
  - `payload_slot_offset` boundary values: 0-byte field → 0, 1-byte → 0, 8-byte → 0, 9-byte → 8, 24-byte → 0,8,16
  - Negative pin: `compute_enum_layout_info` for `Option<[int]>` does NOT use niche (empty list = null data ptr)

- [ ] **Subsection close-out (01.1)** — Run `/improve-tooling` retrospectively on this subsection's work. Commit improvements separately via `/commit-push`.

---

## 01.2 Packing Rule Centralization

**File(s):** `compiler/ori_repr/src/layout/mod.rs` (modify existing `compute_enum_payload_layout()`)

- [ ] **Extract `slot_padded_size(field_bytes: u64) -> u64` function** in `layout/mod.rs`:
  ```rust
  /// Canonical i64-slot padding rule. A field of `field_bytes` bytes
  /// occupies ceil(field_bytes / 8) i64 slots (minimum 1 slot for non-zero fields).
  /// This is the SINGLE source of truth — no consumer computes this independently.
  pub fn slot_padded_size(field_bytes: u64) -> u64 {
      if field_bytes == 0 { return 0; }
      field_bytes.div_ceil(8) * 8
  }
  ```
  This replaces the 8+ scattered `div_ceil(8) * 8` computations. The name `slot_padded_size` is chosen to be searchable and self-documenting.

- [ ] **Update `compute_enum_payload_layout()`** to use `slot_padded_size()` internally instead of inline arithmetic.

- [ ] **Add `slot_count(field_bytes: u64) -> u64` helper** — returns `field_bytes.div_ceil(8).max(1)` for non-zero fields, used by derive walkers that need slot COUNT (not byte size). This is the second half of the scattered rule.

- [ ] **Export both functions** from `ori_repr::layout` — they will be consumed by §02 when migrating ori_llvm consumers.

- [ ] **Rust unit tests**: boundary-value tests for `slot_padded_size`: 0→0, 1→8, 7→8, 8→8, 9→16, 16→16, 24→24, 25→32. And `slot_count`: 0→0, 1→1, 8→1, 9→2, 16→2, 24→3.

- [ ] **Subsection close-out (01.2)** — Run `/improve-tooling` retrospectively. Commit improvements separately.

---

## 01.3 Completion Checklist

- [ ] `EnumLayoutInfo` struct defined with all fields and query methods
- [ ] `compute_enum_layout_info()` handles all four `EnumTag` variants
- [ ] `slot_padded_size()` and `slot_count()` are the SINGLE source of truth for packing
- [ ] `ReprPlan::enum_layout_info()` query wired and tested
- [ ] All existing `./test-all.sh` tests pass — no behavioral change (the API is additive; consumers are migrated in §02)
- [ ] `./clippy-all.sh` green
- [ ] `/tpr-review` passed — independent review found no critical or major issues
- [ ] `/impl-hygiene-review` passed — MUST run AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` section-close sweep completed

---

## 01.R Third Party Review Findings

- None.

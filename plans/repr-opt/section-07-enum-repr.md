---
section: "07"
title: "Enum Representation Optimization"
status: in-progress
reviewed: true
third_party_review:
  status: resolved
  updated: 2026-03-31
goal: "Optimize enum layout with niche filling, discriminant narrowing, tagged pointers, and payload compression — matching Rust's enum layout optimizations"
inspired_by:
  - "Rust niche optimization (compiler/rustc_abi/src/layout.rs, Niche struct)"
  - "Rust Option<&T> = pointer with null niche (compiler/rustc_abi/src/lib.rs)"
  - "Swift enum layout (lib/IRGen/GenEnum.cpp)"
  - "Zig optional representation (src/Type.zig)"
depends_on: ["04", "05"]
sections:
  - id: "07.0"
    title: "Prerequisites: Codegen Consumer Inventory"
    status: complete
  - id: "07.1"
    title: "Discriminant Narrowing"
    status: in-progress
  - id: "07.2"
    title: "Niche Filling"
    status: in-progress
  - id: "07.3"
    title: "Tagged Pointers"
    status: not-started
  - id: "07.4"
    title: "Payload Compression"
    status: not-started
  - id: "07.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Enum Representation Optimization

**Context:** Today, Ori enums use `{i64 tag, [M x i64] payload}` — every enum has a full i64 tag plus the maximum variant payload (padded to i64 word size). This wastes memory:
- `Option<int>`: 16 bytes (i64 tag + i64 value) → could be 8 bytes via niche or tagged pointer
- `Option<bool>`: 16 bytes (i64 tag + padded i1 value) → could be 1 byte (value 2 = None)
- `Option<str>`: 32 bytes (i64 tag + 24-byte str payload) → could be 24 bytes (null ptr = None)
- `Option<[int]>`: 32 bytes (i64 tag + 24-byte list payload) → remains 32 bytes (empty lists use null data ptr — no niche available)
- All-unit enum with N variants: 8 bytes (i64 tag only, no payload) → could be 1 byte (i8 tag)

Rust's niche optimization is the gold standard. We study and match it.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: `Niche` struct with `available()`, `reserve()` — tracks invalid bit patterns per type
- **Rust** `compiler/rustc_abi/src/lib.rs`: `NaiveLayout`, `LayoutData`, `Variants::Multiple`
- **Swift** `lib/IRGen/GenEnum.cpp`: Multi-payload enum layout with spare bits analysis

**Depends on:** §04 (narrowed integer types create new niches — e.g., a narrowed `i8` field with range `[0, 2]` has 253 unused values as niches), §05 (float-narrowed fields may have niche patterns — e.g., `f32` has NaN spare bits usable for Option optimization, though the value must be checked carefully against IEEE 754 NaN semantics before use).

**Terminology:** This section uses "tag" and "discriminant" with the following distinction: the **discriminant** is the logical variant index (0, 1, 2, ...) that identifies which variant an enum value holds. The **tag** is the physical encoding of the discriminant in memory — this may be an explicit integer field (`EnumTag::Explicit`), a niche value in an existing field (`EnumTag::Niche`), bits stolen from a pointer (`EnumTag::Tagged`), or absent entirely (`EnumTag::None` for single-variant enums). `TagAccess` abstracts over all tag encodings; it loads/stores discriminants regardless of how the tag is physically encoded.

---

## 07.0 Prerequisites: Codegen Consumer Inventory

**Context:** Changing the enum tag from i64 to a narrowed width (i8/i16) or a niche encoding requires updating EVERY codegen consumer that reads/writes enum tags or accesses variant payloads. Missing any one consumer causes silent data corruption. This is the same coordination problem as §06 field reordering.

**Codegen consumers that emit/read enum tags (ALL must be updated for §07):**

1. **`layout_resolver.rs` → `resolve_enum()`** — constructs the LLVM struct type. Currently `{i64 tag, [M x i64] payload}`. Must emit narrowed tag type (i8/i16/i32) for discriminant narrowing, and entirely different layouts for niche-optimized enums.
2. **`arc_emitter/construction.rs` → `emit_construct()`** — stores tag via `const_i64(variant)`. Must use narrowed constant width and niche-based construction for niche-optimized enums.
3. **`arc_emitter/instr_dispatch.rs` → `ArcInstr::SetTag`** — stores tag via `const_i64(*tag)` + GEP to field 0. Must use narrowed width and write to niche location for niche-optimized enums.
4. **`arc_emitter/instr_dispatch.rs` → `ArcInstr::Project { field: 0 }`** — extracts tag as i64 from field 0. Must extract narrowed-width tag and decode niche.
5. **`arc_emitter/instr_dispatch.rs` → `ArcTerminator::Switch`** — emits LLVM switch on i64 scrutinee. Must switch on narrowed tag or niche-decoded discriminant.
6. **`arc_emitter/drop_enum.rs` → `emit_enum_drop()`** — loads i64 tag at field 0, switches on it, drops per-variant fields at i64-slot offsets. Must use narrowed tag and correct payload offsets.
7. **`arc_emitter/rc_helpers.rs` → `emit_inline_enum_inc/dec()`** — loads i64 tag, switches per-variant for field RC ops. Must use narrowed tag.
8. **`arc_emitter/rc_value_traversal.rs` → `emit_inline_enum_inc/dec()`** — same pattern as rc_helpers.
9. **`arc_emitter/builtins/option_result.rs`** — Option/Result-specific builtins hardcode `{i64 tag, T payload}` layout. Must handle niche-optimized layout.
10. **`arc_emitter/builtins/compound_traits.rs`** — Eq/Comparable/Debug etc. for Option/Result hardcode tag-first layout.
11. **`arc_emitter/builtins/compound_type_impls.rs`** — clone/hash/etc. for Option/Result.
12. **`arc_emitter/builtins/iterator_consumers.rs`** — constructs Option from iterator next.
13. **`arc_emitter/builtins/collections/list_builtins.rs`** — `first()`/`last()` return Option with hardcoded layout.
14. **`arc_emitter/variant_construction.rs`** — variant construction for Option/Result/Enum.
15. **`codegen/abi/mod.rs`** — ABI size computation references `{i64 tag, payload}`.
16. **`arc_emitter/operators/strategy.rs` → `emit_coalesce()`** — `??` operator extracts tag via `extract_value(lhs, 0, "coal.tag")` + compares with `const_i64(0)`. Must use TagAccess for niche-aware tag comparison.

**Strategy:** Introduce a `TagAccess` helper in `ori_llvm` that encapsulates tag read/write/switch for a given `EnumRepr`. Codegen consumers call `TagAccess::load_discriminant()`, `TagAccess::store_tag()`, `TagAccess::emit_switch()` instead of hardcoding i64 GEP+load+switch. This localizes the tag encoding change to one place.

```rust
/// Encapsulates all tag encoding/decoding for a given enum layout.
/// Lives in `ori_llvm::codegen::arc_emitter::tag_access.rs`.
pub struct TagAccess<'a, 'll> {
    enum_repr: &'a EnumRepr,
    builder: &'a IrBuilder<'ll>,
}

impl<'a, 'll> TagAccess<'a, 'll> {
    /// Load the discriminant value from an enum pointer.
    /// For Explicit tags: GEP to field 0, load with narrowed width.
    /// For Niche tags: load the niche field, decode to discriminant.
    /// For None: returns a constant 0 (single-variant).
    pub fn load_discriminant(&self, enum_ptr: ValueId) -> ValueId;

    /// Store a tag for a given variant index.
    /// For Explicit: store narrowed-width constant at field 0.
    /// For Niche: store the niche value if this is the niche variant,
    ///            otherwise the payload write implicitly sets the tag.
    /// For None: no-op.
    pub fn store_tag(&self, enum_ptr: ValueId, variant_idx: u32);

    /// Emit an LLVM switch on the discriminant.
    /// For Explicit: switch on narrowed-width tag.
    /// For Niche: compare niche field against niche value, branch.
    /// For None: unconditional branch to the single variant.
    pub fn emit_switch(
        &self, enum_ptr: ValueId,
        cases: &[(u32, BlockId)],
        default: BlockId,
    );

    /// Get the LLVM type for the tag (i8/i16/i32/i64 or none).
    pub fn tag_llvm_type(&self) -> Option<BasicTypeEnum<'ll>>;

    /// Get the GEP offset to the payload for a given variant.
    /// For Explicit tags: always after the tag field.
    /// For Niche tags: offset 0 (payload IS the entire value).
    pub fn payload_offset(&self, variant_idx: u32) -> u32;
}
```
**ABI module stale comments (`abi/mod.rs`):** The `abi_size_inner()` function in `codegen/abi/mod.rs` has comments saying "1 byte tag" but the actual LLVM layout uses i64. The comments and size computation must be updated alongside the tag width change in §07.1. This is consumer #16 (not listed in the original inventory).

**`TagAccess` data source:** `ArcIrEmitter` already has `repr_plan: Option<&ReprPlan>` (line 214 in `arc_emitter/mod.rs`). `TagAccess` obtains `EnumRepr` via `repr_plan.get_repr(type_idx)` → match `MachineRepr::Enum(e)` → use `e.tag` to determine encoding strategy. For types without a `ReprPlan` entry (pre-§07 or when repr_plan is None), fall back to `EnumTag::Explicit { width: IntWidth::I64 }` (the current default). This fallback path ensures backward compatibility during incremental migration.

**Evaluator is unaffected:** `ori_eval` uses `Value::Variant { tag, fields, ... }` — a Rust-native enum representation with no concept of machine layout, niches, or tag widths. All §07 optimizations are LLVM-only. The evaluator does not need any changes, and dual-execution parity tests (interpreter vs LLVM) verify that the optimized layout produces identical observable behavior.

- [x] Audit ALL codegen consumers listed above and verify completeness. Found 16 direct consumers (items 1-16). Consumer #16 (`operators/strategy.rs` → `emit_coalesce()`) was discovered during audit — `??` operator extracts tag via `extract_value(lhs, 0)`. Note: `terminators.rs` `Switch` already adapts to scrutinee width via `const_int_matching()`. `compound_traits.rs` delegates to `compound_type_impls.rs` (indirect). `rc_value_traversal.rs` delegates to `rc_helpers.rs` (indirect). (2026-03-30)
- [x] **[BUG]** `abi/mod.rs:165-182` — `abi_size_inner` for `TypeInfo::Enum` returned `1` for all-unit enums but actual LLVM layout is `{ i64 }` = 8 bytes. Fixed: all-unit enum size now returns `8`. Stale "1 byte tag" comments replaced with "i64 tag". Regression test `all_unit_enum_abi_size_is_tag_size` added. (2026-03-30)
- [x] Design `TagAccess` abstraction in `compiler/ori_llvm/src/codegen/arc_emitter/tag_access/mod.rs` (~150 lines). `TagEncoding` struct with pure encoding logic for Explicit/Niche/None tags. 11 methods: `from_enum_repr`, `new`, `tag_width`, `tag_gep_index`, `variant_to_tag_value`, `payload_gep_index`, `needs_tag_store`, `is_niche`, `is_tagless`, `niche_field_index`, `niche_value`. Wired into `arc_emitter/mod.rs` via `pub(super) mod tag_access;`. (2026-03-30)
- [x] Create empty files for new modules: `compiler/ori_repr/src/layout/niche.rs` (niche analysis) and `compiler/ori_repr/src/layout/tagged_ptr.rs` (tagged pointer analysis). Registered in `layout/mod.rs` via `pub(crate) mod niche;` and `pub(crate) mod tagged_ptr;`. Module docs added. (2026-03-30)
- [x] **ARC IR tag width assumption:** Documented in TagEncoding design. `ArcTerminator::Switch` already adapts to scrutinee width via `const_int_matching()`. `ArcInstr::SetTag` must be updated to use `TagEncoding::variant_to_tag_value()` + narrowed-width constant in §07.1. (2026-03-30)
- [x] Plan incremental migration: discriminant narrowing (§07.1) BEFORE niche filling (§07.2). Subsections already reordered by plan review. (2026-03-30)

**§07.0 Tests (TDD — write before implementation):**

- [x] **Rust unit tests** (`compiler/ori_llvm/src/codegen/arc_emitter/tag_access/tests.rs`): 22 tests covering all 3 EnumTag variants: Explicit {I64/I8/I16} (tag_width, tag_gep_index, variant_to_tag_value, payload_gep_index, needs_tag_store), Niche (field_index, niche_value, niche-vs-non-niche variant semantics), None (tagless, constant 0, no-op store). Plus `from_enum_repr` integration test. All pass. (2026-03-30)
- [x] **ABI bug regression test** (`compiler/ori_llvm/src/codegen/abi/tests.rs`): `all_unit_enum_abi_size_is_tag_size` asserts `abi_size == 8` (not 1). `enum_with_payload_abi_size` asserts `abi_size == 16`. Both pass. (2026-03-30)
- [x] TDD verified: ABI regression test failed before fix (returned 1), passed after fix (returns 8). TagEncoding tests pass on initial implementation. (2026-03-30)

---

## 07.1 Discriminant Narrowing

**File(s):** `compiler/ori_repr/src/enum_repr.rs` (existing — add `min_tag_width()` here near `EnumTag`), `compiler/ori_llvm/src/codegen/arc_emitter/tag_access.rs` (new), `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs` (update `resolve_enum()`), `compiler/ori_repr/src/canonical/type_repr.rs` (update `canonical_enum()`/`canonical_option()`/`canonical_result()`), `compiler/ori_llvm/src/codegen/abi/mod.rs` (fix `abi_size_inner()`)

**Why first:** Discriminant narrowing is the safest starting point because the tag remains an explicit field at offset 0 — only its width changes (i64 -> i8/i16/i32). The layout structure `{tag, payload}` is preserved. This makes it the ideal first consumer of the `TagAccess` abstraction from §07.0, validating the abstraction before niche filling changes the layout structure entirely.

The discriminant (tag) should use the minimum width needed.

- [x] Compute minimum tag width: (2026-03-30)
  ```rust
  pub fn min_tag_width(variant_count: usize) -> IntWidth {
      match variant_count {
          0 | 1 => IntWidth::I8, // single variant or empty → minimal tag (or EnumTag::None)
          n => {
              // Bits needed = ceil(log2(n)), computed without floating point:
              // (n - 1).leading_zeros() counts unused high bits in usize;
              // usize::BITS - leading_zeros = bits needed.
              let bits_needed = usize::BITS - (n - 1).leading_zeros();
              match bits_needed {
                  0..=8 => IntWidth::I8,    // up to 256 variants
                  9..=16 => IntWidth::I16,  // up to 65536 variants
                  17..=32 => IntWidth::I32, // up to 4 billion variants
                  _ => IntWidth::I64,
              }
          }
      }
  }
  ```

- [x] Tag narrowed from i64 to i8 for USER-DEFINED enums with ≤256 variants via `resolve_enum()`. (2026-03-30)
- [ ] **Option/Result tag narrowing** — Option/Result keep i64 tags for `ori_rt` runtime compatibility. <!-- blocked-by:07.5 "ori_rt Option/Result tag narrowing" item -->
- [x] For single-variant enums (newtypes), eliminate tag entirely (`EnumTag::None`) — implemented in §07.2 (canonical_enum emits EnumTag::None when variants.len() == 1, resolve_enum_tagless omits tag field). (2026-03-31)
- [x] Added `min_tag_width()` to `compiler/ori_repr/src/enum_repr.rs` with 7 boundary-value unit tests. (2026-03-30)
- [x] `TagEncoding` abstraction implemented in `tag_access/mod.rs` (§07.0). Consumer migration used `const_int_matching` + `struct_field_type` + `const_int_for_struct_field` helpers instead of full TagAccess LLVM emission — simpler and equally correct. (2026-03-30)
- [x] All 16 codegen consumers migrated from hardcoded `const_i64`/`type_i64` to narrowed tag types. Changes across 15 files: `construction.rs`, `instr_dispatch.rs`, `drop_enum.rs`, `rc_helpers.rs`, `variant_construction.rs`, `option_result.rs`, `compound_type_impls.rs`, `iterator_consumers.rs`, `list_builtins.rs`, `operators/strategy.rs`, `enum_eq.rs`, `enum_comparable.rs`, `enum_hashable.rs`, `abi/mod.rs`, `layout_resolver.rs`. Key helpers added: `IrBuilder::struct_field_type()`, `IrBuilder::const_int_for_struct_field()`, `IrBuilder::const_i16()`, `IrBuilder::i16_type()`. (2026-03-30)
- [x] Updated `resolve_enum()` — uses `min_tag_width(variants.len())` to emit narrowed `i8`/`i16`/`i32`/`i64` tag type. (2026-03-30)
- [x] Updated `abi_size_inner()` — uses `min_tag_width().size_bytes()` for tag size. (2026-03-30)
- [x] Updated `canonical_enum()`, `canonical_option()`, `canonical_result()` — all use `min_tag_width()`. Non-unit enum sizes unchanged (LLVM `[M x i64]` padding absorbs the difference). All-unit enum sizes shrink from 8 to 1. (2026-03-30)
- [x] All-unit enum path preserved: `resolve_enum()` emits `{ i8 }` (no payload array). (2026-03-30)
- [x] **[BLOAT]** `compound_type_impls.rs` (519→4 files): `mod.rs` (15), `option.rs` (102), `result.rs` (246), `str_map.rs` (91), `tuple.rs` (112). All under 500. (2026-03-30)
- [x] **[BLOAT]** `list_builtins.rs` (712→3 files): `mod.rs` (356), `helpers.rs` (157), `sort_thunks.rs` (229). All under 500. (2026-03-30)
- [x] `./test-all.sh` passes: 14,678 tests, 0 failures. Debug and release builds verified. (2026-03-30)

**§07.1 Tests (TDD — write BEFORE implementation, verify they fail):**

- [x] **Rust unit tests**: `min_tag_width` boundary tests (7 tests in `layout/tests.rs`), `canonical_enum` updated to expect I8 tag, `canonical_option_int` updated to expect I8 tag, all-unit enum size = 1, ABI tests updated. 22 TagEncoding tests. All pass. (2026-03-30)
- [x] **Ori spec tests** (`tests/spec/types/sum/test_discriminant_narrowing.ori`) — 12 tests: all-unit enum match, Option int/str match, Result match, for-yield with Option, closure capturing Option, `?` on Result, nested enum match, Option predicates, Result predicates, unwrap_or, coalesce `??`. All pass. (2026-03-30)
- [x] **AOT tests** (`compiler/ori_llvm/tests/aot/enum_discriminant.rs`) — 6 tests: IR inspection (all-unit enum `{ i8 }` type, Option i64 runtime-compat), behavioral (all-unit match, Option match, Result match, RC payload enum). All pass. (2026-03-30)
- [x] **Dual-execution parity**: 14,666 tests pass in both interpreter and LLVM. (2026-03-30)
- [x] **Leak check**: Valgrind 87/90 pass (3 failures are pre-existing COW bugs BUG-05-001). `diagnose-aot.sh` on custom enum test: compilation pass, execution clean, leak check clean. No regressions from discriminant narrowing. (2026-03-30)

---

## 07.2 Niche Filling

**File(s):** `compiler/ori_repr/src/layout/niche.rs` (new, ~200 lines — `Niche` struct, `find_niches()`, `find_enum_niches()`, `optimize_option_repr()`, `optimize_result_repr()`), `compiler/ori_repr/src/enum_repr.rs` (add `EnumTag::Niche` support — already defined), `compiler/ori_repr/src/canonical/type_repr.rs` (update `canonical_option()`/`canonical_result()` to call niche optimization), `compiler/ori_llvm/src/codegen/arc_emitter/tag_access.rs` (extend for niche encoding)

**Depends on:** §07.1 (the `TagAccess` abstraction must be implemented and validated with explicit narrowed tags before niche encoding changes the layout structure)

A "niche" is an invalid bit pattern in a type. If an enum variant's payload has a niche, we can use it to encode a different variant, eliminating the explicit tag.

**Layout boundary note:** Internal runtime representations such as `FatPointer`, `str`, `[T]`, `{K:V}`, `Set<T>`, closures, and ranges are exempt from §06 field reordering. They are represented by dedicated `MachineRepr` / `TypeInfo` variants, not by `MachineRepr::Struct`, so `field_index: 2` on `FatPointer` is stable unless this section explicitly changes that dedicated runtime layout.

- [x] Define `Niche` struct in `compiler/ori_repr/src/layout/niche.rs`. Also extended `EnumTag::Niche` with `niche_variant_idx: u32` to support niche at any variant position (not just last). Updated `TagEncoding` and all tests. (2026-03-31)
  ```rust
  /// A niche (invalid bit pattern) discovered in a type's representation.
  /// Used to eliminate explicit discriminant tags in enum layouts.
  pub struct Niche {
      /// Which field contains the niche (for fat pointers: 2 = data ptr)
      pub field_index: u32,
      /// Byte offset within the field
      pub offset: u32,
      /// Number of available niche values
      pub available: u128,
      /// Starting value of the niche range
      pub start: u128,
  }
  ```

- [x] Identify niches for each type (implemented as `find_niches()` in `niche.rs` — handles Bool, Ordering, Char, RcPointer, FatPointer(Str), nested Enum; conservatively skips Byte, Int, Float, collections): (2026-03-31)
  ```rust
  pub fn find_niches(repr: &MachineRepr) -> Vec<Niche> {
      match repr {
          // bool: values 0 and 1 → niche at value 2..=255 (254 niches)
          MachineRepr::Bool => vec![Niche {
              field_index: 0, offset: 0, available: 254, start: 2,
          }],

          // Ordering: values 0,1,2 → niche at 3..=255 (253 niches)
          // MachineRepr::Ordering is a dedicated variant (NOT Int { I8 })
          MachineRepr::Ordering => vec![Niche {
              field_index: 0, offset: 0, available: 253, start: 3,
          }],

          // Byte: all 256 values valid → no niche (unless range-narrowed)
          MachineRepr::Byte => vec![],

          // Narrowed int i8 with known range [lo, hi] → niche at hi+1..=i8::MAX
          // or lo-1..=i8::MIN (requires range info from ReprPlan)
          MachineRepr::Int { width: IntWidth::I8, .. } => {
              // Must query ReprPlan for the actual value range.
              // Without range info, conservatively return empty.
              // The caller (optimize_enum_repr) passes range info separately.
              vec![]
          }

          // Reference/pointer: null (0) is never a valid heap pointer
          // (ori_rc_alloc guarantees non-null, min 8-byte aligned)
          MachineRepr::RcPointer(_) => vec![Niche {
              field_index: 0, offset: 0, available: 1, start: 0, // null = niche
          }],

          // Fat pointer — ONLY str has a null-ptr niche.
          // str uses SSO for empty strings (OriStr::EMPTY has SSO_FLAG set in
          // byte 23, making the data-pointer-slot always non-zero). Therefore
          // null data pointer (all-zero in bytes 16-23) is an invalid str.
          //
          // [T], {K:V}, Set<T> use {0, 0, null} for empty collections, so
          // null data pointer IS a valid value — NO niche available.
          MachineRepr::FatPointer(FatRepr::Str) => vec![Niche {
              field_index: 2, offset: 0, available: 1, start: 0,
          }],
          MachineRepr::FatPointer(FatRepr::Collection { .. })
          | MachineRepr::FatPointer(FatRepr::Map { .. }) => vec![],

          // Nested enum: if it has unused discriminant values
          MachineRepr::Enum(e) => find_enum_niches(e),

          // Char: 0x110000..=0xFFFFFFFF are invalid Unicode (huge niche space)
          // MachineRepr::Char is a dedicated variant (NOT Int { I32 })
          MachineRepr::Char => vec![Niche {
              field_index: 0, offset: 0,
              available: 0xFFFF_FFFF - 0x10_FFFF, start: 0x11_0000,
          }],

          _ => vec![],
      }
  }
  ```

- [x] Implement `find_enum_niches()` for nested enums: handles Explicit (unused tag values), Niche (remaining capacity after one value consumed), and None (delegates to payload). Verified with `Option<Option<bool>>` → niche value 3. (2026-03-31)

- [x] Implement `optimize_option_repr()` in `niche.rs`. Wired into `canonical_option()` in `type_repr.rs` — delegates fully. Variant order matches type checker (None=0, Some=1). Uses `niche_variant_idx: 0` for None. Falls back to explicit I64 tag for types without niches. (2026-03-31)

- [x] Apply niche to `Result<T, E>` via `optimize_result_repr()` in `niche.rs`. Wired into `canonical_result()` in `type_repr.rs`. Tries Ok's niches first (Err encoded via Ok's niche), then Err's niches. Falls back to explicit I64 tag. (2026-03-31)

- [x] Update `resolve_enum()` in `layout_resolver.rs` to handle `EnumTag::Niche` AND `EnumTag::None`. Refactored into 4 methods: `resolve_enum()` (dispatcher), `resolve_enum_explicit()` (existing `{ tag, payload }`), `resolve_enum_tagless()` (single-variant, payload only), `resolve_enum_niche()` (data variant payload only). Consults `ReprPlan` for tag encoding. (2026-03-31)
- [x] **Single-variant enum (newtype) erasure**: `canonical_enum()` emits `EnumTag::None` when `variants.len() == 1`. The LLVM layout via `resolve_enum_tagless()` omits the tag field. All 14,798 tests pass. (2026-03-31)
- [x] Pattern matching codegen for niche-encoded variants — implemented in `terminators.rs` via `emit_niche_switch()`: loads niche field, compares against niche_value (with `ptrtoint` for pointer niches), conditional branch to niche/data blocks. Project (field 0) in `instr_dispatch.rs` extracts niche field and records in `niche_scrutinees` map. SetTag handles niche/tagless/explicit paths. Gated by `NICHE_CODEGEN_READY` flag. (2026-03-31)

- [x] RC inc/dec for niche-encoded variants — implemented in `rc_helpers.rs` via shared `emit_niche_enum_rc()`: stores to alloca, loads niche field, compares against niche_value, conditionally skips RC for niche variant. Handles both pointer and integer niche fields. (2026-03-31)

- [x] Drop for niche-encoded variants — implemented in `drop_enum.rs` via `emit_drop_enum_niche()`: loads niche field, compares against niche_value, skips to done for niche variant, drops data variant fields at struct offset 0 (no tag field). (2026-03-31)

- [x] **[BUG]** Fixed Option variant ordering mismatch: `canonical_option()` was creating `[None=0, Some=1]` but type checker assigns `[Some=0, None=1]`. This would have caused `niche_variant_idx` to map to the wrong variant. Fixed: `[Some=0, None=1]` everywhere, `niche_variant_idx: 1` for None. (2026-03-31)

- [x] **Codegen consumers updated** — all 4 remaining consumers are niche-aware: (2026-03-31)
  - [x] `option_result.rs` — Option builtins use niche field comparison for `is_some`/`is_none`/`unwrap`/`unwrap_or`; Result builtins use `niche_variant_idx` for `is_ok`/`is_err`
  - [x] `operators/strategy.rs` — `emit_coalesce()` is dead code (BUG-04-009 routes `??` through ARC IR control flow), no changes needed
  - [x] `instr_dispatch.rs` — `try_emit_project_enum_payload()` uses `field - 1` for niche layout
  - [x] `construction.rs` — `emit_niche_variant_construct()` inserts payload at index 0, skips tag for data variant
  - [x] `layout_resolver.rs` — `TypeInfo::Option` and `TypeInfo::Result` check ReprPlan for niche, produce named struct with `{ payload }` layout
  - [x] `niche_is_sentinel()` shared helper eliminates 4 inline ptrtoint+icmp patterns

- [x] **ABI layer niche awareness** — `abi/mod.rs` updated: `ReprPlan` threaded through `abi_size`, `compute_param_passing`, `compute_return_passing`, `compute_function_abi`, `compute_function_abi_with_ownership`. Niche checks added for `TypeInfo::Option`, `TypeInfo::Result`, and `TypeInfo::Enum` (tagless/niche variants). All callers updated (function_compiler, define_phase, arc_emitter, derive_codegen). Also fixed `populate_canonical()` in `ori_repr` to canonicalize types with resolved variable children (was skipping `Option<Var(T→str)>` due to overly aggressive `has_vars()` check). Added `dst_ty` to `BuiltinCtx` for future niche-aware monadic dispatch. `NICHE_CODEGEN_READY` gate remains `false` — flipping it revealed ~154 AOT test failures from 8+ codegen consumers that construct explicit `{ i64, T }` structs. These need niche-aware paths before the gate can be enabled: `result_monadic.rs`, `option_result_monadic.rs`, `compound_type_impls/option.rs`, `compound_type_impls/result.rs`, `list_builtins/helpers.rs`, `map_builtins.rs`. (2026-04-04)

**§07.2 Tests (TDD — write BEFORE implementation, verify they fail):**

- [x] **Rust unit tests** (`compiler/ori_repr/src/layout/tests.rs`): 22 niche tests covering all `find_niches` types (Bool/254, Ordering/253, Char/0x110000, Str/null-ptr, RcPointer/null, Byte/empty, Int/empty, Float/empty, Unit/empty, List/empty), `find_enum_niches` (4-variant i8 → 252), `optimize_option_repr` semantic pins (Bool→1 byte, Ordering→1 byte, Char→4 bytes, Str→24 bytes, RcPointer→8 bytes), negative pins (Int→explicit, List→explicit), nested niche (Option<Option<Bool>>→1 byte with niche 3), and `optimize_result_repr` (Bool×Ordering→niche, Int×Int→explicit). Also 3 new `TagEncoding` tests for `niche_variant_idx: 0`. All pass. (2026-03-31)
- [x] **Ori spec tests** (`tests/spec/types/enum/niche/`): 8 test files, 62 tests total, all passing via interpreter. (2026-04-06)
  - `option_bool.ori`: `Some(true)`, `Some(false)`, `None` match correctly; roundtrip through list, distinctness, predicates, unwrap_or
  - `option_ordering.ori`: all four values (`Some(Less)`, `Some(Equal)`, `Some(Greater)`, `None`) match correctly; all distinct
  - `option_char.ori`: `Some('a')`, `Some('\u{10FFFF}')`, `None` match correctly (boundary: last valid Unicode)
  - `option_str.ori`: `Some("hello")`, `Some("")`, `None` match correctly (empty string uses SSO, not null ptr); `Some("") != None` pin; map
  - `option_list.ori`: `Some([1,2])`, `Some([])`, `None` all distinct (negative pin: no niche — uses len-based verification)
  - `option_option_bool.ori`: all four values of `Option<Option<bool>>` are distinct; nested match
  - `result_niche.ori`: `Result<bool, Ordering>` match with all 5 variant combinations; is_ok/is_err
  - `niche_rc.ori`: `Option<str>` created in loop (RC correctness); shared references; None clone; list of mixed Some/None
  - `niche_cross_feature.ori`: for...yield+match, closure capture Option<bool>, Option.map chaining, filter, and_then, `?` on Result<str, Error>
- [ ] **AOT tests** (`compiler/ori_llvm/tests/aot/enum_niche.rs`): <!-- blocked-by:NICHE_CODEGEN_READY gate (07.5 §ori_rt tag narrowing + 8 codegen consumers) -->
  - LLVM IR inspection: `Option<bool>` compiles to `i8` (not `{ i8, i8 }`)
  - LLVM IR inspection: `Option<str>` compiles to `%ori.str` (not `{ i8, %ori.str }`)
  - LLVM IR inspection: `Option<[int]>` still has explicit tag (negative pin)
  - RC inc/dec for `Option<str>` includes null-ptr check before `ori_str_rc_inc`
- [ ] **Dual-execution parity**: every Ori spec test must produce identical output in interpreter and LLVM <!-- blocked-by:NICHE_CODEGEN_READY gate -->
- [ ] **Leak check**: `ORI_CHECK_LEAKS=1` on all niche spec tests (critical — niche encoding changes RC paths) <!-- blocked-by:NICHE_CODEGEN_READY gate -->
- [ ] **Valgrind**: `./diagnostics/valgrind-aot.sh` on niche-related tests (niche encoding is a memory-safety-sensitive change) <!-- blocked-by:NICHE_CODEGEN_READY gate -->

---

## 07.3 Tagged Pointers

**File(s):** `compiler/ori_repr/src/layout/tagged_ptr.rs` (new, ~100 lines — `can_use_tagged_pointer()`, `is_taggable_pointer()`), `compiler/ori_llvm/src/codegen/arc_emitter/tag_access.rs` (extend `TagAccess` for tagged pointer encoding/decoding)

On 64-bit systems, heap pointers have alignment ≥8, meaning the low 3 bits are always zero. These bits can store a 3-bit tag (up to 8 variants).

- [ ] Implement tagged pointer optimization:
  ```rust
  /// Check if a variant payload is a single-word pointer suitable for tagging.
  ///
  /// FatPointer (str, [T], {K:V}, Set<T>) is 24 bytes — NOT taggable.
  /// Only single-word pointers (RcPointer, OpaquePtr, UnmanagedPtr) qualify.
  fn is_taggable_pointer(repr: &MachineRepr) -> bool {
      matches!(repr,
          MachineRepr::RcPointer(_)
          | MachineRepr::OpaquePtr
          | MachineRepr::UnmanagedPtr
      )
  }

  pub fn can_use_tagged_pointer(enum_repr: &EnumRepr) -> bool {
      // At most 8 variants (3 bits for tag)
      if enum_repr.variants.len() > 8 {
          return false;
      }
      // Every non-unit variant must have exactly one single-word pointer field.
      // FatPointer/Closure/Struct/Tuple are excluded — they are multi-word.
      // The decode path uses `value & ~0x7` to recover the pointer, which
      // would corrupt non-pointer payloads (e.g., masking int(5) gives 0).
      // Unit variants (no fields) are fine — they carry no payload, just a tag.
      enum_repr.variants.iter().all(|v| {
          v.fields.is_empty()
              || (v.fields.len() == 1 && is_taggable_pointer(&v.fields[0]))
      })
      // At least one variant must have a pointer (otherwise no benefit)
      && enum_repr.variants.iter().any(|v| {
          v.fields.len() == 1 && is_taggable_pointer(&v.fields[0])
      })
  }
  ```
  Note: `VariantRepr::is_pointer()` (in `enum_repr.rs`) includes `FatPointer` which is correct for general "is this a pointer type?" queries but NOT correct for tagged pointer optimization. §07.3 uses `is_taggable_pointer()` (single-word only) instead.

- [ ] Tagged pointer layout:
  ```
  [63:3] pointer value  [2:0] tag
  ```
  - Store pointer variant: `ptr | tag` (low 3 bits of ptr are 0 due to alignment)
  - Load tag: `value & 0x7`
  - Load pointer: `value & ~0x7`
  - Unit variants: only the tag value matters, no payload to decode

- [ ] Safety:
  - Only applicable when the runtime guarantees 8-byte aligned allocations (ori_rt already does: alignment is always ≥ 8)
  - Non-pointer scalar payloads (int, bool, float) are **excluded** — their low bits carry data that `& ~0x7` would destroy
  - Future: could support scalar payloads by shifting them left 3 bits during encode and right 3 bits during decode, at the cost of reducing the usable range (61 bits instead of 64)

**§07.3 Tests (TDD — write BEFORE implementation, verify they fail):**

- [ ] **Rust unit tests** (`compiler/ori_repr/src/layout/tests.rs`):
  - `is_taggable_pointer(RcPointer)` = true
  - `is_taggable_pointer(FatPointer(Str))` = false (negative pin — 24 bytes, not single-word)
  - `is_taggable_pointer(FatPointer(Collection))` = false
  - `is_taggable_pointer(Int { I64 })` = false (negative pin — scalar, not pointer)
  - `is_taggable_pointer(Bool)` = false
  - `can_use_tagged_pointer()` for enum with 2 variants (unit + RcPointer) = true
  - `can_use_tagged_pointer()` for enum with 9 variants = false (exceeds 3-bit tag)
  - `can_use_tagged_pointer()` for enum with int payload = false (negative pin)
  - `can_use_tagged_pointer()` for enum with FatPointer payload = false (negative pin — 24 bytes)
  - `can_use_tagged_pointer()` for all-unit enum = false (no pointer, no benefit)
- [ ] **Ori spec tests** (`tests/spec/types/enum/tagged_ptr.ori`):
  - Enum with unit + RcPointer variants: construct, match, RC correct
  - Roundtrip: store tagged pointer in list, retrieve, match — value preserved
  - RC stress: create many tagged-pointer enum values in a loop, verify `ORI_CHECK_LEAKS=1` reports zero
- [ ] **AOT tests** (`compiler/ori_llvm/tests/aot/enum_tagged_ptr.rs`):
  - LLVM IR inspection: tagged pointer enum fits in single `i64` (not `{ i8, ptr }`)
  - Verify tag bits extracted correctly: `value & 0x7` gives correct variant index
  - Verify pointer recovered correctly: `value & ~0x7` gives valid heap pointer
- [ ] **Dual-execution parity**: every spec test produces identical output in interpreter and LLVM
- [ ] **Leak check and Valgrind**: tagged pointer encoding changes how RC pointers are accessed — mandatory memory verification

---

## 07.4 Payload Compression

**File(s):** `compiler/ori_repr/src/canonical/type_repr.rs` (update `canonical_enum()` payload sizing), `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs` (update `resolve_enum()` payload layout), `compiler/ori_llvm/src/codegen/arc_emitter/drop_enum.rs` (update `compute_variant_field_offsets()`)

When variant payloads have different sizes, the current approach uses `max(sizeof(variant))` for all, padded to i64 slot boundaries. §07.4 addresses the achievable payload optimizations.

- [ ] All-unit variant detection (already implemented in `resolve_enum`):
  - Enums where every variant is unit → tag only, no payload
  - `resolve_enum` already omits the payload array when `max_payload_bytes == 0`
  - Verify this path is preserved when changing the tag width from i64 to i8
  - After §07.1, all-unit enums shrink from 8 bytes (i64 tag) to 1 byte (i8 tag)

- [ ] Payload alignment optimization:
  - Current layout pads every field to i64 slot boundary (`size.div_ceil(8) * 8`)
  - With narrowed fields from §04/§05, variant payloads can use tighter packing
  - Example: `type Color = RGB(r: i8, g: i8, b: i8) | HSL(h: i16, s: i8, l: i8)` — RGB payload = 3 bytes (not 24), HSL = 4 bytes (not 24)
  - Must match payload access offsets in `drop_enum.rs:compute_variant_field_offsets()` and `arc_emitter/construction.rs`

- [ ] **Shared prefix optimization (future work — NOT in §07 scope):**
  - Sharing field prefixes across variants requires fundamentally different codegen (shared GEP paths) and complicates pattern matching
  - Defer to a future section when benchmarks show the padding cost is significant
  - Rust does implement this (`Variants::Multiple { offsets }`) but it's one of their most complex codegen paths

- [ ] **Size-class bucketing (future work — NOT in §07 scope):**
  - Heap-allocating large variant payloads requires runtime changes (new allocation paths, drop function changes, RC interaction)
  - The overhead of indirection (extra pointer chase + allocation) often exceeds the memory savings
  - Rust chose NOT to implement this; Swift does (for multi-payload enums with spare bits exhausted)
  - Defer until escape analysis (§08) can determine which enums are stack-only (where boxing hurts) vs heap-only (where boxing helps)

**§07.4 Tests (TDD — write BEFORE implementation, verify they fail):**

- [ ] **Rust unit tests** (`compiler/ori_repr/src/layout/tests.rs`):
  - All-unit 4-variant enum: payload size = 0, total size = tag width (1 byte after §07.1)
  - `Color = RGB(r: i8, g: i8, b: i8) | HSL(h: i16, s: i8, l: i8)`: max payload = 4 bytes (not 24)
  - Enum with one unit variant and one 1-byte payload variant: payload = 1 byte
  - Enum with mixed narrowed fields: payload size = max of all variant payload sizes with correct alignment
- [ ] **Ori spec tests** (`tests/spec/types/enum/payload_compression.ori`):
  - Mixed-size variant enum: construct each variant, match, verify values preserved
  - Narrowed-field enum from §04: field values survive construction + match roundtrip
- [ ] **AOT tests** (`compiler/ori_llvm/tests/aot/enum_payload.rs`):
  - LLVM IR inspection: payload array uses narrowed element types, not `[M x i64]`
  - Verify `compute_variant_field_offsets()` matches actual LLVM struct offsets
- [ ] **Dual-execution parity**: every spec test produces identical output in interpreter and LLVM
- [ ] **Leak check**: `ORI_CHECK_LEAKS=1` on all payload compression spec tests

---

## 07.5 Completion Checklist

**Implementation order:** §07.0 (prerequisites) → §07.1 (discriminant narrowing — safe, validates TagAccess) → §07.2 (niche filling — layout-changing, uses validated TagAccess) → §07.3 (tagged pointers — alternative encoding for pointer-heavy enums) → §07.4 (payload compression — padding reduction). Each subsection must pass `./test-all.sh` before proceeding to the next.

**Test matrix for §07 (write failing tests FIRST, verify they fail, then implement):**

**Phase 1 tests (§07.1 — discriminant narrowing only, no niche filling):**

| Type | Expected after §07.1 | Semantic pin |
|---|---|---|
| All-unit enum `type Dir = North \| South \| East \| West` | `{ i8 tag }` — no payload, tag narrowed from i64 | Yes — `sizeof == 1` (down from 8) |
| `Option<int>` | `{ i8 tag, i64 payload }` — 16 bytes (tag narrowed, padding between i8 tag and i64 payload) | Yes — tag is i8, not i64 |
| `Option<bool>` | `{ i8 tag, i8 payload }` — 2 bytes (or padded to alignment) | Yes — smaller than current 16 bytes |
| Single-variant enum `type Wrapper(val: int)` | `EnumTag::None` — newtype erasure, same as `int` | Yes — `sizeof == 8` |
| Enum with 257 variants | `{ i16 tag, payload }` — tag auto-widens to i16 | Yes — i16 not i8 |

**Phase 2 tests (§07.2 — niche filling, builds on §07.1):**

| Type | Expected after §07.2 | Semantic pin |
|---|---|---|
| `Option<bool>` | 1 byte `i8`: `Some(false)=0`, `Some(true)=1`, `None=2` | Yes — `sizeof == 1`, no struct wrapper |
| `Option<Ordering>` | 1 byte `i8`: `Some(Less)=0`, `Some(Equal)=1`, `Some(Greater)=2`, `None=3` | Yes — `sizeof == 1` |
| `Option<str>` | 24 bytes (null data ptr niche for None, no tag field) | Yes — `sizeof == sizeof(str)` |
| `Option<[int]>` | 32 bytes (i8 tag + 24-byte payload — no niche, empty lists use null ptr) | Yes — `sizeof == 32` (only tag narrowing i64->i8 from §07.1) |
| `Option<int>` | 16 bytes (no niche available in i64 — must use explicit tag, narrowed to i8) | Yes — `sizeof == 16` |
| `Option<char>` | 4 bytes (char niche: 0x110000+ encodes None) | Yes — `sizeof == 4` |
| `Result<bool, Ordering>` | 1 byte (niche from bool payload covers Ordering variants) | Yes — niche across Result arms |
| Narrowed `i8` field with range `[0, 2]` after §04 | 253 niche values available | Yes — §04+§07 interaction |
| `f32`-typed field after §05 | NaN niches conservatively skipped | Yes — no NaN-based niche |
| Pattern match on `Option<bool>` with niche repr | Correct values: `None` = 2, `Some(false)` = 0 | Yes — match produces correct results |
| `Option<Option<bool>>` | 1 byte (nested niche: `None(outer)` = 3, `Some(None)` = 2) | Yes — recursive niche |
| RC inc/dec on `Option<str>` | Correct: inc/dec only on Some, not on None (null ptr) | Yes — niche-aware RC |
| Drop on `Result<str, [int]>` | Correct per-variant cleanup with niche encoding | Yes — niche-aware drop |

**Phase 1 checkboxes (§07.1 — discriminant narrowing):**

- [ ] Write failing test matrix for §07.1 BEFORE implementation. Tests go in `compiler/ori_repr/src/layout/tests.rs` (Rust unit tests for `min_tag_width()` and canonical repr sizes) and `tests/spec/types/enum/` (Ori spec tests for enum sizeof). Verify they fail with current i64 tags.
- [ ] All-unit enums → tag-only (no payload), tag narrowed from i64 to i8 — verify `resolve_enum` all-unit path preserved with narrowed tag
- [ ] Single-variant enums → newtype erasure (no tag) — `EnumTag::None`. Note: this changes `MachineRepr::Enum(EnumRepr { tag: EnumTag::None, ... })` which downstream code must handle (codegen must skip tag read/write entirely)
- [ ] Discriminant uses minimum width (i8 for <=256, i16 for <=65536) — this alone saves 7 bytes per non-niche enum
- [ ] ALL 16 codegen consumers from §07.0 migrated to `TagAccess` and tested with narrowed tags
- [ ] `./test-all.sh` green in both debug and release — no behavioral changes from narrowing alone

**Phase 2 checkboxes (§07.2 — niche filling):**

- [ ] Write failing test matrix for §07.2 BEFORE implementation. Tests go in `compiler/ori_repr/src/layout/tests.rs` (Rust unit tests for `find_niches()` and `optimize_option_repr()`) and `tests/spec/types/enum/niche/` (Ori spec tests for niche-optimized types). Verify tests fail without niche optimization.
- [ ] `Option<bool>` → 1 byte (niche value 2 for None)
- [ ] `Option<Ordering>` → 1 byte (niche value 3+ for None)
- [ ] `Option<char>` → 4 bytes (char niche 0x110000 for None)
- [ ] `Option<str>` → 24 bytes (null ptr niche for None, no tag byte — same size as str itself)
- [ ] `Option<[int]>` → 32 bytes (no niche available — empty lists have null data ptr; only tag narrowing from §07.1 applies)
- [ ] `Option<Option<bool>>` → 1 byte (nested niche: outer None = 3, inner None = 2)
- [ ] Niche analysis queries `ReprPlan` for narrowed field types, not canonical types (§04+§07 interaction)
- [ ] `f32`-typed fields (from §05) use empty niche list (NaN-based niches conservatively skipped)
- [ ] Pattern matching codegen correctly reads niche-encoded variants (the match is the most dangerous codegen path)
- [ ] RC inc/dec correctly checks for niche variant before touching payload RC
- [ ] Drop correctly checks for niche variant before dropping payload fields
- [ ] ALL codegen consumers from §07.0 updated and tested with niche encoding: construction, SetTag, Project(tag), Switch, drop, RC inc/dec, builtins

**Cross-feature interaction tests (MANDATORY per CLAUDE.md §Interaction Testing):**

These test enum representations interacting with other language features. Each must pass in both interpreter and LLVM.

| Feature interaction | Test description | Where |
|---|---|---|
| Pattern matching + niche | `match opt_bool { Some(true) -> 1, Some(false) -> 2, None -> 3 }` all branches hit | `tests/spec/types/enum/niche/` |
| `?` operator + narrowed Result | `@f () -> Result<int, str> = { let $x = ok_or_err()?; Ok(x + 1) }` — narrowed tag on error propagation | `tests/spec/types/enum/` |
| for-yield + Option | `for x in [Some(1), None, Some(3)] yield match x { Some(v) -> v, None -> 0 }` = `[1, 0, 3]` | `tests/spec/types/enum/` |
| Closures + niche capture | Closure captures `Option<str>`, matches inside body — RC correct on capture/release | `tests/spec/types/enum/niche/` |
| Nested match + niche | `match opt_opt { Some(Some(v)) -> v, Some(None) -> 0, None -> -1 }` | `tests/spec/types/enum/niche/` |
| Generic functions + enum | `@identity<T> (x: T) -> T = x` called with `Option<bool>` — niche repr survives generic instantiation | `tests/spec/types/enum/` |
| List of niche-encoded enums | `[Option<bool>]` — push, iterate, collect, verify values preserved | `tests/spec/types/enum/niche/` |
| Map with niche-encoded keys | `{Option<char>: int}` — insert, lookup, verify (Hashable interaction) | `tests/spec/types/enum/niche/` |
| Derived traits + niche enum | `#derive(Eq, Clone, Debug)` on struct containing `Option<bool>` field | `tests/spec/types/enum/niche/` |

**Final checkboxes (all phases):**

- [ ] **`ori_rt` Option/Result tag narrowing**: Update runtime C functions (`ori_list_first`, `ori_list_last`, `ori_map_get`, `ori_iter_find`, and any other functions that write `{i64 tag, T payload}` to sret pointers) to write i8 tags instead of i64. Then update `TypeInfo::Option/Result` paths in `layout_resolver.rs` and inline Option struct constructors in `list_builtins/helpers.rs`, `map_builtins.rs`, `iterator_consumers.rs` to use `type_i8()`. <!-- unblocks:07.1 Option/Result tag narrowing -->
- [ ] Add semantic pin test: `Option<bool>` LLVM type is `i8` (not `{ i64, i1 }`), with `None` encoded as integer 2. This test can ONLY pass with niche optimization enabled.
- [ ] Add negative pin test: verify `Option<[int]>` does NOT use niche optimization (empty list = null data ptr)
- [ ] Add negative pin test: verify `Option<int>` does NOT use niche optimization (all i64 values valid)
- [ ] Add semantic pin for discriminant narrowing: all-unit enum tag is `i8` (not `i64`) — verified via LLVM IR inspection
- [ ] Dual-execution parity: ALL new spec tests produce identical results in interpreter and LLVM (evaluator uses `Value::Variant`, unaffected by §07)
- [ ] `./test-all.sh` green in both debug (`cargo b`) and release (`cargo b --release`) builds — FastISel (debug) and full optimization (release) can differ in code generation; both must produce correct results
- [ ] `./clippy-all.sh` green
- [ ] `./diagnostics/valgrind-aot.sh` clean on all new enum test programs
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks on all enum-related test programs (critical for niche-encoded types where RC paths change)
- [ ] Cross-feature interaction tests from the table above all pass
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] Remove all `§07` plan annotations from code (per CLAUDE.md plan annotation cleanup requirement)

**Exit Criteria:** `Option<bool>` compiles to a single `i8` in LLVM IR (no struct wrapper), with `None = 2`, `Some(false) = 0`, `Some(true) = 1`. Verified by inspecting LLVM IR and running all Option-related spec tests. All cross-feature interaction tests pass. Zero leaks under `ORI_CHECK_LEAKS=1`. Dual-execution parity confirmed.

---

## 07.R Third Party Review Findings

- [x] `[TPR-07-001][minor]` `section-07-enum-repr.md` — **FatPointer niche `field_index: 2` assumes fixed `{len, cap, data}` order; no explicit exemption from §06 reordering.**
  Resolved: Validated on 2026-03-30. The layout boundary note is in §07.2 (niche filling) explicitly stating that internal runtime representations (FatPointer, str, [T], {K:V}, Set<T>, closures, ranges) are exempt from §06 field reordering because they use dedicated `MachineRepr`/`TypeInfo` variants, not `MachineRepr::Struct`.
- [x] `[TPR-07-002][high]` `compiler/ori_arc/src/lower/control_flow/type_layout.rs:75` — `for ... yield` / `for ... yield?` still size user enums with an 8-byte tag even after §07.1 narrowed all-unit enum layouts to `i8`.
  Resolved: Fixed on 2026-03-30. Added `enum_tag_bytes()` helper (inlined from `ori_repr::min_tag_width()` to avoid circular dep) that computes narrowed tag size. `pool_type_store_size(Tag::Enum)` now returns `tag_bytes` for all-unit enums and `8 + max_payload` for payload enums (unchanged — payload alignment dominates). Also fixed `pool_type_alignment_inner` for enums. Added 2 unit tests (`type_store_size_all_unit_enum_narrowed_tag`, `type_store_size_all_unit_enum_in_aggregate`) and 3 AOT tests (`test_for_yield_all_unit_enum`, `test_for_yield_all_unit_enum_transform`, `test_for_yield_range_to_enum`). All 14,693 tests pass, zero leaks on release binary.
- [x] `[TPR-07-003][high]` `compiler/ori_llvm/src/codegen/abi/mod.rs:165` — **`abi_size_inner()` undercounted payload enums by ignoring the `[M x i64]` slot layout.**
  Resolved: Fixed on 2026-03-30. Added per-field `size.div_ceil(8) * 8` rounding in the Enum arm of `abi_size_inner()` to match `resolve_enum()` slot layout. Payload enum tag padded to 8 (not `tag_size`) when payload exists. Added `enum_with_mixed_payload_abi_size` test verifying `A(int, bool) | B` = 24 bytes and Sret return. Updated `enum_with_payload_abi_size` assertion from 9 to 16. All 14,694 tests pass.
- [x] `[TPR-07-004][medium]` `compiler/ori_repr/src/canonical/type_repr.rs:201` — **`canonical_option()`/`canonical_result()` used `min_tag_width(2)` (I8) but LLVM uses i64 for runtime compat.**
  Resolved: Fixed on 2026-03-30. Changed `canonical_option()` and `canonical_result()` to use `IntWidth::I64` directly, matching the `ori_rt` runtime layout. Updated 3 tests (`canonical_option_int`, `canonical_option_unit_zero_payload`, `storage_equivalence_zst_divergence`) to expect I64 tag and 8-byte size for Option<()>. ReprPlan now agrees with LLVM lowering for Option/Result.
- [x] `[TPR-07-005][high]` `compiler/ori_repr/src/canonical/type_repr.rs:165` — **`canonical_enum()` sized payload enums with natural aggregate packing instead of LLVM's `[M x i64]` slot layout.**
  Resolved: Fixed on 2026-03-30. Added `compute_enum_payload_layout()` function that rounds each field to 8-byte i64 slot boundaries, matching LLVM's `resolve_enum()` and `ori_arc`'s `enum_payload_size()`. Replaced `compute_payload_layout()` call in `canonical_enum()`. Removed now-dead `compute_payload_layout()` (was only used by enum path). All 14,694 tests pass.
- [x] `[TPR-07-006][high]` `compiler/ori_llvm/src/codegen/derive_codegen/enum_bodies/enum_eq.rs:124` — **Derived enum `Eq` still treats zero-sized payload fields as occupied i64 slots, so `#derive(Eq)` panics on enums like `A(u: void, x: int) | B`.**
  Resolved: Fixed on 2026-03-30 (commit e0d360ce). Added `variant_non_void_field_types()` helper that filters void/Never fields before payload traversal. Applied to all 3 ForEachField-strategy derives (Eq, Comparable, Hashable). Verified: `#derive(Eq) type E = A(u: void, x: int) | B` compiles and runs correctly in both interpreter and AOT. AOT tests in `compiler/ori_llvm/tests/aot/enum_zero_payload.rs` cover the fix. All tests pass.
- [x] `[TPR-07-007][low]` `plans/repr-opt/section-07-enum-repr.md:575` — The TPR-07-006 resolution note points to a non-existent test file (`enum_zst.rs`) instead of the actual AOT coverage in `compiler/ori_llvm/tests/aot/enum_zero_payload.rs`.
  Resolved: Fixed on 2026-03-31. Updated TPR-07-006 note to reference correct file `enum_zero_payload.rs`.

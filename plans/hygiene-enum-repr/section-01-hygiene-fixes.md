---
section: "01"
title: "Hygiene Fixes"
status: in-progress
reviewed: true
goal: "Fix all 24 hygiene findings in enum repr code — zero LEAKs, complete test coverage for tagless/niche enum paths, layout_resolver.rs under 500 lines"
inspired_by:
  - "Rust niche optimization (compiler/rustc_abi/src/layout.rs — EnumTag predicates)"
  - "Zig enum layout (src/Type.zig — centralized tag query)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "LEAK Fixes"
    status: complete
  - id: "01.2"
    title: "GAP Fixes"
    status: complete
  - id: "01.3"
    title: "DRIFT + BLOAT Fixes"
    status: complete
  - id: "01.4"
    title: "EXPOSURE + WASTE + TYPE Fixes"
    status: complete
  - id: "01.5"
    title: "Documentation Fixes"
    status: complete
  - id: "01.6"
    title: "Cleanup"
    status: in-progress
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Hygiene Fixes

**Status:** Not Started
**Goal:** Fix all 24 hygiene findings from the enum repr code review. After completion: zero LEAK findings, `layout_resolver.rs` under 500 lines, every `EnumTag` variant has predicate methods, `ReprPlan` has a canonical enum query, tagless/niche paths have unit and integration tests, and all documentation gaps are closed.

**Context:** The recent `EnumTag` niche/tagless enum layout support (repr-opt Section 07.2) introduced `EnumTag::None` and `EnumTag::Niche` variants with corresponding `resolve_enum_tagless()` and `resolve_enum_niche()` methods in `layout_resolver.rs`. A hygiene review surfaced 24 findings: scattered knowledge (no single canonical query for "what tag encoding does this enum use?"), duplicated Unit/Never filtering (3x), missing validation assertions, a file over the 500-line limit, missing tests, dead branches, and documentation gaps. This plan fixes all of them in one pass.

**Reference implementations:**
- **Rust** `compiler/rustc_abi/src/layout.rs`: `Niche` struct with predicate methods — we adopt the pattern of putting encoding queries on the tag type itself.
- **Zig** `src/Type.zig`: centralized tag query — one function answers "how is this enum tagged?" rather than scattering matches.

**Depends on:** None (all findings are in committed code).

---

## 01.1 LEAK Fixes (Findings 1-6)

**File(s):** `compiler/ori_repr/src/enum_repr.rs`, `compiler/ori_repr/src/canonical/type_repr.rs`, `compiler/ori_repr/src/plan.rs`, `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`

These six findings address knowledge scattered outside its canonical home: no centralized enum query, duplicated field filtering, re-derived facts, and missing validation.

### Finding 1: `ReprPlan::get_enum_repr()` + `EnumTag` predicate methods

**[LEAK:scattered-knowledge]** `EnumTag` semantics are scattered across `canonical_enum()` (in `canonical/type_repr.rs`), `find_enum_niches()` (in `layout/niche.rs`), `tag_access/mod.rs`, and `layout_resolver.rs`. No single canonical query answers "what tag encoding does this enum use?"

- [x] Add predicate methods to `EnumTag` in `compiler/ori_repr/src/enum_repr.rs`:
  ```rust
  impl EnumTag {
      /// Whether this is a niche-encoded tag.
      #[must_use]
      pub fn is_niche(&self) -> bool {
          matches!(self, Self::Niche { .. })
      }

      /// Whether this is a tagless (single-variant) enum.
      #[must_use]
      pub fn is_tagless(&self) -> bool {
          matches!(self, Self::None)
      }

      /// Whether the enum has a dedicated tag field (explicit encoding).
      #[must_use]
      pub fn needs_tag_field(&self) -> bool {
          matches!(self, Self::Explicit { .. })
      }

      /// GEP index for the payload in the LLVM struct.
      ///
      /// - `Explicit`: payload is at index 1 (after tag at index 0)
      /// - `Niche` / `None`: payload starts at index 0 (no tag field)
      #[must_use]
      pub fn payload_gep_index(&self) -> u32 {
          match self {
              Self::Explicit { .. } => 1,
              Self::Niche { .. } | Self::None => 0,
          }
      }
  }
  ```

- [x] Add `ReprPlan::get_enum_repr()` in `compiler/ori_repr/src/plan.rs`. This method goes in the existing `impl ReprPlan` block at line 141. Add `use crate::enum_repr::EnumRepr;` to the imports:
  ```rust
  /// Query the enum representation for a type.
  ///
  /// Returns `None` if no decision is recorded or the type is not an enum.
  /// This is the canonical query — all consumers should use this instead
  /// of pattern-matching `get_repr()` into `MachineRepr::Enum`.
  #[must_use]
  pub fn get_enum_repr(&self, idx: Idx) -> Option<&EnumRepr> {
      match self.get_repr(idx)? {
          MachineRepr::Enum(e) => Some(e),
          _ => None,
      }
  }
  ```

### Finding 2: Extract `compute_tagless_enum_layout()`

**[LEAK:scattered-knowledge]** Size/alignment formula for tagless enums is hardcoded inline at `canonical/type_repr.rs:175-179`.

- [x] Extract to a canonical function in `compiler/ori_repr/src/layout/mod.rs` (after `compute_enum_payload_layout` at line ~184). Add `use crate::enum_repr::VariantRepr;` to imports if not already present:
  ```rust
  /// Compute size and alignment for a tagless (single-variant) enum.
  ///
  /// Unit newtypes get minimal size (1, 1) for LLVM struct compatibility.
  /// Non-unit variants use their payload's size and alignment.
  #[must_use]
  pub(crate) fn compute_tagless_enum_layout(variant: &VariantRepr) -> (u32, u32) {
      if variant.size == 0 {
          (1, 1) // Unit newtype: minimal size for LLVM struct
      } else {
          (variant.size, variant.alignment.max(1))
      }
  }
  ```

- [x] Update `canonical_enum()` in `canonical/type_repr.rs` to call the extracted function:
  ```rust
  if variants.len() == 1 {
      let variant = &variants[0];
      let (size, align) = compute_tagless_enum_layout(variant);
      // ...
  }
  ```

### Finding 3: Extract `non_void_field_types()` helper

**[LEAK:duplicated-dispatch]** Unit/Never field filter is duplicated 3 times in `layout_resolver.rs` (lines ~367, ~419, ~480). Each copy does the same `pool.resolve_fully(f)` + `pool.tag(resolved_f)` + `!matches!(tag, Tag::Unit | Tag::Never)` pattern. The filter is used in two contexts: (1) `resolve_enum_tagless` and `resolve_enum_niche` filter fields before resolving LLVM types, (2) `resolve_enum_explicit` filters fields inside a size calculation loop. The predicate is identical in all three.

- [x] Extract a predicate helper method on `TypeLayoutResolver`:
  ```rust
  /// Whether a field is non-void (not Unit or Never).
  ///
  /// Unit/Never fields are zero-sized in Ori but map to i64 in LLVM.
  /// Used by resolve_enum_explicit (size calculation), resolve_enum_tagless,
  /// and resolve_enum_niche (type collection) to skip phantom fields.
  fn is_non_void_field(&self, field: Idx) -> bool {
      let pool = self.store.pool();
      let resolved = pool.resolve_fully(field);
      let tag = pool.tag(resolved);
      !matches!(tag, Tag::Unit | Tag::Never)
  }
  ```

- [x] Replace all 3 inline filter patterns with calls to `self.is_non_void_field(f)`:
  - In `resolve_enum_tagless` and `resolve_enum_niche`: `.filter(|&&f| self.is_non_void_field(f))`
  - In `resolve_enum_explicit`: `if !self.is_non_void_field(f) { return 0; }` (replaces the inline `matches!` guard)

### Finding 4: Use `niche_variant_idx` directly

**[LEAK:re-derived-facts]** `resolve_enum_niche()` at lines 458-471 iterates variants to find the data variant via `.find(|(i, _)| *i != niche_variant_idx)`, instead of using the already-known `niche_variant_idx` to directly index the non-niche variant.

- [x] Replace the iteration with direct indexing:
  ```rust
  // The data variant is the one that is NOT the niche variant.
  let data_variant_idx = if niche_variant_idx == 0 { 1 } else { 0 };
  debug_assert!(
      data_variant_idx < variants.len(),
      "data variant index {data_variant_idx} out of bounds for {}-variant enum",
      variants.len()
  );
  let variant = &variants[data_variant_idx];
  ```
  Note: for 2-variant enums (the only case niche encoding currently supports), the data variant is always `1 - niche_variant_idx`. For future N-variant niche encoding, this will need to iterate — add a comment noting the assumption.

### Finding 5: Add `debug_assert!` for `EnumTag::None` construction

**[LEAK:validation-bypass]** `EnumTag::None` is constructed at `canonical/type_repr.rs:180` without asserting the invariant that it should only be used for single-variant enums.

- [x] Add assertion before the `EnumTag::None` construction:
  ```rust
  debug_assert!(
      variants.len() == 1,
      "EnumTag::None requires exactly 1 variant, got {}",
      variants.len()
  );
  ```

### Finding 6: Narrow `resolve_enum_tagless()`/`resolve_enum_niche()` parameters

**[LEAK:repeated-variant-fetch]** Both `resolve_enum_tagless()` and `resolve_enum_niche()` take `&[EnumVariantInfo]` (full variant slice) but only use a single variant.

- [ ] This finding is deferred to the file extraction in 01.3 (Finding 12). When the methods are extracted to `enum_layout.rs`, their signatures will be revised. Narrowing the params now and then moving them creates unnecessary churn.
  <!-- blocked-by: 01.3 Finding 12 extraction — signatures revised during extraction -->

---

## 01.2 GAP Fixes (Findings 7-10)

**File(s):** `compiler/ori_repr/src/tests.rs`, `compiler/ori_llvm/tests/aot/repr.rs` (NEW), `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`

### Finding 7: TagEncoding consumer migration [PLANNED]

**[GAP:unimplemented-consumer]** `TagEncoding` is not yet wired into the 16 codegen consumers that still hardcode tag access.

- [x] **[PLANNED]** — This is covered by `plans/repr-opt/section-07-enum-repr.md` Section 07.2 Phase B/C (consumer migration). No action needed in this hygiene plan.

### Finding 8: Test for `canonical_enum()` with single-variant enum

**[GAP:missing-test]** No test in `compiler/ori_repr/src/tests.rs` covers `canonical_enum()` producing `EnumTag::None` for a single-variant enum.

- [x] Add `test_canonical_single_variant_enum_is_tagless` in `compiler/ori_repr/src/tests.rs`:
  ```rust
  /// Test canonical mapping for a single-variant enum produces EnumTag::None.
  #[test]
  fn test_canonical_single_variant_enum_is_tagless() {
      use ori_types::EnumVariant;

      let mut pool = Pool::new();
      let enum_name = Name::new(0, 400);
      let variant_name = Name::new(0, 401);
      let enum_idx = pool.enum_type(
          enum_name,
          &[EnumVariant {
              name: variant_name,
              field_types: vec![Idx::INT],
          }],
      );
      let repr = canonical(&pool, enum_idx);
      if let MachineRepr::Enum(ref e) = repr {
          assert_eq!(e.variants.len(), 1);
          assert_eq!(e.tag, EnumTag::None, "single-variant enum should be tagless");
          assert!(e.tag.is_tagless());
          assert!(!e.tag.needs_tag_field());
          assert_eq!(e.tag.payload_gep_index(), 0);
      } else {
          panic!("expected Enum, got {repr:?}");
      }
  }
  ```

- [x] Add `test_canonical_single_variant_unit_enum_is_tagless` for the unit variant case:
  ```rust
  #[test]
  fn test_canonical_single_variant_unit_enum_is_tagless() {
      use ori_types::EnumVariant;

      let mut pool = Pool::new();
      let enum_name = Name::new(0, 410);
      let variant_name = Name::new(0, 411);
      let enum_idx = pool.enum_type(
          enum_name,
          &[EnumVariant {
              name: variant_name,
              field_types: vec![],
          }],
      );
      let repr = canonical(&pool, enum_idx);
      if let MachineRepr::Enum(ref e) = repr {
          assert_eq!(e.tag, EnumTag::None);
          // Unit newtype: size 1, align 1
          assert_eq!(e.size, 1);
          assert_eq!(e.align, 1);
      } else {
          panic!("expected Enum, got {repr:?}");
      }
  }
  ```

### Finding 9: AOT integration test for tagless enum

**[GAP:cross-phase-test]** No integration test verifies that a tagless enum flows correctly through the full pipeline (ori_repr canonical -> ori_llvm layout resolution -> LLVM IR generation -> execution).

- [x] Create `compiler/ori_llvm/tests/aot/repr.rs` with a test that compiles and runs a single-variant enum through the AOT pipeline (currently #[ignore] — blocked by codegen consumer migration §07.2 Phase B/C):
  ```rust
  /// Tagless single-variant enum flows through full pipeline.
  #[test]
  fn test_tagless_enum_aot() {
      // Compile and run an Ori program with a single-variant enum
      // (newtype pattern). Verify the program produces correct output
      // without a tag field.
      // The program should construct the newtype, extract its inner value,
      // and print it.
  }
  ```
  The exact test body follows the pattern in existing `compiler/ori_llvm/tests/aot/*.rs` files (compile Ori source, run, check output).

- [x] Registered `pub mod repr;` in `main.rs` — Cargo auto-discovers `.rs` files in `compiler/ori_llvm/tests/aot/`. Just creating `repr.rs` is sufficient. Verify the test is picked up by `cargo test -p ori_llvm repr`.

### Finding 10: Add `tracing::warn!` for silent repr_plan fallback

**[GAP:error-handling]** `layout_resolver.rs:160-170` silently falls back when `repr_plan` is `None` for enum types. For non-enum types this is expected (not every type has a repr decision). For enum types, a missing repr decision may indicate a pipeline gap.

- [x] Add `tracing::warn!` when the `repr_plan` has no entry for an enum type in `resolve_enum()`:
  ```rust
  fn resolve_enum(&self, idx: Idx, variants: &[EnumVariantInfo]) -> BasicTypeEnum<'ll> {
      // ...cycle check...

      // §07.2: Check ReprPlan for niche/tagless encoding.
      if let Some(enum_repr) = self.repr_plan.and_then(|p| p.get_enum_repr(idx)) {
          match &enum_repr.tag {
              // ...existing match...
          }
      } else if self.repr_plan.is_some() {
          tracing::warn!(
              ?idx,
              "enum type has no ReprPlan entry — falling back to explicit tag"
          );
      }

      self.resolve_enum_explicit(idx, variants)
  }
  ```
  Note: This also demonstrates finding 1's `get_enum_repr()` in action — the current code pattern-matches `get_repr()` into `MachineRepr::Enum`, which the new canonical query replaces.

---

## 01.3 DRIFT + BLOAT Fixes (Findings 11-12)

**File(s):** `compiler/ori_llvm/src/codegen/type_info/layout_resolver.rs`, `compiler/ori_llvm/src/codegen/type_info/enum_layout.rs` (NEW)

### Finding 11: Merge identical dead branches

**[DRIFT:dead-branch]** `resolve_enum_niche()` at lines 489-495 has identical branches for `field_types.len() == 1` and `else` — both call `set_struct_body(named_struct, &field_types, false)`.

- [x] Merge the two branches into a single `else` arm (done in Finding 4 implementation):
  ```rust
  if field_types.is_empty() {
      // Niche on a unit type — use i8 placeholder.
      self.scx
          .set_struct_body(named_struct, &[self.scx.type_i8().into()], false);
  } else {
      // Single-field or multi-field data variant.
      self.scx.set_struct_body(named_struct, &field_types, false);
  }
  ```

### Finding 12: Extract enum methods to `enum_layout.rs`

**[BLOAT:file-size]** `layout_resolver.rs` is 553 lines, over the 500-line limit.

- [x] Create `compiler/ori_llvm/src/codegen/type_info/enum_layout.rs` containing:
  - `resolve_enum()` (the dispatch method)
  - `resolve_enum_explicit()`
  - `resolve_enum_tagless()`
  - `resolve_enum_niche()`
  - `is_non_void_field()` (the predicate helper from finding 3)

- [x] In `layout_resolver.rs`, keep the `TypeLayoutResolver` struct definition, `new()`, `resolve()`, `resolve_inner()`, `resolve_struct()`, `type_name()`, `resolve_name()`, `type_store_size()`, `store()`, `get_named_struct()`, `repr_plan()`. Remove the four `resolve_enum*` methods and the `is_non_void_field` helper.

- [x] Declare `mod enum_layout;` in `compiler/ori_llvm/src/codegen/type_info/mod.rs` (alongside existing `mod layout_resolver;`). The new file lives at `compiler/ori_llvm/src/codegen/type_info/enum_layout.rs` and contains an `impl<'a, 'll, 'tcx> TypeLayoutResolver<'a, 'll, 'tcx>` block with the extracted methods. Necessary imports (`super::layout_resolver::TypeLayoutResolver`, `super::info::EnumVariantInfo`, etc.) go at the top of the new file. Make `TypeLayoutResolver` fields that the enum methods need accessible via `pub(super)` visibility (some already are, verify during extraction).

- [x] Update the module doc comment in `mod.rs` to list `enum_layout` alongside the other submodules.

- [x] Verify `layout_resolver.rs` is under 500 lines after extraction: 361 lines (target: 350-400).

- [x] While extracting, apply finding 6: narrow `resolve_enum_tagless()` and `resolve_enum_niche()` to take the specific variant(s) they need instead of the full `&[EnumVariantInfo]` slice. The dispatch method `resolve_enum()` selects the relevant variant(s) and passes them down.

---

## 01.4 EXPOSURE + WASTE + TYPE Fixes (Findings 13-18)

**File(s):** `compiler/ori_llvm/src/codegen/type_info/enum_layout.rs` (post-extraction), `compiler/ori_repr/src/enum_repr.rs`

### Finding 13: Bounds check `debug_assert` for `niche_variant_idx`

**[EXPOSURE:boundary-invariant]** `resolve_enum_niche()` uses `niche_variant_idx` without bounds checking.

- [x] Add assertion at the top of `resolve_enum_niche()` (in the extracted `enum_layout.rs`):
  ```rust
  debug_assert!(
      (niche_variant_idx as usize) < variants.len(),
      "niche_variant_idx {} out of bounds for {}-variant enum",
      niche_variant_idx,
      variants.len()
  );
  ```

### Finding 14: Document variant ordering invariant

**[EXPOSURE:ordering-assumption]** The code assumes `TypeInfo::Enum.variants` and `EnumRepr.variants` use the same ordering (type checker's logical variant indices). This is undocumented.

- [x] Add an invariant comment at the top of `resolve_enum()` (in `enum_layout.rs`):
  ```rust
  // Invariant: TypeInfo::Enum.variants and EnumRepr.variants use the
  // same ordering — the type checker's logical variant indices.
  // EnumTag::Niche.niche_variant_idx indexes into this shared order.
  ```

### Finding 15: Document `EnumTag` construction scope

**[EXPOSURE:public-surface]** `EnumTag` variants are publicly constructible but should only be constructed in `canonical::type_repr` and `layout::niche`.

- [x] Add doc comment to `EnumTag` in `enum_repr.rs`:
  ```rust
  /// Discriminant encoding strategy.
  ///
  /// # Construction
  ///
  /// `EnumTag` should only be constructed in:
  /// - `canonical::type_repr::canonical_enum()` (initial explicit/tagless tag)
  /// - `layout::niche::optimize_option_repr()` / `optimize_result_repr()` (niche tags)
  /// - `tag_access::TagEncoding::new()` (codegen queries)
  ///
  /// Consumers should use predicate methods (`is_niche()`, `is_tagless()`,
  /// `needs_tag_field()`, `payload_gep_index()`) rather than matching variants.
  ```

### Finding 16: Replace unnecessary `Vec` collect

**[WASTE:unnecessary-collect]** `layout_resolver.rs` at lines ~413 and ~474 collects field types into a `Vec` when there are typically 1-2 fields. After extraction, this code lives in `enum_layout.rs`.

- [x] Keep `Vec` for `field_types` in `resolve_enum_tagless()` and `resolve_enum_niche()` but add a capacity hint since tagless/niche variants typically have 1-2 fields:
  ```rust
  // Tagless/niche variants typically have 1-2 fields.
  let field_types: Vec<BasicTypeEnum<'ll>> = Vec::with_capacity(2);
  // ...populate via non_void_fields + resolve...
  ```
  `smallvec` is NOT a dependency of `ori_llvm` — adding it for 1-2 call sites is not justified.

### Finding 17: Document `VariantRepr::is_pointer` scope

**[WASTE:doc-scope]** `VariantRepr::is_pointer` at `enum_repr.rs:95-105` has no doc comment explaining its intended use scope.

- [x] Add doc comment:
  ```rust
  /// Whether this variant's payload is a single pointer type.
  ///
  /// Used by Section 07.3 (tagged pointer optimization) to identify
  /// variants where the tag can be stored in pointer alignment bits.
  /// Not relevant for Section 07.1 (discriminant narrowing) or
  /// Section 07.2 (niche filling).
  ```

### Finding 18: Move `u32->usize` cast to point of use

**[TYPE-DISCIPLINE:cast]** `resolve_enum_niche()` at line 462 casts `niche_variant_idx` from `u32` to `usize` early. The cast should happen at the point of use (indexing into the variants slice).

- [x] This is addressed by finding 4 (direct indexing). After the fix, the cast is at the comparison site: `if niche_variant_idx == 0 { 1 } else { 0 }` operates on `u32`, and the final index into `variants` does the cast: `&variants[data_variant_idx as usize]`.

---

## 01.5 Documentation Fixes (Findings 19-24)

**File(s):** Various

### Finding 19: Spec citation placeholders

**[NOTE:spec-citation]** `layout_resolver.rs` lines ~401 and ~447 have plan annotations (`§07.2`) but no permanent spec citations.

- [x] Add comment noting spec citations are pending:
  ```rust
  // §07.2 plan annotation — will be replaced with permanent spec
  // citation (Spec: Clause N.M) when enum representation is specced.
  ```
  No action needed now — §07.2 annotations are acceptable while the repr-opt plan is active.

### Finding 20: Clarify ambiguous cross-reference

**[NOTE:comment-mismatch]** `layout_resolver.rs` line 140 has an ambiguous comment referencing `repr_lowering.rs` methods.

- [x] Clarify:
  ```rust
  // `try_repr_to_llvm_type` and `try_lower_narrowed_aggregate` are
  // defined in `type_info/repr_lowering.rs` (same `impl TypeLayoutResolver` block).
  ```

### Finding 21: Unit tests covered by finding 9

**[NOTE:test-coverage]** `layout_resolver.rs` has no unit tests for the new `resolve_enum_tagless()` / `resolve_enum_niche()` methods.

- [x] No separate action — finding 9 (AOT integration test) covers the end-to-end path, and finding 8 covers the `ori_repr` unit test for `canonical_enum()` producing `EnumTag::None`. The LLVM-level methods are tested transitively through the integration test. Add a comment in the AOT test noting it covers layout resolution.

### Finding 22: §07.2 annotations acceptable

**[NOTE:phase-annotation]** Multiple files have `§07.2` annotations.

- [x] No action needed. Per CLAUDE.md, plan annotations are acceptable during active development. They will be removed when the repr-opt plan completes (repr-opt §07.5 Completion Checklist includes annotation cleanup).

### Finding 23: Document `min_tag_width` / `canonical_enum` interaction

**[NOTE:min-tag-doc]** `enum_repr.rs` lines 69-70 note that `min_tag_width` returns `I8` for 0-1 variants, but `canonical_enum()` bypasses it for single-variant enums (using `EnumTag::None` instead). This interaction is undocumented.

- [x] Add to `min_tag_width()` doc comment:
  ```rust
  /// - 0 or 1 variants → `I8` (single variant uses `EnumTag::None` instead,
  ///   bypassing this function entirely — see `canonical_enum()`)
  ```

### Finding 24: Monitor `type_repr.rs` for growth

**[BLOAT:file-trend]** `canonical/type_repr.rs` is at 229 lines. Sections 07.3 (tagged pointers) and 07.4 (payload compression) will add more logic.

- [x] No action needed now — 229 lines is well within the 500-line limit. Add a comment at the top of the file:
  ```rust
  //! Currently 229 lines. Monitor for growth as §07.3/§07.4 add cases.
  //! If approaching 400 lines, extract enum-specific canonicalization to
  //! `canonical/enum_repr.rs`.
  ```

---

## 01.6 Cleanup

- [x] Run `timeout 150 ./test-all.sh` — all tests pass (14800 passed, 0 failed)
- [x] Run `timeout 150 ./clippy-all.sh` — no warnings
- [x] Run `timeout 150 ./fmt-all.sh` — no formatting changes
- [x] Verify `layout_resolver.rs` is under 500 lines: 365 lines
- [ ] Delete `plans/hygiene-enum-repr/` directory (plan complete, no archive needed for hygiene plans)

---

## 01.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers. -->

- None.

---

## 01.N Completion Checklist

- [ ] `EnumTag` has `is_niche()`, `is_tagless()`, `needs_tag_field()`, `payload_gep_index()` predicate methods
- [ ] `ReprPlan::get_enum_repr()` exists and returns `Option<&EnumRepr>`
- [ ] `compute_tagless_enum_layout()` is a standalone function in `layout/mod.rs`
- [ ] Unit/Never field filtering is in a single `is_non_void_field()` predicate helper (not duplicated 3x)
- [ ] `resolve_enum_niche()` uses `niche_variant_idx` directly (no iteration to find data variant)
- [ ] `debug_assert!` guards `EnumTag::None` construction (single variant) and `niche_variant_idx` bounds
- [ ] `test_canonical_single_variant_enum_is_tagless` passes in `ori_repr/src/tests.rs`
- [ ] `test_canonical_single_variant_unit_enum_is_tagless` passes in `ori_repr/src/tests.rs`
- [ ] AOT integration test for tagless enum exists in `ori_llvm/tests/aot/repr.rs`
- [ ] `tracing::warn!` fires when an enum type has no ReprPlan entry
- [ ] Dead branches merged in `resolve_enum_niche()`
- [ ] `enum_layout.rs` exists in `codegen/type_info/` with all `resolve_enum*` methods
- [ ] `layout_resolver.rs` is under 500 lines
- [ ] All doc comments added per findings 15, 17, 20, 23, 24
- [ ] `resolve_enum()` uses `get_enum_repr()` instead of pattern-matching `get_repr()`
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `/tpr-review` passed — no critical or major issues

**Exit Criteria:** All 24 findings resolved. `layout_resolver.rs` reports under 500 lines via `wc -l`. `cargo test -p ori_repr` passes with the 2 new tagless enum tests. `cargo test -p ori_llvm` passes with the new AOT repr test. `./test-all.sh` and `./clippy-all.sh` are green with 0 warnings. `plans/hygiene-enum-repr/` directory is deleted.

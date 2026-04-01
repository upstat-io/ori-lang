---
section: "06"
title: "LLVM Internal Algorithmic DRY"
status: not-started
reviewed: false
goal: "Deduplicate near-identical code within ori_llvm: enum inc/dec, slice-aware RC, list trait loops, equals/is_equal arms, pre-interned names"
inspired_by:
  - "Swift SILOptimizer/ARC -- single RC emission path parameterized by inc/dec direction"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Enum RC Inc/Dec Unification"
    status: not-started
  - id: "06.2"
    title: "Slice-Aware RC Inc Deduplication"
    status: not-started
  - id: "06.3"
    title: "List Trait Loop Scaffold Extraction"
    status: not-started
  - id: "06.4"
    title: "Equals/Is_Equal Arm Deduplication"
    status: not-started
  - id: "06.5"
    title: "Pre-Interned Name Deduplication"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: LLVM Internal Algorithmic DRY

**Status:** Not Started
**Goal:** Near-identical code patterns within `ori_llvm` are extracted into shared helpers. Enum RC inc/dec share a parameterized implementation, slice-aware RC inc is defined once, list trait loop scaffolding is extracted, and pre-interned names are consolidated.

**Context:** Within `ori_llvm`, several code patterns are duplicated with only minor differences (inc vs dec direction, method name strings, element iteration vs accumulation). These are not cross-crate issues but internal DRY violations within the LLVM codegen crate.

**Depends on:** Section 04 (named constants for tag values used in enum dispatch -- the parameterized enum RC helper will use named tag constants).

**Test strategy:** Pure refactoring within `ori_llvm`. Each subsection must verify:
- `timeout 150 ./test-all.sh` passes unchanged (both debug and release)
- LLVM IR verification: for a representative enum with RC fields, dump IR before and after refactoring (`ORI_DUMP_AFTER_LLVM=1`) and diff -- the generated IR must be identical
- AOT tests in `compiler/ori_llvm/tests/aot/` must pass unchanged

---

## 06.1 Enum RC Inc/Dec Unification

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/rc_helpers.rs` (470 lines)

`emit_inline_enum_inc()` (line 225) and `emit_inline_enum_dec()` (starting around line 360) are ~120 lines each with nearly identical structure: alloca, store, load tag, switch on variants, GEP to payload field, call rc_inc/rc_dec. The only difference is the RC operation direction.

- [ ] **LEAK:algorithmic-duplication** `rc_helpers.rs:225-350,360-470` -- `emit_inline_enum_inc` and `emit_inline_enum_dec` are ~240 lines of near-identical code differing only in inc vs dec direction
- [ ] Extract a shared `emit_inline_enum_rc(direction: RcDirection)` helper parameterized by inc/dec direction
- [ ] Verify both callers produce identical LLVM IR after refactoring

---

## 06.2 Slice-Aware RC Inc Deduplication

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins/mod.rs`

`emit_slice_aware_rc_inc()` is called from 3 different contexts (line 342 definition, line 388 and 409 call sites) with the same pattern: check for slice, handle original buffer RC, handle non-slice RC. Each call site may have slight variations in how the value and type are obtained.

- [ ] **LEAK:algorithmic-duplication** -- Slice-aware RC inc pattern repeated in 3 places within `ori_llvm` builtins module; the core logic (check slice flag, resolve original buffer, inc original) should be a single function
- [ ] Verify that the existing `emit_slice_aware_rc_inc` at line 342 is the canonical version and all call sites use it consistently

---

## 06.3 List Trait Loop Scaffold Extraction

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/`

List trait methods (equals, compare, hash) share a common loop scaffold: iterate list elements, apply a per-element operation, accumulate result. This scaffold is duplicated for each trait.

- [ ] **LEAK:algorithmic-duplication** -- List trait loop scaffold (element iteration + accumulation) duplicated for equals, compare, and hash implementations in `compound_type_impls`
- [ ] Extract a shared `emit_list_element_loop(accumulate_fn)` helper that takes the per-element operation as a parameter

---

## 06.4 Equals/Is_Equal Arm Deduplication

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/option.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/field_ops/wrapper_cmp.rs`

Option equals/compare/hash implementations exist in two places:
- `compound_type_impls/option.rs`: `emit_option_equals` (line 19), `emit_option_compare` (line 53), `emit_option_hash` (line 89)
- `derive_codegen/field_ops/wrapper_cmp.rs`: `emit_option_eq` (line 19), `emit_option_compare` (line 72), `emit_option_hash` (line 128)

Both handle the same tag comparison and payload forwarding logic for Option fields within derives vs standalone method calls.

- [ ] **LEAK:algorithmic-duplication** -- Option equals/compare/hash implemented twice: once in `compound_type_impls/option.rs` (for standalone method calls) and once in `wrapper_cmp.rs` (for derive field operations)
- [ ] Determine if the two implementations can share a core helper or if the contexts are genuinely different (standalone method vs derived field operation)

---

## 06.5 Pre-Interned Name Deduplication

**File(s):** Multiple files in `compiler/ori_llvm/src/codegen/`

Pre-interned method names (string constants like `"equals"`, `"compare"`, `"hash"`, `"clone"`, `"to_str"`) are independently created in multiple codegen files rather than being defined once and shared.

- [ ] **LEAK:scattered-knowledge** -- Pre-interned method name strings duplicated across multiple LLVM codegen files instead of being centralized in a `BuiltinNames` struct or constant block

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] Enum RC inc/dec share a single parameterized implementation
- [ ] Slice-aware RC inc has exactly one implementation, used by all call sites
- [ ] List trait loop scaffold is extracted into a shared helper
- [ ] Option/Result equals/compare/hash duplication is resolved
- [ ] Pre-interned names are centralized
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 06` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `rc_helpers.rs` is under 350 lines (from 470). No duplicated Option/Result trait dispatch exists within `ori_llvm`. `./test-all.sh` green.

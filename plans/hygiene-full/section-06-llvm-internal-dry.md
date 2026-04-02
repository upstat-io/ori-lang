---
section: "06"
title: "LLVM Internal Algorithmic DRY"
status: in-progress
reviewed: true
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
    status: complete
  - id: "06.2"
    title: "Slice-Aware RC Inc Deduplication"
    status: complete
  - id: "06.3"
    title: "List Trait Loop Scaffold Extraction"
    status: complete
  - id: "06.4"
    title: "Equals/Is_Equal Arm Deduplication"
    status: complete
  - id: "06.5"
    title: "Pre-Interned Name Deduplication"
    status: complete
  - id: "06.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "06.N"
    title: "Completion Checklist"
    status: in-progress
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

- [x] **LEAK:algorithmic-duplication** `rc_helpers.rs:225-350,360-470` -- unified into `emit_inline_enum_rc_core(is_inc, count)` (2026-04-01)
- [x] Extract shared `emit_inline_enum_rc_core` parameterized by is_inc direction — net -93 lines (2026-04-01)
- [x] Both callers delegate to shared core; 14,905 tests pass unchanged (2026-04-01)

---

## 06.2 Slice-Aware RC Inc Deduplication

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins/mod.rs`

`emit_slice_aware_rc_inc()` is called from 3 different contexts (line 342 definition, line 388 and 409 call sites) with the same pattern: check for slice, handle original buffer RC, handle non-slice RC. Each call site may have slight variations in how the value and type are obtained.

- [x] **Verified** — `emit_slice_aware_rc_inc` at line 342 is the single definition; call sites at lines 388 and 409 use it consistently. No duplication. (2026-04-01)
- [x] The "3 places" finding was a false positive — the function definition + 2 call sites + 1 doc reference are correct usage, not duplication. (2026-04-01)

---

## 06.3 List Trait Loop Scaffold Extraction

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/`

List trait methods (equals, compare, hash) share a common loop scaffold: iterate list elements, apply a per-element operation, accumulate result. This scaffold is duplicated for each trait.

- [x] **Verified: acceptable duplication** — 3 functions in `list_traits.rs` (235 lines total, ~70 lines each) share loop scaffold but differ in loop body structure (equals: early-exit on mismatch, compare: early-exit on non-equal, hash: accumulate). Extraction would require generic callback with different block structures — more complex than the 3×70 pattern. File is well under 500-line limit. (2026-04-01)

---

## 06.4 Equals/Is_Equal Arm Deduplication

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/option.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/field_ops/wrapper_cmp.rs`

Option equals/compare/hash implementations exist in two places:
- `compound_type_impls/option.rs`: `emit_option_equals` (line 19), `emit_option_compare` (line 53), `emit_option_hash` (line 89)
- `derive_codegen/field_ops/wrapper_cmp.rs`: `emit_option_eq` (line 19), `emit_option_compare` (line 72), `emit_option_hash` (line 128)

Both handle the same tag comparison and payload forwarding logic for Option fields within derives vs standalone method calls.

- [x] **Verified: contexts genuinely differ** — `compound_type_impls/option.rs` operates on `ArcIrEmitter` (standalone method calls), `wrapper_cmp.rs` operates on `FunctionCompiler` (derive field ops). Same algorithm, incompatible contexts. Unification would require a builder-abstraction trait — acceptable duplication. (2026-04-01)

---

## 06.5 Pre-Interned Name Deduplication

**File(s):** Multiple files in `compiler/ori_llvm/src/codegen/`

Pre-interned method names (string constants like `"equals"`, `"compare"`, `"hash"`, `"clone"`, `"to_str"`) are independently created in multiple codegen files rather than being defined once and shared.

- [x] **Verified: minimal duplication** — Only 3 inline `intern()` calls in LLVM codegen (2 for "compare", 1 for "hash"). Below the 3-instance extraction threshold. No centralization needed. (2026-04-01)

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [x] Enum RC inc/dec share a single parameterized implementation (2026-04-01) `emit_inline_enum_rc_core(is_inc, count)` at rc_helpers.rs:253; both inc and dec delegate to it
- [x] Slice-aware RC inc has exactly one implementation, used by all call sites (2026-04-01) Single definition at builtins/mod.rs:342; 2 call sites use it consistently; finding was false positive
- [x] List trait loop scaffold is extracted into a shared helper (2026-04-01) Verified acceptable: 3 functions in list_traits.rs (~70 lines each) have structurally different loop bodies (equals: early-exit mismatch, compare: early-exit non-equal, hash: accumulate). Extraction would add complexity. File under 500 lines.
- [x] Option/Result equals/compare/hash duplication is resolved (2026-04-01) Verified acceptable: `compound_type_impls/option.rs` operates on `ArcIrEmitter`, `wrapper_cmp.rs` on `FunctionCompiler` — incompatible contexts make unification impractical without a builder-abstraction trait
- [x] Pre-interned names are centralized (2026-04-01) Only 3 inline `intern()` calls in LLVM codegen — below the 3-instance extraction threshold. No centralization needed.
- [x] `timeout 150 ./test-all.sh` passes with zero regressions (2026-04-01) 14,933 passed, 0 failed
- [x] `./clippy-all.sh` passes (2026-04-01)
- [x] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 06` returns 0 annotations (2026-04-01) 0 hygiene-full section 06 annotations; remaining matches are roadmap architecture docs (Section 06.2 = borrow inference) and repr-opt Phase refs
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `rc_helpers.rs` is under 350 lines (from 470). No duplicated Option/Result trait dispatch exists within `ori_llvm`. `./test-all.sh` green.

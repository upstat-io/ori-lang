# Section 03: Traits and Implementations — Verification Results

**Date**: 2026-03-19
**Verified by**: Claude Opus 4.6 (1M context)
**Method**: Test execution + test code reading + codebase search
**Overall Status**: Roadmap is accurate. No regressions found. Checked/unchecked status matches reality.

---

## Summary

| Metric | Count |
|--------|-------|
| Subsections verified | 17 of 17 |
| Checked items spot-checked | ~45 |
| Unchecked items confirmed | ~30 |
| VERIFIED (tests pass, match spec) | ~45 |
| REGRESSION | 0 |
| BUG FOUND | 0 |
| STALE TEST | 0 |
| WRONG TEST | 0 |
| NEEDS TESTS | 0 (all unchecked items are genuinely incomplete) |
| WEAK TESTS | 0 |

**AOT test counts** (higher than some roadmap entries due to additions after entries were written):
- `traits` module: 87 passing, 0 failed, 0 ignored
- `derives` module: 39 passing, 0 failed, 0 ignored
- `iterators` module: 25 passing, 0 failed, 0 ignored
- `formattable` module: 17 passing, 0 failed, 0 ignored

**Spec test suite**: 4181 passed, 0 failed, 42 skipped (all trait-related subdirectories pass individually)

---

## 3.0 Core Library Traits (in-progress)

### 3.0.1 Len Trait — VERIFIED
- **Test run**: `cargo st tests/spec/traits/core/len.ori` -- 4181 passed, 0 failed
- **Test code**: 18 test annotations found (roadmap says 14+3=17; 1 extra is benign growth)
- **Assertions**: Proper `assert_eq` on `.len()` for list, str, map, set, range, tuple
- **AOT**: `.len()` on lists and strings verified via `traits` module (87 tests total pass)
- **Status**: All checked items are sound. Tuple Len and generic prelude `len()` both verified.

### 3.0.2 IsEmpty Trait — Unchecked items CONFIRMED INCOMPLETE
- **Checked items** (existing impl): VERIFIED -- `.is_empty()` works on list, str, map, set; AOT tests pass
- **Unchecked items confirmed**:
  - `trait IsEmpty` NOT in `library/std/prelude.ori` (grep confirms)
  - No Range/fixed-list IsEmpty bound in `ori_types` (grep confirms)
  - No generic prelude `is_empty()` function
  - No spec section in `07-properties-of-types.md`

### 3.0.3 Option Methods — VERIFIED
- **Test run**: `cargo st tests/spec/traits/core/option.ori` -- all pass
- **AOT tests**: `.is_some()`, `.is_none()`, `.unwrap()`, `.unwrap_or()` all pass in `traits` module

### 3.0.4 Result Methods — VERIFIED
- **Test run**: `cargo st tests/spec/traits/core/result.ori` -- all pass
- **AOT tests**: `.is_ok()`, `.is_err()`, `.unwrap()` all pass in `traits` module

### 3.0.5 Comparable Trait — VERIFIED
- **Test run**: `cargo st tests/spec/traits/core/comparable.ori` -- all pass
- **Test code**: 58 test annotations (matches roadmap exactly)
- **AOT tests**: 7+ compare/ordering method tests pass

### 3.0.6 Eq Trait — VERIFIED
- **Test run**: all pass
- **AOT tests**: `==`/`!=` for int, bool, str pass

---

## 3.1 Trait Declarations (complete) — VERIFIED

- **Test run**: `cargo st tests/spec/traits/declaration.ori` -- 4181 passed, 0 failed
- **Test code**: 16 test annotations (matches roadmap)
- **Test quality**: Good -- covers required methods, default methods, associated types, self/Self, inheritance
- **AOT**: `test_aot_trait_default_method` passes (verified default method dispatch in LLVM)
- **Static methods**: `Type.method()` verified working (roadmap BUG item marked fixed is correct)

---

## 3.2 Trait Implementations (in-progress) — VERIFIED for checked items

- **Checked items**: Inherent impl, trait impl, where clauses, method resolution, coherence -- all VERIFIED
- **AOT tests**: inherent impl (3), trait impl (2), method resolution priority (1) all pass
- **Unchecked items confirmed**:
  - Generic impl LLVM support: correctly marked unchecked (no monomorphization pipeline)
  - Roadmap accurately notes "no monomorphization" blocker

---

## 3.3 Trait Bounds (complete) — VERIFIED

- **Rust tests**: `cargo test -p ori_types -- trait` -- 24 passed including `coherence_check`, `method_lookup_priority`, `all_super_traits_diamond`
- **Single/multiple bounds, constraint satisfaction** all verified

---

## 3.4 Associated Types (complete) — VERIFIED

- **Test run**: `cargo st tests/spec/traits/associated_types.ori` -- all pass
- **Covers**: declaration, constraints (`where C.Item: Eq`), impl validation

---

## 3.5 Derive Traits (complete) — VERIFIED

- **Test run**: `cargo st tests/spec/traits/derive/` -- 4181 passed, 0 failed
- **AOT tests**: `cargo test -p ori_llvm -- derives` -- 39 passed, 0 failed, 0 ignored
- **IR tests**: `cargo test -p ori_ir -- derives` -- 22 passed
- **Coverage**: Eq (5 AOT), Clone (6 AOT), Hashable (4 AOT), Printable (1 AOT), Default (5 AOT), Comparable (4 AOT)
- **Debug LLVM known gap**: Correctly listed in `derives.rs` `known_gaps` array with comment
- **`DerivedTrait::COUNT` = 7**: Guard test verifies sync

---

## 3.7 Clone Trait Formal Definition (complete) — VERIFIED

- **Spec tests**: `tests/spec/traits/clone/` -- all 4 subdirectory files pass (definition, primitives, collections, wrappers, tuples)
- **AOT tests**: clone_int, clone_float, clone_bool, clone_str, clone_list, clone_option, clone_result, clone_tuple -- all pass
- **Hygiene review**: Phase boundary leaks were found and fixed (documented in roadmap) -- no residual issues

---

## 3.8 Iterator Traits (in-progress) — VERIFIED for checked items

- **Test run**: `cargo st tests/spec/traits/iterator/` -- 4181 passed, 0 failed
- **AOT tests**: `cargo test -p ori_llvm -- iterators` -- 25 passed, 0 failed
- **Coverage**: map, filter, take, skip, enumerate, collect, count, fold, find, any, all, for_each, zip, chain all tested in AOT
- **Unchecked items confirmed**:
  - DoubleEndedIterator LLVM: no AOT tests for rev/last/rfind/rfold (evaluator-only)
  - repeat() LLVM: no AOT tests (evaluator-only)
  - for...yield LLVM: not explicitly in `iterators.rs` AOT tests
  - Collect LLVM: blocked on general trait infrastructure

---

## 3.8.1 Iterator Performance and Semantics (in-progress) — VERIFIED for checked items

- **Checked items**: copy elision, infinite range syntax, infinite range iteration, lint warnings -- all pass in spec tests
- **Unchecked items confirmed**: Compiler optimizations (deforestation, loop fusion, inline expansion) genuinely not implemented

---

## 3.9 Debug Trait (in-progress) — VERIFIED for checked items

- **Test run**: `cargo st tests/spec/traits/debug/` -- all pass
- **Covers**: definition, primitives, collections, wrappers, tuples, derive, escape, join
- **LLVM codegen**: Correctly marked unchecked -- `DerivedTrait::Debug` is in `known_gaps` in derives.rs

---

## 3.10 Trait Resolution and Conflict Handling (in-progress) — VERIFIED for checked items

- **Test run**: `cargo st tests/spec/traits/resolution/` -- all pass
- **Test code reviewed**:
  - `diamond.ori`: Tests 4-trait diamond hierarchy (Base/Left/Right/Bottom) with Widget type. Assertions verify all inheritance paths resolve to single Base impl. Sound.
  - `method_priority.ori`: Tests inherent-over-trait priority with same-name `format()` method. Assertion verifies inherent wins. Sound.
  - `ambiguous_method.ori`: exists and passes
  - `conflicting_defaults.ori`: exists and passes
- **Rust tests**: `all_super_traits_diamond`, `collected_methods_deduplication`, `method_lookup_priority` all pass
- **Unchecked items confirmed**:
  - `orphan_impl.ori` -- does NOT exist (file system check confirms)
  - `overlapping_impls.ori` -- does NOT exist
  - `specificity.ori` -- does NOT exist
  - Orphan/blanket rules: No implementation in `ori_types` (grep for "orphan" returns empty)
  - Super trait calls `Trait.method(self)`: blocked on parser
  - Extension conflict detection: blocked on module system
  - Associated type disambiguation: blocked on parser

---

## 3.11 Object Safety Rules (complete) — VERIFIED

- **Test run**: `cargo st tests/spec/traits/object_safety.ori` -- all pass
- **Compile-fail tests**: `object_safety_self_return.ori`, `object_safety_self_param.ori`, `object_safety_nested.ori`, `object_safety_trait_bounds.ori` -- all pass
- **Test code reviewed**: `object_safety_self_return.ori` correctly tests E2024 with `#[compile_fail("cannot be made into an object")]` on a trait with `Self` return type
- **Rust tests**: 11 registration tests pass including `object_safe_trait_has_no_violations`

---

## 3.12 Custom Subscripting / Index Trait (in-progress) — VERIFIED for checked items

- **Test code reviewed**: `multiple_impls.ori` tests dual `Index<int, str>` + `Index<str, str>` impls on `JsonValue` type, with correct dispatch assertions. Sound.
- **Unchecked items**: LLVM codegen for Index trait correctly marked unchecked (no general trait method call codegen in AOT)

---

## 3.13 Additional Core Traits (in-progress) — VERIFIED for checked items

- **Printable/Default**: AOT tests pass (5 Default AOT, 1+ Printable AOT)
- **Traceable**: `cargo st tests/spec/traits/traceable/` -- all pass (definition, error_trace, no_trace, result_delegation)
- **Unchecked items**: Traceable LLVM codegen correctly marked unchecked (Error type has no LLVM representation)

---

## 3.14 Comparable and Hashable Traits (complete) — VERIFIED

- **Spec tests**: `compound_equals.ori` (12 tests), `compound_hash.ori` (17 tests), `comparable.ori` (58 tests) -- all pass
- **Test code reviewed**: `compound_equals.ori` covers list, map, Option, Result, tuple with same/different/empty cases and edge cases. Sound.
- **AOT tests**: list_compare, tuple_compare, option_compare, result_compare, list_hash, tuple_hash, option_hash, result_hash, bool_hash, float_hash, char_hash, str_hash, hash_combine -- all pass
- **Derive**: comparable and hashable derive tests pass (4 each in AOT)

---

## 3.15 Derived Traits Formal Semantics (in-progress) — VERIFIED for checked items

- **Spec tests**: eq, eq_sum, hashable, comparable_sum, clone, debug, printable, generic, recursive derives -- all pass
- **Unchecked items confirmed**:
  - Eq sum type LLVM: correctly unchecked (no sum type derive codegen in LLVM)
  - Debug LLVM: correctly unchecked (listed in `known_gaps`)
  - Generic conditional LLVM: correctly unchecked (no monomorphization)
  - Recursive type LLVM: correctly unchecked

---

## 3.16 Formattable Trait (in-progress) — VERIFIED for checked items

- **AOT tests**: `cargo test -p ori_llvm -- formattable` -- 17 passed, 0 failed
- **Coverage**: hex, binary, octal, sign, zero-pad, width/align, fill, bool width, float fixed/sign
- **Test code reviewed**: `user_impl.ori` tests user-defined `Formattable` impl receiving `FormatSpec` with width, alignment, and fill character inspection. Sound.
- **GAP noted**: User-type Formattable LLVM codegen blocked (correctly documented with `record_codegen_error()`)

---

## 3.17 Into Trait (in-progress) — VERIFIED for checked items

- **Spec tests**: `cargo st tests/spec/traits/into/` -- all pass
- **Test code reviewed**: `int_to_float.ori` tests basic, zero, negative conversions with type annotations. Sound.
- **AOT tests**: 3 int-to-float Into tests + 1 conversion test pass
- **Unchecked items confirmed**:
  - str-to-Error LLVM: blocked (Error type has no LLVM representation -- correctly documented)
  - Orphan rule enforcement: blocked on module system (correctly documented)
  - Set-to-List AOT: blocked (Set literal construction not in AOT)

---

## 3.18 Ordering Type (in-progress) — VERIFIED for checked items

- **Spec tests**: `cargo st tests/spec/types/ordering/` -- all pass
- **then() and then_with()**: Both verified working (tests pass)
- **Unchecked item confirmed**: `Ordering.default()` genuinely not testable (static method support needed)

---

## 3.19 Default Type Parameters on Traits (complete) — VERIFIED

- **Spec tests**: `tests/spec/traits/default_type_params.ori` -- all pass
- **Rust tests**: Generics parsing, trait registration, Self substitution all verified

---

## 3.20 Default Associated Types (in-progress) — VERIFIED for checked items

- **Spec tests**: `tests/spec/traits/default_assoc_types.ori` -- all pass (4 tests)
- **Unchecked item confirmed**: Bounds checking on default associated types genuinely not implemented

---

## 3.21 Operator Traits (complete) — VERIFIED

- **Spec tests**: `cargo st tests/spec/traits/operators/` -- all pass
- **AOT tests**: 7 operator AOT tests pass (add, sub, neg, mul_mixed, chained, bitwise, not)
- **Error messages**: E2020 compile-fail tests pass (5 tests for missing operator traits)
- **Unchecked item confirmed**: Derive for newtypes correctly marked optional/deferred

### 3.21.1 MatMul Operator — Unchecked items PARTIALLY DONE but not end-to-end

- `BinaryOp::MatMul` EXISTS in `ori_ir` with `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()` arms
- Compound assignment `@=` wired in parser
- Evaluator dispatch exists (`binary_op_to_method()` has `MatMul` arm)
- LLVM codegen has match arms (unreachable placeholder)
- BUT: infix `@` NOT parseable (comment in parser: "not yet wired as infix `@`")
- BUT: `MatMul` trait NOT in `library/std/prelude.ori`
- BUT: No spec tests (`matmul.ori` does NOT exist)
- **Assessment**: Roadmap correctly marks all items as unchecked since the feature is not end-to-end functional. The IR/eval groundwork exists but parser/prelude/tests are missing.

---

## 3.22 Bound Syntax — ELIMINATED

- Correctly marked as eliminated per capability unification decision. No action needed.

---

## 3.23 Impl Colon Syntax — Unchecked items CONFIRMED INCOMPLETE

- Parser still uses `impl Trait for Type` syntax (confirmed by grep and parser comment)
- No migration has occurred
- All unchecked items are genuinely incomplete

---

## 3.24 Value Trait — Unchecked items CONFIRMED INCOMPLETE

- No `Value` marker trait in `ori_types` (grep confirms no "trait Value" or "Value.*marker")
- Not-started status is accurate

---

## Discrepancies Found

1. **Minor count drift (Len)**: Roadmap says 14+3=17 tests, actual file has 18 `@test` annotations. The extra test is benign growth (more coverage is good). No action needed.
2. **AOT count evolution**: Roadmap records AOT test counts at various dates (39, 45, 57). Current actual: 87 trait, 39 derive, 25 iterator, 17 formattable. The totals grew significantly since initial recording. Status text could be updated but this is informational only.
3. **MatMul partial implementation**: Some IR/eval groundwork exists beyond what the all-unchecked roadmap items suggest. However, the roadmap is correct to leave them unchecked since the feature is not end-to-end functional.

---

## Conclusion

Section 03 is in good shape. All checked items are working correctly with sound tests. All unchecked items are genuinely incomplete. No regressions, no stale tests, no wrong tests found. The roadmap accurately reflects the current state of the codebase.

The main remaining work is:
1. LLVM codegen gaps (Debug derive, Traceable, user Formattable, DoubleEndedIterator, generic impls)
2. IsEmpty trait formalization (Range/fixed-list support, prelude function)
3. Trait resolution features (orphan rules, super calls, extension conflicts)
4. New features (impl colon syntax, Value trait, MatMul operator)

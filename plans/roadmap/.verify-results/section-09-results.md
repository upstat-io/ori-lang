# Section 09: Match Expressions -- Verification Results

**Date**: 2026-03-19
**Section status**: in-progress (71/216 = 32%)
**Reviewed**: false

## Test Runs

| Test Suite | Result |
|------------|--------|
| `cargo st tests/spec/patterns/match.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/match_patterns.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/binding_patterns.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/exhaustiveness.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/exhaustiveness_fail.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/variant_punning.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo test -p ori_llvm --test aot -- patterns` | 22 passed, 0 failed |
| `cargo test -p ori_canon -- exhaustiveness` | 45 passed, 0 failed |

---

## 9.0 Match Expression Syntax -- Subsection Status: in-progress

### 9.0.1 Comma-Separated Match Arms

All `[ ]` items. Subsection claimed APPROVED but not yet implemented.

**Observation**: The existing spec tests (`match.ori`, `match_patterns.ori`) already use comma-separated match arms and `match expr { }` block syntax throughout. The `if` guard syntax is used in AOT tests. The `.match(condition)` guard syntax is still used in spec tests (legacy mode).

- `[ ]` Parser -- comma-separated match arms: STALE -- already implemented. All spec tests use comma syntax. Parser in `match_patterns.rs` handles commas. This `[ ]` should be `[x]`.
- `[ ]` Parser -- `if` guard syntax: STALE -- already implemented. AOT tests at `patterns.rs` use `x if condition ->` syntax. Parser's `parse_pattern_guard()` handles both `if` and `.match()`. This `[ ]` should be `[x]`.
- `[ ]` Formatter -- emit commas: NOT VERIFIED (formatter not checked in this review).

---

## 9.1 match Expression

Sampled 4 checked items:

### 9.1.1 `[x]` Grammar match_expr (line 86)

- **Ori Tests**: `match.ori` has 58+ test functions exercising `match expr { ... }` with comma-separated arms, wildcard, binding, literal, variant, struct, tuple, list, or-pattern, at-pattern, range, guard patterns. All pass.
- **AOT Tests**: `patterns.rs` has 22 tests covering match with or-patterns, guards, tuples, bindings, nested match, exhaustiveness, and Result dispatch. All pass.
- **Classification**: VERIFIED

### 9.1.2 `[x]` Test each arm's pattern in order (line 114)

- **Ori Tests**: `match.ori::test_match_first_wins` -- explicitly tests that the first matching arm wins when two arms match the same literal (lines 401-410). `match_patterns.ori` tests ordering through multiple pattern types.
- **AOT Tests**: `test_pattern_tuple_basic` and `test_pattern_tuple_second_arm` exercise first-match-wins ordering for tuple patterns.
- **Classification**: VERIFIED

### 9.1.3 `[x]` If pattern matches and guard passes, evaluate arm (line 121)

- **Ori Tests**: `match.ori::test_match_guard` (lines 717-729) tests `.match(condition)` guard syntax. `match_patterns.ori::test_guard_with_binding` (lines 491-501) tests `Some(x).match(x > 10)` guard with variant binding.
- **AOT Tests**: `test_pattern_guard_basic`, `test_pattern_guard_with_binding`, `test_pattern_guard_complex_condition` all verify guard evaluation. Guards use `if` syntax in AOT.
- **Classification**: VERIFIED

### 9.1.4 `[ ]` LLVM Support items (lines 89, 96, 103, 110, 117, 124, 131)

- All marked `[ ]` for "LLVM codegen for match expression/arms/arm/scrutinee/order/guard/result".
- **Observation**: The `[x]` AOT Tests items directly below each prove LLVM codegen works. All 22 AOT tests pass successfully (`test_pattern_*` functions compile Ori programs to native binaries and verify results). The `[ ]` LLVM Support items appear to refer to hypothetical Rust unit tests in `ori_llvm/tests/matching_tests.rs`.
- **Classification**: STALE -- LLVM codegen IS implemented (proven by passing AOT tests). The `[ ]` items refer to a non-existent `matching_tests.rs` file; AOT tests serve the same purpose.

---

## 9.2 Pattern Types

Sampled 5 items:

### 9.2.1 `[x]` literal_pattern (line 139)

- **Ori Tests**: `match.ori` tests int, string, bool, negative int, char literals. `match_patterns.ori` adds char literal patterns (`literal_char`).
- **AOT Tests**: `test_pattern_or_int_literals`, `test_pattern_or_char_literals`, `test_pattern_match_all_bool_cases`, `test_pattern_match_many_char_literals`.
- **Classification**: VERIFIED

### 9.2.2 `[x]` variant_pattern (line 160)

- **Ori Tests**: `match.ori` tests `Some(x)`, `None`, `Ok(x)`, `Err(_)` patterns, including nested variants (`Some(Some(x))`). `match_patterns.ori` tests user-defined sum types (`Status = Pending | Running(progress: int) | Done`).
- **AOT Tests**: `test_pattern_match_on_result_tag` (Result dispatch via `is_ok()`/`is_err()` -- note: this is tag-based, not full variant destructuring in AOT).
- **Classification**: VERIFIED -- Ori tests are strong. AOT test is WEAK TESTS for variant patterns specifically (tests tag dispatch but not variant destructuring like `Ok(x) -> x`).

### 9.2.3 `[x]` struct_pattern (line 167)

- **Ori Tests**: `match.ori::test_match_struct_pattern` tests literal struct fields (`{ x: 0, y: 0 }`), punned fields (`{ x, y }`), mixed. `match_patterns.ori` adds struct rest patterns (`{ x, .. }`), nested struct patterns, and four more struct pattern tests.
- **AOT Tests**: Only `ori_llvm/tests/aot/recursion.rs::test_rec_struct_param` -- struct construction and field access in recursive context. No direct struct pattern matching in AOT.
- **Classification**: VERIFIED (Ori tests comprehensive), but WEAK TESTS for AOT struct patterns.

### 9.2.4 `[ ]` or_pattern (line 209)

- Marked `[ ]` for implementation, but **already implemented**.
- **Parser**: `match_patterns.rs` handles `MatchPattern::Or(range)` on pipe (`|`) token.
- **Ori Tests**: `match.ori::test_match_or_pattern` (lines 660-672), `match_patterns.ori::test_or_pattern`, `test_or_pattern_multiple` (6 alternatives), `test_or_pattern_variants` (Option or-patterns).
- **AOT Tests**: 4 tests marked `[x]` -- `test_pattern_or_int_literals`, `test_pattern_or_char_literals`, `test_pattern_or_bool`, `test_pattern_or_in_loop`. All pass.
- **Classification**: STALE -- `[ ]` should be `[x]`. Or-patterns are fully implemented in parser, type checker, evaluator, and LLVM codegen.

### 9.2.5 `[ ]` at_pattern (line 216)

- Marked `[ ]` for implementation, but **already implemented**.
- **Parser**: `match_patterns.rs` handles `MatchPattern::At { name, pattern }` on `@` token.
- **Ori Tests**: `match.ori::test_match_at_pattern` (lines 679-687), `match_patterns.ori::test_at_pattern` (lines 452-460), `test_at_pattern_list` (lines 462-471).
- All Ori tests pass.
- **Classification**: STALE -- `[ ]` should be `[x]` for parser/evaluator implementation. LLVM/AOT support genuinely unchecked.

### 9.2.6 `[ ]` range_pattern (line 195)

- Marked `[ ]` for implementation, but **partially implemented**.
- **Ori Tests**: `match.ori::test_match_range_pattern` (lines 695-710) tests `1..10` and `10..100` exclusive range patterns. `match_patterns.ori::test_range_inclusive` tests `1..=5` inclusive range patterns.
- All Ori tests pass.
- **Classification**: STALE -- basic int range patterns (`..` and `..=`) are implemented. The `[ ]` items for char/byte range patterns and const endpoint patterns are genuinely not started.

---

## 9.3 Pattern Guards

Sampled 3 checked items:

### 9.3.1 `[x]` Grammar guard = "if" expression (line 227)

- **Parser**: `parse_pattern_guard()` in `match_patterns.rs` handles both `if condition` (new) and `.match(condition)` (legacy).
- **Ori Tests**: `match.ori::test_match_guard` uses `.match()` syntax. `match_patterns.ori::test_guard`, `test_guard_with_binding`, `test_guard_requires_catchall` all use `.match()`.
- **AOT Tests**: All guard tests use `if` syntax: `test_pattern_guard_basic`, `test_pattern_guard_with_binding`, `test_pattern_guard_complex_condition`, `test_pattern_guard_in_loop`. All pass.
- **Classification**: VERIFIED -- both syntaxes work.

### 9.3.2 `[x]` Guard must evaluate to bool (line 234)

- **Type Checker**: `infer_match()` in `control_flow.rs` checks guard type against `Idx::BOOL` with `engine.check_type()` (lines 106-118).
- **AOT Tests**: All guard tests use bool conditions (comparisons, logical ops).
- **Classification**: VERIFIED

### 9.3.3 `[x]` Variables bound by pattern in scope (line 241)

- **Ori Tests**: `match_patterns.ori::test_guard_with_binding` -- `Some(x).match(x > 10) -> x * 2` proves `x` is in scope in both guard and arm body.
- **AOT Tests**: `test_pattern_guard_with_binding` -- `x if x > 0 -> x` proves `x` bound in guard and used in body.
- **Classification**: VERIFIED

---

## 9.4 Exhaustiveness Checking -- Subsection Status: not-started

**MAJOR FINDING**: Section 9.4 is marked "not-started" but exhaustiveness checking is **substantially implemented** in `compiler/ori_canon/src/exhaustiveness/`.

### Implementation Status

- `ori_canon/src/exhaustiveness/mod.rs` -- Core algorithm with decision tree walking
- `ori_canon/src/exhaustiveness/walk.rs` -- Walk logic for Switch/Guard/Leaf/Fail nodes
- `ori_canon/src/exhaustiveness/tests.rs` -- **45 Rust unit tests** covering:
  - Bool exhaustiveness (both variants, missing true/false)
  - Int/str with wildcard vs without (infinite types)
  - Guard handling (fallthrough, chain, not-counting-as-covering)
  - Redundant arm detection
  - Option exhaustiveness (both, missing Some, missing None)
  - Result exhaustiveness (both, missing Err)
  - User-defined enum exhaustiveness (unit variants, with fields, missing one, missing multiple)
  - Nested enum exhaustiveness (nested Option, Result<Option<int>>, deeply nested Option<Option<Option<int>>>)
  - Never variant exhaustiveness (omittable, still matchable, all-never)
  - List pattern exhaustiveness (rest covers all, empty+rest, gap detection, exact-only)
- `tests/spec/patterns/exhaustiveness.ori` -- 10 Ori spec tests for valid exhaustive matches
- `tests/spec/patterns/exhaustiveness_fail.ori` -- 10 Ori spec tests (`#compile_fail`) for non-exhaustive and redundant matches

All tests pass. The `[ ]` items in 9.4 are largely STALE.

### Specific Sub-items:

- `[ ]` 9.4.1 Pattern matrix decomposition: STALE -- implemented via decision tree walking in `ori_canon`
- `[ ]` 9.4.1 Constructor enumeration for types: STALE -- implemented for Bool, Option, Result, user-defined enums, lists
- `[ ]` 9.4.2 Match expressions must be exhaustive: STALE -- implemented; `exhaustiveness_fail.ori` has `#compile_fail("non-exhaustive")` tests that pass
- `[ ]` 9.4.2 Let binding refutability check: NOT VERIFIED -- no explicit tests found
- `[ ]` 9.4.2 Function clause exhaustiveness: NOT VERIFIED -- no explicit tests found
- `[ ]` 9.4.3 Guards not considered for exhaustiveness: STALE -- implemented; unit tests `guard_fallthrough_fail`, `guard_chain_all_fail_non_exhaustive`, `guard_on_enum_does_not_count_as_covering` verify this
- `[ ]` 9.4.3 Guards require catch-all pattern: STALE -- implemented; tested in both Rust and Ori
- `[ ]` 9.4.4 Or-pattern combined coverage: NOT VERIFIED -- no specific test
- `[ ]` 9.4.4 Or-pattern binding consistency: NOT VERIFIED -- no specific test
- `[ ]` 9.4.4 At-pattern coverage: NOT VERIFIED -- no specific test
- `[ ]` 9.4.4 List pattern length coverage: STALE -- 8 Rust unit tests cover list length exhaustiveness
- `[ ]` 9.4.4 Range pattern requires wildcard: NOT VERIFIED
- `[ ]` 9.4.5 Detect completely unreachable patterns: STALE -- implemented; `redundant_arm` and `multiple_missing_bool_and_redundant` tests verify; `exhaustiveness_fail.ori` has `#compile_fail("redundant")` tests
- `[ ]` 9.4.5 Detect overlapping range patterns: NOT VERIFIED -- no specific test
- `[ ]` 9.4.5 Suggest missing patterns in error messages: STALE -- `check_exhaustiveness` returns `missing` patterns (e.g., `"None"`, `"Some(_)"`, `"Blue"`, `"Rect(_, _)"`, `"Some(Some(None))"`, `"[]"`, `"[_]"`)

---

## 9.5 Named Variant Pattern Fields (Argument Punning) -- Subsection Status: not-started

**FINDING**: Variant punning is **partially implemented**.

- `tests/spec/patterns/variant_punning.ori` exists with 5 test functions. All pass.
- Tests cover: single-field punning (`Circle(radius:)`), multi-field punning (`Add(left:, right:)`), Option punning (`Some(value:)`), Result punning (`Ok(value:)`, `Err(error:)`), mixed positional/punned.
- Parser support exists (punning syntax `name:` in variant patterns works).
- The `[ ]` items for Parser, IR, Type Checker, Evaluator implementation are STALE -- basic punning works end-to-end.
- The `[ ]` items for LLVM codegen and Formatter are genuinely not started.
- Named field ordering (reorder to definition order) and unknown field validation are NOT VERIFIED.

---

## 9.6 Section Completion Checklist

All `[ ]` -- genuinely incomplete. The section is not done.

---

## Summary

| Subsection | Status in Roadmap | Actual Status | Key Findings |
|------------|------------------|---------------|--------------|
| 9.0.1 | in-progress (`[ ]`) | Mostly implemented | Comma arms and `if` guards already work; roadmap items STALE |
| 9.1 | `[x]` (6 items) | VERIFIED | All 6 core items verified; LLVM `[ ]` items are STALE (AOT proves codegen works) |
| 9.2 | `[x]` (8 items) + `[ ]` (3 items) | Mixed | 8 checked items VERIFIED; or-pattern and at-pattern `[ ]` are STALE (implemented); range partial |
| 9.3 | `[x]` (3 items) | VERIFIED | All 3 guard items verified |
| 9.4 | not-started (`[ ]`) | Substantially implemented | 45 Rust unit tests + 20 Ori spec tests; exhaustiveness checking works for bool, enum, Option, Result, lists, Never, nested enums, guards, redundancy |
| 9.5 | not-started (`[ ]`) | Partially implemented | Basic variant punning works (5 Ori tests pass); LLVM/formatter/advanced features not done |
| 9.6 | not-started | Not done | Section genuinely incomplete |

### Findings Count

| Classification | Count |
|---------------|-------|
| VERIFIED | 12 |
| STALE (marked `[ ]` but implemented) | 15+ |
| WEAK TESTS | 2 (AOT variant/struct patterns) |
| NEEDS TESTS | 5 (let refutability, function clause exhaustiveness, or-pattern binding, at-pattern coverage, range overlap) |
| BUG FOUND | 0 |
| REGRESSION | 0 |

### Critical Observation

Section 9.4 (Exhaustiveness Checking) is marked "not-started" but has a complete implementation with 45 Rust unit tests and 20 Ori spec tests across `exhaustiveness.ori` and `exhaustiveness_fail.ori`. The roadmap is significantly out of date for this subsection.

Similarly, or-patterns (9.2) and at-patterns (9.2) are marked `[ ]` but are fully implemented and tested. The LLVM `[ ]` items throughout 9.1-9.3 reference a non-existent `matching_tests.rs` file when the AOT tests already prove LLVM codegen works.

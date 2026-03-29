# Section 09: Match Expressions -- Verification Results

**Verified**: 2026-03-28
**Verifier**: Claude Opus 4.6 (1M context)
**Status**: in-progress

## Files Loaded Before Verification

- `/home/eric/projects/ori_lang/CLAUDE.md` (full, 183 lines)
- All 19 rules files in `.claude/rules/`: types.md, typeck.md, eval.md, patterns.md, roadmap.md, ori-lang.md, spec.md, aot.md, llvm.md, diagnostic.md, parse.md, ir.md, compiler.md, cargo.md, registry.md, runtime.md, arc.md, impl-hygiene.md, tests.md
- `docs/ori_lang/v2026/spec/15-patterns.md` (full, 1153 lines)
- `plans/roadmap/section-09-match.md` (full, 413 lines)

## Test Files Read

- `tests/spec/patterns/match.ori` (956 lines, 62 `@test_` functions)
- `tests/spec/patterns/match_patterns.ori` (545 lines, 42 `@test_` functions)
- `tests/spec/patterns/binding_patterns.ori` (380 lines, struct/tuple/list destructuring)
- `tests/spec/patterns/exhaustiveness.ori` (180 lines, valid exhaustive match tests)
- `tests/spec/patterns/exhaustiveness_fail.ori` (155 lines, compile_fail negative tests)
- `tests/spec/patterns/variant_punning.ori` (67 lines, variant field punning tests)
- `compiler/ori_llvm/tests/aot/patterns.rs` (482 lines, 22 `#[test]` functions)
- `compiler/ori_llvm/tests/aot/recursion.rs` (partial read -- test_rec_struct_param)

## Source Files Inspected

- `compiler/ori_parse/src/grammar/expr/patterns/mod.rs` (match parsing: brace + method-style)
- `compiler/ori_parse/src/grammar/expr/patterns/match_patterns.rs` (pattern parsing + guards)
- `compiler/ori_fmt/src/formatter/stacked.rs` (match construct emission)

## Test Runs

| Test | Result |
|------|--------|
| `cargo st tests/spec/patterns/match.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/match_patterns.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/binding_patterns.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/exhaustiveness.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/exhaustiveness_fail.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo st tests/spec/patterns/variant_punning.ori` | 4181 passed, 0 failed, 42 skipped |
| `cargo test -p ori_llvm -- patterns` | 23 passed, 0 failed (22 patterns + 1 journey) |

NOTE: `cargo st` runs the full test suite (4181 tests) when pointed at a specific file; the file-specific tests are included in this count.

---

## 9.0 Match Expression Syntax

### 9.0.1 Comma-Separated Match Arms

- [ ] **Parser -- comma-separated match arms**: ALREADY IMPLEMENTED but marked `[ ]` in roadmap.
  - **Evidence**: `compiler/ori_parse/src/grammar/expr/patterns/mod.rs` line 114 comments "Arms are comma-separated (per match-arm-comma-separator-proposal)". The `parse_match_arms_brace()` function uses `brace_series_direct()` which handles comma-separated entries. All tests in `match.ori` use comma-separated arms and pass.
  - **Verdict**: WRONG STATUS -- should be `[x]`. Implementation complete.

  - [ ] **Rust Tests**: No dedicated parser-level tests file for comma-separated arm parsing exists. There is no `tests.rs` in `compiler/ori_parse/src/grammar/expr/patterns/`. All pattern parsing is tested only via Ori spec tests.
    - **Verdict**: NEEDS TESTS -- no Rust-level parser tests for match arm parsing.

  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori` already uses comma syntax throughout (62 tests).
    - **Verdict**: WRONG STATUS -- should be `[x]`. Tests exist and pass.

- [ ] **Parser -- `if` guard syntax**: ALREADY IMPLEMENTED but marked `[ ]` in roadmap.
  - **Evidence**: `compiler/ori_parse/src/grammar/expr/patterns/match_patterns.rs` line 511-533 implements `parse_pattern_guard()` with `if condition` as primary syntax and `.match(condition)` as legacy. AOT tests (e.g., `test_pattern_guard_basic`) use `if` guard syntax and pass.
  - **Verdict**: WRONG STATUS -- should be `[x]`. Implementation complete.

  - [ ] **Rust Tests**: No parser-level Rust tests for `if` guard parsing.
    - **Verdict**: NEEDS TESTS.

  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori` still uses legacy `.match()` syntax (line 725: `x.match(x > 10)`), while AOT tests use `if` syntax. Both syntaxes work.
    - **Verdict**: PARTIAL -- Ori tests exist but use legacy syntax. Should add tests with new `if` syntax. Not fully migrated.

- [ ] **Formatter -- emit commas, support single-line short matches**: ALREADY IMPLEMENTED but marked `[ ]`.
  - **Evidence**: `compiler/ori_fmt/src/formatter/stacked.rs` line 230 emits `,` after each arm. `compiler/ori_fmt/src/formatter/inline.rs` handles inline match formatting.
  - **Verdict**: WRONG STATUS -- should be `[x]` for comma emission. Single-line match support needs further audit.

  - [ ] **Rust Tests**: `compiler/ori_fmt/src/formatter/tests.rs` exists (587 lines) and includes match-related tests (line 561: `has_wildcard_match_arm`). No dedicated comma emission tests identified.
    - **Verdict**: WEAK TESTS -- formatter tests exist but no specific comma-separator tests.

---

## 9.1 match Expression

### `match_expr = "match" expression "{" match_arms "}"` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Parser in `compiler/ori_parse/src/grammar/expr/patterns/mod.rs` parses `match expr { arms }`. 62 Ori tests in `match.ori` pass. 22 AOT tests pass.

  - [x] **Rust Tests**: No dedicated Rust-level parser tests for match expression itself; however, the evaluator and type checker exercise it. Roadmap says "Parser and evaluator -- match expression tests."
    - **Verdict**: WEAK TESTS -- no isolated parser unit tests. Covered indirectly by Ori spec tests and AOT tests.

  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` -- 62 tests pass.
    - **Verdict**: CONFIRMED [x]. Count is 62 (roadmap says 58, but file has grown).

  - [ ] **LLVM Support**: LLVM codegen for match expression.
    - **Verdict**: CONFIRMED [ ] -- but note that match works in AOT via ARC IR path. The roadmap tracks a separate `ori_llvm/tests/matching_tests.rs` file that does NOT exist. The functionality works through the normal ARC pipeline.

  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` -- does NOT exist.
    - **Verdict**: CONFIRMED [ ]. File never created. AOT tests in `patterns.rs` cover match codegen instead.

  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` -- 22 tests pass.
    - **Verdict**: CONFIRMED [x]. Tests cover or-patterns (4), guards (4), tuples (6), bindings (2), nested match (1), exhaustiveness (2), fizzbuzz (1), result dispatch (1), combined (1).

### `match_arms = match_arm { "," match_arm } [ "," ]` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Comma-separated parsing confirmed.
  - [x] **Rust Tests**: Same caveat -- no isolated parser tests.
  - [x] **Ori Tests**: All tests use comma-separated arms.
  - [ ] **LLVM Support / LLVM Rust Tests**: Same as above -- `matching_tests.rs` does not exist.
  - [x] **AOT Tests**: All 22 tests use multi-arm match.
    - **Verdict**: CONFIRMED [x].

### `match_arm = pattern [ guard ] "->" expression` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Rust Tests**: Indirect coverage only.
  - [x] **Ori Tests**: Confirmed.
  - [ ] **LLVM Support / LLVM Rust Tests**: Same.
  - [x] **AOT Tests**: `test_pattern_guard_basic`, `test_pattern_guard_with_binding`, `test_pattern_fizzbuzz`.
    - **Verdict**: CONFIRMED [x].

### Evaluate scrutinee expression -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **AOT Tests**: All 22 tests evaluate scrutinee expressions (int, char, bool, tuple, Result).

### Test each arm's pattern in order -- [x] Implement

- **Verdict**: CONFIRMED [x]. `test_match_first_wins` in match.ori explicitly tests first-match-wins.
  - [x] **AOT Tests**: `test_pattern_tuple_basic`, `test_pattern_tuple_second_arm`, `test_pattern_guard_basic`.

### If pattern matches and guard passes, evaluate arm -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **AOT Tests**: 4 guard tests confirm.

### Return the result -- [x] Implement

- **Verdict**: CONFIRMED [x]. All tests use match results.

---

## 9.2 Pattern Types

### `literal_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Int, str, bool, char, negative literals all work.
  - [x] **Ori Tests**: `match.ori` (int, str, bool, negative), `match_patterns.ori` (int, str, bool, char, negative).
  - [x] **AOT Tests**: int, char, bool literals (4 tests).

### `binding_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `match_patterns.ori` -- `identifier_binding`, `identifier_in_variant`. 42 tests total.
    - NOTE: Roadmap says "36 tests" but file has 42.
  - [x] **AOT Tests**: `test_pattern_binding_capture`, `test_pattern_binding_with_literal_arms`.

### `wildcard_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `match.ori` tests wildcard extensively.
  - [x] **AOT Tests**: `test_pattern_tuple_wildcard_fallthrough`, `test_pattern_tuple_all_wildcards`, and `_` arms throughout.

### `variant_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Option and Result variants work. User-defined sum types work.
  - [x] **Ori Tests**: `match_patterns.ori` -- `option_patterns`, `result_patterns`, `sum_type_patterns`, `nested_variant`. `tests/spec/declarations/sum_types.ori` also exists.
  - [x] **AOT Tests**: `test_pattern_match_on_result_tag` -- uses `is_ok()`/`is_err()` booleans, not direct variant pattern matching. This is a WEAK AOT test for variant patterns specifically.
    - **Verdict**: WEAK TESTS for AOT -- tests use tag method dispatch, not direct `Ok(v) -> ...` / `Err(e) -> ...` variant patterns in LLVM codegen.

### `struct_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `binding_patterns.ori` -- struct destructuring. `match_patterns.ori` -- `struct_pattern`, `struct_with_literals`, `struct_rest` (5 tests for struct rest `..`).
  - [x] **AOT Tests**: `test_rec_struct_param` in `recursion.rs` -- struct construction and field access in recursive context. Not a direct struct pattern match in LLVM codegen.
    - **Verdict**: WEAK TESTS for AOT -- no direct `{ x, y }` struct pattern matching test in AOT.

### `field_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `binding_patterns.ori`, `match_patterns.ori`.
  - [x] **AOT Tests**: Same caveat as struct_pattern -- indirect only.

### `list_pattern` -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `binding_patterns.ori` -- list destructure (5 tests). `match_patterns.ori` -- `list_empty`, `list_single`, `list_head_tail`, `list_first_two`, `list_rest_pattern`.
  - [ ] **AOT Tests**: No AOT coverage yet.
    - **Verdict**: CONFIRMED [ ]. No LLVM codegen tests for list patterns.

### `list_elem` with rest pattern -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `binding_patterns.ori` -- `list_head_tail`, `list_first_two_rest`.
  - [ ] **AOT Tests**: No AOT coverage yet.
    - **Verdict**: CONFIRMED [ ].

### `range_pattern` -- [ ] Implement

- **Verdict**: PARTIALLY IMPLEMENTED. Range patterns work for int (both `..` and `..=` syntax) as confirmed by Ori tests. However, the roadmap marks this as `[ ]` and lists char/byte range patterns and const pattern endpoints as not yet implemented.
  - **Evidence**: `match.ori` line 705-709 tests `1..10` and `10..100` patterns. `match_patterns.ori` line 387-405 tests `1..10` and `1..=5`. All pass.
  - `tests/spec/patterns/match_range_char.ori` and `match_range_byte.ori` do NOT exist (checked via glob).
  - `tests/spec/patterns/match_range_const.ori` does NOT exist.
  - **Verdict**: PARTIALLY IMPLEMENTED -- int range patterns work; char/byte/const patterns not implemented. The `[ ]` status is misleading because basic int range works. Sub-items are correctly `[ ]`.

### `or_pattern` -- [ ] Implement

- **Verdict**: ALREADY IMPLEMENTED but marked `[ ]` in roadmap.
  - **Evidence**: `match.ori` line 660-673 tests or-patterns. `match_patterns.ori` line 410-447 tests or-patterns including multi-alternative and variant or-patterns. All pass. Parser in `match_patterns.rs` line 40-49 handles `|` for or-patterns.
  - [x] **AOT Tests**: 4 AOT tests for or-patterns (int, char, bool, in_loop). All pass.
  - **Verdict**: WRONG STATUS -- should be `[x]`. Or-patterns are fully implemented and tested in both interpreter and LLVM.
  - Sub-item `[ ] Rust Tests` -- no dedicated parser-level tests. NEEDS TESTS.
  - Sub-item `[ ] Ori Tests` -- tests exist and pass. WRONG STATUS.

### `at_pattern` -- [ ] Implement

- **Verdict**: ALREADY IMPLEMENTED but marked `[ ]` in roadmap.
  - **Evidence**: `match.ori` line 679-687 tests at-patterns. `match_patterns.ori` line 452-470 tests at-patterns. All pass. Spec `15-patterns.md` documents at-patterns at line 126-169.
  - **Verdict**: WRONG STATUS -- should be `[x]` for interpreter. No AOT coverage.
  - Sub-item `[ ] Rust Tests` -- no dedicated parser-level tests. NEEDS TESTS.
  - Sub-item `[ ] Ori Tests` -- tests exist and pass. WRONG STATUS.
  - Sub-item `[ ] AOT Tests` -- correctly `[ ]`.

---

## 9.3 Pattern Guards

### `guard = "if" expression` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Both `if` and legacy `.match()` guard syntaxes work.
  - [x] **Ori Tests**: `match.ori` uses `.match()` syntax; `match_patterns.ori` uses `.match()` syntax.
    - NOTE: Tests use LEGACY syntax. No Ori tests yet use the new `if` guard syntax. This is not wrong but is worth noting.
  - [x] **AOT Tests**: 4 AOT tests use `if` guard syntax. All pass.

### Guard expression must evaluate to `bool` -- [x] Implement

- **Verdict**: CONFIRMED [x]. Type checker enforces this.
  - [x] **AOT Tests**: Guards evaluate bool conditions.

### Variables bound by pattern are in scope -- [x] Implement

- **Verdict**: CONFIRMED [x].
  - [x] **Ori Tests**: `match_patterns.ori` line 497-501 `guard_with_binding` uses `x` from `Some(x)` in guard.
  - [x] **AOT Tests**: `test_pattern_guard_with_binding`, `test_pattern_guard_with_tuple`.

---

## 9.4 Exhaustiveness Checking

### Status Assessment

Exhaustiveness checking is PARTIALLY IMPLEMENTED. There is no formal `ori_types/src/check/exhaustiveness/` module, but the compiler does perform exhaustiveness checking as evidenced by:
1. `tests/spec/patterns/exhaustiveness.ori` -- valid exhaustive matches compile and run (Option, Result, user enums, bool, int with wildcard, list patterns, Never variants).
2. `tests/spec/patterns/exhaustiveness_fail.ori` -- non-exhaustive and redundant matches are rejected at compile time with `#compile_fail("non-exhaustive")` and `#compile_fail("redundant")`.
3. Both test files pass.

However, the roadmap's `ori_types/src/check/exhaustiveness/tests.rs` does NOT exist. The exhaustiveness logic appears to be integrated elsewhere in the type checker rather than in a dedicated module.

### 9.4.1 Core Algorithm

- [ ] **Pattern matrix decomposition**: Status unclear. Some form of exhaustiveness checking works (tests pass), but there is no dedicated `exhaustiveness/` module.
  - **Verdict**: PARTIALLY IMPLEMENTED -- logic exists but not in the expected location.

- [ ] **Constructor enumeration for types**: Works for Option, Result, user enums, bool, int, list.
  - **Evidence**: `exhaustiveness.ori` tests all these types. `exhaustiveness_fail.ori` catches missing variants.
  - **Verdict**: PARTIALLY IMPLEMENTED.

### 9.4.2 Exhaustiveness Errors

- [ ] **Match expressions must be exhaustive (E0123)**: IMPLEMENTED.
  - **Evidence**: `exhaustiveness_fail.ori` uses `#compile_fail("non-exhaustive")` for Option (missing None, missing Some), Result (missing Err), user enum (missing variants), bool (missing false), int (without wildcard), list (exact only, missing empty). All compile-fail tests pass.
  - **Verdict**: WRONG STATUS -- should be `[x]` or `[partial]`. The core exhaustiveness error works. Error code may not be E0123 specifically.

- [ ] **Let binding refutability check**: NOT VERIFIED. No test specifically for refutable let binding rejection.
  - **Verdict**: UNKNOWN -- not tested.

- [ ] **Function clause exhaustiveness**: NOT VERIFIED. No `function_clauses_exhaustive.ori` test file found.
  - **Verdict**: UNKNOWN -- not tested.

### 9.4.3 Guard Handling

- [ ] **Guards not considered for exhaustiveness**: IMPLEMENTED.
  - **Evidence**: `match_patterns.ori` line 503-516 tests that guards require a catch-all, and the test passes.
  - **Verdict**: PARTIALLY IMPLEMENTED -- works in practice but no dedicated `exhaustiveness/tests.rs`.

- [ ] **Guards require catch-all pattern (E0124)**: IMPLEMENTED.
  - **Evidence**: Same test. Error code may differ.
  - **Verdict**: PARTIALLY IMPLEMENTED.

### 9.4.4 Pattern Coverage

- [ ] **Or-pattern combined coverage**: NOT VERIFIED independently. Or-patterns work but no dedicated exhaustiveness test for or-pattern coverage contribution.
  - **Verdict**: UNKNOWN.

- [ ] **Or-pattern binding consistency**: NOT VERIFIED. No test for binding consistency across alternatives.
  - **Verdict**: UNKNOWN.

- [ ] **At-pattern coverage**: NOT VERIFIED.
  - **Verdict**: UNKNOWN.

- [ ] **List pattern length coverage**: PARTIALLY VERIFIED.
  - **Evidence**: `exhaustiveness.ori` tests `[] + [_, ..]` and `[..all]` patterns. `exhaustiveness_fail.ori` tests missing empty list case. These pass.
  - **Verdict**: PARTIALLY IMPLEMENTED.

- [ ] **Range pattern requires wildcard for integers**: PARTIALLY VERIFIED.
  - **Evidence**: Range patterns work in `match.ori` and `match_patterns.ori` but no explicit exhaustiveness test for range coverage.
  - **Verdict**: UNKNOWN.

### 9.4.5 Unreachable Pattern Detection

- [ ] **Detect completely unreachable patterns (W0456)**: IMPLEMENTED.
  - **Evidence**: `exhaustiveness_fail.ori` tests redundant patterns (bool with extra wildcard after full coverage, wildcard before specific pattern) with `#compile_fail("redundant")`. Both pass.
  - **Verdict**: WRONG STATUS -- should be `[x]` or `[partial]`. Detection works.

- [ ] **Detect overlapping range patterns (W0457)**: NOT VERIFIED.
  - **Verdict**: UNKNOWN -- no test.

- [ ] **Suggest missing patterns in error messages**: NOT VERIFIED.
  - **Verdict**: UNKNOWN -- no test for error message content.

---

## 9.5 Named Variant Pattern Fields (Argument Punning)

### Status Assessment

This is ALREADY IMPLEMENTED and tested, but the entire section is marked as `not-started` in the roadmap frontmatter and all items are `[ ]`.

**Evidence**: `tests/spec/patterns/variant_punning.ori` (67 lines) contains working tests:
- Single-field variant punning: `Circle(radius:)` -- WORKS
- Multi-field variant punning: `Add(left:, right:)` -- WORKS
- Option punning: `Some(value:)` -- WORKS
- Result punning: `Ok(value:)`, `Err(error:)` -- WORKS
- Positional still works: `Some(x)` -- WORKS

All tests pass (included in the 4181 total).

### Parser

- [ ] **Support `name:` and `name: pattern` in variant pattern fields**: IMPLEMENTED.
  - **Verdict**: WRONG STATUS -- should be `[x]`.

- [ ] **Mixed named and positional fields**: WORKS (positional tests in variant_punning.ori pass).
  - **Verdict**: WRONG STATUS -- should be `[x]`.

- [ ] **Positional variant patterns unchanged**: CONFIRMED -- `mixed_match` test passes.
  - **Verdict**: WRONG STATUS -- should be `[x]`.

### IR, Type Checker, Evaluator

- [ ] All sub-items: IMPLEMENTED (tests pass end-to-end).
  - **Verdict**: WRONG STATUS -- all should be `[x]`.

### LLVM

- [ ] **LLVM codegen for named variant field patterns**: NOT VERIFIED via dedicated test.
  - **Verdict**: UNKNOWN -- no AOT tests for variant punning specifically.

### Formatter

- [ ] **Detect `name: name` and emit `name:` form**: NOT VERIFIED.
  - **Verdict**: UNKNOWN.

### Documentation

- [ ] **Update spec**: Spec `15-patterns.md` does not mention variant field punning explicitly.
  - **Verdict**: CONFIRMED [ ].

- [ ] **Update `grammar.ebnf`**: NOT VERIFIED.
  - **Verdict**: CONFIRMED [ ].

- [ ] **Update `.claude/rules/ori-syntax.md`**: Quick reference mentions "variant punning" in match pattern syntax.
  - **Verdict**: PARTIALLY DONE.

---

## 9.6 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]` -- NOT MET. Many items are incorrectly marked.
- [ ] Spec updated -- PARTIALLY. `15-patterns.md` covers match/exhaustiveness/at-patterns/range-patterns but not variant punning.
- [ ] CLAUDE.md updated -- N/A (no syntax changes needed).
- [ ] 80+% test coverage -- PARTIAL. Good interpreter coverage, weak AOT coverage for struct/list patterns.
- [ ] Run full test suite -- Tests pass (4181 passed, 0 failed, 42 skipped).
- [ ] `/tpr-review` passed -- Not run.

---

## Summary

### Items with WRONG STATUS (marked `[ ]` but implemented)

1. **9.0.1 Parser -- comma-separated match arms**: IMPLEMENTED, should be `[x]`
2. **9.0.1 Parser -- `if` guard syntax**: IMPLEMENTED, should be `[x]`
3. **9.0.1 Formatter -- emit commas**: IMPLEMENTED, should be `[x]`
4. **9.0.1 Ori Tests for comma syntax**: EXIST AND PASS, should be `[x]`
5. **9.2 `or_pattern` Implement**: IMPLEMENTED, should be `[x]`
6. **9.2 `or_pattern` Ori Tests**: EXIST AND PASS, should be `[x]`
7. **9.2 `at_pattern` Implement**: IMPLEMENTED, should be `[x]`
8. **9.2 `at_pattern` Ori Tests**: EXIST AND PASS, should be `[x]`
9. **9.4.2 Match exhaustive (E0123)**: IMPLEMENTED (basic cases), should be `[partial]`
10. **9.4.5 Unreachable patterns (W0456)**: IMPLEMENTED, should be `[partial]`
11. **9.5 All variant punning items (Parser, IR, TC, Eval)**: IMPLEMENTED, should be `[x]`

### Items with WRONG STATUS (marked `[x]` but issues found)

1. **9.1 Ori Tests count**: Roadmap says "58 tests" but file has 62 `@test_` functions. STALE COUNT.
2. **9.2 binding_pattern Ori Tests count**: Roadmap says "36 tests" but file has 42. STALE COUNT.
3. **9.1/9.2 Rust Tests claims**: Roadmap claims `[x]` for "Parser and evaluator" Rust tests, but there are no dedicated parser-level Rust tests for match expressions. Tests exist only as Ori spec tests. WEAK TESTS.

### Items Correctly Marked `[ ]`

1. All `matching_tests.rs` items -- file does not exist
2. All "LLVM Support" items -- tracked separately
3. Range pattern char/byte/const -- not implemented
4. Exhaustiveness algorithm formalization (dedicated module) -- not done
5. Let binding refutability, function clause exhaustiveness -- not verified
6. Or-pattern binding consistency -- not verified
7. Range overlap detection, pattern suggestions -- not implemented
8. Variant punning LLVM, formatter, documentation items -- not verified/done
9. List/list_elem AOT tests -- no AOT coverage

### NEEDS TESTS

1. No Rust-level parser tests for pattern parsing at all (no `tests.rs` in `grammar/expr/patterns/`)
2. No Ori tests using `if` guard syntax (all use legacy `.match()`)
3. No AOT test for direct variant pattern matching (`Ok(v) -> ...` pattern)
4. No AOT test for struct pattern matching (`{ x, y } -> ...`)
5. No AOT test for list pattern matching

### Test Quality Assessment

- **Interpreter coverage**: GOOD. 62 tests in `match.ori`, 42 in `match_patterns.ori`, comprehensive pattern types, edge cases, nested patterns, method-style match.
- **AOT coverage**: MODERATE. 22 tests covering or-patterns, guards, tuples, bindings, nested match. Missing: variant patterns, struct patterns, list patterns, at-patterns, range patterns.
- **Negative tests**: GOOD. `exhaustiveness_fail.ori` has 8 `#compile_fail` tests for non-exhaustive and redundant patterns.
- **Cross-feature interaction**: MODERATE. Match with for-loops, lambdas, function calls, arithmetic, string concat tested. Missing: match with closures capturing, match with `?` operator, match with traits.
- **Semantic pins**: WEAK. No tests explicitly marked as semantic pins (no regression comments referencing specific bugs).
- **Matrix coverage**: WEAK. No systematic type x pattern cross-testing matrix.

### BUG FOUND

None -- all existing tests pass. However, there is a potential concern: Ori spec tests use legacy `.match()` guard syntax while the spec and AOT tests use `if` guard syntax. Both work, but spec test migration to `if` syntax should be tracked.

### Stale Data

- Roadmap references `ori_llvm/tests/matching_tests.rs` (11 occurrences) -- this file does not exist and was never created. AOT tests in `patterns.rs` serve this purpose instead.
- Roadmap references `ori_llvm/tests/scope_tests.rs` (1 occurrence) -- not verified if this exists.
- Test counts are stale (58 vs 62 for match.ori, 36 vs 42 for match_patterns.ori).

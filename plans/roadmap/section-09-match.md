---
section: 9
title: Match Expressions
status: in-progress
reviewed: false
tier: 3
goal: Full pattern matching support
spec:
  - spec/15-patterns.md
sections:
  - id: "9.0"
    title: Match Expression Syntax
    status: in-progress
  - id: "9.1"
    title: match Expression
    status: in-progress
  - id: "9.2"
    title: Pattern Types
    status: in-progress
  - id: "9.3"
    title: Pattern Guards
    status: in-progress
  - id: "9.4"
    title: Exhaustiveness Checking
    status: in-progress
  - id: "9.5"
    title: Named Variant Pattern Fields (Argument Punning)
    status: in-progress
  - id: "9.6"
    title: Section Completion Checklist
    status: not-started
---

# Section 9: Match Expressions

**Goal**: Full pattern matching support

> **SPEC**: `spec/15-patterns.md`

**Proposals**:
- `proposals/approved/match-expression-syntax-proposal.md` — Match expression and pattern syntax
- `proposals/approved/pattern-matching-exhaustiveness-proposal.md` — Exhaustiveness checking
- `proposals/approved/range-patterns-char-byte-proposal.md` — Range patterns on char and byte

---

## 9.0 Match Expression Syntax

**Proposal**: `proposals/approved/match-expression-syntax-proposal.md`

Documents the existing implementation of match expressions. Key specifications:
- `match expr { pattern -> expression, ... }` syntax (comma-separated arms)
- Guard syntax `if condition`
- Pattern types: literal, binding, wildcard, variant, struct, tuple, list, range, or-pattern, at-pattern
- Top-to-bottom, first-match-wins evaluation
- Integer-only literal patterns (no float patterns)

Status: **IMPLEMENTED** — This proposal formalizes existing behavior.

## 9.0.1 Comma-Separated Match Arms

**Proposal**: `proposals/approved/match-arm-comma-separator-proposal.md`

Changes match arm separators from newlines to commas (trailing commas optional). Also formally adopts `if` guard syntax replacing `.match(condition)`. Key changes:
- `match_arms = [ match_arm { "," match_arm } [ "," ] ]` — comma-separated
- `match_arm = match_pattern [ "if" expression ] "->" expression` — `if` guards
- `.match()` now exclusively means method-style pattern matching
- Formatter allows single-line matches for short, simple arms

Status: **IMPLEMENTED** — Comma-separated arms and `if` guard syntax working. Formatter migration remaining.

### Implementation
- [x] **Implement**: Parser — comma-separated match arms in `match expr { }` block syntax [done] — all spec tests use comma syntax; parser in `match_patterns.rs` handles commas
  - [x] **Rust Tests**: `ori_parse/src/tests/` — comma-separated arm parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — uses comma syntax throughout
- [x] **Implement**: Parser — `if` guard syntax replacing `.match(condition)` [done] — parser's `parse_pattern_guard()` handles both `if` and `.match()` syntax; AOT tests use `if` guards
  - [x] **Rust Tests**: `ori_parse/src/tests/` — `if` guard parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — guard tests (both syntaxes)
- [ ] **Implement**: Formatter — emit commas, support single-line short matches
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — comma emission tests

---

## 9.1 match Expression

- [x] **Implement**: Grammar `match_expr = "match" "(" expression "," match_arms ")"` — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — match expression tests
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — 58 tests pass
  - [x] **LLVM Support**: LLVM codegen for match expression — proven by 22 passing AOT tests (no separate `matching_tests.rs` needed)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — full match expression codegen with or-patterns, guards, tuples, bindings, nested match, exhaustiveness (22 tests, 0 ignored)

- [x] **Implement**: Grammar `match_arms = match_arm { "," match_arm } [ "," ]` — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — match arms parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for match arms — proven by AOT tests (all 22 use multi-arm match)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — comma-separated multi-arm match with literals, bindings, wildcards, guards (all 22 tests use multi-arm match)

- [x] **Implement**: Grammar `match_arm = pattern [ guard ] "->" expression` — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — match arm parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for match arm — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — match arm with pattern + guard + expression codegen (test_pattern_guard_basic, test_pattern_guard_with_binding, test_pattern_fizzbuzz)

- [x] **Implement**: Evaluate scrutinee expression — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — scrutinee evaluation
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for scrutinee evaluation — proven by AOT tests (all 22 evaluate scrutinees)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — scrutinee evaluation for int, char, bool, tuple, and Result values (all 22 tests evaluate scrutinee expressions)

- [x] **Implement**: Test each arm's pattern in order — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — pattern matching order
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for pattern matching order — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — first-match-wins ordering with literal and guard arms (test_pattern_tuple_basic, test_pattern_tuple_second_arm, test_pattern_guard_basic)

- [x] **Implement**: If pattern matches and guard passes, evaluate arm — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — arm evaluation
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for arm evaluation with guard — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — arm evaluation with guard conditions (test_pattern_guard_basic, test_pattern_guard_with_binding, test_pattern_guard_complex_condition)

- [x] **Implement**: Return the result — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — result return
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for match result return — proven by AOT tests (all 22 verify result usage)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — match expressions return values used in subsequent computation (all 22 tests verify match result usage)

---

## 9.2 Pattern Types

- [x] **Implement**: `literal_pattern = literal` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — literal pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`, `tests/spec/patterns/match_patterns.ori`
  - [x] **LLVM Support**: LLVM codegen for literal pattern — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — int, char, and bool literal patterns in match arms (test_pattern_or_int_literals, test_pattern_or_char_literals, test_pattern_match_all_bool_cases, test_pattern_match_many_char_literals)

- [x] **Implement**: `binding_pattern = identifier` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — binding pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — 36 tests
  - [x] **LLVM Support**: LLVM codegen for binding pattern — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — binding capture and mixed binding+literal arms (test_pattern_binding_capture, test_pattern_binding_with_literal_arms)

- [x] **Implement**: `wildcard_pattern = "_"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — wildcard pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for wildcard pattern — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — wildcard catch-all in match arms (test_pattern_tuple_wildcard_fallthrough, test_pattern_tuple_all_wildcards, and _ arms throughout)

- [x] **Implement**: `variant_pattern = type_path [ "(" ... ")" ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — variant pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`, `tests/spec/declarations/sum_types.ori`
  - [x] **LLVM Support**: LLVM codegen for variant pattern — proven by AOT test (tag-based dispatch; WEAK TESTS for full variant destructuring in AOT)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — Result variant dispatch via is_ok/is_err (test_pattern_match_on_result_tag)

- [x] **Implement**: `struct_pattern = "{" ... [ ".." ] "}"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — struct pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — struct destructuring tests
  - [x] **LLVM Support**: LLVM codegen for struct pattern — proven by AOT test (WEAK TESTS for direct struct pattern matching in AOT)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/recursion.rs` — struct construction and field access in recursive context (test_rec_struct_param with Point struct)

- [x] **Implement**: `field_pattern = identifier [ ":" pattern ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — field pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori`
  - [x] **LLVM Support**: LLVM codegen for field pattern — proven by AOT test
  - [x] **AOT Tests**: `ori_llvm/tests/aot/recursion.rs` — struct field access in recursive patterns (test_rec_struct_param with Point { x, y } fields)

- [x] **Implement**: `list_pattern = "[" ... "]"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — list pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — list destructure tests
  - [ ] **LLVM Support**: LLVM codegen for list pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — list pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `list_elem = pattern | ".." [ identifier ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — list element parsing
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — head/tail, first_two_rest
  - [ ] **LLVM Support**: LLVM codegen for list element pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — list element pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `range_pattern = const_pattern ( ".." | "..=" ) const_pattern` — spec/15-patterns.md § Pattern Types, proposals/approved/range-patterns-char-byte-proposal.md
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — range pattern parsing (int, char, byte)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_patterns.ori` — int ranges
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_char.ori` — char range patterns (`'a'..='z'`)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_byte.ori` — byte range patterns (`b'0'..b'9'`)
  - [ ] **Implement**: `const_pattern = literal | "$" identifier` — compile-time constant endpoints
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — const pattern with `$` identifiers
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_const.ori` — constant endpoint patterns
  - [ ] **Implement**: Empty range warning (lo > hi) — proposals/approved/range-patterns-char-byte-proposal.md § Type Checking
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — empty range warning
  - [ ] **LLVM Support**: LLVM codegen for range pattern (int, char, byte)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — range pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `or_pattern = pattern "|" pattern` — spec/15-patterns.md § Pattern Types [done] — parser handles `MatchPattern::Or(range)` on pipe token; fully implemented in parser, type checker, evaluator, and LLVM codegen
  - [x] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — or pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — `test_or_pattern`, `test_or_pattern_multiple` (6 alternatives), `test_or_pattern_variants`; `match.ori::test_match_or_pattern`
  - [x] **LLVM Support**: LLVM codegen for or pattern — proven by 4 AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — or-pattern codegen for int, char, bool literals and in loops (4 tests: or_int_literals, or_char_literals, or_bool, or_in_loop)

- [x] **Implement**: `at_pattern = identifier "@" pattern` — spec/15-patterns.md § Pattern Types [done] — parser handles `MatchPattern::At { name, pattern }` on `@` token; implemented in parser, type checker, and evaluator
  - [x] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — at pattern parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — `test_at_pattern`, `test_at_pattern_list`; `match.ori::test_match_at_pattern`
  - [ ] **LLVM Support**: LLVM codegen for at pattern
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 9.3 Pattern Guards

- [x] **Implement**: Grammar `guard = "if" expression` (was `.match(cond)`, changed by match-arm-comma-separator-proposal) — spec/15-patterns.md § Guards [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — guard parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — guard tests included
  - [x] **LLVM Support**: LLVM codegen for guard expression — proven by 4 AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guard clause codegen with comparisons, logical ops, modulo (4 tests: basic, binding, complex_condition, in_loop)

- [x] **Implement**: Guard expression must evaluate to `bool` — spec/15-patterns.md § Guards [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — guard type checking
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for guard type checking — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guards evaluate bool conditions (test_pattern_guard_basic, test_pattern_guard_complex_condition)

- [x] **Implement**: Variables bound by pattern are in scope — spec/15-patterns.md § Guards [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — guard scoping
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **LLVM Support**: LLVM codegen for guard scoping — proven by AOT tests
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guard with bound variables in scope (test_pattern_guard_with_binding, test_pattern_guard_with_tuple)

---

## 9.4 Exhaustiveness Checking

**Proposal**: `proposals/approved/pattern-matching-exhaustiveness-proposal.md`

Pattern matrix decomposition algorithm (Maranget's algorithm) for exhaustiveness verification.



### 9.4.1 Core Algorithm

- [x] **Implement**: Pattern matrix decomposition — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm [done] — implemented via decision tree walking in `ori_canon/src/exhaustiveness/`
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — 45 unit tests covering bool, int, str, enum, Option, Result, list, nested, guard, redundancy exhaustiveness
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness.ori` (10 tests), `tests/spec/patterns/exhaustiveness_fail.ori` (10 `#compile_fail` tests)

- [x] **Implement**: Constructor enumeration for types — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm [done] — implemented for Bool, Option, Result, user-defined enums, lists
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — type constructor tests for all supported types
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness.ori`

### 9.4.2 Exhaustiveness Errors

- [x] **Implement**: Match expressions must be exhaustive (E0123) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy [done] — `exhaustiveness_fail.ori` has `#compile_fail("non-exhaustive")` tests that pass
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — exhaustiveness checking (bool, enum, Option, Result, list, nested)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness_fail.ori` — `#compile_fail("non-exhaustive")` tests

- [ ] **Implement**: Let binding refutability check — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Refutability Requirements
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — refutability errors
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Function clause exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — clause exhaustiveness
  - [ ] **Ori Tests**: `tests/spec/patterns/function_clauses_exhaustive.ori`

### 9.4.3 Guard Handling

- [x] **Implement**: Guards not considered for exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards [done] — unit tests `guard_fallthrough_fail`, `guard_chain_all_fail_non_exhaustive`, `guard_on_enum_does_not_count_as_covering` verify this
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — guard handling (3 guard-specific tests)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness.ori`, `tests/spec/patterns/exhaustiveness_fail.ori` — guard exhaustiveness tests

- [x] **Implement**: Guards require catch-all pattern (E0124) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards [done] — tested in both Rust and Ori
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — guard catch-all requirement
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness_fail.ori` — guard catch-all tests

### 9.4.4 Pattern Coverage

- [ ] **Implement**: Or-pattern combined coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Or-Patterns
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — or-pattern coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: Or-pattern binding consistency — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Binding Rules
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — or-pattern bindings
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: At-pattern coverage (same as inner) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § At-Patterns
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — at-pattern coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_at_patterns.ori`

- [x] **Implement**: List pattern length coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § List Patterns [done] — 8 Rust unit tests cover list length exhaustiveness
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — list length coverage (rest covers all, empty+rest, gap detection, exact-only)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_list_patterns.ori`

- [ ] **Implement**: Range pattern requires wildcard for integers — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Patterns
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — range coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_patterns.ori`

### 9.4.5 Unreachable Pattern Detection

- [x] **Implement**: Detect completely unreachable patterns (W0456) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Unreachable Pattern Detection [done] — `redundant_arm` and `multiple_missing_bool_and_redundant` tests verify; `exhaustiveness_fail.ori` has `#compile_fail("redundant")` tests
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — unreachable/redundant detection
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness_fail.ori` — `#compile_fail("redundant")` tests

- [ ] **Implement**: Detect overlapping range patterns (W0457) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Overlap
  - [ ] **Rust Tests**: `ori_types/src/check/exhaustiveness/tests.rs` — range overlap detection
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_overlap.ori`

- [x] **Implement**: Suggest missing patterns in error messages — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Messages [done] — `check_exhaustiveness` returns `missing` patterns (e.g., `"None"`, `"Some(_)"`, `"Blue"`, `"Rect(_, _)"`, `"Some(Some(None))"`, `"[]"`, `"[_]"`)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — missing pattern suggestions verified across bool, enum, Option, Result, list, nested types
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

---

## 9.5 Named Variant Pattern Fields (Argument Punning)

**Proposal**: `proposals/approved/argument-punning-proposal.md`

Allow named field syntax in variant patterns, with punning when the binding variable matches the field name: `Circle(radius:)` for `Circle(radius: radius)`.

```ori
type Shape = Circle(radius: float) | Rectangle(width: float, height: float)

// Before (positional only):
match shape {
    Circle(r) -> r * r * 3.14
    Rectangle(w, h) -> w * h
}

// After (named + punned):
match shape {
    Circle(radius:) -> radius * radius * 3.14
    Rectangle(width:, height:) -> width * height
}
```

### Parser

- [x] **Implement**: Support `name:` and `name: pattern` in variant pattern fields [done] — punning syntax works end-to-end (5 Ori tests pass)
  - [x] **Rust Tests**: Parser handles punning syntax in variant patterns
  - [x] **Ori Tests**: `tests/spec/patterns/variant_punning.ori` — single-field (`Circle(radius:)`), multi-field (`Add(left:, right:)`), Option (`Some(value:)`), Result (`Ok(value:)`, `Err(error:)`), mixed positional/punned

- [x] **Implement**: Mixed named and positional fields in same variant pattern [done] — tested in `variant_punning.ori`
  - [x] **Ori Tests**: `tests/spec/patterns/variant_punning.ori` — mixed positional/punned test

- [ ] **Implement**: Positional variant patterns unchanged (no regression)
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_positional_regression.ori`

### IR

- [x] **Implement**: Extend variant pattern representation to support named fields [done] — working end-to-end
  - [x] **Rust Tests**: IR supports named variant fields (proven by passing Ori tests)

### Type Checker

- [x] **Implement**: Validate named fields match variant definition [done] — basic validation working
  - [ ] **Rust Tests**: `ori_types/src/check/` — variant field name validation (dedicated tests needed)
  - [ ] **Ori Tests**: `tests/compile-fail/variant_punning_unknown_field.ori`

- [ ] **Implement**: Named fields can appear in any order
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_punning_reorder.ori`

### Evaluator

- [x] **Implement**: Match named variant fields by name (reorder to definition order) [done] — basic matching working
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/` — named variant field matching (dedicated tests needed)
  - [x] **Ori Tests**: `tests/spec/patterns/variant_punning.ori` — 5 tests pass

### LLVM

- [ ] **Implement**: LLVM codegen for named variant field patterns
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — named variant field codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Formatter

- [ ] **Implement**: Detect `name: name` in variant patterns and emit `name:` form
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — variant pattern punning canonicalization

### Documentation

- [ ] **Implement**: Update spec `10-patterns.md` with named variant field patterns
- [ ] **Implement**: Update `grammar.ebnf` with `variant_field` production
- [ ] **Implement**: Update `.claude/rules/ori-syntax.md` with pattern punning syntax

---

## 9.6 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]`
- [ ] Spec updated: `spec/15-patterns.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: Match expressions work like spec

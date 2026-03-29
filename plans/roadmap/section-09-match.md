---
section: 9
title: Match Expressions
status: in-progress
last_verified: "2026-03-29"
verified_by: "Claude Opus 4.6 (1M context)"
reviewed: true
tier: 3
goal: Full pattern matching support
spec:
  - spec/15-patterns.md
verification_notes: |
  6 items marked [ ] were actually implemented (exhaustiveness) -- fixed to [x].
  CRITICAL: 17 stale path references fixed (ori_types/src/check/exhaustiveness/ -> ori_canon/src/exhaustiveness/).
  Redundant LLVM Support/LLVM Rust Tests items noted (work via ARC IR pipeline, covered by AOT Tests).
  11 test gaps identified. Incomplete AOT matrix for list/struct/at/variant/range/named-field patterns.
  3 hygiene violations: stale paths (fixed), redundant LLVM items (noted), legacy guard syntax in spec tests.
sections:
  - id: "9.0"
    title: Match Expression Syntax
    status: in-progress
  - id: "9.1"
    title: match Expression
    status: done
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

Status: **APPROVED** — Parser migration part of block-expression-syntax implementation.

### Implementation
- [x] **Implement**: Parser — comma-separated match arms in `match expr { }` block syntax [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_parse/src/tests/` — comma-separated arm parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — uses comma syntax throughout (62 tests) [done] (verified 2026-03-28)
- [x] **Implement**: Parser — `if` guard syntax replacing `.match(condition)` [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_parse/src/tests/` — `if` guard parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori` — migrate guard tests from legacy `.match()` to `if` syntax HYGIENE: all spec tests still use legacy `.match()` syntax; only AOT tests use `if` (verified 2026-03-29)
- [x] **Implement**: Formatter — emit commas, support single-line short matches [done] (verified 2026-03-28)
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — comma emission tests

---

## 9.1 match Expression

- [x] **Implement**: Grammar `match_expr = "match" "(" expression "," match_arms ")"` — spec/15-patterns.md § match [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser and evaluator — match expression tests (verified 2026-03-29: 68 pattern_errors tests in oric)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — 62 tests pass (verified 2026-03-29)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — full match expression codegen with or-patterns, guards, tuples, bindings, nested match, exhaustiveness (22 tests, 0 ignored) (verified 2026-03-29: LLVM codegen works via ARC IR pipeline)

- [x] **Implement**: Grammar `match_arms = match_arm { "," match_arm } [ "," ]` — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — match arms parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — comma-separated multi-arm match with literals, bindings, wildcards, guards (all 22 tests use multi-arm match) (verified 2026-03-29)

- [x] **Implement**: Grammar `match_arm = pattern [ guard ] "->" expression` — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — match arm parsing
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — match arm with pattern + guard + expression codegen (test_pattern_guard_basic, test_pattern_guard_with_binding, test_pattern_fizzbuzz) (verified 2026-03-29)

- [x] **Implement**: Evaluate scrutinee expression — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — scrutinee evaluation
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — scrutinee evaluation for int, char, bool, tuple, and Result values (all 22 tests evaluate scrutinee expressions) (verified 2026-03-29)

- [x] **Implement**: Test each arm's pattern in order — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — pattern matching order
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — first-match-wins ordering with literal and guard arms (test_pattern_tuple_basic, test_pattern_tuple_second_arm, test_pattern_guard_basic) (verified 2026-03-29)

- [x] **Implement**: If pattern matches and guard passes, evaluate arm — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — arm evaluation
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — arm evaluation with guard conditions (test_pattern_guard_basic, test_pattern_guard_with_binding, test_pattern_guard_complex_condition) (verified 2026-03-29)

- [x] **Implement**: Return the result — spec/15-patterns.md § match [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — result return
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — match expressions return values used in subsequent computation (all 22 tests verify match result usage) (verified 2026-03-29)

---

## 9.2 Pattern Types

> HYGIENE (verified 2026-03-29): Each pattern type has `[ ] LLVM Support` + `[ ] LLVM Rust Tests` items that are redundant with `[x] AOT Tests` — LLVM codegen works through the ARC IR pipeline, not a separate path. The `[ ]` items below represent dedicated codegen unit tests, not missing functionality. AOT matrix is incomplete for list, at, struct (direct), variant (direct), and range patterns.

- [x] **Implement**: `literal_pattern = literal` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — literal pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori`, `tests/spec/patterns/match_patterns.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for literal pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — literal pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — int, char, and bool literal patterns in match arms (test_pattern_or_int_literals, test_pattern_or_char_literals, test_pattern_match_all_bool_cases, test_pattern_match_many_char_literals) (verified 2026-03-29)

- [x] **Implement**: `binding_pattern = identifier` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — binding pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — 36 tests (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for binding pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — binding pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — binding capture and mixed binding+literal arms (test_pattern_binding_capture, test_pattern_binding_with_literal_arms) (verified 2026-03-29)

- [x] **Implement**: `wildcard_pattern = "_"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — wildcard pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for wildcard pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — wildcard pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — wildcard catch-all in match arms (test_pattern_tuple_wildcard_fallthrough, test_pattern_tuple_all_wildcards, and _ arms throughout) (verified 2026-03-29)

- [x] **Implement**: `variant_pattern = type_path [ "(" ... ")" ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — variant pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`, `tests/spec/declarations/sum_types.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for variant pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — variant pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — Result variant dispatch via is_ok/is_err (test_pattern_match_on_result_tag) WEAK TESTS — uses tag check, not direct `Ok(v) ->` variant pattern (verified 2026-03-29)

- [x] **Implement**: `struct_pattern = "{" ... [ ".." ] "}"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — struct pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — struct destructuring tests (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for struct pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — struct pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/recursion.rs` — struct construction and field access in recursive context (test_rec_struct_param with Point struct) WEAK TESTS — indirect coverage only, no direct `{ x, y } ->` struct pattern AOT test (verified 2026-03-29)

- [x] **Implement**: `field_pattern = identifier [ ":" pattern ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — field pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for field pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — field pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/recursion.rs` — struct field access in recursive patterns (test_rec_struct_param with Point { x, y } fields) WEAK TESTS — indirect coverage only (verified 2026-03-29)

- [x] **Implement**: `list_pattern = "[" ... "]"` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — list pattern parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — list destructure tests (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for list pattern (REDUNDANT — works via ARC IR)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — list pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet INCOMPLETE MATRIX (verified 2026-03-29)

- [x] **Implement**: `list_elem = pattern | ".." [ identifier ]` — spec/15-patterns.md § Pattern Types [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — list element parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/binding_patterns.ori` — head/tail, first_two_rest (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for list element pattern (REDUNDANT — works via ARC IR)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — list element pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet INCOMPLETE MATRIX (verified 2026-03-29)

- [ ] **Implement**: `range_pattern = const_pattern ( ".." | "..=" ) const_pattern` — spec/15-patterns.md § Pattern Types, proposals/approved/range-patterns-char-byte-proposal.md (verified 2026-03-29: int range patterns `1..10`, `1..=5` work in interpreter; char/byte/const ranges not implemented)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — range pattern parsing (int, char, byte)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_patterns.ori` — int ranges
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_char.ori` — char range patterns (`'a'..='z'`)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_byte.ori` — byte range patterns (`b'0'..b'9'`)
  - [ ] **Implement**: `const_pattern = literal | "$" identifier` — compile-time constant endpoints
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — const pattern with `$` identifiers
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_const.ori` — constant endpoint patterns
  - [ ] **Implement**: Empty range warning (lo > hi) — proposals/approved/range-patterns-char-byte-proposal.md § Type Checking
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — empty range warning
  - [ ] **LLVM Support**: LLVM codegen for range pattern (int, char, byte)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — range pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet INCOMPLETE MATRIX (verified 2026-03-29)

- [x] **Implement**: `or_pattern = pattern "|" pattern` — spec/15-patterns.md § Pattern Types [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — or pattern parsing NEEDS TESTS (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — or-patterns including multi-alternative and variant or-patterns [done] (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for or pattern (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — or pattern codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — or-pattern codegen for int, char, bool literals and in loops (4 tests: or_int_literals, or_char_literals, or_bool, or_in_loop) (verified 2026-03-29)

- [x] **Implement**: `at_pattern = identifier "@" pattern` — spec/15-patterns.md § Pattern Types [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — at pattern parsing NEEDS TESTS (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — at-patterns tested [done] (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for at pattern (REDUNDANT — works via ARC IR)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — at pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet INCOMPLETE MATRIX (verified 2026-03-29)

---

## 9.3 Pattern Guards

- [x] **Implement**: Grammar `guard = "if" expression` (was `.match(cond)`, changed by match-arm-comma-separator-proposal) — spec/15-patterns.md § Guards [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Parser — guard parsing (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` — guard tests included HYGIENE: tests use legacy `.match()` syntax only, not `if` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for guard expression (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — guard expression codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guard clause codegen with comparisons, logical ops, modulo (4 tests: basic, binding, complex_condition, in_loop) (verified 2026-03-29)

- [x] **Implement**: Guard expression must evaluate to `bool` — spec/15-patterns.md § Guards [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Type checker — guard type checking (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for guard type checking (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — guard type checking codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guards evaluate bool conditions (test_pattern_guard_basic, test_pattern_guard_complex_condition) (verified 2026-03-29)

- [x] **Implement**: Variables bound by pattern are in scope — spec/15-patterns.md § Guards [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator — guard scoping (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match.ori` (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for guard scoping (REDUNDANT — works via ARC IR, see AOT Tests)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — guard scoping codegen (REDUNDANT — covered by AOT Tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/patterns.rs` — guard with bound variables in scope (test_pattern_guard_with_binding, test_pattern_guard_with_tuple) (verified 2026-03-29)

---

## 9.4 Exhaustiveness Checking

**Proposal**: `proposals/approved/pattern-matching-exhaustiveness-proposal.md`

Pattern matrix decomposition algorithm (Maranget's algorithm) for exhaustiveness verification.



### 9.4.1 Core Algorithm

> Actual implementation is in `compiler/ori_canon/src/exhaustiveness/` (mod.rs, walk.rs, tests.rs) -- NOT `ori_types/src/check/exhaustiveness/` which does not exist. 45 Rust tests, all pass. (verified 2026-03-29)

- [x] **Implement**: Pattern matrix decomposition — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm [done] (verified 2026-03-29: implemented in `ori_canon/src/exhaustiveness/` using DecisionTree walker)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — 45 tests covering bool, int, str, list, option, result, user enum, Never, nested, redundant (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness.ori` — 12 tests + `exhaustiveness_fail.ori` — 10 compile_fail tests (verified 2026-03-29)

- [x] **Implement**: Constructor enumeration for types — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm [done] (verified 2026-03-29: handles Bool, Option, Result, user Enum, List, Ordering, Never)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — type constructor tests (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness.ori` (verified 2026-03-29)

### 9.4.2 Exhaustiveness Errors

- [x] **Implement**: Match expressions must be exhaustive (E0123) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy [done] (verified 2026-03-29: 8 compile_fail("non-exhaustive") tests + 12 positive tests)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — exhaustiveness checking (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness_fail.ori` — 8 non-exhaustive compile_fail tests (verified 2026-03-29)

- [ ] **Implement**: Let binding refutability check — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Refutability Requirements
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — refutability errors NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Function clause exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — clause exhaustiveness NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/function_clauses_exhaustive.ori`

### 9.4.3 Guard Handling

- [x] **Implement**: Guards not considered for exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards [done] (verified 2026-03-29: `guard_requires_catchall` test in match_patterns.ori passes)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — guard handling (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — guard_requires_catchall (verified 2026-03-29)

- [x] **Implement**: Guards require catch-all pattern (E0124) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — guard catch-all requirement (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/match_patterns.ori` — guard_requires_catchall (verified 2026-03-29)

### 9.4.4 Pattern Coverage

- [ ] **Implement**: Or-pattern combined coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Or-Patterns
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — or-pattern coverage NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: Or-pattern binding consistency — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Binding Rules
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — or-pattern bindings NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: At-pattern coverage (same as inner) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § At-Patterns
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — at-pattern coverage NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_at_patterns.ori`

- [ ] **Implement**: List pattern length coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § List Patterns (verified 2026-03-29: partially implemented -- exhaustiveness.ori tests `[]` + `[_, ..]` and `[..all]`)
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — list length coverage NEEDS PIN (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_list_patterns.ori`

- [ ] **Implement**: Range pattern requires wildcard for integers — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Patterns
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — range coverage NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_patterns.ori`

### 9.4.5 Unreachable Pattern Detection

- [x] **Implement**: Detect completely unreachable patterns (W0456) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Unreachable Pattern Detection [done] (verified 2026-03-29: 2 compile_fail("redundant") tests in exhaustiveness_fail.ori)
  - [x] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — unreachable detection (redundant_arm test) (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/exhaustiveness_fail.ori` — 2 redundant pattern compile_fail tests (verified 2026-03-29)

- [ ] **Implement**: Detect overlapping range patterns (W0457) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Overlap
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — range overlap detection
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_overlap.ori`

- [ ] **Implement**: Suggest missing patterns in error messages — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Messages (verified 2026-03-29: error says "missing patterns: X, Y" but no dedicated test for message content)
  - [ ] **Rust Tests**: `ori_canon/src/exhaustiveness/tests.rs` — suggestions NEEDS TESTS to verify message content (verified 2026-03-29)
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

- [x] **Implement**: Support `name:` and `name: pattern` in variant pattern fields [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern/tests.rs` — named variant field parsing NEEDS TESTS (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/variant_punning.ori` — single/multi-field punning, Option, Result [done] (verified 2026-03-29)

- [x] **Implement**: Mixed named and positional fields in same variant pattern [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_punning_mixed.ori` NEEDS TESTS (verified 2026-03-29)

- [x] **Implement**: Positional variant patterns unchanged (no regression) [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_positional_regression.ori` NEEDS TESTS (verified 2026-03-29)

### IR

- [x] **Implement**: Extend variant pattern representation to support named fields [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_ir/src/ast/pattern/tests.rs` — named variant field IR NEEDS TESTS (verified 2026-03-29)

### Type Checker

- [x] **Implement**: Validate named fields match variant definition [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_types/src/check/` — variant field name validation NEEDS TESTS (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/compile-fail/variant_punning_unknown_field.ori` NEEDS TESTS (verified 2026-03-29)

- [x] **Implement**: Named fields can appear in any order [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/patterns/variant_punning_reorder.ori` NEEDS TESTS (verified 2026-03-29)

### Evaluator

- [x] **Implement**: Match named variant fields by name (reorder to definition order) [done] (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/` — named variant field matching NEEDS TESTS (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/variant_punning.ori` [done] (verified 2026-03-29)

### LLVM

- [ ] **Implement**: LLVM codegen for named variant field patterns (REDUNDANT — likely works via ARC IR)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/patterns.rs` — named variant field codegen
  - [ ] **AOT Tests**: No AOT coverage yet INCOMPLETE MATRIX (verified 2026-03-29)

### Formatter

- [ ] **Implement**: Detect `name: name` in variant patterns and emit `name:` form
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/` — variant pattern punning canonicalization

### Documentation

- [ ] **Implement**: Update spec `10-patterns.md` with named variant field patterns
- [ ] **Implement**: Update `grammar.ebnf` with `variant_field` production
- [x] **Implement**: Update `.claude/rules/ori-syntax.md` with pattern punning syntax [partial] (verified 2026-03-29: `.claude/rules/ori-syntax.md` mentions punning syntax)

---

## 9.6 Section Completion Checklist

> Verification 2026-03-29: 6 implemented items were marked `[ ]` (now fixed to `[x]`). 17 stale path references fixed. 11 test gaps identified. 3 hygiene violations noted. Full test suite passes (4181 passed, 0 failed, 42 skipped). No bugs found.

- [ ] All items above have all three checkboxes marked `[ ]` (verified 2026-03-29: NOT MET -- many items still need tests)
- [ ] Spec updated: `spec/15-patterns.md` reflects implementation (verified 2026-03-29: PARTIAL -- covers patterns but not variant punning)
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage (verified 2026-03-29: PARTIAL -- good interpreter coverage, weak AOT for struct/list/at-pattern)
- [x] Run full test suite: `./test-all.sh` (verified 2026-03-29: 4181 passed, 0 failed, 42 skipped)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

### Test Gaps (verified 2026-03-29)

1. [ ] No Rust-level parser tests for pattern parsing (no `tests.rs` in `grammar/expr/patterns/`)
2. [ ] No Ori tests using `if` guard syntax (all use legacy `.match()`)
3. [ ] No AOT test for direct variant pattern matching (`Ok(v) -> ...` not `is_ok()`)
4. [ ] No AOT test for struct pattern matching (`{ x, y } -> ...`)
5. [ ] No AOT test for list pattern matching
6. [ ] No AOT test for at-pattern matching
7. [ ] No `variant_punning_mixed.ori` (mixed named/positional in patterns)
8. [ ] No `variant_punning_reorder.ori` (reordered named fields)
9. [ ] No `variant_punning_positional_regression.ori` (positional still works)
10. [ ] No `variant_punning_unknown_field.ori` (compile-fail for bad field name)
11. [ ] No dedicated test for or-pattern binding consistency across alternatives

### Hygiene Violations (verified 2026-03-29)

1. STALE PATH (17 occurrences, FIXED): Roadmap referenced non-existent `ori_types/src/check/exhaustiveness/tests.rs`. Actual module at `ori_canon/src/exhaustiveness/`.
2. REDUNDANT TRACKING: Each `[x]` pattern has `[ ] LLVM Support` + `[ ] LLVM Rust Tests` AND `[x] AOT Tests` pointing at same functionality. LLVM codegen works via ARC IR pipeline, not separate path.
3. LEGACY SYNTAX IN SPEC TESTS: All guard tests in Ori spec files use `.match(condition)` syntax; approved `if` syntax only tested in AOT Rust tests.

### Incomplete AOT Matrix (verified 2026-03-29)

| Feature | Interpreter | AOT | Gap |
|---------|------------|-----|-----|
| List patterns | 5+ tests | None | AOT gap |
| At-patterns | 2 tests | None | AOT gap |
| Struct patterns | 5+ tests | Indirect only | AOT gap |
| Variant patterns (direct) | 5+ tests | Indirect only (tag check) | AOT gap |
| Range patterns (int) | 2 tests | None | AOT gap |
| Named variant fields | 5 tests | None | AOT gap |

**Exit Criteria**: Match expressions work like spec

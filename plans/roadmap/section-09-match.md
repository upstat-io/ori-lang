---
section: 9
title: Match Expressions
status: not-started
tier: 3
goal: Full pattern matching support
spec:
  - spec/10-patterns.md
sections:
  - id: "9.0"
    title: Match Expression Syntax
    status: not-started
  - id: "9.1"
    title: match Expression
    status: not-started
  - id: "9.2"
    title: Pattern Types
    status: not-started
  - id: "9.3"
    title: Pattern Guards
    status: not-started
  - id: "9.4"
    title: Exhaustiveness Checking
    status: not-started
  - id: "9.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 9: Match Expressions

**Goal**: Full pattern matching support

> **SPEC**: `spec/10-patterns.md`

**Proposals**:
- `proposals/approved/match-expression-syntax-proposal.md` — Match expression and pattern syntax
- `proposals/approved/pattern-matching-exhaustiveness-proposal.md` — Exhaustiveness checking

---

## 9.0 Match Expression Syntax

**Proposal**: `proposals/approved/match-expression-syntax-proposal.md`

Documents the existing implementation of match expressions. Key specifications:
- `match(scrutinee, pattern -> expression, ...)` syntax
- Guard syntax `.match(condition)`
- Pattern types: literal, binding, wildcard, variant, struct, tuple, list, range, or-pattern, at-pattern
- Top-to-bottom, first-match-wins evaluation
- Integer-only literal patterns (no float patterns)

Status: **IMPLEMENTED** — This proposal formalizes existing behavior.

---

## 9.1 match Expression

- [ ] **Implement**: Grammar `match_expr = "match" "(" expression "," match_arms ")"` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match expression parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for match expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — match expression codegen

- [ ] **Implement**: Grammar `match_arms = match_arm { "," match_arm } [ "," ]` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match arms parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for match arms
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — match arms codegen

- [ ] **Implement**: Grammar `match_arm = pattern [ guard ] "->" expression` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match arm parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for match arm
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — match arm codegen

- [ ] **Implement**: Evaluate scrutinee expression — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — scrutinee evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for scrutinee evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — scrutinee evaluation codegen

- [ ] **Implement**: Test each arm's pattern in order — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — pattern matching order
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for pattern matching order
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — pattern matching order codegen

- [ ] **Implement**: If pattern matches and guard passes, evaluate arm — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — arm evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for arm evaluation with guard
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — arm evaluation codegen

- [ ] **Implement**: Return the result — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — result return
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for match result return
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — match result return codegen

---

## 9.2 Pattern Types

- [ ] **Implement**: `literal_pattern = literal` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — literal pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for literal pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — literal pattern codegen

- [ ] **Implement**: `binding_pattern = identifier` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — binding pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for binding pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — binding pattern codegen

- [ ] **Implement**: `wildcard_pattern = "_"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — wildcard pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`
  - [ ] **LLVM Support**: LLVM codegen for wildcard pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — wildcard pattern codegen

- [ ] **Implement**: `variant_pattern = type_path [ "(" ... ")" ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — variant pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for variant pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — variant pattern codegen

- [ ] **Implement**: `struct_pattern = "{" ... [ ".." ] "}"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — struct pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — struct pattern codegen

- [ ] **Implement**: `field_pattern = identifier [ ":" pattern ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — field pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for field pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — field pattern codegen

- [ ] **Implement**: `list_pattern = "[" ... "]"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for list pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — list pattern codegen

- [ ] **Implement**: `list_elem = pattern | ".." [ identifier ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list element parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for list element pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — list element pattern codegen

- [ ] **Implement**: `range_pattern = [ literal ] ".." [ literal ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — range pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for range pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — range pattern codegen

- [ ] **Implement**: `or_pattern = pattern "|" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — or pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for or pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — or pattern codegen

- [ ] **Implement**: `at_pattern = identifier "@" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — at pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`
  - [ ] **LLVM Support**: LLVM codegen for at pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — at pattern codegen

---

## 9.3 Pattern Guards

- [ ] **Implement**: Grammar `guard = "." "match" "(" expression ")"` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — guard parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — guard expression codegen

- [ ] **Implement**: Guard expression must evaluate to `bool` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — guard type checking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — guard type checking codegen

- [ ] **Implement**: Variables bound by pattern are in scope — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — guard scoping
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard scoping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — guard scoping codegen

---

## 9.4 Exhaustiveness Checking

**Proposal**: `proposals/approved/pattern-matching-exhaustiveness-proposal.md`

Pattern matrix decomposition algorithm (Maranget's algorithm) for exhaustiveness verification.

### 9.4.1 Core Algorithm

- [ ] **Implement**: Pattern matrix decomposition — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — matrix decomposition
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Constructor enumeration for types — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Algorithm
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — type constructors
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

### 9.4.2 Exhaustiveness Errors

- [ ] **Implement**: Match expressions must be exhaustive (E0123) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — exhaustiveness checking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Let binding refutability check — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Refutability Requirements
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — refutability errors
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Function clause exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Policy
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — clause exhaustiveness
  - [ ] **Ori Tests**: `tests/spec/patterns/function_clauses_exhaustive.ori`

### 9.4.3 Guard Handling

- [ ] **Implement**: Guards not considered for exhaustiveness — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — guard handling
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards_exhaustive.ori`

- [ ] **Implement**: Guards require catch-all pattern (E0124) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Guards
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — guard catch-all requirement
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards_exhaustive.ori`

### 9.4.4 Pattern Coverage

- [ ] **Implement**: Or-pattern combined coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Or-Patterns
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — or-pattern coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: Or-pattern binding consistency — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Binding Rules
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — or-pattern bindings
  - [ ] **Ori Tests**: `tests/spec/patterns/match_or_patterns.ori`

- [ ] **Implement**: At-pattern coverage (same as inner) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § At-Patterns
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — at-pattern coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_at_patterns.ori`

- [ ] **Implement**: List pattern length coverage — proposals/approved/pattern-matching-exhaustiveness-proposal.md § List Patterns
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — list length coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_list_patterns.ori`

- [ ] **Implement**: Range pattern requires wildcard for integers — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Patterns
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — range coverage
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_patterns.ori`

### 9.4.5 Unreachable Pattern Detection

- [ ] **Implement**: Detect completely unreachable patterns (W0456) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Unreachable Pattern Detection
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — unreachable detection
  - [ ] **Ori Tests**: `tests/spec/patterns/match_unreachable.ori`

- [ ] **Implement**: Detect overlapping range patterns (W0457) — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Range Overlap
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — range overlap detection
  - [ ] **Ori Tests**: `tests/spec/patterns/match_range_overlap.ori`

- [ ] **Implement**: Suggest missing patterns in error messages — proposals/approved/pattern-matching-exhaustiveness-proposal.md § Error Messages
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — suggestions
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

---

## 9.5 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/10-patterns.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Match expressions work like spec

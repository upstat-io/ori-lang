# Phase 9: Match Expressions

**Goal**: Full pattern matching support

> **SPEC**: `spec/10-patterns.md`

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

- [ ] **Implement**: Match expressions must be exhaustive — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — exhaustiveness checking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for exhaustive match
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — exhaustive match codegen

- [ ] **Implement**: Error if any value not covered — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — coverage errors
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for coverage error handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — coverage error codegen

- [ ] **Implement**: Track covered patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — pattern tracking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for pattern tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — pattern tracking codegen

- [ ] **Implement**: Warn on non-exhaustive match — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — warnings
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for non-exhaustive match warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — non-exhaustive warning codegen

- [ ] **Implement**: Suggest missing patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — suggestions
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for missing pattern suggestions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — missing pattern suggestions codegen

---

## 9.5 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/10-patterns.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Match expressions work like spec

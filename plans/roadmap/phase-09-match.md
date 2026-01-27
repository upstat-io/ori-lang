# Phase 9: Match Expressions

**Goal**: Full pattern matching support

> **SPEC**: `spec/10-patterns.md`

---

## 9.1 match Expression

- [ ] **Implement**: Grammar `match_expr = "match" "(" expression "," match_arms ")"` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match expression parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: Grammar `match_arms = match_arm { "," match_arm } [ "," ]` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match arms parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: Grammar `match_arm = pattern [ guard ] "->" expression` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — match arm parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: Evaluate scrutinee expression — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — scrutinee evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: Test each arm's pattern in order — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — pattern matching order
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: If pattern matches and guard passes, evaluate arm — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — arm evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: Return the result — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — result return
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

---

## 9.2 Pattern Types

- [ ] **Implement**: `literal_pattern = literal` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — literal pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: `binding_pattern = identifier` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — binding pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `wildcard_pattern = "_"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — wildcard pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match.ori`

- [ ] **Implement**: `variant_pattern = type_path [ "(" ... ")" ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — variant pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `struct_pattern = "{" ... [ ".." ] "}"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — struct pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `field_pattern = identifier [ ":" pattern ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — field pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `list_pattern = "[" ... "]"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `list_elem = pattern | ".." [ identifier ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list element parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `range_pattern = [ literal ] ".." [ literal ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — range pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `or_pattern = pattern "|" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — or pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

- [ ] **Implement**: `at_pattern = identifier "@" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — at pattern parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_patterns.ori`

---

## 9.3 Pattern Guards

- [ ] **Implement**: Grammar `guard = "." "match" "(" expression ")"` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — guard parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`

- [ ] **Implement**: Guard expression must evaluate to `bool` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — guard type checking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`

- [ ] **Implement**: Variables bound by pattern are in scope — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `oric/src/eval/exec/match.rs` — guard scoping
  - [ ] **Ori Tests**: `tests/spec/patterns/match_guards.ori`

---

## 9.4 Exhaustiveness Checking

- [ ] **Implement**: Match expressions must be exhaustive — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — exhaustiveness checking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Error if any value not covered — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — coverage errors
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Track covered patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — pattern tracking
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Warn on non-exhaustive match — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — warnings
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

- [ ] **Implement**: Suggest missing patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `oric/src/typeck/checker/exhaustiveness.rs` — suggestions
  - [ ] **Ori Tests**: `tests/spec/patterns/match_exhaustive.ori`

---

## 9.5 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/10-patterns.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: Match expressions work like spec

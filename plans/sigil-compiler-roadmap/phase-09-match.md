# Phase 9: Match Expressions

**Goal**: Full pattern matching support

> **SPEC**: `spec/10-patterns.md`

---

## 9.1 match Expression

- [ ] **Implement**: Grammar `match_expr = "match" "(" expression "," match_arms ")"` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — match expression parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: Grammar `match_arms = match_arm { "," match_arm } [ "," ]` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — match arms parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: Grammar `match_arm = pattern [ guard ] "->" expression` — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — match arm parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: Evaluate scrutinee expression — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/match.rs` — scrutinee evaluation
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: Test each arm's pattern in order — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/match.rs` — pattern matching order
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: If pattern matches and guard passes, evaluate arm — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/match.rs` — arm evaluation
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: Return the result — spec/10-patterns.md § match
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/match.rs` — result return
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

---

## 9.2 Pattern Types

- [ ] **Implement**: `literal_pattern = literal` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — literal pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: `binding_pattern = identifier` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — binding pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `wildcard_pattern = "_"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — wildcard pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match.si`

- [ ] **Implement**: `variant_pattern = type_path [ "(" ... ")" ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — variant pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `struct_pattern = "{" ... [ ".." ] "}"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — struct pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `field_pattern = identifier [ ":" pattern ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — field pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `list_pattern = "[" ... "]"` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — list pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `list_elem = pattern | ".." [ identifier ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — list element parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `range_pattern = [ literal ] ".." [ literal ]` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — range pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `or_pattern = pattern "|" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — or pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

- [ ] **Implement**: `at_pattern = identifier "@" pattern` — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — at pattern parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_patterns.si`

---

## 9.3 Pattern Guards

- [ ] **Implement**: Grammar `guard = "." "match" "(" expression ")"` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — guard parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_guards.si`

- [ ] **Implement**: Guard expression must evaluate to `bool` — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/pattern.rs` — guard type checking
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_guards.si`

- [ ] **Implement**: Variables bound by pattern are in scope — spec/10-patterns.md § Guards
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/match.rs` — guard scoping
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_guards.si`

---

## 9.4 Exhaustiveness Checking

- [ ] **Implement**: Match expressions must be exhaustive — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/exhaustiveness.rs` — exhaustiveness checking
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_exhaustive.si`

- [ ] **Implement**: Error if any value not covered — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/exhaustiveness.rs` — coverage errors
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_exhaustive.si`

- [ ] **Implement**: Track covered patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/exhaustiveness.rs` — pattern tracking
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_exhaustive.si`

- [ ] **Implement**: Warn on non-exhaustive match — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/exhaustiveness.rs` — warnings
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_exhaustive.si`

- [ ] **Implement**: Suggest missing patterns — spec/10-patterns.md § Exhaustiveness
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/exhaustiveness.rs` — suggestions
  - [ ] **Sigil Tests**: `tests/spec/patterns/match_exhaustive.si`

---

## 9.5 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/10-patterns.md` reflects implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: Match expressions work like spec

# Phase 6: Comments

**Goal**: Implement comment preservation, positioning, and doc comment reordering.

> **DESIGN**: `docs/tooling/formatter/design/03-comments/`

## Phase Status: ⏳ Not Started

## 6.1 Comment Extraction

- [ ] **Implement**: Comment extraction from token stream
  - [ ] **Rust Tests**: `ori_fmt/src/comments/tests.rs`
- [ ] **Implement**: Comment-to-AST node association
  - [ ] **Rust Tests**: Correct node binding
- [ ] **Implement**: Leading comment detection
  - [ ] **Rust Tests**: Comments before nodes
- [ ] **Implement**: Trailing comment handling (move to next line)
  - [ ] **Rust Tests**: Inline comments converted

## 6.2 Regular Comments

- [ ] **Implement**: Own-line comment preservation
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/own_line.ori`
  ```ori
  // This is a comment
  let x = 42
  ```
- [ ] **Implement**: Space after `//` enforcement
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/spacing.ori`
- [ ] **Implement**: Multiple consecutive comments
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/consecutive.ori`
- [ ] **Implement**: Comments between declarations
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/between.ori`
- [ ] **Implement**: Comments inside blocks
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/inside.ori`
- [ ] **Implement**: Inline comment conversion (move to own line)
  - [ ] **Golden Tests**: `tests/fmt/comments/regular/inline_fix.ori`
  ```ori
  // Before:
  // let x = 42  // inline comment

  // After:
  // inline comment
  let x = 42
  ```

## 6.3 Doc Comments

### Basic Doc Comments

- [ ] **Implement**: Description marker (`// #`)
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/description.ori`
  ```ori
  // #Calculates the sum of two integers.
  @add (a: int, b: int) -> int = a + b
  ```
- [ ] **Implement**: Parameter marker (`// @param`)
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/param.ori`
  ```ori
  // @param a The first operand
  // @param b The second operand
  @add (a: int, b: int) -> int = a + b
  ```
- [ ] **Implement**: Field marker (`// @field`)
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/field.ori`
- [ ] **Implement**: Error marker (`// !`)
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/error.ori`
  ```ori
  // !DivisionByZero: when b is zero
  @divide (a: int, b: int) -> Result<int, Error> = ...
  ```
- [ ] **Implement**: Example marker (`// >`)
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/example.ori`
  ```ori
  // >add(a: 1, b: 2) -> 3
  @add (a: int, b: int) -> int = a + b
  ```

### Doc Comment Ordering

- [ ] **Implement**: Enforce marker order: `#` → `@param`/`@field` → `!` → `>`
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/order.ori`
- [ ] **Implement**: Reorder out-of-order markers
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/reorder.ori`
  ```ori
  // Before:
  // >example() -> 1
  // #Description

  // After:
  // #Description
  // >example() -> 1
  ```
- [ ] **Implement**: `@param` order matches function signature
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/param_order.ori`
- [ ] **Implement**: `@field` order matches struct definition
  - [ ] **Golden Tests**: `tests/fmt/comments/doc/field_order.ori`

## 6.4 Comment Positioning

- [ ] **Implement**: Comments before function declarations
  - [ ] **Golden Tests**: `tests/fmt/comments/position/function.ori`
- [ ] **Implement**: Comments before type declarations
  - [ ] **Golden Tests**: `tests/fmt/comments/position/type.ori`
- [ ] **Implement**: Comments before trait declarations
  - [ ] **Golden Tests**: `tests/fmt/comments/position/trait.ori`
- [ ] **Implement**: Comments before impl blocks
  - [ ] **Golden Tests**: `tests/fmt/comments/position/impl.ori`
- [ ] **Implement**: Comments before imports
  - [ ] **Golden Tests**: `tests/fmt/comments/position/import.ori`
- [ ] **Implement**: Comments before tests
  - [ ] **Golden Tests**: `tests/fmt/comments/position/test.ori`

## 6.5 Edge Cases

- [ ] **Implement**: Empty comment lines (`//`)
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/empty.ori`
- [ ] **Implement**: Comments with only whitespace
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/whitespace.ori`
- [ ] **Implement**: Comments at end of file
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/eof.ori`
- [ ] **Implement**: Comments in empty file
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/empty_file.ori`
- [ ] **Implement**: Mixed doc and regular comments
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/mixed.ori`

## 6.6 Comment Preservation

- [ ] **Implement**: Preserve comment content exactly
  - [ ] **Rust Tests**: Content unchanged
- [ ] **Implement**: Preserve blank lines between comment groups
  - [ ] **Golden Tests**: `tests/fmt/comments/preserve/blank.ori`
- [ ] **Implement**: Don't add/remove comments
  - [ ] **Rust Tests**: Comment count unchanged

## Completion Checklist

- [ ] All comment extraction tests pass
- [ ] All regular comment tests pass
- [ ] All doc comment tests pass
- [ ] All positioning tests pass
- [ ] All edge case tests pass
- [ ] Comment preservation verified
- [ ] Round-trip verification for all comment scenarios

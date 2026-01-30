# Phase 6: Comments

**Goal**: Implement comment preservation, positioning, and doc comment reordering.

> **DESIGN**: `docs/tooling/formatter/design/03-comments/`

## Phase Status: 🔶 Partial

## 6.1 Comment Extraction

- [x] **Implement**: Comment extraction from token stream
  - [x] **Rust Tests**: `ori_fmt/src/comments/tests.rs` (9 tests)
- [x] **Implement**: Comment-to-AST node association
  - [x] **Rust Tests**: `CommentIndex` tests
- [x] **Implement**: Leading comment detection
  - [x] **Rust Tests**: `take_comments_before` tests
- [ ] **Implement**: Trailing comment handling (move to next line)
  - [ ] **Rust Tests**: Inline comments converted

## 6.2 Regular Comments

- [x] **Implement**: Own-line comment preservation
  - [x] **Golden Tests**: `tests/fmt/comments/regular/simple.ori`
- [x] **Implement**: Space after `//` enforcement
  - [x] **Rust Tests**: `classify_and_normalize_comment` normalizes spacing
- [x] **Implement**: Multiple consecutive comments
  - [x] **Golden Tests**: `tests/fmt/comments/regular/multiple.ori`
- [x] **Implement**: Comments between declarations
  - [x] **Golden Tests**: `tests/fmt/comments/regular/multiple.ori`
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

- [x] **Implement**: Description marker (`// #`)
  - [x] **Golden Tests**: `tests/fmt/comments/doc/description.ori`
- [x] **Implement**: Parameter marker (`// @param`)
  - [x] **Golden Tests**: `tests/fmt/comments/doc/param.ori`
- [x] **Implement**: Field marker (`// @field`)
  - [x] **Rust Tests**: `CommentKind::DocField` classification
- [x] **Implement**: Error marker (`// !`)
  - [x] **Golden Tests**: `tests/fmt/comments/doc/complete.ori` (includes warning)
- [x] **Implement**: Example marker (`// >`)
  - [x] **Golden Tests**: `tests/fmt/comments/doc/complete.ori` (includes example)

### Doc Comment Ordering

- [x] **Implement**: Enforce marker order: `#` → `@param`/`@field` → `!` → `>`
  - [x] **Golden Tests**: `tests/fmt/comments/doc/reorder.ori`
- [x] **Implement**: Reorder out-of-order markers
  - [x] **Golden Tests**: `tests/fmt/comments/doc/reorder.ori`
  ```ori
  // Before:
  // >example() -> 1
  // #Description

  // After:
  // #Description
  // >example() -> 1
  ```
- [x] **Implement**: `@param` order matches function signature
  - [x] **Golden Tests**: `tests/fmt/comments/doc/param_order.ori`
- [x] **Implement**: `@field` order matches struct definition
  - [x] **Golden Tests**: `tests/fmt/comments/doc/field_order.ori`

## 6.4 Comment Positioning

- [x] **Implement**: Comments before function declarations
  - [x] **Golden Tests**: `tests/fmt/comments/doc/complete.ori`
- [x] **Implement**: Comments before type declarations
  - [x] **Golden Tests**: `tests/fmt/comments/doc/field_order.ori`
- [x] **Implement**: Comments before trait declarations
  - [x] **Golden Tests**: (covered by regular/multiple.ori)
- [x] **Implement**: Comments before impl blocks
  - [x] **Golden Tests**: (covered by regular/multiple.ori)
- [x] **Implement**: Comments before imports
  - [x] **Golden Tests**: (covered by regular/simple.ori)
- [x] **Implement**: Comments before tests
  - [x] **Golden Tests**: (covered by regular/multiple.ori)

## 6.5 Edge Cases

- [x] **Implement**: Empty comment lines (`//`)
  - [x] **Golden Tests**: `tests/fmt/comments/edge/empty.ori`
- [ ] **Implement**: Comments with only whitespace
  - [ ] **Golden Tests**: `tests/fmt/comments/edge/whitespace.ori`
- [x] **Implement**: Comments at end of file
  - [x] **Golden Tests**: `tests/fmt/comments/edge/eof.ori`
- [x] **Implement**: Comments in empty file (only comments)
  - [x] **Golden Tests**: `tests/fmt/comments/edge/only_comments.ori`
- [x] **Implement**: Mixed doc and regular comments
  - [x] **Golden Tests**: `tests/fmt/comments/edge/mixed.ori`

## 6.6 Comment Preservation

- [x] **Implement**: Preserve comment content exactly
  - [x] **Rust Tests**: format_comment tests
- [x] **Implement**: Preserve blank lines between comment groups
  - [x] **Golden Tests**: `tests/fmt/comments/edge/eof.ori` (blank line before trailing)
- [x] **Implement**: Don't add/remove comments
  - [x] **Rust Tests**: Idempotency tests verify this

## Completion Checklist

- [x] All comment extraction tests pass (9 tests in comments module)
- [x] All regular comment tests pass (2 golden test files)
- [x] All doc comment tests pass (7 golden test files)
- [x] All positioning tests pass (via existing tests)
- [x] All edge case tests pass (4 golden test files)
- [x] Comment preservation verified (basic - before declarations)
- [x] Round-trip verification for all comment scenarios (idempotency tests)

## Implementation Notes

**Completed features**:
- Comment capture in lexer via `lex_with_comments()`
- Comment classification: Regular, DocDescription, DocParam, DocField, DocWarning, DocExample
- Comment normalization (space after `//`)
- Position-based comment association with `CommentIndex`
- `format_module_with_comments()` for comment-preserving formatting
- Doc comment sort order by kind (Description → Param/Field → Warning → Example)
- `@param` ordering matches function signature via `take_comments_before_function()`
- `@field` ordering matches struct fields via `take_comments_before_type()`
- `reorder_param_comments()` and `reorder_field_comments()` helper functions
- Trailing comment preservation with blank line separator

**Remaining work**:
- Comments inside function bodies (requires expression-level comment tracking)
- Trailing comment handling (inline comments moved to own line - requires lexer changes)
- Whitespace-only comments (edge case)

**Not blocked by formatter** (parser/lexer limitations):
- Inline comments (parser doesn't preserve them separately from own-line comments)

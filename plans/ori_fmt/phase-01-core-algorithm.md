# Phase 1: Core Algorithm

**Goal**: Implement the foundational two-pass formatting algorithm with width calculation and line breaking logic.

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/`

## Phase Status: ✅ Complete

## 1.1 Width Calculator ✅ Complete

Bottom-up traversal calculating inline width of each AST node.

- [x] **Implement**: `WidthCalculator` struct
  - [x] **Rust Tests**: `ori_fmt/src/width/tests.rs`
- [x] **Implement**: Width calculation for literals (int, float, string, bool)
  - [x] **Rust Tests**: Width of `42`, `3.14`, `"hello"`, `true`
- [x] **Implement**: Width calculation for identifiers
  - [x] **Rust Tests**: Width of `@foo`, `$bar`, `MyType`
- [x] **Implement**: Width calculation for operators
  - [x] **Rust Tests**: Width of `+`, `->`, `..=`
- [x] **Implement**: Width caching for performance
  - [x] **Rust Tests**: Cache hit/miss verification
- [x] **Implement**: Recursive width calculation for compound nodes
  - [x] **Rust Tests**: Nested expression width

## 1.2 Formatter Core ✅ Complete

Top-down rendering engine that decides inline vs broken format.

- [x] **Implement**: `Formatter` struct with configuration
  - [x] **Rust Tests**: `ori_fmt/src/formatter/tests.rs`
- [x] **Implement**: Indentation tracking (4 spaces per level)
  - [x] **Rust Tests**: Indentation depth management
- [x] **Implement**: Line width tracking (100 char limit)
  - [x] **Rust Tests**: Width calculation during emit
- [x] **Implement**: `emit_inline` method for inline formatting
  - [x] **Rust Tests**: Simple inline output
- [x] **Implement**: `emit_broken` method for broken formatting
  - [x] **Rust Tests**: Multi-line output with indentation
- [x] **Implement**: Decision logic: inline if width ≤100, break otherwise
  - [x] **Rust Tests**: Threshold behavior at boundary

## 1.3 Output Emitter ✅ Complete

String building and output production.

- [x] **Implement**: `Emitter` trait for output abstraction
  - [x] **Rust Tests**: `ori_fmt/src/emitter.rs` (inline tests)
- [x] **Implement**: `StringEmitter` for in-memory output
  - [x] **Rust Tests**: String concatenation
- [x] **Implement**: `FileEmitter` for file output
  - [x] **Rust Tests**: File write verification
- [x] **Implement**: Newline handling (Unix-style `\n`)
  - [x] **Rust Tests**: Consistent line endings
- [x] **Implement**: Trailing newline enforcement
  - [x] **Rust Tests**: File ends with single newline

## 1.4 Line Breaking Rules ✅ Complete

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/line-breaking.md`

- [x] **Implement**: Always-inline construct detection
  - [x] **Rust Tests**: Simple expressions stay inline
- [x] **Implement**: Always-stacked construct detection
  - [x] **Rust Tests**: `run`, `try`, `match` always break
- [x] **Implement**: Width-based breaking for flexible constructs
  - [x] **Rust Tests**: Break at 100 chars
- [x] **Implement**: Independent breaking for nested constructs
  - [x] **Rust Tests**: Nested call breaks independently
- [x] **Implement**: Trailing comma insertion for multi-line
  - [x] **Rust Tests**: Comma added when broken
- [x] **Implement**: Trailing comma removal for single-line
  - [x] **Rust Tests**: Comma removed when inline (not emitted)

## 1.5 Indentation Rules ✅ Complete

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/indentation.md`

- [x] **Implement**: 4-space indentation increment
  - [x] **Rust Tests**: Correct spacing at each level
- [x] **Implement**: Tab-to-space conversion
  - [x] **Rust Tests**: Tabs become 4 spaces
- [x] **Implement**: Continuation indentation for broken expressions
  - [x] **Rust Tests**: Arguments align properly
- [x] **Implement**: Block indentation for nested bodies
  - [x] **Rust Tests**: Function body indented

## 1.6 Blank Line Handling ✅ Complete

*Implemented in Phase 2 (`ModuleFormatter`)*

- [x] **Implement**: Single blank line between top-level items
  - [x] **Golden Tests**: All declaration tests verify blank lines
- [x] **Implement**: Blank line after import block
  - [x] **Golden Tests**: `tests/fmt/declarations/imports/grouped.ori`
- [x] **Implement**: Blank line after constants block
  - [x] **Golden Tests**: `tests/fmt/declarations/constants/simple.ori`
- [x] **Implement**: Collapse consecutive blank lines
  - [x] **Rust Tests**: `normalize_whitespace()` in golden test harness
- [x] **Implement**: No trailing blank lines at end of file
  - [x] **Golden Tests**: All tests verify single trailing newline

## Completion Checklist

- [x] All width calculation tests pass
- [x] All formatter core tests pass
- [x] All line breaking tests pass
- [x] All indentation tests pass
- [x] Round-trip verification works (AST-level idempotency tests; full parse-format-parse testing requires Phase 7)
- [x] Tab-to-space conversion
- [x] Blank line handling (implemented in Phase 2)

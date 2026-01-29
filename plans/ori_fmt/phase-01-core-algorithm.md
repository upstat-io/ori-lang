# Phase 1: Core Algorithm

**Goal**: Implement the foundational two-pass formatting algorithm with width calculation and line breaking logic.

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/`

## Phase Status: 🔶 Partial

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

## 1.2 Formatter Core

Top-down rendering engine that decides inline vs broken format.

- [ ] **Implement**: `Formatter` struct with configuration
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/tests.rs`
- [ ] **Implement**: Indentation tracking (4 spaces per level)
  - [ ] **Rust Tests**: Indentation depth management
- [ ] **Implement**: Line width tracking (100 char limit)
  - [ ] **Rust Tests**: Width calculation during emit
- [ ] **Implement**: `emit_inline` method for inline formatting
  - [ ] **Rust Tests**: Simple inline output
- [ ] **Implement**: `emit_broken` method for broken formatting
  - [ ] **Rust Tests**: Multi-line output with indentation
- [ ] **Implement**: Decision logic: inline if width ≤100, break otherwise
  - [ ] **Rust Tests**: Threshold behavior at boundary

## 1.3 Output Emitter

String building and output production.

- [ ] **Implement**: `Emitter` trait for output abstraction
  - [ ] **Rust Tests**: `ori_fmt/src/emitter/tests.rs`
- [ ] **Implement**: `StringEmitter` for in-memory output
  - [ ] **Rust Tests**: String concatenation
- [ ] **Implement**: `FileEmitter` for file output
  - [ ] **Rust Tests**: File write verification
- [ ] **Implement**: Newline handling (Unix-style `\n`)
  - [ ] **Rust Tests**: Consistent line endings
- [ ] **Implement**: Trailing newline enforcement
  - [ ] **Rust Tests**: File ends with single newline

## 1.4 Line Breaking Rules

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/line-breaking.md`

- [ ] **Implement**: Always-inline construct detection
  - [ ] **Rust Tests**: Simple expressions stay inline
- [ ] **Implement**: Always-stacked construct detection
  - [ ] **Rust Tests**: `run`, `try`, `match` always break
- [ ] **Implement**: Width-based breaking for flexible constructs
  - [ ] **Rust Tests**: Break at 100 chars
- [ ] **Implement**: Independent breaking for nested constructs
  - [ ] **Rust Tests**: Nested call breaks independently
- [ ] **Implement**: Trailing comma insertion for multi-line
  - [ ] **Rust Tests**: Comma added when broken
- [ ] **Implement**: Trailing comma removal for single-line
  - [ ] **Rust Tests**: Comma removed when inline

## 1.5 Indentation Rules

> **DESIGN**: `docs/tooling/formatter/design/01-algorithm/indentation.md`

- [ ] **Implement**: 4-space indentation increment
  - [ ] **Rust Tests**: Correct spacing at each level
- [ ] **Implement**: Tab-to-space conversion
  - [ ] **Rust Tests**: Tabs become 4 spaces
- [ ] **Implement**: Continuation indentation for broken expressions
  - [ ] **Rust Tests**: Arguments align properly
- [ ] **Implement**: Block indentation for nested bodies
  - [ ] **Rust Tests**: Function body indented

## 1.6 Blank Line Handling

- [ ] **Implement**: Single blank line between top-level items
  - [ ] **Rust Tests**: Functions separated by one blank line
- [ ] **Implement**: Blank line after import block
  - [ ] **Rust Tests**: Imports followed by blank line
- [ ] **Implement**: Blank line after constants block
  - [ ] **Rust Tests**: Constants followed by blank line
- [ ] **Implement**: Collapse consecutive blank lines
  - [ ] **Rust Tests**: Multiple blanks become one
- [ ] **Implement**: No trailing blank lines at end of file
  - [ ] **Rust Tests**: File ends with content + newline

## Completion Checklist

- [x] All width calculation tests pass
- [ ] All formatter core tests pass
- [ ] All line breaking tests pass
- [ ] All indentation tests pass
- [ ] Round-trip verification works

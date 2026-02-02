---
section: "01"
title: Quick Wins
status: not-started
phase: 1
goal: Low-effort, high-impact improvements to parser performance and error quality
reference_parsers:
  - Go (token bitsets)
  - Zig (scratch buffer)
  - Gleam/Roc (rich errors)
sections:
  - id: "1.1"
    title: Token Bitsets
    status: not-started
  - id: "1.2"
    title: Scratch Buffer
    status: not-started
  - id: "1.3"
    title: Rich Error Types
    status: not-started
  - id: "1.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 01: Quick Wins

**Status:** 📋 Planned
**Goal:** Low-effort, high-impact improvements to parser performance and error quality
**Timeline:** 1-2 weeks
**Dependencies:** None

---

## Overview

These improvements can be implemented independently with minimal disruption to existing code. Each provides measurable benefits:

| Improvement | Source | Benefit |
|-------------|--------|---------|
| Token Bitsets | Go | O(1) set membership, faster error recovery |
| Scratch Buffer | Zig | Reduced allocations during parsing |
| Rich Error Types | Gleam/Roc | Better diagnostics, IDE-quality errors |

---

## 1.1 Token Bitsets

> **Reference**: Go's `syntax/tokens.go` uses 64-bit bitsets for ultra-fast token set operations.

### Current State

```rust
// Current Ori implementation
pub struct RecoverySet {
    tokens: &'static [TokenKind],
}

impl RecoverySet {
    pub fn contains(&self, token: TokenKind) -> bool {
        self.tokens.iter().any(|&t| t == token)  // O(n) linear scan
    }
}
```

### Target State

```rust
// Proposed implementation
#[derive(Clone, Copy)]
pub struct TokenSet(u64);  // Assumes ≤64 token kinds

impl TokenSet {
    pub const fn empty() -> Self { Self(0) }

    pub const fn single(token: TokenKind) -> Self {
        Self(1 << token as u64)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub const fn contains(self, token: TokenKind) -> bool {
        (self.0 & (1 << token as u64)) != 0  // O(1) bitwise AND
    }
}

// Pre-defined sets
pub const STMT_BOUNDARY: TokenSet = TokenSet::single(TokenKind::At)
    .union(TokenSet::single(TokenKind::Use))
    .union(TokenSet::single(TokenKind::Pub));
```

### Tasks

- [ ] **Audit TokenKind count** — Verify ≤64 token kinds
  - If >64: Use `u128` or `[u64; 2]` for larger sets
  - Location: `compiler/ori_lexer/src/token.rs`

- [ ] **Implement TokenSet type**
  - [ ] Add `TokenSet` struct with bitwise operations
  - [ ] Implement `const fn` constructors for compile-time initialization
  - [ ] Add `#[inline]` hints for hot paths
  - Location: `compiler/ori_parse/src/recovery.rs`

- [ ] **Migrate RecoverySet**
  - [ ] Replace `RecoverySet` with `TokenSet`
  - [ ] Update all call sites in parser
  - [ ] Benchmark: Should show measurable improvement in error recovery paths

- [ ] **Add expected token tracking** (from Rust)
  - [ ] Track expected tokens during parsing for better error messages
  - [ ] "Expected X, Y, or Z, found W" style messages

### Verification

```bash
# Benchmark before/after
cargo bench --bench parser_recovery
# Or manual timing:
time cargo t -p ori_parse -- --ignored large_error_file
```

---

## 1.2 Scratch Buffer

> **Reference**: Zig's `Parse.zig` uses a reusable scratch buffer to avoid per-list allocations.

### Current State

Each list parse allocates a fresh `Vec`:

```rust
fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ParseError> {
    let mut args = Vec::new();  // New allocation per call
    // ...parse args...
    Ok(args)
}
```

### Target State

```rust
pub struct Parser<'a> {
    // ...existing fields...
    scratch: ScratchBuffer,  // Reusable buffer
}

pub struct ScratchBuffer {
    indices: Vec<u32>,  // Reusable index storage
}

impl ScratchBuffer {
    pub fn scope(&mut self) -> ScratchScope<'_> {
        ScratchScope {
            buffer: self,
            start: self.indices.len(),
        }
    }
}

pub struct ScratchScope<'a> {
    buffer: &'a mut ScratchBuffer,
    start: usize,
}

impl Drop for ScratchScope<'_> {
    fn drop(&mut self) {
        self.buffer.indices.truncate(self.start);  // RAII cleanup
    }
}
```

### Usage Pattern

```rust
fn parse_call_args(&mut self) -> Result<ExprRange, ParseError> {
    let scope = self.scratch.scope();  // RAII guard

    loop {
        if let Some(arg) = self.parse_call_arg()? {
            scope.push(arg.0);  // Push to scratch
        } else {
            break;
        }
    }

    // Copy to arena and return range
    let range = self.arena.alloc_expr_range(scope.items());
    Ok(range)
    // scope drops here, buffer truncated
}
```

### Tasks

- [ ] **Implement ScratchBuffer**
  - [ ] Add `ScratchBuffer` struct with index storage
  - [ ] Implement `ScratchScope` with RAII cleanup via `Drop`
  - [ ] Pre-size based on heuristics (source size / 50)
  - Location: `compiler/ori_parse/src/scratch.rs` (new file)

- [ ] **Integrate into Parser**
  - [ ] Add `scratch` field to `Parser` struct
  - [ ] Initialize in parser constructor

- [ ] **Migrate list parsing**
  - [ ] `parse_call_args` — function call arguments
  - [ ] `parse_params` — function parameters
  - [ ] `parse_generic_params` — generic type parameters
  - [ ] `parse_list_elements` — list literals
  - [ ] `parse_map_entries` — map literals
  - [ ] `parse_match_arms` — match arms

- [ ] **Benchmark memory usage**
  - [ ] Compare allocation counts before/after
  - [ ] Use `#[global_allocator]` with counting allocator in tests

### Verification

```bash
# Memory profiling
RUSTFLAGS="-Z allocation-counter" cargo t -p ori_parse
# Or use heaptrack/valgrind
```

---

## 1.3 Rich Error Types

> **Reference**: Gleam has 50+ error variants with contextual hints. Roc captures nested error context.

### Current State

```rust
pub struct ParseError {
    code: ErrorCode,
    message: String,
    span: Span,
    context: Option<String>,
}
```

### Target State

```rust
pub enum ParseErrorKind {
    // Token-level errors
    UnexpectedToken {
        found: TokenKind,
        expected: TokenSet,
        context: ParseContext,
    },
    UnexpectedEof {
        expected: TokenSet,
        unclosed: Option<(TokenKind, Span)>,  // Unclosed delimiter
    },

    // Expression errors
    ExpectedExpression {
        found: TokenKind,
        position: ExprPosition,  // Primary, Operand, etc.
    },
    TrailingOperator {
        operator: TokenKind,
        suggestion: Option<&'static str>,
    },

    // Declaration errors
    ExpectedDeclaration {
        found: TokenKind,
        hint: Option<&'static str>,
    },
    InvalidFunctionClause {
        reason: ClauseError,
    },

    // Pattern errors
    InvalidPattern {
        found: TokenKind,
        context: PatternContext,  // Match, Let, Function param
    },
    RefutableInIrrefutable {
        pattern_span: Span,
        binding_kind: BindingKind,
    },

    // Type errors (parsing)
    ExpectedType {
        found: TokenKind,
    },
    UnclosedGeneric {
        open_span: Span,
        hint: Option<&'static str>,
    },

    // Recovery markers
    SkippedTokens {
        count: usize,
        sync_point: TokenKind,
    },
}

pub struct ParseError {
    kind: ParseErrorKind,
    span: Span,
    notes: Vec<Note>,  // Additional context
}

pub struct Note {
    message: String,
    span: Option<Span>,  // Optional related location
}
```

### Contextual Hints (from Gleam)

```rust
impl ParseErrorKind {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::UnexpectedToken { found: TokenKind::Semicolon, .. } =>
                Some("Ori doesn't use semicolons; try removing it"),
            Self::UnexpectedToken { found: TokenKind::SingleQuote, .. } =>
                Some("Use double quotes for strings: \"hello\""),
            Self::TrailingOperator { operator: TokenKind::Plus, .. } =>
                Some("Did you mean to add another operand?"),
            _ => None,
        }
    }
}
```

### Tasks

- [ ] **Define ParseErrorKind enum**
  - [ ] Token-level errors (5-10 variants)
  - [ ] Expression errors (5-10 variants)
  - [ ] Declaration errors (5-10 variants)
  - [ ] Pattern errors (5-10 variants)
  - [ ] Type parsing errors (3-5 variants)
  - Location: `compiler/ori_parse/src/error.rs`

- [ ] **Implement contextual hints**
  - [ ] Common mistakes (semicolons, single quotes, etc.)
  - [ ] Unclosed delimiter detection
  - [ ] "Did you mean X?" suggestions

- [ ] **Add Note type for related locations**
  - [ ] "This '{' was opened here" style messages
  - [ ] "Previous clause defined here" for function clauses

- [ ] **Migrate existing error sites**
  - [ ] Replace `ParseError::new(code, message)` with specific variants
  - [ ] Add spans for all related locations
  - [ ] Preserve backward compatibility with ErrorCode enum

- [ ] **Update diagnostic rendering**
  - [ ] Integrate hints into error display
  - [ ] Show related locations
  - Location: `compiler/ori_diagnostic/src/`

### Example Output

```
error[E1001]: unexpected token
  --> src/main.ori:5:10
   |
 5 |     let x = ;
   |             ^ expected expression, found `;`
   |
   = hint: Ori doesn't use semicolons; try removing it
```

### Verification

- [ ] All existing parser tests still pass
- [ ] Error messages include contextual hints where applicable
- [ ] Related locations shown for delimiter mismatches

---

## 1.4 Section Completion Checklist

- [ ] **1.1 Token Bitsets**
  - [ ] TokenSet implemented with O(1) membership
  - [ ] RecoverySet migrated to TokenSet
  - [ ] Benchmarks show improvement

- [ ] **1.2 Scratch Buffer**
  - [ ] ScratchBuffer with RAII scope
  - [ ] All list-parsing functions migrated
  - [ ] Allocation counts reduced

- [ ] **1.3 Rich Error Types**
  - [ ] ParseErrorKind enum with 30+ variants
  - [ ] Contextual hints for common mistakes
  - [ ] Note type for related locations

- [ ] **Integration**
  - [ ] `cargo t -p ori_parse` passes
  - [ ] `./test-all` passes
  - [ ] No performance regressions

**Exit Criteria**: All three improvements integrated, tested, and benchmarked.

---

## Notes

- These improvements are independent and can be developed in parallel
- Token bitsets provide the largest immediate performance benefit
- Rich error types have the largest impact on user experience
- Scratch buffer is the simplest to implement but lowest impact

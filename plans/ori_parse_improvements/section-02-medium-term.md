---
section: "02"
title: Medium-Term Improvements
status: not-started
phase: 2
goal: Architectural improvements for better error recovery and memory efficiency
reference_parsers:
  - Rust (snapshots)
  - TypeScript (speculative)
  - Zig (two-tier storage)
  - Gleam (series combinator)
sections:
  - id: "2.1"
    title: Speculative Parsing with Snapshots
    status: not-started
  - id: "2.2"
    title: Two-Tier Inline/Overflow Storage
    status: not-started
  - id: "2.3"
    title: Series Combinator
    status: not-started
  - id: "2.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 02: Medium-Term Improvements

**Status:** 📋 Planned
**Goal:** Architectural improvements for better error recovery and memory efficiency
**Timeline:** 2-4 weeks
**Dependencies:** Section 01 (recommended but not required)

---

## Overview

These improvements require more significant architectural changes but provide substantial benefits:

| Improvement | Source | Benefit |
|-------------|--------|---------|
| Speculative Parsing | Rust/TypeScript | Better error messages for ambiguous syntax |
| Two-Tier Storage | Zig | 30-50% memory reduction for common cases |
| Series Combinator | Gleam | Cleaner, reusable list parsing code |

---

## 2.1 Speculative Parsing with Snapshots

> **Reference**: Rust's `create_snapshot_for_diagnostic()` and TypeScript's `lookAhead()/tryParse()`.

### Motivation

Some syntactic constructs are ambiguous until more tokens are consumed. Without snapshots, the parser must commit early and produce worse error messages.

**Example Ambiguity**:
```ori
// Is this a type annotation or a ternary-like expression?
let x: Option<int> = ...
//     ^^^^^^^^^^^^
// Parser sees "Option" then "<" - could be less-than comparison
```

### Current State

Ori uses progress tracking for backtracking decisions:

```rust
enum Progress { Made, None }

// If Progress::None on error, try next alternative
// If Progress::Made on error, commit to this path
```

This works for simple alternatives but doesn't help when we need to:
1. Try a parse speculatively
2. Examine the result
3. Decide whether to keep or discard it

### Target State

```rust
pub struct ParserSnapshot {
    cursor_pos: usize,
    error_count: usize,
    context: ParseContext,
}

impl Parser<'_> {
    /// Save parser state for potential rollback
    pub fn snapshot(&self) -> ParserSnapshot {
        ParserSnapshot {
            cursor_pos: self.cursor.position(),
            error_count: self.errors.len(),
            context: self.context,
        }
    }

    /// Restore parser state from snapshot
    pub fn restore(&mut self, snapshot: ParserSnapshot) {
        self.cursor.set_position(snapshot.cursor_pos);
        self.errors.truncate(snapshot.error_count);
        self.context = snapshot.context;
    }

    /// Try parsing, returning None and restoring state on failure
    pub fn try_parse<T, F>(&mut self, f: F) -> Option<T>
    where
        F: FnOnce(&mut Self) -> Result<T, ParseError>,
    {
        let snapshot = self.snapshot();
        match f(self) {
            Ok(result) => Some(result),
            Err(_) => {
                self.restore(snapshot);
                None
            }
        }
    }

    /// Look ahead without consuming, always restores state
    pub fn look_ahead<T, F>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut Self) -> T,
    {
        let snapshot = self.snapshot();
        let result = f(self);
        self.restore(snapshot);
        result
    }
}
```

### Use Cases

**1. Generic vs Comparison Disambiguation**:
```rust
fn parse_type_or_expr(&mut self) -> Result<TypeOrExpr, ParseError> {
    // Try parsing as type first
    if let Some(ty) = self.try_parse(|p| p.parse_type()) {
        // Check if followed by `=` (assignment) or other expr context
        if self.check(TokenKind::Eq) {
            return Ok(TypeOrExpr::Type(ty));
        }
    }

    // Fall back to expression
    let expr = self.parse_expr()?;
    Ok(TypeOrExpr::Expr(expr))
}
```

**2. Better Error Messages**:
```rust
fn parse_function(&mut self) -> Result<Function, ParseError> {
    // Try parsing with generics
    let snapshot = self.snapshot();
    match self.parse_function_with_generics() {
        Ok(f) => return Ok(f),
        Err(e) if self.is_likely_generic_error(&e) => {
            // Restore and try without generics for better error
            self.restore(snapshot);
            return self.parse_function_without_generics();
        }
        Err(e) => return Err(e),
    }
}
```

### Tasks

- [ ] **Implement ParserSnapshot**
  - [ ] Capture cursor position
  - [ ] Capture error count
  - [ ] Capture context flags
  - Location: `compiler/ori_parse/src/cursor.rs`

- [ ] **Add try_parse/look_ahead methods**
  - [ ] Generic over return type
  - [ ] Automatic state restoration
  - [ ] No allocation on snapshot

- [ ] **Integrate with existing progress tracking**
  - [ ] Snapshots complement progress, don't replace it
  - [ ] Use progress for simple alternatives
  - [ ] Use snapshots for complex disambiguation

- [ ] **Apply to ambiguous constructs**
  - [ ] Generic type vs comparison: `Option<int>` vs `Option < int`
  - [ ] Struct literal vs block: `Point { x }` vs `{ x }`
  - [ ] Typed lambda vs grouped: `(x: int) -> ...` vs `(x + 1)`

### Verification

- [ ] No performance regression (snapshots are lightweight)
- [ ] Error messages improved for ambiguous cases
- [ ] Existing tests pass without modification

---

## 2.2 Two-Tier Inline/Overflow Storage

> **Reference**: Zig uses inline storage for small cases (≤2 items) and overflow to extra_data for larger cases.

### Motivation

Most AST nodes have small children counts:
- ~60% of function calls have 0-2 arguments
- ~70% of generic params have 1 type parameter
- ~80% of match expressions have 2-4 arms

Storing these inline saves indirection and improves cache locality.

### Current State

All collections use the same representation:

```rust
pub struct ExprRange {
    start: u32,  // Offset into expr_lists
    len: u16,    // Length
}
```

Every access requires:
1. Load ExprRange from node
2. Index into expr_lists array
3. Load actual ExprId values

### Target State

**Option A: Tagged Union (Zig-style)**

```rust
pub enum Args {
    /// 0-2 args stored inline
    Inline {
        count: u8,
        args: [ExprId; 2],  // Padding reused
    },
    /// 3+ args stored in expr_lists
    Overflow {
        start: u32,
        len: u16,
    },
}
```

**Option B: Separate Node Variants (also Zig-style)**

```rust
pub enum ExprKind {
    // Small case: inline storage
    CallOne {
        callee: ExprId,
        arg: ExprId,
    },
    CallTwo {
        callee: ExprId,
        args: [ExprId; 2],
    },
    // General case: range into expr_lists
    Call {
        callee: ExprId,
        args: ExprRange,
    },
    // ...
}
```

### Analysis

| Approach | Pros | Cons |
|----------|------|------|
| Tagged Union | Cleaner API, single variant | Extra branch on access |
| Separate Variants | No runtime branch | Many variants, larger match arms |

**Recommendation**: Start with Tagged Union (Option A) for cleaner API. Measure performance before considering Option B.

### Tasks

- [ ] **Profile current allocation patterns**
  - [ ] Count calls with 0, 1, 2, 3+ arguments
  - [ ] Count generic params distribution
  - [ ] Count match arms distribution
  - Tool: Add instrumentation to parser, run on test corpus

- [ ] **Implement Args/Params/Items enum**
  - [ ] Inline variant for small counts
  - [ ] Overflow variant for large counts
  - [ ] Helper methods to abstract access
  - Location: `compiler/ori_parse/src/arena.rs`

- [ ] **Migrate ExprKind variants**
  - [ ] `Call` — function calls
  - [ ] `MethodCall` — method invocations
  - [ ] `GenericArgs` — type arguments
  - [ ] `MatchArms` — match expression arms
  - [ ] `ListElements` — list literals
  - [ ] `MapEntries` — map literals

- [ ] **Update arena allocation**
  - [ ] Only allocate to expr_lists when overflow
  - [ ] Benchmark memory usage reduction

- [ ] **Add exhaustive tests**
  - [ ] 0, 1, 2 args (inline path)
  - [ ] 3, 10, 100 args (overflow path)
  - [ ] Edge case: exactly 2 args

### Memory Analysis

```
Current (always ExprRange):
  ExprRange: 6 bytes
  + indirect access to expr_lists

With inline (Args enum):
  Inline: 1 + 8 = 9 bytes (but no indirection)
  Overflow: 1 + 6 = 7 bytes

For 1-2 args: Saves ~50% of access time (no indirection)
For 3+ args: Adds 1 byte overhead
```

### Verification

- [ ] Memory reduction measured (target: 20-30%)
- [ ] No performance regression for large collections
- [ ] All parser tests pass

---

## 2.3 Series Combinator

> **Reference**: Gleam's `series_of()` handles comma-separated lists with trailing separator support.

### Motivation

List parsing is repetitive and error-prone:
- Function parameters
- Generic arguments
- Call arguments
- List/map/struct literals
- Import items
- Match arms

A reusable combinator reduces duplication and ensures consistent error handling.

### Current State

Each list type has custom parsing logic:

```rust
fn parse_call_args(&mut self) -> Result<Vec<CallArg>, ParseError> {
    let mut args = Vec::new();
    loop {
        if self.check(TokenKind::RParen) { break; }
        args.push(self.parse_call_arg()?);
        if !self.eat(TokenKind::Comma) { break; }
    }
    Ok(args)
}

fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
    let mut params = Vec::new();
    loop {
        if self.check(TokenKind::RParen) { break; }
        params.push(self.parse_param()?);
        if !self.eat(TokenKind::Comma) { break; }
    }
    Ok(params)
}
// ... repeated for each list type
```

### Target State

```rust
pub struct SeriesConfig {
    separator: TokenKind,
    terminator: TokenKind,
    allow_trailing: bool,
    min_count: usize,
    max_count: Option<usize>,
}

impl<'a> Parser<'a> {
    /// Parse a series of items with configurable separators and terminators
    pub fn series<T, F>(
        &mut self,
        config: SeriesConfig,
        parse_item: F,
    ) -> Result<Vec<T>, ParseError>
    where
        F: Fn(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let mut items = Vec::new();

        loop {
            // Check for terminator
            if self.check(config.terminator) {
                break;
            }

            // Try to parse item
            match parse_item(self)? {
                Some(item) => items.push(item),
                None => {
                    if items.is_empty() {
                        break;  // Empty series allowed
                    } else {
                        // Error: expected item after separator
                        return Err(self.error_expected_item());
                    }
                }
            }

            // Check for separator
            if !self.eat(config.separator) {
                if !self.check(config.terminator) {
                    // Error: expected separator or terminator
                    return Err(self.error_expected_separator_or(config.terminator));
                }
                break;
            }

            // Record trailing separator position (for formatter)
            if self.check(config.terminator) && config.allow_trailing {
                self.record_trailing_separator();
                break;
            }
        }

        // Validate count constraints
        if items.len() < config.min_count {
            return Err(self.error_too_few_items(config.min_count));
        }
        if let Some(max) = config.max_count {
            if items.len() > max {
                return Err(self.error_too_many_items(max));
            }
        }

        Ok(items)
    }
}
```

### Convenience Methods

```rust
impl<'a> Parser<'a> {
    /// Comma-separated list in parentheses
    pub fn paren_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: Fn(&mut Self) -> Result<Option<T>, ParseError>,
    {
        self.expect(TokenKind::LParen)?;
        let items = self.series(
            SeriesConfig {
                separator: TokenKind::Comma,
                terminator: TokenKind::RParen,
                allow_trailing: true,
                min_count: 0,
                max_count: None,
            },
            parse_item,
        )?;
        self.expect(TokenKind::RParen)?;
        Ok(items)
    }

    /// Comma-separated list in brackets
    pub fn bracket_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: Fn(&mut Self) -> Result<Option<T>, ParseError>,
    {
        self.expect(TokenKind::LBracket)?;
        let items = self.series(
            SeriesConfig {
                separator: TokenKind::Comma,
                terminator: TokenKind::RBracket,
                allow_trailing: true,
                min_count: 0,
                max_count: None,
            },
            parse_item,
        )?;
        self.expect(TokenKind::RBracket)?;
        Ok(items)
    }

    /// Comma-separated list in braces
    pub fn brace_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: Fn(&mut Self) -> Result<Option<T>, ParseError>,
    {
        self.expect(TokenKind::LBrace)?;
        let items = self.series(
            SeriesConfig {
                separator: TokenKind::Comma,
                terminator: TokenKind::RBrace,
                allow_trailing: true,
                min_count: 0,
                max_count: None,
            },
            parse_item,
        )?;
        self.expect(TokenKind::RBrace)?;
        Ok(items)
    }
}
```

### Integration with Scratch Buffer

If Section 1.2 is implemented:

```rust
pub fn series_to_range<T, F>(
    &mut self,
    config: SeriesConfig,
    parse_item: F,
) -> Result<Range<T>, ParseError>
where
    F: Fn(&mut Self) -> Result<Option<T>, ParseError>,
    T: Into<u32>,
{
    let scope = self.scratch.scope();

    loop {
        if self.check(config.terminator) { break; }
        if let Some(item) = parse_item(self)? {
            scope.push(item.into());
        } else {
            break;
        }
        if !self.eat(config.separator) { break; }
    }

    // Allocate to arena from scratch
    let range = self.arena.alloc_range(scope.items());
    Ok(range)
}
```

### Tasks

- [ ] **Implement SeriesConfig struct**
  - [ ] Separator, terminator, trailing, min/max
  - Location: `compiler/ori_parse/src/series.rs` (new file)

- [ ] **Implement series() method**
  - [ ] Generic over item type and parse function
  - [ ] Error handling for missing separators
  - [ ] Trailing separator tracking

- [ ] **Add convenience methods**
  - [ ] `paren_series()` — `(item, item, ...)`
  - [ ] `bracket_series()` — `[item, item, ...]`
  - [ ] `brace_series()` — `{item, item, ...}`
  - [ ] `angle_series()` — `<item, item, ...>`

- [ ] **Migrate existing list parsing**
  - [ ] Function parameters
  - [ ] Generic parameters
  - [ ] Call arguments
  - [ ] List/map/struct literals
  - [ ] Import items
  - [ ] Match arms

- [ ] **Add error recovery**
  - [ ] Skip to next separator on item parse failure
  - [ ] Report all errors, not just first

### Verification

- [ ] Code reduction measured (target: 30-40% less list parsing code)
- [ ] Consistent error messages across all list types
- [ ] Trailing comma tracking works for formatter

---

## 2.4 Section Completion Checklist

- [ ] **2.1 Speculative Parsing**
  - [ ] ParserSnapshot implemented
  - [ ] try_parse/look_ahead methods work
  - [ ] Applied to at least 3 ambiguous constructs

- [ ] **2.2 Two-Tier Storage**
  - [ ] Args/Params enum with inline/overflow variants
  - [ ] Memory reduction measured (20%+)
  - [ ] All access patterns tested

- [ ] **2.3 Series Combinator**
  - [ ] series() method with config
  - [ ] Convenience methods for common delimiters
  - [ ] All list parsing migrated

- [ ] **Integration**
  - [ ] `cargo t -p ori_parse` passes
  - [ ] `./test-all` passes
  - [ ] Benchmarks show no regression

**Exit Criteria**: All three improvements integrated, tested, and measured.

---

## Notes

- 2.1 and 2.2 can be developed in parallel
- 2.3 benefits from 1.2 (scratch buffer) if implemented
- These changes touch core parser infrastructure — careful incremental migration recommended
- Consider feature flags during development for easy rollback

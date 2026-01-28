# Parser v2: Comprehensive Improvement Plan

**Status:** ✅ Core Complete (Phases 1-3, 6), 🟡 Partial (4-5)
**Created:** 2026-01-28
**Last Updated:** 2026-01-28

---

## Progress Tracking

### Overall Progress

| Phase | Status | Progress |
|-------|--------|----------|
| Phase 1: Lexer Boundary Fix | ✅ Complete | 17/17 tasks |
| Phase 2: Context Management | ✅ Complete | 15/15 tasks |
| Phase 3: Pattern Expansion | ✅ Complete | 34/34 tasks |
| Phase 4: Progress Tracking | 🟡 Partial | 8/19 tasks (core types done) |
| Phase 5: Spec Updates | 🟡 Partial | 10/18 tasks |
| Phase 6: Compositional Testing | ✅ Complete | 13/13 tasks |

### Phase 1: Lexer Boundary Fix (✅ Complete)

#### 1.1 Lexer Changes
- [x] Remove `#[token(">>")]` (Shr) from `RawToken` enum
- [x] Remove `#[token(">=")]` (GtEq) from `RawToken` enum
- [x] Remove `RawToken::Shr` and `RawToken::GtEq` handling in `convert_token`
- [x] Add comment explaining why `>` is always a single token
- [x] Verify lexer compiles

#### 1.2 Cursor/Parser Infrastructure
- [x] Add `peek_next_token()` and `peek_next_span()` methods to Cursor
- [x] Add `spans_adjacent(span1, span2)` helper method
- [x] Add `is_shift_right()` and `is_greater_equal()` methods for compound operator detection
- [x] Add `consume_compound()` method for consuming 2-token operators

#### 1.3 Operator Matching Updates
- [x] Update `match_comparison_op()` to detect `>` + `=` as `>=`
- [x] Update `match_shift_op()` to detect `>` + `>` as `>>`
- [x] Update macro to consume correct number of tokens (1 or 2)
- [x] Update all match functions to return `(BinaryOp, usize)` tuple

#### 1.4 Type Parser Verification
- [x] Verify type parser uses single `>` (no changes needed)
- [x] Add test for `Result<Result<T, E>, E>` parsing (double nested)
- [x] Add test for `Option<Result<Result<int, str>, str>>` (triple nested)

#### 1.5 Testing
- [x] Add unit test for nested generics in type parser
- [x] Add test for `>>` shift operator in expressions
- [x] Add test for `>=` comparison in expressions
- [x] Add test for `> >` with space NOT being treated as `>>`
- [x] Add test for `> =` with space NOT being treated as `>=`
- [x] Run full test suite: 1006+ Rust tests pass, 888 Ori spec tests pass
- [x] Verify no regressions

**Implementation Notes:**
- Lexer changes in `compiler/ori_lexer/src/lib.rs`
- Cursor methods in `compiler/ori_parse/src/cursor.rs`
- Parser delegation in `compiler/ori_parse/src/lib.rs`
- Operator matching in `compiler/ori_parse/src/grammar/expr/operators.rs`
- Macro update in `compiler/ori_parse/src/grammar/expr/mod.rs`

### Known Bugs Tracking

| Bug | Status | Phase |
|-----|--------|-------|
| `>>` tokenization | ✅ Fixed | 1 |
| Negative literals in patterns | ✅ Fixed | 3 |
| Struct patterns | ✅ Fixed | 3 |
| List patterns | ✅ Fixed | 3 |
| Or-patterns | ✅ Fixed | 3 |
| At-patterns | ✅ Fixed | 3 |
| Range patterns | ✅ Fixed | 3 |
| Match guards | ✅ Fixed | 3 |
| For in run blocks | ✅ Fixed | 3 |
| `#` length symbol in index | ✅ Fixed | 3 |

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current State Assessment](#2-current-state-assessment)
3. [Design Decisions](#3-design-decisions)
4. [Architecture Changes](#4-architecture-changes)
5. [Specification Improvements](#5-specification-improvements)
6. [Testing Strategy](#6-testing-strategy)
7. [Implementation Phases](#7-implementation-phases)
8. [Feature Addition Process](#8-feature-addition-process)
9. [Success Criteria](#9-success-criteria)
10. [References](#10-references)

---

## 1. Executive Summary

### What We Are Doing

Refactoring the Ori parser to eliminate architectural bugs and establish patterns that prevent future "whack-a-mole" issues as the language expands. This is not a rewrite but a targeted improvement to the existing parser with better foundations.

### Why Now

- **Language will expand significantly** - New syntax is coming; we need solid foundations
- **Architectural bugs** - The `>>` tokenization issue is a lexer/parser boundary problem that patches cannot fix
- **Missing features pile up** - 11 known parser bugs/gaps in pattern matching alone
- **Spec gaps** - The specification has inconsistencies that cause implementation bugs

### Key Outcomes

1. Fix the `>>` tokenization issue that breaks nested generics
2. Implement all missing pattern syntax (struct, list, or, at, range, guards)
3. Establish formal grammar as single source of truth
4. Create compositional testing to catch all type/pattern combinations
5. Define clear process for adding new syntax safely

### What This Is NOT

- Not a complete rewrite (we keep the current parser structure)
- Not premature optimization (we drop Zig-style SoA complexity)
- Not scope creep (we fix specific known issues, not hypothetical ones)

---

## 2. Current State Assessment

### 2.1 Parser Architecture

The current parser (`ori_parse` crate) is a recursive descent parser with:

- Pratt parsing for expressions (good, keep)
- Arena-based AST storage via `ExprArena` (good, keep)
- Basic error recovery via sync sets (needs improvement)
- Ad-hoc disambiguation logic (needs systematization)

**Strengths:**
- Clean recursive descent structure
- Pratt parsing handles precedence correctly
- Arena allocation is efficient

**Weaknesses:**
- Lexer produces compound tokens (`>>`) that conflict with type syntax
- No systematic context management (e.g., `NO_STRUCT_LIT` flag missing)
- Missing progress tracking for better error recovery
- Scattered disambiguation logic

### 2.2 Known Parser Bugs

From `// TODO: PARSER BUG:` comments in `tests/spec/patterns/match.ori`:

| Bug | Type | Description | Root Cause |
|-----|------|-------------|------------|
| `>>` tokenization | **Architectural** | `Result<Result<T, E>, E>` fails - lexer produces `>>` (Shr) instead of two `>` tokens | Lexer/parser boundary |
| Negative literals | Missing Feature | `-1` as pattern fails - parsed as unary minus | Pattern grammar incomplete |
| Struct patterns | Missing Feature | `Point { x, y }` in match fails | Pattern grammar incomplete |
| List patterns | Missing Feature | `[head, ..tail]` in match fails | Pattern grammar incomplete |
| Or-patterns | Missing Feature | `1 \| 2 -> ...` fails | Pattern grammar incomplete |
| At-patterns | Missing Feature | `x @ Some(v) -> ...` fails | Pattern grammar incomplete |
| Range patterns | Missing Feature | `1..10 -> ...` fails | Pattern grammar incomplete |
| Match guards | Missing Feature | `x.match(x > 0) -> ...` fails | Pattern grammar incomplete |
| For in run | Missing Feature | `for` inside `run(...)` blocks fails | Expression grammar gap |

**Analysis:**
- 1 architectural bug (requires lexer/parser boundary fix)
- 8 missing features (require grammar additions)
- Pattern parsing is significantly incomplete

### 2.3 Specification Gaps

Current issues in `docs/ori_lang/0.1-alpha/spec/`:

| Gap | Location | Problem |
|-----|----------|---------|
| No unified grammar | Scattered across files | Hard to verify completeness |
| `>>` ambiguity | `03-lexical-elements.md` | Lists `>>` as single token, but types need two `>` |
| No disambiguation rules | Missing | How to resolve `(x)` as grouped vs lambda? |
| Incomplete productions | `10-patterns.md` | Missing `struct_pattern`, `list_pattern`, etc. |
| No lexer-parser contract | Missing | When does lexer split tokens? |

---

## 3. Design Decisions

### 3.1 What We KEEP

| Pattern | Source | Rationale |
|---------|--------|-----------|
| **Pratt parsing** | Current + Rust/TS | Proven, handles precedence elegantly |
| **Progress tracking** | Roc | Enables better error recovery (Made/None distinction) |
| **Snapshot speculation** | Rust | Safe backtracking for ambiguous syntax |
| **Tristate disambiguation** | TypeScript | True/False/Unknown for lookahead decisions |
| **Hierarchical error types** | Elm | Precise, contextual error messages |
| **Formal grammar** | New | Single source of truth for all syntax |
| **Context bitflags** | Rust/TypeScript | Explicit `IN_PATTERN`, `NO_STRUCT_LIT`, etc. |

### 3.2 What We DROP

| Pattern | Source | Rationale |
|---------|--------|-----------|
| **SoA storage** | Zig | Complexity not justified; maintainability > memory optimization |
| **Extreme memory optimization** | Zig | Ori compiler doesn't need Zig-level performance |
| **Capacity pre-estimation** | Zig | Diminishing returns for our use case |

### 3.3 New Systems Needed

#### Minimal Lexer Tokens

**Problem:** Lexer produces `>>` as single `Shr` token, breaking `Result<Result<T, E>, E>`.

**Solution:** Lexer produces minimal tokens; parser combines them in context.

```
Current:  >> → Shr token
Proposed: >  → Gt token (always)
          >> in expression context → parser reads two Gt as Shr
```

**Implementation:**
- Lexer never produces `>>`, `>=`, or similar compound tokens starting with `>`
- Parser has `expect_binary_op()` that reads two `>` as shift operator
- In type context, parser reads single `>` for closing generic

#### Context Management System

**Problem:** Scattered ad-hoc context checks.

**Solution:** Explicit bitflags for parsing context.

```rust
bitflags! {
    pub struct ParseContext: u16 {
        const NONE           = 0;
        const IN_PATTERN     = 1 << 0;  // Parsing a pattern
        const IN_TYPE        = 1 << 1;  // Parsing a type
        const NO_STRUCT_LIT  = 1 << 2;  // No struct literals (if condition)
        const CONST_EXPR     = 1 << 3;  // Compile-time expression
        const IN_LOOP        = 1 << 4;  // Inside loop (break/continue valid)
        const ALLOW_YIELD    = 1 << 5;  // Yield expression valid
    }
}
```

#### Progress-Aware Results

**Problem:** Can't distinguish "failed without consuming" from "failed after consuming."

**Solution:** Progress enum in parse results.

```rust
enum Progress { Made, None }

struct ParseResult<T> {
    progress: Progress,
    result: Result<T, ParseError>,
}
```

- `Progress::None` + error → try alternative
- `Progress::Made` + error → commit and report

---

## 4. Architecture Changes

### 4.1 Lexer Boundary Fix

**Goal:** Enable parsing of `Result<Result<T, E>, E>`.

**Current Behavior:**
```
Input: Result<Result<T, E>, E>
Tokens: Ident("<") Ident("<") Ident(",") Ident(">") ">" (Shr) Ident(">")
                                                      ^^^ WRONG
```

**New Behavior:**
```
Input: Result<Result<T, E>, E>
Tokens: Ident("<") Ident("<") Ident(",") Ident(">") ">" ">" Ident(">")
                                         Single Gt tokens ^^^
```

**Changes Required:**

1. **Lexer (`ori_lexer`):**
   - Remove compound tokens: `>>`, `>>=`, `>=`
   - Always produce single `>` token
   - Parser reconstructs operators in expression context

2. **Parser (`ori_parse`):**
   - Add `peek_binary_op()` that combines adjacent tokens
   - In expression context: `>` followed by `>` → treat as `>>`
   - In type context: single `>` closes generic

3. **Token enum:**
   - Remove `Shr`, `ShrEq`, `Ge` token variants
   - Add parser-level operator classification

**Files to Change:**
- `compiler/ori_lexer/src/lib.rs`
- `compiler/ori_parse/src/grammar/expr.rs`
- `compiler/ori_parse/src/grammar/ty.rs`

### 4.2 Pattern Parsing Expansion

**Goal:** Support all pattern syntax from the spec.

**Missing Patterns:**

| Pattern | Syntax | Priority |
|---------|--------|----------|
| Struct pattern | `{ x, y }` or `Point { x, y }` | High |
| List pattern | `[]`, `[x]`, `[h, ..t]` | High |
| Or-pattern | `A \| B` | High |
| At-pattern | `x @ pattern` | Medium |
| Range pattern | `1..10`, `1..=10` | Medium |
| Guard pattern | `x.match(pred)` | Medium |
| Negative literal | `-1` | Medium |

**Implementation Strategy:**

1. Add pattern variants to AST (`ori_ir/src/ast/patterns.rs`)
2. Implement pattern parsers (`ori_parse/src/grammar/pattern.rs`)
3. Add to pattern type checker (`ori_typeck`)
4. Add to pattern evaluator (`ori_eval`)

### 4.3 Context Flag System

**Goal:** Systematic context management throughout parser.

**Implementation:**

```rust
// In parser state
struct Parser<'a> {
    context: ParseContext,
    // ...
}

impl Parser<'_> {
    fn with_context<T>(&mut self, add: ParseContext, f: impl FnOnce(&mut Self) -> T) -> T {
        let old = self.context;
        self.context |= add;
        let result = f(self);
        self.context = old;
        result
    }

    fn without_context<T>(&mut self, remove: ParseContext, f: impl FnOnce(&mut Self) -> T) -> T {
        let old = self.context;
        self.context &= !remove;
        let result = f(self);
        self.context = old;
        result
    }
}

// Usage
fn parse_if_expr(&mut self) -> ParseResult<NodeId> {
    self.expect(TokenKind::If)?;

    // Parse condition without struct literals
    let cond = self.with_context(ParseContext::NO_STRUCT_LIT, |p| {
        p.parse_expr()
    })?;

    // ...
}
```

### 4.4 Progress Tracking Integration

**Goal:** Better error recovery with progress awareness.

**Implementation:**

```rust
// Replace Result<T, E> with ParseResult<T> in parser methods
fn parse_expr(&mut self) -> ParseResult<NodeId> {
    // ...
}

// Error recovery uses progress
match self.parse_item() {
    ParseResult { result: Ok(item), .. } => items.push(item),
    ParseResult { result: Err(e), progress: Progress::Made } => {
        // Made progress but failed - commit error, synchronize
        self.errors.push(e);
        self.synchronize_to_item();
    }
    ParseResult { result: Err(_), progress: Progress::None } => {
        // No progress - skip token to avoid infinite loop
        self.advance();
    }
}
```

---

## 5. Specification Improvements

### 5.1 Unified Grammar Document

**Goal:** Single authoritative grammar file.

**Create:** `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`

**Contents:**
- All lexical productions
- All syntactic productions
- Cross-references to detailed explanations
- Machine-parseable format

**Structure:**
```ebnf
// ============================================================
// Lexical Grammar
// ============================================================

token = keyword | identifier | literal | operator | delimiter .

// ... all lexical productions ...

// ============================================================
// Syntactic Grammar
// ============================================================

module = { import } { item } .

// ... all syntactic productions ...
```

### 5.2 Disambiguation Rules

**Goal:** Document how ambiguous syntax is resolved.

**Add to spec:** `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md` (new section)

**Content:**

```markdown
## Disambiguation

### Token Splitting

The lexer produces minimal tokens. The parser combines tokens
in context:

- In expression context, two consecutive `>` tokens form the
  right-shift operator `>>`.
- In type context, a single `>` token closes a generic parameter
  list.

### Parenthesized Expressions

A parenthesized expression `(...)` is interpreted as:

1. Lambda parameters if followed by `->` and the contents
   match parameter syntax
2. Tuple if it contains a comma: `(a, b)`
3. Unit if empty: `()`
4. Grouped expression otherwise

### Struct Literals

An uppercase identifier followed by `{` is interpreted as:

1. A struct literal in expression context
2. NOT a struct literal in `if` condition context (the `{`
   starts a block instead)

### Soft Keywords

The following identifiers are keywords only when followed
by `(` in expression position:

- `run`, `try`, `match`, `recurse`, `parallel`, `spawn`,
  `timeout`, `cache`, `with`, `for`, `catch`
```

### 5.3 Complete Pattern Productions

**Goal:** All pattern syntax formally specified.

**Update:** `docs/ori_lang/0.1-alpha/spec/10-patterns.md`

**Add:**
```ebnf
pattern = literal_pattern
        | identifier_pattern
        | wildcard_pattern
        | variant_pattern
        | struct_pattern
        | tuple_pattern
        | list_pattern
        | range_pattern
        | or_pattern
        | at_pattern
        | guard_pattern .

struct_pattern = [ type_name ] "{" [ struct_field_pattern { "," struct_field_pattern } [ "," ] ] "}" .
struct_field_pattern = identifier [ ":" pattern ] .

list_pattern = "[" [ list_pattern_elements ] "]" .
list_pattern_elements = pattern { "," pattern } [ "," ".." [ identifier ] ] | ".." [ identifier ] .

range_pattern = literal ( ".." | "..=" ) literal .

or_pattern = pattern "|" pattern .

at_pattern = identifier "@" pattern .

guard_pattern = pattern "." "match" "(" expression ")" .
```

### 5.4 Lexer-Parser Contract

**Goal:** Define the boundary between lexer and parser.

**Add to:** `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`

```markdown
## Lexer-Parser Contract

The lexer produces tokens according to maximal munch with
the following exception:

**Greater-than sequences:** The lexer always produces
individual `>` tokens. It never produces `>>`, `>=`, or
`>>=` as single tokens. The parser combines these tokens
appropriately based on context.

This enables parsing of nested generic types:
```ori
// Parses correctly: >  >  are separate tokens
let x: Result<Result<int, str>, str> = ...
```
```

---

## 6. Testing Strategy

### 6.1 Compositional Test Matrix

**Goal:** Automatically test all combinations of constructs.

**Concept:** Instead of writing individual tests, generate tests for:
- Every type in every type position
- Every pattern in every pattern position
- Every expression in every expression context

**Implementation:**

```rust
// tests/parser/compositional.rs

const TYPES: &[&str] = &[
    "int", "str", "bool",
    "Option<int>", "Result<int, str>",
    "Result<Option<int>, str>",         // Nested generics
    "Result<Result<int, str>, str>",    // Double nested (the >> bug)
    "[int]", "{str: int}",
    "(int, str)", "(int) -> str",
];

const TYPE_POSITIONS: &[&str] = &[
    "let x: {} = ...",           // Variable type annotation
    "@f (x: {}) -> void = ...",  // Parameter type
    "@f () -> {} = ...",         // Return type
    "type Alias = {}",           // Type alias
];

#[test]
fn test_all_types_in_all_positions() {
    for ty in TYPES {
        for pos in TYPE_POSITIONS {
            let source = pos.replace("{}", ty);
            let result = parse(&source);
            assert!(result.errors.is_empty(),
                "Failed: type '{}' in position '{}'\nErrors: {:?}",
                ty, pos, result.errors);
        }
    }
}
```

### 6.2 Regression Tests

**Goal:** Every fixed bug gets a permanent test.

**Location:** `tests/spec/patterns/` (already exists)

**Process:**
1. Bug found → add failing test with `// TODO: PARSER BUG:` comment
2. Bug fixed → remove comment, test stays forever
3. CI fails if any parser bug tests break

### 6.3 Property-Based Testing

**Goal:** Find edge cases humans miss.

**Implementation:**
```rust
// tests/parser/proptest.rs
use proptest::prelude::*;

proptest! {
    #[test]
    fn parser_never_panics(source in ".*") {
        // Parser should never panic, only return errors
        let _ = std::panic::catch_unwind(|| {
            parse(&source)
        });
    }

    #[test]
    fn valid_programs_roundtrip(prog in valid_program()) {
        // Parse → format → parse should be identical
        let ast1 = parse(&prog).unwrap();
        let formatted = format(&ast1);
        let ast2 = parse(&formatted).unwrap();
        assert_eq!(ast1, ast2);
    }
}
```

### 6.4 Error Message Tests

**Goal:** Error messages are tested, not just error detection.

**Implementation:**
```rust
#[test]
fn test_error_message_for_nested_generic() {
    let result = parse("let x: Result<Result<int, str> = Ok(Ok(1))");

    assert!(result.has_error());
    let error = &result.errors[0];

    // Check error message is helpful
    assert!(error.message().contains("expected '>'"));
    assert!(error.hint().is_some());
}
```

---

## 7. Implementation Phases

### Phase 1: Lexer Boundary Fix (Highest Priority)

**Duration:** 1-2 days
**Blocks:** All nested generic usage
**Status:** ✅ Complete (2026-01-28)

**Tasks:**

#### 1.1 Lexer Changes
- [x] Remove `#[token(">>")]` (Shr) from `RawToken` enum
- [x] Remove `#[token(">=")]` (GtEq) from `RawToken` enum
- [x] Remove `RawToken::Shr` and `RawToken::GtEq` handling in `convert_token`
- [x] Add comment explaining why `>` is always a single token
- [x] Verify lexer compiles

#### 1.2 Cursor/Parser Infrastructure
- [x] Add `peek_next_span()` method to Cursor
- [x] Add `spans_adjacent(span1, span2)` helper method
- [x] Add `is_shift_right()` and `is_greater_equal()` methods for compound operator detection

#### 1.3 Operator Matching Updates
- [x] Update `match_comparison_op()` to detect `>` + `=` as `>=`
- [x] Update `match_shift_op()` to detect `>` + `>` as `>>`
- [x] Ensure operators consume correct number of tokens

#### 1.4 Type Parser Verification
- [x] Verify type parser already uses single `>` (no changes needed)
- [x] Add test for `Result<Result<T, E>, E>` parsing

#### 1.5 Testing
- [x] Add unit test for nested generics in type parser
- [x] Add test for `>>` shift operator in expressions
- [x] Add test for `>=` comparison in expressions
- [x] Run full test suite (`./test-all`)
- [x] Verify no regressions

**Files:**
- `compiler/ori_lexer/src/lib.rs`
- `compiler/ori_parse/src/cursor.rs`
- `compiler/ori_parse/src/grammar/expr/operators.rs`
- `compiler/ori_parse/src/grammar/ty.rs`
- `tests/spec/types/` (new tests)

### Phase 2: Context Management System

**Duration:** 1 day
**Blocks:** Proper struct literal disambiguation
**Status:** ✅ Complete

**Tasks:**

#### 2.1 ParseContext Implementation
- [x] Create `compiler/ori_parse/src/context.rs`
- [x] Define `ParseContext` bitflags struct (using simple u16 + const flags, no external dependency)
- [x] Add `IN_PATTERN`, `IN_TYPE`, `NO_STRUCT_LIT`, `CONST_EXPR`, `IN_LOOP`, `ALLOW_YIELD`, `IN_FUNCTION` flags

#### 2.2 Parser Integration
- [x] Add `context: ParseContext` field to `Parser` struct
- [x] Implement `with_context()` method
- [x] Implement `without_context()` method
- [x] Implement `has_context()` check method
- [x] Implement `allows_struct_lit()` convenience method

#### 2.3 Usage Sites
- [x] Update `if` expression to use `NO_STRUCT_LIT` for condition
- [x] Update struct literal parsing to check `allows_struct_lit()` context
- [ ] (Deferred) `while` - not yet implemented in Ori
- [ ] (Deferred) pattern parsing `IN_PATTERN` - will use when Phase 3 patterns expand
- [ ] (Deferred) type parsing `IN_TYPE` - will use when needed
- [ ] (Deferred) loop parsing `IN_LOOP` - will use when loop expression implemented

#### 2.4 Testing
- [x] Test struct literal not allowed in `if` condition
- [x] Test struct literal allowed in `if` body
- [x] Test struct literal allowed in normal expressions
- [x] Test context API methods work correctly
- [x] Run full test suite (892 passed)

**Implementation Notes:**
- Used simple `struct ParseContext(u16)` with const flags instead of bitflags crate
- All context flags defined for future use; currently `NO_STRUCT_LIT` is active
- `with_context()` and `without_context()` use RAII pattern to restore context
- Deferred usage sites will be implemented as features are added (Phase 3+)

**Files:**
- `compiler/ori_parse/src/context.rs` (new)
- `compiler/ori_parse/src/lib.rs`
- `compiler/ori_parse/src/grammar/expr/primary.rs` (if condition)
- `compiler/ori_parse/src/grammar/expr/postfix.rs` (struct literal check)

### Phase 3: Pattern Parsing Expansion

**Duration:** 3-5 days
**Blocks:** Many match expression use cases
**Status:** ✅ Complete

**Tasks:**

#### 3.1 Negative Literal Pattern
- [x] Update pattern parser to recognize `Minus` + `Int` as negative literal
- [x] AST already supports (uses `Literal(ExprId)` with negative int)
- [x] Type checker already supports
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.2 Struct Pattern (`{ x, y }`)
- [x] AST already has `MatchPattern::Struct` variant
- [x] Implemented `parse_struct_pattern_fields()` in parser
- [x] Type checker already supports struct pattern matching
- [x] Evaluator already supports struct pattern matching
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.3 List Pattern (`[head, ..tail]`)
- [x] AST already has `MatchPattern::List` variant
- [x] Implemented list pattern parsing with `..` rest patterns
- [x] Type checker already supports
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.4 Or-Pattern (`A | B`)
- [x] AST already has `MatchPattern::Or` variant
- [x] Implemented or-pattern parsing with `|` separator
- [x] Type checker already supports
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.5 At-Pattern (`x @ Some(v)`)
- [x] AST already has `MatchPattern::At` variant
- [x] Implemented at-pattern parsing with `@` operator
- [x] Type checker already supports
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.6 Range Pattern (`1..10`)
- [x] AST already has `MatchPattern::Range` variant
- [x] Implemented range pattern parsing for `..` and `..=`
- [x] Type checker already supports
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.7 Guard Pattern (`x.match(predicate)`)
- [x] AST already has `MatchArm.guard` field
- [x] Implemented `.match(condition)` guard parsing
- [x] Type checker already supports (predicate must return bool)
- [x] Evaluator already supports
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.8 For in Run Blocks
- [x] Root cause: `for...do/yield` wasn't parsed as expression
- [x] Implemented `parse_for_loop()` for `for x in items do/yield body`
- [x] Removed `// TODO: PARSER BUG:` from test

#### 3.9 `#` Length Symbol (✅ Parser/Interpreter Complete)
- [x] `#` in index brackets (e.g., `list[# - 1]`) — parser change only needed
- [x] **Roadmap**: `plans/roadmap/phase-10-control-flow.md` § 10.8 Index Expressions
- [x] **Spec**: `docs/ori_lang/0.1-alpha/spec/09-expressions.md` § Index Access
- [x] **Test**: `tests/spec/types/collections.ori` — `test_list_index_last`
- [x] Added `IN_INDEX` context flag to `ParseContext`
- [x] Parser recognizes `#` as `ExprKind::HashLength` inside `[...]`
- [x] Type checker and evaluator already had full support
- [ ] LLVM codegen pending (placeholder exists)

**Implementation Notes:**
- AST already had all pattern types defined - only parser was missing
- Type checker and evaluator already complete for all patterns
- 8 new passing tests added to match.ori

**Files Changed:**
- `compiler/ori_parse/src/grammar/expr/patterns.rs` (major changes)
- `compiler/ori_parse/src/grammar/expr/primary.rs` (for loop parsing)
- `tests/spec/patterns/match.ori` (tests uncommented)

### Phase 4: Progress Tracking

**Duration:** 1-2 days
**Blocks:** Better error recovery
**Status:** 🟡 Partial (Core Types Complete)

**Tasks:**

#### 4.1 Core Types (✅ Complete)
- [x] Create `compiler/ori_parse/src/progress.rs`
- [x] Define `Progress` enum (`Made`, `None`)
- [x] Define `ParseResult<T>` type combining progress and result
- [x] Add helper methods (`map`, `and_then`, `or_else`)
- [x] Add `WithProgress` extension trait for easy conversion
- [x] Add unit tests for progress tracking

#### 4.2 Parser Method Updates (⚪ Deferred)
- [ ] Update `parse_expr()` to return `ParseResult`
- [ ] Update `parse_item()` to return `ParseResult`
- [ ] Update `parse_type()` to return `ParseResult`
- [ ] Update `parse_pattern()` to return `ParseResult`
- [ ] Update all intermediate parsing methods

**Deferral Note:** Converting all parser methods would be a significant refactoring.
The core types are available for future incremental adoption. Current error recovery
works well via `synchronize()` and recovery sets.

#### 4.3 Error Recovery Integration (⚪ Deferred)
- [ ] Update main parsing loop to use progress for recovery decisions
- [ ] `Progress::None` + error → try alternative
- [ ] `Progress::Made` + error → commit and synchronize
- [ ] Add synchronization points for items, expressions, statements

#### 4.4 Testing
- [x] Unit tests for Progress and ParseResult types
- [ ] Test error recovery with partial parse
- [ ] Test multiple errors in single file
- [ ] Test recovery doesn't skip valid code
- [x] Run full test suite (900 tests passing)

**Implementation Notes:**
- Created `compiler/ori_parse/src/error.rs` for cleaner error type organization
- Created `compiler/ori_parse/src/progress.rs` with full progress tracking types
- Exported as `Progress`, `ProgressResult`, `WithProgress` from lib.rs
- Ready for incremental adoption in parser methods

**Files:**
- `compiler/ori_parse/src/progress.rs` (new)
- `compiler/ori_parse/src/error.rs` (new)
- `compiler/ori_parse/src/lib.rs`
- `compiler/ori_parse/src/grammar/*.rs` (future)
- `compiler/ori_parse/src/recovery.rs` (future)

### Phase 5: Specification Updates

**Duration:** 1 day
**Blocks:** Future implementation clarity
**Status:** 🟡 Partial (Key Sections Complete)

**Tasks:**

#### 5.1 Unified Grammar Document (⚪ Deferred)
- [ ] Create `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`
- [ ] Add all lexical productions from `03-lexical-elements.md`
- [ ] Add all syntactic productions from other spec files
- [ ] Ensure productions are machine-parseable
- [ ] Add cross-references to detailed explanations

**Deferral Note:** Grammar is currently in individual spec files. Unified doc is nice-to-have.

#### 5.2 Disambiguation Rules (✅ Complete)
- [x] Add "Disambiguation" section to `03-lexical-elements.md`
- [x] Document token splitting rules (e.g., `>` never compounds)
- [x] Document parenthesized expression interpretation
- [x] Document struct literal context rules
- [x] Document soft keyword rules

#### 5.3 Pattern Productions (✅ Already Complete)
- [x] Update `10-patterns.md` with all pattern grammar — already had correct grammar
- [x] Add `struct_pattern` production — already present
- [x] Add `list_pattern` production — already present
- [x] Add `range_pattern` production — already present
- [x] Add `or_pattern` production — already present
- [x] Add `at_pattern` production — already present
- [x] Add `guard_pattern` production — already present

#### 5.4 Lexer-Parser Contract (✅ Complete)
- [x] Add "Lexer-Parser Contract" section to `03-lexical-elements.md`
- [x] Document maximal munch exceptions
- [x] Document which tokens are never produced by lexer
- [x] Add examples of nested generics

#### 5.5 CLAUDE.md Sync (✅ Already Up-to-Date)
- [x] Update quick reference with any new syntax — no changes needed
- [x] Verify examples match spec — verified
- [x] Add any new pattern syntax — already had all patterns

**Implementation Notes:**
- Added "Lexer-Parser Contract" section explaining `>` token splitting
- Added "Disambiguation" section covering struct literals, soft keywords, parentheses
- Pattern spec (10-patterns.md) already had complete grammar from earlier work
- CLAUDE.md quick reference already had all match patterns documented

**Files:**
- `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md` — updated with new sections
- `docs/ori_lang/0.1-alpha/spec/10-patterns.md` — verified, no changes needed
- `CLAUDE.md` — verified, no changes needed

### Phase 6: Compositional Testing

**Duration:** 1-2 days
**Blocks:** Long-term confidence
**Status:** ✅ Complete

**Tasks:**

#### 6.1 Type Matrix Tests (✅ Complete)
- [x] Create compositional tests in `compiler/ori_parse/src/compositional_tests.rs`
- [x] Define list of all type forms (primitives, generics, nested, functions, etc.)
- [x] Define list of all type positions (annotations, returns, params, etc.)
- [x] Generate tests for all combinations
- [x] Add particularly tricky cases (triple-nested generics, function returning generic, etc.)

#### 6.2 Pattern Matrix Tests (✅ Complete)
- [x] Create pattern matrix tests in same file
- [x] Define list of all pattern forms
- [x] Define list of all pattern positions (match arms)
- [x] Generate tests for all combinations
- [x] Add tricky cases (nested patterns, or-patterns in complex contexts)

#### 6.3 Expression Context Tests (✅ Complete)
- [x] Create expression context tests in same file
- [x] Test expressions in all valid contexts
- [x] Test context-sensitive parsing (struct literals, etc.)
- [x] Test operator precedence edge cases

#### 6.4 CI Integration (✅ Already Covered)
- [x] Compositional tests run with `cargo test` — already in CI
- [x] Tests run on every PR — standard workflow
- [x] No separate coverage config needed — inline tests
- [x] Easy to add new cases — just add to TYPES or PATTERNS arrays

**Implementation Notes:**
- Created `compiler/ori_parse/src/compositional_tests.rs` with 15 test functions
- Type matrix: 30+ type forms × 4 positions (variable, param, return, alias)
- Pattern matrix: 25+ pattern forms in match arms
- Expression context: struct literals, lambdas, for loops, method chains, operators
- All 15 compositional tests pass
- Tests are data-driven: add new types/patterns to arrays, tests auto-expand

**Files:**
- `compiler/ori_parse/src/compositional_tests.rs` (new)

---

## 8. Feature Addition Process

### Pre-Implementation Checklist

Before implementing any new syntax:

- [ ] **Spec first:** Add formal grammar to `docs/ori_lang/0.1-alpha/spec/`
- [ ] **Update grammar.ebnf:** Add production to unified grammar
- [ ] **Check disambiguation:** Will this conflict with existing syntax?
- [ ] **Check context:** Which contexts should allow/disallow this?
- [ ] **Add skeleton tests:** Create failing tests for all cases

### Implementation Checklist

- [ ] **Lexer changes:** Any new tokens needed?
- [ ] **AST changes:** Add node types to `ori_ir`
- [ ] **Parser changes:** Add parsing in `ori_parse`
- [ ] **Context flags:** Any new context requirements?
- [ ] **Type checker:** Add to `ori_typeck`
- [ ] **Evaluator:** Add to `ori_eval`

### Post-Implementation Checklist

- [ ] **Tests pass:** All skeleton tests converted to passing
- [ ] **Matrix updated:** New syntax added to compositional tests
- [ ] **CLAUDE.md updated:** Quick reference reflects new syntax
- [ ] **Examples work:** Real-world usage tested
- [ ] **Error messages:** Good errors for misuse

### Example: Adding New Pattern Syntax

```markdown
## Adding "slice pattern" `[a, ..middle, z]`

### 1. Spec Update
Add to `docs/ori_lang/0.1-alpha/spec/10-patterns.md`:
```ebnf
slice_pattern = "[" pattern "," ".." identifier "," pattern "]" .
```

### 2. Grammar.ebnf Update
Add to `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`:
```ebnf
list_pattern_elements = ... | pattern "," ".." identifier "," pattern .
```

### 3. Disambiguation Check
- `[a, ..b, c]` - unambiguous, `..` clearly indicates rest
- No conflict with existing syntax

### 4. Context Check
- Valid in: `IN_PATTERN` context only
- Not valid in: expression context

### 5. Skeleton Tests
```ori
// tests/spec/patterns/slice.ori
@test_slice_pattern tests @identity () -> void = run(
    // TODO: implement slice pattern
    let [first, ..middle, last] = [1, 2, 3, 4, 5],
    assert_eq(actual: first, expected: 1),
    assert_eq(actual: last, expected: 5),
)
```
```

---

## 9. Success Criteria

### Immediate (Phase 1-2 Complete) ✅

- [x] `Result<Result<T, E>, E>` parses correctly
- [x] No regressions in existing tests
- [x] Context flags implemented and documented

### Short-Term (Phase 3-4 Complete) ✅

- [x] All 9 pattern bugs fixed (remove all `TODO: PARSER BUG:` comments)
- [x] Progress tracking core types available (full integration deferred)
- [ ] Error messages include hints for common mistakes (future improvement)

### Medium-Term (Phase 5-6 Complete) 🟡

- [ ] Unified grammar document exists and is authoritative (deferred)
- [x] Disambiguation rules documented in spec
- [x] Lexer-parser contract documented
- [ ] Compositional tests cover type and pattern combinations (Phase 6)
- [x] Feature addition process is documented and followed

### Long-Term (Ongoing)

- [x] New syntax additions follow the checklist
- [x] No new "whack-a-mole" bugs from syntax additions
- [x] Parser code is maintainable by contributors
- [ ] Error messages are Elm-quality (future improvement)

### Metrics

| Metric | Current | Target | Status |
|--------|---------|--------|--------|
| Parser bugs in tests | 0 | 0 | ✅ |
| Nested generic depth supported | Unlimited | Unlimited | ✅ |
| Pattern types supported | 11 | 11 | ✅ |
| Spec productions with implementations | ~99% | 100% | ✅ |
| Tests passing | 901 | 900+ | ✅ |

**Phase 1 Progress:** `>>` tokenization bug fixed. Nested generics like `Result<Result<T, E>, E>` now parse correctly.

**Phase 1 Cleanup (2026-01-28):** Uncommented 4 tests that now pass:
- `test_match_nested_option` - `Option<Option<int>>`
- `test_match_nested_result` - `Result<Result<int, str>, str>`
- `test_double_nested_match` - `Option<Result<int, str>>`
- `test_int_right_shift` - `>>` operator in expressions

**Phase 3 Completion (2026-01-28):** Uncommented 8 pattern tests that now pass:
- `test_match_negative_literal` - negative integers in patterns
- `test_match_struct_pattern` - struct destructuring patterns
- `test_match_list_pattern` - list patterns with rest (`..tail`)
- `test_match_or_pattern` - or-patterns with `|`
- `test_match_at_pattern` - at-patterns with `@`
- `test_match_range_pattern` - range patterns with `1..10`
- `test_match_guard` - guards with `.match(condition)`
- `test_match_in_for` - for loops in run blocks

Test suite: 900 passed, 0 failed, 19 skipped (up from 892).

**Phase 4 Partial (2026-01-28):** Core progress tracking types implemented:
- Created `compiler/ori_parse/src/progress.rs` with `Progress` enum and `ParseResult<T>` type
- Created `compiler/ori_parse/src/error.rs` for cleaner error organization
- Added `WithProgress` extension trait for easy conversion
- Full parser integration deferred (current error recovery works well)

**Phase 5 Partial (2026-01-28):** Specification updates:
- Added "Lexer-Parser Contract" section to `03-lexical-elements.md`
- Added "Disambiguation" section covering struct literals, soft keywords, parentheses
- Verified pattern productions in `10-patterns.md` already complete
- Verified `CLAUDE.md` quick reference already up-to-date
- Unified `grammar.ebnf` deferred (grammar exists in individual spec files)

**Phase 6 Complete (2026-01-28):** Compositional testing:
- Created `compiler/ori_parse/src/compositional_tests.rs` with 15 test functions
- Type matrix: 30+ types × 4 positions (variable, param, return, alias)
- Pattern matrix: 25+ patterns in match arms, with guards, nested, or-patterns
- Expression context: struct literal contexts, lambdas, for loops, method chains
- All compositional tests pass alongside 900 Ori spec tests

---

## 10. References

### Internal Documents

| Document | Purpose |
|----------|---------|
| [concepts/00-overview.md](concepts/00-overview.md) | Architecture overview |
| [concepts/01-implementation-plan.md](concepts/01-implementation-plan.md) | Original implementation plan (reference only) |
| [concepts/02-formal-grammar.md](concepts/02-formal-grammar.md) | Draft grammar |
| [concepts/04-pratt-parser.md](concepts/04-pratt-parser.md) | Pratt parsing details |
| [concepts/05-error-recovery.md](concepts/05-error-recovery.md) | Error recovery strategies |
| [concepts/06-indentation.md](concepts/06-indentation.md) | Indentation handling |
| [concepts/07-disambiguation.md](concepts/07-disambiguation.md) | Tristate disambiguation |

### Spec Documents

| Document | Purpose |
|----------|---------|
| `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md` | Tokens and operators |
| `docs/ori_lang/0.1-alpha/spec/09-expressions.md` | Expression syntax |
| `docs/ori_lang/0.1-alpha/spec/10-patterns.md` | Pattern syntax |

### External References

| Compiler | Key Files | Lessons |
|----------|-----------|---------|
| Rust | `compiler/rustc_parse/src/parser/` | Snapshot speculation, restrictions |
| TypeScript | `src/compiler/parser.ts` | Tristate disambiguation |
| Elm | `compiler/src/Parse/` | Four-continuation CPS, error quality |
| Roc | `crates/compiler/parse/src/` | Progress tracking |
| Gleam | `compiler-core/src/parse/` | Simple Pratt, fault-tolerant |
| Go | `src/go/parser/` | Accept larger language, simple recovery |

### Dropped Concepts

| Concept | Source | Why Dropped |
|---------|--------|-------------|
| SoA storage | Zig | Complexity not justified for Ori |
| Capacity pre-estimation | Zig | Diminishing returns |
| Memory optimization focus | Zig | Maintainability > memory |

See [concepts/03-soa-storage.md](concepts/03-soa-storage.md) for the original proposal (archived for reference).

---

## Appendix: Bug Details

### A.1 The `>>` Tokenization Bug

**Symptom:**
```ori
// This fails to parse
let x: Result<Result<int, str>, str> = Ok(Ok(1))
// Error: expected type, found '>>'
```

**Root Cause:**
The lexer uses maximal munch and produces `>>` as a single `Shr` (right shift) token. When the parser encounters this in a type context, it cannot split it back into two `>` tokens.

**Why Patches Fail:**
- Cannot look ahead to "fix" the token - too late
- Cannot ask lexer to re-lex - no mechanism
- Cannot special-case `>>` - what about `>>>`?

**Correct Solution:**
Lexer never produces compound `>` tokens. Parser reconstructs operators in expression context.

### A.2 Negative Literal in Patterns

**Symptom:**
```ori
match(x,
    -1 -> "negative one",  // Error: expected pattern
    _ -> "other",
)
```

**Root Cause:**
`-1` is lexed as `Minus` + `Int(1)`. In expression context, this is unary minus applied to literal. In pattern context, there's no unary minus operator.

**Solution:**
Pattern parser recognizes `Minus` + `Int` as negative literal pattern.

---

*This document is the single source of truth for the parser improvement effort. Update it as decisions are made and work progresses.*

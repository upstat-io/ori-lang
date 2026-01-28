# Parser v2: Comprehensive Improvement Plan

**Status:** In Progress
**Created:** 2026-01-28
**Last Updated:** 2026-01-28

---

## Progress Tracking

### Overall Progress

| Phase | Status | Progress |
|-------|--------|----------|
| Phase 1: Lexer Boundary Fix | ⚪ Not Started | 0/17 tasks |
| Phase 2: Context Management | ⚪ Not Started | 0/15 tasks |
| Phase 3: Pattern Expansion | ⚪ Not Started | 0/34 tasks |
| Phase 4: Progress Tracking | ⚪ Not Started | 0/13 tasks |
| Phase 5: Spec Updates | ⚪ Not Started | 0/18 tasks |
| Phase 6: Compositional Testing | ⚪ Not Started | 0/13 tasks |

### Phase 1: Lexer Boundary Fix (Current)

#### 1.1 Lexer Changes
- [ ] Remove `#[token(">>")]` (Shr) from `RawToken` enum
- [ ] Remove `#[token(">=")]` (GtEq) from `RawToken` enum
- [ ] Remove `RawToken::Shr` and `RawToken::GtEq` handling in `convert_token`
- [ ] Add comment explaining why `>` is always a single token
- [ ] Verify lexer compiles

#### 1.2 Cursor/Parser Infrastructure
- [ ] Add `peek_next_span()` method to Cursor
- [ ] Add `are_adjacent(current, next)` helper method
- [ ] Add `try_consume_compound_gt()` method for `>>` and `>=`

#### 1.3 Operator Matching Updates
- [ ] Update `match_comparison_op()` to detect `>` + `=` as `>=`
- [ ] Update `match_shift_op()` to detect `>` + `>` as `>>`
- [ ] Ensure operators consume correct number of tokens

#### 1.4 Type Parser Verification
- [ ] Verify type parser already uses single `>` (should be no changes needed)
- [ ] Add test for `Result<Result<T, E>, E>` parsing

#### 1.5 Testing
- [ ] Add unit test for nested generics in type parser
- [ ] Add test for `>>` shift operator in expressions
- [ ] Add test for `>=` comparison in expressions
- [ ] Run full test suite (`./test-all`)
- [ ] Verify no regressions

### Known Bugs Tracking

| Bug | Status | Phase |
|-----|--------|-------|
| `>>` tokenization | ⚪ Not Started | 1 |
| Negative literals in patterns | ⚪ Not Started | 3 |
| Struct patterns | ⚪ Not Started | 3 |
| List patterns | ⚪ Not Started | 3 |
| Or-patterns | ⚪ Not Started | 3 |
| At-patterns | ⚪ Not Started | 3 |
| Range patterns | ⚪ Not Started | 3 |
| Match guards | ⚪ Not Started | 3 |
| For in run blocks | ⚪ Not Started | 3 |

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
**Status:** 🟡 In Progress

**Tasks:**

#### 1.1 Lexer Changes
- [ ] Remove `#[token(">>")]` (Shr) from `RawToken` enum
- [ ] Remove `#[token(">=")]` (GtEq) from `RawToken` enum
- [ ] Remove `RawToken::Shr` and `RawToken::GtEq` handling in `convert_token`
- [ ] Add comment explaining why `>` is always a single token
- [ ] Verify lexer compiles

#### 1.2 Cursor/Parser Infrastructure
- [ ] Add `peek_next_span()` method to Cursor
- [ ] Add `are_adjacent(current, next)` helper method
- [ ] Add `try_consume_compound_gt()` method for `>>` and `>=`

#### 1.3 Operator Matching Updates
- [ ] Update `match_comparison_op()` to detect `>` + `=` as `>=`
- [ ] Update `match_shift_op()` to detect `>` + `>` as `>>`
- [ ] Ensure operators consume correct number of tokens

#### 1.4 Type Parser Verification
- [ ] Verify type parser already uses single `>` (should be no changes needed)
- [ ] Add test for `Result<Result<T, E>, E>` parsing

#### 1.5 Testing
- [ ] Add unit test for nested generics in type parser
- [ ] Add test for `>>` shift operator in expressions
- [ ] Add test for `>=` comparison in expressions
- [ ] Run full test suite (`./test-all`)
- [ ] Verify no regressions

**Files:**
- `compiler/ori_lexer/src/lib.rs`
- `compiler/ori_parse/src/cursor.rs`
- `compiler/ori_parse/src/grammar/expr/operators.rs`
- `compiler/ori_parse/src/grammar/ty.rs`
- `tests/spec/types/` (new tests)

### Phase 2: Context Management System

**Duration:** 1 day
**Blocks:** Proper struct literal disambiguation
**Status:** ⚪ Not Started

**Tasks:**

#### 2.1 ParseContext Implementation
- [ ] Create `compiler/ori_parse/src/context.rs`
- [ ] Define `ParseContext` bitflags enum
- [ ] Add `IN_PATTERN`, `IN_TYPE`, `NO_STRUCT_LIT`, `CONST_EXPR`, `IN_LOOP`, `ALLOW_YIELD` flags

#### 2.2 Parser Integration
- [ ] Add `context: ParseContext` field to `Parser` struct
- [ ] Implement `with_context()` method
- [ ] Implement `without_context()` method
- [ ] Implement `has_context()` check method

#### 2.3 Usage Sites
- [ ] Update `if` expression to use `NO_STRUCT_LIT` for condition
- [ ] Update `while` expression to use `NO_STRUCT_LIT` for condition
- [ ] Update pattern parsing to set `IN_PATTERN`
- [ ] Update type parsing to set `IN_TYPE`
- [ ] Update loop parsing to set `IN_LOOP`

#### 2.4 Testing
- [ ] Test struct literal not allowed in `if` condition
- [ ] Test struct literal allowed in `if` body
- [ ] Test context properly restored after nested parsing
- [ ] Run full test suite

**Files:**
- `compiler/ori_parse/src/context.rs` (new)
- `compiler/ori_parse/src/lib.rs`
- `compiler/ori_parse/src/grammar/expr/mod.rs`
- `compiler/ori_parse/src/grammar/pattern.rs`

### Phase 3: Pattern Parsing Expansion

**Duration:** 3-5 days
**Blocks:** Many match expression use cases
**Status:** ⚪ Not Started

**Tasks:**

#### 3.1 Negative Literal Pattern
- [ ] Update pattern parser to recognize `Minus` + `Int` as negative literal
- [ ] Add AST representation if needed
- [ ] Add type checker support
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.2 Struct Pattern (`Point { x, y }`)
- [ ] Add `PatternKind::Struct` variant to AST
- [ ] Implement `parse_struct_pattern()` in parser
- [ ] Add type checker support for struct pattern matching
- [ ] Add evaluator support for struct pattern matching
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.3 List Pattern (`[head, ..tail]`)
- [ ] Add `PatternKind::List` variant to AST
- [ ] Implement `parse_list_pattern()` in parser
- [ ] Handle rest pattern (`..` and `..name`)
- [ ] Add type checker support
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.4 Or-Pattern (`A | B`)
- [ ] Add `PatternKind::Or` variant to AST
- [ ] Implement or-pattern parsing with correct precedence
- [ ] Ensure all branches bind same variables
- [ ] Add type checker support
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.5 At-Pattern (`x @ Some(v)`)
- [ ] Add `PatternKind::At` variant to AST
- [ ] Implement at-pattern parsing
- [ ] Add type checker support
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.6 Range Pattern (`1..10`)
- [ ] Add `PatternKind::Range` variant to AST
- [ ] Implement range pattern parsing
- [ ] Support both `..` and `..=` variants
- [ ] Add type checker support
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.7 Guard Pattern (`x.match(predicate)`)
- [ ] Add `PatternKind::Guard` variant to AST
- [ ] Implement guard pattern parsing
- [ ] Add type checker support (predicate must return bool)
- [ ] Add evaluator support
- [ ] Remove `// TODO: PARSER BUG:` from test

#### 3.8 For in Run Blocks
- [ ] Investigate root cause of `for` in `run` blocks failing
- [ ] Fix expression grammar gap
- [ ] Add test coverage
- [ ] Remove `// TODO: PARSER BUG:` from test

**Files:**
- `compiler/ori_ir/src/ast/patterns.rs`
- `compiler/ori_parse/src/grammar/pattern.rs`
- `compiler/ori_typeck/src/patterns/`
- `compiler/ori_eval/src/patterns/`
- `tests/spec/patterns/match.ori`

### Phase 4: Progress Tracking

**Duration:** 1-2 days
**Blocks:** Better error recovery
**Status:** ⚪ Not Started

**Tasks:**

#### 4.1 Core Types
- [ ] Create `compiler/ori_parse/src/progress.rs`
- [ ] Define `Progress` enum (`Made`, `None`)
- [ ] Define `ParseResult<T>` type combining progress and result
- [ ] Add helper methods (`map`, `and_then`, `or_else`)

#### 4.2 Parser Method Updates
- [ ] Update `parse_expr()` to return `ParseResult`
- [ ] Update `parse_item()` to return `ParseResult`
- [ ] Update `parse_type()` to return `ParseResult`
- [ ] Update `parse_pattern()` to return `ParseResult`
- [ ] Update all intermediate parsing methods

#### 4.3 Error Recovery Integration
- [ ] Update main parsing loop to use progress for recovery decisions
- [ ] `Progress::None` + error → try alternative
- [ ] `Progress::Made` + error → commit and synchronize
- [ ] Add synchronization points for items, expressions, statements

#### 4.4 Testing
- [ ] Test error recovery with partial parse
- [ ] Test multiple errors in single file
- [ ] Test recovery doesn't skip valid code
- [ ] Run full test suite

**Files:**
- `compiler/ori_parse/src/progress.rs` (new)
- `compiler/ori_parse/src/lib.rs`
- `compiler/ori_parse/src/grammar/*.rs` (all)
- `compiler/ori_parse/src/recovery.rs`

### Phase 5: Specification Updates

**Duration:** 1 day
**Blocks:** Future implementation clarity
**Status:** ⚪ Not Started

**Tasks:**

#### 5.1 Unified Grammar Document
- [ ] Create `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`
- [ ] Add all lexical productions from `03-lexical-elements.md`
- [ ] Add all syntactic productions from other spec files
- [ ] Ensure productions are machine-parseable
- [ ] Add cross-references to detailed explanations

#### 5.2 Disambiguation Rules
- [ ] Add "Disambiguation" section to `03-lexical-elements.md`
- [ ] Document token splitting rules (e.g., `>` never compounds)
- [ ] Document parenthesized expression interpretation
- [ ] Document struct literal context rules
- [ ] Document soft keyword rules

#### 5.3 Pattern Productions
- [ ] Update `10-patterns.md` with all pattern grammar
- [ ] Add `struct_pattern` production
- [ ] Add `list_pattern` production
- [ ] Add `range_pattern` production
- [ ] Add `or_pattern` production
- [ ] Add `at_pattern` production
- [ ] Add `guard_pattern` production

#### 5.4 Lexer-Parser Contract
- [ ] Add "Lexer-Parser Contract" section to `03-lexical-elements.md`
- [ ] Document maximal munch exceptions
- [ ] Document which tokens are never produced by lexer
- [ ] Add examples of nested generics

#### 5.5 CLAUDE.md Sync
- [ ] Update quick reference with any new syntax
- [ ] Verify examples match spec
- [ ] Add any new pattern syntax

**Files:**
- `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` (new)
- `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`
- `docs/ori_lang/0.1-alpha/spec/10-patterns.md`
- `CLAUDE.md`

### Phase 6: Compositional Testing

**Duration:** 1-2 days
**Blocks:** Long-term confidence
**Status:** ⚪ Not Started

**Tasks:**

#### 6.1 Type Matrix Tests
- [ ] Create `tests/parser/type_matrix.rs`
- [ ] Define list of all type forms (primitives, generics, nested, functions, etc.)
- [ ] Define list of all type positions (annotations, returns, params, etc.)
- [ ] Generate tests for all combinations
- [ ] Add particularly tricky cases (triple-nested generics, function returning generic, etc.)

#### 6.2 Pattern Matrix Tests
- [ ] Create `tests/parser/pattern_matrix.rs`
- [ ] Define list of all pattern forms
- [ ] Define list of all pattern positions (match arms, let bindings, function params)
- [ ] Generate tests for all combinations
- [ ] Add tricky cases (nested patterns, or-patterns in complex contexts)

#### 6.3 Expression Context Tests
- [ ] Create `tests/parser/expr_context.rs`
- [ ] Test expressions in all valid contexts
- [ ] Test context-sensitive parsing (struct literals, etc.)
- [ ] Test operator precedence edge cases

#### 6.4 CI Integration
- [ ] Add compositional tests to CI workflow
- [ ] Ensure tests run on every PR
- [ ] Add test coverage reporting
- [ ] Document how to add new cases

**Files:**
- `tests/parser/type_matrix.rs` (new)
- `tests/parser/pattern_matrix.rs` (new)
- `tests/parser/expr_context.rs` (new)
- `.github/workflows/` (CI updates)

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

### Immediate (Phase 1-2 Complete)

- [ ] `Result<Result<T, E>, E>` parses correctly
- [ ] No regressions in existing tests
- [ ] Context flags implemented and documented

### Short-Term (Phase 3-4 Complete)

- [ ] All 11 pattern bugs fixed (remove all `TODO: PARSER BUG:` comments)
- [ ] Progress-aware error recovery working
- [ ] Error messages include hints for common mistakes

### Medium-Term (Phase 5-6 Complete)

- [ ] Unified grammar document exists and is authoritative
- [ ] Compositional tests cover type and pattern combinations
- [ ] Feature addition process is documented and followed

### Long-Term (Ongoing)

- [ ] New syntax additions follow the checklist
- [ ] No new "whack-a-mole" bugs from syntax additions
- [ ] Parser code is maintainable by contributors
- [ ] Error messages are Elm-quality

### Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Parser bugs in tests | 11 | 0 |
| Nested generic depth supported | 1 | Unlimited |
| Pattern types supported | 4 | 11 |
| Spec productions with implementations | ~70% | 100% |

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

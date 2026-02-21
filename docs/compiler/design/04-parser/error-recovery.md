---
title: "Parser Error Recovery"
description: "Ori Compiler Design — Parser Error Recovery"
order: 401
section: "Parser"
---

# Parser Error Recovery

The Ori parser uses a multi-layered error recovery system that combines Elm-style four-way progress tracking with bitset-based token synchronization. This enables reporting multiple errors per parse, producing partial ASTs for downstream phases, and avoiding cascading false errors.

## Goals

1. **Report multiple errors** — Continue parsing after the first error
2. **Produce partial AST** — Downstream phases (type checking, evaluation) can work with `ExprKind::Error` placeholders
3. **Avoid cascading errors** — Progress tracking prevents one error from triggering many false errors
4. **Preserve spans** — Every error has a source location for precise diagnostics

## ParseOutcome: Four-Way Progress Tracking

The `ParseOutcome` type encodes both success/failure and whether input was consumed, creating four distinct parsing states:

| Progress | Result | Variant | Meaning |
|----------|--------|---------|---------|
| Consumed | Ok | `ConsumedOk` | Committed to parse path, succeeded |
| Empty | Ok | `EmptyOk` | Optional content absent, succeeded |
| Consumed | Err | `ConsumedErr` | Hard error — don't backtrack, report error |
| Empty | Err | `EmptyErr` | Soft error — try next alternative |

```rust
pub enum ParseOutcome<T> {
    ConsumedOk { value: T },
    EmptyOk { value: T },
    ConsumedErr { error: ParseError, consumed_span: Span },
    EmptyErr { expected: TokenSet, position: usize },
}
```

The key insight from Elm/Roc: the **combination of progress and result** determines the correct recovery strategy. If tokens were consumed before the error, the parser has committed to a production and should report the error. If no tokens were consumed, the parser can silently try alternative productions.

### Backtracking Macros

Four macros build on `ParseOutcome` for clean parsing logic:

#### `one_of!` — Try alternatives with automatic backtracking

```rust
fn parse_atom(&mut self) -> ParseOutcome<ExprId> {
    one_of!(self,
        self.parse_literal(),      // Try literal first
        self.parse_ident(),        // Then identifier
        self.parse_paren_expr(),   // Then parenthesized expression
    )
}
```

Each alternative is evaluated in order. On `EmptyErr` (soft failure), the parser restores position and tries the next alternative. On `ConsumedErr` (hard failure), the error propagates immediately — no further alternatives are tried. Expected token sets are accumulated across all soft failures for precise error messages like "expected `(`, `[`, or identifier".

#### `try_outcome!` — Parse optional elements

```rust
fn parse_optional_type_annotation(&mut self) -> ParseOutcome<Option<TypeId>> {
    let ty = try_outcome!(self, self.parse_type_annotation());
    ParseOutcome::consumed_ok(Some(ty))
}
```

Returns `Some(value)` on success, `None` on soft error, and propagates hard errors.

#### `require!` — Mandatory elements after commitment

```rust
fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    self.expect(&TokenKind::If)?;  // Already consumed 'if'
    let cond = require!(self, self.parse_expr(), "condition in if expression");
    // ...
}
```

Upgrades soft errors to hard errors with context. Used after the parser has committed to a production (consumed the leading keyword).

#### `chain!` — Sequence operations

```rust
fn parse_binary(&mut self) -> ParseOutcome<ExprId> {
    let lhs = chain!(self, self.parse_atom());
    let op = chain!(self, self.parse_operator());
    let rhs = chain!(self, self.parse_atom());
    ParseOutcome::consumed_ok(self.make_binary(lhs, op, rhs))
}
```

Extracts the value on success, returns early on any error.

#### `committed!` — Bridge from Result to ParseOutcome after commitment

```rust
fn parse_trait(&mut self) -> ParseOutcome<TraitDef> {
    // ... consumed `trait` keyword, now committed to this production
    let name = committed!(self.expect_ident());  // Result<Name, ParseError> → Name or ConsumedErr
    let generics = committed!(self.parse_generics());
    // ...
}
```

After the parser has consumed tokens that commit it to a production (e.g., the `trait` keyword), subsequent parsing steps use `Result<T, ParseError>` internally. The `committed!` macro bridges these `Result` values into `ParseOutcome`: on `Ok(value)` it unwraps the value; on `Err(error)` it returns `ConsumedErr` with the error's span. This is used extensively throughout the parser for the "post-commitment" portion of productions where backtracking is no longer appropriate.

### Combinators

`ParseOutcome` also provides functional combinators:

| Method | Behavior |
|--------|----------|
| `map(f)` | Transform success value, preserve progress |
| `map_err(f)` | Transform error, preserve progress |
| `and_then(f)` | Chain operations, upgrade progress if either consumed |
| `or_else(f)` | Try alternative on soft error only |
| `or_else_accumulate(f)` | Like `or_else`, but merge expected token sets |
| `with_error_context(ctx)` | Attach "while parsing X" to hard errors |

### Error Context

The `in_error_context` method wraps parser functions to add context:

```rust
fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    self.in_error_context(ErrorContext::IfExpression, |p| {
        // ...
    })
}
```

This produces messages like "expected expression, found `}` (while parsing an if expression)" rather than bare "expected expression".

## TokenSet: Bitset-Based Recovery Points

Token sets use a `u128` bitfield for O(1) membership testing. Each bit corresponds to a `TokenKind` discriminant index, supporting all 122 token kinds.

```rust
pub struct TokenSet(u128);

impl TokenSet {
    pub const fn single(kind: TokenKind) -> Self;
    pub const fn with(self, kind: TokenKind) -> Self;  // Builder pattern
    pub const fn contains(&self, kind: &TokenKind) -> bool;  // O(1) lookup
    pub const fn union(self, other: Self) -> Self;
    pub fn format_expected(&self) -> String;  // "`,`, `)`, or `}`"
}
```

### Pre-Defined Recovery Sets

```rust
/// Top-level statement boundaries.
pub const STMT_BOUNDARY: TokenSet = TokenSet::new()
    .with(TokenKind::At)      // Function/test definition
    .with(TokenKind::Use)     // Import statement
    .with(TokenKind::Type)    // Type declaration
    .with(TokenKind::Trait)   // Trait definition
    .with(TokenKind::Impl)    // Impl block
    .with(TokenKind::Pub)     // Public declaration
    .with(TokenKind::Let)     // Module-level constant
    .with(TokenKind::Extend)  // Extension
    .with(TokenKind::Eof);    // End of file

/// Function-level boundaries.
pub const FUNCTION_BOUNDARY: TokenSet = TokenSet::new()
    .with(TokenKind::At)      // Next function/test
    .with(TokenKind::Eof);    // End of file
```

### Synchronization

When recovery is needed, the `synchronize` function skips tokens until reaching a member of the recovery set:

```rust
pub fn synchronize(cursor: &mut Cursor<'_>, recovery: TokenSet) -> bool {
    while !cursor.is_at_end() {
        if recovery.contains(cursor.current_kind()) {
            return true;
        }
        cursor.advance();
    }
    false
}
```

### Error Message Formatting

`TokenSet::format_expected()` produces English-formatted lists:

| Set Contents | Output |
|-------------|--------|
| Empty | `"nothing"` |
| `{(}` | `` "`(`" `` |
| `{(, [}` | `` "`(` or `[`" `` |
| `{,, ), }}` | `` "`,`, `)`, or `}`" `` |

This is used in `EmptyErr` to generate "expected X" messages automatically from the accumulated token set.

## Module-Level Recovery

The `parse_module()` function uses `handle_outcome` for uniform error handling across all declaration types:

```rust
fn handle_outcome<T>(
    &mut self,
    outcome: ParseOutcome<T>,
    collection: &mut Vec<T>,
    errors: &mut Vec<ParseError>,
    recover: impl FnOnce(&mut Self),
) {
    match outcome {
        ConsumedOk { value } | EmptyOk { value } => collection.push(value),
        ConsumedErr { error, .. } => {
            recover(self);  // Skip to recovery point
            errors.push(error);
        }
        EmptyErr { expected, position } => {
            errors.push(ParseError::from_expected_tokens(&expected, position));
        }
    }
}
```

Recovery functions:

| Function | Recovery Point | Used For |
|----------|----------------|----------|
| `recover_to_next_statement()` | `STMT_BOUNDARY` | Import parsing errors |
| `recover_to_function()` | `FUNCTION_BOUNDARY` | Function/type/trait parsing errors |

## Error Types

```rust
pub struct ParseError {
    pub code: ErrorCode,       // E1xxx range
    pub message: String,       // Human-readable description
    pub span: Span,            // Source location
    pub context: Option<String>, // "while parsing X"
    pub help: Vec<String>,     // Suggestion messages
}
```

Error codes:

| Code | Meaning |
|------|---------|
| `E1001` | Unexpected token |
| `E1002` | Expected expression / trailing operator / expected declaration |
| `E1003` | Unclosed delimiter |
| `E1004` | Expected identifier |
| `E1005` | Expected type |
| `E1006` | Orphaned attributes / invalid attribute |
| `E1008` | Invalid pattern |
| `E1009` | Pattern argument error (wrong count, wrong type) |
| `E1015` | Unsupported keyword (e.g., `return` in expression position) |

### ParseErrorKind Variants

The `ParseErrorKind` enum covers structured error categories beyond simple "unexpected token":

| Variant | Description |
|---------|-------------|
| `ExpectedExpression` | Expression expected but not found |
| `TrailingOperator` | Binary operator without a right-hand operand (e.g., `x +`) |
| `ExpectedDeclaration` | Declaration expected at module level |
| `UnclosedDelimiter` | Missing closing bracket, paren, or brace |
| `ExpectedIdentifier` | Identifier expected (e.g., after `let`) |
| `ExpectedType` | Type annotation expected |
| `InvalidPattern` | Malformed pattern in match arm or binding |
| `PatternArgumentError` | Wrong argument count or type in compiler patterns (e.g., `cache`, `recurse`) |
| `InvalidFunctionClause` | Malformed function clause (pre/post check) |
| `InvalidAttribute` | Unknown or malformed attribute |
| `UnsupportedKeyword` | Foreign or reserved keyword (e.g., `return`, `fn`) with guidance toward Ori equivalent |

Each variant carries context-specific fields (the offending token, expected types, pattern names) and maps to an error code via `error_code()`. The `empathetic_hint()` method on each variant provides targeted guidance -- for example, `TrailingOperator { operator: Plus }` produces "The `+` operator needs a value on both sides, like `a + b`."

## Placeholder Nodes

When expression parsing fails, the parser allocates `ExprKind::Error` placeholder nodes. These flow through downstream phases (type checking, evaluation) without crashing, enabling partial compilation and multi-error reporting.

## Speculative Parsing

For disambiguation that goes beyond simple lookahead, the parser uses lightweight snapshots:

```rust
pub struct ParserSnapshot {
    pub cursor_pos: usize,
    pub context: ParseContext,
}
```

Snapshots capture cursor position and context flags (~10 bytes). Arena state is intentionally not captured — speculative parsing examines tokens without allocating AST nodes.

| Method | Behavior | Use Case |
|--------|----------|----------|
| `snapshot()` / `restore()` | Manual control | Complex disambiguation |
| `try_parse(f)` | Auto-restore on failure | Full parse attempts |
| `look_ahead(f)` | Always restores | Multi-token predicates |

The `one_of!` macro uses snapshots internally — each alternative gets a fresh snapshot and automatic restore on soft failure.

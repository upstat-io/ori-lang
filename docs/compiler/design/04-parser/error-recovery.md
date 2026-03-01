---
title: "Parser Error Recovery"
description: "Ori Compiler Design — Parser Error Recovery"
order: 401
section: "Parser"
---

# Parser Error Recovery

## What Is Error Recovery?

A parser that stops at the first syntax error is nearly useless in practice. A programmer with a missing closing brace shouldn't have to fix that one error, recompile, discover the next error, fix it, recompile — cycling through dozens of edit-compile loops just to see all the syntax problems in a file. **Error recovery** is the set of techniques that allow a parser to continue past errors, producing a partial result and reporting as many genuine errors as possible in a single pass.

This is surprisingly hard to do well. The challenge is not continuing — any parser can skip tokens until something looks right — but continuing **accurately**. A single misplaced token can make the entire rest of the file look like garbage if the parser loses track of the grammatical context. The parser's job during recovery is to find a point where it can re-synchronize with the token stream and resume parsing with confidence that subsequent errors are real, not cascading effects of the first one.

### Why Multi-Error Reporting Matters

Modern development workflows compile constantly — on save, on keystroke, on every character typed. An IDE that shows only one error at a time forces the programmer into a serial debugging loop. A parser that reports five genuine errors in one pass lets the programmer fix all five before the next compile, or at least see the full picture of what's wrong.

But multi-error reporting has a critical quality constraint: **false errors are worse than missing errors.** If a parser reports 30 errors but 25 of them are cascading artifacts of the first real error, the programmer learns to ignore the error list entirely. One real error reported accurately is more valuable than thirty errors where the signal is buried in noise.

### Classical Approaches

#### Panic Mode Recovery

The oldest and simplest approach: when the parser encounters an error, it discards tokens until it finds a **synchronization point** — a token that is likely to start a valid construct. Typical synchronization points include statement terminators (`;`), block delimiters (`{`, `}`), and declaration keywords (`fn`, `class`, `let`).

Panic mode is reliable but coarse. It gives up on the current construct entirely, potentially skipping valid code between the error and the sync point. For a missing comma in a function call `f(a b)`, panic mode might skip to the next `)` or `;`, discarding everything in between.

Most production compilers use panic mode as the foundation, augmented with finer-grained strategies for specific constructs.

#### Phrase-Level Recovery

The parser substitutes or inserts tokens to repair the error locally. If it expects `)` but sees `]`, it can report "expected `)`, found `]`" and treat the `]` as if it were `)`. If it expects `;` at the end of a statement, it can insert a virtual `;` and continue.

Phrase-level recovery preserves more of the parse tree than panic mode, but risks making incorrect repairs that lead to cascading false errors. The parser must be conservative about which repairs it attempts.

#### Error Productions

The grammar is augmented with productions that explicitly describe common mistakes. For example, a grammar might include a production that matches `if condition body` (missing `then` keyword) and produces a diagnostic. This approach produces excellent error messages for anticipated mistakes but cannot handle unanticipated errors.

#### Burke-Fisher Error Repair

A more sophisticated approach that tries all possible single-token insertions, deletions, and substitutions at the error point, choosing the repair that allows parsing to continue furthest. [Burke and Fisher (1987)](https://dl.acm.org/doi/10.1145/36177.36188) showed this can be done efficiently for LR parsers. The approach produces optimal repairs for single-token errors but is expensive for multi-token errors and difficult to implement for hand-written parsers.

#### Progress Tracking (Parsec/Elm)

The key innovation from the parser combinator tradition. [Parsec](https://hackage.haskell.org/package/parsec) (2001) introduced the idea that a parser's error behavior should depend on whether it consumed input before failing. If the parser consumed tokens and then failed, it has **committed** to a parse path — the error is real and should be reported. If it consumed nothing and failed, it was merely testing a possibility — the error should be suppressed and the next alternative tried.

This distinction between "committed" and "uncommitted" errors dramatically reduces cascading false errors without requiring explicit synchronization points. [Elm](https://github.com/elm/parser) refined this into a clean four-way model that separates progress (consumed vs empty) from result (ok vs err), producing four distinct states with different recovery behaviors.

## How Ori Recovers from Errors

Ori combines progress tracking (from Elm) with panic-mode synchronization (from the classical tradition). Progress tracking prevents cascading errors at the expression level; synchronization handles recovery at the declaration level. Together, they enable accurate multi-error reporting across entire files.

### ParseOutcome: Four-Way Progress Tracking

The `ParseOutcome` type encodes both success/failure and whether input was consumed, creating four distinct parsing states:

```rust
pub enum ParseOutcome<T> {
    ConsumedOk { value: T },
    EmptyOk { value: T },
    ConsumedErr { error: ParseError, consumed_span: Span },
    EmptyErr { expected: TokenSet, position: usize },
}
```

| Progress | Result | Variant | Meaning |
|----------|--------|---------|---------|
| Consumed | Ok | `ConsumedOk` | Committed to parse path, succeeded |
| Empty | Ok | `EmptyOk` | Optional content absent — succeeded vacuously |
| Consumed | Err | `ConsumedErr` | **Hard error** — committed, then failed. Report immediately, don't backtrack |
| Empty | Err | `EmptyErr` | **Soft error** — nothing consumed. Try next alternative silently |

The distinction between `ConsumedErr` and `EmptyErr` is the core mechanism. Consider parsing an `if` expression: after consuming the `if` keyword, the parser is committed to the if-expression production. If the condition is malformed, that's a `ConsumedErr` — a real error that should be reported. But if the parser is trying to determine whether the next construct is an if-expression and the first token isn't `if`, that's an `EmptyErr` — no tokens were consumed, and the parser should silently try the next alternative (maybe it's a match expression, or a for loop, or a literal).

### Backtracking Macros

Four macros build on `ParseOutcome` for clean parsing logic. These macros are the primary interface — most parsing functions interact with `ParseOutcome` through macros rather than matching on variants directly.

#### `one_of!` — Try alternatives with automatic backtracking

```rust
fn parse_atom(&mut self) -> ParseOutcome<ExprId> {
    one_of!(self,
        self.parse_literal(),
        self.parse_ident(),
        self.parse_paren_expr(),
    )
}
```

Each alternative is evaluated in order. On `EmptyErr` (soft failure), the parser restores its position and tries the next alternative. On `ConsumedErr` (hard failure), the error propagates immediately — no further alternatives are tried, because the parser committed to that path and the error is real.

The macro accumulates expected token sets across all soft failures. If all alternatives fail softly, the combined set produces precise error messages like "expected `(`, `[`, or identifier" — listing everything the parser was willing to accept at that position.

#### `try_outcome!` — Parse optional elements

```rust
let ty = try_outcome!(self, self.parse_type_annotation());
```

Returns `Some(value)` on success, `None` on `EmptyErr` (the element is simply absent), and propagates `ConsumedErr` (a real error inside the element).

#### `require!` — Mandatory elements after commitment

```rust
fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    self.expect(&TokenKind::If)?;  // Consumed 'if' — now committed
    let cond = require!(self, self.parse_expr(), "condition in if expression");
    // ...
}
```

Upgrades `EmptyErr` to `ConsumedErr` with context. Used after the parser has committed to a production — at this point, the missing element is a real error, not a "wrong alternative."

#### `committed!` — Bridge from Result to ParseOutcome

```rust
fn parse_trait(&mut self) -> ParseOutcome<TraitDef> {
    // ... consumed `trait` keyword, now committed
    let name = committed!(self.expect_ident());
    let generics = committed!(self.parse_generics());
    // ...
}
```

After commitment, subsequent parsing steps often use `Result<T, ParseError>` internally (the standard Rust error type). The `committed!` macro bridges these into `ParseOutcome`: `Ok(value)` unwraps to the value; `Err(error)` returns `ConsumedErr`. This is used extensively throughout the parser for the "post-commitment" portion of productions.

### Functional Combinators

`ParseOutcome` also provides functional combinators for composing parsers:

| Method | Behavior |
|--------|----------|
| `map(f)` | Transform success value, preserve progress |
| `map_err(f)` | Transform error, preserve progress |
| `and_then(f)` | Chain operations, upgrade progress if either consumed |
| `or_else(f)` | Try alternative on soft error only |
| `or_else_accumulate(f)` | Like `or_else`, merge expected token sets across failures |
| `with_error_context(ctx)` | Attach "while parsing X" to hard errors |

### Error Context

The `in_error_context` method wraps parser functions to add context to hard errors:

```rust
fn parse_if_expr(&mut self) -> ParseOutcome<ExprId> {
    self.in_error_context(ErrorContext::IfExpression, |p| {
        // ...
    })
}
```

This transforms bare messages like "expected expression, found `}`" into contextual ones like "expected expression, found `}` (while parsing an if expression)". The context stack tells the user not just what was expected, but where in the grammar the parser was when the error occurred.

## TokenSet: Bitset-Based Recovery Points

### The Synchronization Problem

When the parser encounters a hard error, it must skip tokens until it finds a safe point to resume. The question is: which tokens are safe? The answer depends on what the parser was trying to parse. After a failed function declaration, skipping to the next `@` is reasonable. After a failed import, skipping to the next `use` or `@` is better.

Ori represents these recovery points as **token sets** — bitfields where each bit corresponds to a `TokenKind` discriminant. Membership testing is O(1) via bit operations, and sets can be combined with bitwise OR.

```rust
pub struct TokenSet(u128);

impl TokenSet {
    pub const fn single(kind: TokenKind) -> Self;
    pub const fn with(self, kind: TokenKind) -> Self;  // Builder pattern
    pub const fn contains(&self, kind: &TokenKind) -> bool;  // O(1) bit test
    pub const fn union(self, other: Self) -> Self;           // Bitwise OR
    pub fn format_expected(&self) -> String;  // "`,`, `)`, or `}`"
}
```

### Pre-Defined Recovery Sets

```rust
/// Top-level statement boundaries — tokens that can start a new declaration.
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

/// Function-level boundaries — tokens that can start a new function.
pub const FUNCTION_BOUNDARY: TokenSet = TokenSet::new()
    .with(TokenKind::At)      // Next function/test
    .with(TokenKind::Eof);    // End of file
```

### Synchronization

When recovery is needed, the `synchronize` function advances the cursor until it reaches a token in the recovery set:

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

This is classic panic-mode recovery, but scoped to specific recovery sets rather than a single global set. Different grammar levels use different sets: `STMT_BOUNDARY` for top-level errors, `FUNCTION_BOUNDARY` for errors inside declarations.

### Expected Token Formatting

`TokenSet::format_expected()` produces English-formatted lists for error messages:

| Set Contents | Output |
|-------------|--------|
| Empty | `"nothing"` |
| `{(}` | `` "`(`" `` |
| `{(, [}` | `` "`(` or `[`" `` |
| `{,, ), }}` | `` "`,`, `)`, or `}`" `` |

The `one_of!` macro accumulates expected tokens from all `EmptyErr` branches, so when all alternatives fail, the error message lists everything the parser was willing to accept at that position.

## Module-Level Recovery

At the top level, `parse_module()` uses `handle_outcome` for uniform error handling across all declaration types:

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
            recover(self);  // Synchronize to recovery point
            errors.push(error);
        }
        EmptyErr { expected, position } => {
            errors.push(ParseError::from_expected_tokens(&expected, position));
        }
    }
}
```

The recovery function varies by declaration type:

| Function | Recovery Point | Used For |
|----------|----------------|----------|
| `recover_to_next_statement()` | `STMT_BOUNDARY` | Import parsing errors |
| `recover_to_function()` | `FUNCTION_BOUNDARY` | Function/type/trait parsing errors |

This means that a syntax error inside a function body will skip to the next `@` token (the start of the next function), preserving all subsequent declarations. The user sees the error in the malformed function and correct parse results for everything else.

## Error Types

### ParseError

```rust
pub struct ParseError {
    pub code: ErrorCode,       // E1xxx range
    pub message: String,       // Human-readable description
    pub span: Span,            // Source location
    pub context: Option<String>, // "while parsing X"
    pub help: Vec<String>,     // Suggestion messages
}
```

### Error Codes

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

### ParseErrorKind: Structured Errors

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
| `PatternArgumentError` | Wrong argument count or type in compiler patterns |
| `InvalidFunctionClause` | Malformed function clause (pre/post check) |
| `InvalidAttribute` | Unknown or malformed attribute |
| `UnsupportedKeyword` | Foreign keyword with guidance toward Ori equivalent |

Each variant carries context-specific fields (the offending token, expected types, pattern names) and maps to an error code via `error_code()`. The `empathetic_hint()` method provides targeted guidance — for example, `TrailingOperator { operator: Plus }` produces "The `+` operator needs a value on both sides, like `a + b`."

## Placeholder Nodes

When expression parsing fails, the parser allocates `ExprKind::Error` placeholder nodes in the arena. These nodes carry the error's span and flow through downstream phases — type checking infers them as an error type, the evaluator skips them — enabling partial compilation without crashing. A file with three syntax errors produces an AST with three `Error` nodes and valid trees for everything else; the type checker can still check the valid portions.

## Speculative Parsing

For disambiguation that goes beyond single-token lookahead, the parser uses lightweight snapshots:

```rust
pub struct ParserSnapshot {
    pub cursor_pos: usize,
    pub context: ParseContext,
}
```

Snapshots capture cursor position and context flags (~10 bytes). Arena state is intentionally not captured — speculative parsing examines tokens without allocating AST nodes.

| Method | Behavior | Use Case |
|--------|----------|----------|
| `snapshot()` / `restore()` | Manual save/restore | Complex disambiguation |
| `try_parse(f)` | Auto-restore on failure | Full parse attempts |
| `look_ahead(f)` | Always restores | Multi-token predicates |

The `one_of!` macro uses snapshots internally — each alternative gets a fresh snapshot and automatic restore on `EmptyErr`. This means speculative parsing is pervasive but invisible: the programmer writes `one_of!(self, alt1, alt2, alt3)` and the macro handles save, try, restore, accumulate.

## Design Tradeoffs

**Progress tracking vs explicit recovery** — Ori uses both. Progress tracking (from Elm) handles the common case: when the parser is trying alternatives and one doesn't match, it backtracks silently. Explicit synchronization (panic mode) handles the hard case: when the parser has committed to a production and the error is deep inside it. The combination gives excellent error quality for the common case (no cascading) while still recovering from deep errors.

**Soft errors are silent** — `EmptyErr` never reaches the user. It is purely internal bookkeeping for the `one_of!` macro. Only `ConsumedErr` produces user-visible errors. This means the parser can speculatively try many alternatives without polluting the error output.

**Conservative phrase-level recovery** — Ori does not attempt to insert or substitute tokens to repair errors. It reports what it found, synchronizes to a safe point, and continues. This is deliberately conservative — incorrect repairs cause cascading false errors that are worse than the original error. The tradeoff is that some constructs are entirely skipped when they could theoretically be partially recovered, but the errors that are reported are reliable.

## Prior Art

| System | Recovery Strategy | Key Insight |
|--------|-------------------|-------------|
| [Elm](https://github.com/elm/parser) | Four-way `Step` type (consumed/empty × ok/err). Parser combinator library designed for excellent error messages. | Direct ancestor of Ori's `ParseOutcome`. Demonstrated that progress tracking eliminates most cascading errors. |
| [Parsec](https://hackage.haskell.org/package/parsec) | Consumed/empty distinction for backtracking decisions. The `try` combinator forces backtracking even after consumption. | Introduced progress-aware error recovery to the parser combinator tradition (2001). |
| [Roc](https://github.com/roc-lang/roc) | Hand-written parser with Elm-style progress tracking and rich error suggestions. | Shows that Elm's combinator ideas work in hand-written parsers. |
| [Rust](https://github.com/rust-lang/rust) | Panic-mode synchronization with sophisticated expected-token tracking. Recovery tokens per grammar construct. | Demonstrates that panic mode with per-construct recovery sets scales to complex grammars. |
| [GCC](https://gcc.gnu.org/) | Error recovery with token insertion/deletion. `cp_parser_error` attempts local repairs. | Phrase-level recovery in a production compiler — effective for C/C++ but complex to maintain. |
| [Roslyn](https://github.com/dotnet/roslyn) (C#) | Every token is preserved in the tree, including error tokens. Incremental parsing never discards structure. | Error recovery is lossless — no tokens are skipped, they're placed in "skipped tokens trivia" nodes. |
| [tree-sitter](https://tree-sitter.github.io/tree-sitter/) | Error recovery via grammar-aware cost model. Inserts ERROR nodes in the CST. | Automatic recovery for generated parsers — no hand-written recovery code needed. |
| [Clang](https://github.com/llvm/llvm-project) | Balanced delimiter tracking, "fixit" hints, template argument recovery. | Context-specific recovery strategies (each C++ construct has its own) at significant implementation cost. |

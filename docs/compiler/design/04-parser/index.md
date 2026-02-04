---
title: "Parser Overview"
description: "Ori Compiler Design — Parser Overview"
order: 400
section: "Parser"
---

# Parser Overview

The Ori parser transforms a token stream into an AST. It uses recursive descent parsing with operator precedence handling.

## Location

```
compiler/ori_parse/src/
├── lib.rs                  # Parser struct and public API
├── cursor.rs               # Token cursor abstraction
├── context.rs              # ParseContext for context-sensitive parsing
├── progress.rs             # Progress tracking (inspired by Roc)
├── recovery.rs             # RecoverySet and synchronization
├── series.rs               # Series combinator for comma-separated lists
├── snapshot.rs             # Parser snapshots for speculative parsing
├── error.rs                # Parse error types
└── grammar/
    ├── mod.rs              # Grammar module organization
    ├── expr/               # Expression parsing
    │   ├── mod.rs              # Entry point, parse_binary_level! macro
    │   ├── operators.rs        # Operator matching helpers
    │   ├── patterns.rs         # function_seq/function_exp parsing
    │   ├── postfix.rs          # Call, method call, field, index
    │   └── primary.rs          # Primary expressions, literals
    ├── item/               # Item parsing
    │   ├── mod.rs              # Re-exports
    │   ├── function.rs         # Function/test parsing
    │   ├── type_decl.rs        # Type declarations
    │   ├── trait_def.rs        # Trait definitions
    │   ├── impl_def.rs         # Impl blocks
    │   ├── use_def.rs          # Import statements
    │   ├── extend.rs           # Extension definitions
    │   ├── generics.rs         # Generic parameter parsing
    │   └── config.rs           # Config variable parsing
    ├── ty.rs               # Type annotation parsing
    └── attr.rs             # Attribute parsing
```

The parser is a separate crate with dependencies:
- `ori_ir` - for `Token`, `TokenKind`, `Span`, `ExprArena`, etc.
- `ori_diagnostic` - for `Diagnostic`, `ErrorCode`
- `stacker` - for stack overflow protection on deeply nested expressions

## Design Goals

1. **Clear grammar structure** - One file per grammar category
2. **Error recovery** - Parse as much as possible despite errors
3. **Arena allocation** - Build flat AST in ExprArena
4. **Comprehensive spans** - Track source locations for diagnostics

## Parser Structure

The parser uses a layered architecture:

```rust
pub struct Parser<'a> {
    /// Token navigation via Cursor abstraction
    cursor: Cursor<'a>,
    /// Flat AST storage
    arena: ExprArena,
    /// Context flags for context-sensitive parsing
    context: ParseContext,
}

/// Token cursor for navigating the token stream
pub struct Cursor<'a> {
    tokens: &'a TokenList,
    interner: &'a StringInterner,
    pos: usize,
}

/// Context flags for context-sensitive parsing (u16 for expansion room)
pub struct ParseContext(u16);

impl ParseContext {
    const IN_PATTERN: Self = Self(0b0000_0001);    // Inside pattern context
    const IN_TYPE: Self = Self(0b0000_0010);       // Inside type annotation
    const NO_STRUCT_LIT: Self = Self(0b0000_0100); // Disallow struct literals
    const CONST_EXPR: Self = Self(0b0000_1000);    // Constant expression context
    const IN_LOOP: Self = Self(0b0001_0000);       // Inside loop body
    const ALLOW_YIELD: Self = Self(0b0010_0000);   // Yield allowed
    const IN_FUNCTION: Self = Self(0b0100_0000);   // Inside function body
    const IN_INDEX: Self = Self(0b1000_0000);      // Inside index brackets
}
```

## Progress-Aware Parsing

Inspired by the Roc compiler, the parser tracks whether tokens were consumed:

```rust
pub enum Progress {
    Made,  // Parser consumed tokens
    None,  // No tokens consumed
}

pub struct ParseResult<T> {
    pub progress: Progress,
    pub result: Result<T, ParseError>,
}
```

This enables better error recovery:
- `Progress::None` + error → can try alternative productions
- `Progress::Made` + error → commit to this path and report error

### Progress Tracking Implementation

```rust
impl Parser<'_> {
    /// Parse with progress tracking.
    fn parse_with_progress<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> ParseResult<T> {
        let start_pos = self.cursor.position();
        let result = f(self);
        let progress = if self.cursor.position() > start_pos {
            Progress::Made
        } else {
            Progress::None
        };
        ParseResult { progress, result }
    }
}
```

This pattern is used throughout the parser to determine whether to commit to a parse path or try alternatives

## Parsing Flow

```
TokenList
    │
    │ parse_module()
    ▼
Module { functions, types, tests, imports }
    │
    │ Each item calls:
    │   - parse_function()
    │   - parse_type_def()
    │   - parse_test()
    │   - parse_import()
    ▼
ExprArena (populated during parsing)
```

## Core Methods

### Token Access (via Cursor)

Token navigation is delegated to the `Cursor` type:

```rust
impl Cursor<'_> {
    fn current(&self) -> &Token { ... }
    fn current_kind(&self) -> &TokenKind { &self.current().kind }
    fn current_span(&self) -> Span { self.current().span }
    fn peek_next_kind(&self) -> &TokenKind { ... }  // Lookahead

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.current_kind()) == std::mem::discriminant(kind)
    }

    fn advance(&mut self) -> &Token { ... }
    fn consume(&mut self, kind: &TokenKind) -> bool { ... }

    fn position(&self) -> usize { self.pos }  // For progress tracking
}
```

### Context Management

```rust
impl Parser<'_> {
    fn with_context<T>(&mut self, add: ParseContext, f: impl FnOnce(&mut Self) -> T) -> T {
        let old = self.context;
        self.context = self.context.with(add);
        let result = f(self);
        self.context = old;
        result
    }

    fn allows_struct_lit(&self) -> bool {
        self.context.allows_struct_lit()
    }
}
```

### Expression Parsing

Binary operator precedence levels are generated via the `parse_binary_level!` macro, which creates a precedence chain of parsing functions. Each level calls the next-higher precedence as its "next" parser:

```rust
/// Generate a binary operator parsing function.
/// Two forms:
///   parse_binary_level! { fn_name, next_fn, matcher_fn }     — for multi-op levels
///   parse_binary_level! { fn_name, next_fn, token: T, op: O } — for single-token levels
macro_rules! parse_binary_level { ... }

// 10 precedence levels generated:
parse_binary_level! { parse_logical_or, parse_logical_and, match_logical_or_op }
parse_binary_level! { parse_logical_and, parse_bitwise_or, match_logical_and_op }
// ... through to:
parse_binary_level! { parse_multiply, parse_coalesce, match_multiplicative_op }
```

This replaces 10 hand-written functions that differed only in which operator they matched and which "next" function they called.

### Arena Allocation

```rust
impl Parser<'_> {
    fn alloc(&mut self, kind: ExprKind) -> ExprId {
        self.arena.alloc(Expr {
            kind,
            span: self.current_span(),
        })
    }
}
```

## Expression Precedence

| Prec | Operators | Associativity |
|------|-----------|---------------|
| 1 | `\|\|` | Left |
| 2 | `&&` | Left |
| 3 | `\|` | Left |
| 4 | `^` | Left |
| 5 | `&` | Left |
| 6 | `==` `!=` | Left |
| 7 | `<` `>` `<=` `>=` | Left |
| 8 | `..` `..=` | Left |
| 9 | `<<` `>>` | Left |
| 10 | `+` `-` | Left |
| 11 | `*` `/` `%` | Left |
| 12 | Unary `-` `!` `~` | Right |
| 13 | `.` `[]` `()` `?` | Left |

**Note:** `>>` and `>=` are synthesized from adjacent `>` tokens. See [Token Design](../03-lexer/token-design.md#lexer-parser-token-boundary).

## Error Recovery

Error recovery uses the `TokenSet` type (bitset-based, O(1) membership):

```rust
/// Bitset for efficient token membership testing.
/// Uses u128 to support up to 128 token kinds with O(1) lookup.
pub struct TokenSet(u128);

impl TokenSet {
    /// Create a set containing a single token kind.
    pub const fn single(kind: TokenKind) -> Self {
        Self(1u128 << kind.discriminant_index())
    }

    /// Add a token kind to the set.
    pub const fn with(self, kind: TokenKind) -> Self {
        Self(self.0 | (1u128 << kind.discriminant_index()))
    }

    /// Check if a token kind is in the set (O(1) via bitwise AND).
    pub const fn contains(&self, kind: &TokenKind) -> bool {
        (self.0 & (1u128 << kind.discriminant_index())) != 0
    }

    /// Union of two token sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
```

The progress tracking enables smarter recovery decisions:
- If no tokens consumed, try alternative parsing strategies
- If tokens consumed, report error and synchronize

## Salsa Integration

Parsing is a Salsa query:

```rust
#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let tokens = tokens(db, file);
    parse(db, tokens)
}
```

## Grammar

The authoritative grammar is defined in EBNF notation. Each production maps to parsing functions in `compiler/ori_parse/src/grammar/`.

```ebnf
{{#include ../../../ori_lang/0.1-alpha/spec/grammar.ebnf}}
```

## Series Combinator

The parser uses a reusable series combinator (inspired by Gleam's `series_of()`) for parsing delimiter-separated lists. This unifies the common pattern found throughout the parser:

```rust
/// Configuration for parsing a series of items.
pub struct SeriesConfig {
    pub separator: TokenKind,      // Usually Comma
    pub terminator: TokenKind,     // e.g., RParen, RBracket
    pub trailing: TrailingSeparator,
    pub skip_newlines: bool,
    pub min_count: usize,
    pub max_count: Option<usize>,
}

/// Policy for trailing separators.
pub enum TrailingSeparator {
    Allowed,   // Trailing separator accepted but not required
    Forbidden, // Error if separator appears before terminator
    Required,  // Separator required between items, not after last
}
```

**Convenience methods** handle the most common patterns:

| Method | Delimiters | Usage |
|--------|------------|-------|
| `paren_series()` | `(` `)` | Function arguments, tuples |
| `bracket_series()` | `[` `]` | List literals, indexing |
| `brace_series()` | `{` `}` | Struct literals, blocks |
| `angle_series()` | `<` `>` | Generic parameters |

**Example usage:**

```rust
// Parse function arguments: (arg1, arg2, ...)
let args = self.paren_series(|p| {
    if p.check(&TokenKind::RParen) {
        Ok(None)  // No more items
    } else {
        Ok(Some(p.parse_expr()?))
    }
})?;
```

## Speculative Parsing

The parser includes infrastructure for speculative parsing via snapshots. Snapshots are lightweight (~10 bytes) and capture only cursor position and context flags—arena state is not captured.

**Use the right tool for the job:**

| Approach | When to Use |
|----------|-------------|
| Direct lookahead | 1-2 token peek, token kind decisions |
| `look_ahead(predicate)` | Multi-token patterns, complex predicates |
| `try_parse(parser_fn)` | Full parse attempt with automatic restore |
| `snapshot()`/`restore()` | Manual control, examine result before deciding |

```rust
// Simple lookahead
fn is_typed_lambda_params(&self) -> bool {
    self.check_ident() && self.next_is_colon()
}

// Full speculative parse
if let Some(ty) = self.try_parse(|p| p.parse_type()) {
    // Type parsed successfully
} else {
    // Fall back to expression parsing
}
```

**Current usage:** The Ori parser primarily uses simple lookahead predicates for disambiguation, as they're sufficient and efficient. The snapshot infrastructure is available for IDE tooling, language extensions, and better error messages.

## Related Documents

- [Recursive Descent](recursive-descent.md) - Parsing approach
- [Error Recovery](error-recovery.md) - Handling syntax errors
- [Grammar Modules](grammar-modules.md) - Module organization
- [Formal EBNF Grammar](#grammar) - Complete grammar definition

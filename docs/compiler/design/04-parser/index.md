---
title: "Parser Overview"
description: "Ori Compiler Design — Parser Overview"
order: 400
section: "Parser"
---

# Parser Overview

The Ori parser transforms a token stream into a flat, arena-allocated AST. It uses recursive descent with a Pratt parser for binary operator precedence and Elm-style four-way progress tracking for automatic backtracking.

## Location

```
compiler/ori_parse/src/
├── lib.rs                  # Parser struct, public API, parse_module()
├── cursor.rs               # Token cursor abstraction
├── context.rs              # ParseContext bitfield for context-sensitive parsing
├── outcome.rs              # ParseOutcome (4-way result) + backtracking macros
├── recovery.rs             # TokenSet bitset and synchronization
├── series.rs               # Series combinator for comma-separated lists
├── snapshot.rs             # Parser snapshots for speculative parsing
├── error.rs                # ParseError, ErrorContext, ParseWarning
├── incremental.rs          # Incremental parsing for IDE reuse
└── grammar/
    ├── mod.rs              # Grammar module organization
    ├── expr/               # Expression parsing
    │   ├── mod.rs              # Entry point, Pratt parser for binary operators
    │   ├── operators.rs        # Binding power table, operator matching
    │   ├── patterns.rs         # function_seq/function_exp parsing
    │   ├── postfix.rs          # Call, method call, field, index, await, try
    │   └── primary.rs          # Literals, identifiers, lambdas, let bindings
    ├── item/               # Top-level declarations
    │   ├── mod.rs              # Re-exports
    │   ├── function.rs         # Function/test parsing
    │   ├── type_decl.rs        # Struct, sum, newtype declarations
    │   ├── trait_def.rs        # Trait definitions
    │   ├── impl_def.rs         # Impl blocks and def impl
    │   ├── use_def.rs          # Import statements
    │   ├── extend.rs           # Extension blocks
    │   ├── generics.rs         # Generic parameters, bounds, where clauses
    │   └── config.rs           # Config variable ($NAME = value)
    ├── ty.rs               # Type annotation parsing
    └── attr.rs             # Attribute parsing (#derive, #test, #skip)
```

Dependencies:

- `ori_ir` — `Token`, `TokenKind`, `Span`, `ExprArena`, `ExprId`, `Module`
- `ori_diagnostic` — `Diagnostic`, `ErrorCode`
- `ori_stack` — Stack overflow protection for deeply nested expressions

## Design Goals

1. **Pratt-based operator parsing** — Single-loop binding power table replaces 12-level recursive descent chain
2. **Elm-style progress tracking** — Four-way `ParseOutcome` enables automatic backtracking without explicit lookahead
3. **Arena allocation** — Flat AST in `ExprArena` with `ExprId` handles (4 bytes each)
4. **Incremental reuse** — Reuse unchanged declarations from previous parses for IDE responsiveness
5. **Comprehensive error recovery** — Bitset-based `TokenSet` for O(1) synchronization point detection

## Parser Structure

```rust
pub struct Parser<'a> {
    /// Token navigation via Cursor abstraction
    cursor: Cursor<'a>,
    /// Flat AST storage (Struct-of-Arrays layout)
    arena: ExprArena,
    /// Context flags for context-sensitive parsing
    context: ParseContext,
}

pub struct Cursor<'a> {
    tokens: &'a TokenList,
    interner: &'a StringInterner,
    pos: usize,
}

/// Context flags as a u16 bitfield (room for 16 flags).
pub struct ParseContext(u16);

impl ParseContext {
    const IN_PATTERN: Self = Self(0b0000_0001);
    const IN_TYPE: Self = Self(0b0000_0010);
    const NO_STRUCT_LIT: Self = Self(0b0000_0100);
    const CONST_EXPR: Self = Self(0b0000_1000);
    const IN_LOOP: Self = Self(0b0001_0000);
    const ALLOW_YIELD: Self = Self(0b0010_0000);
    const IN_FUNCTION: Self = Self(0b0100_0000);
    const IN_INDEX: Self = Self(0b1000_0000);
}
```

Context flags affect how tokens are interpreted. For example, `NO_STRUCT_LIT` prevents struct literal syntax inside `if` conditions to avoid `if Point { x: 0, ... }` ambiguity. `IN_TYPE` changes how `>` is parsed (closes a generic parameter list rather than comparison).

## Parsing Flow

```
TokenList
    │
    │ parse_module()
    ▼
Module { functions, types, tests, imports, traits, impls, extends, consts }
    │
    │ Each declaration calls:
    │   parse_function() / parse_test()     → Function / TestDef
    │   parse_type_decl()                   → TypeDecl
    │   parse_trait()                       → TraitDef
    │   parse_impl()                        → ImplDef
    │   parse_extend()                      → ExtendDef
    │   parse_use_inner()                   → Import
    │   parse_const()                       → ConstDef
    ▼
ExprArena (populated during parsing via alloc_expr)
```

## Public API

Three entry points cover different use cases:

```rust
/// Basic parsing — no metadata preservation.
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseOutput;

/// Parse with metadata for formatters and IDEs.
/// Preserves comments, blank lines, and trivia for lossless roundtrip.
pub fn parse_with_metadata(
    tokens: &TokenList,
    metadata: ModuleExtra,
    interner: &StringInterner,
) -> ParseOutput;

/// Incremental parsing — reuse unchanged declarations from old AST.
/// Only re-parses declarations that overlap with the text change.
pub fn parse_incremental(
    tokens: &TokenList,
    interner: &StringInterner,
    old_result: &ParseOutput,
    change: TextChange,
) -> ParseOutput;
```

All return `ParseOutput`:

```rust
pub struct ParseOutput {
    pub module: Module,
    pub arena: ExprArena,
    pub errors: Vec<ParseError>,
    pub warnings: Vec<ParseWarning>,
    pub metadata: ModuleExtra,
}
```

## Expression Precedence

Binary operator precedence uses a Pratt binding power table. Higher values bind tighter. See [Pratt Parser](pratt-parser.md) for details.

| Prec | Operators | Associativity | Binding Power |
|------|-----------|---------------|---------------|
| 1 | `??` | Right | (2, 1) |
| 2 | `\|\|` | Left | (3, 4) |
| 3 | `&&` | Left | (5, 6) |
| 4 | `\|` | Left | (7, 8) |
| 5 | `^` | Left | (9, 10) |
| 6 | `&` | Left | (11, 12) |
| 7 | `==` `!=` | Left | (13, 14) |
| 8 | `<` `>` `<=` `>=` | Left | (15, 16) |
| 9 | `..` `..=` | Non-assoc | 17 |
| 10 | `<<` `>>` | Left | (19, 20) |
| 11 | `+` `-` | Left | (21, 22) |
| 12 | `*` `/` `%` `div` | Left | (23, 24) |
| 13 | Unary `-` `!` `~` | Right | — |
| 14 | `.` `[]` `()` `?` `.await` `as` | Left | — |

**Note:** `>>` and `>=` are synthesized from adjacent `>` tokens. See [Token Design](../03-lexer/token-design.md#lexer-parser-token-boundary).

## Salsa Integration

Parsing is a Salsa query with automatic caching:

```rust
#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let tokens = tokens(db, file);
    parse(db, tokens)
}
```

## Grammar

The authoritative grammar is defined in EBNF at [`grammar.ebnf`](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf). Each production maps to parsing functions in `compiler/ori_parse/src/grammar/`.

## Related Documents

- [Pratt Parser](pratt-parser.md) — Binding power table and operator precedence
- [Error Recovery](error-recovery.md) — ParseOutcome, TokenSet, synchronization
- [Grammar Modules](grammar-modules.md) — Module organization and naming
- [Incremental Parsing](incremental-parsing.md) — IDE reuse of unchanged declarations
- [Grammar Spec](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) — Complete EBNF grammar definition

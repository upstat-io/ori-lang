---
title: "Token Design"
description: "Ori Compiler Design — Token Design"
order: 301
section: "Lexer"
---

# Token Design

This document describes the design of tokens in the Ori lexer — the types, metadata, and data structures that represent a tokenized source file. The lexical grammar is formally defined in the [grammar spec](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) (see the "LEXICAL GRAMMAR" section).

## Token Categories

### Keywords

Reserved words with specific meaning. The lexer resolves these from identifiers via length-bucketed lookup:

```rust
// Control flow
Let, If, Else, Then, For, In, Do, Loop, Break, Continue, Yield

// Declarations
Type, Trait, Impl, Pub, Use, Mut, Def, Extend, Extension

// Values
True, False, Self_, SelfType

// Pattern keywords
Match, With, Run, Try

// Testing
Tests, Skip

// Type system
Dyn, Where, Uses, As
```

**Soft keywords** (context-sensitive, only when followed by `(`):

```rust
Cache, Catch, Parallel, Spawn, Recurse, Timeout
```

**Reserved-future keywords** (produce an error, lex as identifier for recovery):

```rust
// asm, inline, static, union, view
```

**Note:** `Fn` and `While` are NOT keywords in Ori. Functions use `@name` syntax, and there is no while loop (use `loop` with `break`).

**Note:** `Return` exists in `TokenKind` as a reserved keyword despite Ori having no `return` statement. It is recognized during lexing so the parser can produce a targeted error message when users coming from other languages type `return` — rather than a confusing generic parse error.

### Operators

Binary and unary operators:

```rust
// Arithmetic
Plus,    // +
Minus,   // -
Star,    // *
Slash,   // /
Percent, // %

// Comparison
Eq,      // =
EqEq,    // ==
NotEq,   // != (RawTag: BangEqual, TokenKind: NotEq)
Lt,      // <
Gt,      // >
LtEq,    // <=
// Note: >= and >> are NOT lexed as single tokens (see § Greater-Than Design)
// TokenKind::GtEq and TokenKind::Shr exist for AST representation only —
// the parser synthesizes them from adjacent Gt tokens, they never appear
// in the lexer's token stream.

// Logical
And,     // &&
Or,      // ||
Bang,    // !

// Bitwise
Amp,     // &
Pipe,    // |
Caret,   // ^
Tilde,   // ~
LtLt,    // <<

// Special
Arrow,          // ->
FatArrow,       // =>
Question,       // ?
DoubleQuestion, // ??
DotDot,         // ..
DotDotEq,       // ..=
DotDotDot,      // ...
```

### Delimiters

Grouping and punctuation:

```rust
LParen,      // (
RParen,      // )
LBracket,    // [
RBracket,    // ]
LBrace,      // {
RBrace,      // }
Comma,       // ,
Colon,       // :
DoubleColon, // ::
Semicolon,   // ; (error-detection only — triggers "Ori uses newlines" suggestion)
Dot,         // .
At,          // @
Hash,        // #
HashBracket, // #[ (combined token for attributes)
Underscore,  // _ (standalone wildcard, distinct from identifiers containing underscores)
Dollar,      // $
Newline,     // \n (significant for statement separation)
```

### Literals

Value-carrying tokens:

```rust
Int(u64),                      // 42, 1_000_000, 0xFF, 0b1010
Float(u64),                    // 3.14, 2.5e-8 (stored as f64 bits for Eq/Hash)
String(Name),                  // "hello" (interned after unescape)
Char(char),                    // 'a'
Duration(u64, DurationUnit),   // 100ms, 5s, 1.5s
Size(u64, SizeUnit),           // 4kb, 10mb, 1.5kb
```

### Template Literals

Backtick-delimited interpolated strings produce context-dependent token types:

```rust
TemplateHead(Name),     // `text...{  — opening segment before first interpolation
TemplateMiddle(Name),   // }text...{  — segment between two interpolations
TemplateTail(Name),     // }text...`  — closing segment after last interpolation
TemplateFull(Name),     // `text...`  — complete template with no interpolations
FormatSpec(Name),       // :>10.2f   — format specifier within interpolation
```

All template text content is interned after escape processing (backtick escapes and brace escapes).

### Identifiers

```rust
Ident(Name),       // Interned identifier
```

Identifiers that match reserved keywords are resolved to the corresponding keyword variant during cooking. Soft keywords are resolved only when followed by `(`.

## Token Representation

### TokenKind

`TokenKind` is the core token enum. All data-carrying variants store their payload inline:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Keywords (no data)
    Let, If, Else, Then, /* ... */

    // Soft keywords (no data, resolved contextually)
    Cache, Catch, Parallel, Spawn, Recurse, Timeout,

    // Operators (no data)
    Plus, Minus, Star, /* ... */

    // Literals (with data)
    Int(u64),
    Float(u64),                     // f64 bits as u64
    String(Name),
    Char(char),
    Duration(u64, DurationUnit),
    Size(u64, SizeUnit),

    // Template literals
    TemplateHead(Name),
    TemplateMiddle(Name),
    TemplateTail(Name),
    TemplateFull(Name),
    FormatSpec(Name),

    // Identifiers
    Ident(Name),

    // Special
    Newline,
    Eof,
    Error,
}
```

**Design note:** `Float` stores `f64` bits as `u64` to derive `Eq` and `Hash`, which are required for Salsa compatibility. The bit representation preserves all floating-point values including NaN.

### Token (with span)

```rust
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,       // u32 start + u32 end = 8 bytes
}
```

### Span

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}
```

`u32` positions support source files up to 4 GiB. Files exceeding this limit saturate to `u32::MAX`.

## TokenFlags

`TokenFlags` is a bitfield that captures per-token metadata about the surrounding whitespace and context. It is stored in a parallel array alongside tokens in `TokenList`.

```rust
bitflags! {
    pub struct TokenFlags: u8 {
        const SPACE_BEFORE    = 0x01;  // Preceded by horizontal whitespace
        const NEWLINE_BEFORE  = 0x02;  // Preceded by newline
        const TRIVIA_BEFORE   = 0x04;  // Preceded by comment
        const ADJACENT        = 0x08;  // No whitespace/newline/trivia before
        const LINE_START      = 0x10;  // First token on line (after newline)
        const HAS_ERROR       = 0x20;  // Error during cooking (overflow, etc.)
        const CONTEXTUAL_KW   = 0x40;  // Soft keyword resolved via lookahead
        const IS_DOC          = 0x80;  // First token after a doc comment
    }
}
```

### Flag Semantics

| Flag | Set When | Used By |
|------|----------|---------|
| `SPACE_BEFORE` | Horizontal whitespace precedes the token | Formatter (spacing decisions) |
| `NEWLINE_BEFORE` | A newline precedes the token | Parser (statement boundaries) |
| `TRIVIA_BEFORE` | A comment precedes the token | Formatter (comment attachment) |
| `ADJACENT` | No whitespace, newline, or trivia before | Parser (compound operator synthesis: `> >` → `>>`) |
| `LINE_START` | First non-trivia token on a line | Formatter (indentation) |
| `HAS_ERROR` | Cooking produced an error (overflow, reserved keyword, etc.) | Diagnostics |
| `CONTEXTUAL_KW` | Token is a soft keyword resolved by lookahead | Parser (distinguishes `cache(` keyword from `cache` identifier) |
| `IS_DOC` | First token after a doc comment | Parser (doc comment attachment to declarations) |

### Flag Production

Flags are accumulated in a pending state during trivia scanning and applied when the next content token is pushed:

```
Whitespace  → pending |= SPACE_BEFORE
Newline     → pending |= NEWLINE_BEFORE | LINE_START
LineComment → pending |= TRIVIA_BEFORE
Content token → flags = pending; if pending == 0 → flags |= ADJACENT
```

The `IS_DOC` flag is only computed in the `lex_with_comments()` path, not the fast `lex()` path.

## TokenList

`TokenList` stores the complete output of lexing a source file. It uses three parallel arrays:

```rust
pub struct TokenList {
    tokens: Vec<Token>,         // kind + span per token
    tags: Vec<u8>,              // discriminant index per token
    flags: Vec<TokenFlags>,     // metadata per token
}
```

### Discriminant Tags

The `tags` array stores the `TokenKind` discriminant index (a `u8`), enabling O(1) kind checks without pattern matching:

```rust
// Instead of: matches!(tokens[i].kind, TokenKind::Let | TokenKind::If | ...)
// The parser can check: tags[i] == LET_TAG || tags[i] == IF_TAG || ...
```

This is particularly valuable for the parser's `check()` and `at()` methods, which run on every token during parsing.

### Position-Independent Equality

`TokenList` has custom `Eq`/`Hash` implementations that compare `TokenKind` values and `TokenFlags` but ignore `Span` positions (and `tags`, which are derived from kinds). This enables Salsa early cutoff: whitespace-only edits change span positions but produce equal `TokenList` values, preventing unnecessary re-parsing.

### Pre-allocation

All arrays are pre-allocated based on source length:

```rust
let capacity = source_len / 2 + 1;  // ~1 token per 2-3 bytes
TokenList {
    tokens: Vec::with_capacity(capacity),
    tags: Vec::with_capacity(capacity),
    flags: Vec::with_capacity(capacity),
}
```

## Ori-Specific Tokens

### @ (Function Sigil)

```rust
At  // @

// Used for function declarations
@main () -> void = ...
@add (a: int, b: int) -> int = ...
```

### $ (Config Sigil)

```rust
Dollar  // $

// Used for config variables
$timeout = 30s
$max_retries = 3
```

### #[ (Attribute Opener)

`#[` is lexed as a single combined token to avoid lookahead when parsing attributes:

```rust
HashBracket  // #[

// #[derive(Eq, Clone)] → HashBracket Ident LParen ... RParen RBracket
#[derive(Eq, Clone)]
#[skip("reason")]
```

## Integer Literals

Three formats with underscore separators:

```
42          → Int(42)       // decimal
1_000_000   → Int(1000000)  // underscores stripped during parsing
0xFF        → Int(255)      // hexadecimal (0x prefix)
0xDEAD_BEEF → Int(...)      // hex with underscores
0b1010      → Int(10)       // binary (0b prefix)
0b1111_0000 → Int(240)      // binary with underscores
```

Integer overflow (value > `u64::MAX`) produces a `TokenKind::Error` with `HAS_ERROR` flag and accumulates an `IntegerOverflow` error.

## Float Literals

```
3.14       → Float(bits)    // standard
2.5e-8     → Float(bits)    // scientific notation
1e10       → Float(bits)    // integer mantissa
1_000.5    → Float(bits)    // underscores in mantissa
```

Floats are parsed via `f64::from_str` (with underscores stripped) and stored as `u64` bit patterns for `Eq`/`Hash` compatibility.

## String Literals

```
"hello"              → String(Name)   // simple
"with \"escapes\""   → String(Name)   // escaped quotes
"line\nbreak"        → String(Name)   // escape sequences
```

Valid escapes: `\"` `\\` `\n` `\t` `\r` `\0`

Invalid: `\'` produces a `SingleQuoteEscapeInString` error with suggestion to use `'` directly.

A fast path detects strings with no backslashes and interns the source slice directly, avoiding allocation.

## Char Literals

```
'a'    → Char('a')     // ASCII
'λ'    → Char('λ')     // Unicode (2-byte)
'中'   → Char('中')     // Unicode (3-byte)
'\n'   → Char('\n')    // escape sequence
```

Valid escapes: `\'` `\\` `\n` `\t` `\r` `\0`

Invalid: `\"` produces a `DoubleQuoteEscapeInChar` error with suggestion to use `"` directly.

## Duration Literals

All time units from nanoseconds to hours:

```
100ns  → Duration(100, Nanoseconds)
50us   → Duration(50, Microseconds)
100ms  → Duration(100, Milliseconds)
5s     → Duration(5, Seconds)
2m     → Duration(2, Minutes)
1h     → Duration(1, Hours)
```

Decimal durations are converted to integer base units:

```
1.5s   → Duration(1_500_000_000, Nanoseconds)   // 1.5 seconds in nanoseconds
0.5m   → Duration(30, Seconds)                   // 0.5 minutes in seconds
```

If a decimal value cannot be represented exactly as an integer in its base unit, a `DecimalNotRepresentable` error is produced.

## Size Literals

All size units from bytes to terabytes:

```
1024b  → Size(1024, Bytes)
4kb    → Size(4, Kilobytes)
10mb   → Size(10, Megabytes)
2gb    → Size(2, Gigabytes)
1tb    → Size(1, Terabytes)
```

Decimal sizes follow the same exact-representation rule:

```
1.5kb  → Size(1500, Bytes)   // 1.5 kilobytes in bytes
```

## Comments

Ori has line comments only (`//`). No block comments (`/* */`).

### Comment Handling

By default, `lex()` strips comments from the token stream — they contribute only to `TokenFlags` (setting `TRIVIA_BEFORE`). The `lex_with_comments()` path preserves and classifies them:

```rust
pub struct Comment {
    pub kind: CommentKind,
    pub span: Span,
    pub text: Name,       // interned comment text (after `//` prefix)
}
```

### Comment Classification

```rust
pub enum CommentKind {
    Regular,          // // normal comment
    DocDescription,   // // #Description text
    DocMember,        // // * name: description (params and fields)
    DocWarning,       // // !Warning or !Panics text
    DocExample,       // // >example() -> result
}
```

Doc comments are attached to declarations via the `IS_DOC` flag on the token following the comment.

### Detached Doc Comment Detection

The `lex_with_comments()` path tracks pending doc comments and warns if:
- A blank line separates a doc comment from its declaration
- No declaration keyword (`@`, `type`, `trait`, `let`, `pub`, `impl`, `use`) follows

## Whitespace

Horizontal whitespace (spaces, tabs, `\r`) is consumed by the raw scanner and recorded in `TokenFlags`:
- `SPACE_BEFORE` indicates horizontal whitespace preceded the token
- `ADJACENT` indicates no whitespace of any kind preceded the token

### Newlines

Newlines are significant tokens (`TokenKind::Newline`) used by the parser for statement separation:

```
let x = 42\nlet y = 10
→ [Let, Ident(x), Eq, Int(42), Newline, Let, Ident(y), Eq, Int(10), Eof]
```

### Line Continuation

A backslash at the end of a line (`\ \n`) suppresses the newline token, allowing multi-line expressions:

```ori
let result = very_long_expression \
    + another_part \
    + final_part
```

## Error Token

Invalid input produces `TokenKind::Error` tokens with the `HAS_ERROR` flag. This allows the parser to:
1. See where errors occurred via spans
2. Attempt recovery by skipping error tokens
3. Report multiple errors from a single source file

## Greater-Than Token Design

The lexer produces individual `>` tokens, never `>>` (right shift) or `>=` (greater-equal) as single tokens. The parser synthesizes compound operators from adjacent tokens in expression context.

This design enables the type parser to handle nested generics without special lexer modes:

```ori
type Nested = Result<Result<int, str>, str>
//                                    ^^--- Two > tokens closing two generic lists

let x = 8 >> 2  // Shift right (synthesized from adjacent > >)
let y = x >= 0  // Greater-equal (synthesized from adjacent > =)
```

| Operator | Lexer Output | Parser Handling |
|----------|--------------|-----------------|
| `>` | Single `Gt` token | Used as-is |
| `>>` | Two adjacent `Gt` tokens | Synthesized in expression context |
| `>=` | Adjacent `Gt` + `Eq` tokens | Synthesized in expression context |
| `<` | Single `Lt` token | Used as-is |
| `<<` | Single `LtLt` token | Used as-is |
| `<=` | Single `LtEq` token | Used as-is |

Adjacency is determined by span endpoints: `tokens[i].span.end == tokens[i+1].span.start`.

## Related Documents

- [Lexer Overview](index.md) — Architecture, two-layer design, performance
- [Architecture: Pipeline](../01-architecture/pipeline.md) — Pipeline overview

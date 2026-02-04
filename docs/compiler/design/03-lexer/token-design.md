---
title: "Token Design"
description: "Ori Compiler Design — Token Design"
order: 301
section: "Lexer"
---

# Token Design

This document describes the design of tokens in the Ori lexer. The lexical grammar is formally defined in [Parser Overview § Formal Grammar](../04-parser/index.md#formal-grammar) (see the "LEXICAL GRAMMAR" section).

## Token Categories

### Keywords

Reserved words with specific meaning:

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

// Reserved for future
Async  // Reserved for future async features
```

**Note:** `Fn` and `While` are NOT keywords in Ori. Functions use `@name` syntax, and there is no while loop (use `loop` with `break`).

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
BangEq,  // !=
Lt,      // <
Gt,      // >
LtEq,    // <=
GtEq,    // >=

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
// Note: >> and >= are NOT lexed as single tokens (see below)

// Special
Arrow,       // ->
FatArrow,    // =>
Question,    // ?
DoubleQuestion, // ??
DotDot,      // ..
DotDotEq,    // ..=
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
Semicolon,   // ;
Dot,         // .
At,          // @
Hash,        // #
HashBracket, // #[ (combined token for attributes)
Dollar,      // $
Newline,     // \n (significant for statement separation)
```

### Literals

Value-carrying tokens:

```rust
Int(u64),           // 42, 1_000_000, 0xFF (negation folded in parser)
Float(f64),         // 3.14, 2.5e-8
String(Name),       // "hello" (interned for Hash compatibility)
Char(char),         // 'a'
Bool(bool),         // true, false
Duration(Duration), // 100ms, 5s
Size(Size),         // 4kb, 10mb
```

### Identifiers

```rust
Ident(Name),       // Interned identifier
```

## Token Representation

### TokenKind

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    // Keywords (no data)
    Let,
    If,
    // ...

    // Operators (no data)
    Plus,
    Minus,
    // ...

    // Literals (with data)
    Int(u64),
    Float(f64),
    String(Name),     // Interned
    Ident(Name),      // Identifier
    // ...

    // Special
    Eof,
    Error,
}
```

### Token (with span)

```rust
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
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

## Ori-Specific Tokens

### @ (Function Ori)

```rust
At  // @

// Used for function names
@main () -> void = ...
@add (a: int, b: int) -> int = ...
```

### $ (Config Ori)

```rust
Dollar  // $

// Used for config variables
$timeout = 30s
$max_retries = 3
```

### name: (Named Arguments)

```rust
Dot,        // .
Identifier, // name
Colon,      // :

// Parsed as: Dot Identifier Colon
map(over: items, transform: fn)
```

### # and #[ (Attributes)

For efficiency, `#[` is lexed as a single combined token:

```rust
Hash,        // # (standalone)
HashBracket, // #[ (combined for attributes)

// #[derive(Eq, Clone)] is lexed as: HashBracket Ident LParen ... RParen RBracket
#[derive(Eq, Clone)]
#[skip("reason")]
```

This avoids lookahead when parsing attributes.

## Integer Literals

Multiple formats supported:

```rust
// Decimal
42
1_000_000   // Underscores for readability

// Hexadecimal
0xFF
0xDEAD_BEEF

// Binary
0b1010
0b1111_0000
```

Parsing uses zero-allocation helpers for common cases:

```rust
#[regex(r"[0-9][0-9_]*", |lex| parse_int_skip_underscores(lex.slice(), 10))]
Int(u64),

#[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| parse_int_skip_underscores(&lex.slice()[2..], 16))]
HexInt(u64),

#[regex(r"0b[01][01_]*", |lex| parse_int_skip_underscores(&lex.slice()[2..], 2))]
BinInt(u64),
```

## String Literals

```rust
// Simple strings
"hello"
"with \"escapes\""

// Escape sequences
\\  -> backslash
\"  -> double quote
\n  -> newline
\t  -> tab
\r  -> carriage return
```

## Duration Literals

All time units from nanoseconds to hours:

```rust
#[regex(r"[0-9]+ns")]
Nanoseconds,

#[regex(r"[0-9]+us")]
Microseconds,

#[regex(r"[0-9]+ms")]
Milliseconds,

#[regex(r"[0-9]+s")]
Seconds,

#[regex(r"[0-9]+m")]
Minutes,

#[regex(r"[0-9]+h")]
Hours,
```

Post-processing combines into `Duration`:

```rust
pub enum DurationUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

// TokenKind::Duration(value, unit)
```

## Size Literals

All size units from bytes to terabytes:

```rust
#[regex(r"[0-9]+b")]
Bytes,

#[regex(r"[0-9]+kb")]
Kilobytes,

#[regex(r"[0-9]+mb")]
Megabytes,

#[regex(r"[0-9]+gb")]
Gigabytes,

#[regex(r"[0-9]+tb")]
Terabytes,
```

Post-processing combines into `Size`:

```rust
pub enum SizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

// TokenKind::Size(value, unit)
```

## Comments

Comments are NOT part of the token stream by default, but can be preserved:

### Standard Lexing

The default `lex()` function strips comments:

```rust
// Line comments
#[regex(r"//[^\n]*")]
LineComment,  // Not added to token output
```

### Comment-Preserving Lexing

The `lex_with_comments()` function returns both tokens and classified comments:

```rust
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput {
    // Returns TokenList + CommentList
}
```

### Comment Classification

Comments are classified for documentation/formatter purposes:

```rust
pub enum CommentKind {
    Regular,        // Normal comments
    DocDescription, // // Description text
    DocParam,       // // * param_name: description
    DocWarning,     // // ! Warning text
    DocExample,     // // > example -> result
}
```

**Note:** Ori has no block comments (`/* */`). Only line comments (`//`) are supported.

## Whitespace

Horizontal whitespace (spaces, tabs) is stripped:

```rust
#[logos(skip r"[ \t\r]+")]  // Skip horizontal whitespace
```

**Important:** Newlines are NOT stripped - they become `Newline` tokens used for statement separation.

### Line Continuation

A backslash at the end of a line allows continuing expressions:

```rust
#[regex(r"\\[ \t]*\n")]
LineContinuation,  // Skipped (allows multi-line expressions)
```

```ori
let result = very_long_expression \
    + another_part \
    + final_part
```

## Error Token

Invalid characters become Error tokens:

```rust
#[error]
Error,
```

This allows the parser to:
1. See where errors occurred (via span)
2. Attempt recovery
3. Report multiple errors

## Keyword vs Identifier

Keywords take precedence:

```rust
// "let" -> Let (keyword)
// "letter" -> Identifier("letter")
```

logos handles this automatically via longest-match semantics.

## Reserved Words

Some words are reserved for future language features:

```rust
Async,   // Reserved for future async features
```

These lex as keywords to prevent their use as identifiers.

**Note:** `await` is NOT currently reserved (planned for future).

## Lexer-Parser Token Boundary

### `>` Token Design

The lexer produces individual `>` tokens, never `>>` (right shift) or `>=` (greater-equal) as single tokens. The parser synthesizes compound operators from adjacent tokens in expression context.

This separation allows the type parser to handle nested generics:

```ori
type MyResult = Result<Result<int, str>, str>
//                                    ^^-- Two > tokens closing two generic lists

let x = 8 >> 2  // Shift right (synthesized from adjacent > >)
let y = x >= 0  // Greater-equal (synthesized from adjacent > =)
```

### Token Production

| Operator | Lexer Output | Parser Handling |
|----------|--------------|-----------------|
| `>` | Single `Gt` token | Used as-is |
| `>>` | Two adjacent `Gt` tokens | Synthesized in expression context |
| `>=` | Adjacent `Gt` + `Eq` tokens | Synthesized in expression context |
| `<` | Single `Lt` token | Used as-is |
| `<<` | Single `Shl` token | Used as-is |
| `<=` | Single `LtEq` token | Used as-is |

### Whitespace Sensitivity

Compound operator synthesis requires adjacent tokens (no whitespace):

```ori
8 >> 2   // Valid: >> from adjacent > >
8 > > 2  // Invalid: two separate > operators

5 >= 3   // Valid: >= from adjacent > =
5 > = 3  // Invalid: > followed by =
```

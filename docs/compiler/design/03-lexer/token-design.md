# Token Design

This document describes the design of tokens in the Ori lexer.

## Token Categories

### Keywords

Reserved words with specific meaning:

```rust
// Control flow
Let, If, Else, Then, For, In, Do, Loop, While, Break, Continue, Yield

// Declarations
Fn, Type, Trait, Impl, Pub, Use, Mut

// Values
True, False, Self_, SelfType

// Pattern keywords
Match, With, Run, Try

// Special
Where, Uses, As, Async
```

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
GtGt,    // >>

// Special
Arrow,       // ->
FatArrow,    // =>
Question,    // ?
Coalesce,    // ??
DotDot,      // ..
DotDotEq,    // ..=
```

### Delimiters

Grouping and punctuation:

```rust
LParen,    // (
RParen,    // )
LBracket,  // [
RBracket,  // ]
LBrace,    // {
RBrace,    // }
Comma,     // ,
Colon,     // :
ColonColon,// ::
Semicolon, // ;
Dot,       // .
At,        // @
Hash,      // #
Dollar,    // $
```

### Literals

Value-carrying tokens:

```rust
Int(i64),           // 42, 1_000_000, 0xFF
Float(f64),         // 3.14, 2.5e-8
String(String),     // "hello"
Char(char),         // 'a'
Bool(bool),         // true, false
Duration(Duration), // 100ms, 5s
Size(Size),         // 4kb, 10mb
```

### Identifiers

```rust
Identifier(Name),  // Interned identifier
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
    Int(i64),
    Float(f64),
    String(String),
    Identifier(Name),
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

### # (Attributes)

```rust
Hash,    // #
LBracket // [

// Parsed as: Hash LBracket ... RBracket
#[derive(Eq, Clone)]
#[skip("reason")]
```

## Integer Literals

Multiple formats supported:

```rust
// Decimal
42
1_000_000   // Underscores for readability

// Hexadecimal
0xFF
0xDEAD_BEEF
```

Parsing:

```rust
#[regex(r"0x[0-9a-fA-F_]+", |lex| {
    i64::from_str_radix(&lex.slice()[2..].replace("_", ""), 16).ok()
})]
HexInt(i64),

#[regex(r"[0-9][0-9_]*", |lex| {
    lex.slice().replace("_", "").parse().ok()
})]
Int(i64),
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

```rust
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
pub enum Duration {
    Milliseconds(u64),
    Seconds(u64),
    Minutes(u64),
    Hours(u64),
}
```

## Size Literals

```rust
#[regex(r"[0-9]+b")]
Bytes,

#[regex(r"[0-9]+kb")]
Kilobytes,

#[regex(r"[0-9]+mb")]
Megabytes,

#[regex(r"[0-9]+gb")]
Gigabytes,
```

Post-processing combines into `Size`:

```rust
pub enum Size {
    Bytes(u64),
    Kilobytes(u64),
    Megabytes(u64),
    Gigabytes(u64),
}
```

## Comments

Comments are **not** tokens - they're stripped during lexing:

```rust
// Line comments
#[regex(r"//[^\n]*", logos::skip)]

// No block comments in Ori
```

## Whitespace

Whitespace is also stripped:

```rust
#[regex(r"[ \t\n\r]+", logos::skip)]
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

Some words are reserved but not yet used:

```rust
Async,   // Reserved for future async features
Await,   // Reserved for future async features
```

These lex as keywords to prevent their use as identifiers.

---
title: "Lexer Overview"
description: "Ori Compiler Design — Lexer Overview"
order: 300
section: "Lexer"
---

# Lexer Overview

The Ori lexer converts source text into a stream of tokens. It's implemented using the [logos](https://github.com/maciejhirsz/logos) crate for DFA-based tokenization.

## Location

```
compiler/ori_lexer/src/
├── lib.rs              # Public API: lex(), lex_with_comments(), LexOutput
├── raw_token.rs        # Logos-derived RawToken enum
├── convert.rs          # Token conversion with string interning
├── escape.rs           # Escape sequence processing
├── comments.rs         # Comment classification/normalization
└── parse_helpers.rs    # Numeric literal parsing utilities
```

The lexer is a separate crate with minimal dependencies:
- `ori_ir` - for `Token`, `TokenKind`, `Span`, `TokenList`, `StringInterner`
- `logos` - for DFA-based tokenization

## Design Goals

1. **Fast tokenization** via logos DFA
2. **String interning** for identifiers
3. **Special literals** (duration, size)
4. **No errors** - invalid input becomes Error token

## Architecture

```
Source Text
    │
    │ logos::Lexer
    ▼
Raw Tokens (logos-generated)
    │
    │ Post-processing
    ▼
TokenList (with spans, interned names)
```

## Token Definition

Tokens are defined in `raw_token.rs` using the logos derive macro, then converted to final `TokenKind` in `convert.rs`:

```rust
#[derive(Logos, Debug, Clone, Copy, PartialEq)]
#[logos(skip r"[ \t\r]+")] // Skip horizontal whitespace
pub enum RawToken {
    // Comments and trivia
    #[regex(r"//[^\n]*")]
    LineComment,
    #[token("\n")]
    Newline,
    #[regex(r"\\[ \t]*\n")]
    LineContinuation,

    // Keywords
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("then")]
    Then,
    #[token("def")]
    Def,
    #[token("tests")]
    Tests,
    #[token("dyn")]
    Dyn,
    #[token("extend")]
    Extend,
    #[token("extension")]
    Extension,
    #[token("async")]
    Async,
    #[token("mut")]
    Mut,
    // ... more keywords

    // Literals with inline parsing (zero-allocation for common cases)
    #[regex(r"[0-9][0-9_]*", |lex| parse_int_skip_underscores(lex.slice(), 10))]
    Int(u64),
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", |lex| parse_int_skip_underscores(&lex.slice()[2..], 16))]
    HexInt(u64),
    #[regex(r"0b[01][01_]*", |lex| parse_int_skip_underscores(&lex.slice()[2..], 2))]
    BinInt(u64),
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?", ...)]
    Float(f64),

    // Duration literals (each suffix is a separate variant)
    #[regex(r"[0-9]+ns", ...)]
    DurationNs((u64, DurationUnit)),
    #[regex(r"[0-9]+us", ...)]
    DurationUs((u64, DurationUnit)),
    // ... ms, s, m, h

    // Size literals
    #[regex(r"[0-9]+b", ...)]
    SizeB((u64, SizeUnit)),
    // ... kb, mb, gb, tb

    // String/Char (unescaping done in convert.rs)
    #[regex(r#""([^"\\\n\r]|\\.)*""#)]
    String,
    #[regex(r"'([^'\\\n\r]|\\.)'")]
    Char,

    // Identifiers (interned in convert.rs)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}
```

## Tokenization Process

### Entry Points

Two main functions:
- `lex(source, interner)` - Standard tokenization for parsing
- `lex_with_comments(source, interner)` - Preserves comments for formatter

### 1. Logos Tokenization

```rust
pub fn lex(source: &str, interner: &StringInterner) -> TokenList {
    let mut result = TokenList::new();
    let mut logos = RawToken::lexer(source);

    while let Some(token_result) = logos.next() {
        let span = Span::try_from_range(logos.span()).unwrap_or_else(|_| {
            // File exceeds u32::MAX - use saturated position
            Span::new(u32::MAX.saturating_sub(1), u32::MAX)
        });
        let slice = logos.slice();

        match token_result {
            Ok(raw) => {
                match raw {
                    RawToken::LineComment | RawToken::LineContinuation => {}  // Skip
                    RawToken::Newline => result.push(Token::new(TokenKind::Newline, span)),
                    _ => {
                        let kind = convert_token(raw, slice, interner);
                        result.push(Token::new(kind, span));
                    }
                }
            }
            Err(()) => result.push(Token::new(TokenKind::Error, span)),
        }
    }

    result.push(Token::new(TokenKind::Eof, Span::point(source.len() as u32)));
    result
}
```

### 2. Token Conversion

The `convert_token()` function in `convert.rs`:
- Interns identifiers to `Name` indices
- Unescapes string and char literals
- Maps `RawToken` variants to `TokenKind`

## Escape Sequence Handling

String and character unescaping share a common `resolve_escape()` helper:

```rust
fn resolve_escape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '0' => Some('\0'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}
```

Both `unescape_string()` and `unescape_char()` delegate to this function, avoiding duplicated escape logic.

## Special Literals

### Duration Literals

```
100ns  -> Duration(100, Nanoseconds)
50us   -> Duration(50, Microseconds)
100ms  -> Duration(100, Milliseconds)
5s     -> Duration(5, Seconds)
2m     -> Duration(2, Minutes)
1h     -> Duration(1, Hours)
```

### Size Literals

```
1024b  -> Size(1024, Bytes)
4kb    -> Size(4, Kilobytes)
10mb   -> Size(10, Megabytes)
2gb    -> Size(2, Gigabytes)
1tb    -> Size(1, Terabytes)
```

### Integer Literals

```
42       -> Int(42)
1_000    -> Int(1000)      // underscores for readability
0xFF     -> HexInt(255)    // hex
0b1010   -> BinInt(10)     // binary
```

## TokenList Structure

Currently Array-of-Structs (AoS). Migration to SoA planned (see `plans/parser_v2/section-02-lexer.md` § 02.9).

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TokenList {
    tokens: Vec<Token>,  // Token = { kind: TokenKind, span: Span }
}

impl TokenList {
    pub fn get(&self, index: usize) -> Option<&Token> {
        self.tokens.get(index)
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }
}
```

## Newline Handling

Newlines are significant tokens used by the parser for statement separation. The lexer:
- Emits `TokenKind::Newline` for `\n`
- Supports line continuation with `\` at end of line
- Skips horizontal whitespace (spaces, tabs)

```rust
// Input: "let x = 42\nlet y = 10"
// Output: [Let, Ident(x), Eq, Int(42), Newline, Let, Ident(y), Eq, Int(10), Eof]
```

## No Error Recovery

The lexer does not attempt error recovery. Invalid characters become `Error` tokens:

```rust
// Input: "let x = @#$"
// @ is a valid token, but say if there was invalid Unicode:
// Output: [Let, Ident, Eq, At, Error, Error, Eof]
```

Error handling is deferred to the parser, which can provide better diagnostics with context.

### Special Error Tokens

Two specific error types catch semantic validation at lex time:

```rust
FloatDurationError  // e.g., 1.5s (float + duration suffix not allowed)
FloatSizeError      // e.g., 2.5kb (float + size suffix not allowed)
```

These high-priority regexes prevent ambiguous tokenization:

```rust
#[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?ns", priority = 2)]
FloatDurationNs,  // Error: floats can't have duration suffix
```

## Token Statistics

| Aspect | Details |
|--------|---------|
| Total token kinds | 116 variants |
| Token size | 24 bytes (TokenKind 16 + Span 8) |
| Escape sequences | 7 (`\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`) |
| Duration units | 6 (ns, us, ms, s, m, h) |
| Size units | 5 (b, kb, mb, gb, tb) |
| Integer formats | 3 (decimal, hex `0x`, binary `0b`) |

## Greater-Than Token Handling

The lexer produces individual `>` tokens to enable nested generic parsing:

```ori
type Nested = Result<Result<int, str>, str>
//                                    ^^--- Must be TWO separate > tokens
```

| Operator | Lexer Output | Parser Handling |
|----------|--------------|-----------------|
| `>` | Single `Gt` token | Used as-is |
| `>>` | Two adjacent `Gt` tokens | Parser synthesizes in expression context |
| `>=` | `Gt` + `Eq` tokens | Parser synthesizes in expression context |

The parser checks token adjacency (span endpoints touch) to combine tokens when needed.

## Performance

Using logos provides:

- **DFA-based** - O(n) tokenization
- **Zero-copy** where possible
- **Compiled regex** - patterns compiled at build time

## Salsa Integration

Tokenization is a Salsa query:

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let source = file.text(db);
    tokenize(db, &source)
}
```

This enables:
- Caching of token results
- Early cutoff if tokens unchanged
- Dependency tracking

## Related Documents

- [Token Design](token-design.md) - Token type details
- [Architecture: Pipeline](../01-architecture/pipeline.md) - Pipeline overview

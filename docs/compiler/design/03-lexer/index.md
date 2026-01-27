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
compiler/ori_lexer/src/lib.rs (~707 lines)
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

Tokens are defined using logos derive macro:

```rust
#[derive(Logos, Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    #[token("let")]
    Let,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("then")]
    Then,
    // ...

    // Operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    // ...

    // Literals
    #[regex(r"[0-9]+", |lex| lex.slice().parse().ok())]
    Int(i64),

    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice().parse().ok())]
    Float(f64),

    #[regex(r#""[^"]*""#, |lex| {
        let s = lex.slice();
        Some(s[1..s.len()-1].to_string())
    })]
    String(String),

    // Identifiers (interned later)
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,

    // Special - handled in post-processing
    #[regex(r"[0-9]+(ms|s|m|h)")]
    Duration,

    #[regex(r"[0-9]+(b|kb|mb|gb)")]
    Size,

    // Error fallback
    #[error]
    Error,
}
```

## Tokenization Process

### 1. Initial Tokenization

```rust
pub fn tokenize(db: &dyn Db, source: &str) -> TokenList {
    let lexer = TokenKind::lexer(source);

    let mut tokens = Vec::new();
    let mut spans = Vec::new();

    for (kind, span) in lexer.spanned() {
        tokens.push(kind);
        spans.push(Span::new(span.start, span.end));
    }

    // ...
}
```

### 2. Post-Processing

After logos tokenization:

```rust
// Intern identifiers
for (i, token) in tokens.iter_mut().enumerate() {
    if let TokenKind::Ident = token {
        let text = &source[spans[i].start..spans[i].end];
        let name = db.interner().intern(text);
        *token = TokenKind::Identifier(name);
    }
}

// Parse duration literals
for (i, token) in tokens.iter_mut().enumerate() {
    if let TokenKind::Duration = token {
        let text = &source[spans[i].start..spans[i].end];
        *token = parse_duration(text);
    }
}
```

### 3. Result

```rust
TokenList {
    tokens,
    spans,
}
```

## Special Literals

### Duration Literals

```
100ms  -> Duration(Milliseconds(100))
5s     -> Duration(Seconds(5))
2m     -> Duration(Minutes(2))
1h     -> Duration(Hours(1))
```

### Size Literals

```
1024b  -> Size(Bytes(1024))
4kb    -> Size(Kilobytes(4))
10mb   -> Size(Megabytes(10))
2gb    -> Size(Gigabytes(2))
```

## TokenList Structure

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TokenList {
    tokens: Vec<TokenKind>,
    spans: Vec<Span>,
}

impl TokenList {
    pub fn get(&self, index: usize) -> Option<&TokenKind> {
        self.tokens.get(index)
    }

    pub fn span(&self, index: usize) -> Span {
        self.spans[index]
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }
}
```

## No Error Recovery

The lexer does not attempt error recovery. Invalid characters become `Error` tokens:

```rust
// Input: "let x = @#$"
// Output: [Let, Ident, Eq, Error, Error, Error]
```

Error handling is deferred to the parser, which can provide better diagnostics with context.

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

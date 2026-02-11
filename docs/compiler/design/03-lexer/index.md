---
title: "Lexer Overview"
description: "Ori Compiler Design — Lexer Overview"
order: 300
section: "Lexer"
---

# Lexer Overview

The Ori lexer converts source text into a stream of tokens. It uses a **two-layer architecture** inspired by `rustc_lexer` / `rustc_parse::lexer`: a raw scanner that operates on bytes with zero dependencies, and a cooking layer that transforms raw tokens into compiler-ready `TokenKind` values with interning, keyword resolution, and diagnostics.

## Crate Structure

```
compiler/
├── ori_lexer_core/src/         # Layer 1: Raw scanner (zero ori_* deps)
│   ├── lib.rs                  # Public exports, crate docs
│   ├── tag.rs                  # RawTag enum (#[repr(u8)])
│   ├── raw_scanner.rs          # Byte-level state machine
│   ├── cursor.rs               # Cursor for byte navigation
│   └── source_buffer.rs        # UTF-8 validation + sentinel byte
│
└── ori_lexer/src/              # Layer 2: Cooking + driver
    ├── lib.rs                  # Public API: lex(), lex_with_comments(), LexOutput
    ├── cooker.rs               # TokenCooker: RawTag → TokenKind
    ├── keywords.rs             # Keyword resolution (reserved + soft + future)
    ├── cook_escape.rs          # Spec-strict escape processing
    ├── lex_error.rs            # LexError types (WHERE+WHAT+WHY+HOW)
    ├── comments.rs             # Comment classification (doc vs regular)
    ├── parse_helpers.rs        # Numeric literal parsing utilities
    ├── unicode_confusables.rs  # Unicode → ASCII suggestions
    └── what_is_next.rs         # Context-aware error suggestions
```

### Dependencies

- **`ori_lexer_core`** — Zero `ori_*` dependencies. Depends only on `std`. Can be consumed by external tools (syntax highlighters, editor plugins) without pulling in the compiler.
- **`ori_lexer`** — Depends on `ori_ir` (for `Token`, `TokenKind`, `Span`, `TokenList`, `StringInterner`, `CommentList`) and `ori_lexer_core`.

## Design Goals

1. **Reusable raw scanner** — `ori_lexer_core` has no compiler dependencies, enabling use in external tooling
2. **Fast tokenization** — SIMD-accelerated string/template scanning, pre-allocated buffers, `#[inline]` on hot paths
3. **Rich metadata** — `TokenFlags` capture whitespace context for formatting and whitespace-sensitive parsing
4. **Comprehensive diagnostics** — Errors with WHERE+WHAT+WHY+HOW structure, cross-language pattern detection
5. **Template literals** — First-class support for backtick-delimited interpolated strings with format specs

## Architecture

```
Source Text (bytes)
    │
    │  RawScanner (ori_lexer_core)
    │  byte-level state machine, SIMD acceleration
    ▼
(RawTag, len) pairs
    │
    │  TokenCooker (ori_lexer)
    │  keyword resolution, interning, escape processing, numeric parsing
    ▼
TokenKind values
    │
    │  Driver Loop (ori_lexer::lex / lex_with_comments)
    │  span calculation, flag accumulation, comment classification
    ▼
TokenList (tokens + tags + flags)
```

### Layer 1: Raw Scanner

The raw scanner (`ori_lexer_core::RawScanner`) is a byte-level state machine that produces `(RawTag, len)` pairs. It operates entirely on the stack with no allocations.

Key characteristics:
- **Sentinel byte** — A NUL byte is appended to the source buffer, eliminating bounds checks during scanning
- **SIMD acceleration** — 16-byte chunk scanning for string and template literal bodies (finds delimiters like `"`, `\`, `{`, `` ` `` in parallel)
- **Template depth tracking** — A stack of `InterpolationDepth` structs tracks nested parentheses, brackets, and braces within template interpolations
- **No interning or keyword resolution** — The raw scanner sees identifiers and keywords identically (`RawTag::Ident`)

```rust
// RawTag is a u8 enum with ~50 variants
#[repr(u8)]
pub enum RawTag {
    Ident, Int, Float, HexInt, BinInt, String, Char, Duration, Size,
    TemplateHead, TemplateMiddle, TemplateTail, TemplateComplete, FormatSpec,
    Plus, Minus, Star, Slash, /* ... operators ... */,
    LParen, RParen, LBrace, RBrace, /* ... delimiters ... */,
    Whitespace, Newline, LineComment,
    InvalidByte, UnterminatedString, UnterminatedTemplate, /* ... errors ... */,
    Eof,
}
```

### Layer 2: Cooking

The cooking layer (`ori_lexer::TokenCooker`) transforms `(RawTag, len)` pairs into `TokenKind` values by:

1. **Keyword resolution** — Identifiers are checked against reserved keywords (length-bucketed lookup), soft keywords (with lookahead), and reserved-future keywords (emit error + continue)
2. **String interning** — Identifiers and string content are interned via `StringInterner` to `Name` indices
3. **Escape processing** — String, char, and template escapes are resolved per-spec (separate rules for each context)
4. **Numeric parsing** — Integers (decimal, hex, binary), floats, durations, and sizes are parsed with overflow detection
5. **Error accumulation** — Errors are pushed to a `Vec<LexError>` and scanning continues

```
RawTag::Ident + "cache" → keyword lookup → TokenKind::Cache (if followed by `(`)
RawTag::Ident + "cache" → keyword lookup → TokenKind::Ident(Name) (otherwise)
RawTag::String + slice   → unescape → intern → TokenKind::String(Name)
RawTag::Int + "1_000"    → parse → TokenKind::Int(1000)
```

### Driver Loop

The driver loop in `lex()` / `lex_with_comments()` orchestrates the scanner and cooker:

1. Call `scanner.next_token()` → `(RawTag, len)`
2. For trivia (whitespace, comments): update pending flags, skip token (or classify comment)
3. For content tokens: call `cooker.cook(tag, offset, len)` → `TokenKind`
4. Calculate span from `offset` and `len`
5. Push token + flags to `TokenList`
6. Reset pending flags

## Entry Points

### Fast Path — `lex()`

```rust
pub fn lex(source: &str, interner: &StringInterner) -> TokenList
```

Returns only the token stream. Used by the parser. Skips comment classification, blank line tracking, and `IS_DOC` flag computation.

### Full Path — `lex_with_comments()`

```rust
pub fn lex_with_comments(source: &str, interner: &StringInterner) -> LexOutput
```

Returns the complete `LexOutput` structure. Used by the formatter, LSP, and other tools that need metadata:

```rust
pub struct LexOutput {
    pub tokens: TokenList,
    pub comments: CommentList,
    pub blank_lines: Vec<u32>,
    pub newlines: Vec<u32>,
    pub errors: Vec<LexError>,
    pub warnings: Vec<DetachedDocWarning>,
}
```

## Keyword System

### Reserved Keywords

Reserved keywords are resolved via a length-bucketed lookup table. The cooker first filters by identifier length and ASCII start byte, then performs a direct string match within the appropriate bucket. This avoids hash table overhead for the common case.

```
"let" → length 3 bucket → match → TokenKind::Let
"letter" → length 6 bucket → no match → TokenKind::Ident(Name)
```

### Soft Keywords

Six pattern keywords are context-sensitive — they are only recognized as keywords when immediately followed by `(`:

```
cache, catch, parallel, spawn, recurse, timeout
```

Resolution uses a three-stage filter:
1. **Length + first byte** — Eliminates >99% of identifiers before any string comparison
2. **String match** — Checks against the 6 known soft keywords
3. **Lookahead** — Skips horizontal whitespace (` `, `\t`) but NOT newlines, checks for `(`

Resolved soft keywords have the `CONTEXTUAL_KW` flag set in `TokenFlags`.

### Reserved-Future Keywords

Five identifiers are reserved for future language features:

```
asm, inline, static, union, view
```

These produce a `ReservedFutureKeyword` error but lex as identifiers (with `HAS_ERROR` flag) to allow parser recovery.

## Escape Processing

Escape rules are **spec-strict** and differ by context:

| Context | Valid Escapes | Invalid |
|---------|--------------|---------|
| String (`"..."`) | `\"` `\\` `\n` `\t` `\r` `\0` | `\'` (specific error) |
| Char (`'...'`) | `\'` `\\` `\n` `\t` `\r` `\0` | `\"` (specific error) |
| Template (`` `...` ``) | `` \` `` `\\` `\n` `\t` `\r` `\0` `{{` `}}` | — |

Each context has a dedicated unescape function. A **fast path** detects when no escapes are present (no backslash in strings/chars, no backslash or consecutive braces in templates) and directly interns the source slice, avoiding allocation.

## Template Literal Handling

Template literals use backtick delimiters and support interpolation with `{expr}` syntax:

```ori
`hello {name}, you have {count} messages`
```

### Token Production

The scanner produces four template token types:

```
`text{expr}more{expr2}end`

TemplateHead("text")          // ` to first {
  ... expression tokens ...   // normal tokens for expr
TemplateMiddle("more")        // } to next {
  ... expression tokens ...   // normal tokens for expr2
TemplateTail("end")           // } to closing `

`no interpolation`
TemplateComplete("no interpolation")  // ` to ` with no { }
```

### Depth Tracking

The raw scanner maintains a stack of `InterpolationDepth` structs to handle nested constructs within interpolations:

```rust
struct InterpolationDepth {
    paren_depth: u16,     // tracks ( )
    bracket_depth: u16,   // tracks [ ]
    brace_depth: u16,     // tracks { }
    seen_colon: bool,     // for format spec detection
}
```

This enables nested expressions like `{map[key]}` or `{fn(a, b)}` within templates. A `}` only closes an interpolation when all nested depths are zero.

### Format Specs

When a `:` appears at the top level of an interpolation (all nested depths zero), it begins a format spec:

```ori
`value: {x:>10.2f}`
//         ^^^^^^ FormatSpec token
```

The format spec content is emitted as a `FormatSpec(Name)` token with the interned spec string.

### Nested Templates

Templates can be nested within interpolation expressions:

```ori
`outer {`inner {x}`} end`
```

The depth stack handles this naturally — entering a new backtick pushes a new `InterpolationDepth`.

## Special Literals

### Duration Literals

```
100ns  → Duration(100, Nanoseconds)     1.5s  → Duration(1_500_000_000, Nanoseconds)
50us   → Duration(50, Microseconds)     0.5m  → Duration(30, Seconds)
100ms  → Duration(100, Milliseconds)
5s     → Duration(5, Seconds)
2m     → Duration(2, Minutes)
1h     → Duration(1, Hours)
```

Decimal durations (e.g. `1.5s`) are converted to the largest base unit that represents the value exactly using integer arithmetic. If the value cannot be represented exactly, a `DecimalNotRepresentable` error is emitted.

### Size Literals

```
1024b  → Size(1024, Bytes)              1.5kb → Size(1500, Bytes)
4kb    → Size(4, Kilobytes)
10mb   → Size(10, Megabytes)
2gb    → Size(2, Gigabytes)
1tb    → Size(1, Terabytes)
```

Decimal sizes follow the same exact-representation rule as durations.

### Integer Formats

```
42       → Int(42)           // decimal
1_000    → Int(1000)         // underscores for readability
0xFF     → Int(255)          // hexadecimal
0b1010   → Int(10)           // binary
```

Overflow is detected and produces an error token with `HAS_ERROR` flag.

## TokenList Structure

`TokenList` uses parallel arrays for cache-friendly access patterns:

```rust
pub struct TokenList {
    tokens: Vec<Token>,         // Token = { kind: TokenKind, span: Span }
    tags: Vec<u8>,              // discriminant tag per token (for fast kind checks)
    flags: Vec<TokenFlags>,     // metadata per token
}
```

The `tags` array stores `TokenKind` discriminant indices, enabling O(1) kind checks without pattern matching on the full enum. The `flags` array stores per-token metadata (see [Token Design — TokenFlags](token-design.md#tokenflags)).

### Salsa Early Cutoff

`TokenList` has custom `Eq`/`Hash` implementations that compare only `TokenKind` values and `TokenFlags`, ignoring `Span` positions and `tags` (which are derived from kinds). This enables Salsa early cutoff: whitespace-only edits shift token positions but produce equal `TokenList` values, so downstream queries (parsing, type checking) skip re-execution.

## Newline Handling

Newlines are significant tokens used by the parser for statement separation:

- `\n` produces a `TokenKind::Newline` token
- Line continuation (`\` at end of line) suppresses the newline token
- Horizontal whitespace (spaces, tabs, `\r`) is consumed and recorded in `TokenFlags`

```rust
// Input: "let x = 42\nlet y = 10"
// Output: [Let, Ident(x), Eq, Int(42), Newline, Let, Ident(y), Eq, Int(10), Eof]
```

## Error Handling

The lexer accumulates errors without bailing, producing `TokenKind::Error` tokens with `HAS_ERROR` flag so the parser can attempt recovery.

### Error Shape (WHERE+WHAT+WHY+HOW)

```rust
pub struct LexError {
    pub span: Span,                       // WHERE in source
    pub kind: LexErrorKind,               // WHAT went wrong
    pub context: LexErrorContext,          // WHY (surrounding context)
    pub suggestions: Vec<LexSuggestion>,  // HOW to fix it
}
```

### Cross-Language Detection

The lexer recognizes common patterns from other languages and provides targeted suggestions:

| Pattern | Detection | Suggestion |
|---------|-----------|------------|
| `;` | Semicolons | "Ori uses newlines for statement separation" |
| `===` | Triple equals | "Use `==` for equality" |
| `++` / `--` | Increment/decrement | "Use `x += 1` / `x -= 1`" |
| `? :` | Ternary operator | "Use `if ... then ... else ...`" |
| Unicode confusables | Full-width chars, etc. | "Did you mean ASCII `X`?" |

### Detached Doc Comment Warning

The `lex_with_comments()` path detects doc comments that are separated from their declaration by a blank line, emitting `DetachedDocWarning` to catch documentation mistakes.

## Greater-Than Token Handling

The lexer produces individual `>` tokens to enable nested generic parsing:

```ori
type Nested = Result<Result<int, str>, str>
//                                    ^^--- Two separate > tokens
```

| Operator | Lexer Output | Parser Handling |
|----------|--------------|-----------------|
| `>` | Single `Gt` token | Used as-is |
| `>>` | Two adjacent `Gt` tokens | Synthesized in expression context |
| `>=` | `Gt` + `Eq` tokens | Synthesized in expression context |

The parser checks token adjacency (span endpoints touch) to combine tokens when needed.

## Performance

### Pre-allocation

Buffer sizes are estimated from source length to minimize reallocation:

```rust
tokens: source_len / 2 + 1      // ~1 token per 2-3 bytes
blank_lines: source_len / 400    // ~1 per 10 lines
newlines: source_len / 40        // ~1 per line
```

### SIMD Scanning

The raw scanner uses 16-byte SIMD chunks to find string and template delimiters:
- `skip_to_string_delim()` — finds `"`, `\`, `\r`, `\n`, or NUL
- `skip_to_template_delim()` — finds `` ` ``, `{`, `}`, `\`, or NUL

Falls back to byte-by-byte scanning when fewer than 16 bytes remain.

### Inline Strategy

All cross-crate hot functions are marked `#[inline]` to let the compiler optimize call chains across crate boundaries. This includes the cooker's cooking helpers, keyword pre-filters, span construction, and source slicing functions.

## Salsa Integration

Two Salsa queries expose the lexer:

```rust
// Fast path for parsing — returns only tokens
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let source = file.text(db);
    ori_lexer::lex(db.interner(), &source)
}

// Full path for formatter/IDE — returns tokens + metadata
#[salsa::tracked]
pub fn tokens_with_metadata(db: &dyn Db, file: SourceFile) -> LexOutput {
    let source = file.text(db);
    ori_lexer::lex_with_comments(db.interner(), &source)
}
```

This enables:
- **Caching** — Token results are memoized across queries
- **Early cutoff** — If token kinds and flags are unchanged, downstream queries skip re-execution
- **Dependency tracking** — Changes to source text automatically invalidate token queries

## Related Documents

- [Token Design](token-design.md) — Token types, flags, and metadata
- [Architecture: Pipeline](../01-architecture/pipeline.md) — Pipeline overview

# Lexer Patterns & Implementation

Quick-reference guide to lexer/scanner design and implementation patterns.

---

## Token Classification

### Core Token Categories
- **Keywords**: Reserved words (`if`, `while`, `fn`, `let`, `return`)
- **Identifiers**: User-defined names (`foo`, `myVar`, `_private`)
- **Literals**: Values in source (`42`, `3.14`, `"hello"`, `'c'`, `true`)
- **Operators**: Computation symbols (`+`, `-`, `==`, `&&`, `->`)
- **Delimiters**: Structural punctuation (`(`, `)`, `{`, `}`, `,`, `;`)
- **Comments**: Documentation/notes (not passed to parser)
- **Whitespace**: Spaces, tabs, newlines (usually discarded)

### Token Data Structure
```rust
// Minimal token (Rust rustc_lexer style)
struct Token {
    kind: TokenKind,
    len: u32,           // byte length in source
}

// Rich token (with span info)
struct Token {
    kind: TokenKind,
    lexeme: String,     // raw text
    literal: Value,     // parsed value for literals
    line: u32,
    column: u32,
}

// With full span
struct Token {
    kind: TokenKind,
    span: Span,         // start..end byte offsets
}
```

### Literal Sub-types
- **Integers**: decimal, hex (`0x`), octal (`0o`), binary (`0b`)
- **Floats**: decimal with `.` and/or exponent (`1.5`, `1e10`, `1.5e-3`)
- **Strings**: regular, raw, byte strings
- **Characters**: single char with escape support
- **Booleans**: `true`, `false`

---

## Lexer Architecture Patterns

### Hand-Written Lexer (Recommended)
- Full control over error messages and recovery
- Can handle context-sensitive lexing
- Easier to debug and modify
- Used by: Rust, Go, TypeScript, most production compilers

### Generated Lexer
- Lex/Flex style generators
- Define patterns in DSL, generate code
- Good for simple languages, harder to customize errors
- Used by: older compilers, academic projects

### Derive Macro (Logos-style)
```rust
#[derive(Logos)]
enum Token {
    #[token("fn")]
    Fn,
    #[token("+")]
    Plus,
    #[regex("[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
    #[regex("[0-9]+", |lex| lex.slice().parse())]
    Number(i64),
}
```
- Compile-time DFA generation
- Regex patterns on enum variants
- Callbacks for value extraction
- Good balance of convenience and speed

---

## Two-Stage Lexing (Rust Pattern)

### Stage 1: Raw Lexer (`rustc_lexer`)
- Operates on `&str` directly
- Produces simple tokens: `(TokenKind, length)`
- No error reporting, just flags on tokens
- No span tracking, no string interning
- Reusable library (IDE, tools, etc.)

### Stage 2: Token Stream (`rustc_parse::lexer`)
- Converts raw tokens to rich tokens
- Adds spans, interned strings
- Reports errors to handler
- Handles context-sensitive features
- Edition-aware keyword handling

### Benefits of Two-Stage
- Clean separation of concerns
- Raw lexer can be embedded anywhere
- Error handling customizable per use case
- Raw lexer remains simple and fast

---

## Cursor/Source Abstraction

### Cursor Pattern (Rust)
```rust
struct Cursor<'a> {
    chars: Chars<'a>,       // Iterator over input
    len_remaining: usize,   // For position tracking
}

impl Cursor {
    fn first(&self) -> char;     // Peek next char
    fn second(&self) -> char;    // Peek 2nd char
    fn bump(&mut self) -> Option<char>;  // Consume char
    fn eat_while(&mut self, pred: impl Fn(char) -> bool);
    fn is_eof(&self) -> bool;
}
```

### Source Pattern (Go)
```go
type source struct {
    buf []byte          // source buffer
    ch  rune            // current character
    pos int             // current position
    line, col uint      // for error reporting
}

func (s *source) nextch()              // advance
func (s *source) pos() (line, col)     // position
func (s *source) segment() []byte      // current token text
```

### Key Methods
- `peek()` / `first()`: Look at next char without consuming
- `advance()` / `bump()` / `nextch()`: Consume and return char
- `match(expected)`: Consume if matches, return bool
- `eat_while(predicate)`: Consume while predicate true
- `eat_until(char)`: Consume until finding char

---

## Span/Location Tracking

### Byte Offsets (Recommended)
```rust
struct Span {
    start: u32,  // byte offset from file start
    end: u32,    // exclusive end
}
```
- Compact (8 bytes)
- Easy arithmetic
- Line/column computed on demand from source

### Line/Column
```rust
struct Location {
    line: u32,    // 1-based
    column: u32,  // 1-based, in characters or bytes
}
```
- Human-readable
- Expensive to track continuously
- Column definition varies (chars vs bytes vs graphemes)

### Hybrid Approach
- Store byte spans on tokens
- Maintain line offset table separately
- Compute line/column when needed for errors
- `LineOffsets: Vec<u32>` - byte offset where each line starts

---

## Whitespace Handling

### Insignificant Whitespace
- Consume and discard spaces, tabs, newlines
- Track newlines for line counting only
- Most languages (C, Java, Rust, Go)

### Significant Whitespace
- Indentation determines blocks (Python, Haskell)
- Emit INDENT/DEDENT tokens
- Track indentation stack
- Newlines may be significant

### Automatic Semicolon Insertion (Go)
```go
// Insert semicolon after these tokens if followed by newline:
nlsemi = tok in [_Name, _Literal, _Break, _Continue,
                 _Fallthrough, _Return, _Rparen, _Rbrack, _Rbrace]
```
- Track `nlsemi` flag on scanner
- `\n` → `;` token when flag set
- Simplifies grammar, enables brace-on-same-line rule

---

## Line Continuation

### Backslash Continuation (Python, shell)
```python
if a > 0 and \
   b > 0:
```
- Backslash before newline continues line
- Lexer consumes `\` and newline, continues

### Natural Continuation (Ori)
```
if a > 0
   && b > 0
then result
```
- Lines continue naturally after binary operators
- No explicit continuation character needed
- Parentheses can also be used for grouping multi-line expressions

### Implicit Continuation
- Inside `()`, `[]`, `{}` newlines are insignificant
- Track bracket depth, ignore newlines when > 0
- Python, JavaScript (in some contexts)

---

## String Literal Handling

### Standard Strings
```rust
fn double_quoted_string(&mut self) -> bool {
    while let Some(c) = self.bump() {
        match c {
            '"' => return true,           // terminated
            '\\' => { self.bump(); }      // skip escaped char
            _ => ()
        }
    }
    false  // unterminated
}
```

### Escape Sequences
| Escape | Meaning |
|--------|---------|
| `\\` | Backslash |
| `\"` | Quote |
| `\n` | Newline |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | Null |
| `\xNN` | Hex byte |
| `\uNNNN` | Unicode (4 hex) |
| `\U00NNNNNN` | Unicode (8 hex) |

### Raw Strings (No escapes)
```rust
// Rust: r"no \n escapes" or r#"can include " quotes"#
// Go: `raw string with
//      newlines preserved`
// Python: r"raw \n string"
```

### String Interpolation
- Lex string segments between `${...}` or `{...}`
- Option 1: Lexer handles, emits string parts + expressions
- Option 2: Lexer emits whole string, parser re-lexes
- Option 3: Treat as special syntax, not in lexer

### Multi-line Strings
- Track newlines inside string for line counting
- Decide: preserve literal newlines or require escapes?
- Triple-quoted strings (Python): `"""multi-line"""`

---

## Number Literal Patterns

### Integer Bases
```rust
fn number(&mut self, first_digit: char) -> LiteralKind {
    if first_digit == '0' {
        match self.first() {
            'b' => { self.bump(); self.eat_binary_digits(); }
            'o' => { self.bump(); self.eat_octal_digits(); }
            'x' => { self.bump(); self.eat_hex_digits(); }
            _ => self.eat_decimal_digits()
        }
    }
}
```

### Floating Point
```
[0-9]+ '.' [0-9]* ([eE] [+-]? [0-9]+)?
[0-9]* '.' [0-9]+ ([eE] [+-]? [0-9]+)?
[0-9]+ [eE] [+-]? [0-9]+
```
- Avoid leading/trailing `.` ambiguity with method calls
- `1.foo()` - is `.` part of number or method call?
- Solution: require digit after `.` for floats

### Digit Separators
```rust
let million = 1_000_000;
let hex = 0xFF_FF_FF;
```
- Allow `_` between digits for readability
- Validate: no leading/trailing `_`, no double `__`

### Suffixes
```rust
let x = 42u64;
let y = 3.14f32;
```
- Literal suffixes for type annotation
- Lexer captures suffix, type checker validates

---

## Comment Handling

### Line Comments
```rust
fn line_comment(&mut self) -> TokenKind {
    self.eat_until(b'\n');
    LineComment { doc_style: self.detect_doc_style() }
}
```

### Block Comments
```rust
fn block_comment(&mut self) -> TokenKind {
    let mut depth = 1;  // Support nesting!
    while let Some(c) = self.bump() {
        match c {
            '/' if self.first() == '*' => { self.bump(); depth += 1; }
            '*' if self.first() == '/' => { self.bump(); depth -= 1; }
            _ => ()
        }
        if depth == 0 { break; }
    }
    BlockComment { terminated: depth == 0 }
}
```

### Doc Comments
- `///` → outer doc (documents following item)
- `//!` → inner doc (documents enclosing item)
- `/** */` and `/*! */` for block doc comments
- Preserve for AST, not just discard

---

## Lookahead Strategies

### Single Lookahead (LL(1))
- Most common, sufficient for most languages
- `peek()` to see next char without consuming
- Covers: most operators, keywords, identifiers

### Double Lookahead
- Needed for: float vs int (`.` followed by digit?), `..` vs `.`
- `peek_next()` or `second()` to see char after next

### Maximal Munch
- Match longest possible token
- `===` before `==` before `=`
- Order matters in switch/match

### Rewind Strategy (Go)
```go
// After seeing '.', check if float or dot
if s.ch == '.' {
    s.nextch()
    if isDecimal(s.ch) {
        // float: continue number
    } else if s.ch == '.' {
        // might be '..'
        s.rewind()
    }
}
```

---

## Keyword Recognition

### Hash Table Lookup
```go
// Perfect hash for keywords
func hash(s []byte) uint {
    return (uint(s[0])<<4 ^ uint(s[1]) + uint(len(s))) & mask
}
var keywordMap [64]token
```

### Trie/State Machine
- DFA generated from keyword list
- Logos crate builds this automatically

### Simple Map After Identifier
```rust
fn ident_or_keyword(&mut self) {
    self.eat_identifier();
    let text = self.current_text();
    match KEYWORDS.get(text) {
        Some(tok) => tok,
        None => Identifier
    }
}
```

### Context-Sensitive Keywords
- Some words are keywords only in specific contexts
- Example: `async`, `await` only in async context
- Solution: lex as identifier, parser promotes to keyword

---

## Error Recovery

### Skip and Continue
- Report error, consume bad character, continue lexing
- Allows multiple errors per file
- Better UX than stopping at first error

### Unterminated Constructs
```rust
// Unterminated string
fn string(&mut self) -> TokenKind {
    while !self.is_eof() {
        if self.first() == '"' { ... }
        if self.first() == '\n' {
            return Literal { terminated: false };  // Error flag
        }
        self.bump();
    }
    Literal { terminated: false }  // EOF before close
}
```

### Invalid Characters
```rust
match first_char {
    // ... valid cases ...
    _ => {
        self.error("invalid character");
        Unknown  // Continue scanning
    }
}
```

### Error Tokens
- Include `Unknown` or `Error` token type
- Parser can handle or skip error tokens
- Preserves source location for error reporting

---

## Performance Tips

### Batch Character Reading
- `memchr` for finding delimiters
- Read ahead for common patterns
- SIMD for whitespace skipping (advanced)

### Minimize Allocations
- Return slices into source, not copies
- Intern strings later if needed
- Token length + source offset, compute text on demand

### Branch Prediction
- Put common cases first in match/switch
- Separate fast path (ASCII) from slow path (Unicode)
- Use lookup tables for character classification

### Go Scanner Optimization
```go
// Fast path: 7-bit ASCII identifiers
for isLetter(s.ch) || isDecimal(s.ch) {
    s.nextch()  // tight loop
}
// Slow path: Unicode
if s.ch >= utf8.RuneSelf {
    for s.atIdentChar(false) { s.nextch() }
}
```

---

## Real-World Examples

### Rust (`rustc_lexer/src/lib.rs`)
- Two-stage: raw lexer + parse-time tokenizer
- Cursor abstraction for char iteration
- Comprehensive literal support (raw strings, lifetimes)
- Pure library, no dependencies on compiler infrastructure

### Go (`cmd/compile/internal/syntax/scanner.go`)
- Single-stage, integrated with parser
- Automatic semicolon insertion (`nlsemi` flag)
- Perfect hash for keywords
- Source abstraction with segment tracking

### Logos Crate
- Derive macro generates DFA
- Regex and literal patterns
- Callbacks for value parsing
- Compile-time optimization

---

## Lexer Checklist

### Token Types
- [ ] All keywords defined
- [ ] All operators (single and multi-char)
- [ ] All delimiters
- [ ] Identifier pattern (start char, continue chars)
- [ ] Integer literals (bases, separators)
- [ ] Float literals (exponent, no ambiguity with methods)
- [ ] String literals (escapes, raw, multi-line?)
- [ ] Char literals (escapes)
- [ ] Comments (line, block, doc?)

### Infrastructure
- [ ] Position tracking (byte offset or line/col)
- [ ] Lookahead methods (1 or 2 chars)
- [ ] Error reporting with location
- [ ] Error recovery (continue after bad input)
- [ ] Whitespace handling (significant or not?)
- [ ] Line continuation support?

### Testing
- [ ] Each token type individually
- [ ] Edge cases (empty strings, zero, max values)
- [ ] Error cases (unterminated, invalid chars)
- [ ] Unicode identifiers (if supported)
- [ ] Fuzzing for robustness

---

## Key References
- Rust rustc_lexer: `compiler/rustc_lexer/src/lib.rs`
- Go scanner: `src/cmd/compile/internal/syntax/scanner.go`
- Crafting Interpreters (Scanning): https://craftinginterpreters.com/scanning.html
- Logos crate: https://github.com/maciejhirsz/logos

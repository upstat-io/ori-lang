---
section: "02"
title: Raw Scanner
status: not-started
goal: "Hand-written state-machine scanner replacing logos, producing (RawTag, length) pairs with zero allocation"
sections:
  - id: "02.1"
    title: RawTag Enum
    status: not-started
  - id: "02.2"
    title: Scanner State Machine
    status: not-started
  - id: "02.3"
    title: Operator & Punctuation Scanning
    status: not-started
  - id: "02.4"
    title: Identifier Scanning
    status: not-started
  - id: "02.5"
    title: String & Char Literal Scanning
    status: not-started
  - id: "02.6"
    title: Template Literal Scanning
    status: not-started
  - id: "02.7"
    title: Numeric Literal Scanning
    status: not-started
  - id: "02.8"
    title: Comment & Whitespace Scanning
    status: not-started
  - id: "02.9"
    title: Newline Handling
    status: not-started
  - id: "02.10"
    title: Tests & Verification
    status: not-started
---

# Section 02: Raw Scanner

**Status:** :clipboard: Planned
**Goal:** Replace the logos-generated DFA with a hand-written state-machine scanner that produces `(RawTag, length)` pairs with zero heap allocation. The scanner operates on the sentinel-terminated `Cursor` from Section 01 and lives in `ori_lexer_core` (zero `ori_*` dependencies).

> **REFERENCE**: Rust's `rustc_lexer` (pure scanner producing `(kind, len)` with zero deps); Zig's labeled-switch state machine (compiled to computed goto); Go's `source.nextch()` + scanner dispatch; TypeScript's template literal four-token strategy.
>
> **CONVENTIONS**: v2-conventions SS2 (Tag/Discriminant Enums), SS10 (Two-Layer Crate Pattern). `RawTag` is defined in `ori_lexer_core` with no `ori_*` deps. The integration layer (`ori_lexer`) maps `RawTag` -> `ori_ir::TokenTag`.

---

## Design Rationale

### Why Replace Logos?

1. **Control over hot paths**: Logos generates a DFA we cannot tune. Hand-written code lets us insert SWAR acceleration, `memchr` calls, and sentinel-aware loops exactly where they matter.

2. **Eliminate the RawToken->TokenKind duplication**: Currently, `RawToken` (88 variants, logos-derived) maps near-1:1 to `TokenKind` (116 variants, in `ori_ir`) via a 183-line match. With a hand-written scanner, we produce a single `RawTag` enum that maps directly to `TokenTag` discriminants, eliminating the conversion layer.

3. **Deferred validation**: Logos must validate literals during regex matching (e.g., parsing integers in callbacks). A hand-written scanner can greedily consume literal characters and defer validation to the cooking layer (Zig pattern), keeping the hot loop simpler.

4. **Better error recovery**: Logos produces `Err(())` on unrecognized input with no context. A hand-written scanner can record the problematic byte, attempt recovery, and provide rich error context.

5. **Template literals**: Backtick-delimited template strings with `{expr}` interpolation require stack-based nesting that is not expressible in logos regex patterns. A hand-written scanner handles this naturally.

### Architecture

The raw scanner is a **pure function**: given a `Cursor`, it produces the next `(RawTag, length)` pair. It has no side effects, no allocation, no error reporting. Error conditions are encoded as `RawTag` variants (e.g., `RawTag::InvalidByte`, `RawTag::UnterminatedString`).

The scanner lives in `ori_lexer_core` and has zero `ori_*` dependencies (v2-conventions SS10).

```rust
/// Pure, allocation-free scanner.
///
/// Produces one token at a time as a (tag, length) pair.
/// The tag is a byte-sized discriminant. The length is the
/// number of source bytes consumed. Error conditions are
/// encoded as tag variants, not as Result::Err.
///
/// Lives in `ori_lexer_core` -- no `ori_*` dependencies.
pub struct RawScanner<'a> {
    cursor: Cursor<'a>,
    /// Stack for template literal nesting.
    /// Tracks brace depth to distinguish `}` closing an interpolation
    /// from `}` closing a normal block inside an interpolation.
    template_depth: Vec<u32>,
}

impl<'a> RawScanner<'a> {
    pub fn next(&mut self) -> RawToken {
        // ... state machine dispatch ...
    }
}

pub struct RawToken {
    pub tag: RawTag,
    pub len: u32,
}
```

---

## 02.1 RawTag Enum

`RawTag` is defined in `ori_lexer_core` with `#[repr(u8)]` and semantic ranges with gaps for future variants (v2-conventions SS2). It has no `ori_*` dependencies. The integration layer (`ori_lexer`) maps `RawTag` -> `ori_ir::TokenTag` at the crate boundary (v2-conventions SS10).

- [ ] Define `RawTag` as a `#[non_exhaustive] #[repr(u8)]` enum with semantic ranges:
  ```rust
  /// Raw token kind -- lightweight, standalone (no ori_* dependencies).
  /// Mapped to `ori_ir::TokenTag` in the integration layer (`ori_lexer`).
  /// See plans/v2-conventions.md §2 (Tag Enums), §10 (Two-Layer Pattern).
  ///
  /// NOTE on intentional removals from current RawToken:
  ///  - `BinInt` removed: grammar.ebnf lines 91-93 specifies `int_literal = decimal_lit | hex_lit` only.
  ///    Binary integer literals (`0b...`) are not part of the Ori spec.
  ///  - `LineContinuation` removed: backslash line continuation (`\<newline>`) is not
  ///    in the spec. The parser handles implicit continuation across newlines when
  ///    the preceding token is an operator or delimiter.
  ///  - Float duration/size suffixes: **CRITICAL CHANGE** — grammar.ebnf lines 136, 143 show
  ///    `decimal_duration` and `decimal_size` ARE valid (e.g., `0.5s`, `1.5kb`). These are NOT
  ///    errors. The current RawToken implementation (lines 271-292) incorrectly treats these as
  ///    errors (FloatDurationNs, FloatSizeKb, etc.). The grammar explicitly states these are
  ///    "compile-time sugar computed via integer arithmetic". The raw scanner MUST produce
  ///    `Duration` or `Size` tags with the appropriate unit; the cooking layer validates and
  ///    converts decimal values to integer nanoseconds/bytes via compile-time arithmetic.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  #[non_exhaustive]
  #[repr(u8)]
  pub enum RawTag {
      // === Identifiers & Literals (0-15) ===
      Ident = 0,
      Int = 1,
      Float = 2,
      HexInt = 3,
      String = 4,
      Char = 5,
      Duration = 6,
      Size = 7,

      // === Template Literals (16-19) ===
      TemplateHead = 16,       // ` ... {
      TemplateMiddle = 17,     // } ... {
      TemplateTail = 18,       // } ... `
      TemplateComplete = 19,   // ` ... ` (no interpolation)

      // === Operators (32-79) ===
      Plus = 32,
      Minus = 33,
      Star = 34,
      Slash = 35,
      Percent = 36,
      Caret = 37,
      Ampersand = 38,
      Pipe = 39,
      Tilde = 40,
      Bang = 41,
      Equal = 42,
      Less = 43,
      Greater = 44,
      Dot = 45,
      Question = 46,
      // Compound operators
      // NOTE: No compound assignment operators (+=, -=, *=, /=).
      // The Ori grammar has NO compound assignment; `=` is the only
      // assignment operator.
      EqualEqual = 48,
      BangEqual = 49,
      LessEqual = 50,
      // Note: no GreaterEqual -- parser synthesizes from adjacent > =
      // Note: no ShiftRight (>>) -- parser synthesizes from adjacent > >
      AmpersandAmpersand = 52,
      PipePipe = 53,
      Arrow = 54,          // ->
      FatArrow = 55,       // =>
      DotDot = 56,         // ..
      DotDotEqual = 57,    // ..=
      DotDotDot = 58,      // ...
      ColonColon = 59,     // ::
      Shl = 60,            // <<
      QuestionQuestion = 61, // ??
      // NOTE: No PipeRight (|>). The pipe-right operator is not in the
      // Ori grammar. `|` is used only as bitwise OR and pattern alternation.

      // === Delimiters (80-95) ===
      LeftParen = 80,
      RightParen = 81,
      LeftBracket = 82,
      RightBracket = 83,
      LeftBrace = 84,
      RightBrace = 85,
      Comma = 86,
      Colon = 87,
      Semicolon = 88,      // Error-detection token: semicolons are not valid Ori
                            // syntax. Emitted so the cooker can produce a helpful
                            // diagnostic for users coming from C/Rust/JS/etc.
      At = 89,             // @
      Hash = 90,           // #
      Underscore = 91,
      Backslash = 92,      // Error-detection token: backslash is only valid inside
                            // escape sequences within string/char/template literals.
                            // A standalone backslash is always an error. Emitted so
                            // the cooker can produce a targeted diagnostic.
      Dollar = 93,         // $
      HashBracket = 94,    // #[  (attribute prefix; grammar uses `"#" identifier` on line 198,
                            //      but the implementation lexes `#[` as a single token)
      HashBang = 95,       // #!  (file attribute prefix, grammar.ebnf line 156)

      // === Trivia (112-119) ===
      Whitespace = 112,
      Newline = 113,
      LineComment = 114,

      // === Errors (240-254) ===
      InvalidByte = 240,
      UnterminatedString = 241,
      UnterminatedChar = 242,
      InvalidEscape = 243,
      UnterminatedTemplate = 244,

      // === Control (255) ===
      Eof = 255,
  }
  ```
- [ ] Ensure `RawTag` fits in a single byte (`u8`) -- must have <= 256 variants
- [ ] Implement `RawTag::name(&self) -> &'static str` for display/debugging (v2-conventions SS2)
- [ ] Implement `RawTag::lexeme(&self) -> Option<&'static str>` for fixed-length tokens
- [ ] Add `#[cold]` attribute hint for error variant construction paths
- [ ] Size assertion: `const _: () = assert!(size_of::<RawTag>() == 1);`

---

## 02.2 Scanner State Machine

- [ ] Implement the main dispatch in `RawScanner::next()`:
  ```rust
  pub fn next(&mut self) -> RawToken {
      let start = self.cursor.pos();

      // If we are inside a template interpolation and hit `}` at depth 0,
      // resume template scanning (see 02.6).
      if self.in_template_interpolation() && self.cursor.current() == b'}' {
          return self.template_middle_or_tail(start);
      }

      match self.cursor.current() {
          0 => self.eof(),
          b' ' | b'\t' | b'\r' => self.whitespace(),
          b'\n' => self.newline(),
          b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.identifier(),
          b'0'..=b'9' => self.number(),
          b'"' => self.string(),
          b'\'' => self.char_literal(),
          b'`' => self.template_literal(start),
          b'/' => self.slash_or_comment(),
          b'+' | b'-' | b'*' | b'%' | b'^' | b'~' => self.operator(start),
          b'=' | b'!' | b'<' => self.comparison_or_assign(start),
          b'>' => self.greater(start),
          b'.' => self.dot(),
          b'(' | b')' | b'[' | b']' => self.delimiter(),
          b'{' => self.left_brace(start),
          b'}' => self.right_brace(start),
          b':' => self.colon(),
          b',' | b';' | b'@' | b'$' => self.single_char_token(),
          b'#' => self.hash(),
          b'?' => self.question(),
          b'|' => self.pipe(),
          b'&' => self.ampersand(),
          b'\\' => self.backslash(),
          1..=31 => self.invalid_control_char(),
          128..=255 => self.invalid_byte(),
          _ => self.invalid_byte(),
      }
  }
  ```
- [ ] Each arm calls a focused method that advances the cursor and returns `RawToken { tag, len }`
- [ ] The sentinel byte (0x00) naturally dispatches to `self.eof()`
- [ ] Verify the dispatch covers all 256 byte values exhaustively (use a `const` assertion or test)

**Note on Unicode identifiers:** Grammar.ebnf line 29 defines `letter = 'A' … 'Z' | 'a' … 'z'` only (ASCII letters). Grammar.ebnf line 52 defines `identifier = ( letter | "_" ) { letter | digit | "_" }`. Bytes 128-255 (non-ASCII) are invalid identifier starts and produce `InvalidByte`. There is no XID/Unicode identifier support.

---

## 02.3 Operator & Punctuation Scanning

Grammar.ebnf lines 72-77 (operators):
- `arith_op = "+" | "-" | "*" | "/" | "%" | "div"`
- `comp_op = "==" | "!=" | "<" | ">" | "<=" | ">="`
- `logic_op = "&&" | "||" | "!"`
- `bit_op = "&" | "|" | "^" | "~" | "<<" | ">>"`
- `other_op = ".." | "..=" | "??" | "?" | "->" | "=>"`

Grammar.ebnf lines 81-82 (delimiters):
- `delimiter = "(" | ")" | "[" | "]" | "{" | "}" | "," | ":" | "." | "@" | "$"`

NOTE: The grammar does NOT specify compound assignment operators (`+=`, `-=`, `*=`, `/=`, etc.). The `=` operator is the only assignment operator.

NOTE: The grammar does NOT specify a pipe-right operator (`|>`). The `|` is used only as bitwise OR (grammar.ebnf line 75) and pattern alternation (grammar.ebnf line 525).

- [ ] Implement compound operator recognition with direct lookahead:
  - `+` -> `Plus` (no compound assignment in Ori)
  - `-` -> check `->` -> `Minus`, `Arrow`
  - `*` -> `Star` (no compound assignment in Ori)
  - `=` -> check `==`, `=>` -> `Equal`, `EqualEqual`, `FatArrow`
  - `!` -> check `!=` -> `Bang`, `BangEqual`
  - `<` -> check `<=`, `<<` -> `Less`, `LessEqual`, `Shl`
  - `>` -> **always single token** (parser synthesizes `>>`, `>=` from adjacent `>` tokens for generics support)
  - `|` -> check `||` -> `Pipe`, `PipePipe` (no `|>` pipe-right in Ori)
  - `&` -> check `&&` -> `Ampersand`, `AmpersandAmpersand`
  - `:` -> check `::` -> `Colon`, `ColonColon`
  - `.` -> check `..`, `..=`, `...` -> `Dot`, `DotDot`, `DotDotEqual`, `DotDotDot`
  - `?` -> check `??` -> `Question`, `QuestionQuestion`
  - `/` -> check `//` (comment) -> `Slash` or dispatch to `line_comment()`
  - `#` -> check `#[`, `#!` -> `Hash`, `HashBracket`, `HashBang`
- [ ] Each compound check uses `cursor.peek()` (single-byte lookahead into sentinel-safe buffer)
- [ ] Delimiter tokens (`(`, `)`, `[`, `]`) are single-byte, single-dispatch
- [ ] `{` and `}` have special handling for template literal nesting (02.6)
- [ ] `$` produces `Dollar` -- used as delimiter in const generics and binding patterns

**Note on `div`:** The `div` keyword operator (grammar.ebnf line 72) is scanned as `RawTag::Ident` (it is alphabetic). The cooking layer in `ori_lexer` resolves it to the appropriate keyword tag. The raw scanner does not distinguish `div` from any other identifier.

---

## 02.4 Identifier Scanning

- [ ] Fast ASCII loop for the common case:
  ```rust
  fn identifier(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // consume first char (already validated as ident start)
      // Fast ASCII continuation loop
      loop {
          match self.cursor.current() {
              b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' => {
                  self.cursor.advance();
              }
              _ => break,
          }
      }
      RawToken { tag: RawTag::Ident, len: self.cursor.pos() - start }
  }
  ```
- [ ] The identifier is NOT resolved to a keyword here -- that happens in the cooking layer (Section 03) via perfect hash (Section 06)
- [ ] No Unicode identifier support -- grammar.ebnf line 29 defines `letter = 'A' … 'Z' | 'a' … 'z'` only (ASCII letters). Non-ASCII bytes do not extend identifiers.

---

## 02.5 String & Char Literal Scanning

- [ ] String scanning (`"..."`) -- find boundaries only, defer escape validation:
  ```rust
  fn string(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // consume opening '"'
      loop {
          match self.cursor.current() {
              b'"' => {
                  self.cursor.advance(); // consume closing '"'
                  return RawToken { tag: RawTag::String, len: self.cursor.pos() - start };
              }
              b'\\' => {
                  self.cursor.advance(); // consume '\'
                  self.cursor.advance(); // consume escaped char (whatever it is)
              }
              b'\n' | b'\r' | 0 => {
                  // Unterminated string
                  return RawToken { tag: RawTag::UnterminatedString, len: self.cursor.pos() - start };
              }
              _ => self.cursor.advance(),
          }
      }
  }
  ```
- [ ] Note: The scanner does NOT validate escape sequences. It only skips `\X` pairs to avoid treating `\"` as a string terminator. The cooking layer (Section 03) validates that escapes are one of (grammar.ebnf line 102): `\"`, `\\`, `\n`, `\t`, `\r`, `\0`. No hex escapes (`\xHH`), no Unicode escapes (`\u{XXXX}`) -- these are not in the spec.
- [ ] Character literal scanning (`'X'`) -- similar boundary-finding approach. Spec escapes for char (grammar.ebnf line 127): `\'`, `\\`, `\n`, `\t`, `\r`, `\0`.
- [ ] Use `memchr` for fast scanning to `"` or `\` (Section 05 will optimize this)
- [ ] Regular `"..."` strings do NOT support interpolation. Only backtick-delimited template literals support `{expr}` interpolation.

---

## 02.6 Template Literal Scanning

Template literals use backtick delimiters (`` ` ``) with `{expr}` interpolation (bare braces, NOT `${}`). The scanner produces distinct token types for each segment, enabling the parser to handle interpolated expressions without re-scanning.

> **Grammar reference:** grammar.ebnf lines 105-109: `template_literal = '`' { template_char | template_escape | template_brace | interpolation } '`'`

### Token Types

| Token | Pattern | Meaning |
|-------|---------|---------|
| `TemplateComplete` | `` `hello world` `` | Template with no interpolation |
| `TemplateHead` | `` `hello {` `` | Opening backtick to first unescaped `{` |
| `TemplateMiddle` | `} middle {` | Between interpolations (closing `}` to next `{`) |
| `TemplateTail` | `` } end` `` | Last `}` to closing backtick |

### Stack-Based Nesting

Template interpolations can contain arbitrary expressions, including blocks with braces and even nested template literals. The scanner maintains a `template_depth: Vec<u32>` stack to track brace nesting:

- When scanning a `TemplateHead` or `TemplateMiddle`, push brace depth `0` onto the stack.
- When encountering `{` inside an interpolation, increment the top of the stack.
- When encountering `}` inside an interpolation:
  - If top of stack is `> 0`, decrement it (normal block close).
  - If top of stack is `0`, pop the stack and scan the template continuation (`TemplateMiddle` or `TemplateTail`).

### Tasks

- [ ] Implement `template_literal()` -- entered when scanner sees opening `` ` ``:
  ```rust
  fn template_literal(&mut self, start: u32) -> RawToken {
      self.cursor.advance(); // consume opening '`'
      loop {
          match self.cursor.current() {
              b'`' => {
                  self.cursor.advance(); // consume closing '`'
                  return RawToken {
                      tag: RawTag::TemplateComplete,
                      len: self.cursor.pos() - start,
                  };
              }
              b'{' => {
                  // Check for escaped brace `{{`
                  if self.cursor.peek() == b'{' {
                      self.cursor.advance(); // consume first '{'
                      self.cursor.advance(); // consume second '{'
                      continue;
                  }
                  self.cursor.advance(); // consume '{'
                  self.template_depth.push(0); // enter interpolation
                  return RawToken {
                      tag: RawTag::TemplateHead,
                      len: self.cursor.pos() - start,
                  };
              }
              b'}' => {
                  // Escaped brace `}}`
                  if self.cursor.peek() == b'}' {
                      self.cursor.advance();
                      self.cursor.advance();
                      continue;
                  }
                  // Lone `}` inside template text -- invalid, but consume
                  self.cursor.advance();
              }
              b'\\' => {
                  self.cursor.advance(); // consume '\'
                  self.cursor.advance(); // consume escaped char
              }
              b'\n' => {
                  // Templates can span multiple lines
                  self.cursor.advance();
              }
              0 => {
                  return RawToken {
                      tag: RawTag::UnterminatedTemplate,
                      len: self.cursor.pos() - start,
                  };
              }
              _ => self.cursor.advance(),
          }
      }
  }
  ```

- [ ] Implement `template_middle_or_tail()` -- entered when `}` closes an interpolation (brace depth 0):
  ```rust
  fn template_middle_or_tail(&mut self, start: u32) -> RawToken {
      self.cursor.advance(); // consume closing '}'
      loop {
          match self.cursor.current() {
              b'`' => {
                  self.cursor.advance(); // consume closing '`'
                  return RawToken {
                      tag: RawTag::TemplateTail,
                      len: self.cursor.pos() - start,
                  };
              }
              b'{' => {
                  if self.cursor.peek() == b'{' {
                      self.cursor.advance();
                      self.cursor.advance();
                      continue;
                  }
                  self.cursor.advance(); // consume '{'
                  self.template_depth.push(0);
                  return RawToken {
                      tag: RawTag::TemplateMiddle,
                      len: self.cursor.pos() - start,
                  };
              }
              b'}' => {
                  if self.cursor.peek() == b'}' {
                      self.cursor.advance();
                      self.cursor.advance();
                      continue;
                  }
                  self.cursor.advance();
              }
              b'\\' => {
                  self.cursor.advance();
                  self.cursor.advance();
              }
              b'\n' => {
                  self.cursor.advance();
              }
              0 => {
                  return RawToken {
                      tag: RawTag::UnterminatedTemplate,
                      len: self.cursor.pos() - start,
                  };
              }
              _ => self.cursor.advance(),
          }
      }
  }
  ```

- [ ] Handle `{` and `}` in the main scanner with template awareness:
  ```rust
  fn left_brace(&mut self, start: u32) -> RawToken {
      self.cursor.advance();
      // If inside a template interpolation, increment brace depth
      if let Some(depth) = self.template_depth.last_mut() {
          *depth += 1;
      }
      RawToken { tag: RawTag::LeftBrace, len: self.cursor.pos() - start }
  }

  fn right_brace(&mut self, start: u32) -> RawToken {
      if let Some(depth) = self.template_depth.last_mut() {
          if *depth == 0 {
              // This `}` closes the interpolation -- scan template continuation
              self.template_depth.pop();
              return self.template_middle_or_tail(start);
          }
          *depth -= 1;
      }
      self.cursor.advance();
      RawToken { tag: RawTag::RightBrace, len: self.cursor.pos() - start }
  }
  ```

- [ ] Implement `in_template_interpolation()`:
  ```rust
  fn in_template_interpolation(&self) -> bool {
      !self.template_depth.is_empty()
  }
  ```

### Template Escape Sequences

Template literals support these escapes (grammar.ebnf line 107, validated in cooking layer): `` \` ``, `\\`, `\n`, `\t`, `\r`, `\0`. Literal braces are written as `{{` and `}}` (line 108). The raw scanner skips `\X` pairs without validation, just like string scanning.

### Example Token Sequences

```
`hello`              -> [TemplateComplete]
`hello {name}`       -> [TemplateHead, Ident("name"), TemplateTail]
`{a} + {b}`          -> [TemplateHead, Ident("a"), TemplateMiddle, Ident("b"), TemplateTail]
`{x + {a: 1}}`       -> [TemplateHead, Ident("x"), Plus, LeftBrace, Ident("a"), Colon, Int, RightBrace, TemplateTail]
`outer {`inner {x}`}` -> [TemplateHead, TemplateHead, Ident("x"), TemplateTail, TemplateTail]
```

---

## 02.7 Numeric Literal Scanning

- [ ] Greedy consumption of numeric characters (Zig pattern -- defer validation):
  ```rust
  fn number(&mut self) -> RawToken {
      let start = self.cursor.pos();
      let first = self.cursor.current();
      self.cursor.advance();

      // Check for hex prefix: 0x
      if first == b'0' {
          match self.cursor.current() {
              b'x' | b'X' => return self.hex_number(start),
              _ => {}
          }
      }

      // Decimal digits and underscores
      self.eat_decimal_digits();

      // Check for float (dot followed by digit, not dot-dot)
      if self.cursor.current() == b'.' && self.cursor.peek().is_ascii_digit() {
          self.cursor.advance(); // consume '.'
          self.eat_decimal_digits();
          // Check for exponent
          if matches!(self.cursor.current(), b'e' | b'E') {
              self.cursor.advance();
              if matches!(self.cursor.current(), b'+' | b'-') {
                  self.cursor.advance();
              }
              self.eat_decimal_digits();
          }
          // Check for duration/size suffix (decimal duration/size ARE valid per grammar)
          return self.check_suffix(start);
      }

      // Check for duration/size suffix (integer duration/size)
      self.check_suffix(start)
  }
  ```
- [ ] **Only decimal and hex** -- grammar.ebnf lines 91-93 specifies `int_literal = decimal_lit | hex_lit`. No binary (`0b`) or octal (`0o`) literals.
- [ ] Duration suffixes (grammar.ebnf line 137): `ns`, `us`, `ms`, `s`, `m`, `h` -- consumed greedily
- [ ] Size suffixes (grammar.ebnf line 144): `b`, `kb`, `mb`, `gb`, `tb` -- consumed greedily
- [ ] Decimal duration/size (grammar.ebnf lines 136, 143): `0.5s`, `1.5kb` ARE valid. The grammar explicitly defines `decimal_duration = decimal_lit "." decimal_lit` and `decimal_size = decimal_lit "." decimal_lit`. These produce `Duration` or `Size` tags; the cooking layer converts them to integer nanoseconds/bytes via compile-time arithmetic (spec: "compile-time sugar computed via integer arithmetic").
- [ ] Implement `check_suffix()` helper that checks for duration/size suffix after both integer and float tokens. Returns `Int`, `Float`, `Duration`, or `Size` depending on whether a suffix is present and what type of number preceded it.
- [ ] Actual numeric value parsing (converting string to `u64`/`f64`) happens in the cooking layer
- [ ] Underscores in numbers: consumed as part of the token; validated in cooking layer

---

## 02.8 Comment & Whitespace Scanning

Grammar.ebnf line 42: `comment = "//" { unicode_char - newline } newline .`
Grammar.ebnf line 33: `whitespace = ' ' | '\t' | '\r' | newline .`

NOTE: The Ori grammar does NOT support block comments (`/* */`). Only line comments (`//`) are specified.

- [ ] Line comments (`//`):
  ```rust
  fn line_comment(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // consume second '/'
      // Scan to end of line (memchr for '\n' in Section 05)
      self.cursor.eat_while(|b| b != b'\n' && b != 0);
      RawToken { tag: RawTag::LineComment, len: self.cursor.pos() - start }
  }
  ```
- [ ] Whitespace (horizontal only: space, tab, carriage return):
  ```rust
  fn whitespace(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.eat_while(|b| b == b' ' || b == b'\t' || b == b'\r');
      RawToken { tag: RawTag::Whitespace, len: self.cursor.pos() - start }
  }
  ```
- [ ] Whitespace tokens are produced (not skipped) so the cooking layer can decide what to do with them:
  - In `lex()` mode: whitespace is consumed/skipped, `TokenFlags` computed
  - In `lex_with_comments()` mode: whitespace is consumed but newline positions are tracked

---

## 02.9 Newline Handling

Ori is newline-significant: newlines serve as implicit statement separators. The scanner emits `RawTag::Newline` for every newline encountered.

Grammar.ebnf line 32: `newline = /* U+000A */` (LF, not CRLF).
Grammar.ebnf line 33: `whitespace = ' ' | '\t' | '\r' | newline` (CR is horizontal whitespace, NOT a newline).

NOTE: `\r\n` (CRLF) should be handled as: `\r` (whitespace) followed by `\n` (newline). The grammar does not define CRLF as a single newline unit. However, for practical compatibility with Windows files, the scanner MAY normalize `\r\n` to a single `Newline` token.

- [ ] `\n` produces `RawTag::Newline`
- [ ] `\r\n` SHOULD be normalized to a single newline for Windows compatibility (consume `\r` then `\n` as one `Newline` token), even though the grammar technically treats `\r` as horizontal whitespace
- [ ] No backslash line continuation -- the spec uses implicit operator/delimiter context continuation. Line continuation is a parser concern, not a lexer concern:
  - The parser checks if the preceding token is an operator or delimiter to determine whether to continue across a newline
  - The lexer simply emits `RawTag::Newline` for every newline
  - This avoids needing the lexer to understand expression context

---

## 02.10 Tests & Verification

- [ ] **Equivalence tests**: For every file in `tests/spec/`, verify that the raw scanner produces the same token sequence (mapped through tags) as the current logos-based lexer
- [ ] **Byte coverage**: Test that all 256 byte values at position 0 produce a valid `RawToken` (no panics)
- [ ] **Fuzz testing** (if available): Random byte sequences produce valid `RawToken` streams ending in `Eof`
- [ ] **Template literal tests**:
  - Empty template: `` ` ` `` -> `TemplateComplete`
  - No interpolation: `` `hello` `` -> `TemplateComplete`
  - Single interpolation: `` `{x}` `` -> `TemplateHead`, `Ident`, `TemplateTail`
  - Multiple interpolations: `` `{a} and {b}` `` -> `TemplateHead`, `Ident`, `TemplateMiddle`, `Ident`, `TemplateTail`
  - Nested braces in interpolation: `` `{if x then {a: 1} else {b: 2}}` `` -> correct nesting
  - Nested templates: `` `outer {`inner {x}`}` `` -> double nesting
  - Escaped braces: `` `{{literal}}` `` -> `TemplateComplete`
  - Escaped backtick: `` `hello \` world` `` -> `TemplateComplete`
  - Unterminated template: `` `hello `` -> `UnterminatedTemplate`
  - Multiline template: backtick strings spanning lines
- [ ] **Operator tests** (verify against grammar.ebnf lines 72-77):
  - `??` -> `QuestionQuestion` (grammar.ebnf line 77)
  - `...` -> `DotDotDot` (grammar.ebnf line 184 for C-style variadic, line 259 for variadic params, line 371 for spread args)
  - `$` -> `Dollar` (grammar.ebnf line 82; used in const generics and binding patterns)
  - `>` always single token (parser handles `>=`, `>>`)
  - `..=` -> `DotDotEqual` (grammar.ebnf line 77)
  - `#[` -> `HashBracket` (implementation token; grammar uses `"#" identifier` on line 198)
  - `#!` -> `HashBang` (grammar.ebnf line 156 for file-level attributes)
  - `#` alone -> `Hash`
  - `;` -> `Semicolon` (error-detection token; semicolons are NOT in the grammar as valid Ori syntax)
  - `\` standalone -> `Backslash` (error-detection token; backslash only valid inside escape sequences)
  - `_` -> `Underscore` (grammar.ebnf uses underscore in identifiers, patterns, etc.)
  - No `+=`, `-=`, `*=`, `/=` (compound assignment not in grammar)
  - No `|>` (pipe-right not in grammar)
- [ ] **Edge cases**:
  - Empty source -> single `Eof` token
  - Source containing only whitespace/newlines
  - Unterminated string at EOF
  - Unterminated char at EOF
  - All operator combinations (single and compound)
  - Interior null byte (should produce `InvalidByte`, not premature EOF)
  - Maximum-length tokens (very long strings, identifiers, numbers)
  - Adjacent tokens with no whitespace (`a+b`, `1+2`, `"x""y"`)
  - Non-ASCII bytes (128-255) produce `InvalidByte` (grammar.ebnf line 29: `letter = 'A' … 'Z' | 'a' … 'z'` — ASCII only)
  - Decimal duration/size: `0.5s`, `1.5kb` are VALID tokens (grammar.ebnf lines 136, 143)
- [ ] **Property tests**:
  - Total bytes consumed by all tokens equals source length (no gaps, no overlaps)
  - Every token has `len > 0` (except `Eof` which has `len == 0`)
  - `Eof` is always the last token
  - Scanner never enters an infinite loop (timeout-based test)
  - Template depth stack is always empty after scanning a complete source

---

## 02.11 Completion Checklist

- [ ] `raw_scanner.rs` module added to `ori_lexer_core`
- [ ] `RawTag` enum defined with `#[repr(u8)]`, `#[non_exhaustive]`, <= 256 variants
- [ ] `RawTag::name()` and `RawTag::lexeme()` implemented (v2-conventions SS2)
- [ ] `RawScanner` produces correct tokens for all existing test files
- [ ] Template literal scanning works with stack-based nesting
- [ ] `??`, `...`, `$`, `#[`, `#!` operators/delimiters handled
- [ ] No `0b`/`0o` numeric prefixes (only decimal + hex per grammar)
- [ ] No backslash line continuation (parser handles continuation)
- [ ] Zero heap allocation verified (no `String`, `Vec`, `Box` in scanner code except `template_depth` stack)
- [ ] All 256 byte values handled without panic
- [ ] Equivalence with logos output verified for full test suite
- [ ] `cargo t -p ori_lexer_core` passes

**Exit Criteria:** The raw scanner produces identical token boundaries to the current logos-based scanner for all test files (minus template literals, which are new; and decimal duration/size literals, which are currently incorrectly treated as errors). It allocates zero bytes on the heap in the non-template path. The `template_depth` stack is the only heap allocation and only active during template literal scanning. All byte values are handled gracefully. Non-ASCII bytes produce `InvalidByte`.

**Critical Fix Required:** The current RawToken implementation (lines 271-292) incorrectly treats decimal duration/size literals as errors (e.g., `FloatDurationNs`, `FloatSizeKb`). The grammar explicitly allows these (grammar.ebnf lines 136, 143). The raw scanner MUST produce `Duration` or `Size` tags for these literals.

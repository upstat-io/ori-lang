---
section: "07"
title: Diagnostics & Error Recovery
status: complete
goal: "Context-aware, actionable error messages with WHERE+WHAT+WHY+HOW shape, cross-language habit detection, and graceful recovery"
sections:
  - id: "07.1"
    title: LexError Structure
    status: complete
  - id: "07.2"
    title: LexErrorKind Variants
    status: complete
  - id: "07.3"
    title: whatIsNext Context Inspection
    status: complete
  - id: "07.4"
    title: Cross-Language Habit Detection
    status: complete
  - id: "07.5"
    title: Unicode Confusable Recovery
    status: complete
  - id: "07.6"
    title: Detached Doc Comment Warnings
    status: complete
  - id: "07.7"
    title: Recovery Strategies
    status: complete
  - id: "07.8"
    title: Tests
    status: complete
---

# Section 07: Diagnostics & Error Recovery

**Status:** :white_check_mark: Complete
**Goal:** Generate context-aware, actionable error messages for all lexer error classes following the WHERE+WHAT+WHY+HOW error shape. Detect common mistakes from developers coming from other languages. Recover gracefully from errors to enable continued parsing.

> **REFERENCE**: Elm's `whatIsNext` pattern (inspect the stuck character to tailor error messages); Gleam's proactive detection of JavaScript/C habits (`===`, `;`, `'`); Rust's Unicode confusable table (200+ character substitutions); Go's curly quote detection.

> **Conventions:** Error shape follows `plans/v2-conventions.md` section 5 (WHERE+WHAT+WHY+HOW). All types derive `Clone, Eq, PartialEq, Hash, Debug` for Salsa compatibility (section 8). Factory methods use `#[cold]`; fluent builders use `#[must_use]` (section 5).

---

## Design Rationale

### Error Message Philosophy

Following the Ori diagnostic guidelines and learning from Elm's famous error messages:

1. **Imperative suggestions**: "Try removing this semicolon" not "Unexpected token `;`"
2. **Verb phrase fixes**: "Replace `===` with `==`" not "Invalid operator `===`"
3. **Context from source**: Inspect what character/pattern the lexer got stuck on
4. **Cross-language empathy**: Detect patterns from JavaScript, Python, C, Rust and provide Ori-specific guidance
5. **Never generic**: Every error case should have its own tailored message. No catch-all "unexpected character" for common cases.

### Architecture

Error diagnostics are generated in the cooking layer (Section 03), not the raw scanner. The raw scanner produces error tags (`RawTag::InvalidByte`, `RawTag::UnterminatedString`, etc.) and the cooker converts these to rich diagnostics using the source context.

`LexSuggestion` is internal to the lexer phase. Final rendering in `oric` maps `LexSuggestion` to `ori_diagnostic::Suggestion` (with `Applicability`). This is the same separation pattern types V2 uses -- phase crates remain independent of the diagnostic rendering system.

---

## 07.1 LexError Structure

The `LexError` type follows the cross-system error shape from `v2-conventions.md` section 5: WHERE (span) + WHAT (kind) + WHY (context) + HOW (suggestions).

- [x] Define `LexError`:
  ```rust
  /// Lexical error -- follows the cross-system error shape
  /// (plans/v2-conventions.md §5: WHERE + WHAT + WHY + HOW).
  ///
  /// All types derive Clone, Eq, PartialEq, Hash, Debug
  /// for Salsa compatibility (§8).
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub struct LexError {
      pub span: Span,                         // WHERE (from ori_ir)
      pub kind: LexErrorKind,                 // WHAT went wrong
      pub context: LexErrorContext,            // WHY we were checking
      pub suggestions: Vec<LexSuggestion>,    // HOW to fix
  }
  ```

- [x] Define `LexErrorContext` (WHY we were checking when the error occurred):
  ```rust
  /// Lexing context at the point of error -- the WHY (v2-conventions §5).
  /// Matches the `ErrorContext` pattern from types V2's `TypeCheckError`.
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub enum LexErrorContext {
      TopLevel,
      InsideString { start: u32 },
      InsideChar,
      InsideTemplate { start: u32, nesting: u32 },
      NumberLiteral { base: NumBase },
  }

  impl Default for LexErrorContext {
      fn default() -> Self { Self::TopLevel }
  }
  ```

- [x] Define `LexSuggestion` and `LexReplacement` (HOW to fix):
  ```rust
  /// Suggestion for fixing a lexical error -- the HOW (v2-conventions §5).
  /// Internal type; final rendering in `oric` maps to
  /// `ori_diagnostic::Suggestion` (with Applicability).
  /// Same pattern as types V2's `Suggestion`.
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub struct LexSuggestion {
      pub message: String,
      pub replacement: Option<LexReplacement>,
      pub priority: u8,
  }

  /// A concrete text replacement for an auto-fix.
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub struct LexReplacement {
      pub span: Span,
      pub text: String,
  }
  ```

- [x] Add `#[cold]` factory methods and `#[must_use]` fluent builders (v2-conventions section 5):
  ```rust
  impl LexError {
      /// Create an unterminated string error.
      #[cold]
      pub fn unterminated_string(start: u32, span: Span) -> Self {
          Self {
              span,
              kind: LexErrorKind::UnterminatedString,
              context: LexErrorContext::InsideString { start },
              suggestions: Vec::new(),
          }
      }

      /// Create an unterminated template literal error.
      #[cold]
      pub fn unterminated_template(start: u32, span: Span) -> Self {
          Self {
              span,
              kind: LexErrorKind::UnterminatedTemplate,
              context: LexErrorContext::InsideTemplate { start, nesting: 0 },
              suggestions: Vec::new(),
          }
      }

      /// Create an invalid escape sequence error.
      #[cold]
      pub fn invalid_escape(escape_char: char, span: Span) -> Self {
          Self {
              span,
              kind: LexErrorKind::InvalidEscape { escape: escape_char },
              context: LexErrorContext::default(),
              suggestions: Vec::new(),
          }
      }

      /// Create a semicolon error with a removal suggestion.
      #[cold]
      pub fn semicolon(span: Span) -> Self {
          Self {
              span,
              kind: LexErrorKind::Semicolon,
              context: LexErrorContext::TopLevel,
              suggestions: vec![LexSuggestion {
                  message: "Remove the semicolon".to_string(),
                  replacement: Some(LexReplacement {
                      span,
                      text: String::new(),
                  }),
                  priority: 0,
              }],
          }
      }

      /// Add a context to this error.
      #[must_use]
      pub fn with_context(mut self, ctx: LexErrorContext) -> Self {
          self.context = ctx;
          self
      }

      /// Add a suggestion to this error.
      #[must_use]
      pub fn with_suggestion(mut self, suggestion: LexSuggestion) -> Self {
          self.suggestions.push(suggestion);
          self
      }
  }
  ```

---

## 07.2 LexErrorKind Variants

- [x] Define `LexErrorKind` enum with specific variants for each error class:

  **String/char errors:**
  - `UnterminatedString` -- string literal reaches newline or EOF without closing `"`
  - `UnterminatedChar` -- char literal reaches newline or EOF without closing `'`
  - `InvalidEscape { escape: char }` -- unrecognized escape sequence `\X`
  - `EmptyCharLiteral` -- `''` with nothing between quotes
  - `MultiCharLiteral` -- `'ab'` with multiple characters

  **Template literal errors:**
  - `UnterminatedTemplate` -- template literal `` `...` `` reaches EOF without closing backtick
  - `InvalidTemplateEscape { escape: char }` -- bad escape sequence inside template string

  **Numeric errors:**
  - `IntegerOverflow` -- literal exceeds `u64::MAX`
  - `InvalidDigitForRadix { digit: char, radix: u8 }` -- `0xGG` (G is not hex)
  - `EmptyExponent` -- `1.5e` with no exponent digits
  - `LeadingZero` -- `007` (leading zeros are not allowed in decimal literals)
  - `TrailingUnderscore` -- `100_`
  - `ConsecutiveUnderscores` -- `1__000`

  **Character errors:**
  - `InvalidByte { byte: u8 }` -- unrecognized byte in source
  - `UnicodeConfusable { found: char, suggested: char, name: &'static str }` -- visually similar Unicode character (Section 07.5)
  - `InvalidNullByte` -- interior null byte in source
  - `InvalidControlChar { byte: u8 }` -- ASCII control character (0x01-0x1F except `\t`, `\n`, `\r`)

  **Cross-language pattern errors** (Section 07.4):
  - `Semicolon` -- `;` used (C/JavaScript/Rust habit)
  - `TripleEqual` -- `===` used (JavaScript habit)
  - `SingleQuoteString` -- `'string'` instead of `"string"`
  - `IncrementDecrement { op: &'static str }` -- `++` or `--` used (C/JavaScript habit)
  - `TernaryOperator` -- `? :` pattern (C habit -- Ori uses `if`/`else` expressions)

  ```rust
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub enum LexErrorKind {
      // === String/Char Errors ===
      UnterminatedString,
      UnterminatedChar,
      InvalidEscape { escape: char },
      EmptyCharLiteral,
      MultiCharLiteral,

      // === Template Literal Errors ===
      UnterminatedTemplate,
      InvalidTemplateEscape { escape: char },

      // === Numeric Errors ===
      IntegerOverflow,
      InvalidDigitForRadix { digit: char, radix: u8 },
      EmptyExponent,
      LeadingZero,
      TrailingUnderscore,
      ConsecutiveUnderscores,

      // === Character Errors ===
      InvalidByte { byte: u8 },
      UnicodeConfusable {
          found: char,
          suggested: char,
          name: &'static str,
      },
      InvalidNullByte,
      InvalidControlChar { byte: u8 },

      // === Cross-Language Pattern Errors (Section 07.4) ===
      Semicolon,
      TripleEqual,
      SingleQuoteString,
      IncrementDecrement { op: &'static str },
      TernaryOperator,
  }
  ```

  **Deliberately excluded:**
  - ~~`FloatDuration` / `FloatSize`~~ -- the spec allows decimal durations and sizes
  - `MissingDigitsAfterRadix` for hex (`0x`) and binary (`0b`) -- needed when prefix present but no digits follow
  - ~~`MissingDigitsAfterRadix` for octal~~ -- `0o` not in spec
  - ~~`InvalidHexEscape` / `InvalidUnicodeEscape`~~ -- `\xHH` and `\u{XXXX}` escapes are not in spec

---

## 07.3 whatIsNext Context Inspection

- [x] Implement a `what_is_next(source: &[u8], pos: u32) -> NextContext` function (inspired by Elm):
  ```rust
  /// Inspects the character at the given position and classifies it
  /// for use in error message generation. This is the Elm "whatIsNext"
  /// pattern -- inspect what the lexer got stuck on to tailor the message.
  pub enum NextContext {
      Keyword(String),                          // A known keyword from another language
      Operator(String),                         // An operator-like sequence
      Identifier(String),                       // An identifier-like sequence
      StringLiteral,                            // Start of a string
      Number,                                   // Start of a number
      Whitespace,                               // Unexpected whitespace
      Punctuation(char),                        // Single punctuation character
      Unicode(char, &'static str),              // Non-ASCII with Unicode name
      EndOfFile,                                // At EOF
      Other(char),                              // Anything else
  }
  ```

- [x] Use `NextContext` to generate tailored error messages:
  ```rust
  match what_is_next(source, error_pos) {
      NextContext::Punctuation(';') => {
          "Ori doesn't use semicolons. Try removing this character."
      }
      NextContext::Operator("===") => {
          "Ori uses `==` for equality comparison, not `===`."
      }
      NextContext::Punctuation('\'') => {
          "Strings in Ori use double quotes: \"hello\" not 'hello'"
      }
      NextContext::Keyword(kw) if kw == "return" => {
          "Ori is expression-based. The last expression in a block is its value -- \
           there is no `return` keyword."
      }
      NextContext::Keyword(kw) if kw == "null" || kw == "nil" => {
          "Ori uses `void` for the absence of a value."
      }
      NextContext::Unicode(ch, name) => {
          format!(
              "Found Unicode character '{}' ({}). \
               Ori source code uses ASCII characters only.",
              ch, name
          )
      }
      // ... etc
  }
  ```

---

## 07.4 Cross-Language Habit Detection

- [x] Detect and provide helpful messages for common patterns from other languages:

  | Pattern | Source Language | Ori Message |
  |---------|---------------|-------------|
  | `;` at end of expression | C, JS, Rust, Java | "Ori doesn't use semicolons -- expressions are separated by newlines. Remove this semicolon." |
  | `===` / `!==` | JavaScript | "Use `==` for equality and `!=` for inequality in Ori." |
  | `'string'` | Python, JS | "Use double quotes for strings in Ori: `\"string\"` instead of `'string'`" |
  | `++` / `--` | C, JS, Java | "Ori doesn't have increment/decrement operators. Use `x + 1` or `x - 1`." |
  | `#include` / `#define` | C/C++ | "Ori doesn't use preprocessor directives. Use `use` for modules." |
  | `var` / `const` | JS, Go | "Use `let` for bindings in Ori." |
  | `func` | Go | "Use `@name (params) -> type = body` to declare functions in Ori." |
  | `return` | C, JS, Rust, Go | "Ori is expression-based. The last expression in a block is its value -- there is no `return` keyword." |
  | `null` | C, JS, Java | "Ori uses `void` for the absence of a value." |
  | `class` | JS, Python, Java | "Use `type` for type definitions in Ori." |

- [x] Implement detection in the cooking layer when `RawTag::InvalidByte` or `RawTag::Ident` produces an unexpected sequence

- [x] Cross-language keyword detection:
  ```rust
  /// Known keywords from other languages that are not Ori keywords.
  /// When encountered as identifiers, the cooker can produce a helpful
  /// note (not an error) suggesting the Ori equivalent.
  const FOREIGN_KEYWORDS: &[(&str, &str)] = &[
      ("function", "Use `@name (params) -> type = body` to declare functions in Ori."),
      ("func",     "Use `@name (params) -> type = body` to declare functions in Ori."),
      ("fn",       "Use `@name (params) -> type = body` to declare functions in Ori."),
      ("var",      "Use `let` for variable bindings in Ori."),
      ("const",    "Use `let` for variable bindings in Ori."),
      ("return",   "Ori is expression-based. The last expression in a block \
                    is its value -- there is no `return` keyword."),
      ("null",     "Ori uses `void` for the absence of a value."),
      ("nil",      "Ori uses `void` for the absence of a value."),
      ("class",    "Use `type` for type definitions in Ori."),
      ("struct",   "Use `type Name = { fields }` for record types in Ori."),
      ("interface","Use `trait` for interfaces in Ori."),
      ("enum",     "Use `type` with variants for enums in Ori."),
      ("switch",   "Use `match` for pattern matching in Ori."),
      ("while",    "Use `loop` with `if`/`break` in Ori."),
  ];
  ```

---

## 07.5 Unicode Confusable Recovery

Unicode confusable detection provides error messages about visually similar Unicode characters found in source code. This is for error recovery -- Ori source is ASCII-only per spec, so any non-ASCII byte is an error. The confusable table lets us produce helpful messages instead of generic "unexpected byte" errors.

- [x] Build a lookup table of visually confusable Unicode characters (inspired by Rust's `unicode_chars.rs`):
  ```rust
  /// Maps Unicode characters that are visually similar to ASCII to their
  /// ASCII equivalents. When a confusable is detected, the lexer emits
  /// an error with a substitution suggestion.
  ///
  /// This is NOT Unicode identifier support -- Ori is ASCII-only.
  /// This table exists solely for better error messages.
  const UNICODE_CONFUSABLES: &[(char, char, &str)] = &[
      // Dashes
      ('\u{2010}', '-', "Hyphen"),
      ('\u{2011}', '-', "Non-Breaking Hyphen"),
      ('\u{2012}', '-', "Figure Dash"),
      ('\u{2013}', '-', "En Dash"),
      ('\u{2014}', '-', "Em Dash"),

      // Quotes (most common confusable in practice -- copy/paste from word processors)
      ('\u{2018}', '\'', "Left Single Quotation Mark"),
      ('\u{2019}', '\'', "Right Single Quotation Mark"),
      ('\u{201C}', '"', "Left Double Quotation Mark"),
      ('\u{201D}', '"', "Right Double Quotation Mark"),

      // Fullwidth characters
      ('\u{FF0B}', '+', "Fullwidth Plus Sign"),
      ('\u{FF0D}', '-', "Fullwidth Hyphen-Minus"),
      ('\u{FF1D}', '=', "Fullwidth Equals Sign"),
      ('\u{FF08}', '(', "Fullwidth Left Parenthesis"),
      ('\u{FF09}', ')', "Fullwidth Right Parenthesis"),
      ('\u{FF1A}', ':', "Fullwidth Colon"),
      ('\u{FF1B}', ';', "Fullwidth Semicolon"),
      ('\u{FF3B}', '[', "Fullwidth Left Square Bracket"),
      ('\u{FF3D}', ']', "Fullwidth Right Square Bracket"),
      ('\u{FF5B}', '{', "Fullwidth Left Curly Bracket"),
      ('\u{FF5D}', '}', "Fullwidth Right Curly Bracket"),

      // Common look-alikes
      ('\u{00B7}', '.', "Middle Dot"),
      ('\u{2219}', '.', "Bullet Operator"),
      ('\u{2024}', '.', "One Dot Leader"),
      ('\u{00D7}', '*', "Multiplication Sign"),
      ('\u{2217}', '*', "Asterisk Operator"),
      ('\u{2212}', '-', "Minus Sign"),
      ('\u{2215}', '/', "Division Slash"),
      ('\u{2044}', '/', "Fraction Slash"),

      // Zero-width and invisible characters
      ('\u{200B}', ' ', "Zero Width Space"),
      ('\u{200C}', ' ', "Zero Width Non-Joiner"),
      ('\u{200D}', ' ', "Zero Width Joiner"),
      ('\u{FEFF}', ' ', "Zero Width No-Break Space (BOM)"),

      // ... ~50-100 common confusables total
  ];
  ```

- [x] When the raw scanner produces `InvalidByte` for a non-ASCII byte:
  1. Decode the UTF-8 character at the error position
  2. Look up in the confusable table
  3. If found: emit a `UnicodeConfusable` error with the substitution suggestion and continue scanning
  4. If not found: emit a generic "unexpected character" error with the Unicode name if available

- [x] Special case: "smart quotes" (curly quotes from word processors) -- these are by far the most common confusable in practice. Produce an especially clear message:
  ```
  Found left double quotation mark ("\u{201C}"). Use a straight double
  quote (") instead. This often happens when copying code from a word
  processor or web page.
  ```

---

## 07.6 Detached Doc Comment Warnings

The spec defines doc comment markers (`grammar.ebnf` lines 43-47): `*` (member), `!` (warning), `>` (example). A doc comment that is not followed by a declaration is "detached" and likely a mistake.

- [x] Detect detached doc comments:
  ```rust
  /// A doc comment (`/// ...`) that is not immediately followed by a
  /// declaration (`@name`, `type`, `trait`, `let`, etc.) is detached.
  /// This is a warning, not an error -- the comment is still valid
  /// but likely not attached to what the author intended.
  ///
  /// Detection happens in the cooking layer:
  /// 1. When the cooker encounters a doc comment token (RawTag::DocComment),
  ///    it records it as "pending doc."
  /// 2. When the next non-trivia token is produced, check:
  ///    - If it's a declaration keyword or `@` (type, trait, let, pub, impl, use, @):
  ///      clear pending doc (correctly attached).
  ///    - If it's anything else or another doc comment with a blank line gap:
  ///      emit a DetachedDocComment warning for the pending doc.
  #[derive(Clone, Debug, Eq, PartialEq, Hash)]
  pub struct DetachedDocWarning {
      pub span: Span,
      pub marker: DocMarker,
  }

  #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
  pub enum DocMarker {
      Member,   // `* field:`
      Warning,  // `!`
      Example,  // `>`
      Plain,    // no marker
  }
  ```

- [x] Warning message:
  ```
  This doc comment is not attached to any declaration. Doc comments
  should appear immediately before a function (`@name`), `type`, `trait`,
  or other declaration.
  ```

- [x] Detached doc warnings are accumulated alongside `LexError`s but are separate -- they are warnings, not errors, and do not prevent compilation.

---

## 07.7 Recovery Strategies

All recovery strategies must ensure forward progress (never re-scan the same byte). Error tokens carry their span so the parser can report accurate positions.

- [x] **After unterminated string**: Resume scanning at the next newline (the string was likely meant to be single-line)
- [x] **After unterminated template**: Resume scanning at the next unmatched backtick or EOF
- [x] **After invalid byte**: Skip the byte and continue scanning
- [x] **After invalid escape in string**: Include the literal `\X` in the string content and continue
- [x] **After numeric overflow**: Produce a placeholder integer token and continue
- [x] **After unterminated char**: Resume at next `'` or newline

- [x] Implement error accumulation during tokenization:
  ```rust
  /// Errors are accumulated, not fatal -- the lexer continues past errors
  /// for IDE support (v2-conventions §6). Error vectors start empty
  /// (v2-conventions §9).
  pub struct TokenizerState {
      errors: Vec<LexError>,
      warnings: Vec<DetachedDocWarning>,
      context: LexErrorContext,
  }

  impl TokenizerState {
      fn emit_error(&mut self, kind: LexErrorKind, span: Span) {
          self.errors.push(LexError {
              span,
              kind,
              context: self.context.clone(),
              suggestions: Vec::new(),
          });
      }
  }
  ```

- [x] All recovery strategies ensure the lexer resumes at a well-defined position and produces a valid (if erroneous) token stream. The parser receives error tokens and can provide its own recovery on top.

---

## 07.8 Tests

- [x] **Error message tests**: Each error class produces a specific, non-generic message
  - `UnterminatedString` includes the position of the opening quote
  - `InvalidEscape` includes the unrecognized escape character
  - `UnicodeConfusable` includes the character name and ASCII replacement
  - `IntegerOverflow` identifies the literal that overflowed

- [x] **Error structure tests** (v2-conventions section 5 compliance):
  - Every `LexError` has a non-empty span (WHERE)
  - Every `LexError` has a specific `LexErrorKind` (WHAT), not a generic catch-all
  - Context-dependent errors have the correct `LexErrorContext` (WHY)
  - Fixable errors have at least one `LexSuggestion` (HOW)

- [x] **Cross-language detection tests**:
  - Source containing `;` after expressions -> `Semicolon` error with removal suggestion
  - Source containing `===` -> `TripleEqual` error with `==` replacement
  - Source containing `'hello'` -> `SingleQuoteString` error with double-quote replacement
  - Source containing `++x` -> `IncrementDecrement` error
  - Source containing `#include` -> helpful note about `use`

- [x] **Unicode confusable tests**:
  - Smart quotes (`\u{201C}`, `\u{201D}`) -> straight double quote suggestion
  - En dash (`\u{2013}`) -> hyphen-minus suggestion
  - Fullwidth characters -> ASCII equivalents
  - Zero-width spaces -> detected and reported
  - Multiple confusables in one file -> all reported

- [x] **Template literal error tests**:
  - `` `hello {name `` (unterminated) -> `UnterminatedTemplate` with opening position
  - `` `hello \q world` `` -> `InvalidTemplateEscape` for `\q`

- [x] **Detached doc comment tests**:
  - `/// doc\n@foo () -> void = ...` -> no warning (correctly attached)
  - `/// doc\n\n@foo () -> void = ...` -> warning (blank line gap)
  - `/// doc\nlet x = 5` -> no warning (attached to let)
  - `/// doc\nx + y` -> warning (not a declaration)

- [x] **Recovery tests**:
  - Unterminated string followed by valid code -> subsequent tokens are correct
  - Unterminated template followed by valid code -> subsequent tokens are correct
  - Invalid byte followed by valid code -> subsequent tokens are correct
  - Multiple errors in one file -> all are reported

- [x] **Error accumulation tests**: Verify that multiple lexer errors are collected and reported together, not just the first one

---

## 07.9 Completion Checklist

- [x] `LexError` with WHERE+WHAT+WHY+HOW shape (v2-conventions section 5)
- [x] `LexErrorKind` enum with specific variants for all error classes
- [x] `LexErrorContext` enum covering all lexing states
- [x] `LexSuggestion` + `LexReplacement` for auto-fix support
- [x] `#[cold]` factory methods, `#[must_use]` fluent builders
- [x] All types derive `Clone, Eq, PartialEq, Hash, Debug` (v2-conventions section 8)
- [x] `what_is_next` context inspection function
- [x] Cross-language habit detection for top 10 patterns
- [x] Unicode confusable table with >= 30 entries
- [x] Detached doc comment warning detection
- [x] Recovery strategies for all error classes
- [x] Every error class has a tailored, non-generic message
- [x] Template literal errors (`UnterminatedTemplate`, `InvalidTemplateEscape`)
- [x] Tests for all error classes and recovery paths
- [x] `cargo t -p ori_lexer` passes

**Exit Criteria:** Every lexer error follows the WHERE+WHAT+WHY+HOW shape and produces a context-aware, actionable diagnostic message. Cross-language habits and Unicode confusables are detected. Template literal errors are handled. Detached doc comments produce warnings. Recovery after errors allows continued parsing. No generic "unexpected character" messages for common cases.

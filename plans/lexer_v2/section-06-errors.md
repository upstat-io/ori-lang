---
section: "06"
title: Error Handling
status: not-started
goal: Rich, educational lexical error messages
sections:
  - id: "06.1"
    title: Structured Error Types
    status: not-started
  - id: "06.2"
    title: Empathetic Messages
    status: not-started
  - id: "06.3"
    title: Common Mistake Detection
    status: not-started
  - id: "06.4"
    title: Error Recovery Strategies
    status: not-started
---

# Section 06: Error Handling

**Status:** 📋 Planned
**Goal:** Rich, educational lexical error messages
**Source:** Elm, Gleam (`compiler-core/src/parse/error.rs`)

---

## Background

### Current Ori Approach

```rust
TokenKind::Error  // No information about what went wrong
```

The current lexer returns a single `Error` token with no details about the error type.

### Elm/Gleam's Approach

Elm and Gleam produce some of the best error messages in any language:

```
-- UNEXPECTED CHARACTER ----------------------------------------- src/Main.elm

I ran into an unexpected character:

3|   let x = 5;
              ^
I was not expecting to see this semicolon. In Elm, we don't use semicolons
to end statements. You can simply remove it.
```

---

## 06.1 Structured Error Types

**Goal:** Detailed error types that capture what went wrong

### Tasks

- [ ] Define comprehensive `LexError` enum
  ```rust
  /// Lexical error with detailed information
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct LexError {
      pub kind: LexErrorKind,
      pub span: Span,
  }

  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum LexErrorKind {
      // === String Errors ===
      UnterminatedString {
          /// Position of opening quote
          start: u32,
      },
      UnterminatedRawString {
          start: u32,
          expected_hashes: u32,
      },
      InvalidStringEscape {
          escape_char: char,
      },
      InvalidHexEscape {
          found: String,  // The invalid hex sequence
      },
      InvalidUnicodeEscape {
          reason: UnicodeEscapeError,
      },
      StringNewline,  // Unescaped newline in string

      // === Character Errors ===
      EmptyCharLiteral,
      UnterminatedCharLiteral,
      MultiCharLiteral {
          /// Number of characters found
          char_count: usize,
      },
      InvalidCharEscape {
          escape_char: char,
      },

      // === Number Errors ===
      InvalidDigit {
          digit: char,
          base: NumBase,
      },
      EmptyExponent,
      LeadingZero,
      TrailingUnderscore,
      ConsecutiveUnderscores,
      FloatDuration,  // 1.5s is invalid
      FloatSize,      // 1.5kb is invalid

      // === Comment Errors ===
      UnterminatedBlockComment {
          start: u32,
          nesting_depth: u32,
      },

      // === Identifier Errors ===
      InvalidIdentifier {
          reason: IdentifierError,
      },

      // === General Errors ===
      UnexpectedChar {
          found: char,
      },
      UnexpectedByte {
          byte: u8,
      },
      InvalidUtf8 {
          position: u32,
      },

      // === Common Mistakes ===
      Semicolon,           // ; is not used in Ori
      SingleQuoteString,   // 'hello' should be "hello"
      TripleEquals,        // === should be ==
      CStyleComment,       // /* without matching */
      HashComment,         // # comment (Python style)
  }

  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum UnicodeEscapeError {
      MissingOpenBrace,
      MissingCloseBrace,
      EmptyEscape,
      TooManyDigits { count: usize },
      InvalidHexDigit { found: char },
      InvalidCodepoint { value: u32 },
      SurrogateCodepoint { value: u32 },
  }

  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum IdentifierError {
      StartsWithDigit,
      InvalidChar { char: char, suggestion: Option<char> },
      ReservedWord { word: String },
      Confusable { found: char, looks_like: char, name: &'static str },
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum NumBase {
      Binary,
      Octal,
      Decimal,
      Hex,
  }
  ```

- [ ] Store errors during tokenization
  ```rust
  pub struct Tokenizer<'a> {
      buffer: &'a [u8],
      index: usize,
      errors: Vec<LexError>,
  }

  impl<'a> Tokenizer<'a> {
      fn emit_error(&mut self, kind: LexErrorKind, span: Span) {
          self.errors.push(LexError { kind, span });
      }

      pub fn finish(self) -> (Vec<RawToken>, Vec<LexError>) {
          // Return both tokens and errors
          (self.tokens, self.errors)
      }
  }
  ```

---

## 06.2 Empathetic Messages

**Goal:** User-friendly, educational error messages

### Tasks

- [ ] Implement error message generation
  ```rust
  impl LexErrorKind {
      /// Get the error title (short description)
      pub fn title(&self) -> &'static str {
          match self {
              Self::UnterminatedString { .. } => "Unterminated String",
              Self::InvalidStringEscape { .. } => "Invalid Escape Sequence",
              Self::Semicolon => "Unexpected Semicolon",
              Self::SingleQuoteString => "Unexpected Single Quote",
              Self::TripleEquals => "Invalid Operator",
              Self::UnexpectedChar { .. } => "Unexpected Character",
              // ...
          }
      }

      /// Get the empathetic message (Elm style)
      pub fn message(&self) -> String {
          match self {
              Self::UnterminatedString { .. } => {
                  "I found a string that wasn't closed. Every opening quote \
                   needs a matching closing quote.".to_string()
              }

              Self::InvalidStringEscape { escape_char } => {
                  format!(
                      "I don't recognize the escape sequence '\\{}'. \
                       The valid escapes are: \\n, \\r, \\t, \\\\, \\\", \\', \
                       \\0, \\xHH, and \\u{{XXXX}}.",
                      escape_char
                  )
              }

              Self::Semicolon => {
                  "I found a semicolon, but Ori doesn't use semicolons. \
                   You can safely remove it - statements are separated by \
                   newlines instead.".to_string()
              }

              Self::SingleQuoteString => {
                  "I found a single quote here, but Ori uses double quotes \
                   for strings. Try changing ' to \" instead.".to_string()
              }

              Self::TripleEquals => {
                  "I found '===', but Ori uses '==' for equality comparison. \
                   Unlike JavaScript, there's no distinction between == and === \
                   in Ori.".to_string()
              }

              Self::UnexpectedChar { found } => {
                  format!(
                      "I ran into an unexpected character: '{}'",
                      found.escape_debug()
                  )
              }

              // ... more cases
          }
      }

      /// Get a hint for fixing the error
      pub fn hint(&self) -> Option<String> {
          match self {
              Self::Semicolon => Some("Remove the semicolon".to_string()),

              Self::SingleQuoteString => {
                  Some("Change single quotes to double quotes".to_string())
              }

              Self::TripleEquals => {
                  Some("Use == for equality comparison".to_string())
              }

              Self::InvalidStringEscape { escape_char: 'a' } => {
                  Some("Did you mean \\x07 for the alert/bell character?".to_string())
              }

              Self::InvalidStringEscape { escape_char: 'b' } => {
                  Some("Did you mean \\x08 for backspace?".to_string())
              }

              _ => None,
          }
      }

      /// Get a code suggestion for auto-fix
      pub fn suggestion(&self, source: &str, span: Span) -> Option<CodeSuggestion> {
          match self {
              Self::Semicolon => Some(CodeSuggestion {
                  span,
                  replacement: "".to_string(),
                  message: "Remove the semicolon".to_string(),
              }),

              Self::SingleQuoteString => {
                  let content = &source[span.start as usize + 1..span.end as usize - 1];
                  Some(CodeSuggestion {
                      span,
                      replacement: format!("\"{}\"", content),
                      message: "Use double quotes".to_string(),
                  })
              }

              Self::TripleEquals => Some(CodeSuggestion {
                  span,
                  replacement: "==".to_string(),
                  message: "Use == instead".to_string(),
              }),

              _ => None,
          }
      }
  }

  pub struct CodeSuggestion {
      pub span: Span,
      pub replacement: String,
      pub message: String,
  }
  ```

---

## 06.3 Common Mistake Detection

**Goal:** Detect and provide helpful messages for common mistakes

### Tasks

- [ ] Detect semicolons
  ```rust
  fn handle_start(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b';' => {
              self.emit_error(
                  LexErrorKind::Semicolon,
                  Span::new(self.index as u32, self.index as u32 + 1),
              );
              *tag = Tag::Error;
              self.advance();
              State::Done
          }
          // ...
      }
  }
  ```

- [ ] Detect single-quote strings
  ```rust
  fn handle_single_quote(&mut self, tag: &mut Tag) -> State {
      let start = self.index;
      self.advance(); // Skip opening '

      // Check if this looks like a string (not a char)
      let mut char_count = 0;
      while self.current() != b'\'' && self.current() != 0 && self.current() != b'\n' {
          char_count += 1;
          if self.current() == b'\\' {
              self.advance();
          }
          self.advance();
      }

      if char_count > 1 && self.current() == b'\'' {
          // This looks like 'string', not 'c'
          self.emit_error(
              LexErrorKind::SingleQuoteString,
              Span::new(start as u32, self.index as u32 + 1),
          );
          self.advance(); // Skip closing '
          *tag = Tag::Error;
      } else if char_count == 1 {
          // Valid char literal
          self.advance(); // Skip closing '
          *tag = Tag::Char;
      } else {
          // Error in char literal
          *tag = Tag::Error;
      }

      State::Done
  }
  ```

- [ ] Detect JavaScript/C patterns
  ```rust
  fn handle_equals(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b'=' => {
              self.advance();
              if self.current() == b'=' {
                  // === detected
                  self.emit_error(
                      LexErrorKind::TripleEquals,
                      Span::new(self.start as u32, self.index as u32 + 1),
                  );
                  self.advance();
                  *tag = Tag::Error;
              } else {
                  *tag = Tag::EqEq;
              }
          }
          b'>' => {
              self.advance();
              *tag = Tag::FatArrow;
          }
          _ => {
              *tag = Tag::Eq;
          }
      }
      State::Done
  }
  ```

- [ ] Detect confusable Unicode characters
  ```rust
  fn check_confusables(&mut self, c: char, pos: u32) {
      if let Some((name, replacement)) = check_confusable(c) {
          self.emit_error(
              LexErrorKind::InvalidIdentifier {
                  reason: IdentifierError::Confusable {
                      found: c,
                      looks_like: replacement,
                      name,
                  },
              },
              Span::point(pos),
          );
      }
  }
  ```

### Common Mistakes Table

| Mistake | Detection | Message |
|---------|-----------|---------|
| `;` | Direct check | "Ori doesn't use semicolons" |
| `'string'` | Multi-char in single quotes | "Use double quotes for strings" |
| `===` | Three equals | "Use == for equality" |
| `/* */` | Block comment start | "Use // for comments" |
| `#` | Hash at line start | "Use // for comments" |
| `return` | Keyword check | "Ori is expression-based, no return needed" |
| `null` | Keyword check | "Use nil instead of null" |
| `var` | Keyword check | "Use let for variable binding" |
| `function` | Keyword check | "Use fn for functions" |
| `class` | Keyword check | "Use type for type definitions" |

---

## 06.4 Error Recovery Strategies

**Goal:** Continue lexing after errors for better IDE support

### Tasks

- [ ] Implement skip-to-sync recovery
  ```rust
  /// Skip to a synchronization point after error
  fn synchronize(&mut self) {
      // Skip to next line or clear stopping point
      loop {
          match self.current() {
              0 => break,               // EOF
              b'\n' => {
                  self.advance();
                  break;
              }
              b'"' | b'\'' => {
                  // Try to recover at string boundary
                  self.skip_string();
                  break;
              }
              b'{' | b'}' | b'(' | b')' | b'[' | b']' => {
                  // Delimiter - good stopping point
                  break;
              }
              _ => {
                  self.advance();
              }
          }
      }
  }
  ```

- [ ] Implement string recovery
  ```rust
  fn handle_unterminated_string(&mut self, tag: &mut Tag, start: u32) -> State {
      // Emit error
      self.emit_error(
          LexErrorKind::UnterminatedString { start },
          Span::new(start, self.index as u32),
      );

      // Try to find end of string (might be on next line)
      // But don't consume more than one line
      *tag = Tag::UnterminatedString;
      State::Done
  }
  ```

- [ ] Implement comment recovery
  ```rust
  fn handle_unterminated_block_comment(&mut self, start: u32, depth: u32) {
      self.emit_error(
          LexErrorKind::UnterminatedBlockComment {
              start,
              nesting_depth: depth,
          },
          Span::new(start, self.index as u32),
      );

      // Continue to EOF - block comment consumes rest of file
  }
  ```

- [ ] Continue after errors
  ```rust
  impl Iterator for Tokenizer<'_> {
      type Item = RawToken;

      fn next(&mut self) -> Option<RawToken> {
          loop {
              let token = self.next_token();

              if token.tag == Tag::Eof {
                  return None;
              }

              // Always return tokens, even errors
              // Parser can decide what to do with them
              return Some(token);
          }
      }
  }
  ```

---

## 06.5 Completion Checklist

- [ ] `LexError` enum with all error types
- [ ] Empathetic messages for all errors
- [ ] Hints for common errors
- [ ] Code suggestions for auto-fix
- [ ] Semicolon detection
- [ ] Single-quote string detection
- [ ] Triple-equals detection
- [ ] Confusable character detection
- [ ] Error recovery implemented
- [ ] All errors tested

**Exit Criteria:**
- Every lexer error has a helpful message
- Common mistakes from other languages detected
- Lexer continues after errors
- IDE can display rich error information

---
section: "05"
title: Unicode & Escape Handling
status: not-started
goal: Comprehensive Unicode identifier and escape sequence support
sections:
  - id: "05.1"
    title: Unicode Identifiers (XID)
    status: not-started
  - id: "05.2"
    title: Extended Escape Sequences
    status: not-started
  - id: "05.3"
    title: String Interpolation
    status: not-started
  - id: "05.4"
    title: Raw Strings
    status: not-started
---

# Section 05: Unicode & Escape Handling

**Status:** 📋 Planned
**Goal:** Comprehensive Unicode identifier and escape sequence support
**Source:** Rust (`unicode-ident`), TypeScript, Roc

---

## Background

### Current Ori Approach

```rust
#[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
Ident,
```

Only ASCII identifiers supported. Basic escapes only (`\n`, `\r`, `\t`, `\\`, `\"`, `\'`, `\0`).

### Modern Language Support

| Feature | Rust | TypeScript | Go | Gleam | Ori (current) |
|---------|------|------------|-------|-------|---------------|
| Unicode identifiers | ✓ (XID) | ✓ | ✓ | ASCII | ASCII |
| `\xHH` hex escape | ✓ | ✓ | ✓ | ✓ | ✗ |
| `\u{XXXX}` unicode | ✓ | ✓ | ✗ | ✓ | ✗ |
| Raw strings | ✓ | ✗ | ✓ (`) | ✗ | ✗ |
| String interpolation | ✗ | ✓ | ✗ | ✗ | ✗ |

---

## 05.1 Unicode Identifiers (XID)

**Goal:** Support Unicode identifiers using XID_Start/XID_Continue

### Tasks

- [ ] Add `unicode-ident` dependency
  ```toml
  [dependencies]
  unicode-ident = "1.0"
  ```

- [ ] Implement Unicode identifier detection
  ```rust
  use unicode_ident::{is_xid_start, is_xid_continue};

  /// Check if character can start an identifier
  #[inline]
  fn is_ident_start(c: char) -> bool {
      c == '_' || is_xid_start(c)
  }

  /// Check if character can continue an identifier
  #[inline]
  fn is_ident_continue(c: char) -> bool {
      is_xid_continue(c)
  }
  ```

- [ ] Update tokenizer for UTF-8 handling
  ```rust
  fn handle_identifier(&mut self, tag: &mut Tag) -> State {
      // Consume identifier characters (may be multi-byte)
      loop {
          let c = self.current_char(); // UTF-8 decode

          if !is_ident_continue(c) {
              break;
          }

          self.advance_char(c); // Advance by char width
      }

      // Check for keyword (ASCII only - keywords are always ASCII)
      let ident = &self.buffer[self.start..self.index];
      if ident.is_ascii() {
          if let Some(kw_tag) = keyword_lookup(ident) {
              *tag = kw_tag;
          }
      }

      State::Done
  }

  /// Decode current position as UTF-8 char
  fn current_char(&self) -> char {
      let bytes = &self.buffer[self.index..];
      let (c, _) = decode_utf8_char(bytes).unwrap_or(('�', 1));
      c
  }

  /// Advance by character width
  fn advance_char(&mut self, c: char) {
      self.index += c.len_utf8();
  }
  ```

- [ ] Handle NFC normalization (like Rust)
  ```rust
  /// Normalize identifier to NFC form
  /// This ensures `café` == `café` (composed vs decomposed)
  fn normalize_identifier(ident: &str) -> String {
      use unicode_normalization::UnicodeNormalization;
      ident.nfc().collect()
  }
  ```

- [ ] Add confusables detection for errors
  ```rust
  /// Common confusable characters
  const CONFUSABLES: &[(char, &str, char)] = &[
      ('−', "Minus Sign", '-'),       // U+2212 vs U+002D
      ('＝', "Fullwidth Equals", '='), // U+FF1D vs U+003D
      ('"', "Left Double Quote", '"'), // U+201C vs U+0022
      ('"', "Right Double Quote", '"'),// U+201D vs U+0022
      ('０', "Fullwidth Zero", '0'),   // U+FF10 vs U+0030
      // ... many more
  ];

  fn check_confusable(c: char) -> Option<(&'static str, char)> {
      for (conf, name, replacement) in CONFUSABLES {
          if c == *conf {
              return Some((name, *replacement));
          }
      }
      None
  }
  ```

### Example Usage

```ori
// Unicode identifiers
let 名前 = "山田"
let prénom = "Jean"
let имя = "Иван"

// Emoji in strings (not identifiers)
let greeting = "Hello 👋"
```

---

## 05.2 Extended Escape Sequences

**Goal:** Support `\xHH` and `\u{XXXX}` escape sequences

### Tasks

- [ ] Implement hex escape (`\xHH`)
  ```rust
  fn handle_string_escape(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          // Basic escapes
          b'n' => { self.advance(); State::String }
          b'r' => { self.advance(); State::String }
          b't' => { self.advance(); State::String }
          b'\\' => { self.advance(); State::String }
          b'"' => { self.advance(); State::String }
          b'\'' => { self.advance(); State::String }
          b'0' => { self.advance(); State::String }

          // Hex escape: \xHH
          b'x' => {
              self.advance();
              State::StringHexEscape
          }

          // Unicode escape: \u{XXXX}
          b'u' => {
              self.advance();
              State::StringUnicodeEscape
          }

          _ => {
              *tag = Tag::InvalidEscape;
              self.advance();
              State::String
          }
      }
  }

  fn handle_string_hex_escape(&mut self, tag: &mut Tag) -> State {
      // Expect exactly 2 hex digits
      let d1 = self.current();
      if !d1.is_ascii_hexdigit() {
          *tag = Tag::InvalidEscape;
          return State::String;
      }
      self.advance();

      let d2 = self.current();
      if !d2.is_ascii_hexdigit() {
          *tag = Tag::InvalidEscape;
          return State::String;
      }
      self.advance();

      // Value must be valid byte (0x00-0xFF) - always true for 2 hex digits
      State::String
  }
  ```

- [ ] Implement Unicode escape (`\u{XXXX}`)
  ```rust
  fn handle_string_unicode_escape(&mut self, tag: &mut Tag) -> State {
      // Expect opening brace
      if self.current() != b'{' {
          *tag = Tag::InvalidEscape;
          return State::String;
      }
      self.advance();

      // Consume hex digits (1-6)
      let start = self.index;
      while self.current().is_ascii_hexdigit() {
          self.advance();
      }
      let hex_len = self.index - start;

      // Validate length
      if hex_len == 0 || hex_len > 6 {
          *tag = Tag::InvalidEscape;
          // Skip to closing brace or end
          while self.current() != b'}' && self.current() != 0 {
              self.advance();
          }
          if self.current() == b'}' {
              self.advance();
          }
          return State::String;
      }

      // Expect closing brace
      if self.current() != b'}' {
          *tag = Tag::InvalidEscape;
          return State::String;
      }
      self.advance();

      // Validate codepoint (done at a higher level with actual value)
      State::String
  }
  ```

- [ ] Implement escape processing in token processor
  ```rust
  /// Process escape sequences in string content
  pub fn unescape_string(s: &str) -> Result<String, EscapeError> {
      let mut result = String::with_capacity(s.len());
      let mut chars = s.chars().peekable();

      while let Some(c) = chars.next() {
          if c != '\\' {
              result.push(c);
              continue;
          }

          match chars.next() {
              Some('n') => result.push('\n'),
              Some('r') => result.push('\r'),
              Some('t') => result.push('\t'),
              Some('\\') => result.push('\\'),
              Some('"') => result.push('"'),
              Some('\'') => result.push('\''),
              Some('0') => result.push('\0'),

              Some('x') => {
                  let hex: String = chars.by_ref().take(2).collect();
                  let value = u8::from_str_radix(&hex, 16)
                      .map_err(|_| EscapeError::InvalidHex)?;
                  result.push(value as char);
              }

              Some('u') => {
                  if chars.next() != Some('{') {
                      return Err(EscapeError::InvalidUnicode);
                  }
                  let hex: String = chars.by_ref()
                      .take_while(|&c| c != '}')
                      .collect();
                  let codepoint = u32::from_str_radix(&hex, 16)
                      .map_err(|_| EscapeError::InvalidUnicode)?;
                  let c = char::from_u32(codepoint)
                      .ok_or(EscapeError::InvalidCodepoint(codepoint))?;
                  result.push(c);
              }

              Some(c) => return Err(EscapeError::Unknown(c)),
              None => return Err(EscapeError::UnexpectedEnd),
          }
      }

      Ok(result)
  }
  ```

### Example Usage

```ori
let tab = "\t"           // Basic escape
let byte = "\x1B"        // Hex escape (ESC)
let emoji = "\u{1F600}"  // Unicode escape (😀)
let mixed = "Hello\u{2764}World"  // Hello❤World
```

---

## 05.3 String Interpolation

**Goal:** Support `$"..."` or `"...${expr}..."` string interpolation

### Tasks

- [ ] Design interpolation syntax
  ```ori
  // Option A: Template prefix (like C#, TypeScript)
  let msg = $"Hello, {name}!"
  let calc = $"Sum: {a + b}"

  // Option B: Dollar in string (like Kotlin, Roc)
  let msg = "Hello, ${name}!"
  let simple = "Hello, $name!"  // Simple variable only

  // Recommendation: Option B (more familiar, no prefix needed)
  ```

- [ ] Implement interpolation tokenization
  ```rust
  // When entering string and seeing $
  fn handle_string(&mut self, tag: &mut Tag) -> State {
      loop {
          match self.current() {
              b'"' => {
                  self.advance();
                  return State::Done;
              }

              b'$' => {
                  if self.peek() == b'{' {
                      // End current string segment
                      *tag = Tag::StringHead; // or StringMiddle
                      return State::Done;
                      // Next call will return InterpolationStart
                  }
                  self.advance();
              }

              // ... other cases
          }
      }
  }

  // Stack-based interpolation tracking
  struct Tokenizer<'a> {
      buffer: &'a [u8],
      index: usize,
      interpolation_stack: Vec<InterpolationKind>,
  }

  enum InterpolationKind {
      DoubleQuote,   // "...${...}..."
      // Future: MultilineString, etc.
  }
  ```

- [ ] Define interpolation tokens
  ```rust
  enum Tag {
      // ...
      StringHead,          // "Hello, ${ - start of interpolated string
      StringMiddle,        // }...${ - middle segment
      StringTail,          // }!" - end of interpolated string
      InterpolationStart,  // ${
      InterpolationEnd,    // } (when in interpolation context)
  }
  ```

- [ ] Handle nested interpolation
  ```rust
  // "outer ${x + "inner ${y}"}" should work
  fn handle_close_brace(&mut self, tag: &mut Tag) -> State {
      if let Some(kind) = self.interpolation_stack.pop() {
          *tag = Tag::InterpolationEnd;
          // Resume string scanning
          self.resume_string(kind);
          State::String
      } else {
          *tag = Tag::RBrace;
          State::Done
      }
  }
  ```

---

## 05.4 Raw Strings

**Goal:** Support raw strings with no escape processing

### Tasks

- [ ] Design raw string syntax
  ```ori
  // Option A: r prefix (like Rust, Python)
  let path = r"C:\Users\name\file.txt"
  let regex = r"\d+\.\d+"

  // Option B: Backticks (like Go, JavaScript)
  let path = `C:\Users\name\file.txt`

  // Recommendation: Option A (r prefix)
  // Backticks reserved for future template literals
  ```

- [ ] Implement raw string tokenization
  ```rust
  fn handle_start(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b'r' => {
              if self.peek() == b'"' {
                  self.advance(); // Skip 'r'
                  self.advance(); // Skip '"'
                  *tag = Tag::RawString;
                  State::RawString
              } else {
                  // Regular identifier starting with 'r'
                  *tag = Tag::Ident;
                  self.advance();
                  State::Identifier
              }
          }
          // ... other cases
      }
  }

  fn handle_raw_string(&mut self, tag: &mut Tag) -> State {
      loop {
          match self.current() {
              0 | b'\n' => {
                  // Unterminated
                  *tag = Tag::UnterminatedString;
                  return State::Done;
              }
              b'"' => {
                  self.advance();
                  return State::Done;
              }
              _ => {
                  // No escape processing!
                  self.advance();
              }
          }
      }
  }
  ```

- [ ] Support multi-hash raw strings (like Rust)
  ```ori
  // For strings containing "
  let json = r#"{"key": "value"}"#
  let nested = r##"has "# inside"##
  ```

  ```rust
  fn handle_raw_string_hashes(&mut self, tag: &mut Tag) -> State {
      // Count opening hashes
      let mut hashes = 0;
      while self.current() == b'#' {
          hashes += 1;
          self.advance();
      }

      if self.current() != b'"' {
          *tag = Tag::Error;
          return State::Done;
      }
      self.advance();

      // Scan until closing " followed by same number of #
      loop {
          match self.current() {
              0 => {
                  *tag = Tag::UnterminatedString;
                  return State::Done;
              }
              b'"' => {
                  self.advance();
                  let mut closing_hashes = 0;
                  while self.current() == b'#' && closing_hashes < hashes {
                      closing_hashes += 1;
                      self.advance();
                  }
                  if closing_hashes == hashes {
                      return State::Done;
                  }
                  // Not enough hashes, continue scanning
              }
              _ => {
                  self.advance();
              }
          }
      }
  }
  ```

---

## 05.5 Completion Checklist

- [ ] Unicode identifiers via XID
- [ ] NFC normalization
- [ ] Confusables detection
- [ ] `\xHH` hex escapes
- [ ] `\u{XXXX}` unicode escapes
- [ ] String interpolation
- [ ] Raw strings
- [ ] Multi-hash raw strings
- [ ] All existing tests pass
- [ ] New feature tests added

**Exit Criteria:**
- Unicode identifiers work correctly
- All escape sequences processed correctly
- Interpolation nesting works
- Raw strings preserve content exactly

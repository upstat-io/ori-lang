---
section: "01"
title: Two-Layer Architecture
status: not-started
goal: Separate pure tokenization from compiler integration for reusability
sections:
  - id: "01.1"
    title: Low-Level Tokenizer Crate
    status: not-started
  - id: "01.2"
    title: High-Level Processor
    status: not-started
  - id: "01.3"
    title: Crate Boundary Design
    status: not-started
  - id: "01.4"
    title: API Stability Guarantees
    status: not-started
---

# Section 01: Two-Layer Architecture

**Status:** 📋 Planned
**Goal:** Separate pure tokenization from compiler integration for reusability
**Source:** Rust (`rustc_lexer` / `rustc_parse::lexer`)

---

## Background

Rust's compiler has a brilliant two-layer lexer design:

1. **`rustc_lexer`** — Low-level, pure, no compiler dependencies
   - Works on raw `&str`
   - Returns simple tokens: kind + length
   - No spans, no interning, no errors
   - Stable API, usable by rust-analyzer

2. **`rustc_parse::lexer`** — High-level, compiler-integrated
   - "Cooks" raw tokens into AST tokens
   - Adds spans, interns symbols
   - Emits diagnostics
   - Edition-aware behavior

This enables code reuse across tools while keeping the compiler's lexer optimized.

---

## 01.1 Low-Level Tokenizer Crate

**Goal:** Create `ori_lexer_core` — a standalone, pure tokenization crate

### Tasks

- [ ] Create new crate `compiler/ori_lexer_core/`
  ```
  compiler/ori_lexer_core/
  ├── Cargo.toml
  └── src/
      ├── lib.rs        # Public API
      ├── tokenizer.rs  # State machine
      ├── tag.rs        # Token tag enum
      └── cursor.rs     # Byte cursor
  ```

- [ ] Design minimal dependencies
  ```toml
  [dependencies]
  # No ori_* dependencies!
  # No interner, no spans, no diagnostics
  ```

- [ ] Define core types
  ```rust
  /// Raw token from low-level tokenizer
  pub struct RawToken {
      pub tag: Tag,
      pub len: u32,  // Byte length only
  }

  /// Token kind without semantic payload
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  #[repr(u8)]
  pub enum Tag {
      // Literals
      Ident,
      Int,
      Float,
      String,
      Char,

      // Keywords (separate variants for fast matching)
      KwLet,
      KwFn,
      KwIf,
      // ... all keywords

      // Operators
      Plus,
      Minus,
      Star,
      // ... all operators

      // Errors (flag-based, not diagnostic)
      InvalidStringEscape,
      UnterminatedString,
      UnterminatedComment,
      InvalidChar,
      Unknown,

      Eof,
  }
  ```

- [ ] Implement main entry point
  ```rust
  /// Tokenize entire source, yielding raw tokens
  pub fn tokenize(source: &str) -> impl Iterator<Item = RawToken> + '_ {
      Tokenizer::new(source.as_bytes())
  }

  /// Get the lexeme for a fixed token
  impl Tag {
      pub fn lexeme(self) -> Option<&'static str> {
          match self {
              Tag::Plus => Some("+"),
              Tag::KwLet => Some("let"),
              // Variable-length tokens return None
              Tag::Ident | Tag::Int | Tag::String => None,
              // ...
          }
      }
  }
  ```

- [ ] Ensure no panics in public API
  - All errors become `Tag::Unknown` or specific error tags
  - Invalid UTF-8 handled gracefully

### Validation

```rust
#[test]
fn no_compiler_dependencies() {
    // Verify ori_lexer_core compiles with just std
    // This is a build-time check via CI
}

#[test]
fn tokenize_simple() {
    let tokens: Vec<_> = tokenize("let x = 42").collect();
    assert_eq!(tokens[0].tag, Tag::KwLet);
    assert_eq!(tokens[0].len, 3);
}
```

---

## 01.2 High-Level Processor

**Goal:** Rewrite `ori_lexer` to process raw tokens into compiler tokens

**NOTE:** This completely replaces the current `ori_lexer` implementation. All existing files (`raw_token.rs`, `convert.rs`, etc.) are deleted and rewritten.

### Tasks

- [ ] Delete current implementation
  - [ ] Remove `raw_token.rs` (Logos-derived)
  - [ ] Remove `convert.rs` (token conversion)
  - [ ] Remove `escape.rs` (moved to new design)
  - [ ] Remove `parse_helpers.rs` (integrated into tokenizer)
  - [ ] Remove Logos from `Cargo.toml`

- [ ] Add dependency on `ori_lexer_core`
  ```toml
  [dependencies]
  ori_lexer_core = { path = "../ori_lexer_core" }
  ori_ir = { path = "../ori_ir" }
  # NO logos dependency
  ```

- [ ] Create token "cooking" layer
  ```rust
  /// Convert raw tokens to compiler tokens
  pub struct TokenProcessor<'a> {
      source: &'a str,
      raw_tokens: impl Iterator<Item = RawToken>,
      interner: &'a StringInterner,
      pos: u32,  // Current byte position
  }

  impl<'a> TokenProcessor<'a> {
      pub fn process(&mut self) -> Token {
          let raw = self.raw_tokens.next()?;
          let start = self.pos;
          let end = start + raw.len;
          self.pos = end;

          let span = Span::new(start, end);
          let kind = self.cook_token(raw.tag, start, end);

          Token::new(kind, span)
      }

      fn cook_token(&self, tag: Tag, start: u32, end: u32) -> TokenKind {
          match tag {
              Tag::Ident => {
                  let text = &self.source[start as usize..end as usize];
                  let name = self.interner.get_or_intern(text);
                  TokenKind::Ident(name)
              }
              Tag::Int => {
                  let text = &self.source[start as usize..end as usize];
                  let value = parse_int(text);
                  TokenKind::Int(value)
              }
              Tag::KwLet => TokenKind::Let,
              // ... etc
          }
      }
  }
  ```

- [ ] Handle trivia (comments, whitespace)
  ```rust
  pub struct TokenProcessor<'a> {
      // ... existing fields
      trivia_buffer: Vec<Trivia>,
      preserve_trivia: bool,
  }

  pub struct Trivia {
      pub kind: TriviaKind,
      pub span: Span,
  }

  pub enum TriviaKind {
      Whitespace,
      Newline,
      LineComment,
      BlockComment,
      DocComment,
  }
  ```

- [ ] Emit diagnostics for lexical errors
  ```rust
  fn cook_token(&self, tag: Tag, ...) -> TokenKind {
      match tag {
          Tag::InvalidStringEscape => {
              self.emit_error(LexError::InvalidEscape { span });
              TokenKind::Error
          }
          Tag::UnterminatedString => {
              self.emit_error(LexError::UnterminatedString { start: span.start });
              TokenKind::Error
          }
          // ...
      }
  }
  ```

### Validation

```rust
#[test]
fn cooking_interns_identifiers() {
    let interner = StringInterner::new();
    let tokens = process("foo bar foo", &interner);

    // Same identifier interned once
    let TokenKind::Ident(n1) = tokens[0].kind;
    let TokenKind::Ident(n2) = tokens[2].kind;
    assert_eq!(n1, n2);  // Same Name
}
```

---

## 01.3 Crate Boundary Design

**Goal:** Define clean interfaces between layers

### Tasks

- [ ] Document public API of `ori_lexer_core`
  ```rust
  //! # ori_lexer_core
  //!
  //! Low-level tokenizer for Ori source code.
  //!
  //! This crate is designed to be:
  //! - **Pure**: No side effects, no global state
  //! - **Minimal**: No dependencies on compiler internals
  //! - **Stable**: API suitable for external tools
  //!
  //! ## Usage
  //!
  //! ```rust
  //! use ori_lexer_core::{tokenize, Tag};
  //!
  //! for token in tokenize("let x = 42") {
  //!     println!("{:?}: {} bytes", token.tag, token.len);
  //! }
  //! ```
  ```

- [ ] Define what belongs in each layer

  | Concern | `ori_lexer_core` | `ori_lexer` |
  |---------|------------------|-------------|
  | Tokenization | ✓ | |
  | Token tags | ✓ | |
  | Byte lengths | ✓ | |
  | Spans | | ✓ |
  | Interning | | ✓ |
  | Number parsing | | ✓ |
  | Error messages | | ✓ |
  | Trivia handling | | ✓ |
  | Salsa integration | | ✓ |

- [ ] Create re-exports in `ori_lexer`
  ```rust
  // ori_lexer/src/lib.rs
  pub use ori_lexer_core::{Tag, RawToken, tokenize as tokenize_raw};

  // High-level API
  pub fn lex(source: &str, interner: &StringInterner) -> TokenList { ... }
  ```

---

## 01.4 API Stability Guarantees

**Goal:** Define stability expectations for external users

### Tasks

- [ ] Mark `ori_lexer_core` as stable-ish
  ```toml
  # Cargo.toml
  [package]
  name = "ori_lexer_core"
  version = "0.1.0"
  # Note: API may change until Ori 1.0
  ```

- [ ] Document breaking change policy
  ```rust
  //! ## Stability
  //!
  //! - `Tag` enum: Variants may be added (non-exhaustive)
  //! - `RawToken` struct: Fields are stable
  //! - `tokenize()`: Signature is stable
  //! - Error tags: May be refined (new error kinds)
  ```

- [ ] Add `#[non_exhaustive]` where appropriate
  ```rust
  #[non_exhaustive]
  #[repr(u8)]
  pub enum Tag {
      // ...
  }
  ```

- [ ] Create stability tests
  ```rust
  #[test]
  fn api_stability() {
      // These patterns must continue to work
      let tok = RawToken { tag: Tag::Ident, len: 3 };
      assert!(tok.tag == Tag::Ident);
      assert!(tok.len == 3);

      // Tag must have lexeme method
      assert_eq!(Tag::Plus.lexeme(), Some("+"));
  }
  ```

---

## 01.5 Completion Checklist

- [ ] `ori_lexer_core` crate created
- [ ] No compiler dependencies in core crate
- [ ] `ori_lexer` uses core crate
- [ ] Token cooking layer implemented
- [ ] Trivia handling in high-level layer
- [ ] Error messages in high-level layer
- [ ] API documented
- [ ] Stability expectations documented
- [ ] All existing tests pass

**Exit Criteria:**
- `ori_lexer_core` can be used standalone (e.g., for syntax highlighting)
- `ori_lexer` produces identical output to current implementation
- No performance regression

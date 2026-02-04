---
section: "03"
title: State Machine Design
status: not-started
goal: Hand-written state machine for full control over tokenization
sections:
  - id: "03.1"
    title: Labeled Switch Pattern
    status: not-started
  - id: "03.2"
    title: Sentinel-Terminated Buffers
    status: not-started
  - id: "03.3"
    title: State Enum Design
    status: not-started
  - id: "03.4"
    title: Logos Migration Path
    status: not-started
---

# Section 03: State Machine Design

**Status:** 📋 Planned
**Goal:** Hand-written state machine for full control over tokenization
**Source:** Zig (`lib/std/zig/tokenizer.zig`)

---

## Background

### Current Ori Approach (Logos)

```rust
#[derive(Logos)]
enum RawToken {
    #[token("let")]
    Let,
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
    // ...
}
```

**Pros:** Fast, declarative, auto-generated DFA
**Cons:** Limited control, no context-sensitivity, black-box errors

### Zig's Approach (Hand-Written State Machine)

```zig
state: switch (State.start) {
    .start => switch (self.buffer[self.index]) {
        'a'...'z' => continue :state .identifier,
        '"' => continue :state .string_literal,
        // ...
    },
    .identifier => {
        // ...
    },
}
```

**Pros:** Full control, optimal code gen, context-aware, predictable
**Cons:** More code to maintain

---

## 03.1 Labeled Switch Pattern

**Goal:** Implement Zig-style state machine in Rust

### Tasks

- [ ] Design the state machine structure
  ```rust
  pub struct Tokenizer<'a> {
      buffer: &'a [u8],
      index: usize,
  }

  impl<'a> Tokenizer<'a> {
      pub fn next(&mut self) -> RawToken {
          let start = self.index;
          let mut tag = Tag::Eof;

          // State machine using loop + match
          let mut state = State::Start;
          loop {
              state = match state {
                  State::Start => self.handle_start(&mut tag),
                  State::Identifier => self.handle_identifier(&mut tag),
                  State::String => self.handle_string(&mut tag),
                  State::Number => self.handle_number(&mut tag),
                  State::Operator => self.handle_operator(&mut tag),
                  State::Done => break,
              };
          }

          RawToken {
              tag,
              len: (self.index - start) as u32,
          }
      }
  }
  ```

- [ ] Implement `handle_start` - the entry point
  ```rust
  fn handle_start(&mut self, tag: &mut Tag) -> State {
      let c = self.current();

      match c {
          0 => {
              // EOF (sentinel)
              *tag = Tag::Eof;
              State::Done
          }

          // Whitespace - skip and restart
          b' ' | b'\t' => {
              self.advance();
              State::Start
          }

          // Newline
          b'\n' => {
              *tag = Tag::Newline;
              self.advance();
              State::Done
          }

          // Identifiers and keywords
          b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
              *tag = Tag::Ident;
              self.advance();
              State::Identifier
          }

          // Numbers
          b'0'..=b'9' => {
              *tag = Tag::Int;
              self.advance();
              State::Number
          }

          // String
          b'"' => {
              *tag = Tag::String;
              self.advance();
              State::String
          }

          // Single-char operators (no state transition needed)
          b'(' => {
              *tag = Tag::LParen;
              self.advance();
              State::Done
          }
          b')' => {
              *tag = Tag::RParen;
              self.advance();
              State::Done
          }

          // Multi-char operators
          b'+' => {
              self.advance();
              State::Plus
          }
          b'-' => {
              self.advance();
              State::Minus
          }
          b'=' => {
              self.advance();
              State::Equals
          }

          // Unknown
          _ => {
              *tag = Tag::Error;
              self.advance();
              State::Done
          }
      }
  }
  ```

- [ ] Implement identifier handling with keyword lookup
  ```rust
  fn handle_identifier(&mut self, tag: &mut Tag) -> State {
      // Consume identifier characters
      while self.is_ident_continue(self.current()) {
          self.advance();
      }

      // Check for keyword
      let ident = &self.buffer[self.start..self.index];
      if let Some(kw_tag) = keyword_lookup(ident) {
          *tag = kw_tag;
      }

      State::Done
  }

  #[inline]
  fn is_ident_continue(&self, c: u8) -> bool {
      matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
  }
  ```

- [ ] Implement multi-char operator handling
  ```rust
  fn handle_plus(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b'=' => {
              *tag = Tag::PlusEq;
              self.advance();
          }
          _ => {
              *tag = Tag::Plus;
          }
      }
      State::Done
  }

  fn handle_equals(&mut self, tag: &mut Tag) -> State {
      match self.current() {
          b'=' => {
              *tag = Tag::EqEq;
              self.advance();
          }
          b'>' => {
              *tag = Tag::FatArrow;
              self.advance();
          }
          _ => {
              *tag = Tag::Eq;
          }
      }
      State::Done
  }
  ```

### Why This Pattern Works

1. **Compiler optimizes** the match into a jump table
2. **No allocations** in the hot path
3. **Predictable branches** for common cases
4. **Full control** over every state transition

---

## 03.2 Sentinel-Terminated Buffers

**Goal:** Eliminate bounds checks with sentinel value

### Tasks

- [ ] Design buffer API with sentinel
  ```rust
  impl<'a> Tokenizer<'a> {
      /// Create tokenizer with sentinel-terminated buffer
      pub fn new(source: &'a str) -> Self {
          // SAFETY: We ensure buffer ends with sentinel
          let buffer = Self::ensure_sentinel(source);
          Tokenizer {
              buffer,
              index: 0,
          }
      }

      /// Add sentinel byte if not present
      fn ensure_sentinel(source: &str) -> &[u8] {
          let bytes = source.as_bytes();
          // Rely on Rust strings being null-terminated in memory
          // or allocate a buffer with explicit sentinel
          bytes
      }

      /// Get current byte - no bounds check needed
      #[inline]
      fn current(&self) -> u8 {
          // SAFETY: Buffer is sentinel-terminated
          // When we hit sentinel (0), we return Eof
          self.buffer[self.index]
      }

      /// Advance position
      #[inline]
      fn advance(&mut self) {
          self.index += 1;
      }
  }
  ```

- [ ] Handle sentinel in state machine
  ```rust
  fn handle_start(&mut self, tag: &mut Tag) -> State {
      let c = self.current();

      if c == 0 {
          // Sentinel = end of input
          if self.index == self.buffer.len() - 1 {
              *tag = Tag::Eof;
              return State::Done;
          }
          // Embedded NUL = error
          *tag = Tag::Error;
          self.advance();
          return State::Done;
      }

      // ... rest of handling
  }
  ```

- [ ] Benchmark: With vs without bounds checks
  ```rust
  #[bench]
  fn tokenize_with_bounds_check(b: &mut Bencher) {
      let source = include_str!("large_file.ori");
      b.iter(|| tokenize_checked(source).count());
  }

  #[bench]
  fn tokenize_sentinel(b: &mut Bencher) {
      let source = include_str!("large_file.ori");
      b.iter(|| tokenize_sentinel(source).count());
  }
  ```

### Performance Impact

| Operation | With Bounds Check | Sentinel |
|-----------|-------------------|----------|
| Byte access | `if index < len` | Direct |
| Instructions | 3+ | 1 |
| Branch prediction | Miss on EOF | Predictable |

---

## 03.3 State Enum Design

**Goal:** Define all tokenizer states

### Tasks

- [ ] Define `State` enum
  ```rust
  /// Tokenizer state machine states
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  enum State {
      // Entry point
      Start,
      Done,

      // Identifiers
      Identifier,

      // Numbers
      Number,
      NumberAfterDot,       // Might be float or range
      NumberExponent,       // Scientific notation
      NumberHex,            // 0x...
      NumberBinary,         // 0b...
      NumberOctal,          // 0o...

      // Strings
      String,
      StringEscape,
      StringUnicodeEscape,
      RawString,            // r"..."
      MultilineString,      // """..."""

      // Characters
      Char,
      CharEscape,

      // Comments
      LineComment,
      BlockComment,
      DocComment,

      // Operators (need lookahead)
      Plus,         // + or +=
      Minus,        // - or -= or ->
      Star,         // * or *=
      Slash,        // / or /= or // or /*
      Equals,       // = or == or =>
      Bang,         // ! or !=
      Lt,           // < or <= or <<
      Gt,           // > or >= or >>
      Ampersand,    // & or &&
      Pipe,         // | or || or |>
      Dot,          // . or .. or ...
      Colon,        // : or ::
  }
  ```

- [ ] Document state transitions
  ```
  State Transition Diagram:

  Start ─┬─► Identifier ──► Done (with keyword check)
         │
         ├─► Number ─┬─► NumberAfterDot ──► Done (float)
         │           └─► NumberHex/Bin/Oct ──► Done
         │
         ├─► String ─┬─► StringEscape ──► String
         │           └─► Done (on closing quote)
         │
         ├─► Slash ─┬─► LineComment ──► Done
         │          ├─► BlockComment ─┬─► Done
         │          │                 └─► BlockComment (nested)
         │          └─► Done (just /)
         │
         ├─► [Operators] ──► Done (with lookahead)
         │
         └─► Done (single-char tokens)
  ```

- [ ] Implement each state handler as a method
  ```rust
  impl Tokenizer<'_> {
      fn handle_string(&mut self, tag: &mut Tag) -> State {
          loop {
              match self.current() {
                  0 | b'\n' => {
                      // Unterminated
                      *tag = Tag::UnterminatedString;
                      return State::Done;
                  }
                  b'"' => {
                      // End of string
                      self.advance();
                      return State::Done;
                  }
                  b'\\' => {
                      self.advance();
                      return State::StringEscape;
                  }
                  _ => {
                      self.advance();
                  }
              }
          }
      }

      fn handle_string_escape(&mut self, tag: &mut Tag) -> State {
          match self.current() {
              b'n' | b'r' | b't' | b'\\' | b'"' | b'0' => {
                  self.advance();
                  State::String
              }
              b'u' => {
                  self.advance();
                  State::StringUnicodeEscape
              }
              _ => {
                  // Invalid escape - mark error but continue
                  *tag = Tag::InvalidEscape;
                  self.advance();
                  State::String
              }
          }
      }
  }
  ```

---

## 03.4 Logos Replacement

**Goal:** Completely replace Logos with hand-written state machine

**NOTE:** This is a **full replacement**, not a migration. The old Logos-based lexer will be deleted entirely. No backwards compatibility layer is needed.

### Tasks

- [ ] Delete current Logos-based implementation
  - [ ] Remove `compiler/ori_lexer/src/raw_token.rs` (Logos derive)
  - [ ] Remove `compiler/ori_lexer/src/convert.rs` (token conversion)
  - [ ] Remove Logos dependency from `Cargo.toml`

- [ ] Replace with new implementation
  - [ ] New `ori_lexer_core` crate (pure tokenizer)
  - [ ] Updated `ori_lexer` crate (uses core, adds interning/spans)
  - [ ] New `TokenStorage` replaces `TokenList`

- [ ] Update all consumers
  - [ ] `ori_parse` - update to use new token API
  - [ ] `ori_typeck` - if any direct token access
  - [ ] `oric` - CLI token display/debug
  - [ ] Tests - all lexer tests rewritten for new API

- [ ] Verify correctness via spec tests
  ```rust
  // Tests are based on language SPEC, not old implementation
  // If old lexer had bugs, new lexer should fix them
  #[test]
  fn spec_string_escapes() {
      // Test against spec, not "what Logos did"
      assert_tokens(r#""\n\t\r""#, &[Tag::String]);
      assert_tokens(r#""\u{1F600}""#, &[Tag::String]);
  }
  ```

- [ ] Benchmark new implementation
  ```rust
  #[bench]
  fn bench_new_lexer(b: &mut Bencher) {
      let source = include_str!("test_data/large.ori");
      b.iter(|| tokenize(source).count());
  }
  ```

### What Gets Deleted

| File/Item | Reason |
|-----------|--------|
| `logos` dependency | Replaced by hand-written |
| `RawToken` enum (Logos-derived) | Replaced by `Tag` enum |
| `convert.rs` | Integrated into new design |
| `TokenKind` (current) | Replaced by `Tag` + `TokenValue` |
| `Token` struct (24 bytes) | Replaced by `TokenStorage` (SoA) |

### No Backwards Compatibility

- Old `TokenKind` enum is **deleted**, not deprecated
- Old `Token` struct is **deleted**, not wrapped
- Old `lex()` function signature **changes**
- Parser must be updated to new API (not optional)

---

## 03.5 Completion Checklist

- [ ] State machine core implemented
- [ ] All token types handled
- [ ] Sentinel buffer optimization
- [ ] State enum complete
- [ ] Old Logos implementation deleted
- [ ] Logos dependency removed from Cargo.toml
- [ ] All spec tests pass with new lexer
- [ ] Parser updated to new token API
- [ ] Performance benchmarked
- [ ] Documentation complete

**Exit Criteria:**
- New lexer passes all spec-based tests
- No Logos code or dependency remains
- Parser fully integrated with new lexer
- Clean, maintainable code

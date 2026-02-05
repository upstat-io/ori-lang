---
section: "02"
title: Compact Token Representation
status: not-started
goal: Minimize token size while maximizing information density
sections:
  - id: "02.1"
    title: 8-Byte Raw Tokens
    status: not-started
  - id: "02.2"
    title: No End Offset Storage
    status: not-started
  - id: "02.3"
    title: Structure-of-Arrays Storage
    status: not-started
  - id: "02.4"
    title: TokenFlags Bitfield
    status: not-started
  - id: "02.5"
    title: Lazy Line/Column Computation
    status: not-started
---

# Section 02: Compact Token Representation

**Status:** 📋 Planned
**Goal:** Minimize token size while maximizing information density
**Source:** Zig (`lib/std/zig/Ast.zig`), TypeScript, Roc

> **Conventions:** Follows `plans/v2-conventions.md` §1 (Index Types), §2 (Tags), §3 (SoA Containers), §4 (Flags)

---

## 02.0 TokenIdx — Typed Index into Token Storage

**Goal:** Strongly-typed index following the conventions §1 pattern

`TokenIdx` is defined in `ori_ir` (not in `ori_lexer`) because the parser and other phases reference tokens by index.

```rust
// Defined in ori_ir
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct TokenIdx(u32);

impl TokenIdx {
    pub const NONE: Self = Self(u32::MAX);

    #[inline]
    pub const fn from_raw(raw: u32) -> Self { Self(raw) }

    #[inline]
    pub const fn raw(self) -> u32 { self.0 }
}

const _: () = assert!(std::mem::size_of::<TokenIdx>() == 4);
```

---

## Background

### Current Ori Token (24 bytes)
```rust
pub struct Token {
    pub kind: TokenKind,  // 16 bytes (enum with String, u64 payloads)
    pub span: Span,       // 8 bytes (start: u32, end: u32)
}
```

### Zig's Approach (5 bytes per token!)
```zig
pub const Token = struct {
    tag: Tag,       // 1 byte
    start: u32,     // 4 bytes (no end!)
};
```

Zig recomputes `end` on demand via `Tag.lexeme()` or re-tokenization.

### Target: 8-Byte Tokens + SoA Storage
```rust
// Raw token in low-level layer (ori_lexer_core)
struct RawToken {
    tag: RawTag,   // 1 byte
    _pad: [u8; 3], // 3 bytes alignment
    start: u32,    // 4 bytes
}  // Total: 8 bytes

// High-level storage uses SoA (ori_lexer, uses TokenTag from ori_ir)
struct TokenStorage {
    tags: Vec<TokenTag>,      // 1 byte each
    starts: Vec<u32>,         // 4 bytes each
    values: Vec<TokenValue>,  // Variable, only for literals
    flags: Vec<TokenFlags>,   // 1 byte each
}
```

---

## 02.1 8-Byte Raw Tokens

**Goal:** Define minimal token structure for low-level layer

### Tasks

- [ ] Define `RawToken` structure
  ```rust
  /// Raw token from tokenizer - 8 bytes
  #[derive(Clone, Copy, Debug)]
  #[repr(C)]
  pub struct RawToken {
      /// Token kind (see conventions §2 — standalone, no ori_* deps)
      pub tag: RawTag,
      /// Padding for alignment
      _pad: [u8; 3],
      /// Byte length (not end position!)
      pub len: u32,
  }

  // Compile-time size assertion
  const _: () = assert!(std::mem::size_of::<RawToken>() == 8);
  ```

- [ ] Design `RawTag` enum to fit in 1 byte
  ```rust
  /// Raw token tag — lightweight, standalone (no ori_* dependencies).
  /// Mapped to `ori_ir::TokenTag` in the integration layer (`ori_lexer`).
  /// See plans/v2-conventions.md §2 (Tag/Discriminant Enums), §10 (Two-Layer Pattern).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  #[repr(u8)]
  pub enum RawTag {
      // === Literals (0-9) ===
      Ident = 0,
      Int = 1,
      Float = 2,
      String = 3,
      Char = 4,
      Duration = 5,
      Size = 6,

      // === Keywords (10-59) ===
      KwLet = 10,
      KwFn = 11,
      KwIf = 12,
      KwElse = 13,
      KwMatch = 14,
      KwFor = 15,
      KwWhile = 16,
      KwLoop = 17,
      KwBreak = 18,
      KwContinue = 19,
      KwType = 20,
      KwTrait = 21,
      KwImpl = 22,
      KwUse = 23,
      KwMod = 24,
      KwPub = 25,
      KwMut = 26,
      KwSelf_ = 27,
      KwTrue = 28,
      KwFalse = 29,
      KwNil = 30,
      KwAnd = 31,
      KwOr = 32,
      KwNot = 33,
      KwIn = 34,
      KwAs = 35,
      KwWhere = 36,
      KwTests = 37,
      KwPrecondition = 38,
      KwPostcondition = 39,
      KwWith = 40,
      KwUses = 41,
      KwAsync = 42,
      KwAwait = 43,
      // ... more keywords up to ~50

      // === Operators (60-119) ===
      Plus = 60,
      Minus = 61,
      Star = 62,
      Slash = 63,
      Percent = 64,
      Caret = 65,
      Ampersand = 66,
      Pipe = 67,
      Tilde = 68,
      Bang = 69,
      Eq = 70,
      Lt = 71,
      Gt = 72,
      At = 73,
      Dot = 74,
      Comma = 75,
      Colon = 76,
      Semi = 77,
      Hash = 78,
      Question = 79,
      // Compound operators
      PlusEq = 80,
      MinusEq = 81,
      StarEq = 82,
      SlashEq = 83,
      EqEq = 84,
      BangEq = 85,
      LtEq = 86,
      GtEq = 87,
      AndAnd = 88,
      PipePipe = 89,
      Arrow = 90,
      FatArrow = 91,
      DotDot = 92,
      DotDotEq = 93,
      ColonColon = 94,
      PipeGt = 95,  // |>

      // === Delimiters (120-129) ===
      LParen = 120,
      RParen = 121,
      LBracket = 122,
      RBracket = 123,
      LBrace = 124,
      RBrace = 125,

      // === Trivia (130-139) ===
      Whitespace = 130,
      Newline = 131,
      LineComment = 132,
      BlockComment = 133,
      DocComment = 134,

      // === Special (240-255) ===
      Error = 250,
      UnterminatedString = 251,
      UnterminatedComment = 252,
      InvalidEscape = 253,
      InvalidChar = 254,
      Eof = 255,
  }
  ```

- [ ] Implement `lexeme()` and `name()` for `RawTag`
  ```rust
  impl RawTag {
      /// Get the fixed lexeme for this tag, if any
      #[inline]
      pub const fn lexeme(self) -> Option<&'static str> {
          match self {
              Self::KwLet => Some("let"),
              Self::KwFn => Some("fn"),
              Self::Plus => Some("+"),
              Self::EqEq => Some("=="),
              Self::LParen => Some("("),
              // Variable-length tokens
              Self::Ident | Self::Int | Self::String => None,
              // ...
              _ => None,
          }
      }

      /// Is this a fixed-length token?
      #[inline]
      pub const fn is_fixed_length(self) -> bool {
          self.lexeme().is_some()
      }

      /// Human-readable name for debugging (conventions §2)
      pub fn name(self) -> &'static str {
          match self {
              Self::Ident => "identifier",
              Self::Int => "integer",
              Self::Plus => "+",
              Self::KwLet => "let",
              // ...
          }
      }
  }
  ```

### Validation

```rust
#[test]
fn raw_tag_fits_in_u8() {
    assert!(std::mem::size_of::<RawTag>() == 1);
}

#[test]
fn raw_token_is_8_bytes() {
    assert!(std::mem::size_of::<RawToken>() == 8);
}
```

---

## 02.2 No End Offset Storage

**Goal:** Compute token end on demand, not store it

### Tasks

- [ ] Store only `start` in token storage
  ```rust
  struct TokenStorage {
      tags: Vec<TokenTag>,   // TokenTag from ori_ir (cooked from RawTag)
      starts: Vec<u32>,      // Only start positions!
      // No ends: Vec<u32>
  }
  ```

- [ ] Compute end via lexeme length for fixed tokens
  ```rust
  impl TokenStorage {
      pub fn token_end(&self, index: usize) -> u32 {
          let tag = self.tags[index];
          let start = self.starts[index];

          if let Some(lexeme) = tag.lexeme() {
              // Fixed-length token
              start + lexeme.len() as u32
          } else if index + 1 < self.starts.len() {
              // Variable-length: end is next token's start (or use stored value)
              self.starts[index + 1]
          } else {
              // Last token: need source length
              self.source_len
          }
      }
  }
  ```

- [ ] Handle variable-length tokens with auxiliary storage
  ```rust
  struct TokenStorage {
      tags: Vec<TokenTag>,
      starts: Vec<u32>,
      // For variable-length tokens (ident, string, number)
      // Store end offset only when needed
      var_ends: HashMap<u32, u32>,  // index -> end
  }
  ```

- [ ] Alternative: Re-tokenize on demand (Zig approach)
  ```rust
  impl TokenStorage {
      pub fn token_slice<'a>(&self, index: usize, source: &'a str) -> &'a str {
          let tag = self.tags[index];
          let start = self.starts[index] as usize;

          if let Some(lexeme) = tag.lexeme() {
              lexeme
          } else {
              // Re-tokenize from start to find length
              let end = Tokenizer::new(&source[start..])
                  .next()
                  .map(|t| start + t.len as usize)
                  .unwrap_or(source.len());
              &source[start..end]
          }
      }
  }
  ```

### Memory Savings

| Approach | Per Token | 10K Tokens |
|----------|-----------|------------|
| Current (start + end) | 8 bytes | 80 KB |
| Proposed (start only) | 4 bytes | 40 KB |
| **Savings** | **4 bytes** | **40 KB (50%)** |

---

## 02.3 Structure-of-Arrays Storage

**Goal:** Cache-optimal token storage using SoA layout

### Tasks

- [ ] Implement `TokenStorage` with SoA layout
  ```rust
  /// Structure-of-Arrays token storage (conventions §3).
  /// Uses `TokenTag` from `ori_ir` (cooked from `RawTag` by the integration layer).
  /// Indexed by `TokenIdx` (conventions §1).
  pub struct TokenStorage {
      /// Token tags - hot path during parsing
      tags: Vec<TokenTag>,
      /// Token start positions
      starts: Vec<u32>,
      /// Token values (interned strings, parsed numbers)
      /// Only populated for tokens that need it
      values: Vec<TokenValue>,
      /// Token flags (whitespace, trivia info)
      flags: Vec<TokenFlags>,
  }

  /// Value payload for tokens that need it
  pub enum TokenValue {
      None,
      Name(Name),           // Interned identifier
      Int(u64),             // Parsed integer
      Float(u64),           // Float bits
      String(Name),         // Interned string content
      Char(char),           // Character value
      Duration(u64, u8),    // Value + unit
      Size(u64, u8),        // Value + unit
  }
  ```

- [ ] Provide efficient accessors (matching `Pool` pattern — conventions §3)
  ```rust
  impl TokenStorage {
      /// Number of tokens
      #[inline]
      pub fn len(&self) -> usize {
          self.tags.len()
      }

      /// Get tag at index - most common operation
      #[inline]
      pub fn tag(&self, idx: TokenIdx) -> TokenTag {
          debug_assert!((idx.raw() as usize) < self.tags.len());
          self.tags[idx.raw() as usize]
      }

      /// Get start position
      #[inline]
      pub fn start(&self, idx: TokenIdx) -> u32 {
          debug_assert!((idx.raw() as usize) < self.starts.len());
          self.starts[idx.raw() as usize]
      }

      /// Get value (may be None for keywords/operators)
      #[inline]
      pub fn value(&self, idx: TokenIdx) -> &TokenValue {
          debug_assert!((idx.raw() as usize) < self.values.len());
          &self.values[idx.raw() as usize]
      }

      /// Get flags
      #[inline]
      pub fn flags(&self, idx: TokenIdx) -> TokenFlags {
          debug_assert!((idx.raw() as usize) < self.flags.len());
          self.flags[idx.raw() as usize]
      }

      /// Iterate just tags (hot path for token matching)
      pub fn tags(&self) -> &[TokenTag] {
          &self.tags
      }
  }
  ```

- [ ] Implement bulk operations
  ```rust
  impl TokenStorage {
      /// Pre-allocate based on source size heuristic (conventions §9)
      /// N=6: ~1 token per 6 source bytes (empirical, matches Zig measurements)
      pub fn with_capacity(source_len: usize) -> Self {
          let estimated = source_len / 6;
          Self {
              tags: Vec::with_capacity(estimated),
              starts: Vec::with_capacity(estimated),
              values: Vec::with_capacity(estimated),
              flags: Vec::with_capacity(estimated),
          }
      }

      /// Push a token, returning its index
      pub fn push(&mut self, tag: TokenTag, start: u32, value: TokenValue, flags: TokenFlags) -> TokenIdx {
          let idx = TokenIdx::from_raw(self.tags.len() as u32);
          self.tags.push(tag);
          self.starts.push(start);
          self.values.push(value);
          self.flags.push(flags);
          idx
      }
  }
  ```

### Cache Benefits

| Access Pattern | AoS (current) | SoA (proposed) |
|----------------|---------------|----------------|
| Sequential tag scan | 1 tag + 23 bytes garbage per line | 64 tags per cache line |
| Tag comparison | Random access | Sequential, prefetchable |
| Token count | O(1) | O(1) |

---

## 02.4 TokenFlags Bitfield

**Goal:** Compact metadata for whitespace-sensitive parsing

> **Width rationale (conventions §4):** `TokenFlags` uses `u8` while `TypeFlags` uses `u32`.
> Tokens need only ~8 flags (space_before, newline_before, adjacent, line_start, trivia_before, contextual_kw, has_error, is_doc).
> Types need ~20+ flags across 4 categories (presence, category, optimization, capability).
> Both use `bitflags!` with semantic bit ranges — the convention is the **pattern**, not the width.

### Tasks

- [ ] Define `TokenFlags` bitfield
  ```rust
  bitflags::bitflags! {
      /// Metadata flags for each token
      #[derive(Clone, Copy, Debug, Default)]
      pub struct TokenFlags: u8 {
          /// Whitespace (space/tab) preceded this token
          const SPACE_BEFORE = 0b0000_0001;
          /// Newline preceded this token
          const NEWLINE_BEFORE = 0b0000_0010;
          /// This token had trivia (comment) before it
          const TRIVIA_BEFORE = 0b0000_0100;
          /// This token is adjacent to previous (no space)
          const ADJACENT = 0b0000_1000;
          /// This token is at start of line
          const LINE_START = 0b0001_0000;
          /// This is a contextual keyword (not reserved)
          const CONTEXTUAL_KW = 0b0010_0000;
          /// This token had an error during lexing
          const HAS_ERROR = 0b0100_0000;
          /// This token is doc comment
          const IS_DOC = 0b1000_0000;
      }
  }
  ```

- [ ] Use flags for whitespace-sensitive parsing (Roc pattern)
  ```rust
  impl Cursor<'_> {
      /// Check if current token is adjacent to previous
      /// Used for `foo.bar` vs `foo . bar` disambiguation
      pub fn is_adjacent(&self) -> bool {
          self.current_flags().contains(TokenFlags::ADJACENT)
      }

      /// Check if current token starts a new line
      pub fn at_line_start(&self) -> bool {
          self.current_flags().contains(TokenFlags::LINE_START)
      }

      /// Check for function call syntax: `foo(` with no space
      pub fn is_call_syntax(&self) -> bool {
          self.check(TokenTag::LParen) && self.is_adjacent()
      }
  }
  ```

- [ ] Set flags during tokenization
  ```rust
  impl TokenProcessor<'_> {
      fn process_token(&mut self, raw: RawToken) -> (TokenTag, TokenValue, TokenFlags) {
          let mut flags = TokenFlags::empty();

          // Check preceding whitespace
          if self.had_space {
              flags |= TokenFlags::SPACE_BEFORE;
          }
          if self.had_newline {
              flags |= TokenFlags::NEWLINE_BEFORE;
              flags |= TokenFlags::LINE_START;
          }
          if !self.had_space && !self.had_newline {
              flags |= TokenFlags::ADJACENT;
          }
          if self.had_trivia {
              flags |= TokenFlags::TRIVIA_BEFORE;
          }

          // ... cook token
          (tag, value, flags)
      }
  }
  ```

---

## 02.5 Lazy Line/Column Computation

**Goal:** Compute line/column only when needed (errors)

### Tasks

- [ ] Store only byte offsets in tokens
  ```rust
  // Tokens store byte offsets only
  struct TokenStorage {
      starts: Vec<u32>,  // Byte offsets
      // No line/column storage!
  }
  ```

- [ ] Create `LineIndex` for lazy computation
  ```rust
  /// Line index for lazy line/column lookup
  pub struct LineIndex {
      /// Byte offset of each line start
      line_starts: Vec<u32>,
  }

  impl LineIndex {
      /// Build line index from source (one-time cost)
      pub fn new(source: &str) -> Self {
          let mut line_starts = vec![0];
          for (i, c) in source.char_indices() {
              if c == '\n' {
                  line_starts.push(i as u32 + 1);
              }
          }
          LineIndex { line_starts }
      }

      /// Convert byte offset to line/column (1-indexed)
      pub fn line_col(&self, offset: u32) -> (u32, u32) {
          // Binary search for line
          let line = match self.line_starts.binary_search(&offset) {
              Ok(i) => i,
              Err(i) => i.saturating_sub(1),
          };
          let col = offset - self.line_starts[line];
          (line as u32 + 1, col + 1)
      }

      /// Get line number only
      pub fn line(&self, offset: u32) -> u32 {
          self.line_col(offset).0
      }
  }
  ```

- [ ] Cache line index per file
  ```rust
  /// Cached per-file data including line index
  pub struct SourceFile {
      pub content: String,
      pub line_index: OnceCell<LineIndex>,
  }

  impl SourceFile {
      pub fn line_index(&self) -> &LineIndex {
          self.line_index.get_or_init(|| LineIndex::new(&self.content))
      }
  }
  ```

- [ ] Use lazy computation only in error paths
  ```rust
  impl ParseError {
      pub fn with_location(self, source: &SourceFile) -> DiagnosticWithLocation {
          // Only compute line/column when formatting error
          let (line, col) = source.line_index().line_col(self.span.start);
          DiagnosticWithLocation {
              error: self,
              line,
              column: col,
          }
      }
  }
  ```

### Performance Impact

| Operation | Compute Always | Lazy Compute |
|-----------|----------------|--------------|
| Tokenize 10K tokens | 10K line lookups | 0 lookups |
| Parse (no errors) | 0 additional | 0 lookups |
| Parse (1 error) | 0 additional | 1 lookup |
| Format error | Already computed | 1 lookup |

---

## 02.6 Completion Checklist

- [ ] `RawToken` is 8 bytes
- [ ] `RawTag` fits in 1 byte (< 256 variants)
- [ ] `TokenStorage` uses SoA layout
- [ ] Token end computed on demand
- [ ] `TokenFlags` captures whitespace info
- [ ] `LineIndex` computes line/column lazily
- [ ] No regression in functionality
- [ ] Benchmarks show improvement

**Exit Criteria:**
- Token memory usage reduced by ~50%
- Parser still works correctly
- Error messages still show correct line/column
- No performance regression

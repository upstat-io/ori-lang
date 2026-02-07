---
section: "01"
title: Architecture & Source Buffer
status: done
goal: "Establish the two-layer crate architecture (ori_lexer_core + ori_lexer) and provide a sentinel-terminated, cache-aligned input buffer and cursor for zero-bounds-check scanning"
sections:
  - id: "01.1"
    title: Two-Layer Crate Design
    status: done
  - id: "01.2"
    title: Crate Boundary Design
    status: done
  - id: "01.3"
    title: SourceBuffer Type
    status: done
  - id: "01.4"
    title: Cursor
    status: done
  - id: "01.5"
    title: BOM & Encoding Detection
    status: done
  - id: "01.6"
    title: API Stability Guarantees
    status: done
  - id: "01.7"
    title: Tests
    status: done
---

# Section 01: Architecture & Source Buffer

**Status:** :white_check_mark: Done
**Goal:** Establish the two-layer crate architecture and provide a sentinel-terminated, cache-aligned input buffer and cursor that eliminates bounds checks in the scanner's hot loop.

> **REFERENCE**: Rust's `rustc_lexer` / `rustc_parse::lexer` two-layer separation; Zig's sentinel-terminated `[:0]const u8` buffer; Go's `source` struct with sentinel byte at `buf[e]`; Roc's `Src64` 64-byte-aligned loader with cache prefetching.
>
> **CONVENTIONS**: v2-conventions SS1 (Index Types), SS2 (Tag Enums), SS7 (Shared Types in `ori_ir`), SS10 (Two-Layer Crate Pattern).

---

## Design Rationale

### Two-Layer Architecture

Rust's compiler has a two-layer lexer design that cleanly separates concerns:

1. **`rustc_lexer`** -- Low-level, pure, no compiler dependencies. Works on raw `&str`, returns simple tokens as `(kind, length)` pairs. No spans, no interning, no diagnostics. Stable API usable by rust-analyzer and external tools.

2. **`rustc_parse::lexer`** -- High-level, compiler-integrated. "Cooks" raw tokens into AST tokens. Adds spans, interns symbols, emits diagnostics, handles edition-aware behavior.

Ori adopts this pattern: `ori_lexer_core` (standalone, pure) feeds into `ori_lexer` (compiler integration). This enables code reuse across tools (LSP, formatter, syntax highlighter) while keeping the compiler's lexer optimized.

### Sentinel-Terminated Buffer

The scanner's inner loop executes billions of iterations on large codebases. Every bounds check in that loop is wasted work. By guaranteeing a sentinel byte (0x00) at the end of the buffer, the scanner can use a simple `match buf[pos]` dispatch where the sentinel naturally terminates scanning -- no `if pos >= len` check needed.

Cache-line alignment (64 bytes) ensures the first cache line is loaded optimally and enables future SIMD operations on the buffer without alignment faults.

### Patterns Adopted

| Pattern | Source | Rationale |
|---------|--------|-----------|
| Two-layer crate separation | Rust (`rustc_lexer`) | Reusable core, compiler-specific integration |
| Sentinel-terminated buffer | Zig, Go | Eliminates bounds checks in scanning hot loop |
| Cache-line alignment | Roc (Src64) | Optimal cache line loading; SIMD-ready |
| Cache prefetching on construction | Roc (Src64) | Warm up L1 cache for first ~256 bytes |
| BOM detection at init | Zig, Go | Handle once, advance start index, zero overhead after |
| Position tracking via subtraction | Rust (`rustc_lexer`) | Compute token length as `initial_remaining - current_remaining` |

---

## 01.1 Two-Layer Crate Design

**Goal:** Create `ori_lexer_core` as a standalone, pure tokenization crate with zero `ori_*` dependencies (v2-conventions SS10).

- [x] Create new crate `compiler/ori_lexer_core/`:
  ```
  compiler/ori_lexer_core/
  +-- Cargo.toml
  +-- src/
      +-- lib.rs           # Public API
      +-- raw_scanner.rs   # State machine
      +-- tag.rs           # RawTag enum
      +-- cursor.rs        # Byte cursor
      +-- source_buffer.rs # Sentinel-terminated buffer
  ```
- [x] Design minimal dependencies:
  ```toml
  [dependencies]
  # No ori_* dependencies!
  # No interner, no spans, no diagnostics
  ```
- [x] Define core public types:
  ```rust
  /// Raw token from low-level tokenizer.
  /// See plans/v2-conventions.md §10 (Two-Layer Pattern).
  pub struct RawToken {
      pub tag: RawTag,
      pub len: u32,
  }

  /// Raw token kind -- lightweight, standalone (no ori_* dependencies).
  /// Mapped to `ori_ir::TokenKind` in the integration layer (`ori_lexer`).
  /// See plans/v2-conventions.md §2 (Tag/Discriminant Enums), §10 (Two-Layer Pattern).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  #[non_exhaustive]
  #[repr(u8)]
  pub enum RawTag {
      // ... (defined in Section 02)
  }
  ```
- [x] Implement main entry point:
  ```rust
  /// Tokenize entire source, yielding raw tokens.
  pub fn tokenize(source: &str) -> impl Iterator<Item = RawToken> + '_ {
      RawScanner::new(source)
  }
  ```
- [x] Ensure no panics in public API -- all errors become `RawTag` error variants
- [x] Add `RawTag::lexeme()` and `RawTag::name()` methods (v2-conventions SS2)

---

## 01.2 Crate Boundary Design

**Goal:** Define clean interfaces between `ori_lexer_core` and `ori_lexer` (v2-conventions SS7, SS10).

- [x] Document what belongs in each layer:

  | Concern | `ori_lexer_core` | `ori_lexer` |
  |---------|------------------|-------------|
  | Tokenization | Yes | |
  | Tag definition | `RawTag` (local) | `TokenKind` (from `ori_ir`) |
  | Byte lengths | Yes | |
  | Spans | | Yes |
  | Interning | | Yes |
  | Number parsing | | Yes |
  | Escape validation | | Yes |
  | Template literal nesting | Stack-based brace depth | |
  | Context-sensitive keywords | | Yes (cooking layer) |
  | BOM/encoding detection | Detect, return position | Convert to LexError |
  | Null byte detection | Detect, return position | Convert to LexError |
  | Error diagnostics | | Yes |
  | Trivia handling | | Yes |
  | Salsa integration | | Yes |
  | TokenFlags computation | | Yes |
  | `ori_*` dependencies | **None** | `ori_lexer_core`, `ori_ir` |

- [x] Add dependency on `ori_lexer_core` in `ori_lexer`:
  ```toml
  [dependencies]
  ori_lexer_core = { path = "../ori_lexer_core" }
  ori_ir = { path = "../ori_ir" }
  # NO logos dependency
  ```
- [x] Create re-exports in `ori_lexer`:
  ```rust
  // ori_lexer/src/lib.rs
  pub use ori_lexer_core::{
      RawTag, RawToken,
      EncodingIssue, EncodingIssueKind,
      tokenize as tokenize_raw,
  };

  // High-level API
  pub fn lex(source: &str, interner: &StringInterner) -> LexOutput { ... }
  ```
- [x] Document the crate boundary mappings:
  - Core produces `(RawTag, len)` pairs and detects `EncodingIssue`s
  - Integration maps `RawTag` -> `ori_ir::TokenKind`, adds `Span`, interns identifiers/strings, validates escapes and numbers, computes `TokenFlags`
  - Integration converts `EncodingIssue` -> `LexError` with proper spans and diagnostic messages

---

## 01.3 SourceBuffer Type

- [x] Create `source_buffer.rs` module in `ori_lexer_core`
- [x] Define `SourceBuffer` struct:
  ```rust
  /// Sentinel-terminated source buffer for zero-bounds-check scanning.
  ///
  /// The buffer is guaranteed to end with a 0x00 byte that is NOT part of
  /// the source content. The scanner uses this sentinel to detect EOF without
  /// explicit bounds checking.
  pub struct SourceBuffer {
      /// Owned buffer with sentinel. The last byte is always 0x00.
      /// Layout: [source_bytes..., 0x00, padding...]
      buf: Vec<u8>,
      /// Length of the actual source content (excludes sentinel and padding).
      source_len: u32,
      /// Encoding issues detected during construction (byte positions).
      /// Integration layer (ori_lexer) converts these to LexError diagnostics.
      encoding_issues: Vec<EncodingIssue>,
  }

  /// Encoding issue detected in source buffer.
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub struct EncodingIssue {
      pub kind: EncodingIssueKind,
      pub pos: u32,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum EncodingIssueKind {
      Utf8Bom,      // UTF-8 BOM at start (forbidden)
      Utf16LeBom,   // UTF-16 LE BOM at start
      Utf16BeBom,   // UTF-16 BE BOM at start
      InteriorNull, // Null byte in source content
  }
  ```
- [x] Implement `SourceBuffer::new(source: &str) -> Self`:
  - Allocate `source.len() + 1` bytes minimum (for sentinel)
  - Round up to next 64-byte boundary for cache alignment (provides padding after sentinel)
  - Copy source bytes, append 0x00 sentinel
  - Detect and record (not error) UTF-8 BOM (0xEF 0xBB 0xBF) at start
  - Detect and record UTF-16 BOMs (0xFF 0xFE or 0xFE 0xFF) at start
  - Detect and record interior null bytes (scan for 0x00 before sentinel position)
  - Return SourceBuffer with encoding_issues populated
- [x] Implement `SourceBuffer::as_bytes(&self) -> &[u8]` -- returns source bytes (without sentinel)
- [x] Implement `SourceBuffer::as_sentinel_bytes(&self) -> &[u8]` -- returns bytes including sentinel and padding
- [x] Implement `SourceBuffer::cursor(&self) -> Cursor` -- creates a cursor at position 0
- [x] Implement `SourceBuffer::len(&self) -> u32` -- source length
- [x] Implement `SourceBuffer::is_empty(&self) -> bool`
- [x] Implement `SourceBuffer::encoding_issues(&self) -> &[EncodingIssue]` -- access detected issues
- [x] Add `#[cfg(target_arch = "x86_64")]` cache prefetch hint on construction (prefetch first 4 cache lines)
- [x] Add `#[cfg(target_arch = "aarch64")]` cache prefetch hint (PRFM equivalent)
- [x] Size assertion: `SourceBuffer` should be 56 bytes on 64-bit (Vec<u8> is 24 bytes, u32 is 4 bytes, Vec<EncodingIssue> is 24 bytes, +4 padding)

---

## 01.4 Cursor

- [x] Define `Cursor` struct:
  ```rust
  /// Zero-cost cursor over a sentinel-terminated buffer.
  ///
  /// The cursor advances through the buffer byte-by-byte. EOF is detected
  /// when the current byte equals the sentinel (0x00). No bounds checking
  /// is performed -- the sentinel guarantees safe termination.
  pub struct Cursor<'a> {
      /// Pointer to the sentinel-terminated buffer (includes sentinel and padding).
      buf: &'a [u8],
      /// Current read position (byte index into buf).
      pos: u32,
      /// Length of actual source content (excludes sentinel and padding).
      source_len: u32,
  }
  ```
- [x] Implement core methods:
  - `current(&self) -> u8` -- returns `buf[pos]` (0x00 at EOF)
  - `peek(&self) -> u8` -- returns `buf[pos + 1]` (safe: sentinel guarantees valid read)
  - `peek2(&self) -> u8` -- returns `buf[pos + 2]` (safe: cache-line alignment guarantees padding after sentinel)
  - `advance(&mut self)` -- `self.pos += 1`
  - `advance_n(&mut self, n: u32)` -- `self.pos += n`
  - `is_eof(&self) -> bool` -- `self.current() == 0 && self.pos >= self.source_len`
  - `pos(&self) -> u32` -- current byte offset
  - `slice(&self, start: u32, end: u32) -> &'a str` -- extract source substring (unsafe: caller ensures valid UTF-8 range)
  - `slice_from(&self, start: u32) -> &'a str` -- extract from `start` to current position
- [x] Ensure `Cursor` is `Copy` (all fields are `Copy`)
- [x] Size assertion: `Cursor` should be <= 24 bytes (pointer + 2 × u32 + padding)
- [x] Add `eat_while(&mut self, pred: impl Fn(u8) -> bool)` -- advance while predicate holds
  - Sentinel (0x00) naturally stops the loop since `pred(0)` should return false for all reasonable predicates
- [x] Add `eat_until(&mut self, byte: u8) -> u32` -- advance until `byte` is found, return bytes consumed
  - Will use `memchr` in Section 05; initial impl is byte-by-byte

---

## 01.5 BOM & Encoding Detection

This detection happens in `SourceBuffer::new` in `ori_lexer_core`. The core layer records issue positions; the integration layer (`ori_lexer`) converts them to `LexError` diagnostics with proper spans and messages.

- [x] Detect UTF-8 BOM (0xEF 0xBB 0xBF) at buffer start
  - Record as `EncodingIssueKind::Utf8Bom` at position 0
  - Spec: 02-source-code.md § Encoding states "Source files must be valid UTF-8 without byte order mark"
  - Integration layer message: "UTF-8 BOM detected. Ori source files must not contain a byte order mark."
- [x] Detect UTF-16 BOMs at buffer start:
  - Little-endian: 0xFF 0xFE -> record as `EncodingIssueKind::Utf16LeBom` at position 0
  - Big-endian: 0xFE 0xFF -> record as `EncodingIssueKind::Utf16BeBom` at position 0
  - Integration layer message: "This file appears to be UTF-16 encoded. Ori source files must be UTF-8."
- [x] Detect null bytes (0x00) in source content before sentinel position
  - NUL (U+0000) is not allowed per grammar.ebnf line 28: unicode_char excludes NUL
  - Record each occurrence as `EncodingIssueKind::InteriorNull` at its byte position
  - Scanner continues past them (the sentinel is distinguished by being at `pos >= source_len`)

---

## 01.6 API Stability Guarantees

**Goal:** Define stability expectations for external users of `ori_lexer_core` (v2-conventions SS10).

- [x] Mark `ori_lexer_core` with appropriate version:
  ```toml
  [package]
  name = "ori_lexer_core"
  version = "0.1.0"
  # Note: API may change until Ori 1.0
  ```
- [x] Add `#[non_exhaustive]` on `RawTag` to allow future variant additions without breaking downstream
- [x] Document stability expectations:
  ```rust
  //! ## Stability
  //!
  //! - `RawTag` enum: Variants may be added (non-exhaustive)
  //! - `RawToken` struct: Fields are stable
  //! - `tokenize()`: Signature is stable
  //! - Error tags: May be refined (new error kinds)
  ```
- [x] Create stability tests:
  ```rust
  #[test]
  fn api_stability() {
      // These patterns must continue to work
      let tok = RawToken { tag: RawTag::Ident, len: 3 };
      assert!(tok.tag == RawTag::Ident);
      assert!(tok.len == 3);

      // RawTag must have lexeme method
      assert_eq!(RawTag::Plus.lexeme(), Some("+"));

      // RawTag must have name method (conventions §2)
      assert_eq!(RawTag::Ident.name(), "identifier");
  }
  ```

---

## 01.7 Tests

- [x] Unit tests for `SourceBuffer::new`:
  - Empty source -> buffer is `[0x00]`, len is 0, no encoding issues
  - ASCII source -> bytes match, sentinel at end, no encoding issues
  - UTF-8 source (multi-byte chars) -> bytes preserved, sentinel at end, no encoding issues
  - Source with UTF-8 BOM -> `Utf8Bom` issue recorded at position 0
  - Source with UTF-16 LE BOM -> `Utf16LeBom` issue recorded at position 0
  - Source with UTF-16 BE BOM -> `Utf16BeBom` issue recorded at position 0
  - Source with interior null (0x00) -> `InteriorNull` issue(s) recorded at byte position(s)
  - Source with multiple issues -> all issues recorded with correct positions
  - Large source (> 64KB) -> alignment and sentinel correct, no false positive issues
- [x] Unit tests for `Cursor`:
  - Basic advance/current/peek through ASCII
  - EOF detection at sentinel
  - `eat_while` with various predicates
  - `eat_until` finding target byte
  - `slice` and `slice_from` correctness
  - Peek and peek2 near end of buffer (sentinel padding ensures safety)
- [x] Crate boundary tests:
  - `ori_lexer_core` compiles with zero `ori_*` dependencies (CI build check)
  - `tokenize()` returns valid `RawToken` stream for basic inputs
  - `RawTag::lexeme()` returns correct fixed lexemes
  - `RawTag::name()` returns human-readable names for all variants
- [x] API stability tests (01.6 above)
- [x] Property tests (if proptest is available):
  - For any valid UTF-8 input, `SourceBuffer::new` produces a buffer where the last byte is 0x00
  - `Cursor` advancing past every byte eventually reaches EOF

---

## 01.8 Completion Checklist

- [x] `ori_lexer_core` crate created with zero `ori_*` dependencies
- [x] `ori_lexer` updated to depend on `ori_lexer_core` + `ori_ir`
- [x] `source_buffer.rs` module added to `ori_lexer_core`
- [x] `SourceBuffer`, `Cursor`, `EncodingIssue`, and `EncodingIssueKind` types implemented and tested
- [x] BOM detection records `EncodingIssue`s for UTF-8 BOM (forbidden per spec) and UTF-16 BOMs
- [x] Interior null byte (U+0000) detection records `EncodingIssue`s (forbidden per grammar)
- [x] Cache prefetch hints on supported architectures (x86_64 `_mm_prefetch`)
- [x] Size assertions pass (SourceBuffer <= 64 bytes, Cursor <= 24 bytes)
- [x] `#[non_exhaustive]` on `RawTag`
- [x] API stability tests pass
- [x] `cargo t -p ori_lexer_core` passes (48 unit tests + 1 doc test)
- [x] `cargo t -p ori_lexer` passes (no regressions)

**Exit Criteria:** `ori_lexer_core` crate exists with `SourceBuffer`, `Cursor`, and `RawTag` types. It compiles standalone with zero `ori_*` dependencies. `ori_lexer` depends on it and can map `RawTag` -> `TokenKind`. All tests pass. No performance regression in existing lexer (this section doesn't modify the existing lexer yet).

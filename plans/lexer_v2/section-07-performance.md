---
section: "07"
title: Performance Optimizations
status: not-started
goal: SIMD and memory optimizations for maximum speed
sections:
  - id: "07.1"
    title: SIMD Whitespace Skipping
    status: not-started
  - id: "07.2"
    title: memchr for Delimiters
    status: not-started
  - id: "07.3"
    title: Branchless Character Checks
    status: not-started
  - id: "07.4"
    title: Buffer Management
    status: not-started
---

# Section 07: Performance Optimizations

**Status:** 📋 Planned
**Goal:** SIMD and memory optimizations for maximum speed
**Source:** Roc, Rust (`memchr`), Go

---

## Background

### Performance Characteristics of Lexing

Lexing is typically:
- **Memory-bound**: Reading source bytes
- **Branch-heavy**: Character classification
- **Allocation-light**: Tokens are small

Key optimization opportunities:
1. **Whitespace skipping**: Often 20-30% of source
2. **Comment skipping**: Can be large blocks
3. **Character classification**: Hot path
4. **Memory layout**: Cache efficiency

---

## 07.1 SIMD Whitespace Skipping

**Goal:** Skip whitespace 8 bytes at a time

### Tasks

- [ ] Implement SIMD whitespace detection
  ```rust
  /// Skip whitespace using SIMD-like operations
  /// Processes 8 bytes at a time on 64-bit systems
  #[inline]
  pub fn skip_whitespace_fast(bytes: &[u8], mut pos: usize) -> usize {
      // Process 8 bytes at a time
      while pos + 8 <= bytes.len() {
          // Load 8 bytes as u64
          let chunk = u64::from_ne_bytes(
              bytes[pos..pos + 8].try_into().unwrap()
          );

          // Check if all bytes are spaces (0x20)
          // Uses SWAR (SIMD Within A Register) technique
          let spaces = 0x2020_2020_2020_2020u64;
          if chunk == spaces {
              pos += 8;
              continue;
          }

          // Check for mixed whitespace (space or tab)
          // This is more complex but still faster than byte-by-byte
          let has_non_ws = has_non_whitespace(chunk);
          if has_non_ws == 0 {
              pos += 8;
              continue;
          }

          // Found non-whitespace, count leading whitespace
          let leading = count_leading_whitespace(chunk);
          pos += leading;
          break;
      }

      // Handle remaining bytes
      while pos < bytes.len() {
          match bytes[pos] {
              b' ' | b'\t' => pos += 1,
              _ => break,
          }
      }

      pos
  }

  /// Check if any byte in u64 is not space or tab
  #[inline]
  fn has_non_whitespace(chunk: u64) -> u64 {
      // Create masks for space (0x20) and tab (0x09)
      let spaces = 0x2020_2020_2020_2020u64;
      let tabs = 0x0909_0909_0909_0909u64;

      // XOR with spaces and tabs, OR results
      // Non-zero bytes indicate non-whitespace
      let not_space = chunk ^ spaces;
      let not_tab = chunk ^ tabs;

      // Use the "has zero byte" trick
      has_zero_byte(not_space) & has_zero_byte(not_tab)
  }

  /// Detect if any byte in u64 is zero
  #[inline]
  fn has_zero_byte(x: u64) -> u64 {
      const LO: u64 = 0x0101_0101_0101_0101;
      const HI: u64 = 0x8080_8080_8080_8080;
      (x.wrapping_sub(LO)) & !x & HI
  }
  ```

- [ ] Add architecture-specific SIMD
  ```rust
  #[cfg(target_arch = "x86_64")]
  mod simd_x86 {
      use std::arch::x86_64::*;

      /// AVX2 whitespace skipping (32 bytes at a time)
      #[target_feature(enable = "avx2")]
      pub unsafe fn skip_whitespace_avx2(bytes: &[u8], mut pos: usize) -> usize {
          let space = _mm256_set1_epi8(b' ' as i8);
          let tab = _mm256_set1_epi8(b'\t' as i8);

          while pos + 32 <= bytes.len() {
              let chunk = _mm256_loadu_si256(
                  bytes[pos..].as_ptr() as *const __m256i
              );

              // Compare with space and tab
              let is_space = _mm256_cmpeq_epi8(chunk, space);
              let is_tab = _mm256_cmpeq_epi8(chunk, tab);
              let is_ws = _mm256_or_si256(is_space, is_tab);

              // Get mask of non-whitespace bytes
              let mask = _mm256_movemask_epi8(is_ws) as u32;

              if mask == 0xFFFF_FFFF {
                  pos += 32;
                  continue;
              }

              // Find first non-whitespace
              let first_non_ws = (!mask).trailing_zeros() as usize;
              pos += first_non_ws;
              break;
          }

          // Fallback to scalar for remainder
          skip_whitespace_scalar(bytes, pos)
      }
  }
  ```

- [ ] Benchmark whitespace skipping
  ```rust
  #[bench]
  fn bench_skip_whitespace_scalar(b: &mut Bencher) {
      let input = "    ".repeat(1000);
      b.iter(|| skip_whitespace_scalar(input.as_bytes(), 0));
  }

  #[bench]
  fn bench_skip_whitespace_fast(b: &mut Bencher) {
      let input = "    ".repeat(1000);
      b.iter(|| skip_whitespace_fast(input.as_bytes(), 0));
  }

  #[bench]
  fn bench_skip_whitespace_avx2(b: &mut Bencher) {
      let input = "    ".repeat(1000);
      b.iter(|| unsafe { skip_whitespace_avx2(input.as_bytes(), 0) });
  }
  ```

---

## 07.2 memchr for Delimiters

**Goal:** Use SIMD-accelerated memchr for finding delimiters

### Tasks

- [ ] Add memchr dependency
  ```toml
  [dependencies]
  memchr = "2"
  ```

- [ ] Use memchr for comment skipping
  ```rust
  use memchr::memchr;

  /// Skip to end of line comment
  #[inline]
  fn skip_line_comment(bytes: &[u8], pos: usize) -> usize {
      match memchr(b'\n', &bytes[pos..]) {
          Some(offset) => pos + offset + 1,
          None => bytes.len(),
      }
  }

  /// Skip to end of string (finds closing quote)
  #[inline]
  fn find_string_end(bytes: &[u8], pos: usize) -> Option<usize> {
      // memchr2 finds either quote or backslash
      use memchr::memchr2;

      let mut i = pos;
      loop {
          match memchr2(b'"', b'\\', &bytes[i..]) {
              Some(offset) => {
                  let found_pos = i + offset;
                  if bytes[found_pos] == b'"' {
                      return Some(found_pos);
                  }
                  // Skip escape sequence
                  i = found_pos + 2;
                  if i >= bytes.len() {
                      return None;
                  }
              }
              None => return None,
          }
      }
  }
  ```

- [ ] Use memchr for identifier scanning
  ```rust
  /// Find end of identifier using memchr for delimiter
  #[inline]
  fn find_identifier_end(bytes: &[u8], pos: usize) -> usize {
      // Scan until we find a non-identifier byte
      // For ASCII identifiers, we can check for common delimiters
      for i in pos..bytes.len() {
          let c = bytes[i];
          if !is_ident_byte(c) {
              return i;
          }
      }
      bytes.len()
  }

  /// Fast check for identifier continuation byte
  #[inline]
  fn is_ident_byte(c: u8) -> bool {
      // Lookup table for O(1) check
      const TABLE: [bool; 256] = {
          let mut t = [false; 256];
          let mut i = 0;
          while i < 256 {
              t[i] = matches!(i as u8,
                  b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
              );
              i += 1;
          }
          t
      };
      TABLE[c as usize]
  }
  ```

---

## 07.3 Branchless Character Checks

**Goal:** Use lookup tables and branchless comparisons

### Tasks

- [ ] Create character classification table
  ```rust
  /// Character classification flags
  bitflags::bitflags! {
      #[derive(Clone, Copy)]
      struct CharClass: u8 {
          const IDENT_START = 0b0000_0001;  // a-z, A-Z, _
          const IDENT_CONT  = 0b0000_0010;  // a-z, A-Z, 0-9, _
          const DIGIT       = 0b0000_0100;  // 0-9
          const HEX_DIGIT   = 0b0000_1000;  // 0-9, a-f, A-F
          const WHITESPACE  = 0b0001_0000;  // space, tab
          const NEWLINE     = 0b0010_0000;  // \n, \r
          const OPERATOR    = 0b0100_0000;  // +, -, *, etc.
          const DELIMITER   = 0b1000_0000;  // (, ), {, }, etc.
      }
  }

  /// Lookup table for ASCII characters (0-127)
  const CHAR_CLASS: [CharClass; 128] = {
      let mut t = [CharClass::empty(); 128];

      // Identifiers
      let mut c = b'a';
      while c <= b'z' {
          t[c as usize] = CharClass::IDENT_START
              .union(CharClass::IDENT_CONT);
          c += 1;
      }
      c = b'A';
      while c <= b'Z' {
          t[c as usize] = CharClass::IDENT_START
              .union(CharClass::IDENT_CONT);
          c += 1;
      }
      t[b'_' as usize] = CharClass::IDENT_START
          .union(CharClass::IDENT_CONT);

      // Digits
      c = b'0';
      while c <= b'9' {
          t[c as usize] = t[c as usize]
              .union(CharClass::IDENT_CONT)
              .union(CharClass::DIGIT)
              .union(CharClass::HEX_DIGIT);
          c += 1;
      }
      // Hex letters
      c = b'a';
      while c <= b'f' {
          t[c as usize] = t[c as usize].union(CharClass::HEX_DIGIT);
          c += 1;
      }
      c = b'A';
      while c <= b'F' {
          t[c as usize] = t[c as usize].union(CharClass::HEX_DIGIT);
          c += 1;
      }

      // Whitespace
      t[b' ' as usize] = CharClass::WHITESPACE;
      t[b'\t' as usize] = CharClass::WHITESPACE;
      t[b'\n' as usize] = CharClass::NEWLINE;
      t[b'\r' as usize] = CharClass::NEWLINE;

      // Operators
      let ops = b"+-*/%&|^~!<>=";
      let mut i = 0;
      while i < ops.len() {
          t[ops[i] as usize] = CharClass::OPERATOR;
          i += 1;
      }

      // Delimiters
      let delims = b"()[]{},.;:@#?";
      i = 0;
      while i < delims.len() {
          t[delims[i] as usize] = CharClass::DELIMITER;
          i += 1;
      }

      t
  };

  /// Fast character classification
  #[inline]
  fn char_class(c: u8) -> CharClass {
      if c < 128 {
          CHAR_CLASS[c as usize]
      } else {
          CharClass::empty()
      }
  }

  #[inline]
  fn is_ident_start(c: u8) -> bool {
      char_class(c).contains(CharClass::IDENT_START)
  }

  #[inline]
  fn is_ident_cont(c: u8) -> bool {
      char_class(c).contains(CharClass::IDENT_CONT)
  }
  ```

- [ ] Implement branchless lowercase
  ```rust
  /// Branchless ASCII lowercase (Go pattern)
  #[inline]
  fn to_lower(c: u8) -> u8 {
      // Works because 'a' - 'A' = 32 = 0b0010_0000
      // OR-ing with 0x20 converts uppercase to lowercase
      // For non-letters, result is garbage but we only use
      // it for comparisons that will fail anyway
      c | 0x20
  }

  /// Check if c is in range [a, z] (lowercase)
  #[inline]
  fn is_lowercase(c: u8) -> bool {
      // Branchless: c >= 'a' && c <= 'z'
      // Equivalent to: c - 'a' <= 'z' - 'a'
      c.wrapping_sub(b'a') <= (b'z' - b'a')
  }

  /// Check if c is a letter (any case)
  #[inline]
  fn is_letter(c: u8) -> bool {
      is_lowercase(to_lower(c))
  }
  ```

---

## 07.4 Buffer Management

**Goal:** Optimize memory access patterns

### Tasks

- [ ] Pre-allocate token buffer
  ```rust
  impl TokenStorage {
      /// Create with estimated capacity
      pub fn with_source_size(source_len: usize) -> Self {
          // Empirical: ~1 token per 6 bytes (Zig's ratio)
          let estimated_tokens = source_len / 6;

          Self {
              tags: Vec::with_capacity(estimated_tokens),
              starts: Vec::with_capacity(estimated_tokens),
              values: Vec::with_capacity(estimated_tokens / 4), // Most tokens don't need values
              flags: Vec::with_capacity(estimated_tokens),
          }
      }
  }
  ```

- [ ] Use sentinel-terminated buffer
  ```rust
  /// Source buffer with sentinel for fast EOF detection
  pub struct SourceBuffer {
      data: Vec<u8>,
  }

  impl SourceBuffer {
      pub fn new(source: &str) -> Self {
          let mut data = Vec::with_capacity(source.len() + 1);
          data.extend_from_slice(source.as_bytes());
          data.push(0); // Sentinel
          Self { data }
      }

      /// Get byte at position without bounds check
      #[inline]
      pub fn get(&self, pos: usize) -> u8 {
          // SAFETY: Buffer is sentinel-terminated
          // Reading sentinel (0) at end is safe
          debug_assert!(pos < self.data.len());
          unsafe { *self.data.get_unchecked(pos) }
      }
  }
  ```

- [ ] Minimize allocations during tokenization
  ```rust
  impl<'a> Tokenizer<'a> {
      /// Tokenize without intermediate allocations
      pub fn tokenize_into(&mut self, storage: &mut TokenStorage) {
          loop {
              let token = self.next_token();

              if token.tag == Tag::Eof {
                  break;
              }

              storage.push_raw(token);
          }
      }
  }

  impl TokenStorage {
      /// Push raw token without processing
      #[inline]
      fn push_raw(&mut self, token: RawToken) {
          self.tags.push(token.tag);
          self.starts.push(token.start);
          // Values and flags populated in second pass if needed
      }
  }
  ```

- [ ] Batch process trivia
  ```rust
  /// Process trivia in batch after tokenization
  pub fn process_trivia(storage: &mut TokenStorage, source: &[u8]) {
      let mut prev_end = 0u32;

      for i in 0..storage.len() {
          let start = storage.starts[i];

          // Check for whitespace/newline between tokens
          let mut flags = TokenFlags::empty();

          if prev_end < start {
              let gap = &source[prev_end as usize..start as usize];

              if gap.iter().any(|&c| c == b'\n') {
                  flags |= TokenFlags::NEWLINE_BEFORE;
              }
              if gap.iter().any(|&c| c == b' ' || c == b'\t') {
                  flags |= TokenFlags::SPACE_BEFORE;
              }
          } else if prev_end == start && i > 0 {
              flags |= TokenFlags::ADJACENT;
          }

          storage.flags.push(flags);
          prev_end = storage.token_end(i);
      }
  }
  ```

---

## 07.5 Completion Checklist

- [ ] SIMD whitespace skipping implemented
- [ ] memchr used for comment/string scanning
- [ ] Character classification table
- [ ] Branchless character checks
- [ ] Sentinel buffer implemented
- [ ] Pre-allocation heuristics tuned
- [ ] Benchmarks show improvement
- [ ] No correctness regressions

**Exit Criteria:**
- Tokenization is measurably faster
- Memory usage is reduced
- All tests pass
- Benchmarks documented

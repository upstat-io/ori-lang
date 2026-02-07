---
section: "05"
title: SWAR & Fast Paths
status: not-started
goal: "Accelerate whitespace scanning, comment scanning, string scanning, and template body scanning using SWAR, memchr, and ASCII-optimized loops"
sections:
  - id: "05.1"
    title: SWAR Whitespace Scanning
    status: not-started
  - id: "05.2"
    title: memchr Integration
    status: not-started
  - id: "05.3"
    title: Optimized Scanning Loops
    status: not-started
  - id: "05.4"
    title: Tests & Verification
    status: not-started
---

# Section 05: SWAR & Fast Paths

**Status:** :clipboard: Planned
**Goal:** Accelerate the scanner's hot paths using SWAR (SIMD Within A Register) bit manipulation for whitespace scanning, `memchr` for delimiter searching, and sentinel-based ASCII fast paths.

> **REFERENCE**: Roc's `fast_eat_whitespace` (8-byte SWAR scanning for spaces); Rust's `cursor.eat_until` (memchr for `\n`, `"`); Go's sentinel-based ASCII fast path (`source.nextch()`); Zig's sentinel-terminated EOF detection.
>
> **Conventions:** Follows `plans/v2-conventions.md` -- SS10 (Two-Layer Pattern: all fast-path code lives in `ori_lexer_core`, not `ori_lexer`)

---

## Design Rationale

Three operations dominate lexer execution time:
1. **Whitespace skipping** -- runs of spaces/tabs between every token
2. **Comment body scanning** -- scanning to `\n` for line comments
3. **String body scanning** -- scanning to `"` or `\` for string literals

A fourth operation becomes relevant with template literal support:
4. **Template body scanning** -- scanning to `` ` ``, `{`, `}`, or `\` for template literal segments (grammar excludes all four from `template_char`)

The first three are "scan until a specific byte" operations that process many bytes without branching. SWAR and `memchr` turn these from byte-at-a-time loops into chunk-at-a-time operations. Template body scanning requires handling 4 delimiters, which exceeds `memchr3`'s efficient range, so it uses a tight byte-by-byte loop that the compiler can inline and vectorize.

### SWAR: How It Works

SWAR processes 8 bytes at a time using 64-bit integer operations. To find the first non-space byte in a chunk:

```
chunk:     [0x20, 0x20, 0x20, 0x41, 0x20, 0x20, 0x20, 0x20]  // "   A    "
XOR space: [0x00, 0x00, 0x00, 0x61, 0x00, 0x00, 0x00, 0x00]
```

Through bit manipulation, we can find the position of the first non-zero byte in O(1) using `trailing_zeros()`, processing 8 bytes with ~5 CPU instructions instead of 8 comparisons.

### memchr: SIMD-Powered Byte Search

The `memchr` crate uses SIMD (SSE2/AVX2/NEON) to find a specific byte in a haystack. It can process 16-32 bytes per cycle on modern hardware. This is ideal for:
- Line comments: find `\n` or `\r` (spec: "A lone carriage return is treated as newline")
- Strings: find `"`, `\`, `\n`, or `\r` (newlines not allowed in strings per grammar; spec treats lone `\r` as newline)

Template bodies require checking 4 delimiters (`` ` ``, `{`, `}`, `\`), which exceeds `memchr3`'s efficient range. Template scanning uses a tight byte-by-byte loop instead.

**Crate placement**: `memchr` is added as a dependency of `ori_lexer_core` (not `ori_lexer`) since it is used in the raw scanner which lives in the core crate (v2-conventions SS10).

---

## 05.1 SWAR Whitespace Scanning

- [ ] Implement `fast_eat_spaces(buf: &[u8], start: usize) -> usize`:
  ```rust
  /// Scans forward through a run of space bytes (0x20), processing 8 bytes at a time.
  /// Returns the position of the first non-space byte (or buf.len() if all spaces).
  ///
  /// Uses SWAR (SIMD Within A Register) to check 8 bytes simultaneously.
  pub fn fast_eat_spaces(buf: &[u8], start: usize) -> usize {
      let mut i = start;

      // Process 8 bytes at a time
      while i + 8 <= buf.len() {
          let chunk = u64::from_le_bytes(buf[i..i+8].try_into().unwrap());
          let spaces = 0x2020_2020_2020_2020u64;
          let xor = chunk ^ spaces;
          if xor == 0 {
              i += 8; // All 8 bytes are spaces
              continue;
          }
          // Find first non-space byte
          return i + (xor.trailing_zeros() as usize / 8);
      }

      // Process remaining bytes one at a time
      while i < buf.len() && buf[i] == b' ' {
          i += 1;
      }
      i
  }
  ```
- [ ] Implement `fast_eat_whitespace(buf: &[u8], start: usize) -> usize`:
  - Handles spaces (0x20), tabs (0x09), and carriage returns (0x0D)
  - Uses SWAR with a more complex mask to match all three
  - Roc's approach: check `chunk ^ 0x2020...` for spaces, then check for tabs/CRs in the mismatched positions
- [ ] Ensure the sentinel byte (0x00) naturally stops SWAR scanning:
  - `0x00 ^ 0x20 = 0x20` which is non-zero, so scanning stops at the sentinel
- [ ] Add simple scalar fallback for small buffers (< 8 bytes)
- [ ] Add property tests: `fast_eat_spaces` returns the same result as the naive byte-by-byte loop

---

## 05.2 memchr Integration

- [ ] Add `memchr` crate dependency to `ori_lexer_core` (it belongs in the core crate since the raw scanner uses it, per v2-conventions SS10)
- [ ] Implement `Cursor::eat_until_memchr(&mut self, byte: u8)`:
  ```rust
  /// Advance the cursor to the next occurrence of `byte`, using SIMD-accelerated search.
  /// If `byte` is not found, advances to EOF (sentinel).
  pub fn eat_until_memchr(&mut self, byte: u8) {
      let remaining = &self.buf[self.pos as usize..];
      match memchr::memchr(byte, remaining) {
          Some(offset) => self.pos += offset as u32,
          None => self.pos = self.source_len, // sentinel position
      }
  }
  ```
- [ ] Use `memchr2` in line comment scanning (scan to `\n` or `\r`, since spec treats lone CR as newline):
  ```rust
  fn line_comment(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // consume second '/'
      // Scan to newline or carriage return (spec: lone CR = newline)
      let remaining = &self.cursor.buf[self.cursor.pos as usize..];
      match memchr::memchr2(b'\n', b'\r', remaining) {
          Some(offset) => self.cursor.pos += offset as u32,
          None => self.cursor.pos = self.cursor.source_len, // sentinel
      }
      RawToken { tag: RawTag::LineComment, len: self.cursor.pos() - start }
  }
  ```
- [ ] Use `memchr3` in string scanning (find `"`, `\`, or `\n`), then verify `\r` separately:
  String scanning needs to stop on 4 bytes (`"`, `\`, `\n`, `\r`) since the spec treats a lone carriage return as a newline (02-source-code.md: "A lone carriage return is treated as newline"). The `memchr3` function only supports 3 needles, so we use `memchr3` for the three most common delimiters and then verify no `\r` appears before the found position:
  ```rust
  fn string(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // consume opening '"'
      loop {
          // Fast-scan to next interesting byte
          let remaining = &self.cursor.buf[self.cursor.pos as usize..];
          // Find the nearest of ", \, or \n
          let found = memchr::memchr3(b'"', b'\\', b'\n', remaining);
          // Also check for lone \r before that position (spec: lone CR = newline)
          let cr_pos = memchr::memchr(b'\r', remaining);
          // Take the earlier of the two
          let offset = match (found, cr_pos) {
              (Some(f), Some(c)) => Some(f.min(c)),
              (Some(f), None) => Some(f),
              (None, Some(c)) => Some(c),
              (None, None) => None,
          };
          match offset {
              Some(offset) => {
                  self.cursor.pos += offset as u32;
                  match self.cursor.current() {
                      b'"' => { self.cursor.advance(); return RawToken { tag: RawTag::String, len: self.cursor.pos() - start }; }
                      b'\\' => { self.cursor.advance(); self.cursor.advance(); } // skip escape
                      b'\n' | b'\r' => {
                          // Newline (or lone CR) in string literal is an error
                          // (grammar: string_char excludes newline; spec: lone CR = newline)
                          return RawToken { tag: RawTag::UnterminatedString, len: self.cursor.pos() - start };
                      }
                      _ => unreachable!(),
                  }
              }
              None => {
                  // Unterminated string -- advance to EOF
                  self.cursor.pos = self.cursor.source_len;
                  return RawToken { tag: RawTag::UnterminatedString, len: self.cursor.pos() - start };
              }
          }
      }
  }
  ```
  **NOTE:** The dual `memchr3`+`memchr` approach adds a second SIMD scan per loop iteration but only in the uncommon case where `\r` exists in the source. An alternative is to normalize all `\r` to `\n` during buffer creation (Section 01), which would eliminate this complexity. If profiling shows the dual scan is a bottleneck, switch to buffer normalization.
- [ ] Template literal body scanning: find `` ` ``, `{`, `}`, or `\` (grammar: `template_char` excludes all four; `template_brace` is `{{` or `}}`):
  ```rust
  fn template_body(&mut self) -> RawToken {
      let start = self.cursor.pos();
      loop {
          let remaining = &self.cursor.buf[self.cursor.pos as usize..];
          // Note: We scan for both { and } because:
          //   - {{ is template_brace (escaped opening brace)
          //   - }} is template_brace (escaped closing brace)
          //   - { alone is interpolation start
          //   - } alone in template body would be unmatched (grammar excludes it via template_char)
          // Use byte-by-byte scanning since memchr doesn't support 4+ needles efficiently
          loop {
              match self.cursor.current() {
                  b'`' => {
                      self.cursor.advance();
                      return RawToken { tag: RawTag::TemplateTail, len: self.cursor.pos() - start };
                  }
                  b'{' => {
                      // Check for {{ (escaped brace)
                      if self.cursor.peek() == b'{' {
                          self.cursor.advance();
                          self.cursor.advance();
                          continue;
                      }
                      self.cursor.advance();
                      return RawToken { tag: RawTag::TemplateMiddle, len: self.cursor.pos() - start };
                  }
                  b'}' => {
                      // Check for }} (escaped brace)
                      if self.cursor.peek() == b'}' {
                          self.cursor.advance();
                          self.cursor.advance();
                          continue;
                      }
                      // Unmatched } in template body -- error
                      return RawToken { tag: RawTag::Error, len: self.cursor.pos() - start };
                  }
                  b'\\' => {
                      self.cursor.advance();
                      self.cursor.advance(); // skip escaped char
                  }
                  0 => {
                      // EOF sentinel
                      return RawToken { tag: RawTag::UnterminatedTemplate, len: self.cursor.pos() - start };
                  }
                  _ => {
                      self.cursor.advance();
                  }
              }
          }
      }
  }
  ```
  **NOTE:** Template body scanning uses byte-by-byte instead of `memchr` because we need to check 4 delimiters (`` ` ``, `{`, `}`, `\`) and `memchr` only supports up to 3 efficiently. The compiler should inline and vectorize the inner loop. If profiling shows this is a bottleneck, we can use `memchr::memmem` or a custom SWAR approach.
- [ ] Benchmark: `memchr` vs byte-by-byte for strings and templates of various lengths

---

## 05.3 Optimized Scanning Loops

- [ ] Optimize the identifier scanning loop for pure-ASCII identifiers (>99% of real-world identifiers):
  ```rust
  fn identifier(&mut self) -> RawToken {
      let start = self.cursor.pos();
      self.cursor.advance(); // first char already validated

      // Fast ASCII path: stay in tight loop for a-z, A-Z, 0-9, _
      loop {
          let b = self.cursor.current();
          if b.is_ascii_alphanumeric() || b == b'_' {
              self.cursor.advance();
          } else {
              break; // Non-ident char (or sentinel 0x00) -- done
          }
      }

      RawToken { tag: RawTag::Ident, len: self.cursor.pos() - start }
  }
  ```
- [ ] The sentinel (0x00) falls through to the `else` branch, naturally terminating the loop
- [ ] No Unicode identifier support needed -- Ori spec (grammar line 52) restricts identifiers to ASCII: `identifier = ( letter | "_" ) { letter | digit | "_" }` where `letter = 'A' ... 'Z' | 'a' ... 'z'`
- [ ] Consider SWAR for identifier continuation scanning:
  - Create a bitmask for `[a-zA-Z0-9_]` bytes in a u64 chunk
  - Find the first non-identifier byte position
  - Only beneficial for very long identifiers (> 8 chars); benchmark to verify

---

## 05.4 Tests & Verification

- [ ] **Correctness tests**: SWAR functions produce identical results to naive scalar loops
  - Use property testing with random byte sequences
  - Test edge cases: empty input, all-spaces, single non-space, non-space at every position
- [ ] **Sentinel safety**: SWAR never reads past the sentinel in the buffer
  - Test with buffer sizes that are not multiples of 8
  - Test with very short buffers (1-7 bytes)
- [ ] **String scanning tests**: `memchr3`+`memchr` correctly finds `"`, `\`, `\n`, and `\r` in string literals
  - String with no escapes
  - String with escape sequences (`\"`, `\\`, `\n`, `\t`, `\r`, `\0`)
  - String with newline (should produce `UnterminatedString` error per grammar)
  - String with lone carriage return (should produce `UnterminatedString` -- spec: lone CR = newline)
  - String with CRLF (should produce `UnterminatedString` -- `\r` hit first)
  - Unterminated string (no closing quote)
- [ ] **Template scanning tests**: byte-by-byte scan correctly finds `` ` ``, `{`, `}`, and `\` in template bodies
  - Template with no interesting bytes (long text segment)
  - Template with escaped backtick (`` \` ``)
  - Template with escaped opening brace (`{{`)
  - Template with escaped closing brace (`}}`)
  - Template with interpolation (`{expr}`)
  - Template with unmatched `}` (should error per grammar -- `}` is excluded from `template_char`)
  - Unterminated template (no closing backtick)
- [ ] **Benchmark**: Compare throughput (bytes/sec) with and without SWAR/memchr:
  - Whitespace runs of varying lengths (1, 4, 8, 16, 64, 256 bytes)
  - String literals of varying lengths
  - Template literals of varying lengths (with and without interpolations)
  - Line comments of varying lengths
  - Full-file lexing with and without fast paths
- [ ] **Platform tests**: Verify on both x86_64 and aarch64 (if CI supports both)

---

## 05.5 Completion Checklist

- [ ] SWAR whitespace scanning implemented and tested
- [ ] `memchr` added to `ori_lexer_core` (not `ori_lexer`) per v2-conventions SS10
- [ ] `memchr2` integrated for comment scanning (`\n`, `\r` -- spec: lone CR = newline)
- [ ] `memchr3`+`memchr` integrated for string scanning (`"`, `\`, `\n`, `\r` -- newlines and lone CR forbidden per grammar/spec)
- [ ] Template body scanning uses tight byte-by-byte loop for 4 delimiters (`` ` ``, `{`, `}`, `\`)
- [ ] ASCII identifier fast path optimized (no Unicode -- ASCII-only per spec)
- [ ] Property tests verify SWAR correctness
- [ ] String scanning correctly rejects newlines and lone CR (grammar: `string_char` excludes newline; spec: lone CR = newline)
- [ ] Template scanning correctly handles `}}` (escaped closing brace per grammar)
- [ ] Template scanning correctly errors on unmatched `}` (excluded from `template_char` per grammar)
- [ ] Benchmark shows measurable improvement for whitespace/comment/string scanning
- [ ] No regressions in overall lexer throughput
- [ ] `cargo t -p ori_lexer_core` passes

**Exit Criteria:** SWAR and memchr fast paths are integrated into the raw scanner in `ori_lexer_core`. Benchmark shows measurable improvement (target >= 1.3x) for whitespace-heavy and comment-heavy inputs. String scanning uses `memchr3`+`memchr` and correctly rejects both `\n` and lone `\r` (spec: lone CR = newline). Comment scanning uses `memchr2` to stop on both `\n` and `\r`. Template body scanning handles all 4 delimiters correctly (including `}}` and unmatched `}`). All correctness tests pass.

---
section: "04"
title: "SIMD Token Boundary Extraction"
status: not-started
goal: "Convert SIMD classification bitmasks into token (tag, offset) pairs, replacing the byte-at-a-time RawScanner"
sections:
  - id: "04.1"
    title: "Boundary Bitmask Computation"
    status: not-started
  - id: "04.2"
    title: "Bitmask-to-Token Extraction"
    status: not-started
  - id: "04.3"
    title: "Special Token Handling"
    status: not-started
  - id: "04.4"
    title: "Compound Operator Resolution"
    status: not-started
  - id: "04.5"
    title: "Replace RawScanner Hot Path"
    status: not-started
  - id: "04.6"
    title: "Performance Validation"
    status: not-started
---

# Section 04: SIMD Token Boundary Extraction

**Status:** Planned
**Goal:** Use the category bitmasks from Section 03 to extract token boundaries and produce `(tag, offset)` pairs, replacing the `RawScanner::next_token()` byte-at-a-time dispatch loop.

---

## Background

Section 03 produces per-chunk bitmasks like:

```
Source:       "let  x = foo(42)\n"
              0123456789...

ALPHA mask:   110..1...111.....   (positions 0-2, 5, 9-11)
DIGIT mask:   .............11..   (positions 13-14)
WHITESPACE:   ...11.1.........1   (positions 3-4, 6)
OPERATOR:     .......1.........   (position 7: '=')
DELIMITER:    ............1..1.   (positions 12: '(', 15: ')')
NEWLINE:      ................1   (position 16)
QUOTE:        .................   (none)
```

This section converts those bitmasks into a flat token stream.

---

## 04.1 Boundary Bitmask Computation

A **token boundary** is the first byte of a new token. Boundaries occur at:

1. **Category transitions**: byte `i` has a different category from byte `i-1`
2. **Always-boundary bytes**: OPERATOR, DELIMITER, QUOTE, NEWLINE bits set

```rust
fn compute_boundaries(chunk: &ClassifiedChunk, prev_chunk_last_category: u8) -> u32 {
    // "Runs" of same-category bytes (identifiers, whitespace, digits)
    // produce boundaries only at their start.
    //
    // Operators, delimiters, quotes, and newlines are ALWAYS boundaries
    // (even when adjacent: `==` is two separate boundary positions that
    // get merged into a compound token in Section 04.4).

    let always_boundary = chunk.operator | chunk.delimiter | chunk.quote | chunk.newline;

    // Category transition detection:
    // For ident_cont runs and whitespace runs, a boundary occurs when
    // the previous byte was NOT the same category.
    let ident_starts = chunk.ident_cont & !(chunk.ident_cont << 1 | carry_from_prev);
    let ws_starts = chunk.whitespace & !(chunk.whitespace << 1 | ws_carry);
    let digit_starts = chunk.digit & !(chunk.digit << 1 | digit_carry);

    always_boundary | ident_starts | ws_starts | digit_starts
}
```

- [ ] Implement `compute_boundaries()` for AVX2 (operates on u32 bitmasks)
- [ ] Implement `compute_boundaries()` for NEON (operates on u16 bitmasks)
- [ ] Handle cross-chunk carry: last byte of previous chunk's category must propagate
  - [ ] Store `prev_last_category: u8` between chunks
- [ ] Handle the first chunk (no previous byte → everything is a boundary at position 0)
- [ ] Unit tests: verify boundaries for representative source fragments

---

## 04.2 Bitmask-to-Token Extraction

Convert a `u32` boundary bitmask into a sequence of `(tag, offset)` pairs.

```rust
fn extract_tokens(
    boundary_mask: u32,
    chunk_offset: u32,
    source: &[u8],
    classification: &ClassifiedChunk,
    output: &mut CompactTokenStream,
) {
    let mut mask = boundary_mask;
    while mask != 0 {
        let bit_pos = mask.trailing_zeros(); // tzcnt: 1 cycle
        let byte_offset = chunk_offset + bit_pos;
        let category = classify_byte_at(classification, bit_pos);
        let tag = category_to_raw_tag(source, byte_offset, category);
        output.push(tag as u8, byte_offset, 0); // flags filled later
        mask &= mask - 1; // clear lowest set bit: 1 cycle
    }
}
```

- [ ] Implement `extract_tokens()` with `tzcnt` loop
- [ ] Implement `category_to_raw_tag()` — map category bits to `RawTag`
  - [ ] ALPHA at identifier start → `RawTag::Ident`
  - [ ] DIGIT at number start → `RawTag::Int` (refined later by compound resolution)
  - [ ] WHITESPACE → `RawTag::Whitespace`
  - [ ] NEWLINE → `RawTag::Newline`
  - [ ] OPERATOR → dispatch on actual byte to specific `RawTag` (e.g., `+` → `Plus`)
  - [ ] DELIMITER → dispatch on actual byte (e.g., `(` → `LeftParen`)
  - [ ] QUOTE → special handling (see 04.3)
- [ ] Benchmark: tokens extracted per cycle (target: >1 billion tokens/sec for operators)

---

## 04.3 Special Token Handling

Some token types cannot be fully resolved by SIMD classification alone. These require scalar post-processing:

### Strings, Chars, and Templates (QUOTE category)

When the SIMD pass encounters a QUOTE byte (`"`, `'`, `` ` ``), it must hand off to the existing `memchr`-based scanning logic to find the closing delimiter. This is because string/template bodies can contain arbitrary bytes including category-triggering characters.

```rust
// When QUOTE bit is set at position i:
match source[byte_offset] {
    b'"'  => skip_string_body(cursor),   // reuse existing memchr logic
    b'\'' => skip_char_body(cursor),     // reuse existing logic
    b'`'  => skip_template_body(cursor), // reuse existing logic + depth tracking
}
```

- [ ] Integrate existing `string()`, `char_literal()`, `template_literal()` scanning
- [ ] After scanning string/template body, resume SIMD classification at the next chunk boundary
- [ ] Handle template interpolation depth tracking (reuse existing `template_depth` Vec)

### Comments (`//`)

When SIMD sees `/` (OPERATOR) followed by `/` (OPERATOR), it must switch to comment scanning:

- [ ] Detect `//` pattern in boundary extraction
- [ ] Hand off to `eat_until_newline_or_eof()` (existing memchr-based)
- [ ] Emit `RawTag::LineComment` spanning to the newline
- [ ] Resume SIMD at next chunk after the newline

### Interior Null Bytes

- [ ] Null bytes (0x00) classify as no category (all bits 0)
- [ ] Boundary extraction produces a boundary at the null byte position
- [ ] Tag assigned: `RawTag::InteriorNull`
- [ ] Sentinel null at EOF: detected by offset >= source_len (same as current)

---

## 04.4 Compound Operator Resolution

SIMD classification sees each operator byte individually. Multi-byte operators (`==`, `!=`, `->`, `..=`, etc.) need post-processing to merge adjacent operator boundaries:

```
Source: "x == y"
SIMD boundaries: x(0), =(3), =(4), y(6)
After merging:   x(0), ==(3), y(6)
```

Strategy: **forward peek during extraction**. When extracting an OPERATOR boundary, peek at the next 1-2 bytes to check for compound operators:

```rust
fn resolve_operator(source: &[u8], offset: u32) -> (RawTag, u32) {
    match source[offset as usize] {
        b'=' => match source.get(offset as usize + 1) {
            Some(b'=') => (RawTag::EqualEqual, 2),
            Some(b'>') => (RawTag::FatArrow, 2),
            _ => (RawTag::Equal, 1),
        },
        b'-' => match source.get(offset as usize + 1) {
            Some(b'>') => (RawTag::Arrow, 2),
            _ => (RawTag::Minus, 1),
        },
        b'.' => match source.get(offset as usize + 1) {
            Some(b'.') => match source.get(offset as usize + 2) {
                Some(b'=') => (RawTag::DotDotEqual, 3),
                Some(b'.') => (RawTag::DotDotDot, 3),
                _ => (RawTag::DotDot, 2),
            },
            _ => (RawTag::Dot, 1),
        },
        // ... same logic as current RawScanner operator methods
    }
}
```

- [ ] Implement `resolve_operator()` with all compound operators from current `RawScanner`
- [ ] When a compound operator consumes N bytes, skip the next N-1 boundary positions
- [ ] Handle `::` (colon-colon) — colons are DELIMITER category
- [ ] Handle `#[` and `#!` — hash is DELIMITER category
- [ ] Unit tests: all compound operators from `RawTag` enum
- [ ] Verify: compound resolution produces same tokens as current `RawScanner` for all operator sequences

---

## 04.5 Replace RawScanner Hot Path

The SIMD scanner replaces `RawScanner::next_token()` for the main scanning loop. The architecture:

```rust
pub struct SimdScanner<'a> {
    source: &'a [u8],
    source_len: u32,
    pos: u32,
    template_depth: Vec<InterpolationDepth>, // reused from RawScanner

    // SIMD state
    #[cfg(target_arch = "x86_64")]
    lo_lut: __m256i,
    #[cfg(target_arch = "x86_64")]
    hi_lut: __m256i,

    #[cfg(target_arch = "aarch64")]
    lo_lut: uint8x16_t,
    #[cfg(target_arch = "aarch64")]
    hi_lut: uint8x16_t,
}

impl<'a> SimdScanner<'a> {
    /// Scan the entire source and produce a CompactTokenStream.
    ///
    /// This replaces the token-at-a-time RawScanner with a bulk
    /// chunk-at-a-time pass.
    pub fn scan_all(&mut self) -> CompactTokenStream {
        let mut output = CompactTokenStream::with_capacity(self.source_len as usize);
        let mut pos = 0u32;

        while pos + 32 <= self.source_len {
            let chunk = self.classify_chunk(pos);
            let boundaries = compute_boundaries(&chunk, ...);
            self.extract_tokens(boundaries, pos, &chunk, &mut output);
            pos += 32;
        }

        // Scalar tail for remaining < 32 bytes
        self.scan_tail(pos, &mut output);

        output.seal(self.source_len);
        output
    }
}
```

- [ ] Implement `SimdScanner` struct
- [ ] Implement `scan_all()` bulk scanning method
- [ ] Implement scalar tail handling (< 32 bytes remaining)
  - [ ] Reuse existing `RawScanner` logic for the tail
  - [ ] Or: rely on sentinel zero-padding to classify the partial chunk safely
- [ ] Handle mode switches:
  - [ ] When QUOTE is encountered → switch to scalar string/template scanning
  - [ ] When `//` comment detected → switch to memchr newline scan
  - [ ] After scalar section completes → resume SIMD at next 32-byte aligned position
- [ ] Wire `SimdScanner` into `lex_with_comments()` replacing `RawScanner`
- [ ] Verify: `SimdScanner` produces identical `CompactTokenStream` as `RawScanner` + conversion for all test inputs

---

## 04.6 Performance Validation

- [ ] Run `/benchmark short` before changes (record baseline — should include Section 01+02 gains)
- [ ] Run `/benchmark short` after SIMD integration
- [ ] Microbenchmark: scan 1 MB of Ori source
  - [ ] Target throughput: >1 GB/s on x86_64, >500 MB/s on aarch64
  - [ ] Compare to V2 RawScanner throughput
- [ ] `perf stat` analysis:
  - [ ] IPC (instructions per cycle) — SIMD should increase this significantly
  - [ ] Branch mispredictions — should drop substantially (SIMD replaces 256-way match)
  - [ ] L1 cache misses — should be negligible (streaming access pattern)
- [ ] No regressions in any existing test
- [ ] Profile with `callgrind`: verify SIMD classification is now the dominant cost (not cooking, not Vec allocation)

**Exit Criteria:** SIMD scanner produces identical token streams to V2 RawScanner for all existing test inputs and spec tests. Scanning throughput exceeds 1 GB/s on x86_64. Total lexer speedup (from Section 01 baseline) exceeds 4x.

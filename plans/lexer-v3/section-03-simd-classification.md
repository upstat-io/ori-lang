---
section: "03"
title: "SIMD Byte Classification"
status: not-started
goal: "Replace byte-at-a-time RawScanner dispatch with SIMD 32-byte chunk classification using AVX2/NEON nibble lookup"
sections:
  - id: "03.1"
    title: "Byte Category Design"
    status: not-started
  - id: "03.2"
    title: "Nibble Lookup Table Construction"
    status: not-started
  - id: "03.3"
    title: "AVX2 Implementation"
    status: not-started
  - id: "03.4"
    title: "NEON Implementation"
    status: not-started
  - id: "03.5"
    title: "Scalar Reference Implementation"
    status: not-started
  - id: "03.6"
    title: "Property Testing"
    status: not-started
  - id: "03.7"
    title: "Performance Validation"
    status: not-started
---

# Section 03: SIMD Byte Classification

**Status:** Planned
**Goal:** Classify all 256 possible byte values into token-relevant categories using SIMD nibble lookup tables, processing 32 bytes per cycle on AVX2 and 16 bytes per cycle on NEON.

---

## Background

### The simdjson Nibble-Lookup Trick

The key insight (from Geoff Langdale & Daniel Lemire's simdjson paper) is that any byte-to-category mapping for all 256 byte values can be encoded as two 16-entry lookup tables indexed by the low and high nibbles:

```
byte 0x41 ('A'):
  low nibble  = 0x1 → LO_LUT[0x1] = 0b0000_0110  (ALPHA | IDENT)
  high nibble = 0x4 → HI_LUT[0x4] = 0b0000_0110  (ALPHA | IDENT)
  AND result  =       0b0000_0110  (ALPHA & IDENT confirmed)

byte 0x28 ('('):
  low nibble  = 0x8 → LO_LUT[0x8] = 0b0001_1000  (DELIM | OPEN)
  high nibble = 0x2 → HI_LUT[0x2] = 0b0001_0000  (DELIM)
  AND result  =       0b0001_0000  (DELIM confirmed)
```

The `vpshufb` / `vqtbl1q_u8` instruction implements this lookup in a single cycle for 16/32 bytes. Two lookups + AND = 3 instructions to classify an entire chunk.

### Limitation: 8 Category Bits

Each LUT entry is 8 bits, so we can distinguish at most 8 categories. This is sufficient because the lexer only needs coarse classification — fine-grained token identification happens in the boundary extraction pass (Section 04).

---

## 03.1 Byte Category Design

Define 8 category bits that capture all information needed for token boundary detection:

```
Bit 0: ALPHA      — a-z, A-Z (identifier start / keyword start)
Bit 1: DIGIT      — 0-9 (number start / continuation)
Bit 2: IDENT_CONT — a-z, A-Z, 0-9, _ (identifier continuation)
Bit 3: WHITESPACE — space (0x20), tab (0x09)
Bit 4: OPERATOR   — +, -, *, /, %, ^, &, |, ~, !, =, <, >, ?, .
Bit 5: DELIMITER  — (, ), [, ], {, }, ,, :, ;, @, #, $, \, _
Bit 6: QUOTE      — ", ', ` (string/char/template start)
Bit 7: NEWLINE    — \n (0x0A), \r (0x0D)
```

- [ ] Map all 256 byte values to their category bitmask
- [ ] Verify coverage: every non-null byte maps to at least one category
- [ ] Verify mutual exclusion where needed: ALPHA implies IDENT_CONT
- [ ] Document edge cases:
  - [ ] `_` (0x5F): IDENT_CONT + DELIMITER (standalone `_` is a token, `_foo` is ident)
  - [ ] `.` (0x2E): OPERATOR (could start `..`, `...`, `..=`)
  - [ ] `-` (0x2D): OPERATOR (could start `->`)
  - [ ] `0` (0x30): DIGIT (could start `0x`, `0b`)
  - [ ] `/` (0x2F): OPERATOR (could start `//` comment)
  - [ ] `#` (0x23): DELIMITER (could start `#[`, `#!`)
  - [ ] `\r` (0x0D): NEWLINE (could start `\r\n` CRLF)

### Token Boundary Detection Rule

A **token boundary** occurs when:
1. The category of byte `i` differs from the category of byte `i-1` (category transition), OR
2. Byte `i` has the OPERATOR, DELIMITER, QUOTE, or NEWLINE bit set (these are always boundaries)

This means:
- Identifier runs (`foobar`) produce boundaries only at start and end
- Whitespace runs (`    `) produce boundaries only at start and end
- Operators are always individual boundaries (even when adjacent: `==` is two boundaries)
- Quotes start special scanning (string/template body — hand off to existing memchr logic)

---

## 03.2 Nibble Lookup Table Construction

- [ ] Construct `LO_LUT[16]` and `HI_LUT[16]` such that `LO_LUT[byte & 0x0F] & HI_LUT[byte >> 4]` gives the correct category bits for all 256 byte values
- [ ] This is a constraint satisfaction problem:
  - [ ] For each byte value, `LO_LUT[lo] & HI_LUT[hi]` must equal the target category
  - [ ] Some entries are over-determined (conflicts) — resolve by adding post-classification fixup
  - [ ] Known conflict: `\t` (0x09) shares low nibble with `\n`-adjacent values
- [ ] Implement exhaustive verification: loop over all 256 bytes, assert classification matches
- [ ] Write the tables as `const` arrays for compile-time evaluation

### Expected Table Shape

```rust
/// Low nibble lookup table (16 entries, loaded into SIMD register).
const LO_LUT: [u8; 16] = [
    // 0x0: NULL/control → 0
    // 0x1: ...
    // ...
    // 0x9: tab → WHITESPACE
    // 0xA: newline → NEWLINE
    // ...
];

/// High nibble lookup table (16 entries, loaded into SIMD register).
const HI_LUT: [u8; 16] = [
    // 0x0: control chars (0x00-0x0F)
    // 0x2: space(0x20), !"#$%&'()*+,-./ (0x21-0x2F)
    // 0x3: digits (0x30-0x39), :;<=>? (0x3A-0x3F)
    // 0x4: @, A-O (0x40-0x4F)
    // 0x5: P-Z, [\]^_ (0x50-0x5F)
    // 0x6: `, a-o (0x60-0x6F)
    // 0x7: p-z, {|}~ (0x70-0x7F)
    // 0x8-0xF: non-ASCII (set no bits, or INVALID bit)
];
```

- [ ] Build a Rust build-script or const-fn that generates and validates the tables
- [ ] Property test: `for byte in 0..=255 { assert_eq!(classify_scalar(byte), classify_simd_single(byte)); }`

---

## 03.3 AVX2 Implementation (x86_64)

Target: process 32 bytes per iteration.

```rust
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn classify_chunk_avx2(
    src: *const u8,
    lo_lut: __m256i,
    hi_lut: __m256i,
) -> ClassifiedChunk {
    let chunk = _mm256_loadu_si256(src.cast());

    // Nibble-based classification
    let mask_0f = _mm256_set1_epi8(0x0F);
    let lo_nibbles = _mm256_and_si256(chunk, mask_0f);
    let hi_nibbles = _mm256_and_si256(_mm256_srli_epi16(chunk, 4), mask_0f);

    let lo_class = _mm256_shuffle_epi8(lo_lut, lo_nibbles);
    let hi_class = _mm256_shuffle_epi8(hi_lut, hi_nibbles);
    let classification = _mm256_and_si256(lo_class, hi_class);

    // Extract bitmasks per category
    ClassifiedChunk {
        alpha:      extract_bit_mask(classification, 0), // u32 bitmask
        digit:      extract_bit_mask(classification, 1),
        ident_cont: extract_bit_mask(classification, 2),
        whitespace: extract_bit_mask(classification, 3),
        operator:   extract_bit_mask(classification, 4),
        delimiter:  extract_bit_mask(classification, 5),
        quote:      extract_bit_mask(classification, 6),
        newline:    extract_bit_mask(classification, 7),
    }
}
```

- [ ] Implement `classify_chunk_avx2()`
- [ ] Implement `extract_bit_mask()` using `_mm256_movemask_epi8` after isolating each bit
  - [ ] For bit N: `_mm256_movemask_epi8(_mm256_slli_epi16(classification, 7 - N))`
  - [ ] This extracts the Nth bit of each byte into a u32 bitmask
- [ ] Implement `ClassifiedChunk` struct (8 x `u32` bitmasks)
- [ ] Handle partial chunks (< 32 bytes remaining before EOF)
  - [ ] Option A: masked load (`_mm256_maskload_epi32` — complex)
  - [ ] Option B: rely on sentinel padding (SourceBuffer already guarantees 64 bytes of zero padding)
  - [ ] Recommend Option B: zero-padded bytes classify as "nothing" (category 0), which produces no false boundaries
- [ ] Unit tests: verify classification of known byte sequences
- [ ] Benchmark: cycles per 32-byte chunk

---

## 03.4 NEON Implementation (aarch64)

Target: process 16 bytes per iteration (2x calls to cover 32 bytes).

```rust
#[cfg(target_arch = "aarch64")]
unsafe fn classify_chunk_neon(
    src: *const u8,
    lo_lut: uint8x16_t,
    hi_lut: uint8x16_t,
) -> ClassifiedChunk16 {
    let chunk = vld1q_u8(src);

    let mask_0f = vdupq_n_u8(0x0F);
    let lo_nibbles = vandq_u8(chunk, mask_0f);
    let hi_nibbles = vshrq_n_u8(chunk, 4);

    let lo_class = vqtbl1q_u8(lo_lut, lo_nibbles);
    let hi_class = vqtbl1q_u8(hi_lut, hi_nibbles);
    let classification = vandq_u8(lo_class, hi_class);

    // NEON doesn't have movemask — extract bits differently
    // Use narrowing shifts + horizontal operations, or
    // use the shrn/ushr approach from simdjson's ARM port
    extract_bitmasks_neon(classification)
}
```

- [ ] Implement `classify_chunk_neon()`
- [ ] Implement bitmask extraction for NEON
  - [ ] NEON lacks `movemask` — use the `shrn` narrowing trick or byte-by-byte extraction
  - [ ] Alternative: use `vgetq_lane_u64` to extract as two u64s and process with scalar bit ops
  - [ ] Benchmark both approaches, pick faster
- [ ] Process 32 bytes by calling twice (two 16-byte chunks)
- [ ] Same padding/sentinel strategy as AVX2
- [ ] Unit tests: same test cases as AVX2
- [ ] Benchmark: cycles per 16-byte chunk

---

## 03.5 Scalar Reference Implementation

For testing and validation, not for production use.

- [ ] Implement `classify_byte_scalar(byte: u8) -> u8` using `static TABLE: [u8; 256]`
- [ ] Implement `classify_chunk_scalar(src: &[u8]) -> ClassifiedChunk` using byte-at-a-time loop
- [ ] This is the oracle for property testing

---

## 03.6 Property Testing

- [ ] Exhaustive byte coverage: all 256 values produce correct category bits
- [ ] AVX2 matches scalar for random 32-byte inputs
- [ ] NEON matches scalar for random 16-byte inputs
- [ ] Boundary detection: category transitions correctly identified
- [ ] Sentinel handling: null bytes produce no false boundaries
- [ ] UTF-8 multi-byte: non-ASCII bytes (0x80-0xFF) produce no categories (handled separately)
- [ ] Fuzz: random source strings produce same token boundaries as V2 scanner

---

## 03.7 Performance Validation

- [ ] Run `/benchmark short` before changes (record baseline)
- [ ] Microbenchmark: classify 1 MB of representative Ori source
  - [ ] Target: >2 GB/s on x86_64, >1 GB/s on aarch64
  - [ ] Current V2 scanning: ~200-500 MB/s estimated (byte-at-a-time)
- [ ] Run `/benchmark short` after integration
- [ ] No regressions >5% vs baseline (classification is additive, not replacing V2 yet)
- [ ] `perf stat`: measure IPC improvement

**Exit Criteria:** SIMD classification produces identical category bitmasks to scalar reference for all inputs. Throughput exceeds 1 GB/s on both x86_64 and aarch64. Property tests pass on 10K+ random inputs.

---
plan: "lexer-v3"
title: "Lexer V3: SIMD-Accelerated Compact Token Stream"
status: in-progress
goal: "8-12x lexer throughput via compact SoA token storage, lazy cooking, and SIMD byte classification"
sections:
  - id: "01"
    title: "Compact Token Stream (SoA)"
    status: done
  - id: "02"
    title: "Lazy Cooking"
    status: in-progress
  - id: "03"
    title: "SIMD Byte Classification"
    status: not-started
  - id: "04"
    title: "SIMD Token Boundary Extraction"
    status: not-started
  - id: "05"
    title: "Parser Adaptation"
    status: in-progress
  - id: "06"
    title: "Integration & Benchmarks"
    status: not-started
---

# Lexer V3: SIMD-Accelerated Compact Token Stream

**Status:** In Progress
**Goal:** Achieve 8-12x lexer throughput improvement through three synergistic techniques: compact Structure-of-Arrays token storage, lazy/deferred token cooking, and SIMD-accelerated byte classification.

---

## Motivation

The lexer is the first stage of the Ori compiler pipeline. Every `ori check`, `ori run`, `ori build`, and `ori fmt` invocation begins with lexing. Its performance sets the floor for the entire compilation pipeline. Improvements here cascade:

1. **Parser runs faster** because the token stream is more cache-dense
2. **Salsa early-cutoff is cheaper** because token equality checks hash less data
3. **Memory pressure drops** across all downstream phases
4. **Cold start time** (first compile, no Salsa cache) improves proportionally

## Current Architecture (V2)

```
source &str
  --> SourceBuffer (sentinel-terminated, cache-line aligned)
  --> Cursor (zero-cost, no bounds checks)
  --> RawScanner (byte-at-a-time dispatch, 256-way match)
  --> (RawTag, len) pairs
  --> TokenCooker (keyword resolution, interning, numeric parsing, escape processing)
  --> TokenList { tokens: Vec<Token>, tags: Vec<u8>, flags: Vec<TokenFlags> }
```

### Current Performance Characteristics

| Metric | Current | Notes |
|--------|---------|-------|
| Token size | 24 bytes (`TokenKind` 16 + `Span` 8) | Bloated by `Duration(u64, DurationUnit)` variant |
| Per-token storage | 26 bytes (24 + 1 tag + 1 flags) | ~2.7 tokens per cache line |
| Scanning model | Byte-at-a-time | 256-way match, ~3 bytes/cycle |
| Cooking model | Eager (all tokens cooked immediately) | Wasted work on error recovery, Salsa cutoff |
| SIMD usage | memchr for strings/comments/templates | Not used for general token scanning |

### Existing Optimizations (Keep)

- Sentinel-terminated buffer (no bounds checks)
- Cache-line aligned `SourceBuffer` with prefetch
- SIMD `memchr` for string/comment/template body scanning
- SWAR whitespace counting
- `IS_IDENT_CONTINUE_TABLE` lookup table
- Length-bucketed keyword dispatch with pre-filters
- `unsafe` `from_utf8_unchecked` on hot path (`slice_source`)
- Pre-allocated output buffers based on source length heuristic

## Target Architecture (V3)

```
source &str
  --> SourceBuffer (unchanged)
  --> SIMD Classification Pass (32 bytes/cycle, AVX2/NEON)
  --> Category bitmasks + token boundary positions
  --> CompactTokenStream { tags: Vec<u8>, offsets: Vec<u32>, flags: Vec<u8> }
  --> Lazy Cooker (on-demand: keyword lookup, interning, parsing)
  --> TokenKind values (only when parser inspects)
```

### Target Performance Characteristics

| Metric | Target | Improvement |
|--------|--------|-------------|
| Per-token storage | 6 bytes (1 tag + 4 offset + 1 flags) | 4.3x denser |
| Tokens per cache line | ~10 | 3.7x more |
| Scanning model | SIMD 32-byte chunks | 8-10x scanning throughput |
| Cooking model | Lazy (cook on parser access) | 30-50% work avoided |
| Expected overall | 8-12x throughput | Combined effect |

## Platform Requirements

| Platform | ISA | Instruction | Status |
|----------|-----|-------------|--------|
| x86_64 | AVX2 (2013+) | `vpshufb`, `vpmovmskb` | Required baseline |
| aarch64 | NEON (ARMv8+) | `vqtbl1q_u8` | Required baseline |
| Pre-2020 hardware | - | - | Not supported |

No scalar fallback. No runtime feature detection. Compile-time `#[cfg(target_arch)]` dispatch only.

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Parser API breakage | High | Section 05 manages incremental migration |
| SIMD correctness bugs | Medium | Extensive property testing, scalar reference impl |
| Salsa compatibility | Low | CompactTokenStream implements same traits |
| String/template edge cases in SIMD | Medium | Reuse existing memchr-based scanning for these |
| Benchmark doesn't show 8x | Low-Medium | Compact stream alone gives 2-4x; SIMD stacks |

## Prior Art

- **simdjson**: SIMD structural index for JSON, 2.5 GB/s parsing. Same nibble-lookup classification technique.
- **tree-sitter**: Exploring SIMD lexer acceleration (in development).
- **Zig compiler**: SoA data-oriented design throughout. Lexer uses compact token storage.
- **Rust `logos`**: Lookup-table-driven lexer generator, similar byte classification idea but without SIMD.
- **ripgrep**: memchr + SIMD for byte searching, same pattern we'd extend to full classification.

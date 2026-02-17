---
section: "06"
title: "Integration & Benchmarks"
status: not-started
goal: "Validate end-to-end correctness and measure combined 8-12x throughput improvement"
sections:
  - id: "06.1"
    title: "End-to-End Correctness"
    status: not-started
  - id: "06.2"
    title: "Throughput Benchmarks"
    status: not-started
  - id: "06.3"
    title: "Cache Analysis"
    status: not-started
  - id: "06.4"
    title: "Regression Testing"
    status: not-started
  - id: "06.5"
    title: "Documentation & Cleanup"
    status: not-started
---

# Section 06: Integration & Benchmarks

**Status:** Planned
**Goal:** Validate the combined Lexer V3 system end-to-end, measure the actual throughput improvement, and ensure no regressions across the entire compiler pipeline.

---

## 06.1 End-to-End Correctness

Verify that the entire compiler pipeline produces identical results with the V3 lexer:

- [ ] `cargo t` — all Rust unit tests pass
- [ ] `cargo st` — all Ori spec tests pass
- [ ] `./test-all.sh` — full test suite passes
- [ ] `./llvm-test.sh` — AOT compilation tests pass
- [ ] `./clippy-all.sh` — no new warnings
- [ ] Manual smoke tests:
  - [ ] `ori check library/std/prelude.ori` — stdlib passes type checking
  - [ ] `ori run` on representative programs
  - [ ] `ori fmt` produces identical output (formatter depends on comment positions)
  - [ ] Error messages have correct spans (offsets are computed differently now)
- [ ] Edge case validation:
  - [ ] Empty file
  - [ ] File with only comments
  - [ ] File with only whitespace
  - [ ] File with deeply nested templates (`\`..{..{..}..}\``)
  - [ ] File at exactly 32-byte boundaries (SIMD chunk edge cases)
  - [ ] File with mixed CRLF/LF line endings
  - [ ] File with interior null bytes
  - [ ] File with BOM (should error)
  - [ ] File with Unicode identifiers / confusables (should error)
  - [ ] Maximum-length identifier
  - [ ] Maximum-length string literal
  - [ ] Integer overflow in literals

---

## 06.2 Throughput Benchmarks

### Microbenchmarks (Lexer-Isolated)

- [ ] Create dedicated benchmark harness in `compiler/ori_lexer/benches/` (or `ori_lexer_core/benches/`)
- [ ] Benchmark inputs:
  - [ ] `prelude.ori` — dense stdlib definitions, keyword-heavy
  - [ ] Synthetic: 10K lines of mixed Ori code (identifiers, literals, operators)
  - [ ] Synthetic: operator-heavy (many single-char tokens)
  - [ ] Synthetic: string-heavy (long strings with escapes)
  - [ ] Synthetic: comment-heavy (50% comments)
  - [ ] Synthetic: whitespace-heavy (deep indentation)
- [ ] Metrics per benchmark:
  - [ ] **Throughput**: MB/s of source processed
  - [ ] **Tokens/sec**: raw scanning rate
  - [ ] **Bytes/token**: output compactness
  - [ ] **Cook rate**: fraction of tokens actually cooked

### Target Throughput

| Scenario | V2 Estimated | V3 Target | Improvement |
|----------|-------------|-----------|-------------|
| Mixed Ori code | ~300 MB/s | >2 GB/s | >6x |
| Keyword-heavy | ~200 MB/s | >1.5 GB/s | >7x |
| String-heavy | ~400 MB/s | >2 GB/s | >5x (memchr dominates) |
| Comment-heavy | ~500 MB/s | >3 GB/s | >6x (memchr + skip) |
| Operator-heavy | ~250 MB/s | >2.5 GB/s | >10x |

### End-to-End Benchmarks

- [ ] `ori check` on `library/std/prelude.ori` — cold (no Salsa cache)
- [ ] `ori check` on `library/std/prelude.ori` — warm (Salsa cached, whitespace edit)
- [ ] `ori check` on a large synthetic file (1000+ functions)
- [ ] `/benchmark short` — standard benchmark suite

### Comparison Points

- [ ] V2 lexer (current) — baseline
- [ ] V3 compact stream only (Section 01) — cache density improvement
- [ ] V3 compact + lazy cooking (Section 01+02) — cooking avoidance
- [ ] V3 compact + lazy + SIMD (Section 01+02+03+04) — full pipeline
- [ ] V3 full with parser adapted (Section 01-05) — end-to-end

Record all five data points to quantify each contribution.

---

## 06.3 Cache Analysis

- [ ] `perf stat -e cache-references,cache-misses,L1-dcache-load-misses,LLC-load-misses`
  - [ ] Compare V2 vs V3 on representative input
  - [ ] Target: >50% reduction in L1 cache misses
- [ ] `perf stat -e branches,branch-misses`
  - [ ] Compare V2 (256-way match) vs V3 (SIMD + tzcnt)
  - [ ] Target: >60% reduction in branch mispredictions
- [ ] `perf stat -e instructions,cycles`
  - [ ] IPC improvement (expect V3 > V2 due to SIMD)
- [ ] `cachegrind` analysis
  - [ ] Instruction cache: verify SIMD loop fits in L1i
  - [ ] Data cache: verify token stream fits in L1d/L2 for typical files

---

## 06.4 Regression Testing

Ensure no behavioral changes anywhere in the pipeline:

- [ ] Span accuracy: all error messages point to correct source locations
  - [ ] Run error-producing test cases, compare diagnostic output character-by-character
- [ ] Salsa early-cutoff: whitespace-only edits still skip re-parsing
  - [ ] Test: lex file, edit whitespace, re-lex, verify CompactTokenStream equality
- [ ] Formatter output: `ori fmt` produces byte-identical output
  - [ ] Run formatter on all stdlib files, diff against pre-V3 output
- [ ] Comment positions: comments captured at correct spans
  - [ ] Critical for doc generation and formatter
- [ ] TokenFlags: all flag bits (SPACE_BEFORE, NEWLINE_BEFORE, TRIVIA_BEFORE, LINE_START, HAS_ERROR, IS_DOC, ADJACENT, CONTEXTUAL_KW) set identically to V2
  - [ ] Write a comparison test that lexes with both V2 and V3, asserts identical flags sequence

---

## 06.5 Documentation & Cleanup

- [ ] Update `CLAUDE.md` with new lexer architecture description
- [ ] Update `.claude/rules/compiler.md` with V3 lexer details
- [ ] Update module-level `//!` docs in:
  - [ ] `ori_lexer_core/src/lib.rs`
  - [ ] `ori_lexer/src/lib.rs`
  - [ ] `ori_ir/src/token/mod.rs`
- [ ] Remove dead code:
  - [ ] Legacy `TokenList` if fully replaced
  - [ ] Old `RawScanner` if fully replaced by `SimdScanner`
  - [ ] Or: keep `RawScanner` as scalar reference for testing
- [ ] Update tracing instrumentation for V3 path
- [ ] Add `ORI_LOG=ori_lexer=debug` support for the SIMD scanner
- [ ] Run `./clippy-all.sh` and `./fmt-all.sh` final clean

**Exit Criteria:** All tests pass. Throughput exceeds 1 GB/s on x86_64 for representative Ori source. Combined improvement from V2 baseline is at least 6x (stretch goal: 8-12x). No behavioral regressions in any compiler phase. Documentation reflects new architecture.

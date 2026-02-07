---
section: "10"
title: Benchmarking & Performance Validation
status: not-started
goal: "Establish benchmarks, measure V2 performance against V1 baseline, and validate the 1.5x throughput target"
sections:
  - id: "10.1"
    title: Baseline Measurement
    status: not-started
  - id: "10.2"
    title: Benchmark Suite
    status: not-started
  - id: "10.3"
    title: Tier Checkpoints
    status: not-started
  - id: "10.4"
    title: Profiling & Optimization
    status: not-started
  - id: "10.5"
    title: Final Validation
    status: not-started
---

# Section 10: Benchmarking & Performance Validation

**Status:** :clipboard: Planned
**Goal:** Measure V2 lexer performance against V1 baseline at every tier boundary. Validate the target of >= 1.5x throughput improvement. Profile and optimize hot paths.

> **REFERENCE**: Existing benchmark infrastructure at `compiler/oric/benches/lexer.rs` (lexer), `compiler/oric/benches/parser.rs` (parser), `compiler/oric/benches/memory.rs` (memory profiling); v2-conventions §9 capacity heuristic (6:1 source:token ratio, from Zig tokenizer measurements); Go's `testing.B` benchmark patterns.
>
> **CONVENTIONS**: Follows `plans/v2-conventions.md` §9 (Capacity Estimation).

---

## Design Rationale

Performance is a primary motivation for the V2 lexer. Every architectural decision (sentinel buffer, hand-written scanner, SoA storage, SWAR, memchr) is chosen for performance. We must validate these decisions with data.

### Measurement Strategy

Three levels of measurement:

1. **Microbenchmarks**: Isolated component performance (whitespace scanning, keyword lookup, string scanning)
2. **End-to-end lexer**: Full `lex()` throughput in bytes/sec and tokens/sec
3. **Full pipeline**: Impact on `ori check` and `ori run` wall-clock time

### V1 Baseline (Measured February 2026)

These are the numbers to beat. Measured on the existing logos-based lexer.

**Lexer Raw Throughput:**

| Workload | Throughput | Input Size |
|----------|-----------|------------|
| 10 funcs | 234 MiB/s | ~295 B |
| 50 funcs | 255 MiB/s | ~1.5 KB |
| 100 funcs | 264 MiB/s | ~3 KB |
| 500 funcs | 290 MiB/s | ~16 KB |
| 1000 funcs | 286 MiB/s | ~33 KB |
| 5000 funcs | 288 MiB/s | ~174 KB |

**Realistic Workloads:**

| Workload | Lexer Throughput | Parser Throughput (lex+parse) |
|----------|-----------------|------------------------------|
| Small (~1 KB) | 259 MiB/s | 131 MiB/s |
| Medium (~10 KB) | 281 MiB/s | 154 MiB/s |
| Large (~50 KB) | 292 MiB/s | 159 MiB/s |

Parser throughput includes tag-based dispatch and direct-append arena optimizations (commit 3706d8f, +12-16%).

**Token Throughput:** ~119 Mtokens/s

**Benchmark Command:** `cargo bench -p oric --bench lexer -- "lexer/raw" --noplot`

### Industry Reference Points

| Compiler | Approx Throughput | Notes |
|----------|------------------|-------|
| Zig | ~1000 MiB/s | SoA + sentinel + hand-written; gold standard for lexer speed |
| Go | ~300 MiB/s | Hand-written with ASCII fast paths |
| Rust (rustc) | ~100 MiB/s | Two-layer architecture; string interning included |
| Ori V1 (logos) | ~234-281 MiB/s | DFA-generated; competitive with Go (varies by input size) |

Ori V1 is already competitive with Go. The V2 target of 400+ MiB/s aims to close the gap with Zig by adopting its key techniques (sentinel buffer, SWAR, SoA).

### Performance Targets

| Metric | V1 Baseline | V2 Target | Improvement | Rationale |
|--------|-------------|-----------|-------------|-----------|
| Lexer throughput (bytes/sec) | 234-281 MiB/s | 400+ MiB/s | >= 1.5x | SoA + SWAR + memchr combined gains |
| Lexer throughput (tokens/sec) | ~119 Mtokens/s | 180+ Mtokens/s | >= 1.5x | Fewer indirections in token construction |
| Parser throughput | 131-159 MiB/s | 144-175+ MiB/s | >= 1.1x | SoA cache efficiency for tag scanning |
| `ori check` wall time | Measured | No regression | -- | Lexing is a small fraction of total time |
| Peak memory (lexer) | Measured | <= 0.85x | ~15% reduction | SoA layout saves ~16% per token |

---

## 10.1 Baseline Measurement

Existing benchmark files to build on:
- `compiler/oric/benches/lexer.rs` -- raw throughput (bytes/s, tokens/s), Salsa-cached, scaling (10-5000 funcs), realistic (1KB/10KB/50KB)
- `compiler/oric/benches/parser.rs` -- raw parser throughput, parser-only (pre-lexed), incremental parsing, SyntaxCursor reuse-rate
- `compiler/oric/benches/memory.rs` -- tracking allocator measuring peak bytes, allocation count, and memory amplification for lexer and parser
- `compiler/oric/benches/type_check.rs` -- type checker throughput (annotated, inferred, scaling)
- `compiler/oric/benches/formatter.rs` -- formatter throughput including incremental and parallel

- [ ] Run existing benchmarks to establish V1 baseline:
  ```bash
  cargo bench -p oric --bench lexer -- --save-baseline v1-baseline
  ```
- [ ] Record baseline metrics (some already measured -- see tables above):
  - Bytes/sec for raw lexing (various file sizes: 1KB, 10KB, 50KB)
  - Tokens/sec for raw lexing
  - Bytes/sec for through-Salsa lexing
  - Peak memory for TokenList (various file sizes)
- [ ] Record per-file-size ratios:
  - Source bytes : token count (validate v2-conventions §9's 6:1 heuristic for Ori)
  - Source bytes : TokenList memory (bytes per token)
- [ ] Store baseline results in a reproducible format (JSON or similar)

---

## 10.2 Benchmark Suite

- [ ] Extend the existing benchmark suite (`compiler/oric/benches/lexer.rs`) with V2-specific benchmarks:

  **Component microbenchmarks:**
  - `bench_swar_whitespace` -- SWAR whitespace scanning vs byte-by-byte (various run lengths)
  - `bench_memchr_string` -- memchr string scanning vs byte-by-byte (various string lengths)
  - `bench_memchr_comment` -- memchr comment scanning vs byte-by-byte
  - `bench_keyword_lookup` -- keyword lookup (keywords, non-keywords, near-misses)
  - `bench_source_buffer_construction` -- SourceBuffer creation overhead
  - `bench_identifier_scanning` -- ASCII vs Unicode identifier scanning

  **End-to-end benchmarks:**
  - `bench_lex_small` -- 1KB Ori source
  - `bench_lex_medium` -- 10KB Ori source
  - `bench_lex_large` -- 50KB Ori source
  - `bench_lex_realistic` -- The actual Ori standard library files
  - `bench_lex_worst_case` -- Pathological input (all whitespace, all comments, deeply nested strings)
  - `bench_lex_with_comments` -- Full metadata extraction path (`lex_with_comments()` returning `LexOutput`)

  **Template literal benchmarks:**
  - `bench_lex_template_simple` -- Template with no interpolation: `` `hello world` ``
  - `bench_lex_template_shallow` -- Template with 1-2 interpolations: `` `hello {name}` ``
  - `bench_lex_template_deep` -- Template with 10+ interpolations
  - `bench_lex_template_nested` -- Nested templates: `` `outer {`inner {x}`}` ``
  - `bench_lex_template_long` -- Long template strings (1KB+ of template text)
  - `bench_lex_template_format_spec` -- Templates with format specifiers: `` `{x:>10.2f}` ``

  **Comparative benchmarks (V1 vs V2):**
  - `bench_lex_v1` -- V1 (logos) lexer throughput
  - `bench_lex_v2` -- V2 (hand-written) lexer throughput
  - Side-by-side comparison with identical inputs

- [ ] Use `criterion` (v0.8, already configured in `oric/Cargo.toml`) for statistically rigorous benchmarks (confidence intervals, outlier detection)
- [ ] Use `criterion_group!`/`criterion_main!` macros and `[[bench]] harness = false` for `cargo bench` discovery (matching existing benchmark setup)

---

## 10.3 Tier Checkpoints

Run benchmarks at each tier boundary to track progress:

- [ ] **After Tier 0 (Sections 01-03)**: Raw scanner + cooker vs logos
  - Expected: Roughly equivalent throughput (hand-written without optimizations ~= logos DFA)
  - Baseline: 234-281 MiB/s (V1, varies by input size)
  - Target: >= 230 MiB/s (no regression)
  - Validates: No performance regression from replacing logos
  - **Action**: If V2 is slower, profile and fix before proceeding

- [ ] **After Tier 1 (Sections 04-06)**: SoA + SWAR + keyword optimization
  - Expected: >= 1.3x improvement from SWAR and SoA
  - Baseline: 234-281 MiB/s (V1)
  - Target: >= 340 MiB/s
  - Validates: Fast paths and SoA layout provide measurable gains
  - **Action**: Profile cache behavior (`perf stat`, `cachegrind`) to confirm SoA benefits

- [ ] **After Tier 2 (Section 07)**: Diagnostics added
  - Expected: No regression (diagnostics are cold path only)
  - Baseline: Tier 1 result
  - Target: >= Tier 1 result (no regression)
  - Validates: `#[cold]` annotations keep error paths out of hot loops
  - **Action**: If regression, verify error paths are not inlined into hot loops

- [ ] **After Tier 3 (Sections 08-09)**: Full integration
  - Expected: >= 1.5x overall improvement
  - Baseline: 234-281 MiB/s (V1)
  - Target: >= 400 MiB/s
  - Validates: End-to-end pipeline benefits from all optimizations
  - **Action**: Final profiling and optimization pass

---

## 10.4 Profiling & Optimization

- [ ] **CPU profiling** with `perf` or `cargo flamegraph`:
  - Identify hot functions in the scanning loop
  - Verify SWAR and memchr are being used (not optimized away)
  - Check for unexpected branch mispredictions in the main dispatch
- [ ] **Cache profiling** with `perf stat` or `cachegrind`:
  - Measure L1 cache miss rate for tag scanning (SoA should show improvement)
  - Compare cache misses for V1 (24-byte tokens) vs V2 (1-byte tags)
- [ ] **Instruction count** with `valgrind --tool=callgrind`:
  - Measure instructions per token for V1 vs V2
  - Identify instruction bloat from complex state machines
- [ ] **Memory profiling** (extend existing `compiler/oric/benches/memory.rs` which already has tracking-allocator infrastructure):
  - Measure peak allocation for lexing various file sizes
  - Verify zero allocation in the raw scanner (no heap allocation in the inner loop)
  - Measure per-token memory in SoA layout vs AoS

---

## 10.5 Final Validation

- [ ] **Performance target met**: V2 throughput >= 1.5x V1 baseline (400+ MiB/s)
- [ ] **No regression in downstream**: `ori check` and `ori run` wall-clock times are unchanged or improved
- [ ] **Parser throughput maintained or improved**: Parser with V2 lexer >= 131-159 MiB/s baseline
- [ ] **Memory target met**: Per-token memory <= 85% of V1
- [ ] **Template literal overhead**: Template benchmarks show no pathological slowdown vs regular strings
- [ ] **All tests pass**: `./test-all.sh` with V2 as default
- [ ] **Benchmark results documented**: Record final numbers with hardware/OS details
- [ ] **Comparison table**:
  ```
  | Metric                | V1 (logos)      | V2 (hand-written) | Improvement |
  |-----------------------|-----------------|---------------------|-------------|
  | Throughput (bytes/s)  | 234-281 MiB/s   | XXX MiB/s          | X.Xx        |
  | Throughput (tok/s)    | ~119 Mtok/s     | XXX Mtok/s         | X.Xx        |
  | Memory (bytes/tok)    | ~25 B           | XX B               | X.Xx        |
  | `ori check` time      | XXX ms          | XXX ms             | X.Xx        |
  | Parser throughput     | 131-159 MiB/s   | XXX MiB/s          | X.Xx        |
  ```

---

## 10.6 Completion Checklist

- [ ] V1 baseline recorded (February 2026 numbers documented above)
- [ ] Benchmark suite extended with microbenchmarks, template literal benchmarks, and comparisons
- [ ] Tier checkpoint measurements completed (4 checkpoints with concrete targets)
- [ ] CPU, cache, and memory profiling completed
- [ ] Performance targets validated (>= 1.5x throughput, <= 85% memory)
- [ ] Template literal benchmarks show acceptable performance
- [ ] Final comparison table documented
- [ ] All tests pass with V2 as default

**Exit Criteria:** V2 lexer meets or exceeds the 1.5x throughput target (400+ MiB/s vs 234-281 MiB/s baseline). No regressions in downstream pipeline performance (parser >= 131-159 MiB/s baseline). Memory usage is reduced. Template literal tokenization performs well. Results are documented with reproducible benchmarks.

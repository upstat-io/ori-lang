---
plan: "parser-perf"
title: "Parser Frontend Performance & API: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "plans/roadmap/section-00-parser.md"
  - "docs/ori_lang/v2026/spec/grammar.ebnf"
  - "docs/ori_lang/v2026/spec/operator-rules.md"
---

# Parser Frontend Performance & API: Exhaustive Implementation Plan

## Mission

Maximize Ori's parser frontend throughput and API quality across the full pipeline — lexer, parser, and Salsa integration — by amplifying existing architectural strengths (parallel tag arrays, static binding power tables, Elm-style progress tracking) and closing identified gaps (file hygiene blockers, missing `#[inline]` annotations, inactive incremental parsing, Salsa query granularity). Target: measurable throughput improvement on existing benchmarks with zero regressions.

## Architecture

```
Source Text (&str)
    │
    ├── ori_lexer_core::RawScanner ─── (RawTag, len) pairs ───┐
    │     Raw byte scanning, DFA-based                         │
    │     ~720-1020 MiB/s                                      │
    │                                                          │
    ├── ori_lexer::TokenCooker ─── CookResult ────────────────┤
    │     Keyword resolution, escape processing                │
    │     IdentCache (256-entry), try_trivial() fast path      │
    │     Const-generic WITH_METADATA monomorphization         │
    │     ~208-240 MiB/s (cooked)                              │
    │                                                          │
    ├── ori_lexer::lex_driver() ─── LexOutput ────────────────┤
    │     Unified driver loop, flag finalization                │
    │     TokenList with parallel tags[u8] + flags[] arrays    │
    │                                                          │
    ├── [Salsa] lex_result() / tokens() ─── TokenList ────────┤
    │     Early cutoff on position-independent hash            │
    │     ~30-40% overhead vs raw                              │
    │                                                          │
    ├── ori_parse::Parser ─── ParseOutput ────────────────────┤
    │     Cursor<'a> (tags + flags parallel arrays)            │
    │     Pratt parser (OPER_TABLE[128] static lookup)         │
    │     ParseOutcome<T> (4-way Elm-style progress)           │
    │     TokenSet ([u128; 2] bitset recovery)                 │
    │     ExprArena allocation                                 │
    │     ~95-128 MiB/s                                        │
    │                                                          │
    └── [Salsa] parsed() ─── ParseOutput                      │
          Early cutoff on AST hash                             │
          Incremental infra exists (copier.rs) but inactive    │
```

### Performance Tiers (Current Baselines — 2026-02-08)

| Tier | Throughput | Where |
|------|-----------|-------|
| Raw scanner | ~720-1020 MiB/s | `ori_lexer_core` |
| Cooked lexer | ~208-240 MiB/s | `ori_lexer` (incl. cooking + interning) |
| Full parser | ~95-128 MiB/s | `ori_parse` (incl. lexing) |
| Salsa overhead | ~30-40% | `oric` query layer |

## Design Principles

### 1. Amplify, Don't Replace

Research confirms Ori's parser already implements or exceeds techniques found in Chumsky and reference compilers (Rust, Go, TypeScript, Zig). The parallel tag array design, static binding power tables, `[u128; 2]` bitset recovery, and Elm-style `ParseOutcome` are all superior patterns for hand-written recursive descent. This plan amplifies these strengths rather than introducing foreign abstractions.

**Evidence:** Chumsky's combinator model loads full 16-byte `TokenKind` for every check; Ori's `tags[u8]` array does it in 1 byte. Chumsky's `Mode` trait (Emit/Check) is combinator-specific and doesn't apply to hand-written parsers. Go is the only reference compiler with a validation-only mode, and its use case (import-only parsing) doesn't apply.

### 2. Measure Before Changing

Every optimization must be validated against the existing Criterion benchmark suite. The plan establishes baselines first (Section 01), then measures each change. This prevents regressions and avoids speculative optimization.

**Evidence:** Prior `#[inline]` work showed 20-30% gains on cross-crate hot functions but SWAR was counterproductive for <8-byte runs. Only profiling distinguishes productive from counterproductive changes.

### 3. Hygiene Before Performance

File hygiene violations (5 files over 500-line limit) must be resolved before performance work to avoid compounding complexity. `lib.rs` at 1,326 lines is the critical blocker — performance changes touching parser state will be harder to review and maintain without first splitting it.

## Section Dependency Graph

```
Section 01 (Baseline)
    │
    ├── Section 02 (Hygiene / File Splits) ─────────────────┐
    │                                                        │
    ├── Section 03 (Lexer Optimizations) ────────────────┐   │
    │                                                    │   │
    ├── Section 04 (Parser Optimizations) ───────────────┤   │
    │       depends on 02 (clean lib.rs split)           │   │
    │                                                    │   │
    ├── Section 05 (Salsa / Incremental) ────────────────┤   │
    │       depends on 01 (baselines for Salsa overhead) │   │
    │                                                    │   │
    └── Section 06 (Verification) ───────────────────────┘   │
            depends on all above                             │
```

- **Section 01** is independent — establishes baselines.
- **Section 02** is independent of 01 but must complete before 04 (parser optimizations touch `lib.rs`).
- **Sections 03, 04, 05** can be worked in any order after their dependencies.
- **Section 06** requires all prior sections.

**Cross-section interactions:**
- **Section 02 + Section 04**: `lib.rs` split (Section 02) must land before parser optimization (Section 04). Optimizing code that will be moved to new files creates merge conflicts and wastes effort.
- **Section 01 + Section 06**: Baselines (Section 01) are re-measured in Section 06 to quantify gains. Same benchmark commands, same input sizes.

## Implementation Sequence

```
Phase 0 - Baseline
  └─ 01: Establish performance baselines (raw + Salsa + incremental)

Phase 1 - Hygiene
  └─ 02: Split oversized files (lib.rs, copier.rs, kind.rs, cursor.rs, outcome.rs)

Phase 2 - Optimizations (parallelizable)
  ├─ 03: Lexer optimizations (#[inline], arena sizing, cooker fast paths)
  ├─ 04: Parser optimizations (#[inline], arena pre-alloc, snapshot improvement)
  └─ 05: Salsa integration (query granularity, incremental parsing activation)
  Gate: All existing benchmarks pass without regression

Phase 3 - Verification
  └─ 06: Re-measure baselines, benchmark new workloads, document findings
  Gate: Measurable improvement on raw throughput benchmarks
```

**Why this order:**
- Phase 0 establishes the measurement framework — all later phases are measured against it.
- Phase 1 is purely structural (no behavioral changes) — risk-free.
- Phase 2 is where performance gains happen. Sections 03/04/05 are independent and can be parallelized.
- Phase 3 proves the work was worthwhile.

**Known failing tests (expected until plan completion):**
- None expected. This plan modifies internal structure and performance, not behavior. All existing tests should pass throughout.

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_lexer` | ~3,120 | ~3,407 | ~6,527 |
| `ori_parse` | ~14,471 | ~10,230 | ~24,701 |
| `oric` (parser-related) | ~600 | ~200 | ~800 |
| **Total** | **~18,191** | **~13,837** | **~32,028** |

### Benchmark Baselines (2026-02-08)

| Benchmark | Throughput |
|-----------|-----------|
| `lexer_core/raw/throughput` | ~720-1020 MiB/s |
| `lexer/raw/throughput` | ~208-240 MiB/s |
| `parser/raw/throughput` | ~95-128 MiB/s |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Baseline | ~50 new (scripts) | Low | — |
| 02 Hygiene | ~0 net (splits) | Low | — |
|   ↳ 02.1 lib.rs split | ~0 | Low | — |
|   ↳ 02.2 copier.rs split | ~0 | Low | — |
|   ↳ 02.3 Other file splits | ~0 | Low | — |
| 03 Lexer Optimizations | ~100 modified | Medium | 01 |
|   ↳ 03.1 Inline audit | ~30 | Low | — |
|   ↳ 03.2 Arena sizing | ~20 | Low | — |
|   ↳ 03.3 Cooker fast paths | ~50 | Medium | — |
| 04 Parser Optimizations | ~150 modified | Medium | 01, 02 |
|   ↳ 04.1 Inline audit | ~40 | Low | — |
|   ↳ 04.2 Arena pre-alloc | ~30 | Low | — |
|   ↳ 04.3 Snapshot enhancement | ~30 | Low | — |
|   ↳ 04.4 Expression parsing | ~50 | Medium | — |
| 05 Salsa Integration | ~200 modified | High | 01 |
|   ↳ 05.1 Query overhead profiling | ~20 | Low | — |
|   ↳ 05.2 Incremental parsing activation | ~100 | High | — |
|   ↳ 05.3 Query granularity | ~80 | Medium | — |
| 06 Verification | ~100 new (benchmarks) | Medium | All |
| **Total modified** | **~600** | | |
| **Total deleted** | **~0** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| Compound assignment double evaluation | `ExprKind` copy duplicates target expression | Section 04 (note only — architectural, not perf) | Known, documented |
| `t.0.1` fails (lexer tokenizes `0.1` as Float) | Lexer merges `0.1` into single token | Out of scope (tracked in roadmap section-05) | Known |
| `lib.rs` at 1,326 lines | No split performed | Section 02.1 | Not Started |
| `copier.rs` at 1,595 lines (LEAK) | Manual AST variant copying | Section 02.2 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Performance Baselines | `section-01-baselines.md` | Not Started |
| 02 | File Hygiene | `section-02-hygiene.md` | Not Started |
| 03 | Lexer Optimizations | `section-03-lexer.md` | Not Started |
| 04 | Parser Optimizations | `section-04-parser.md` | Not Started |
| 05 | Salsa Integration | `section-05-salsa.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |

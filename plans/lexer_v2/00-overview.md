# Lexer V2: Best-of-Breed Lexer Architecture

> **Description**: Replace the logos-based lexer with a hand-written, two-crate lexer architecture synthesized from patterns across Rust, Go, Zig, Gleam, Elm, Roc, and TypeScript compilers. The result is a highly modular, cache-efficient, diagnostically rich lexer that integrates seamlessly with Ori's Salsa-based incremental compilation pipeline.
>
> **Conventions**: Follows `plans/v2-conventions.md` for shared V2 patterns (index types, tag enums, SoA accessors, flags, error shape, phase output, two-layer crate pattern).

## Reference Compiler Analysis

Patterns were analyzed from seven production compilers:

| Compiler | Key Insight Adopted |
|----------|-------------------|
| **Rust** (`rustc_lexer`) | Two-phase architecture: pure scanner + cooking layer; error flags on tokens; `memchr` acceleration |
| **Go** (`cmd/compile/internal/syntax`) | Sentinel-based ASCII fast path; perfect-hash keyword lookup; segment capture for zero-alloc text extraction |
| **Zig** (`std.zig.tokenizer`) | SoA token storage (MultiArrayList); sentinel-terminated buffer; omit end offsets; labeled-switch state machine; deferred literal validation |
| **Gleam** (`compiler-core/src/parse/lexer.rs`) | Proactive cross-language error detection; three name categories at lexer level; streaming iterator with pending queue |
| **Elm** (`compiler/src/Parse/`) | `whatIsNext` error context inspection; deep error type hierarchy mirroring parser structure; first-person diagnostic narratives |
| **Roc** (`crates/compiler/parse/src/`) | SWAR 8-byte-at-a-time whitespace/comment scanning; 64-byte-aligned source buffer with cache prefetching; `Progress`-based error commitment |
| **TypeScript** (`src/compiler/scanner.ts`) | Re-scanning mechanism for context-sensitive tokens; `TokenFlags` bitfield for rich metadata; template literal four-token strategy; speculation/rollback |

## Design Philosophy

### 1. Two-Layer Architecture

Following Rust's `rustc_lexer` / `rustc_parse::lexer` separation, the lexer is split into two crates (v2-conventions §10):

- **`ori_lexer_core`** -- Standalone, pure tokenizer with zero `ori_*` dependencies. Produces `(RawTag, len)` pairs. Suitable for external tools (LSP, formatter, syntax highlighter).
- **`ori_lexer`** -- Compiler integration layer. Maps `RawTag` to `ori_ir::TokenTag`, adds spans/interning/diagnostics, integrates with Salsa.

The boundary is the mapping. Core produces raw data; integration "cooks" it into compiler-ready form.

### 2. Separation of Concerns

Each layer has a single responsibility and zero knowledge of layers above it. The raw scanner knows nothing about string interning, spans, or error messages. The cooker knows nothing about Salsa or the parser. This enables independent testing, benchmarking, and evolution of each layer.

### 3. Preserve What Works

The existing `TokenList` already implements a SoA pattern with a parallel `tags: Vec<u8>` array that the parser cursor uses for O(1) dispatch. The V2 lexer feeds into this existing structure unchanged -- no `TokenList` restructuring is needed. The focus is on improving what happens *before* tokens reach `TokenList`: scanning, cooking, and acceleration.

### 4. Zero-Allocation Hot Path

The raw scanner allocates zero bytes on the heap. It operates on a sentinel-terminated buffer and produces `(tag, length)` pairs by value. All allocation (interning, escape processing, diagnostics) happens in the cooking layer, keeping the inner scanning loop as tight as possible.

### 5. Diagnostic Depth Without Performance Cost

Rich error diagnostics (context-aware messages, `whatIsNext` inspection, cross-language habit detection) are generated in the cooking layer and error recovery paths, never in the scanning hot path. Error paths are `#[cold]` annotated.

### 6. Seamless Integration

The lexer integrates with Ori's existing systems: Salsa queries for incremental caching, `StringInterner` for identifier/string interning, `Span` for source locations, and `ModuleExtra` for formatter metadata. Migration is incremental -- the parser API (`TokenList`, tags, cursor) remains stable.

### Cross-System Cohesion

Lexer V2 follows shared V2 conventions (`plans/v2-conventions.md`) for consistency with parser V2 and types V2:

- **Index types** (§1) -- `TokenIdx(u32)` follows the same `#[repr(transparent)]` / `NONE = u32::MAX` pattern as `ExprId`, `Idx`
- **Tag enums** (§2) -- `RawTag` (in `ori_lexer_core`) and `TokenTag` (in `ori_ir`) use `#[repr(u8)]` with semantic ranges
- **SoA accessors** (§3) -- `TokenStorage` exposes `.tag(idx)`, `.flags(idx)`, `.len()` matching `Pool`'s API
- **Flag types** (§4) -- `TokenFlags(u8)` uses `bitflags!` with semantic bit ranges: SPACE_BEFORE, NEWLINE_BEFORE, TRIVIA_BEFORE, ADJACENT, LINE_START, CONTEXTUAL_KW, HAS_ERROR, IS_DOC
- **Error shape** (§5) -- `LexError` follows WHERE + WHAT + WHY + HOW: `span`, `kind`, `context`, `suggestions`
- **Phase output** (§6) -- `LexOutput { tokens, errors, metadata }` is immutable after creation
- **Shared types** (§7) -- `TokenIdx`, `TokenTag` live in `ori_ir`; `RawTag` stays in `ori_lexer_core`
- **Two-layer pattern** (§10) -- `ori_lexer_core` (standalone) maps to `ori_lexer` (compiler integration)

## Section Overview

### Tier 0: Foundation

| Section | Focus |
|---------|-------|
| 01 | Architecture & Source Buffer -- two-layer crate design, sentinel-terminated cache-aligned input buffer |
| 02 | Raw Scanner -- hand-written state machine replacing logos, template literal scanning |
| 03 | Token Cooking & Interning -- conversion from raw tags to rich TokenKind |

### Tier 1: Performance

| Section | Focus |
|---------|-------|
| 04 | TokenList Compatibility & Tag Alignment -- feed V2 output into existing SoA TokenList; eliminate dual-enum |
| 05 | SWAR & Fast Paths -- 8-byte-at-a-time scanning, memchr, ASCII fast paths |
| 06 | Keyword Recognition -- compile-time perfect hash or length-bucketed lookup |

### Tier 2: Diagnostics

| Section | Focus |
|---------|-------|
| 07 | Diagnostics & Error Recovery -- context-aware messages, recovery strategies |

### Tier 3: Integration

| Section | Focus |
|---------|-------|
| 08 | Parser Integration & Migration -- cursor adaptation, API stability |
| 09 | Salsa & IDE Integration -- incremental caching, formatter metadata |

### Tier 4: Validation

| Section | Focus |
|---------|-------|
| 10 | Benchmarking & Performance Validation -- regression testing, throughput targets |

## Performance Baselines

Measured February 2026. These are the numbers to beat.

| Workload | Lexer | Parser | Combined |
|----------|-------|--------|----------|
| Small (~1KB) | 259 MiB/s | 120 MiB/s | -- |
| Medium (~10KB) | 281 MiB/s | 143 MiB/s | -- |
| Large (~50KB) | 292 MiB/s | 164 MiB/s | -- |

Token throughput: ~122 Mtokens/s. Benchmark: `cargo bench -p oric --bench lexer -- "lexer/raw" --noplot`

### Optimization Targets

| Metric | Current (Logos) | Target | Improvement |
|--------|-----------------|--------|-------------|
| Lexer raw throughput | ~259-292 MiB/s | 400+ MiB/s | ~40-50% faster |
| Keyword lookup | O(1) DFA | O(1) hash | Equivalent |
| Whitespace skip | Byte-by-byte | SWAR (8 bytes) | 3-5x faster |
| Comment skip | Byte-by-byte | memchr | 5-10x faster |
| Error quality | Basic `TokenKind::Error` | Elm-tier structured | Qualitative leap |

## Dependency Graph

```
Section 01 (Architecture & Source Buffer)
    |
    +---> Section 02 (Raw Scanner)
             |
             +---> Section 03 (Token Cooking)
             |        +---> Section 07 (Diagnostics)
             |        +---> Section 08 (Parser Integration)
             |                 +---> Section 09 (Salsa & IDE)
             +---> Section 04 (Tag Alignment)
             |        +---> Section 08 (Parser Integration)
             +---> Section 05 (SWAR & Fast Paths)
             +---> Section 06 (Keyword Recognition)
                      +---> Section 03 (Token Cooking)

Section 10 (Benchmarking) -- independent, runs at every tier boundary
```

**Independent sections** (can run in parallel after their deps):
- Sections 04, 05, 06 are independent of each other (all depend on 02)
- Section 07 depends only on 03
- Section 10 runs continuously

**Critical path**: 01 --> 02 --> 03 --> 08 --> 09

## Architecture Diagram

```
                                +-----------------------------------------+
                                |        ori_lexer_core (crate)           |
                                |  Standalone -- zero ori_* dependencies  |
                                |                                         |
  source: &str ----------------->  +----------------------------------+   |
                                |  |  SourceBuffer                    |   |
                                |  |  - Sentinel-terminated (&[u8;0]) |   |
                                |  |  - Cache-line aligned            |   |
                                |  |  - BOM detection                 |   |
                                |  +---------------+------------------+   |
                                |                  | Cursor { buf, pos }  |
                                |  +---------------v------------------+   |
                                |  |  RawScanner                      |   |
                                |  |  - Hand-written state machine    |   |
                                |  |  - Produces (RawTag, len) pairs  |   |
                                |  |  - Zero allocation               |   |
                                |  |  - SWAR whitespace scanning      |   |
                                |  |  - memchr string/comment scan    |   |
                                |  |  - ASCII sentinel fast path      |   |
                                |  |  - Template literal scanning     |   |
                                |  |    (TemplateHead/Middle/Tail)    |   |
                                |  +---------------+------------------+   |
                                |                  |                      |
                                +------------------+----------------------+
                                                   | Iterator<(RawTag, len)>
                                +-----------------------------------------+
                                |          ori_lexer (crate)              |
                                |  Depends on: ori_lexer_core + ori_ir   |
                                |                                         |
                                |  +----------------------------------+   |
                                |  |  TokenCooker                     |   |
                                |  |  - RawTag --> TokenTag mapping   |   |
                                |  |  - String interning (Name)       |   |
                                |  |  - Escape sequence processing    |   |
                                |  |  - Numeric validation            |   |
                                |  |  - Keyword perfect hash          |   |
                                |  |  - Context-sensitive keywords    |   |
                                |  |  - Span construction             |   |
                                |  |  - TokenFlags computation        |   |
                                |  |  - Error diagnostic generation   |   |
                                |  +---------------+------------------+   |
                                |                  |                      |
                                |  +---------------v------------------+   |
                                |  |  TokenList (existing SoA)         |   |
                                |  |  - tags:   Vec<u8>    (hot)      |   |
                                |  |  - tokens: Vec<Token> (cold)     |   |
                                |  |    Token = TokenKind + Span       |   |
                                |  +----------------------------------+   |
                                |                                         |
                                +-----------------+-----------------------+
                                                  |
                               +------------------v---------------------+
                               |  ori_parse: Parser Cursor (unchanged)   |
                               |  - tags: &[u8] for hot-path dispatch   |
                               |  - tokens: &TokenList for kind/span    |
                               +----------------------------------------+
```

## Comparison: Current vs. V2

| Aspect | Current (logos) | V2 (hand-written) |
|--------|----------------|-------------------|
| Crate structure | Single `ori_lexer` | Two-layer: `ori_lexer_core` + `ori_lexer` (§10) |
| Scanner | logos DFA (generated) | Hand-written state machine |
| Dependencies | `logos` crate | `memchr` only (or zero) |
| Allocation in scanner | Zero (logos is zero-copy) | Zero |
| Token storage | 25 bytes (24 Token + 1 tag) | 25 bytes per token (unchanged -- existing SoA preserved) |
| Keyword lookup | logos regex matching | Perfect hash (compile-time) |
| String scanning | logos regex | `memchr` + manual escape |
| Template literals | Not supported | TemplateHead/Middle/Tail/Complete tokens |
| Context-sensitive keywords | Not handled | ~20 soft keywords recognized in cooking layer |
| TokenFlags | Not present | `u8` bitfield: SPACE_BEFORE, NEWLINE_BEFORE, etc. (§4) |
| Whitespace scanning | logos `#[logos(skip)]` | SWAR 8-bytes-at-a-time |
| Error handling | `TokenKind::Error` (generic) | Structured `LexError` with WHERE+WHAT+WHY+HOW (§5) |
| RawToken->TokenKind | 183-line match (near 1:1) | `RawTag` -> `TokenTag` mapping at crate boundary (§10) |
| Literal validation | During scanning | Deferred to cooking layer |
| Comment handling | Mode-dependent | Unified with metadata extraction + detached doc warnings |
| Buffer type | `&str` | Sentinel-terminated `&[u8]` |
| Incremental support | Full re-lex (Salsa cutoff) | Full re-lex (Salsa cutoff) |
| Reusability | Compiler-only | `ori_lexer_core` usable by LSP, formatter, highlighter |

## Milestones

| Milestone | Tier | Sections | Exit Criteria |
|-----------|------|----------|---------------|
| **M0: Foundation** | 0 | 01, 02, 03 | Hand-written scanner produces identical tokens to logos for all test files; `ori_lexer_core` crate created with zero `ori_*` deps; template literal tokens emitted correctly |
| **M1: Performance** | 1 | 04, 05, 06 | Throughput >= 1.5x baseline; tag alignment verified; dual-enum eliminated |
| **M2: Diagnostics** | 2 | 07 | Context-aware errors for all lexer error classes; detached doc comment warnings |
| **M3: Integration** | 3 | 08, 09 | Full compiler pipeline works with new lexer; `./test-all.sh` passes |
| **M4: Validation** | 4 | 10 | Benchmark suite documents no regressions; logos removed |

## Success Criteria

1. **Implemented** -- All 10 sections complete; logos dependency removed from `ori_lexer`
2. **Two-layer** -- `ori_lexer_core` compiles with zero `ori_*` dependencies; usable standalone
3. **Performance** -- Lexer throughput >= 1.5x current logos-based lexer on benchmark suite (target: 400+ MiB/s)
4. **Template literals** -- Backtick-delimited template strings tokenized correctly with TemplateHead/Middle/Tail/Complete tokens and stack-based nesting
5. **Diagnostics** -- Error messages include context-aware suggestions for all common error classes
6. **Tested** -- 100% of existing lexer tests pass unchanged; new tests for every scanner state
7. **Integrated** -- Salsa queries, parser cursor, and formatter all work with new lexer
8. **Documented** -- Each module has `//!` module docs explaining its role in the architecture

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file -- plan overview |
| `index.md` | Keyword index for quick finding |
| `section-01-architecture.md` | Two-layer crate design + source buffer |
| `section-02-raw-scanner.md` | Hand-written state machine + template literals |
| `section-03-token-cooking.md` | Raw-to-rich token conversion |
| `section-04-soa-storage.md` | Token Representation & Tag Alignment |
| `section-05-swar-fast-paths.md` | SWAR and memchr acceleration |
| `section-06-keyword-recognition.md` | Perfect hash keyword lookup |
| `section-07-diagnostics.md` | Error messages and recovery |
| `section-08-parser-integration.md` | Parser cursor and migration |
| `section-09-salsa-ide.md` | Salsa caching and IDE support |
| `section-10-benchmarking.md` | Performance validation |
| `../v2-conventions.md` | Cross-system V2 conventions |

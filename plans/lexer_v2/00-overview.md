# Lexer 2.0 Implementation Plan

> **ROADMAP**: Extends `plans/parser_v2/section-02-lexer.md`
> **Best-of-Breed Lexer Architecture** — Combining innovations from Rust, Go, Zig, TypeScript, Gleam, Elm, and Roc

## Design Philosophy

Based on deep analysis of 7 production-grade lexers (~30,000+ lines of lexer code), this plan synthesizes the best patterns into a novel architecture for Ori's lexer:

1. **Two-Layer Architecture** — Rust-style separation of pure tokenizer and compiler integration
2. **Compact Token Representation** — Zig-style 8-byte tokens with lazy end computation
3. **Structure-of-Arrays Storage** — Zig's MultiArrayList for cache-optimal traversal
4. **Hand-Written State Machine** — Zig-style labeled switch for full control
5. **Whitespace-Sensitive Tokens** — Roc-style flags for space-aware parsing
6. **Gold-Standard Errors** — Elm/Gleam-style empathetic, educational messages
7. **SIMD Optimizations** — Roc-style fast paths for whitespace and comments

### Cross-System Cohesion

Lexer V2 follows shared V2 conventions (`plans/v2-conventions.md`) for consistency with parser V2 and types V2:

- **Index types** — `TokenIdx(u32)` follows the same `#[repr(transparent)]` / `NONE = u32::MAX` pattern as `ExprId`, `Idx`
- **Tag enums** — `RawTag` (in `ori_lexer_core`) and `TokenTag` (in `ori_ir`) use `#[repr(u8)]` with semantic ranges
- **SoA accessors** — `TokenStorage` exposes `.tag(idx)`, `.flags(idx)`, `.len()` matching `Pool`'s API
- **Flag types** — `TokenFlags` uses `bitflags!` with semantic bit ranges (u8 width, vs TypeFlags' u32)
- **Error shape** — `LexError` follows WHERE + WHAT + WHY + HOW: `span`, `kind`, `context`, `suggestions`

**Two-layer crate pattern:** `ori_lexer_core` is standalone with `RawTag` (no `ori_*` deps). `ori_lexer` maps `RawTag` → `ori_ir::TokenTag` at the integration boundary — the same pattern as Rust's `rustc_lexer` → `rustc_parse::lexer`.

The goal is to create a lexer that is:
- **50% smaller tokens** (8 bytes vs 24 bytes)
- **2-3x faster keyword lookup** via perfect hash
- **Reusable across tools** (LSP, formatter, syntax highlighter)
- **IDE-ready** (incremental-friendly, comment-preserving)
- **Industry-leading error messages** (Elm quality)

---

## Section Overview

### Section 1: Two-Layer Architecture

Separate pure tokenization from compiler integration for reusability.

| Subsection | Focus | Source |
|------------|-------|--------|
| 1.1 | Low-level tokenizer crate | Rust (`rustc_lexer`) |
| 1.2 | High-level processor | Rust (`rustc_parse::lexer`) |
| 1.3 | Crate boundary design | Rust |
| 1.4 | API stability guarantees | Rust |

### Section 2: Compact Token Representation

Minimize token size while maximizing information density.

| Subsection | Focus | Source |
|------------|-------|--------|
| 2.1 | 8-byte raw tokens | Zig |
| 2.2 | No `end` offset storage | Zig |
| 2.3 | Structure-of-Arrays storage | Zig (`MultiArrayList`) |
| 2.4 | TokenFlags bitfield | TypeScript, Roc |
| 2.5 | Lazy line/column computation | Zig, Roc |

### Section 3: State Machine Design

Hand-written state machine for full control over tokenization.

| Subsection | Focus | Source |
|------------|-------|--------|
| 3.1 | Labeled switch pattern | Zig |
| 3.2 | Sentinel-terminated buffers | Go, Zig |
| 3.3 | State enum design | Zig |
| 3.4 | Logos migration path | Ori (existing) |

### Section 4: Keyword & Operator Handling

Optimal keyword recognition and operator metadata.

| Subsection | Focus | Source |
|------------|-------|--------|
| 4.1 | Perfect hash keywords | Go |
| 4.2 | Compile-time collision detection | Go |
| 4.3 | Operator precedence table | Go, Rust |
| 4.4 | Context-sensitive keywords | Roc |
| 4.5 | Token gluing/breaking | Rust |

### Section 5: Unicode & Escape Handling

Comprehensive Unicode identifier and escape sequence support.

| Subsection | Focus | Source |
|------------|-------|--------|
| 5.1 | Unicode identifiers (XID) | Rust |
| 5.2 | Extended escape sequences | TypeScript |
| 5.3 | String interpolation | Roc, TypeScript |
| 5.4 | Raw strings | Rust |

### Section 6: Error Handling

Rich, educational lexical error messages.

| Subsection | Focus | Source |
|------------|-------|--------|
| 6.1 | Structured error types | Gleam |
| 6.2 | Empathetic messages | Elm, Gleam |
| 6.3 | Common mistake detection | Gleam |
| 6.4 | Error recovery strategies | TypeScript |

### Section 7: Performance Optimizations

SIMD and memory optimizations for maximum speed.

| Subsection | Focus | Source |
|------------|-------|--------|
| 7.1 | SIMD whitespace skipping | Roc |
| 7.2 | memchr for delimiters | Rust |
| 7.3 | Branchless character checks | Go |
| 7.4 | Buffer management | Go |

### Section 8: Parser Integration

Seamless integration with Parser V2.

| Subsection | Focus | Source |
|------------|-------|--------|
| 8.1 | Trivia preservation | Gleam |
| 8.2 | Comment classification | Gleam, Roc |
| 8.3 | Incremental lexing support | TypeScript |
| 8.4 | Whitespace-sensitive parsing | Roc |

---

## Performance Targets

### Current Measured Throughput (2026-02-06)

| Workload | Lexer | Parser | Combined |
|----------|-------|--------|----------|
| Small (~1KB) | 259 MiB/s | 120 MiB/s | — |
| Medium (~10KB) | 281 MiB/s | 143 MiB/s | — |
| Large (~50KB) | 292 MiB/s | 164 MiB/s | — |

Token throughput: ~122 Mtokens/s. Benchmark: `cargo bench -p oric --bench lexer -- "lexer/raw" --noplot`

### Optimization Targets

| Metric | Current (Logos) | Target | Improvement |
|--------|-----------------|--------|-------------|
| Lexer raw throughput | ~232-292 MiB/s | 400+ MiB/s | ~40-70% faster |
| Token size | 24 bytes | 8 bytes | 67% reduction |
| Token storage | Vec&lt;Token&gt; + Vec&lt;u8&gt; tags | Full SoA MultiArrayList | Complete SoA (tags already split) |
| Keyword lookup | O(1) DFA | O(1) hash | Equivalent |
| Whitespace skip | Byte-by-byte | SIMD (8 bytes) | 3-5x faster |
| Comment skip | Byte-by-byte | memchr | 5-10x faster |
| Line/column | Computed always | Lazy on error | ~0 cost normally |
| Error quality | Basic | Elm-tier | Qualitative |

---

## Dependency Graph

```
Section 1 (Architecture) ─┬─► Section 3 (State Machine)
                          │
                          └─► Section 2 (Tokens) ─┬─► Section 8 (Integration)
                                                   │
Section 4 (Keywords) ─────────────────────────────┤
                                                   │
Section 5 (Unicode) ──────────────────────────────┤
                                                   │
Section 6 (Errors) ───────────────────────────────┤
                                                   │
Section 7 (Performance) ──────────────────────────┘
```

**Key Dependencies**:
- Section 1 (Architecture) defines the crate structure for everything else
- Section 2 (Tokens) must be designed before Section 3 (State Machine)
- Section 4-7 can proceed in parallel after Sections 1-2
- Section 8 (Integration) comes last, ties everything together

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-4)
**Risk: Low | Impact: High**

| Task | Section | Status |
|------|---------|--------|
| Design two-layer architecture | 1.1-1.4 | Not started |
| Define compact token types | 2.1-2.2 | Not started |
| Implement TokenStorage (SoA) | 2.3-2.4 | Not started |
| Create ori_lexer_core crate | 1.1 | Not started |

### Phase 2: State Machine (Weeks 5-8)
**Risk: Medium | Impact: High**

| Task | Section | Status |
|------|---------|--------|
| Implement state machine core | 3.1-3.3 | Not started |
| Add sentinel buffer support | 3.2 | Not started |
| Port basic tokens from Logos | 3.4 | Not started |
| Implement perfect hash keywords | 4.1-4.2 | Not started |

### Phase 3: Rich Features (Weeks 9-12)
**Risk: Medium | Impact: Medium**

| Task | Section | Status |
|------|---------|--------|
| Add Unicode identifier support | 5.1 | Not started |
| Implement extended escapes | 5.2 | Not started |
| Add string interpolation | 5.3 | Not started |
| Implement raw strings | 5.4 | Not started |

### Phase 4: Errors & Performance (Weeks 13-16)
**Risk: Low | Impact: High**

| Task | Section | Status |
|------|---------|--------|
| Implement structured errors | 6.1-6.2 | Not started |
| Add common mistake detection | 6.3 | Not started |
| Implement SIMD optimizations | 7.1-7.2 | Not started |
| Add branchless optimizations | 7.3-7.4 | Not started |

### Phase 5: Integration (Weeks 17-20)
**Risk: Low | Impact: High**

| Task | Section | Status |
|------|---------|--------|
| Integrate with Parser V2 | 8.1-8.4 | Not started |
| Add trivia preservation | 8.1-8.2 | Not started |
| Implement incremental support | 8.3 | Not started |
| Performance tuning | All | Not started |

---

## Reference Implementations Analyzed

| Language | Lexer Location | Lines | Key Innovation |
|----------|----------------|-------|----------------|
| Rust | `rustc_lexer/`, `rustc_parse/src/lexer/` | ~3K | Two-layer, memchr, token gluing |
| Go | `go/scanner/`, `cmd/compile/internal/syntax/` | ~2K | Perfect hash, sentinel, semicolon |
| Zig | `lib/std/zig/tokenizer.zig` | ~1.5K | State machine, SoA, no end offset |
| TypeScript | `src/compiler/scanner.ts` | ~4K | Closure state, rescan, incremental |
| Gleam | `compiler-core/src/parse/lexer.rs` | ~1K | EcoString, pending queue, errors |
| Elm | `compiler/src/Parse/Primitives.hs` | ~2K | CPS, deferred errors, indentation |
| Roc | `crates/compiler/parse/`, `src/parse/` | ~3K | SIMD, whitespace-sensitive tokens |

---

## Key Patterns Adopted

| Pattern | Source | Section |
|---------|--------|---------|
| Two-layer architecture | Rust | 1.x |
| 8-byte compact tokens | Zig | 2.1 |
| No end offset storage | Zig | 2.2 |
| MultiArrayList (SoA) | Zig | 2.3 |
| TokenFlags bitfield | TypeScript, Roc | 2.4 |
| Lazy line/column | Zig, Roc | 2.5 |
| Labeled switch state machine | Zig | 3.1 |
| Sentinel-terminated buffers | Go, Zig | 3.2 |
| Perfect hash keywords | Go | 4.1 |
| Operator precedence table | Go, Rust | 4.3 |
| Token gluing/breaking | Rust | 4.5 |
| XID Unicode identifiers | Rust | 5.1 |
| String interpolation stack | Roc, TypeScript | 5.3 |
| Structured error types | Gleam | 6.1 |
| Empathetic messages | Elm, Gleam | 6.2 |
| SIMD whitespace skip | Roc | 7.1 |
| memchr for comments | Rust | 7.2 |
| Trivia preservation | Gleam | 8.1 |
| Whitespace-sensitive tokens | Roc | 8.4 |

---

## Relationship to Parser V2

This plan **extends** the Parser V2 plan (`plans/parser_v2/section-02-lexer.md`):

| Parser V2 Section 02 | Lexer V2 Section | Relationship |
|----------------------|------------------|--------------|
| 02.1-02.2 Perfect hash | 4.1-4.2 | Superseded (more detail) |
| 02.3 Precedence metadata | 4.3 | Parser owns via `OPER_TABLE` — lexer provides tags only |
| 02.4 Adjacent tokens | 4.5 | Superseded (token gluing) |
| 02.9 Token SoA tags | 2.3 | **Foundation** — existing `tags: Vec<u8>` evolves into full SoA |
| N/A | 1.x Architecture | New |
| N/A | 2.x Tokens | Builds on existing partial SoA |
| N/A | 3.x State Machine | New |
| N/A | 5.x Unicode | New |
| N/A | 6.x Errors | New |
| N/A | 7.x Performance | Adopts proven parser patterns |
| N/A | 8.x Integration | Must preserve existing parser contracts |

### Parser-Side Infrastructure Already Built

The parser hot path optimization work (2026-02-06) created a significant tag-based dispatch layer that the lexer V2 must integrate with. These components are already working and delivering +12-16% throughput:

- **`tags: Vec<u8>`** in `TokenList` — the seed of full SoA
- **`TAG_*` constants** (116 named `u8` values on `TokenKind`) — the naming convention for tag values
- **`OPER_TABLE[128]`** — static Pratt parser lookup table indexed by tag
- **`POSTFIX_BITSET`** — two-u64 bitset for O(1) postfix token membership
- **Direct dispatch in `parse_primary()`** — tag match before `one_of!` macro
- **Branchless `advance()`** — relies on EOF sentinel token

The lexer V2's `TokenStorage` is the natural completion of this partial SoA: replace `Vec<Token>` (24 bytes/token AoS) with `Vec<u32>` starts + `Vec<TokenValue>` + `Vec<TokenFlags>`, keeping the existing `Vec<u8>` tags as-is.

**Recommendation**: Once Lexer V2 is complete, mark Parser V2 Section 02 as "Superseded by Lexer V2".

---

## What Gets Replaced

**This is a full replacement, not a migration.** The current Logos-based lexer will be entirely deleted:

| Current | Replaced By | Reason |
|---------|-------------|--------|
| Logos DFA | Hand-written state machine | Full control, better errors |
| `TokenKind` (16 bytes) | `RawTag`/`TokenTag` (1 byte) + `TokenValue` | Compact, cache-friendly |
| `Token` (24 bytes) | `TokenStorage` (SoA) | 67% memory reduction |
| `TokenList` | `TokenStorage` | Structure-of-arrays |
| `RawToken` (Logos) | `RawToken` (hand-written) | No external dependency |
| Basic `Error` token | Rich `LexError` enum | Elm-quality messages |

**Already partially done (2026-02-06 hot path work):**
- `TokenList` already has a `tags: Vec<u8>` field — parallel discriminant tag array
- `TokenKind::discriminant_index()` → `TAG_*` constants (116 named `u8` constants)
- Parser's `Cursor` already has `current_tag() -> u8` and `check_tag()` methods
- `OPER_TABLE[128]` static lookup table for Pratt parser binding powers
- `POSTFIX_BITSET` (two `u64`s) for O(1) postfix token membership
- Direct tag dispatch in `parse_primary()` before `one_of!` macro
- Branchless `advance()` relying on EOF sentinel
- `#[cold]` split on `expect()` error paths

These parser-side changes mean `TokenStorage` design should build on the existing `tags: Vec<u8>` infrastructure rather than starting from scratch. The `TAG_*` constant naming convention is established and should be preserved in the new `TokenTag`/`RawTag` design.

**Preserved and enhanced:**
- String interning (sharded interner)
- TokenSet for recovery (128-bit bitset)
- Comment preservation (enhanced with classification)

**No backwards compatibility layer.** Parser and all consumers must update to new API.

---

## Relationship with Types V2

**Lexer V2 is independent of Types V2.** The two systems are cleanly decoupled:

| Aspect | Lexer V2 | Types V2 |
|--------|----------|----------|
| **Phase** | Source → Tokens → AST | AST → Typed AST |
| **Crates** | `ori_lexer`, `ori_lexer_core`, `ori_parse` | `ori_types`, `ori_typeck` |
| **Boundary** | Produces AST | Consumes AST |

### Shared Conventions

Both lexer V2 and types V2 follow the patterns in `plans/v2-conventions.md`: index types, tag enums, SoA accessors, flag types, error shapes, and phase output shapes. This is a shared design language, not a shared dependency.

### No Type System Changes Required

Lexer V2 does **not** modify:
- `ori_types` — unchanged
- `ori_typeck` — unchanged
- `ori_eval` — unchanged

### Parallel Development Safe

Both plans can proceed simultaneously. The AST (`ori_ir`) is the stable interface.

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file - plan overview |
| `index.md` | Keyword index for quick finding |
| `section-01-architecture.md` | Two-layer crate design |
| `section-02-tokens.md` | Compact token representation |
| `section-03-state-machine.md` | Hand-written tokenizer |
| `section-04-keywords.md` | Perfect hash and operators |
| `section-05-unicode.md` | Unicode and escapes |
| `section-06-errors.md` | Rich error messages |
| `section-07-performance.md` | SIMD and optimizations |
| `section-08-integration.md` | Parser V2 integration |
| `../v2-conventions.md` | Cross-system V2 conventions |

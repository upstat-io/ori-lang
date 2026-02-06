# Lexer V2 Index

> **This is a FULL REPLACEMENT of the current Logos-based lexer.**
> No backwards compatibility. Old lexer is deleted entirely.

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Two-Layer Architecture
**File:** `section-01-architecture.md` | **Status:** Not Started

```
two-layer, architecture, crate, separation, rustc_lexer
low-level tokenizer, high-level processor, compiler integration
reusable, LSP, formatter, syntax highlighter
stable API, pure function, no dependencies
ori_lexer_core, ori_lexer
```

---

### Section 02: Compact Token Representation
**File:** `section-02-tokens.md` | **Status:** Not Started (partial SoA already exists — see notes)

```
token, compact, 8-byte, memory, size
RawToken, RawTag, TokenTag, TokenIdx, TokenStorage, TokenList
SoA, Structure-of-Arrays, MultiArrayList
no end offset, lazy computation, derive end
TokenFlags, bitfield, space before, newline before
lazy line/column, LineIndex, binary search
span, position, byte offset
tags Vec<u8>, discriminant_index, TAG_* constants
partial SoA, current_tag, check_tag
```

---

### Section 03: State Machine Design
**File:** `section-03-state-machine.md` | **Status:** Not Started

```
state machine, hand-written, labeled switch
Logos migration, DFA, regex, patterns
sentinel, null terminator, bounds check
State enum, transitions, continue
control flow, jump table, optimization
single pass, streaming, iterator
```

---

### Section 04: Keyword & Operator Handling
**File:** `section-04-keywords.md` | **Status:** Not Started (OPER_TABLE already in parser — see notes)

```
keyword, perfect hash, O(1), lookup
collision, compile-time, validation
operator, precedence, associativity
OPERATOR_INFO, OPER_TABLE, table, metadata
context-sensitive, timeout, parallel, cache
token gluing, breaking, compound operators
>> shift right, >= greater equal, generics
phase separation, parser owns precedence
```

---

### Section 05: Unicode & Escape Handling
**File:** `section-05-unicode.md` | **Status:** Not Started

```
unicode, UTF-8, identifier, XID
XID_Start, XID_Continue, unicode-ident
escape sequence, \n, \r, \t, \\
hex escape, \xHH, \u{XXXX}
string interpolation, ${}, template
raw string, r"...", no escapes
char literal, single quote
```

---

### Section 06: Error Handling
**File:** `section-06-errors.md` | **Status:** Not Started

```
error, diagnostic, message, lexical
LexError, structured, rich, detailed
empathetic, Elm, Gleam, friendly
unterminated string, invalid escape
common mistake, semicolon, single quote
error recovery, continue, resilient
```

---

### Section 07: Performance Optimizations
**File:** `section-07-performance.md` | **Status:** Not Started (some parser-side optimizations already done)

```
performance, optimization, SIMD, fast
whitespace, skip, 8 bytes, chunk
memchr, comment, delimiter, search
branchless, character check, ASCII
buffer, sentinel, bounds check, EOF sentinel
cache, memory, allocation
#[cold] split, inline(never), error paths
branchless advance, debug_assert bounds check
static lookup table, bitset membership
```

---

### Section 08: Parser Integration
**File:** `section-08-integration.md` | **Status:** Not Started (existing parser infrastructure documented)

```
parser, integration, Parser V2, cursor
trivia, comment, whitespace, preserve
ModuleExtra, doc comment, classification
incremental, relex, range, edit
whitespace-sensitive, space before
adjacent, compound, context
current_tag, check_tag, POSTFIX_BITSET, OPER_TABLE
direct dispatch, parse_primary fast path
tag-based cursor, Vec<u8> tags
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Two-Layer Architecture | `section-01-architecture.md` |
| 02 | Compact Token Representation | `section-02-tokens.md` |
| 03 | State Machine Design | `section-03-state-machine.md` |
| 04 | Keyword & Operator Handling | `section-04-keywords.md` |
| 05 | Unicode & Escape Handling | `section-05-unicode.md` |
| 06 | Error Handling | `section-06-errors.md` |
| 07 | Performance Optimizations | `section-07-performance.md` |
| 08 | Parser Integration | `section-08-integration.md` |

---

## Cross-References

| Topic | Lexer V2 Section | Parser V2 Section |
|-------|------------------|-------------------|
| Keywords | 04.1-04.2 | 02.1-02.2 |
| Precedence | 04.3 | 02.3 |
| Adjacent tokens | 04.5 | 02.4 |
| Error messages | 06.x | 04.x |
| Incremental | 08.3 | 05.x |
| Trivia/Metadata | 08.1-08.2 | 06.x |

---

## Related Plans

| Plan | Relationship |
|------|--------------|
| `plans/parser_v2/` | Lexer V2 feeds into Parser V2 (tight integration) |
| `plans/types_v2/` | **Independent** — operates on AST, not tokens |
| `plans/roadmap/` | Overall language roadmap |
| `plans/ori_lsp/` | Uses tokens for syntax highlighting |
| `plans/v2-conventions.md` | **Cross-system conventions** — shared patterns for all V2 systems |

### Cross-System Cohesion

Lexer V2 and Types V2 are **independent in dependency** but share design conventions:
- Different compiler phases (lexing vs type checking)
- Different crates (`ori_lexer` vs `ori_types`/`ori_typeck`)
- Communicate only via `ori_ir` shared types (`Span`, `Name`, `TokenTag`, `ModuleExtra`)
- Can be developed in parallel

Both systems follow the same structural patterns — index types, tag enums, SoA accessors, flag types, error shapes — codified in `plans/v2-conventions.md`. This consistency means knowledge transfers between systems: understanding how `Pool.tag(idx)` works in types V2 immediately tells you how `TokenStorage.tag(idx)` works in lexer V2.

---

## Performance Validation

### Quick Check

Use the `/benchmark` skill for quick validation:

```bash
/benchmark short   # ~30s, sanity check
/benchmark medium  # ~2min, standard validation
/benchmark long    # ~5min, release validation
```

### Baseline (February 2026)

| Metric | Current | Target | Industry Reference |
|--------|---------|--------|-------------------|
| **Lexer throughput** | ~232-292 MiB/s | 400 MiB/s | Zig ~1000, Go ~300, Rust ~100 |
| **Parser throughput** | ~120-164 MiB/s | 200 MiB/s | Go ~100-150, Rust ~50-100 |

#### Lexer Raw Throughput (2026-02-06)

| Workload | Throughput | Input Size |
|----------|-----------|------------|
| 10 funcs | 234 MiB/s | ~295 B |
| 50 funcs | 255 MiB/s | ~1.5 KB |
| 100 funcs | 264 MiB/s | ~3 KB |
| 500 funcs | 290 MiB/s | ~16 KB |
| 1000 funcs | 286 MiB/s | ~33 KB |
| 5000 funcs | 288 MiB/s | ~174 KB |

**Realistic workloads:** small (~1KB) 259 MiB/s, medium (~10KB) 281 MiB/s, large (~50KB) 292 MiB/s.
**Token throughput:** ~122 Mtokens/s (500 functions).

**Benchmark command:** `cargo bench -p oric --bench lexer -- "lexer/raw" --noplot`

> **Note (2026-02-06):** Parser throughput was boosted from ~109-144 MiB/s to ~120-164 MiB/s
> via 7 hot path micro-optimizations (see `plans/parser_v2/section-01-data-oriented-ast.md` §01.7).
> Key changes included adding a partial SoA layer (`tags: Vec<u8>`) to `TokenList`, a static
> `OPER_TABLE[128]` for the Pratt parser, `POSTFIX_BITSET` for O(1) set membership, and direct
> tag dispatch in `parse_primary()`. These existing parser-side optimizations inform lexer V2
> design — see individual sections for "Already Done" notes.

### When to Benchmark

Run `/benchmark short` after modifying:
- Token representation (Section 02)
- State machine (Section 03)
- Keyword/operator handling (Section 04)
- Performance optimizations (Section 07)

**Skip benchmarks** for: error messages (06), unicode handling (05), integration glue (08).

### Manual Comparison

```bash
# Compare against saved baseline
cargo bench --bench lexer -p oric -- --baseline before_v2

# Raw benchmarks (fair comparison to other compilers)
cargo bench --bench lexer -p oric -- "lexer/raw"
```

### Benchmark Categories

| Category | What It Measures | Use For |
|----------|------------------|---------|
| `lexer/raw/*` | Pure lexer, no Salsa | Comparing to Zig/Go/Rust |
| `lexer/scaling/*` | Lexer via Salsa queries | Real-world usage |
| `parser/raw/*` | Lexer + parser, no Salsa | Full frontend comparison |

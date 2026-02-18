# Lexer V3: SIMD-Accelerated Compact Token Stream

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Compact Token Stream (SoA)
**File:** `section-01-compact-token-stream.md` | **Status:** Done

```
compact tokens, SoA, structure of arrays, parallel arrays
token size, cache density, memory layout, 6 bytes per token
RawTokenEntry, tag array, offset array, flags array
TokenList replacement, cache line utilization, L1 cache
```

---

### Section 02: Lazy Cooking
**File:** `section-02-lazy-cooking.md` | **Status:** In Progress

```
lazy cooking, deferred interning, on-demand, pull-based
TokenCooker, keyword resolution, string interning, numeric parsing
cook-on-access, parser-driven materialization
Salsa early cutoff, unused token work, error recovery skip
```

---

### Section 03: SIMD Byte Classification
**File:** `section-03-simd-classification.md` | **Status:** Not Started

```
SIMD, AVX2, NEON, vpshufb, vqtbl1q_u8, byte classification
nibble lookup, structural index, simdjson-inspired
32 bytes per cycle, bitmask extraction, movemask, tzcnt
token boundary detection, category bitmap
```

---

### Section 04: SIMD Token Boundary Extraction
**File:** `section-04-simd-boundary-extraction.md` | **Status:** Not Started

```
token boundary, bitmask iteration, tzcnt, popcnt
run-length classification, identifier runs, whitespace runs
operator extraction, delimiter detection
string/template/comment special handling
```

---

### Section 05: Parser Adaptation
**File:** `section-05-parser-adaptation.md` | **Status:** In Progress

```
parser interface, CompactTokenList, tag-based dispatch
lazy kind access, cook on demand, parser hot loop
backward compatibility, TokenCursor, incremental migration
```

---

### Section 06: Integration & Benchmarks
**File:** `section-06-integration-benchmarks.md` | **Status:** Not Started

```
benchmark, throughput, GB/s, tokens/second
cache miss reduction, perf stat, callgrind
Salsa integration, lex_result query, end-to-end
regression testing, spec test validation
```

---

## Quick Reference

| ID | Title | File | Est. Speedup |
|----|-------|------|-------------|
| 01 | Compact Token Stream (SoA) | `section-01-compact-token-stream.md` | 2-3x cache density |
| 02 | Lazy Cooking | `section-02-lazy-cooking.md` | 1.5-2x avoided work |
| 03 | SIMD Byte Classification | `section-03-simd-classification.md` | 4-8x scanning |
| 04 | SIMD Token Boundary Extraction | `section-04-simd-boundary-extraction.md` | Stacks on 03 |
| 05 | Parser Adaptation | `section-05-parser-adaptation.md` | Enables 01+02 |
| 06 | Integration & Benchmarks | `section-06-integration-benchmarks.md` | Validation |

## Dependency Order

```
01 (Compact Token Stream) ──┐
                            ├──> 05 (Parser Adaptation) ──> 06 (Integration)
02 (Lazy Cooking) ──────────┘
                                        ↑
03 (SIMD Classification) ──> 04 (SIMD Boundary) ──────────┘
```

Sections 01+02 and 03+04 can be developed in parallel. Section 05 merges both tracks. Section 06 validates the combined result.

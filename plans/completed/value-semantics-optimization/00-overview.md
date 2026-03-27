---
plan: "value-semantics-optimization"
title: "Value Semantics Optimization: Exhaustive Implementation Plan"
status: complete
supersedes: []
references:
  - "plans/aot_codegen_pipeline/"
  - "plans/roadmap/section-07C-collections.md"
  - "docs/ori_lang/v2026/spec/"
  - "Counting Immutable Beans (Ullrich & de Moura, 2020)"
  - "Perceus: Garbage Free Reference Counting with Reuse (Reinking et al., 2021)"
  - "FBIP: Functional But In-Place (Lorenzen et al., 2023)"
---

# Value Semantics Optimization: Exhaustive Implementation Plan

## Mission

Make Ori's value semantics **as fast as reference-based mutation in any language** — period. Every collection operation (push, pop, insert, remove, concat, slice, sort) must achieve O(1) amortized or better when the value is uniquely owned, with zero-copy views for read-only access and compile-time elimination of runtime checks when ownership is provable. This plan covers the entire system: runtime primitives, LLVM codegen, interpreter parity, static analysis, and exhaustive verification across correctness, memory safety, and performance.

## Architecture

```
                          ┌─────────────────────────────────────────┐
                          │            Ori Source Code               │
                          │  let items = items.push(42)             │
                          └────────────────┬────────────────────────┘
                                           │
                          ┌────────────────▼────────────────────────┐
                          │         Type Checker (ori_types)         │
                          │  • Uniqueness annotations (§07)         │
                          │  • Slice type tracking (§05)            │
                          └────────────────┬────────────────────────┘
                                           │
                          ┌────────────────▼────────────────────────┐
                          │          ARC Pipeline (ori_arc)          │
                          │  • Borrow inference (existing)          │
                          │  • Static uniqueness analysis (§07)     │
                          │  • RC insertion (existing, Perceus)     │
                          │  • Reset/Reuse (existing + §08)         │
                          │  • RC elimination (existing)            │
                          │  • COW check elimination (§07)          │
                          └────────────┬────────────┬───────────────┘
                                       │            │
                    ┌──────────────────▼──┐    ┌───▼──────────────────┐
                    │  LLVM Codegen        │    │  Interpreter          │
                    │  (ori_llvm)          │    │  (ori_eval)           │
                    │  • COW emission (§02-04)│ │  • Arc::make_mut (§06)│
                    │  • Slice ops (§05)   │    │  • Slice support (§06)│
                    │  • Inline fast path  │    │  • SSO support (§06)  │
                    └──────────┬───────────┘    └───────────────────────┘
                               │
                    ┌──────────▼───────────┐
                    │  Runtime (ori_rt)     │
                    │  • COW primitives (§01)│
                    │  • List COW ops (§02) │
                    │  • String SSO (§03)   │
                    │  • Map/Set COW (§04)  │
                    │  • Slice mgmt (§05)   │
                    │  • Drop spec (§08)    │
                    └──────────────────────┘
```

### The COW Fast Path (Core Pattern)

Every mutating collection operation follows this pattern:

```
┌─────────────────────────────┐
│  ori_list_push(list, elem)  │
└──────────┬──────────────────┘
           │
     ┌─────▼─────┐
     │ RC == 1 ?  │──── Yes ──→ FAST PATH: mutate in-place, O(1) amortized
     └─────┬──────┘             (capacity check → grow if needed → write)
           │ No
           ▼
     SLOW PATH: allocate new buffer, copy, append, dec old
     (still O(n) but only happens when actually shared)
```

With static uniqueness analysis (§07), the compiler can **eliminate the RC==1 check entirely** when it can prove the value is uniquely owned, reducing to:

```
     UNCONDITIONAL FAST PATH: mutate in-place, O(1) amortized
```

## Design Principles

### 1. Pay Only for Sharing

The cost of a mutation is proportional to the actual degree of sharing, not the number of mutations. A value that flows linearly (the common case) is mutated in-place with zero overhead. A value that is shared incurs exactly one copy at the point of divergence — no more, no less.

**Why this matters:** Naive value semantics copies on every mutation (O(n) per push). COW reduces this to O(1) amortized for the common case. Static uniqueness analysis can further eliminate the runtime check. The result: value semantics with reference-semantics performance.

### 2. Zero-Copy by Default

Read-only operations (slicing, substring, iteration) should never allocate. Seamless slices share the underlying buffer with a flag bit, incurring only an RC increment. The original and the slice share memory until one needs to mutate, at which point COW kicks in.

**Why this matters:** Operations like `str.substring(1, 5)` or `list.slice(2, 10)` are common in real programs. Without seamless slices, each creates a new allocation and copies data. With slices, they're O(1) regardless of size.

### 3. Dual-Backend Parity

Every optimization implemented in the LLVM backend must have a corresponding optimization in the interpreter. The interpreter uses `Arc::make_mut()` (Rust's built-in COW for Arc) to achieve the same semantics. Performance characteristics may differ, but correctness must be identical.

**Why this matters:** Ori has two execution paths (interpreter for `ori run`, AOT for `ori build`). If they behave differently, users get inconsistent results. Dual-execution verification (§09) catches any divergence.

## Section Dependency Graph

```
                    ┌────────┐
                    │ §01    │  Runtime COW Foundation
                    │ Found. │  (uniqueness API, capacity, sentinels)
                    └───┬────┘
                        │
           ┌────────────┼────────────┬────────────┐
           ▼            ▼            ▼            ▼
      ┌────────┐  ┌────────┐  ┌────────┐  ┌────────┐
      │ §02    │  │ §03    │  │ §04    │  │ §05    │
      │ List   │  │ String │  │ Map/Set│  │ Slices │
      │ COW    │  │ Optim. │  │ COW    │  │        │
      └───┬────┘  └───┬────┘  └───┬────┘  └───┬────┘
          │            │           │            │
          └────────────┼───────────┼────────────┘
                       ▼           │
                  ┌────────┐       │
                  │ §06    │◀──────┘
                  │ Interp │  Interpreter COW Parity
                  │ Parity │
                  └───┬────┘
                      │
                 ┌────▼────┐
                 │ §07     │  Static Uniqueness Analysis
                 │ Static  │  (requires §01-06 to be testable)
                 │ Unique  │
                 └────┬────┘
                      │
                 ┌────▼────┐
                 │ §08     │  Collection Memory Recycling
                 │ Recycle │  (extends reset/reuse with COW)
                 └────┬────┘
                      │
                 ┌────▼────┐
                 │ §09     │  Verification & Benchmarks
                 │ Verify  │  (requires all above)
                 └─────────┘
```

- **§01** is the foundation — everything depends on it.
- **§02, §03, §04, §05** are independent of each other and can be worked in parallel.
- **§06** requires §02-§05 (implements interpreter versions of all COW operations).
- **§07** requires §01-§06 (static analysis needs testable COW paths to validate).
- **§08** requires §07 (memory recycling leverages uniqueness info).
- **§09** requires all sections (comprehensive verification).

**Cross-section interactions (must be co-implemented):**
- **§01 + §02**: The runtime COW primitives (§01) and list COW operations (§02) must land together. Without §01's uniqueness check API, §02's COW list functions can't branch.
- **§03 (SSO) + §05 (String Slices)**: SSO changes the string struct layout. Slices must be slice-aware of both SSO and heap strings. Landing one without the other creates inconsistent string handling.
- **§05 (Slices) + §06 (Interpreter)**: Slice encoding must be agreed upon before either backend implements it, since both share the same spec tests.

## Implementation Sequence

```
Phase 0 — Prerequisites
  └─ §01.1: Add ori_rc_is_unique() to runtime
  └─ §01.2: Add capacity management helpers
  └─ §01.3: Establish benchmark baselines (§09.1 prereq)
  Gate: ori_rc_is_unique() callable from LLVM, benchmarks recorded

Phase 1 — Foundation (§01 complete)
  └─ §01.3: Growth strategy implementation (2x doubling)
  └─ §01.4: Empty collection sentinels
  └─ §01.5: Runtime function declarations in LLVM
  └─ §01.6: Foundation test suite
  Gate: All §01 tests pass, sentinel lists/strings work in AOT

Phase 2 — Core COW (§02, §03, §04, §05 — parallelizable)
  ├─ §02: List COW operations (all mutations in-place when unique)
  ├─ §03: String SSO + COW concat
  ├─ §04: Map/Set COW operations
  └─ §05: Seamless slices for lists and strings
  Gate: All collection mutations are O(1) when unique, slices are zero-copy

Phase 3 — Dual-Backend Parity (§06)   [CRITICAL PATH]
  └─ §06: Interpreter COW via Arc::make_mut()
  └─ §06.5: Dual-execution verification (JIT vs AOT equivalence)
  Gate: dual-exec-verify.sh passes with 0 mismatches

Phase 4 — Static Analysis (§07)
  └─ §07: Static uniqueness analysis in ori_arc
  └─ §07.5: COW check elimination for provably unique values
  Gate: Benchmarks show eliminated runtime checks, no regressions

Phase 5 — Memory Recycling (§08)
  └─ §08: Extended reset/reuse + drop specialization for collections
  Gate: Valgrind clean, collection recycling measurable in benchmarks

Phase 6 — Verification (§09)   [FINAL GATE]
  └─ §09: Full test matrix, Valgrind suite, perf benchmarks, dual-exec
  Gate: ALL tests green, Valgrind clean, benchmarks show ≥ parity with
        reference languages on canonical collection workloads
```

**Why this order:**
- Phase 0-1 are pure additions — no behavioral changes to existing code.
- Phase 2 sections are independent and can be parallelized across sessions.
- Phase 3 is the critical path because behavioral equivalence must be verified before static analysis can be trusted.
- Phase 4 builds on all COW paths being correct before optimizing them away.
- Phase 5 is the most aggressive optimization, requiring everything else to be solid.

**Known failing tests (expected until plan completion):**

None expected — each phase preserves backward compatibility. COW changes the *performance* of collection operations, not their *semantics*. Existing tests should continue to pass at every phase boundary.

If any test fails during implementation, it indicates a bug in the COW logic (not a missing infrastructure dependency) and must be investigated immediately.

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_rt` | ~620 | ~50 | ~670 |
| `ori_arc` | ~3200 | ~1800 | ~5000 |
| `ori_llvm` (arc_emitter) | ~2400 | ~200 | ~2600 |
| `ori_eval` (methods) | ~1800 | ~400 | ~2200 |
| `ori_patterns` (value) | ~2200 | ~600 | ~2800 |
| **Total affected** | **~10,200** | **~3,050** | **~13,250** |

## Estimated Effort

| Section | Est. New Lines | Est. Test Lines | Complexity | Depends On |
|---------|---------------|-----------------|------------|------------|
| 01 Runtime COW Foundation | ~400 | ~200 | Medium | — |
| 02 List COW Operations | ~500 | ~400 | High | 01 |
| 03 String Optimization | ~600 | ~350 | High | 01 |
| 04 Map & Set COW | ~400 | ~300 | Medium | 01 |
| 05 Seamless Slices | ~500 | ~400 | High | 01 |
| 06 Interpreter Parity | ~300 | ~200 | Medium | 02-05 |
| 07 Static Uniqueness | ~600 | ~400 | Very High | 01-06 |
| 08 Collection Recycling | ~400 | ~300 | High | 07 |
| 09 Verification | ~200 | ~600 | Medium | All |
| **Total new** | **~3,900** | **~3,150** | | |
| **Grand total** | **~7,050** | | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| All list mutations allocate unconditionally | No uniqueness check in runtime | §01 + §02 | Not Started |
| String concat always copies both sides | No COW for strings | §03 | Not Started |
| `list.slice()` copies elements | No seamless slice support | §05 | Not Started |
| Interpreter clones Arc on every mutation | Not using `Arc::make_mut()` | §06 | Not Started |
| Map uses O(n) linear scan | Parallel array layout, no hash table | §04 (noted, hash table is separate concern) | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Runtime COW Foundation | `section-01-runtime-cow-foundation.md` | Not Started |
| 02 | List COW Operations | `section-02-list-cow.md` | Not Started |
| 03 | String Optimization | `section-03-string-optimization.md` | Not Started |
| 04 | Map & Set COW Operations | `section-04-map-set-cow.md` | Not Started |
| 05 | Seamless Slices | `section-05-seamless-slices.md` | Not Started |
| 06 | Interpreter COW Parity | `section-06-interpreter-parity.md` | Not Started |
| 07 | Static Uniqueness Analysis | `section-07-static-uniqueness.md` | Not Started |
| 08 | Collection Memory Recycling | `section-08-collection-recycling.md` | Not Started |
| 09 | Verification & Benchmarks | `section-09-verification.md` | Not Started |

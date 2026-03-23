---
plan: "aot-perf"
title: "AOT Codegen Performance: Overflow Elision & String Indexing"
status: not-started
reviewed: false
references:
  - "compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs"
  - "compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs"
  - "compiler/ori_llvm/src/codegen/arc_emitter/apply_protocols.rs"
  - "compiler/ori_rt/src/string/ops.rs"
  - "plans/repr-opt/section-03-range-analysis.md"
---

# AOT Codegen Performance: Overflow Elision & String Indexing

## Mission

Close the benchmark gaps between Ori AOT and Rust by (1) eliminating provably-unnecessary overflow checks — reducing from 19 to ≤8 for equivalent code, achieving parity with `rustc -C overflow-checks=yes` — and (2) fixing the string indexing codegen crash so `s[i]` works in AOT mode.

## Context: Benchmark Findings (2026-03-23)

| Benchmark | Ori (O3) | Rust checked (O3) | Gap | Root Cause |
|-----------|----------|-------------------|-----|------------|
| Compute | 166.6 ms | 160.9 ms | 1.04x | 19 overflow checks vs 8 |
| Recursion | 137.5 ms | 135.0 ms | 1.02x | Same — post-guard checks not elided |
| Allocation | 53.2 ms | 7.7 ms | 6.9x | ARC runtime (out of scope — see repr-opt) |
| String | CRASH | — | — | `__index` on `TypeInfo::Str` not implemented |

The code journeys (J1–J20, avg 9.98/10) confirm the IR is structurally optimal. The gap is purely unnecessary overflow intrinsics and a missing codegen handler.

## Architecture

```
                    ┌─────────────────────────────────────┐
                    │  ARC IR: BinaryOp(Add, lhs, rhs)    │
                    └──────────────┬──────────────────────┘
                                   │
                                   ▼
                    ┌─────────────────────────────────────┐
                    │  operators/strategy.rs               │
                    │  emit_int_binary_op()                │
                    │                                     │
                    │  TODAY: Add → checked_add (always)   │
                    │  AFTER: Add → checked_add OR add_nsw │
                    │         based on operand provability │
                    └──────────────┬──────────────────────┘
                                   │
                    ┌──────────────┴──────────────────────┐
                    │                                     │
                    ▼                                     ▼
          ┌──────────────────┐              ┌──────────────────────┐
          │  checked_ops.rs  │              │  arithmetic.rs       │
          │  sadd.with.ovf   │              │  add nsw / sub nsw   │
          │  + panic block   │              │  (plain, no panic)   │
          │  7 instructions  │              │  1 instruction       │
          └──────────────────┘              └──────────────────────┘
```

## Design Principles

### 1. Correctness Preserved — No Semantic Change

Overflow checks may only be elided when the operation is **provably safe**. A false elision is silent data corruption — worse than a 3% perf gap. Every elision must be justified by one of:
- Constant operands (compile-time proof)
- Post-guard range (control-flow proof via branch condition)
- Loop counter with known bound (structural proof)

### 2. Incremental — Does Not Block repr-opt

This plan implements **simple, local elisions** that don't require cross-function range analysis. The full range analysis framework (repr-opt §03) will subsume these with a more powerful system. This plan captures the low-hanging fruit now and is forward-compatible with §03.

### 3. Surgical — Minimal Blast Radius

Both fixes touch exactly 2-3 files each. No architectural changes, no new crates, no new pipeline stages.

## Section Dependency Graph

```
§01 Overflow Elision ─── (independent) ─── §02 String Indexing
```

Sections are fully independent. Can be implemented in any order or in parallel.

## Implementation Sequence

```
Phase 1 (parallel)
  ├─ §01: Overflow Check Elision (checked_ops.rs + strategy.rs)
  └─ §02: String Indexing Codegen (apply_protocols.rs + ori_rt/string/ops.rs)

Phase 2
  └─ Re-run benchmarks to verify improvements
```

## Relationship to repr-opt

repr-opt §03 (Range Analysis) will implement a full abstract-interpretation engine that tracks value ranges through the entire ARC IR. When §03 lands, it will subsume §01's local elisions with a more general system. This plan's changes are forward-compatible:

- §01 adds `add_nsw` / `sub_nsw` methods to `IrBuilder` — §03 will use these same methods
- §01 adds constant-operand detection in `emit_checked_binop` — §03 will add range-based detection alongside it
- No new types or APIs need to be removed when §03 arrives

## Codebase Findings

### String indexing: missing codegen handler
- `apply_protocols.rs:92-102` — `match &type_info` has `List` and `Map` but no `Str` case
- `ori_rt/src/string/ops.rs` — no `ori_str_index` or `ori_str_get` function
- Type checker and ARC lowering already handle `str[i]` correctly
- Interpreter handles it correctly
- Only LLVM codegen is missing

### Overflow: hardcoded checked dispatch
- `strategy.rs:103-105` — `Add → checked_add`, `Sub → checked_sub`, `Mul → checked_mul` with no conditions
- `checked_ops.rs:38-46` — CSE cache already detects compile-time constants via `get_zero_extended_constant()` but only uses it for cache keying, not for elision
- `arithmetic.rs` — unchecked `add()`, `sub()`, `mul()` exist but are never called for user-facing integer ops

## Metrics

| Crate | Files Touched | Est. New Lines | Est. Deleted |
|-------|--------------|----------------|--------------|
| `ori_llvm` | 3 | ~80 | ~5 |
| `ori_rt` | 1 | ~40 | 0 |
| Tests | 2 | ~100 | 0 |
| **Total** | **6** | **~220** | **~5** |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Overflow Check Elision | `section-01-overflow-elision.md` | Not Started |
| 02 | String Indexing Codegen | `section-02-string-indexing.md` | Not Started |

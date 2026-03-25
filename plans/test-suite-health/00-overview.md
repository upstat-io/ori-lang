---
plan: "test-suite-health"
title: "Test Suite Health: LCFail Elimination & Performance Optimization"
status: not-started
supersedes: []
references:
  - "plans/roadmap/section-21A-llvm.md"
  - "plans/roadmap/section-23-evaluator.md"
---

# Test Suite Health: LCFail Elimination & Performance Optimization

## Mission

Restore the Ori test suite to full health across two fronts: (1) audit, categorize, and reprioritize the LLVM backend roadmap (Section 21A) so that LCFail tests can be driven from 3,956 to 0 via an impact-ordered implementation sequence — this plan creates the tracking infrastructure and priority ordering, then the actual codegen implementation is executed via Section 21A's subsections in the new priority order, and (2) target reducing the `cargo t` wall time from ~59s to <=30s (-50%) by profiling the compiler, optimizing the AOT test pipeline, and eliminating hot-path inefficiencies — without modifying a single test. The 30s target is aspirational; profiling data may reveal a different achievable floor, in which case the target is revised with rationale.

**Scope**: This plan targets `cargo t` (Rust workspace tests, including AOT integration tests) wall time. `./test-all.sh` runs additional phases (release build, WASM build check, Ori spec tests via interpreter, Ori spec tests via LLVM backend) that are outside scope — those are sequential and dominated by the release build step. The LCFail track affects the Ori spec LLVM backend tests (run by `./test-all.sh`), not `cargo t` directly.

## Current State (2026-03-25)

### LCFail

| Test Suite | Passed | Failed | Skipped | LCFail | Total |
|------------|--------|--------|---------|--------|-------|
| Ori spec (interpreter) | 4,181 | 0 | 42 | - | 4,223 |
| Ori spec (LLVM backend) | 257 | 0 | 10 | 3,956 | 4,223 |
| Roadmap Section 21A (STALE) | 1,082 | 1 | 9 | 1,985 | 3,077 |

The roadmap's numbers are **stale** (from an earlier date). Actual LCFail is **3,956** — nearly double what the roadmap reports. 93.7% of LLVM spec test functions fail. 188 out of 295 test files cannot compile in LLVM mode at all.

**Root cause**: LCFail is NOT a test annotation — it's an automatic classification when LLVM codegen fails to compile a test file. Every test in that file becomes LCFail. The failures map to missing LLVM codegen features in Section 21A.

### Test Performance

| Component | Execution Time | % of Total |
|-----------|---------------|------------|
| AOT integration tests (~1,950 tests) | 35.6s | 60% |
| ori_eval (compilation + tests) | 4.5s | 8% |
| ori_arc (1,012 tests) | 3.4s | 6% |
| ori_patterns (compilation + tests) | 2.6s | 4% |
| ori_ir (compilation + tests) | 2.5s | 4% |
| ori_parse (compilation + tests) | 1.7s | 3% |
| ori_diagnostic (compilation + tests) | 1.5s | 3% |
| ori_lexer (compilation + tests) | 1.2s | 2% |
| ori_registry | 0.4s | <1% |
| ori_types | 0.4s | <1% |
| ori_rt | 0.2s | <1% |
| **Total wall time (parallel)** | **~59s** | **100%** |

**Key observation**: System time (537s) exceeds user time (323s) in `cargo t`, indicating the bottleneck is I/O and process spawning (compile→link→execute cycles), not CPU computation. The AOT test pipeline is the dominant cost. Note: each AOT test spawns `ori build` as a separate subprocess via `Command::new()` (see `compiler/ori_llvm/tests/aot/util/aot.rs:compile_and_run_capture`), which means each test is a complete process — there is no in-process LLVM context sharing or Salsa caching across AOT tests.

## Architecture

```
Part 1: LCFail Elimination (Roadmap Reprioritization)
======================================================
Section 01: Audit & Baseline
  └─ Categorize all 3,956 LCFail tests by root cause
  └─ Fix stale roadmap numbers
  └─ Map each category to Section 21A subsections

Section 02: Roadmap Reprioritization
  └─ Reorder Section 21A subsections by test-unblocking impact
  └─ Create accelerated implementation sequence
  └─ Define LCFail milestones (<1500→<1000→<500→<200→<50→0)


Part 2: Performance Optimization (Target: 30s)
================================================
Section 03: Profiling Infrastructure
  └─ Set up flamegraph generation for cargo t
  └─ Add per-phase timing to AOT test harness
  └─ Establish reproducible measurement methodology

Section 04: AOT Pipeline Optimization
  └─ Profile the 35.6s AOT bottleneck
  └─ Optimize linker configuration
  └─ Reduce per-test compilation overhead
  └─ Target: 35.6s → ≤15s

Section 05: Compiler Hot Path Optimization
  └─ Profile compiler crate compilation times
  └─ Optimize Rust code in hot paths
  └─ Target: remaining ~23s → ≤15s

Section 06: Verification
  └─ Confirm LCFail tracking infrastructure works
  └─ Confirm 30s target met
  └─ Regression guard: CI timing assertions
```

## Design Principles

1. **Data-driven optimization** — Profile first, optimize second. No speculative changes. Every optimization must be measured before and after with reproducible methodology.

2. **Zero test modification** — All performance gains come from compiler/infrastructure changes. Tests are the load — they define the workload, not the variable.

3. **Impact-ordered LCFail work** — Each roadmap item is prioritized by how many LCFail tests it unblocks, not by implementation difficulty or logical ordering. Generic monomorphization first because it unblocks the most tests.

## Section Dependency Graph

```
LCFail Track:        01 (Audit) ──→ 02 (Reprioritize)
                                                          [tracks are independent]
Performance Track:   03 (Profiling) ──┬──→ 04 (AOT Opt) ──┐
                                      └──→ 05 (Hot Paths) ─┤
                                                            └──→ 06 (Verify)
                                                                   ↑
                                                          01 + 02 ─┘
```

- The **LCFail track** (01→02) and **Performance track** (03→04/05) are independent and can be worked in parallel.
- Within the Performance track, Section 03 must precede 04 and 05 (profiling data needed before optimization). Sections 04 and 05 are independent of each other.
- Section 06 depends on ALL prior sections (both tracks).

## Implementation Sequence

```
Phase 0 - Foundation
  └─ Section 01: LCFail audit + fix stale roadmap numbers
  └─ Section 03: Profiling infrastructure (parallel with 01)

Phase 1 - Analysis
  └─ Section 02: Roadmap reprioritization (needs 01)
  └─ Section 04: AOT pipeline optimization (needs 03)
  └─ Section 05: Compiler hot paths (needs 03, parallel with 04)

Phase 2 - Verification
  └─ Section 06: Confirm targets met
```

**Why this order:**
- Phase 0 produces the data that Phase 1 needs (audit results, profiling data).
- Phase 1 items are independent of each other — LCFail reprioritization doesn't affect performance optimization.
- Phase 2 is the gate: LCFail tracking in place, 30s target met.

## LCFail Root Cause Breakdown (Priority Order)

| Priority | Feature (21A Subsection) | Est. Tests Unblocked | Why |
|----------|------------------------|---------------------|-----|
| P0 | Generic monomorphization (in 21.7 "Function Sequences & Expressions") | ~2,500+ | `assert_eq<T>` is generic — used in 3,946+ call sites across nearly all test files |
| P1 | Sum type codegen (in 21.2 "Type Lowering") | ~500+ | `Ordering` type blocks prelude `compare()`. Re-enables prelude compilation in JIT mode |
| P2 | Lambda/closure ABI (21.11 "Lambda & Closure Support") | ~400+ | `.map()`, `.filter()`, all HOF patterns blocked |
| P3 | Operator trait dispatch + impl blocks (21.4 "Operator Trait Dispatch") | ~200+ | Struct methods, operator trait dispatch for user types |
| P4 | Built-in functions (21.12 "Built-in Functions") | ~150+ | Remaining prelude functions beyond assert/compare |
| P5 | Control flow extensions (21.5 "Control Flow") | ~100+ | for-yield, try, catch patterns |
| P6 | Pattern matching (21.6 "Pattern Matching") | ~50+ | match expressions, advanced destructuring |
| P7 | Collections & iterators (21.10 "Collections & Iterators") | ~30+ | Collection operations, iterator trait dispatch |
| P8 | Everything else (21.8-21.15) | remaining | Capabilities, FFI, concurrency, conditional compilation, ARC |

**Note**: Estimates are rough because LCFail is file-level — fixing one feature can unblock ALL tests in files that only fail because of that one feature. The actual impact of P0 (monomorphization) could be even higher because it's the single feature that prevents most files from compiling. The `assert_eq` call site count (3,946 as of 2026-03-25) is higher than the roadmap's stale figure of 2,472.

## Performance Optimization Targets

| Metric | Current | Target | Reduction |
|--------|---------|--------|-----------|
| `cargo t` wall time | 59s | 30s | -49% |
| AOT test execution | 35.6s | <=15s | -58% |
| Non-AOT test time (parallel with AOT) | ~23s combined | <=15s | Overlaps with AOT; matters only if it exceeds AOT time |
| Test binary compilation (sequential) | ??? | Profiling needed | Potentially significant contributor to 59s wall time |
| System:User time ratio | 1.66:1 | <1:1 | I/O reduction |

**Composition**: `cargo t` wall time = max(AOT, non-AOT) in parallel + sequential overhead. Since AOT (35.6s) and non-AOT (~23s) run as separate crate test binaries in parallel, the wall time is dominated by the slowest crate. If AOT drops to 15s and non-AOT drops to 15s, the parallel wall time is ~15s + overhead. The 30s target includes margin for cargo orchestration, compilation of test binaries (if not pre-built), and sequential test phases.

**Optimization candidates** (to be validated by profiling):
1. **Linker**: Switch to `lld` (available) or `mold` (not installed) for AOT test linking — system linkers are slow
2. **Shared compilation**: Pre-compile common runtime functions once, share across AOT tests (ori_rt is already a pre-built static lib; check if the linker re-reads it per test)
3. **Process overhead**: Each AOT test spawns 2 subprocesses (`ori build` + binary execution) via `Command::new()`. With ~1,950 tests, that's ~3,900 process launches. Reducing per-test process overhead (e.g., batching tests, reusing temp dirs, or an in-process compilation mode) could yield significant gains.
4. **Crate compilation**: Profile and optimize Rust build times for test binaries
5. **Compiler hot paths**: Profile the Ori compiler itself during test execution

## Metrics (Baseline — 2026-03-25)

| Suite | Tests | Execution Time | Notes |
|-------|-------|---------------|-------|
| Rust workspace (excl. ori_llvm) | ~6,700 | ~5s execution | Parallel, fast |
| ori_llvm lib | ~206 | 0.5s | LLVM context overhead |
| AOT integration | ~1,950 | 35.6s | Compile+link+execute each |
| ori_rt | ~36 | 0.2s | Runtime library |
| Ori spec (interpreter) | 4,181 | 6.4s | Full pipeline per file |
| Ori spec (LLVM) | 257 pass + 3,956 LCFail | 1.3s | Most fail before execution |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 LCFail Audit & Baseline | ~50 (roadmap update) | Low | -- |
| 02 Roadmap Reprioritization | ~200 (roadmap rewrite) | Medium | 01 |
| 03 Profiling Infrastructure | ~100-200 | Medium | -- |
| 04 AOT Pipeline Optimization | ~200-500 | High | 03 |
| 05 Compiler Hot Paths | ~100-300 | High | 03 |
| 06 Verification | ~50-100 | Low | All |
| **Total** | **~700-1,350** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | LCFail Audit & Baseline | `section-01-lcfail-audit.md` | Not Started |
| 02 | Roadmap Reprioritization | `section-02-roadmap-reprioritization.md` | Not Started |
| 03 | Profiling Infrastructure | `section-03-profiling-infrastructure.md` | Not Started |
| 04 | AOT Pipeline Optimization | `section-04-aot-pipeline-optimization.md` | Not Started |
| 05 | Compiler Hot Paths | `section-05-compiler-hot-paths.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |

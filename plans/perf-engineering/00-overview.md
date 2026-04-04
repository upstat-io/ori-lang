---
plan: "perf-engineering"
title: "Interpreter Performance Engineering: Exhaustive Implementation Plan"
status: not-started
references:
  - "plans/bug-tracker/section-03-eval.md"
  - "plans/aot-perf/"
  - "plans/parser-perf/"
---

# Interpreter Performance Engineering: Exhaustive Implementation Plan

## Mission

Make Ori's interpreter as close to native execution speed as possible. Current function call overhead is ~63µs/call (measured via Ackermann benchmark), approximately 100-600x slower than a register-based bytecode VM like Lua. This plan transforms the tree-walking interpreter into a high-performance bytecode VM through incremental, independently testable phases. Tree-walker hot-path work (Sections 02-03) is useful only while benchmarks show allocation and clone churn still dominate; the critical path is preserving evaluator semantics while moving execution to bytecode.

## Architecture

```
Salsa Pipeline (shared):
  SourceFile → parsed() → typed() → canonicalize_cached() → SharedCanonResult

Current (tree-walking):                    Target (bytecode VM):

  SharedCanonResult                          SharedCanonResult
    │                                          │
    ▼                                          ▼
  Evaluator::builder()                     BytecodeCompiler::compile()
    │                                          │
    ▼                                          ▼
  Interpreter.eval_can()                   Chunk (bytecodes + constants)
    │                                          │
    ▼                                          ▼
  recursive dispatch                       VM.execute()
  + Environment (Rc+HashMap)                   │
  + create_function_interpreter()          ┌───┴───┐
    (5-8 mallocs per call)                │ Tight  │
    │                                     │ dispatch│
    ▼                                     │ loop   │
  ModuleEvalResult                        └───┬───┘
  (~63µs/call)                                │
                                          RegisterFile
                                          (contiguous storage)
                                              │
                                              ▼
                                          ModuleEvalResult
                                          (~0.5-2µs/call)

Post-transition:
  evaluated() [Salsa query] routes to:
    ├─ VM path (Interpret, TestRun modes) — default
    └─ Tree-walker path (ConstEval mode) — preserved for type checker
```

## Design Principles

1. **Zero allocation on the hot path.** Every heap allocation in the function call path is a performance bug. The current interpreter allocates on every call: `CallStack::clone()` (Vec alloc proportional to depth), `Environment::child()` (Vec alloc), `push_scope()` (Rc/RefCell/FxHashMap alloc), plus 5-8 Arc clones in `create_function_interpreter()`. The target is zero allocations per call. This principle is non-negotiable because 44% of current execution time is kernel-side allocation overhead.

2. **Incremental transformation, continuous correctness.** Each phase produces a measurably faster interpreter that passes all existing tests. We never have a "broken" intermediate state. Phase 1 (zero-alloc) improves the existing tree-walker. Phase 2 (bytecode) replaces it. Both must pass the full spec test suite at every step.

3. **Bytecode is the endgame, not a nice-to-have.** Tree-walking has a physics floor of ~5-10µs/call due to recursive dispatch, branch prediction misses, and pointer chasing. To approach native speed, we must eliminate recursive interpretation entirely. The bytecode VM is not an optimization — it's an architectural requirement.

4. **Semantic reuse beats semantic re-implementation.** The VM must reuse Ori's existing semantic assets where possible: canonical `CanExpr`, pre-compiled `DecisionTree`s, `ControlAction` behavior, capability scoping, and the current method-dispatch contract. Re-deriving these ad hoc in the VM is the highest-probability correctness failure mode.

## Section Dependency Graph

```
Section 01 (Benchmarks)
    │
    ├──► Section 04 (Bytecode Compilation)
    │        │
    │        └──► Section 05 (Register VM)
    │                 │
    │                 └──► Section 07 (Salsa Integration & Transition)
    │                          │
    │                          └──► Section 06 (Verification)
    │
    ├──► Section 02 (Zero-Alloc Calls)  [OPTIONAL — profile-gated]
    └──► Section 03 (Value Passing)     [OPTIONAL — profile-gated]
```

- **Sections 02-03 are OPTIONAL, profile-gated side quests.** Most of their work (Environment refactor, CallStack refactor) is rendered obsolete by the bytecode VM in 04-05, which uses a register file instead of an environment and a frame stack instead of CallStack cloning. They should only be worked if (a) benchmarks from Section 01 show allocation churn dominates over recursive dispatch, AND (b) the bytecode VM timeline is long enough to justify the intermediate tree-walker investment. If profiling shows recursive dispatch is the dominant cost (likely), skip 02-03 entirely and proceed to 04. Section 01 must record an explicit go/no-go decision so later sections treat skipped 02-03 work as intentionally non-blocking rather than silently unfinished. The critical path is 01 -> 04 -> 05 -> 07 -> 06.
- Section 04 depends on Section 01 plus a validated semantic inventory of the current evaluator.
- Section 05 depends on 04 (VM executes the bytecodes).
- **Section 07** handles the transition from tree-walker to VM: Salsa query integration, const-eval preservation, the `ori run` / `ori test` switchover, and the feature flag for gradual rollout. This is the section that makes the VM actually usable in the compiler pipeline. It depends on Section 05 (a working VM).
- Section 06 depends on Section 07 (can't verify the full pipeline until the VM is integrated). Section 06 verifies correctness, not just VM execution.
- Section 01 is the prerequisite for everything (can't optimize without measuring).

**Cross-section interactions:**
- **Section 04 + Section 05**: The bytecode instruction set (Section 04) is consumed by the VM dispatch loop (Section 05). The ISA design in 04 must be register-based from the start.
- **Section 05 + Section 07**: The VM must be designed with both integration seams in mind. `ori run` flows through `evaluated()`, but `ori test` uses `TestRunner` + `Evaluator::load_module(...)`. Section 07 owns wiring both paths to the same VM/runtime contract.
- **Section 07 + Section 06**: Verification can only test the full pipeline after Section 07 wires up both `ori run` and `ori test`. Section 06's dual-execution testing must compare the tree-walker and VM through the real entry points, not just an isolated query helper.

## Implementation Sequence

```
Phase 0 - Measurement
  └─ 01: Benchmark infrastructure (Criterion suite, Ackermann gate test)

Phase 1 - Bytecode Compilation  [CRITICAL PATH]
  └─ 04: Bytecode compilation (CanExpr + DecisionTree → bytecode)
  Gate: representative programs compile with source maps and semantic parity tests

Phase 1b - Optional Tree-Walker Hot-Path Wins  [PROFILE-GATED, likely skipped]
  └─ 02: Zero-alloc call path (CallStack + Environment)
  └─ 03: Value passing optimization (parameter binding + self-binding)
  Gate: work these ONLY if Phase 0 profiling shows allocation churn > recursive dispatch cost

Phase 2 - Register VM  [CRITICAL PATH]
  └─ 05: Register-based VM (dispatch loop + register file + control flow unwinding)
  Gate: Ackermann A(4,1)=65533 completes in <5s on the measured dev machine

Phase 3 - Integration & Transition  [CRITICAL PATH]
  └─ 07: Salsa integration, const-eval preservation, tree-walker retention, feature flag switchover
  Gate: `ori run` and the interpreter-backed `ori test` path can execute through the intended tree-walker/VM split

Phase 4 - Verification
  └─ 06: Full test suite parity + performance regression suite
  Gate: All spec tests pass, all benchmarks within 2x of baseline
```

**Why this order:**
- Phase 0 establishes measurement — can't optimize without numbers.
- Section 04 is the architectural hinge. If its semantic contract is wrong, later VM work will be fast but incorrect.
- Sections 02-03 are useful only if profiling shows allocation dominates; they should be skipped if recursive dispatch is the bottleneck (which the 44% kernel time suggests — most of that is `malloc` from `create_function_interpreter`, which the VM eliminates entirely).
- Phase 3 is the critical integration work that makes the VM usable in the real compiler. Without it, the VM is an isolated module that can't actually run programs through `ori run` or the interpreter-backed `ori test` path. This phase handles: (a) plugging the VM into the `evaluated()` path, (b) wiring the test runner/backend path, (c) preserving the tree-walker for `ConstEval` mode (used by the type checker), and (d) a feature flag for gradual rollout.
- Phase 4 proves correctness and becomes the ship gate for switching defaults.

**Known failing tests (expected until plan completion):**

None. Each phase maintains full test suite compatibility. The existing tree-walker is not removed until the bytecode VM passes all tests.

## Metrics (Current State)

**Interpreter call overhead (Ackermann benchmark):**
- A(3,5) = 253: ~42,438 calls — completes in ~3s
- A(3,7) = 1021: ~693,964 calls — completes in ~55s
- A(4,1) = 65533: ~89M calls — timeout (>120s)
- **Per-call cost: ~63µs** (measured: 55s / ~866k calls, where 866k = A(3,6) ~172k + A(3,7) ~694k as run in `ack_perf_test.ori`)
- **System time: 44%** (24s of 55s — malloc/free pressure)

**Target per-call cost by phase:**
| Phase | Target | Speedup | Mechanism | Gate Test |
|-------|--------|---------|-----------|-----------|
| Current | 63µs | 1x | Tree-walking + allocations | A(3,7) = 55s |
| Phase 1b (optional) | ~5-10µs | 6-12x | Zero-alloc tree-walking | A(3,7) < 10s |
| Phase 2 | ~0.2-0.5µs | 120-300x | Bytecode VM | A(3,8) < 1s (Python: 0.386s) |
| Phase 3 | Same as Phase 2 | Same | VM through full Salsa pipeline | `ori run` A(3,8) < 1s |

**Reference interpreter speeds (measured on this system via Ackermann):**

| Language | A(3,7) time | A(3,8) time | µs/call | vs Ori |
|----------|-------------|-------------|---------|--------|
| Node.js V8 22 (JIT) | 0.007s | 0.008s | 0.01 µs | 6,300x faster |
| Python 3.12 (bytecode) | 0.096s | 0.386s | 0.14 µs | 450x faster |
| **Ori (current)** | **~55s** | **timeout** | **63 µs** | **baseline** |

**Concrete target:** reach the same order of magnitude as Python 3.12 on the measured dev machine, with Python treated as a reference point rather than a CI-stable absolute gate.
Stretch target: approach Lua-class bytecode VM (0.05-0.1 µs/call).
V8-class performance (0.01 µs/call) requires JIT to native — out of scope for this plan.

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_eval` | ~14,200 | ~6,200 | ~20,400 |
| `ori_patterns` | ~9,000 | ~4,400 | ~13,400 |
| **Total** | **~23,200** | **~10,600** | **~33,800** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Benchmarks | ~200 | Low | — |
| 02 Zero-Alloc Calls | ~400 | Medium | 01 (OPTIONAL — profile-gated) |
|   ↳ 02.1 CallStack refactor | ~100 | Low | — |
|   ↳ 02.2 Environment refactor | ~200 | Medium | — |
|   ↳ 02.3 Interpreter construction | ~100 | Medium | 02.1, 02.2 |
| 03 Value Passing | ~200 | Medium | 01 (OPTIONAL — profile-gated) |
|   ↳ 03.1 Parameter binding | ~100 | Low | — |
|   ↳ 03.2 Self-binding + captures | ~100 | Low | — |
| 04 Bytecode Compilation | ~1,500 | High | 01 |
|   ↳ 04.1 Instruction set design | ~200 | High | — |
|   ↳ 04.2 Bytecode compiler | ~800 | High | 04.1 |
|   ↳ 04.3 Constant pool + closures | ~500 | Medium | 04.2 |
| 05 Register VM | ~1,000 | High | 04 |
|   ↳ 05.1 VM core + dispatch | ~500 | High | — |
|   ↳ 05.2 Call frames + scoping | ~300 | Medium | 05.1 |
|   ↳ 05.3 Method dispatch integration | ~200 | Medium | 05.2 |
| 07 Salsa Integration & Transition | ~500 | High | 05 |
|   ↳ 07.1 Salsa query integration | ~150 | High | — |
|   ↳ 07.2 Const-eval & test-mode preservation | ~100 | Medium | — |
|   ↳ 07.3 Feature flag & switchover | ~150 | Medium | 07.1, 07.2 |
|   ↳ 07.4 Tree-walker retirement strategy | ~100 | Low | 07.3 |
| 06 Verification | ~300 | Medium | 07 |
| **Total new (critical path)** | **~3,500** | | (01 + 04 + 05 + 07 + 06) |
| **Total new (with optional 02-03)** | **~4,100** | | |
| **Total deleted** | **~2,500** | | (tree-walker *runtime-only* dispatch: ~1,500 in can_eval + ~1,000 in function_call/method_dispatch/operator_dispatch — only after Section 07 proves the VM handles Interpret+TestRun modes; tree-walker code used by ConstEval and PatternExecutor is preserved per 07.4) |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-03-004: 63µs/call overhead | 5-8 heap allocs per call + recursive dispatch | Section 04-05 (bytecode VM eliminates all per-call allocations and recursive dispatch) | Not Started |
| CallStack::clone() per call | Deep copy Vec<CallFrame> | Eliminated by VM frame stack (Section 05.2) | Not Started |
| Environment::child() per call | Vec alloc (scopes) + Rc clone (global) + push_scope allocates Rc/RefCell/FxHashMap | Eliminated by VM register file (Section 05.1) | Not Started |
| Value::clone() per parameter | Repeated clone / Arc bump on every binding | Reduced by VM register passing (Section 05.2) — some clones remain for shared Values | Not Started |
| FunctionValue self-binding clone | 3 Vec allocs (params, can_defaults, capabilities) + 3 Arc bumps per self-bind | Eliminated by VM chunk-based function lookup (Section 05.3) | Not Started |

## Codebase Hygiene Findings (Fix Along the Way)

| Finding | Category | File | Action | Section |
|---------|----------|------|--------|---------|
| `value/mod.rs` is 516 lines (limit: 500) | BLOAT | `compiler/ori_patterns/src/value/mod.rs` | Already over limit — extract a submodule (e.g., `display.rs` or `factory.rs`) on first touch. VM integration (05.1) may need new `Value` helper methods (e.g., `as_int()`, `as_bool()` for register extraction), which would trigger this | 05.1 or any section that adds to `value/mod.rs` |
| 2 TODOs in `decision_tree/mod.rs` reference "section-07" | STYLE | `compiler/ori_eval/src/exec/decision_tree/mod.rs:266,276` | Resolve during Section 04.3 (DecisionTree compilation — primary owner); verify in 07.1 (integration — fallback if 04.3 deferred) | 04.3 (primary), 07.1 (verify) |
| 1 TODO in `can_eval/mod.rs` about type reference resolution | STYLE | `compiler/ori_eval/src/interpreter/can_eval/mod.rs:119` | Verify resolved by canonicalization (CanExpr::TypeRef exists), update comment | 07.1 |
| `Interpreter` struct has 22 fields | NOTE | `compiler/ori_eval/src/interpreter/mod.rs:110-195` | Not a hygiene violation (documented, well-commented), but the VM struct (Section 05.1) should target fewer fields by design | 05.1 |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Benchmark Infrastructure | `section-01-benchmarks.md` | Not Started |
| 02 | Zero-Allocation Call Path (OPTIONAL) | `section-02-zero-alloc-calls.md` | Not Started |
| 03 | Value Passing Optimization (OPTIONAL) | `section-03-value-passing.md` | Not Started |
| 04 | Bytecode Compilation | `section-04-bytecode.md` | Not Started |
| 05 | Register-Based VM | `section-05-register-vm.md` | Not Started |
| 07 | Salsa Integration & Transition | `section-07-integration.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |

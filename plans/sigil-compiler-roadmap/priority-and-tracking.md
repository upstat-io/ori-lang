# Priority Order & Tracking

## Current Status

### Tier 1: Foundation

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Type System Foundation | ✅ Complete | All tests pass |
| 2 | Type Inference | ✅ Complete | All tests pass |
| 3 | Traits | ✅ Complete | All tests pass including map.len(), map.is_empty() |
| 4 | Modules | 🔶 Core complete | 16/16 tests pass; remaining: module alias, re-exports, qualified access |
| 5 | Type Declarations | 🔶 Partial | Structs work; remaining: destructuring, sum type constructors/matching |

### Tier 2: Capabilities & Stdlib

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 6 | Capabilities | ✅ Complete | 27/27 tests pass |
| 7 | Standard Library | 🔶 Partial | Collection methods done (map, filter, fold, find, collect); retry, validate pending |

### Tier 3: Core Patterns

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 8 | Pattern Evaluation | 🔶 Partial | Data transformation tests pass; retry pattern pending |
| 9 | Match Expressions | 🔶 Partial | guards/exhaustiveness pending |
| 10 | Control Flow | ⏳ Not started | |

### Tier 4: FFI & Interop

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 11 | FFI | ⏳ Not started | Critical for ecosystem |
| 12 | Variadic Functions | ⏳ Not started | |

### Tier 5: Language Completion

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 13 | Conditional Compilation | ⏳ Not started | |
| 14 | Testing Framework | ⏳ Not started | Gap analysis complete (see 14.9) |
| 15 | Syntax Proposals | ⏳ Not started | 15.1-15.5 from V3 |

### Tier 6: Async & Concurrency

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 16 | Async Support | ⏳ Not started | |
| 17 | Concurrency Extended | ⏳ Not started | Select, cancel, enhanced channels |

### Tier 7: Advanced Type System

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 18 | Const Generics | ⏳ Not started | |
| 19 | Existential Types | ⏳ Not started | |

### Tier 8: Advanced Features

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 20 | Reflection | ⏳ Not started | |
| 21 | Code Generation | ⏳ Not started | |
| 22 | Tooling | ⏳ Not started | |

---

## Immediate Priority

**Current Focus: Stdlib utilities & Pattern fixes (Tier 2-3)**

```
Phase 6 (Capabilities) ← ✅ COMPLETE (27/27 tests pass)
    ↓
Phase 7 (Stdlib) ← Collection methods DONE; retry, validate PENDING
    ↓
Phase 8 (Patterns) ← Fix timeout, cache, recurse patterns
```

### Phase 6 (Capabilities) — COMPLETED 2026-01-25

- [x] Capability declaration, traits, async, providing, propagation
- [x] Standard capability trait definitions in prelude
- [x] Testing with capabilities (mocking via trait implementations)
- [x] Compile-time enforcement (E2014 propagation errors)
- [x] 27/27 capability tests pass

### Phase 7 (Stdlib) — COMPLETED 2026-01-25 (Collection Methods)

- [x] Collection methods: `map`, `filter`, `fold`, `find` on lists
- [x] Range methods: `collect`, `map`, `filter`, `fold`
- [x] Map methods: `len`, `is_empty`
- [x] List helper methods: `any`, `all`
- [ ] `retry` function (in std.resilience) — 1 test blocked
- [ ] `validate` function (in std.validate) — 4 tests blocked

### Phase 3 (Traits) — COMPLETED 2026-01-25

- [x] All trait tests pass including map.len(), map.is_empty()

---

## Milestones

### M1: Bootstrapped (Tier 1) — 🔶 NEAR COMPLETE

- [x] Type system foundation
- [x] Type inference
- [x] **Traits** — core complete, missing map methods
- [x] Modules (core)
- [x] Type declarations (structs work, sum types partial)

**Exit criteria**: Can write programs using traits and modules ✅ (core functionality works)

### M2: Capabilities & Stdlib (Tier 2) — 🔶 PARTIAL

- [x] Capabilities ✅ — 27/27 tests pass
- [ ] Standard library — not started (map, filter, fold, etc. missing)

**Exit criteria**: Capabilities working ✅, stdlib available ❌

### M3: Core Patterns (Tier 3) — 🔶 PARTIAL

- [ ] Pattern evaluation — 43/74 tests pass (~58%); timeout, cache, recurse failing
- [ ] Match expressions — partial
- [ ] Control flow — not started

**Exit criteria**: All pattern and control flow constructs working

### M4: FFI & Interop (Tier 4)

- [ ] FFI
- [ ] Variadic functions

**Exit criteria**: Can call C libraries

### M5: Language Complete (Tier 5)

- [ ] Conditional compilation
- [ ] Testing framework
- [ ] Syntax proposals

**Exit criteria**: All core language features complete, testing enforced (Phases 1-15 = "core complete")

### M6: Production Async (Tier 6)

- [ ] Async support
- [ ] Concurrency (select, cancel)

**Exit criteria**: Can write server with graceful shutdown

### M7: Advanced Types (Tier 7)

- [ ] Const generics
- [ ] Existential types

**Exit criteria**: Can write matrix library with compile-time checking

### M8: Full Featured (Tier 8)

- [ ] Reflection
- [ ] Code generation
- [ ] Tooling

**Exit criteria**: Full IDE support, generic serialization

---

## Complexity Estimates

| Phase | Name | Complexity | Compiler Areas |
|-------|------|------------|----------------|
| 1-2 | Types/Inference | Low | Types |
| 3 | Traits | **High** | Parser, type checker, evaluator |
| 4-5 | Modules/Type Decls | Medium | Parser, type checker |
| 6 | Capabilities | Medium | Type checker, evaluator |
| 7 | Stdlib | Medium | Stdlib files |
| 8-10 | Patterns/Match/Control | Medium | Evaluator |
| 11 | FFI | **High** | Lexer, parser, type checker, codegen, runtime |
| 12 | Variadics | Medium | Parser, type checker |
| 13 | Conditional Compilation | Medium | Parser, build system |
| 14 | Testing | Medium | Test runner |
| 15 | Syntax Proposals | Low-Medium | Parser, lexer |
| 16 | Async | Medium | Evaluator, runtime |
| 17 | Concurrency | Medium | Evaluator, runtime |
| 18 | Const Generics | **High** | Type checker, const eval |
| 19 | Existential Types | Medium | Type checker, inference |
| 20 | Reflection | **High** | Codegen, runtime, stdlib |
| 21 | Codegen | **High** | Codegen (LLVM/Cranelift) |
| 22 | Tooling | Medium | External tools |

---

## Compiler Infrastructure Work

These are improvements to the compiler itself (Rust code) that don't fit into language phases but improve maintainability and test coverage.

### Technical Debt Cleanup (Completed 2026-01-25)

Consolidated duplicate code across crates:

- **Lexer**: Single source of truth in `sigil_lexer` crate
- **Value types**: Single source in `sigil_patterns::value`
- **Error messages**: Wrapper pattern for consistent errors
- **Type inference fixes**: Range types, await errors, type aliases, Self type, config lookup

See `.claude/plans/witty-nibbling-falcon.md` for details.

### Test Consolidation (Completed 2026-01-25)

Reorganized compiler tests following Go-style test scenarios:

**Files created:**
- `sigilc/src/eval/tests/environment_tests.rs` — 25 tests for scope/binding/capture
- `sigilc/src/eval/tests/unary_operators_tests.rs` — 40 tests for negation, not, bitwise, try
- `sigilc/src/eval/tests/expr_tests.rs` — 55 tests for literals, indexing, collection ops
- `sigilc/src/eval/tests/control_tests.rs` — 28 tests for if/else, pattern binding, loops
- `sigilc/src/eval/tests/call_tests.rs` — 20 tests for function calls, parameter binding

**Results:**
- Test count increased from 653 to 813 (+160 tests)
- Comprehensive edge case coverage (unicode, boundaries, special float values)
- Tests organized by domain, not by implementation file

**Guidelines:**
- See `docs/compiler/design/appendices/E-coding-guidelines.md` for test organization standards
- Inline tests for small utilities, separate files for comprehensive suites

---

## Test Commands

```bash
# Quick check
cargo test

# Full spec tests
sigil test tests/spec/

# By tier
sigil test tests/spec/types/        # Tier 1
sigil test tests/spec/traits/       # Tier 1
sigil test tests/spec/capabilities/ # Tier 2
sigil test tests/spec/patterns/     # Tier 3
sigil test tests/spec/ffi/          # Tier 4
sigil test tests/spec/async/        # Tier 6
```

---

## Current Test Results (2026-01-25)

**Rust unit tests:** 1006 passed, 0 failed

**Sigil spec tests:** 335 passed, 0 failed, 5 skipped (340 total)

| Category | Passed | Skipped | Notes |
|----------|--------|---------|-------|
| Types | 50/50 | 0 | ✅ Complete |
| Expressions | 17/17 | 0 | ✅ Complete |
| Inference | 28/28 | 0 | ✅ Complete |
| Modules | 16/16 | 0 | ✅ Complete |
| Declarations | 8/8 | 0 | ✅ Complete |
| Extensions | 4/4 | 0 | ✅ Complete |
| Capabilities | 27/27 | 0 | ✅ Complete |
| Traits | 119/119 | 0 | ✅ Complete (including map methods) |
| Patterns | 66/71 | 5 | Collection methods done; retry, validate blocked |

**Skipped tests (5 remaining):**
- `retry` function (1): in std.resilience, not implemented
- `validate` function (4): in std.validate, not implemented

**Known issue:** Parallel test runner has a thread-safety panic (index out of bounds on ExprArena). Individual test directories pass; full test run may show panic but still completes.

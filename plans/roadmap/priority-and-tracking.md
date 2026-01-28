# Priority Order & Tracking

## Current Status

### Tier 1: Foundation

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | Type System Foundation | ✅ Complete | All tests pass |
| 2 | Type Inference | ✅ Complete | All tests pass |
| 3 | Traits | ✅ Complete | All tests pass including map.len(), map.is_empty() |
| 4 | Modules | 🔶 Core complete | 16/16 tests pass; remaining: module alias, re-exports, qualified access |
| 5 | Type Declarations | ✅ Complete | Structs, sum types (multi-field variants), newtypes all work |

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
| 21 | LLVM Backend | 🔶 Partial | JIT working; 734/753 tests pass (19 skipped); destructuring support added; AOT pending |
| 22 | Tooling | ⏳ Not started | |

---

## Immediate Priority

**Current Focus: Completing Tier 1-3 partial phases**

### What's Next (Priority Order)

1. **Phase 4 (Modules)** — Advanced features remaining
   - Core imports work (relative, module, private, aliases)
   - Need: module alias (`use std.http as http`), re-exports (`pub use`), qualified access

2. **Phase 8 (Patterns)** — cache TTL remaining (NOW UNBLOCKED)
   - All compiler patterns work with stubs
   - Need: cache TTL with Duration, cache capability enforcement

3. **Phase 9 (Match)** — Guards and exhaustiveness
   - Basic pattern matching works
   - Need: `.match(guard)` syntax, exhaustiveness checking

4. **Phase 7 (Stdlib)** — retry, validate
   - Collection methods complete (map, filter, fold, find, collect, any, all)
   - Need: `retry` function, `validate` function

### Recent Completions

**Phase 6 (Capabilities)** — ✅ COMPLETED 2026-01-25
- 27/27 capability tests pass

**Phase 5.2 (Sum Types)** — ✅ COMPLETED 2026-01-28
- Unit, single-field, and multi-field variants all work
- Pattern matching on variants works
- LLVM backend support complete

**Phase 5.3 (Newtypes)** — ✅ COMPLETED 2026-01-28
- Nominal type identity (newtype != underlying type)
- Constructor pattern (`UserId("abc")`)
- `unwrap()` method to extract inner value
- Runtime transparent (same as underlying at LLVM level)

**Phase 7 (Stdlib Collection Methods)** — ✅ COMPLETED 2026-01-25
- `map`, `filter`, `fold`, `find` on lists
- `collect`, `map`, `filter`, `fold` on ranges
- `len`, `is_empty` on maps
- `any`, `all` on lists
- Rosetta code examples updated (2026-01-28)

### Approved Proposals

**`as` Conversion Syntax** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/as-conversion-proposal.md`
- Implementation: Phase 15.7
- Adds `As<T>`, `TryAs<T>` traits; removes `int()`, `float()`, `str()`, `byte()` special cases
- Blocked on: Phase 3 (needs `As<T>` and `TryAs<T>` traits in prelude)

**Default Parameter Values** — ✅ APPROVED 2026-01-28
- Proposal: `proposals/approved/default-parameters-proposal.md`
- Implementation: Phase 15.8
- Allows `param: Type = default_expr` syntax; call-time evaluation; works with named args
- Blocked on: None (can be implemented independently)

---

## Milestones

### M1: Bootstrapped (Tier 1) — 🔶 NEAR COMPLETE

- [x] Type system foundation
- [x] Type inference
- [x] **Traits** — complete including map methods
- [x] Modules (core)
- [x] Type declarations (structs, sum types, newtypes)

**Exit criteria**: Can write programs using traits and modules ✅

### M2: Capabilities & Stdlib (Tier 2) — 🔶 NEAR COMPLETE

- [x] Capabilities ✅ — 27/27 tests pass
- [x] Collection methods ✅ — map, filter, fold, find, collect, any, all
- [ ] Resilience utilities — retry, validate pending

**Exit criteria**: Capabilities working ✅, stdlib collection methods ✅, resilience utilities ❌

### M3: Core Patterns (Tier 3) — 🔶 PARTIAL

- [x] Pattern evaluation — run, try, recurse, parallel, spawn, timeout, with, for all work
- [ ] Cache pattern — TTL support pending
- [ ] Match expressions — basic works, guards/exhaustiveness pending
- [ ] Control flow — for loops work, labeled loops pending

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

- **Lexer**: Single source of truth in `ori_lexer` crate
- **Value types**: Single source in `ori_patterns::value`
- **Error messages**: Wrapper pattern for consistent errors
- **Type inference fixes**: Range types, await errors, type aliases, Self type, config lookup

See `.claude/plans/witty-nibbling-falcon.md` for details.

### Test Consolidation (Completed 2026-01-25)

Reorganized compiler tests following Go-style test scenarios:

**Files created:**
- `oric/src/eval/tests/environment_tests.rs` — 25 tests for scope/binding/capture
- `oric/src/eval/tests/unary_operators_tests.rs` — 40 tests for negation, not, bitwise, try
- `oric/src/eval/tests/expr_tests.rs` — 55 tests for literals, indexing, collection ops
- `oric/src/eval/tests/control_tests.rs` — 28 tests for if/else, pattern binding, loops
- `oric/src/eval/tests/call_tests.rs` — 20 tests for function calls, parameter binding

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
# Run ALL tests (Rust + Ori interpreter + LLVM backend)
./test-all

# Individual test suites
cargo t                               # Rust unit tests
cargo st                              # Ori language tests (interpreter)
./llvm-test                           # LLVM Rust unit tests
./docker/llvm/run.sh ori test tests/  # Ori language tests (LLVM)

# By category
cargo st tests/spec/types/        # Tier 1
cargo st tests/spec/traits/       # Tier 1
cargo st tests/spec/capabilities/ # Tier 2
cargo st tests/spec/patterns/     # Tier 3
```

---

## Current Test Results (2026-01-28)

**Rust unit tests:** 1006 passed, 0 failed

**Ori spec tests:** 901 passed, 0 failed, 19 skipped (920 total)

| Category | Passed | Skipped | Notes |
|----------|--------|---------|-------|
| Types | 70/70 | 0 | ✅ Complete (sum types, multi-field variants, newtypes) |
| Expressions | 17/17 | 0 | ✅ Complete |
| Inference | 28/28 | 0 | ✅ Complete |
| Modules | 16/16 | 0 | ✅ Complete |
| Declarations | 8/8 | 0 | ✅ Complete |
| Extensions | 4/4 | 0 | ✅ Complete |
| Capabilities | 27/27 | 0 | ✅ Complete |
| Traits | 119/119 | 0 | ✅ Complete (including map methods) |
| Patterns | 71/76 | 5 | Collection methods done; retry, validate blocked |
| Rosetta | 96/110 | 14 | Stack/queue/string-slice blocked |

**Skipped tests (19 remaining):**
- `retry` function (1): in std.resilience, not implemented
- `validate` function (4): in std.validate, not implemented
- Stack methods (6): push, pop, peek, is_empty, size, clear
- Queue methods (6): enqueue, dequeue, peek, is_empty, size, clear
- String slice (2): string slicing syntax not implemented

**Known issue:** Parallel test runner has a thread-safety panic (index out of bounds on ExprArena). Individual test directories pass; full test run may show panic but still completes.

---

## Draft Proposals Pending Review (2026-01-27)

New prelude enhancements from Rust prelude comparison. See `plan.md` for details.

| Proposal | File | Affects Phases |
|----------|------|----------------|
| `as` Conversion Syntax | `proposals/drafts/as-conversion-proposal.md` | 7, 15 |
| Iterator Traits | `proposals/drafts/iterator-traits-proposal.md` | 3, 7 |
| Debug Trait | `proposals/drafts/debug-trait-proposal.md` | 3, 7 |
| Developer Functions | `proposals/drafts/developer-functions-proposal.md` | 7 |

**Decisions made (no proposal needed):**
- NaN comparisons panic (Phase 7.18)
- Skip `AsRef`/`AsMut` — Ori's value semantics don't need them
- Skip `debug_assert*` — same behavior in all builds

---

## LLVM Backend Status (Phase 21) — Updated 2026-01-28

### Test Results

| Test Suite | Passed | Failed | Skipped | Total |
|------------|--------|--------|---------|-------|
| All Ori tests | 734 | 0 | 19 | 753 |
| Rust unit tests | 204 | 0 | 0 | 204 |

### Architecture (Reorganized)

The LLVM backend follows Rust's `rustc_codegen_llvm` patterns:

| Component | File | Status |
|-----------|------|--------|
| SimpleCx | `context.rs` | ✅ Complete |
| CodegenCx | `context.rs` | ✅ Complete |
| Builder | `builder.rs` | ✅ Complete |
| TypeCache | `context.rs` | ✅ Complete |
| Two-phase codegen | `module.rs` | ✅ Complete |
| Trait abstraction | `traits.rs` | ✅ Complete |

### Completed Features

- [x] Context hierarchy (SimpleCx → CodegenCx)
- [x] Separate Builder type for instruction generation
- [x] Two-phase codegen (declare then define)
- [x] Type caching (scalars + complex types)
- [x] Trait-based abstraction (BackendTypes, BuilderMethods)
- [x] JIT execution via inkwell
- [x] Runtime functions (print, panic, assert, collections)
- [x] Expression codegen (literals, binary ops, unary ops)
- [x] Function codegen (signatures, locals, returns)
- [x] Control flow (if/else, loops, break/continue)
- [x] Pattern matching (match expressions, guards)
- [x] Collections (lists, tuples, structs, Option, Result)
- [x] Generic function support (type variable resolution)

### Pending Features

- [ ] AOT compilation (object file generation)
- [ ] Optimization passes (O1, O2, O3)
- [ ] Debug info (DWARF)
- [ ] Executable linking

### Running LLVM Tests

```bash
# All Ori tests via LLVM backend
./docker/llvm/run.sh ori test

# Spec tests only
./docker/llvm/run.sh ori test tests/spec

# Rust unit tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib
```

**Note:** LLVM development requires Docker. See `.claude/rules/llvm.md` for details.

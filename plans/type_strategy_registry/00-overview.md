---
plan: "type_strategy_registry"
title: "Type Strategy Registry: Pure-Data Behavioral Contract for All Compiler Phases"
status: not-started
supersedes:
  - "plans/builtin_ownership_ssot/"
---

# Type Strategy Registry: Pure-Data Behavioral Contract for All Compiler Phases

## Mission

Eliminate cross-phase drift permanently by creating a single, pure-data crate (`ori_registry`) that declares the complete behavioral specification of every builtin type — methods, operators, ownership, memory strategy — as `const` data that all compiler phases consume. No phase hard-codes type knowledge independently. One declaration per fact, one source of truth, structural enforcement via Rust's type system.

## Motivation

### The Problem

Every phase of the Ori compiler independently encodes knowledge about builtin types:

- **ori_types** (`infer/expr/methods.rs`): 380 entries in `TYPECK_BUILTIN_METHODS`, 20 type-specific `resolve_*_method()` functions with hard-coded return types
- **ori_eval** (`methods/helpers/mod.rs`): 193-entry `EVAL_BUILTIN_METHODS` array, `BuiltinMethodNames` struct (74 interned fields), 24-entry `ITERATOR_METHOD_NAMES`
- **ori_ir** (`builtin_methods/mod.rs`): 121 entries in `BUILTIN_METHODS` with `MethodDef` structs
- **ori_llvm** (`codegen/arc_emitter/builtins/`): 163 entries across 7 submodules via `declare_builtins!` macro, `BuiltinRegistration` with `receiver_borrowed`
- **ori_arc** (`borrow/mod.rs`): `borrowing_builtins` parameter injected via `oric` from ori_llvm's `borrowing_builtin_names()`
- **Consistency tests** (`oric/src/eval/tests/methods/consistency.rs`): 506 entries across 6 allowlists tracking intentional gaps

These are all **different projections of the same underlying facts** about the same types. When one changes, the others must be manually updated. When someone forgets, bugs appear silently — like the string ordering bug where `<`, `>`, `<=`, `>=` had no `is_str` guards in `emit_binary_op`, or the `Idx::ERROR` propagation bug where missing method return types in the type checker caused phantom types to reach LLVM codegen.

### The Solution

A dedicated crate (`ori_registry`) at the bottom of the dependency graph that:

1. **Declares** every builtin type's complete behavioral specification as `const` data
2. **Has zero dependencies** on any compiler crate (pure data, no behavior)
3. **Is consumed** by every phase (type checker, evaluator, ARC pass, LLVM backend)
4. **Uses Rust's type system** for enforcement — adding a field to `TypeDef` is a compile error in every phase that doesn't handle it
5. **Eliminates** all parallel allowlists, gap tracking, and manual sync work

### Prior Art & What's Different

Several production compilers centralize builtin *operations* into shared registries consumed by multiple phases:

- **Swift** (`Builtins.def`): X-macro file declaring ~100 builtin operations (arithmetic, casts, memory ops) consumed by 8+ compiler phases (AST, SILGen, IRGen, ConstantFolding). The gold standard for multi-phase operation registries. But it covers *intrinsic operations*, not *type methods* — `String.count`, `Array.append` etc. live in the Swift stdlib and are discovered via normal inherent impl resolution.
- **Zig** (`Zcu.BuiltinDecl` + `BuiltinFn.list`): Central enums declaring ~130 builtin functions (`@memcpy`, `@addWithOverflow`) consumed by both Sema and codegen. Memoized via `EnumArray` for O(1) lookup. Again covers *builtin functions*, not per-type method signatures.
- **Roc** (`LowLevel` enum): ~140-variant enum serving as the API contract between type checker (Can) and LLVM codegen. Both phases dispatch on the same enum. Covers low-level operations, not method-level detail.

What **none** of these do is centralize **per-type method signatures with return types, receiver ownership, and operator codegen strategy**. That's the gap Ori fills:

- Swift knows "add is a binary integer op" but doesn't declare "str.length() returns Int, borrows receiver, has no operator"
- Zig knows "@memcpy takes 2 params" but doesn't declare "str has method X with ownership Y consumed by ARC pass Z"
- Roc knows "StrConcat is a low-level op" but doesn't attach memory strategy or receiver semantics

The remaining compilers (Rust, Go, TypeScript, Gleam, Elm, Koka, Lean 4) distribute builtin knowledge across phases without centralization. This is fine for languages where primitives have few methods (Go), use traits for everything (Rust), or define builtins in the source language itself (TypeScript via lib.d.ts, Lean via Prelude.lean).

Ori is uniquely positioned: rich builtin methods on primitive types (380+ type-checker entries), ARC ownership semantics per method, dual backends (interpreter + LLVM), and a small enough team that registry drift is immediately painful.

## Architecture

```
ori_registry (pure data, zero deps)
  │
  │  const BUILTIN_TYPES: &[&TypeDef]
  │  ├── INT:   methods=[f, byte, abs, to_str], ops=IntInstr, memory=Copy
  │  ├── FLOAT: methods=[floor, ceil, round, abs, to_str], ops=FloatInstr, memory=Copy
  │  ├── STR:   methods=[length, concat, ...], ops=RuntimeCall, memory=Arc
  │  ├── BOOL:  methods=[to_str], ops=BoolInstr, memory=Copy
  │  ├── BYTE:  methods=[to_int, to_char, to_str], ops=UnsignedCmp, memory=Copy
  │  └── CHAR:  methods=[to_int, to_str, is_alpha, ...], ops=UnsignedCmp, memory=Copy
  │
  ├──→ ori_types:  reads returns, validates operators
  ├──→ ori_eval:   reads method existence, validates dispatch coverage
  ├──→ ori_arc:    reads ownership, reads memory strategy
  ├──→ ori_llvm:   reads OpStrategy for emit_binary_op, reads ownership
  └──→ ori_ir:     consolidates existing MethodDef (migration)
  (TypeFlow removed from scope — type inference for higher-order methods stays in ori_types)
```

### Dependency Position

```
LAYER 0 (Foundation)
├── ori_ir           (AST, IR types, spans)
└── ori_registry     (pure behavioral data)  ← NEW

LAYER 1-2 (unchanged)
├── ori_diagnostic   → ori_ir
├── ori_lexer        → ori_ir
└── ori_parse        → ori_ir, ori_diagnostic

LAYER 3 (Type & pattern system)
├── ori_types        → ori_ir, ori_registry, ori_diagnostic  ← adds dep
└── ori_patterns     → ori_ir, ori_diagnostic

LAYER 4 (ARC)
└── ori_arc          → ori_ir, ori_registry, ori_types  ← adds dep

LAYER 5+ (downstream)
├── ori_eval         → ori_ir, ori_registry, ori_patterns  ← adds dep
└── ori_llvm         → ori_ir, ori_registry, ori_arc, ori_types  ← adds dep
```

**No cycles possible.** ori_registry depends on nothing. Everything depends on it. Same pattern as ori_ir.

### Purity Contract

The `ori_registry` crate MUST maintain these invariants permanently:

1. **No `[dependencies]`** in Cargo.toml (only `[dev-dependencies]` for tests)
2. **No functions with logic** — only `const fn` constructors and simple lookups
3. **No trait impls with behavior** — only `derive(Clone, Copy, Debug, PartialEq, Eq, Hash)`
4. **No `unsafe`** — no reason for it
5. **All data `const`-constructible** — baked into the binary's `.rodata` segment
6. **No IO, no allocation, no side effects** — pure data definitions

### What Moves Where

| Current Location | Current Form | Registry Form | Phase Reads |
|---|---|---|---|
| `ori_types/methods.rs` resolve_str_method match arms | `"length" => Some(Idx::INT)` | `MethodDef { returns: TypeTag::Int }` | `find_method(Str, "length").returns` |
| `ori_types/methods.rs` TYPECK_BUILTIN_METHODS | `[("str", "length"), ...]` | `STR.methods` iterator | `BUILTIN_TYPES.methods` enumeration |
| `ori_llvm/codegen/arc_emitter/mod.rs` emit_binary_op is_str guards | `if self.is_str(lhs) { emit_str_cmp }` | `STR.operators.cmp = RuntimeCall { "ori_str_compare" }` | `find_type(ty).operators.cmp` → match strategy |
| `ori_llvm/codegen/arc_emitter/builtins/` receiver_borrowed | `("str", "length", borrow: true)` | `MethodDef { receiver: Ownership::Borrow }` | `find_method(Str, "length").receiver` |
| `ori_ir/builtin_methods/` BUILTIN_METHODS | `MethodDef { receiver_borrows: true, ... }` | `MethodDef { receiver: Ownership::Borrow }` | `find_method(ty, name).receiver` |
| `ori_arc/borrow/` borrowing_builtins | `FxHashSet<Name>` injected via oric | `BUILTIN_TYPES.methods.filter(borrow)` | `find_method(ty, name).receiver == Borrow` |
| `consistency.rs` TYPECK_METHODS_NOT_IN_IR (142 entries) | Allowlist tracking gaps | **Eliminated** | Registry IS the source of truth |
| `consistency.rs` EVAL_METHODS_NOT_IN_TYPECK (62 entries) | Allowlist tracking gaps | **Eliminated** | Registry IS the source of truth |

## Section Dependency Graph

```
  01 Core Data Model ────────┐
  02 Crate Scaffolding ──────┤
                              ├──→ 03 Primitive Types ──────────┐
                              ├──→ 04 String Type ──────────────┤
                              ├──→ 05 Compound Types ───────────┤
                              ├──→ 06 Collection/Wrapper Types ─┤
                              ├──→ 07 Iterator Types ───────────┤
                              │                                  │
                              ├──→ 08 Query API ─────────────────┤
                              │                                  │
                              │    ┌─────────────────────────────┘
                              │    │  (all type defs + query API complete)
                              │    │
                              │    ├──→ 09 Wire Type Checker ────┐
                              │    ├──→ 10 Wire Evaluator ───────┤
                              │    ├──→ 11 Wire ARC/Borrow ──────┤
                              │    ├──→ 12 Wire LLVM Backend ────┤
                              │    └──→ 13 Migrate ori_ir ───────┤
                              │                                  │
                              │    ┌─────────────────────────────┘
                              │    │  (all wiring complete)
                              │    │
                              └────┴──→ 14 Enforcement & Exit ──→ DONE
```

**Phase gates:**
- **Gate 1:** Sections 01-02 complete → type definitions can begin (03-07)
- **Gate 2:** Sections 03-08 complete → wiring can begin (09-13)
- **Gate 3:** Sections 09-13 complete → enforcement tests and legacy removal (14)

**Parallelism within gates:**
- Sections 03-07 are fully independent (different types, different files)
- Sections 09-13 are largely independent (different crates)

## Implementation Sequence

```
Phase 1 ─ Foundation
  ├─ 01: Design and finalize all data model types (TypeTag, OpStrategy, TypeDef, etc.)
  └─ 02: Create ori_registry crate, Cargo.toml, module structure, purity tests

Phase 2 ─ Type Definitions (parallelizable)
  ├─ 03: Primitive types (int, float, bool, byte, char)
  ├─ 04: String type (str — complex, many methods, RuntimeCall operators)
  ├─ 05: Compound types (Duration, Size, Ordering, Error, Channel)
  ├─ 06: Collection & wrapper types (List, Map, Set, Range, Tuple, Option, Result)
  ├─ 07: Iterator types (Iterator, DoubleEndedIterator)
  └─ 08: Query API (BUILTIN_TYPES, find_type, find_method, helpers)
  Gate: cargo c -p ori_registry passes, all types defined, query API works

Phase 3 ─ Wiring (parallelizable per crate)
  ├─ 09: Wire ori_types — replace resolve_*_method, TYPECK_BUILTIN_METHODS
  ├─ 10: Wire ori_eval — replace EVAL_BUILTIN_METHODS, dispatch tables
  ├─ 11: Wire ori_arc — replace borrowing_builtins, fix dependency direction
  ├─ 12: Wire ori_llvm — replace emit_binary_op guards, simplify BuiltinRegistration
  └─ 13: Migrate ori_ir — consolidate BUILTIN_METHODS, DerivedTrait, format specs
  Gate: ./test-all.sh passes, ./llvm-test.sh passes

Phase 4 ─ Enforcement & Exit
  └─ 14: Enforcement tests, testing matrix, allowlist elimination, legacy removal
  Gate: All enforcement tests pass, all allowlists eliminated, grep verification clean
```

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Core Data Model | ~120 | Low | — |
| 02 Crate Scaffolding | ~80 | Low | 01 |
| 03 Primitive Types | ~200 | Low | 01, 02 |
| 04 String Type | ~150 | Medium | 01, 02 |
| 05 Compound Types | ~250 | Medium | 01, 02 |
| 06 Collection & Wrapper Types | ~300 | Medium | 01, 02 |
| 07 Iterator Types | ~200 | Medium-High | 01, 02 |
| 08 Query API | ~60 | Low | 01, 02 |
| 09 Wire Type Checker | ~-300 (net deletion) | Medium-High | 03-08 |
| 10 Wire Evaluator | ~-200 (net deletion) | Medium | 03-08 |
| 11 Wire ARC/Borrow | ~-50 (net deletion) | Low-Medium | 03-08 |
| 12 Wire LLVM Backend | ~-150 (net deletion) | Medium | 03-08 |
| 13 Migrate ori_ir | ~-400 (net deletion) | Medium | 03-08 |
| 14 Enforcement & Exit | ~200 | Medium | 09-13 |
| **Total new (ori_registry)** | **~1,360** | | |
| **Total deleted (legacy)** | **~-1,100** | | |
| **Net change** | **~+260** | | |

## Sync Points Eliminated

This plan eliminates ALL of the following manual sync mechanisms:

| Sync Mechanism | Entries | Location | Replaced By |
|---|---|---|---|
| `TYPECK_BUILTIN_METHODS` | 380 | `ori_types/infer/expr/methods.rs` | `BUILTIN_TYPES` enumeration |
| `resolve_str_method()` + 19 siblings | ~445 lines | `ori_types/infer/expr/methods.rs` | `find_method(tag, name).returns` |
| `EVAL_BUILTIN_METHODS` | 193 | `ori_eval/methods/helpers/mod.rs` | `BUILTIN_TYPES` enumeration |
| `ITERATOR_METHOD_NAMES` | 24 | `ori_eval/interpreter/resolvers/mod.rs` | `find_type(Iterator).methods` |
| `DEI_ONLY_METHODS` | 5 | `ori_types/infer/expr/methods.rs` | Registry-based DEI flag |
| `BUILTIN_METHODS` (ori_ir) | 121 | `ori_ir/builtin_methods/mod.rs` | Consolidated into ori_registry |
| `BuiltinRegistration.receiver_borrowed` | 163 | `ori_llvm/codegen/arc_emitter/builtins/*.rs` | `find_method().receiver` |
| `borrowing_builtin_names()` | ~21 lines | `ori_llvm/codegen/arc_emitter/builtins/mod.rs` | `find_method().receiver == Borrow` |
| `TYPECK_METHODS_NOT_IN_IR` | 142 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `EVAL_METHODS_NOT_IN_TYPECK` | 62 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `TYPECK_METHODS_NOT_IN_EVAL` | 259 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `EVAL_METHODS_NOT_IN_IR` | 22 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` | 10 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `COLLECTION_TYPES` | 11 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| Hard-coded is_str/is_float guards | ~19 | `ori_llvm/codegen/arc_emitter/mod.rs` | OpStrategy dispatch |
| Hard-coded method names in calls.rs | ~25 lines | `ori_types/infer/expr/calls.rs` | Stays in type checker (inference logic, not registry data) |
| **Total eliminated** | **~1,902** | | |

## Structural Guarantees (Post-Implementation)

After this plan is complete, the following become structurally impossible:

1. **Method exists in one phase but not another** — one declaration, all phases read it
2. **Return type disagrees between phases** — one `returns: TypeTag` field
3. **Ownership semantics disagree** — one `receiver: Ownership` field
4. **Operator codegen missing for a type** — `OpStrategy` per operator per type; enforcement test verifies backend handles all non-`Unsupported` strategies
5. **New type added without full phase coverage** — `_enforce_exhaustiveness()` dead functions (Roc pattern) in all 4 consuming crates produce compile errors when a new `TypeTag` variant is added
6. **New method added without full phase coverage** — enforcement test iterates registry, checks every handler exists
7. **Backend-required method missing from a backend** — `backend_required: true` on `MethodDef` + enforcement test verifies both eval and llvm handle it (inspired by Rust's `must_be_overridden`)
8. **Side-effect assumptions disagree** — `pure: bool` on `MethodDef` provides a single source of truth for the optimizer and effect system (inspired by Swift's readnone bit)

## Prior Art Refinements (February 2026 review)

A thorough study of 6 reference compilers (Swift, Zig, Roc, Rust, Go, Lean4) surfaced these refinements, now incorporated into the plan:

| Refinement | Source | Section |
|-----------|--------|---------|
| `pure: bool` side-effect flag | Swift `Builtins.def` readnone, Zig `eval_to_error` | 01 (MethodDef) |
| `backend_required: bool` | Rust `IntrinsicDef.must_be_overridden` | 01 (MethodDef), 14 (enforcement) |
| `_enforce_exhaustiveness` dead fns | Roc `low_level.rs` + `can/builtins.rs` | 14 (compile-time guards) |
| No mutation strategy on MethodDef | Roc `UpdateMode` is per-call-site, not per-method | 01 (design decision) |
| TypeFlow removed from scope | All 6 compilers keep inference logic in the type checker, not the registry | 01, 07, 09 |

## What This Plan Does NOT Cover

- **User-defined types** — these go through trait dispatch, not the builtin registry
- **Trait system architecture** — the registry covers builtin type *behavior*, not the trait system itself
- **Optimization passes** — RC elision, dead code elimination, etc. are independent concerns
- **Parser/lexer changes** — no syntax changes required
- **Runtime library changes** — ori_rt function signatures are unchanged; only the compiler's knowledge of them is centralized

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Core Data Model Design | `section-01-core-data-model.md` | Not Started |
| 02 | Crate Scaffolding & Purity Enforcement | `section-02-crate-scaffolding.md` | Not Started |
| 03 | Primitive Type Definitions | `section-03-primitive-types.md` | Not Started |
| 04 | String Type Definition | `section-04-string-type.md` | Not Started |
| 05 | Compound Type Definitions | `section-05-compound-types.md` | Not Started |
| 06 | Collection & Wrapper Types | `section-06-collection-wrapper-types.md` | Not Started |
| 07 | Iterator Type Definitions | `section-07-iterator-types.md` | Not Started |
| 08 | Query API & Lookup Functions | `section-08-query-api.md` | Not Started |
| 09 | Wire Type Checker (ori_types) | `section-09-wire-type-checker.md` | Not Started |
| 10 | Wire Evaluator (ori_eval) | `section-10-wire-evaluator.md` | Not Started |
| 11 | Wire ARC & Borrow Pass (ori_arc) | `section-11-wire-arc-borrow.md` | Not Started |
| 12 | Wire LLVM Backend (ori_llvm) | `section-12-wire-llvm-backend.md` | Not Started |
| 13 | Migrate ori_ir & Legacy Consolidation | `section-13-migrate-ori-ir.md` | Not Started |
| 14 | Enforcement Tests, Testing Matrix & Exit Criteria | `section-14-enforcement-testing.md` | Not Started |

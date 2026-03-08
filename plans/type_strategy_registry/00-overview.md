---
plan: "type_strategy_registry"
title: "Type Strategy Registry: Pure-Data Behavioral Contract for All Compiler Phases"
status: in-progress
reviewed: false
supersedes:
  - "plans/builtin_ownership_ssot/"
---

# Type Strategy Registry: Pure-Data Behavioral Contract for All Compiler Phases

## Mission

Eliminate cross-phase drift permanently by creating a single, pure-data crate (`ori_registry`) that declares the complete behavioral specification of every builtin type — methods, operators, ownership, memory strategy — as `const` data that all compiler phases consume. No phase hard-codes type knowledge independently. One declaration per fact, one source of truth, structural enforcement via Rust's type system.

## Motivation

### The Problem

Every phase of the Ori compiler independently encodes knowledge about builtin types:

- **ori_types** (`infer/expr/methods/mod.rs` + `resolve_by_type.rs`): 390 entries in `TYPECK_BUILTIN_METHODS`, 20 type-specific `resolve_*_method()` functions (in `resolve_by_type.rs`) with hard-coded return types
- **ori_eval** (`methods/helpers/mod.rs`): 230-entry `EVAL_BUILTIN_METHODS` array, `BuiltinMethodNames` struct (97 interned fields), 24-entry `ITERATOR_METHOD_NAMES`
- **ori_ir** (`builtin_methods/mod.rs`): 123 entries in `BUILTIN_METHODS` with `MethodDef` structs
- **ori_llvm** (`codegen/arc_emitter/builtins/`): 206 entries across 7 submodules via `declare_builtins!` macro, `BuiltinRegistration` with `receiver_borrowed`
- **ori_arc** (`borrow/builtins/mod.rs`): `borrowing_builtin_names()` builds `FxHashSet<Name>` from `BORROWING_METHOD_NAMES` string list
- **Consistency tests** (`oric/src/eval/tests/methods/consistency.rs`): 448 entries across 6 allowlists tracking intentional gaps

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

Ori is uniquely positioned: rich builtin methods on primitive types (390 type-checker entries), ARC ownership semantics per method, dual backends (interpreter + LLVM), and a small enough team that registry drift is immediately painful.

## Architecture

```
ori_registry (pure data, zero deps)
  │
  │  const BUILTIN_TYPES: &[&TypeDef]
  │  ├── INT:   methods=[f, byte, abs, to_str], ops=IntInstr, memory=Copy
  │  ├── FLOAT: methods=[floor, ceil, round, abs, to_str], ops=FloatInstr, memory=Copy
  │  ├── STR:   methods=[length, concat, ...], ops=RuntimeCall, memory=Arc
  │  ├── BOOL:  methods=[to_str], ops=BoolLogic, memory=Copy
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
| `ori_types/infer/expr/methods/resolve_by_type.rs` resolve_str_method match arms | `"length" => Some(Idx::INT)` | `MethodDef { returns: ReturnTag::Concrete(TypeTag::Int) }` | `find_method(Str, "length").returns` |
| `ori_types/infer/expr/methods/mod.rs` TYPECK_BUILTIN_METHODS | `[("str", "length"), ...]` | `STR.methods` iterator | `BUILTIN_TYPES.methods` enumeration |
| `ori_llvm/codegen/arc_emitter/operators.rs` emit_binary_op is_str guards | `if is_str { emit_str_runtime_call(...) }` | `STR.operators.lt = RuntimeCall { fn_name: "ori_str_compare", returns_bool: true }` (each comparison field independently) | `find_type(ty).operators.lt` → match strategy |
| `ori_llvm/codegen/arc_emitter/builtins/` receiver_borrowed | `("str", "length", borrow: true)` | `MethodDef { receiver: Ownership::Borrow }` | `find_method(Str, "length").receiver` |
| `ori_ir/builtin_methods/` BUILTIN_METHODS | `MethodDef { receiver_borrows: true, ... }` | `MethodDef { receiver: Ownership::Borrow }` | `find_method(ty, name).receiver` |
| `ori_arc/borrow/builtins/` BORROWING_METHOD_NAMES | `FxHashSet<Name>` built from const `&[&str]` | `ori_registry::borrowing_method_names()` (derived from `BUILTIN_TYPES`) | `borrowing_method_names().iter()` + intern |
| `consistency.rs` TYPECK_METHODS_NOT_IN_IR (130 entries) | Allowlist tracking gaps | **Eliminated** | Registry IS the source of truth |
| `consistency.rs` EVAL_METHODS_NOT_IN_TYPECK (55 entries) | Allowlist tracking gaps | **Eliminated** | Registry IS the source of truth |

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
                              │    ├──→ 09 Wire Type Checker ─────┐
                              │    │         │                     │
                              │    │         ├──→ 11 Wire ARC ─────┤
                              │    │         │         │           │
                              │    │         └─────────┴──→ 12 Wire LLVM
                              │    │                               │
                              │    ├──→ 10 Wire Evaluator ─────────┤  (independent)
                              │    └──→ 13 Migrate ori_ir ─────────┤  (independent)
                              │                                    │
                              │    ┌───────────────────────────────┘
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
- Sections 09-13 have constrained parallelism: ori_eval (10) and ori_ir (13) are independent; ori_arc (11) must follow ori_types (09); ori_llvm (12) must follow ori_types (09) and ori_arc (11)

## Schema Freeze (Pre-Implementation Checkpoint)

Before any type definition (Sections 03-07) or wiring (Sections 09-13) begins, the following schema decisions are **frozen**. All sections MUST use these exact names, field sets, and variant lists. Contradictions between sections are bugs in the plan, not intentional variation.

### Frozen Decisions

1. **TypeTag**: 23 concrete builtin type variants. **No `SelfType`, `Fresh`, or `Void`** — these are signature-level concepts on `ReturnTag` only.

2. **ReturnTag**: 22-variant enum for method signature type positions. Concrete: `Concrete(TypeTag)`, `SelfType`, `Unit`, `Fresh`. Direct projections: `ElementType`, `KeyType`, `ValueType`, `OkType`, `ErrType`. Fixed wrappers: `List(TypeTag)`, `Option(TypeTag)`, `DoubleEndedIterator(TypeTag)`. Projection wrappers: `OptionOf(TypeProjection)`, `ListOf(TypeProjection)`, `IteratorOf(TypeProjection)`, `DoubleEndedIteratorOf(TypeProjection)`. Composite: `ListKeyValue`, `ListOfTupleIntElement`, `MapIterator`, `IteratorOfTupleIntElement`. Protocol: `NextResult` for `(Option<T>, Self)`, `ResultOfProjectionFresh(TypeProjection)` for `Result<T, E>` with fresh E. Convenience `From<TypeTag> for ReturnTag` wraps concrete types. **Canonical name is `ReturnTag`** -- not `ReturnType`.

3. **OpDefs**: 20 fields (expanded schema). Arithmetic: `add`, `sub`, `mul`, `div`, `rem`, `floor_div`. Comparison: `eq`, `neq`, `lt`, `gt`, `lt_eq`, `gt_eq`. Unary: `neg`, `not`. Bitwise: `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `shl`, `shr`. **No compact `cmp` field** — each comparison operator is independent.

4. **OpStrategy**: 6 variants: `IntInstr`, `FloatInstr`, `UnsignedCmp`, `BoolLogic`, `RuntimeCall { fn_name, returns_bool }`, `Unsupported`. **Canonical name is `BoolLogic`** — not `BoolInstr`.

5. **Ownership**: 3 variants: `Borrow`, `Owned`, `Copy`. `Copy` distinguishes value-type receivers from reference-type borrows.

6. **Error type**: Included in `BUILTIN_TYPES` — it has 8 methods. Not excluded as "no methods."

7. **No silent fallbacks**: `Idx::ERROR` and `const_i64(0)` are **banned** as compatibility fallbacks in core paths. Unreachable code paths use `unreachable!()` or `ice!()`. If a code path is reachable, implement it.

8. **TypeParamArity**: `TypeDef` includes `type_params: TypeParamArity` — `Fixed(0)` for primitives, `Fixed(1)` for `List<T>`/`Option<T>`/etc., `Fixed(2)` for `Map<K,V>`/`Result<T,E>`, `Variadic` for tuples.

9. **MethodKind**: `MethodDef` includes `kind: MethodKind` — `Instance` (default) or `Associated` (for associated functions like `Duration.from_seconds()`). Needed by Sections 04/05 for str, Duration, and Size associated functions.

10. **TypeProjection**: `TypeProjection` enum (`Element | Key | Value | Ok | Err | Fixed(TypeTag)`) is part of the core model. Used by parameterized `ReturnTag` variants (`OptionOf`, `ListOf`, `IteratorOf`, `DoubleEndedIteratorOf`) to express generic return types relative to receiver type parameters.

11. **DeiPropagation**: `MethodDef` includes `dei_propagation: DeiPropagation` — `Propagate` (adapter preserves DEI), `Downgrade` (adapter drops DEI), `NotApplicable` (consumer or non-iterator method). Defined in Section 01.8c.

12. **dei_only**: `MethodDef` includes `dei_only: bool` — `true` for methods only available on `DoubleEndedIterator` (`next_back`, `rev`, `last`, `rfind`, `rfold`), `false` for all other methods. Combined with frozen decision 11, this enables the single-TypeDef iterator model (Section 07 Decision 1).

13. **MethodDef full field list**: `name`, `receiver`, `params`, `returns`, `trait_name`, `pure`, `backend_required`, `kind`, `dei_only`, `dei_propagation`. All 10 fields are required. Sections 03-07 MUST include all fields in every `MethodDef` literal (or use a documented abbreviation comment).

14. **`TypeTag::base_type()`**: `DoubleEndedIterator.base_type()` returns `Iterator`; all other variants return `self`. This is the DEI aliasing mechanism used by the query API (Section 08) to look up the single Iterator `TypeDef` for both `TypeTag::Iterator` and `TypeTag::DoubleEndedIterator`. Defined in Section 01.1.

15. **Function type**: `TypeTag::Function` is in the registry with `methods: &[]` (no methods), `operators: OpDefs::UNSUPPORTED`, `memory: MemoryStrategy::Arc`, `type_params: TypeParamArity::Variadic`. Its value is memory classification, not method registration.

16. **Operator exclusions**: `pow`/`**` (desugared to `Pow.power()` trait method before IR), `matmul`/`@` (always trait-dispatched via `BinaryOp::MatMul`), `as`/`as?` (`Expr::Cast`, not an operator), `&&`/`||` (short-circuit control flow, subsumed by `BoolLogic` on `bit_and`/`bit_or`), `..`/`..=`/`??`/`?` (desugared) — none of these have `OpDefs` fields. Documented in Section 01.7 design decisions 5-10.

17. **Purity semantics**: `pure: true` means "no observable side effects (no IO, no mutation, no global state) but MAY panic on invalid input." Matches Swift's `readnone` and Lean's model. The optimizer MAY reorder, CSE, and hoist pure calls, but MUST NOT eliminate them if reachable (panic must fire). All primitive methods are `pure: true`. A future `may_panic` flag can be added as a separate field without changing `pure` semantics.

18. **Primitive receiver ownership**: All primitive type method receivers use `Ownership::Borrow`, not `Ownership::Copy`. The `Copy` variant exists but is reserved for future use. Rationale: the ARC pass checks `receiver == Borrow` to skip RC ops; using `Copy` would require updating every check to `== Borrow || == Copy`. `Borrow` is semantically correct (the receiver IS borrowed; it happens to be trivially copyable). See Section 03 design decisions.

## Incremental Execution Principles

The full implementation is the goal. The path to it is incremental, with each step production-safe.

1. **Behavior Parity Rule**: Migration steps may move data location but MUST NOT change semantics. A registry-driven code path must produce identical results to the legacy code path it replaces. Verify with `./test-all.sh` after every wiring step.

2. **Vertical Slices**: Type definitions (Sections 03-07) and wiring (Sections 09-13) can be landed in slices (e.g., primitives first, then compound types, then collections/iterators). Each slice must leave the build green.

3. **Consumer Sequence**: Wire consumers in order: ori_types → ori_eval → ori_arc → ori_llvm → ori_ir. Earlier consumers establish the pattern; later consumers follow it. **Dependency constraint**: ori_arc (11) MUST follow ori_types (09) because `ori_arc` imports from `ori_types`. ori_llvm (12) MUST follow both ori_types (09) and ori_arc (11). ori_eval (10) and ori_ir (13) are independent of the others.

4. **Temporary Adapters**: Short-lived bridge code is allowed during migration (e.g., `tag_to_type_tag()` bridge functions). Each adapter has an expiry: "remove when Section X is complete." No adapter survives past Section 14.

5. **Progressive Enforcement**: Section 14 tests start as warning/allowlist-backed for migrated types only, then ratchet to full strictness. Allowlists shrink monotonically and are deleted at completion.

6. **Done Definition**: All phases reading from `ori_registry`, all legacy tables deleted, all allowlists deleted, all enforcement tests green, `./test-all.sh` and `./llvm-test.sh` passing.

## Implementation Sequence

```
Phase 1 ─ Foundation
  ├─ 01: Design and finalize all data model types (TypeTag, OpStrategy, TypeDef, etc.)
  └─ 02: Create ori_registry crate, Cargo.toml, module structure, purity tests

Phase 2 ─ Type Definitions (parallelizable)
  ├─ 03: Primitive types (int, float, bool, byte, char)
  ├─ 04: String type (str — complex, many methods, RuntimeCall operators)
  ├─ 05: Compound types (Duration, Size, Ordering, Error)
  ├─ 06: Collection & wrapper types (List, Map, Set, Range, Tuple, Option, Result, Channel)
  ├─ 07: Iterator types (Iterator, DoubleEndedIterator)
  └─ 08: Query API (BUILTIN_TYPES, find_type, find_method, helpers)
  Gate: cargo c -p ori_registry passes, all types defined, query API works

Phase 3 ─ Wiring (parallelizable per crate)
  ├─ 09: Wire ori_types — replace resolve_*_method, TYPECK_BUILTIN_METHODS
  ├─ 10: Wire ori_eval — replace EVAL_BUILTIN_METHODS, ITERATOR_METHOD_NAMES, rewrite consistency tests
  ├─ 11: Wire ori_arc — replace BORROWING_METHOD_NAMES with registry data
  ├─ 12: Wire ori_llvm — replace emit_binary_op guards, simplify BuiltinRegistration
  └─ 13: Migrate ori_ir — consolidate BUILTIN_METHODS, DerivedTrait, format specs
  Gate: ./test-all.sh passes, ./llvm-test.sh passes

Phase 4 ─ Enforcement & Exit
  └─ 14: Enforcement tests, testing matrix, allowlist elimination, legacy removal
  Gate: All enforcement tests pass, all allowlists eliminated, grep verification clean
```

## Estimated Effort

| Section | Lines (new) | Complexity | Depends On | Status |
|---------|-----------|------------|------------|--------|
| 01 Core Data Model | ~1,000 (tags/, type_def/, method/, operator/) | Low | — | Complete |
| 02 Crate Scaffolding | ~170 (lib.rs, defs/mod.rs) | Low | 01 | Complete |
| 03 Primitive Types | ~510 (5 type defs + tests) | Low | 01, 02 | Complete |
| 04 String Type | ~590 (str.rs + tests) | Medium-High | 01, 02 | Complete |
| 05 Compound Types | ~1,100 (4 directory modules + tests) | Medium | 01, 02 | In Progress |
| 06 Collection & Wrapper Types | ~1,300 (8 types incl. Channel, directory modules + tests) | Medium | 01, 02 | Complete |
| 07 Iterator Types | ~545 (iterator/ directory module + tests) | Medium-High | 01, 02 | Complete |
| 08 Query API | ~510 (query/ directory module + tests) | Low | 01, 02 | Complete |
| 09 Wire Type Checker | ~+320 new / ~-916 deleted = ~-500 to -550 net | **High** | 03-08 | Not Started |
| 10 Wire Evaluator | ~-200 (net deletion) | Medium | 03-08 | Not Started |
| 11 Wire ARC/Borrow | ~-50 (net deletion) | Low-Medium | 03-08, 09 | Not Started |
| 12 Wire LLVM Backend | ~-150 (net deletion) | Medium | 03-08, 09, 11 | Not Started |
| 13 Migrate ori_ir | ~-400 (net deletion) | Medium | 03-08 | Not Started |
| 14 Enforcement & Exit | ~200 | Medium | 09-13 | Not Started |
| **Total new (ori_registry)** | **~6,300** (incl. ~2,000 test lines) | | | |
| **Total deleted (legacy)** | **~-1,600** (estimated) | | | |
| **Net change** | **~+4,700** (incl. tests) | | | |

## Sync Points Eliminated

This plan eliminates ALL of the following manual sync mechanisms:

| Sync Mechanism | Entries | Location | Replaced By |
|---|---|---|---|
| `TYPECK_BUILTIN_METHODS` | 390 | `ori_types/infer/expr/methods/mod.rs` | `BUILTIN_TYPES` enumeration |
| `resolve_*_method()` (20 functions) | ~431 lines | `ori_types/infer/expr/methods/resolve_by_type.rs` | `find_method(tag, name).returns` |
| `EVAL_BUILTIN_METHODS` | 230 | `ori_eval/methods/helpers/mod.rs` | `BUILTIN_TYPES` enumeration |
| `ITERATOR_METHOD_NAMES` | 24 | `ori_eval/interpreter/resolvers/mod.rs` | `find_type(Iterator).methods` |
| `DEI_ONLY_METHODS` | 5 | `ori_types/infer/expr/methods/mod.rs` | Registry-based DEI flag |
| `BUILTIN_METHODS` (ori_ir) | 123 | `ori_ir/builtin_methods/mod.rs` | Consolidated into ori_registry |
| `BuiltinRegistration.receiver_borrowed` | 206 | `ori_llvm/codegen/arc_emitter/builtins/*.rs` | `find_method().receiver` |
| `BORROWING_METHOD_NAMES` + `borrowing_builtin_names()` | ~47 + ~6 lines | `ori_arc/borrow/builtins/mod.rs` | `ori_registry::borrowing_method_names()` (derived from `BUILTIN_TYPES`) |
| `TYPECK_METHODS_NOT_IN_IR` | 130 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `EVAL_METHODS_NOT_IN_TYPECK` | 55 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `TYPECK_METHODS_NOT_IN_EVAL` | 216 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `EVAL_METHODS_NOT_IN_IR` | 26 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `IR_METHODS_DISPATCHED_VIA_RESOLVERS` | 10 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| `COLLECTION_TYPES` | 11 | `oric/src/eval/tests/methods/consistency.rs` | **Eliminated** |
| Hard-coded is_str/is_float guards | ~19 | `ori_llvm/codegen/arc_emitter/operators.rs` | OpStrategy dispatch |
| Hard-coded method names in method_call.rs | ~25 lines | `ori_types/infer/expr/calls/method_call.rs` | Stays in type checker (inference logic, not registry data) |
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
| 01 | Core Data Model Design | `section-01-core-data-model.md` | Complete |
| 02 | Crate Scaffolding & Purity Enforcement | `section-02-crate-scaffolding.md` | Complete |
| 03 | Primitive Type Definitions | `section-03-primitive-types.md` | Complete |
| 04 | String Type Definition | `section-04-string-type.md` | Complete |
| 05 | Compound Type Definitions | `section-05-compound-types.md` | In Progress |
| 06 | Collection & Wrapper Types | `section-06-collection-wrapper-types.md` | Complete |
| 07 | Iterator Type Definitions | `section-07-iterator-types.md` | Complete |
| 08 | Query API & Lookup Functions | `section-08-query-api.md` | Complete |
| 09 | Wire Type Checker (ori_types) | `section-09-wire-type-checker.md` | Not Started |
| 10 | Wire Evaluator (ori_eval) | `section-10-wire-evaluator.md` | Not Started |
| 11 | Wire ARC & Borrow Pass (ori_arc) | `section-11-wire-arc-borrow.md` | Not Started |
| 12 | Wire LLVM Backend (ori_llvm) | `section-12-wire-llvm-backend.md` | Not Started |
| 13 | Migrate ori_ir & Legacy Consolidation | `section-13-migrate-ori-ir.md` | Not Started |
| 14 | Enforcement Tests, Testing Matrix & Exit Criteria | `section-14-enforcement-testing.md` | Not Started |

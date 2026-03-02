# Monomorphization Architecture

**Status:** Design Document
**Created:** 2026-02-24
**Informed by:** Rust (`rustc`), Swift (SIL specialization), Zig (`InternPool`), Lean 4 (selective specialization)
**Scope:** Phases 1-5 of the Capability Unification & Generics Upgrade

---

## Table of Contents

- [1. Overview](#1-overview)
- [2. Design Rationale](#2-design-rationale)
- [3. Core Data Structures](#3-core-data-structures)
  - [3.1 Current: MonoInstance (Phase 1)](#31-current-monoinstance-phase-1)
  - [3.2 Future: GenericArg (Phases 2-5)](#32-future-genericarg-phases-2-5)
  - [3.3 ConstValue](#33-constvalue)
  - [3.4 MonoInstance (Redesigned)](#34-monoinstance-redesigned)
- [4. Pipeline Architecture](#4-pipeline-architecture)
  - [4.1 Phase 1: Discovery (Type Checker)](#41-phase-1-discovery-type-checker)
  - [4.2 Phase 2: Collection (Monomorphization Pass)](#42-phase-2-collection-monomorphization-pass)
  - [4.3 Phase 3: ARC Lowering (Type Substitution)](#43-phase-3-arc-lowering-type-substitution)
  - [4.4 Phase 4: LLVM Codegen](#44-phase-4-llvm-codegen)
- [5. Evolution Across Generics Phases](#5-evolution-across-generics-phases)
  - [5.1 Phase 1: Type Parameters (Current Target)](#51-phase-1-type-parameters-current-target)
  - [5.2 Phase 2: Const Generic Values](#52-phase-2-const-generic-values)
  - [5.3 Phase 3: Expanded Const Eligibility](#53-phase-3-expanded-const-eligibility)
  - [5.4 Phase 4: Associated Consts](#54-phase-4-associated-consts)
  - [5.5 Phase 5: Const Functions in Type Positions](#55-phase-5-const-functions-in-type-positions)
- [6. Comparison with Reference Compilers](#6-comparison-with-reference-compilers)
- [7. Infinite Loop Prevention](#7-infinite-loop-prevention)
- [8. Name Mangling](#8-name-mangling)
- [9. Open Questions](#9-open-questions)
- [10. File Reference](#10-file-reference)

---

## 1. Overview

Monomorphization is the process of generating specialized copies of generic code for each concrete set of type and const-value arguments used at call sites. When a programmer writes:

```ori
@identity<T> (x: T) -> T = x
```

and calls it as `identity(42)` and `identity("hello")`, the compiler produces two concrete functions:

```
identity$m$int   : (x: int) -> int  = x
identity$m$str   : (x: str) -> str  = x
```

Ori uses full monomorphization (as opposed to type erasure or boxing) for three reasons:

1. **ARC memory management requires concrete types.** Each monomorphized function knows the exact layout of every value, enabling the ARC lowerer to insert precise `rc_inc` / `rc_dec` / `rc_drop` instructions. A generic function with type-erased arguments would need to thread drop functions or vtables through every call -- adding runtime overhead and complexity that monomorphization eliminates.

2. **LLVM codegen requires concrete LLVM types.** LLVM IR is explicitly typed. Every `alloca`, `load`, `store`, `getelementptr`, and function call must specify concrete types. Generic IR with type parameters would require a polymorphic IR layer on top of LLVM, which no major compiler uses.

3. **Performance.** Monomorphized code enables inlining, devirtualization, scalar replacement, and all standard LLVM optimizations. There is no runtime dispatch overhead, no boxing, no indirect calls through vtables for generic function bodies.

### Where Monomorphization Fits in the Compilation Pipeline

```
                                     +-----------+
                                     |  Ori      |
                                     |  Source   |
                                     +-----+-----+
                                           |
                                     +-----v-----+
                                     |  Lexer    |
                                     |  Parser   |
                                     +-----+-----+
                                           |
                                     +-----v-----+
                                     |  Type     |  <-- Records MonoInstances
                                     |  Checker  |      when generic functions
                                     +-----+-----+      are called with concrete args
                                           |
                                     +-----v-----+
                                     | Canonical |
                                     |    IR     |
                                     +-----+-----+
                                           |
                              +------------+------------+
                              |                         |
                       +------v------+          +-------v-------+
                       | Interpreter |          |  LLVM Backend |
                       | (ori_eval)  |          |  (ori_llvm)   |
                       +-------------+          +-------+-------+
                                                        |
                                          +-------------+-------------+
                                          |                           |
                                   +------v------+            +------v------+
                                   | Mono Pass   |            | (future)    |
                                   | Collect     |            | WASM        |
                                   | + Dedup     |            | Backend     |
                                   +------+------+            +-------------+
                                          |
                                   +------v------+
                                   | ARC Lowerer |  <-- Uses body_type_map to
                                   | (ori_arc)   |      substitute generic Idx
                                   +------+------+      with concrete Idx
                                          |
                                   +------v------+
                                   | ARC Pipeline|  <-- Borrow inference, RC
                                   | (optimize)  |      insertion, RC elision,
                                   +------+------+      constructor reuse
                                          |
                                   +------v------+
                                   | LLVM IR Gen |  <-- Concrete types enable
                                   | (codegen)   |      precise type lowering
                                   +------+------+
                                          |
                                   +------v------+
                                   |  Native /   |
                                   |  JIT Binary |
                                   +-------------+
```

The interpreter (`ori_eval`) does not need monomorphization -- it evaluates generic functions dynamically using boxed `Value` representations. Monomorphization is exclusively a concern of the compiled (LLVM/WASM) backends.

---

## 2. Design Rationale

### Why Monomorphization Over Type Erasure

| Property | Monomorphization | Type Erasure |
|----------|-----------------|--------------|
| Runtime overhead | None (concrete code) | Vtable dispatch, boxing |
| Code size | O(instantiations) | O(1) per generic |
| ARC compatibility | Direct (concrete types) | Requires drop glue indirection |
| LLVM optimization | Full (inlining, SROA, etc.) | Limited (opaque pointers) |
| Compile time | Proportional to instantiations | Proportional to generics |
| Binary size | Larger (duplicated code) | Smaller |

For Ori's design pillars -- ARC memory management, expression-based semantics, and LLVM/WASM targets -- monomorphization is the natural choice. Every reference compiler targeting similar constraints (Rust, Zig, Roc) uses monomorphization. Languages that use type erasure (Java, Go interfaces) either have a garbage collector or accept the performance penalty of indirect dispatch.

### Why This Specific Design

After studying four reference compilers, the following design principles emerged:

1. **Unified argument representation.** Rust, Swift, and Zig all converge on a single enum that represents "a concrete argument to a generic parameter." Rust calls it `GenericArg`, Swift uses `SubstitutionMap`, Zig uses `InternPool.Index`. The key insight is that type parameters (`T -> int`) and const value parameters (`$N -> 42`) are the same operation from the monomorphizer's perspective: substitute a placeholder with a concrete value.

2. **Identity key is `(function, args)`.** All four compilers use the function definition plus its concrete generic arguments as the cache/dedup key. Rust uses `(DefId, GenericArgsRef)` with pointer comparison on interned lists. Zig uses `(generic_owner, comptime_args)` with InternPool hashing. Ori uses `(fn_name, type_args)` with structural comparison, upgrading to `(fn_name, generic_args)` as const generics are added.

3. **Shared body, substituted types.** Rust and Zig both avoid cloning the function body -- they share the original IR and lazily substitute types during codegen. Swift does clone-and-substitute because SIL transformations benefit from having a complete specialized copy. Ori follows the Swift model: the ARC lowerer needs a complete function body with concrete types to perform borrow inference, RC insertion, and constructor reuse analysis. The `body_type_map` field enables this substitution.

4. **Two-phase collection.** All four compilers separate "discovery" (finding which instantiations are needed) from "generation" (producing the specialized code). Discovery happens during type checking or an early traversal. Generation happens lazily during codegen. This separation allows deduplication before any expensive work begins.

### What We Took from Each Compiler

| Compiler | What Ori Adopts | What Ori Avoids |
|----------|----------------|-----------------|
| **Rust** | `GenericArg` enum unifying types and const values; `(DefId, args)` identity key; lazy substitution pattern | Region/lifetime parameters (Ori has ARC, not borrows); `TypeFoldable` visitor complexity |
| **Swift** | Clone-and-substitute model (fits ARC lowering); infinite loop detection via chain walking | Full SIL cloning overhead; re-abstraction thunks |
| **Zig** | Everything-is-an-index uniformity; InternPool dedup; lazy instantiation | Full comptime evaluation at monomorphization time (too complex for Phase 1) |
| **Lean 4** | Selective specialization insight (not all type args affect runtime); const value tracking | Fixpoint specialization loop; beta-normalized expression keys |

---

## 3. Core Data Structures

### 3.1 Current: MonoInstance (Phase 1)

The current implementation supports only type parameter monomorphization. This is the structure as it exists today in the codebase:

```rust
// compiler/ori_types/src/output/mod.rs

/// A concrete instantiation of a generic function discovered during type checking.
///
/// Recorded when a generic function like `@identity<T>(x: T) -> T` is called
/// with concrete types (e.g., `identity(x: 42)` produces `T = int`). The LLVM
/// monomorphizer stamps out one specialized function per unique `MonoInstance`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonoInstance {
    /// The generic function being instantiated.
    pub fn_name: Name,
    /// Concrete types for each type parameter (parallel to `FunctionSig.scheme_var_ids`).
    pub type_args: Vec<Idx>,
    /// Substituted parameter types (all type variables replaced with concrete types).
    pub concrete_param_types: Vec<Idx>,
    /// Substituted return type.
    pub concrete_return_type: Idx,
    /// Maps generic `Idx` -> concrete `Idx` for body expression types.
    ///
    /// The ARC lowerer uses this to substitute types when lowering the shared
    /// canonical IR body into a monomorphized ARC function.
    pub body_type_map: FxHashMap<Idx, Idx>,
}

impl std::hash::Hash for MonoInstance {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fn_name.hash(state);
        self.type_args.hash(state);
    }
}
```

**Key property:** The `Hash` implementation uses only `(fn_name, type_args)`, matching the identity key semantics described in Section 2. Two `MonoInstance` values with the same function and type arguments are considered identical, regardless of the specific `body_type_map` entries (which are deterministically derived from the type arguments).

### 3.2 Future: GenericArg (Phases 2-5)

As const generics are added, `type_args: Vec<Idx>` must evolve to handle both type substitutions (`T -> int`) and const value substitutions (`$N -> 42`). The unified `GenericArg` enum provides this:

```rust
/// A concrete argument to a generic parameter.
///
/// Unifies type substitution (T -> int) and const value substitution ($N -> 42).
/// Every major compiler converges on this design:
/// - Rust: GenericArgKind { Type(Ty), Const(Const), Lifetime(Region) }
/// - Zig: InternPool.Index (uniform representation)
/// - Swift: SubstitutionMap (types + conformances)
///
/// Ori omits lifetimes (ARC, not borrows) and conformances (resolved separately).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum GenericArg {
    /// Type parameter: `T -> int`
    Type(Idx),
    /// Const generic value: `$N -> 42`, `$B -> true`
    Const(ConstValue),
}
```

This design extends naturally across all five generics phases without structural changes. Phase 1 uses only `GenericArg::Type`. Phase 2 adds `GenericArg::Const`. Phases 3-5 expand `ConstValue` variants. The enum itself never changes shape.

### 3.3 ConstValue

```rust
/// A compile-time value used as a const generic argument.
///
/// Phase 2: Int, Bool
/// Phase 3: Str, Char, Byte, Enum, List, Tuple (any type with Eq + Hashable)
/// Phase 4-5: No new variants (associated consts and const functions produce
///            values already representable by existing variants)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ConstValue {
    // Phase 2 -- basic const generics
    Int(i64),
    Bool(bool),

    // Phase 3 -- expanded const eligibility (any type with Eq + Hashable)
    // Str(Name),        // Interned for O(1) equality
    // Char(char),
    // Byte(u8),
    // Enum { type_name: Name, variant: Name },
    // List(Vec<ConstValue>),
    // Tuple(Vec<ConstValue>),
}
```

**Interning considerations:** When `ConstValue` gains compound variants (`List`, `Tuple`), these values should be interned in the Pool to enable O(1) equality comparison. The natural extension is to follow Zig's model where const values receive their own `Idx` entries in the Pool, but this is a Phase 3 concern. Phases 1-2 use only `Int(i64)` and `Bool(bool)`, which have trivial O(1) equality.

### 3.4 MonoInstance (Redesigned)

When `GenericArg` is introduced, `MonoInstance` evolves from a type-only struct to a unified struct:

```rust
/// A concrete instantiation of a generic function.
///
/// Redesigned from the Phase 1 `type_args: Vec<Idx>` representation to use
/// `generic_args: Vec<GenericArg>`, unifying type and const value substitution.
pub struct MonoInstance {
    /// The generic function being instantiated.
    pub fn_name: Name,

    /// Concrete generic arguments (parallel to function's generic params).
    ///
    /// For `@replicate<T, $N: int>(value: T) -> [T, max N]` called as
    /// `replicate<str, 5>("hi")`, this would be:
    /// `[GenericArg::Type(Idx::STR), GenericArg::Const(ConstValue::Int(5))]`
    pub generic_args: Vec<GenericArg>,

    /// Substituted parameter types (all type variables replaced with concrete types).
    pub concrete_param_types: Vec<Idx>,

    /// Substituted return type.
    pub concrete_return_type: Idx,

    /// Type substitution map for the ARC lowerer.
    ///
    /// Maps generic `Idx` -> concrete `Idx` for every type in the function body.
    /// The ARC lowerer applies this map when lowering the shared canonical IR
    /// body into a monomorphized ARC function.
    pub body_type_map: FxHashMap<Idx, Idx>,
}
```

**Identity key:** `(fn_name, generic_args)`. The `Hash` implementation hashes only these two fields. Two instances with the same function and generic arguments are identical.

**Migration path:** The Phase 1 `type_args: Vec<Idx>` is equivalent to `generic_args: Vec<GenericArg>` where every element is `GenericArg::Type(idx)`. The migration is mechanical: wrap each `Idx` in `GenericArg::Type()` and update pattern matches. This can happen at the boundary between Phase 1 and Phase 2.

---

## 4. Pipeline Architecture

The monomorphization pipeline has four phases, each handled by a different compiler component.

### 4.1 Phase 1: Discovery (Type Checker)

**Component:** `ori_types` (InferEngine, ModuleChecker)
**Files:** `compiler/ori_types/src/infer/expr/calls.rs`, `compiler/ori_types/src/infer/mod.rs`, `compiler/ori_types/src/check/mod.rs`

When the type checker processes a call to a generic function, it records a `MonoInstance`:

```
Source:     let result = identity(42)
            ~~~~~~~~~~~~~~~~~~~~~~~~
Call site:  @identity<T>(x: T) -> T  called with  x: int

Type checker:
  1. Instantiate scheme: fresh_var() -> ?a
  2. Unify ?a with int (from argument)
  3. Resolve ?a -> int
  4. Build type_args: [int]  (parallel to scheme_var_ids)
  5. Build body_type_map: {?a -> int}
  6. Record MonoInstance { fn_name: "identity", type_args: [int], ... }
```

The discovery happens in `infer_call()` and `infer_call_named()`, after argument checking succeeds. The key fields come from the `FunctionSig`:

- **`scheme_var_ids: Vec<u32>`** -- Pool variable IDs for the function's quantified type variables. These are the "placeholder" types that the monomorphizer replaces. Parallel to `FunctionSig.type_params`.

- **`generic_param_mapping: Vec<Option<usize>>`** -- For each type parameter, optionally maps to the function parameter index where that type appears directly. Used for fast concrete type extraction from resolved argument types.

The `InferEngine` accumulates `MonoInstance` values in its `mono_instances: Vec<MonoInstance>` field. After each function body is checked, the `ModuleChecker` calls `engine.take_mono_instances()` and accumulates them via `accumulate_mono_instances()`.

During `ModuleChecker::finish()`, mono instances are sorted by `(fn_name, type_args)` and deduplicated. The resulting list is stored in `TypedModule.mono_instances`.

```
                          InferEngine
                          +---------------------------+
  infer_call() -------->  | mono_instances: Vec<MI>   |
  infer_call_named() -->  |                           |
                          +---------------------------+
                                      |
                          take_mono_instances()
                                      |
                                      v
                          ModuleChecker
                          +---------------------------+
                          | mono_instances: Vec<MI>   |
                          | (accumulated from all     |
                          |  function body checks)    |
                          +---------------------------+
                                      |
                          finish() -> sort + dedup
                                      |
                                      v
                          TypedModule
                          +---------------------------+
                          | mono_instances: Vec<MI>   |
                          | (deduplicated, sorted)    |
                          +---------------------------+
```

### 4.2 Phase 2: Collection (Monomorphization Pass)

**Component:** `ori_llvm` (monomorphization pass)
**Files:** `compiler/ori_llvm/src/monomorphize.rs` (planned)

The collection phase converts `MonoInstance` values (type checker output) into `MonoFunction` values (codegen input). This is where:

1. **Name mangling** happens -- each unique instance gets a deterministic mangled name.
2. **Concrete FunctionSigs** are produced -- generic signatures with type params become non-generic signatures with concrete types.
3. **Deduplication across modules** occurs -- the same generic function called with the same types from different modules produces one MonoFunction, not two.

```rust
/// A monomorphized function ready for codegen.
///
/// Produced by the monomorphization collection pass from `MonoInstance`.
/// Has a non-generic `FunctionSig` (is_generic() = false) and a mangled name.
pub struct MonoFunction {
    /// Mangled name: `{fn_name}$m${type_encoding}`
    pub mangled_name: Name,
    /// Original generic function name (for body lookup in canonical IR).
    pub original_name: Name,
    /// Concrete function signature (no type params, concrete types).
    pub sig: FunctionSig,
    /// Type substitution map for ARC lowering.
    pub body_type_map: FxHashMap<Idx, Idx>,
}
```

The collection pass:

```
collect_mono_functions()
    Input:  TypedModule.mono_instances + FunctionSig map
    Output: Vec<MonoFunction>

    For each unique MonoInstance:
      1. Find the generic FunctionSig by fn_name
      2. Create a concrete FunctionSig:
         - name = mangled_name
         - type_params = []  (empty -- no longer generic)
         - param_types = instance.concrete_param_types
         - return_type = instance.concrete_return_type
         - all other fields copied from original sig
      3. Compute mangled name (see Section 8)
      4. Produce MonoFunction
```

### 4.3 Phase 3: ARC Lowering (Type Substitution)

**Component:** `ori_arc` (ARC lowerer)
**Files:** `compiler/ori_arc/src/lower/mod.rs`, `compiler/ori_arc/src/lower/expr/mod.rs`

The ARC lowerer converts canonical IR into ARC IR -- a basic-block representation with explicit ownership, borrow, and RC operations. For monomorphized functions, the lowerer must substitute generic types with concrete types.

**Swift's model** is the right fit for Ori's ARC pipeline. Swift clones the SIL body and substitutes all types in the clone, because:
- Ownership analysis requires knowing exact types (trivial vs. non-trivial for retain/release decisions)
- Borrow inference needs concrete struct layouts
- Constructor reuse (`reset`/`reuse`) depends on knowing the exact type being constructed

Ori follows the same approach. The `lower_function_can()` entry point receives the **shared** canonical IR body (no cloning at the expression tree level) but produces a **fresh** ARC IR function with concrete types. The `body_type_map` is consulted every time the lowerer reads a type from the canonical IR:

```
Canonical IR body (shared):
    Let(x, Call("foo", [Param(0)]))   types: { Param(0): ?a, Call: ?b }

body_type_map: { ?a -> int, ?b -> str }

ARC IR body (concrete):
    var0 = param(0)          type: int
    var1 = call "foo" [var0]  type: str
```

The planned integration is:

```rust
/// In lower_function_can(), a type_subst parameter enables monomorphization.
///
/// When Some, every type read from the canonical IR's expression types is
/// passed through this map. When None (non-generic functions), types are
/// used as-is.
pub fn lower_function_can(
    name: Name,
    params: &[(Name, Idx)],
    return_type: Idx,
    body: CanId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ArcProblem>,
    is_fbip: bool,
    type_subst: Option<&FxHashMap<Idx, Idx>>,   // NEW for monomorphization
) -> (ArcFunction, Vec<ArcFunction>)
```

Inside the `ArcLowerer`, a helper resolves types:

```rust
fn resolve_body_type(&self, ty: Idx) -> Idx {
    match &self.type_subst {
        Some(map) => map.get(&ty).copied().unwrap_or(ty),
        None => ty,
    }
}
```

**All existing callers** of `lower_function_can()` pass `None` for `type_subst` -- zero behavioral change for non-generic functions.

After ARC lowering, the resulting `ArcFunction` has fully concrete types. The rest of the ARC pipeline -- borrow inference, RC insertion, RC elimination, constructor reuse -- operates on concrete types exactly as it does for non-generic functions. No changes are needed in any downstream ARC pass.

### 4.4 Phase 4: LLVM Codegen

**Component:** `ori_llvm` (FunctionCompiler, ArcIrEmitter)
**Files:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

Once a `MonoFunction` has been ARC-lowered into an `ArcFunction` with concrete types, it enters the LLVM codegen pipeline as a normal (non-generic) function. This is the key insight: **monomorphized functions are indistinguishable from non-generic functions at the codegen level.**

The integration points:

1. **Declaration.** `FunctionCompiler::declare_all()` currently skips functions where `sig.is_generic()`. MonoFunctions have `is_generic() = false` (empty type_params), so they pass through the existing declaration logic without changes.

2. **Definition.** `FunctionCompiler::define_all()` similarly skips generics. MonoFunctions pass through unchanged.

3. **Call site resolution.** When the ArcIrEmitter encounters a call to a generic function, it must resolve to the mangled name of the appropriate monomorphized instance. This requires:
   - Looking up the concrete argument types at the call site
   - Computing the mangled name from those types
   - Resolving to the declared LLVM function value

```
Source:       identity(42)
ARC IR:       call "identity" [var0: int]
Resolution:   lookup("identity", arg_types=[int]) -> "identity$m$int"
LLVM IR:      call @identity$m$int(i64 %0)
```

---

## 5. Evolution Across Generics Phases

The generics system evolves through five phases defined in the Capability Unification & Generics Upgrade proposal. Each phase extends the monomorphization pipeline incrementally.

### 5.1 Phase 1: Type Parameters (Current Target)

**What:** `T -> int` -- only `GenericArg::Type` is used.
**Identity key:** `(fn_name, [Idx, ...])`
**Data structure:** `MonoInstance { type_args: Vec<Idx>, ... }`

This is the current implementation target. Only type parameter monomorphization is supported. An Ori source file:

```ori
@identity<T> (x: T) -> T = x

@main () -> void = {
    let a = identity(42)        // MonoInstance: identity, [int]
    let b = identity("hello")   // MonoInstance: identity, [str]
    print(a.to_str() + " " + b)
}
```

produces two MonoInstances:

```
MonoInstance { fn_name: "identity", type_args: [Idx::INT], ... }
MonoInstance { fn_name: "identity", type_args: [Idx::STR], ... }
```

and two mangled functions:

```
identity$m$int : (int) -> int
identity$m$str : (str) -> str
```

**Scope limitations in Phase 1:**
- Direct type parameters only (`generic_param_mapping[i] = Some(...)`). Indirect parameters (T inside `[T]`, `(T, U)`) are deferred.
- Free functions only. Generic trait methods are deferred.
- No recursive generic instantiation discovery. Only top-level call sites from non-generic functions are collected.

### 5.2 Phase 2: Const Generic Values

**What:** Adding `GenericArg::Const(ConstValue::Int/Bool)`. `$N: int -> 42`, `$B: bool -> true`.
**Identity key:** `(fn_name, [GenericArg, ...])`
**Data structure:** `MonoInstance { generic_args: Vec<GenericArg>, ... }`

Phase 2 introduces const generic parameters. The monomorphization pipeline extends to handle const values as generic arguments:

```ori
@zeros<$N: int> () -> [int, max N]
    where N > 0
= for _ in 0..N yield 0

@main () -> void = {
    let a = zeros<5>()    // MonoInstance: zeros, [Const(Int(5))]
    let b = zeros<10>()   // MonoInstance: zeros, [Const(Int(10))]
}
```

**Impact on identity key:** The identity key becomes `(fn_name, generic_args)` where `generic_args` is `Vec<GenericArg>`. Since `GenericArg` derives `Hash` and `Eq`, deduplication works automatically.

**Impact on name mangling:** Const values are encoded in the mangled name:

```
zeros$m$c5    -- $N = 5
zeros$m$c10   -- $N = 10
```

For mixed type + const parameters:

```ori
@replicate<T, $N: int> (value: T) -> [T, max N] = ...
replicate<str, 3>("hi")   // replicate$m$str_c3
```

**Impact on body_type_map:** Const generic parameters do not appear in `body_type_map` (they are values, not types). However, types that _depend_ on const values -- like `[int, max N]` where `N = 5` resolving to `[int, max 5]` -- must be substituted. The type checker resolves these during discovery, producing concrete types like `[int, max 5]` in `concrete_param_types` and `concrete_return_type`.

**New field: const_value_map.** For the function body to use const generic values in expressions (e.g., `for _ in 0..N`), the codegen layer needs a map from const parameter names to their concrete values:

```rust
/// Maps const parameter Name -> concrete value.
///
/// Used during codegen to replace references to `$N` in the function body
/// with the concrete value (e.g., `5`).
pub const_value_map: FxHashMap<Name, ConstValue>,
```

### 5.3 Phase 3: Expanded Const Eligibility

**What:** Any type with `Eq + Hashable` becomes const-eligible. `$C: Color -> Color.Red`, `$S: [int] -> [1, 2, 3]`.
**Impact:** `ConstValue` gains new variants, but the pipeline architecture is unchanged.

```rust
pub enum ConstValue {
    Int(i64),
    Bool(bool),
    // Phase 3 additions:
    Str(Name),          // Interned string
    Char(char),
    Byte(u8),
    Enum {              // User-defined enum value
        type_name: Name,
        variant: Name,
        fields: Vec<ConstValue>,
    },
    List(Vec<ConstValue>),
    Tuple(Vec<ConstValue>),
}
```

**Interning:** Compound `ConstValue` variants (`List`, `Tuple`, `Enum` with fields) should be interned to avoid O(n) equality comparison during deduplication. The natural interning strategy is to assign each unique `ConstValue` an `Idx` in the Pool, following Zig's "everything is an index" philosophy. This is Phase 3 design work and does not affect the pipeline architecture.

**Name mangling:** Compound values use a deterministic encoding:

```
// $C: Color = Color.Red
fn_name$m$eColor_Red

// $S: [int] = [1, 2, 3]
fn_name$m$Li1_2_3
```

Details are specified in Section 8.

### 5.4 Phase 4: Associated Consts

**What:** Traits can declare const members. `T.$rank -> 2` is resolved during discovery.

```ori
trait Shaped {
    $rank: int
    $shape: [int]
}

@reshape<T with Shaped, U with Shaped> (data: T) -> U
    where $product(T.$shape) == $product(U.$shape)
= ...
```

**Impact on discovery:** During type checking, when a generic function with associated const references is called, the type checker must resolve the associated const values from the concrete type's trait implementation. This adds a new resolution step:

```
Call: reshape<Matrix2x3, Matrix3x2>(data: m)

Resolution:
  T = Matrix2x3  ->  T.$rank = 2,  T.$shape = [2, 3]
  U = Matrix3x2  ->  U.$rank = 2,  U.$shape = [3, 2]

The type checker checks: $product([2, 3]) == $product([3, 2])  (6 == 6, OK)
```

**Impact on MonoInstance:** A new field is needed for associated const values:

```rust
/// Resolved associated const values for this instance.
///
/// Maps (type_param_name, const_name) -> ConstValue.
/// E.g., ("T", "rank") -> ConstValue::Int(2)
pub assoc_const_map: FxHashMap<(Name, Name), ConstValue>,
```

The `body_type_map` is unchanged -- it still maps generic `Idx` to concrete `Idx`. Associated consts are _values_, not types, and flow through `const_value_map`.

### 5.5 Phase 5: Const Functions in Type Positions

**What:** `$product(FROM)` evaluation during discovery. Const functions are evaluated at compile time to produce `ConstValue` results that participate in type checking and monomorphization.

```ori
$product (shape: [int]) -> int = shape.fold(1, (a, b) -> a * b)

@reshape<T with Shaped, U with Shaped> (data: T) -> U
    where $product(T.$shape) == $product(U.$shape)
= ...
```

**Impact on discovery:** The type checker must invoke a const evaluator to compute `$product(T.$shape)` where `T.$shape` is already resolved to a concrete `ConstValue::List(...)`. This requires a mini-interpreter for const functions, limited to pure computations on `ConstValue` inputs.

**Impact on MonoInstance:** No structural change. The const evaluator produces `ConstValue` results that are stored in `assoc_const_map` or `const_value_map`. The pipeline remains:

```
Discovery -> Collection -> ARC Lowering -> Codegen
```

**Impact on the pipeline:** The const evaluator is a new component that sits within the type checker (Phase 4.1, Discovery). It takes `ConstValue` inputs and produces `ConstValue` outputs. It does not affect the Collection, ARC Lowering, or Codegen phases.

---

## 6. Comparison with Reference Compilers

| Aspect | Rust | Swift | Zig | Lean 4 | **Ori** |
|--------|------|-------|-----|--------|---------|
| **Generic arg repr** | `GenericArg` enum (Type, Region, Const) | `SubstitutionMap` (types + conformances) | `InternPool.Index` (uniform) | Expression closure | `GenericArg` enum (Type, Const) |
| **Identity key** | `(DefId, GenericArgsRef)` interned ptr comparison | Mangled name encoding | `(generic_owner, comptime_args)` hash | Beta-normalized expr | `(fn_name, generic_args)` structural |
| **Body strategy** | Shared (lazy substitution during codegen) | Full clone + substitute | Shared ZIR (no cloning) | Substitute + re-typecheck | Shared canonical IR, substituted during ARC lowering |
| **ARC/RC integration** | N/A (borrow checker, no RC codegen) | Post-specialization retain/release elimination | N/A (no RC) | LiveVars with derived value tracking | `body_type_map` drives ARC lowerer substitution |
| **Dedup mechanism** | Interned `GenericArgsRef` pointer equality | Deterministic mangled name | InternPool hashing | Expression structural equality | Sort + dedup by `(fn_name, args)` |
| **Infinite loop protection** | None (types are acyclic by construction) | Chain walking (depth 50, width 2000, length 10) | Comptime eval timeout | Fixpoint loop with iteration limit | Phase 1: none needed; later: Swift-style chain walking |
| **Const values** | `ValTree` (recursive, interned) | N/A (no const generics) | `InternPool.Index` (uniform with types) | Lean expressions | `ConstValue` enum (progressive expansion) |
| **Erasure** | Full monomorphization | Full monomorphization | Full monomorphization | Selective (erase type args that don't affect runtime) | Full monomorphization |
| **Cross-module** | Two-level: local table + serialized imports | Two-level: module table + cross-module serialized | Single InternPool per compilation | Module-boundary specialization points | Mono instances collected per-module, deduped globally |

### Key Architectural Differences

**Ori vs. Rust:** Rust has no ARC lowering phase -- ownership and borrowing are checked at the type level, and codegen emits drops based on MIR analysis. Ori must substitute types _before_ ARC analysis, so the substitution happens earlier in the pipeline (at ARC lowering) rather than later (at LLVM codegen). Rust also has lifetime parameters in its `GenericArg` enum, which Ori omits entirely.

**Ori vs. Swift:** Closest architectural match. Both use ARC, both clone-and-substitute function bodies for specialization, both optimize away retain/release on trivial types after specialization. The main difference is that Swift operates at the SIL level (a higher-level IR with explicit ownership) while Ori operates at the ARC IR level (basic blocks with explicit RC operations). Swift also has a more complex re-abstraction model for witness table thunks.

**Ori vs. Zig:** Zig's "everything is an index" model is elegant but requires a full comptime evaluator at monomorphization time. Ori defers const evaluation to the type checker, keeping the monomorphization pass simple. Zig also doesn't have ARC/RC, so its monomorphization is purely about type specialization.

**Ori vs. Lean 4:** Lean's selective monomorphization is an optimization Ori may adopt in the future -- erasing type arguments that don't affect runtime behavior (e.g., phantom type parameters). For now, Ori uses full monomorphization like Rust/Swift/Zig.

---

## 7. Infinite Loop Prevention

Monomorphization can diverge when generic functions create new instantiations during specialization. Consider:

```ori
@grow<T> (x: T) -> [T] = [grow(x: [x])]
// grow<int> needs grow<[int]> needs grow<[[int]]> needs ...
```

### When Is Protection Needed?

- **Phase 1 (type parameters only):** Protection is not needed. The type checker records monomorphization instances for calls in non-generic function bodies. A generic function body that calls another generic function does not directly produce new MonoInstances -- only concrete call sites do. Recursive growth requires a generic function body that _creates_ new type arguments, which implies that the original call site would need to have already specified the full type chain.

- **Phase 2+ (const generics):** Protection becomes necessary. A const generic function could call itself with a computed value (`grow<N+1>()`), creating an infinite chain. Swift and Lean 4 both handle this:

### Swift's Approach: Chain Walking

Swift detects growing substitution chains during specialization:

1. Walk the specialization chain from the current instance back to the root.
2. At each step, check if the substitution is "growing" (types are getting larger).
3. Abort if: depth > 50, total substitution width > 2000, or chain length > 10.

This is conservative but effective. It catches `grow<T> -> grow<[T]>` because `[T]` is structurally larger than `T`.

### Lean 4's Approach: Fixpoint Loop

Lean 4 runs specialization as a fixpoint computation with an iteration limit (`maxRecSpecialize`). Each iteration discovers new specialization opportunities. If the set of required specializations doesn't stabilize within the limit, specialization stops.

### Ori's Plan

- **Phase 1:** No protection. The type checker only records instances from concrete call sites.
- **Phase 2 (const generics):** Add a simple chain-length limit (e.g., 64 instantiations per generic function). If exceeded, emit a warning and stop monomorphizing.
- **Phase 3+ (expanded eligibility):** Implement Swift-style chain walking. Track the substitution growth from each new instance back to its origin. Abort on detected growth with a clear error message pointing to the recursive pattern.

---

## 8. Name Mangling

Monomorphized functions need deterministic names that encode the concrete generic arguments. The mangling scheme must be:

1. **Deterministic** -- same arguments produce same name across compilations.
2. **Injective** -- different arguments produce different names.
3. **Human-readable** (for debugging) -- not required, but helpful.
4. **Extensible** -- new `GenericArg` variants add new encodings without breaking existing ones.

### Scheme

```
{fn_name}$m${arg1}_{arg2}_{argN}
```

The `$m$` separator distinguishes monomorphized names from user-defined names (which cannot contain `$` except as the const generic sigil).

### Type Encoding

| Type | Encoding | Example |
|------|----------|---------|
| `int` | `int` | `identity$m$int` |
| `float` | `float` | `identity$m$float` |
| `bool` | `bool` | `identity$m$bool` |
| `str` | `str` | `identity$m$str` |
| `char` | `char` | `identity$m$char` |
| `byte` | `byte` | `identity$m$byte` |
| `()` (unit) | `void` | `identity$m$void` |
| `[T]` (List) | `L{elem}` | `filter$m$Lint` (List\<int\>) |
| `Option<T>` | `O{inner}` | `unwrap$m$Oint` |
| `Result<T, E>` | `R{ok}_{err}` | `try$m$Rint_str` |
| `(T, U, ...)` | `T{e1}_{e2}` | `swap$m$Tint_bool` |
| `{name}` (Struct) | `S{name}` | `process$m$SPoint` |
| `{name}` (Enum) | `E{name}` | `handle$m$EColor` |
| `(A) -> B` (Function) | `F{a1}_{aN}_R{ret}` | -- |

### Const Value Encoding (Phase 2+)

| Value | Encoding | Example |
|-------|----------|---------|
| `Int(n)` where n >= 0 | `c{n}` | `zeros$m$c5` |
| `Int(n)` where n < 0 | `cn{abs(n)}` | `offset$m$cn3` (-3) |
| `Bool(true)` | `ctrue` | `flag$m$ctrue` |
| `Bool(false)` | `cfalse` | `flag$m$cfalse` |
| `Str(s)` (Phase 3) | `cs{len}_{hex}` | -- |
| `Enum { variant }` (Phase 3) | `ce{type}_{variant}` | -- |

### Mixed Type + Const Examples

```ori
@replicate<T, $N: int> (value: T) -> [T, max N]

replicate<str, 3>("hi")
// Mangled: replicate$m$str_c3

replicate<int, 10>(0)
// Mangled: replicate$m$int_c10
```

### Encoding Rules

1. Arguments are encoded in declaration order (parallel to the function's generic parameter list).
2. Type arguments and const arguments are intermixed as they appear in the declaration.
3. Nested types are encoded recursively without separators within a single argument (e.g., `LLint` = `[[int]]`).
4. The `_` separator appears only between top-level arguments.
5. For cross-module mangling, the module path may be prepended: `{module_path}$m${args}`.

---

## 9. Open Questions

### 9.1 Recursive Generic Instantiation Discovery

Phase 1 only discovers MonoInstances from non-generic function bodies. When `@foo<T>` calls `@bar<[T]>`, the MonoInstance for `bar` is only recorded if `foo` itself is called with a concrete type. This means:

```ori
@bar<T> (x: T) -> T = x
@foo<T> (x: T) -> T = bar([x]).first()

@main () -> void = {
    foo(42)    // Records: foo<int>, bar<[int]>
}
```

The call `bar([x])` inside `foo<int>` needs `bar<[int]>`, which must be discovered transitively. The current type checker records this because it checks the body of `foo` with `T = int` resolved, seeing the call `bar([x])` with `x: int` and thus `[x]: [int]`.

However, if generic function bodies are only checked once (with unresolved type variables), transitive discovery requires a separate pass. This is a Phase 1 implementation question: does the type checker re-check generic function bodies per instantiation, or does a separate discovery pass walk the call graph?

**Recommendation:** The type checker re-checks generic function bodies per instantiation (same model as Zig's lazy instantiation). This naturally produces all transitive MonoInstances without a separate pass.

### 9.2 Generic Trait Methods

Phase 1 handles only free functions. Generic trait methods (`impl<T with Eq> Eq for [T]`) require resolving both the type parameter and the trait implementation at the call site. This is more complex and is deferred to after Phase 1.

### 9.3 Cross-Module Monomorphization

When module A defines `@identity<T>` and module B calls `identity(42)`, the MonoInstance is recorded in module B. The monomorphization pass must have access to module A's canonical IR body to produce the specialized function. This requires cross-module IR access, which the current Salsa-based compilation pipeline supports through query dependencies.

### 9.4 Pool Interning for ConstValue (Phase 3)

When `ConstValue` gains compound variants, structural equality becomes O(n). Interning const values in the Pool (assigning each unique value an `Idx`) restores O(1) equality. The design for this interning -- whether to reuse the existing Pool or create a separate const value pool -- is a Phase 3 decision.

### 9.5 Selective Monomorphization (Future)

Lean 4's insight that not all type arguments affect runtime behavior is relevant for Ori. A generic function `@ignore<T>(x: T) -> int = 42` does not need different specializations for different `T` values -- the body never uses the type parameter. Detecting and eliding unnecessary specializations is an optimization that could reduce binary size and compile time. This is a post-Phase 5 consideration.

---

## 10. File Reference

### Current Implementation

| File | Role |
|------|------|
| `compiler/ori_types/src/output/mod.rs` | `MonoInstance` struct, `TypedModule.mono_instances` |
| `compiler/ori_types/src/output/mod.rs` | `FunctionSig.scheme_var_ids`, `FunctionSig.generic_param_mapping` |
| `compiler/ori_types/src/infer/mod.rs` | `InferEngine.mono_instances`, `record_mono_instance()`, `take_mono_instances()` |
| `compiler/ori_types/src/check/mod.rs` | `ModuleChecker.mono_instances`, `accumulate_mono_instances()`, dedup in `finish()` |
| `compiler/ori_types/src/infer/expr/calls.rs` | Discovery: records MonoInstance after generic call checking |
| `compiler/ori_types/src/idx/mod.rs` | `Idx` type handle (32-bit pool index) |
| `compiler/ori_types/src/pool/mod.rs` | `Pool` -- unified type storage, interning, deduplication |
| `compiler/ori_arc/src/lower/mod.rs` | `lower_function_can()` -- ARC IR lowering entry point |
| `compiler/ori_arc/src/lower/expr/mod.rs` | `ArcLowerer` -- expression tree to ARC IR conversion |
| `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` | `FunctionCompiler` -- LLVM function declaration/definition |

### Planned Files

| File | Role |
|------|------|
| `compiler/ori_llvm/src/monomorphize.rs` | `MonoFunction`, `collect_mono_functions()` |
| `compiler/ori_types/src/pool/substitute.rs` | `substitute_in_pool()` recursive type substitution |

### Related Proposals and Plans

| Document | Relevance |
|----------|-----------|
| `docs/ori_lang/proposals/approved/capability-unification-generics-proposal.md` | Defines the 5-phase generics upgrade |
| `docs/ori_lang/proposals/approved/const-generics-proposal.md` | Const generic syntax and semantics |
| `plans/roadmap/section-21A-llvm.md` | LLVM backend roadmap, Section 21.7 (monomorphization) |
| `plans/roadmap/section-18-const-generics.md` | Const generics roadmap |
| `plans/roadmap/section-19-existential-types.md` | Existential types (static dispatch via monomorphization) |

### Reference Compiler Sources

| Compiler | Key Files |
|----------|-----------|
| Rust | `compiler/rustc_middle/src/ty/generic_args.rs`, `compiler/rustc_monomorphize/src/collector.rs` |
| Swift | `lib/SILOptimizer/Utils/Generics.cpp`, `include/swift/AST/SubstitutionMap.h` |
| Zig | `src/InternPool.zig` (Key.Func, generic_owner, comptime_args) |
| Lean 4 | `src/Lean/Compiler/Specialize.lean`, `src/Lean/Compiler/IR/RC.lean` |

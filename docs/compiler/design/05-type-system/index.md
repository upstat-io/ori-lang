---
title: "Type System Overview"
description: "Ori Compiler Design — Type System Overview"
order: 500
section: "Type System"
---

# Type System Overview

The Ori type system provides strict static typing with Hindley-Milner type inference, extended with rank-based let-polymorphism, capability tracking, and user-defined types. The entire type system lives in a single crate, `ori_types`, built around a unified pool architecture.

## Location

```
compiler/ori_types/src/
├── lib.rs                    # Module exports, Salsa compatibility assertions
├── idx.rs                    # Idx — 32-bit type handle (the canonical type reference)
├── tag.rs                    # Tag — 1-byte type kind discriminant
├── item.rs                   # Item — compact (Tag, u32) pair stored in pool
├── flags.rs                  # TypeFlags — pre-computed bitflags for O(1) queries
├── pool/                     # Unified type storage (SoA layout)
│   ├── mod.rs                    # Pool struct, queries, variable state management
│   ├── construct.rs              # Type construction methods (interning + dedup)
│   └── format.rs                 # Human-readable type formatting for diagnostics
├── unify/                    # Unification engine
│   ├── mod.rs                    # UnifyEngine — link-based union-find
│   ├── rank.rs                   # Rank system for let-polymorphism
│   └── error.rs                  # UnifyError, UnifyContext
├── infer/                    # Inference engine
│   ├── mod.rs                    # InferEngine — orchestrates inference
│   ├── expr.rs                   # infer_expr() — per-expression inference dispatch
│   └── env.rs                    # TypeEnv — Rc-based scope chain
├── check/                    # Module-level type checker
│   ├── mod.rs                    # ModuleChecker — multi-pass orchestration
│   ├── signatures.rs             # Pass 1: function signature collection
│   ├── bodies.rs                 # Pass 2-4: function/test/impl body checking
│   └── registration.rs          # Pass 0: type/trait/impl registration
├── registry/                 # Type, trait, and method registries
│   ├── mod.rs                    # Re-exports
│   ├── types.rs                  # TypeRegistry — struct/enum/newtype storage
│   ├── traits.rs                 # TraitRegistry — trait definitions and impls
│   └── methods.rs                # MethodRegistry — built-in method resolution
├── output/                   # Type checker output
│   └── mod.rs                    # TypedModule, FunctionSig, PatternResolution
└── type_error/               # Error infrastructure
    └── mod.rs                    # TypeCheckError, TypeErrorKind, ErrorContext
```

## Design Goals

1. **Sound type system** — No runtime type errors for well-typed programs
2. **Full inference** — Minimal type annotations required (function signatures only)
3. **Good error messages** — Rich context with origin tracking and suggestions
4. **Capability tracking** — Side effects tracked in function types
5. **Efficient representation** — Arena-allocated, interned, cache-friendly

## Architecture

The type system is organized into four layers:

```
┌─────────────────────────────────────────────────────┐
│ Pool (Unified Type Storage)                         │
│ ├─ items: Vec<Item>        (tag + data per type)    │
│ ├─ flags: Vec<TypeFlags>   (pre-computed metadata)  │
│ ├─ hashes: Vec<u64>        (for deduplication)      │
│ ├─ extra: Vec<u32>         (variable-length data)   │
│ ├─ intern_map: FxHashMap   (hash → Idx dedup)       │
│ └─ var_states: Vec<VarState> (type variable state)  │
├─────────────────────────────────────────────────────┤
│ Registries (Semantic Information)                    │
│ ├─ TypeRegistry  (structs, enums, aliases)          │
│ ├─ TraitRegistry (traits, implementations)          │
│ └─ MethodRegistry (unified method lookup)           │
├─────────────────────────────────────────────────────┤
│ InferEngine (Hindley-Milner Inference)              │
│ ├─ UnifyEngine   (union-find with path compression) │
│ ├─ TypeEnv       (name → scheme bindings)           │
│ └─ Error accumulation (comprehensive diagnostics)   │
├─────────────────────────────────────────────────────┤
│ ModuleChecker (Multi-Pass Type Checking)            │
│ ├─ Pass 0: Registration (types, traits, impls)      │
│ ├─ Pass 1: Function signatures                      │
│ ├─ Pass 2: Function bodies                          │
│ ├─ Pass 3: Test bodies                              │
│ └─ Pass 4: Impl method bodies                       │
└─────────────────────────────────────────────────────┘
```

## Type Checking Flow

```
ParseResult { Module, ExprArena }
    │
    │ create ModuleChecker
    ▼
ModuleChecker {
    pool: Pool,                  // Type storage
    types: TypeRegistry,         // User-defined types
    traits: TraitRegistry,       // Traits & implementations
    methods: MethodRegistry,     // Built-in methods
}
    │
    │ multi-pass type checking
    ▼
TypedModule {
    expr_types: Vec<Idx>,        // Type per expression
    functions: Vec<FunctionSig>, // Checked signatures
    types: Vec<TypeEntry>,       // Registered types
    errors: Vec<TypeCheckError>, // Accumulated errors
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
}
```

## Core Type Handle: Idx

Every type is represented as a 4-byte `Idx` — a transparent wrapper around `u32`. This is the canonical type reference used throughout the compiler.

```rust
#[repr(transparent)]
pub struct Idx(u32);  // Copy, Clone, Eq, PartialEq, Hash
```

Primitive types occupy fixed indices 0–11:

| Index | Type | Index | Type |
|-------|------|-------|------|
| 0 | `int` | 6 | `()` (unit) |
| 1 | `float` | 7 | `Never` |
| 2 | `bool` | 8 | `Error` |
| 3 | `str` | 9 | `Duration` |
| 4 | `char` | 10 | `Size` |
| 5 | `byte` | 11 | `Ordering` |

Indices 12–63 are reserved for future primitives. Dynamic types (functions, lists, user-defined) start at index 64.

## Type Kind: Tag

Each type has a 1-byte `Tag` discriminant organized by semantic range:

| Range | Category | Example Tags |
|-------|----------|-------------|
| 0–15 | Primitives | `Int`, `Float`, `Bool`, `Str`, `Unit`, `Never` |
| 16–31 | Simple containers | `List`, `Option`, `Set`, `Channel`, `Range` |
| 32–47 | Two-child containers | `Map`, `Result` |
| 48–79 | Complex types | `Function`, `Tuple`, `Struct`, `Enum` |
| 80–95 | Named types | `Named`, `Applied`, `Alias` |
| 96–111 | Type variables | `Var`, `BoundVar`, `RigidVar` |
| 112–127 | Type schemes | `Scheme` |
| 240–255 | Special | `Projection`, `ModuleNs`, `Infer`, `SelfType` |

## Pre-computed Metadata: TypeFlags

Every type carries a `TypeFlags` bitfield (u32) computed at construction time. This enables O(1) queries without traversal:

**Presence flags:** `HAS_VAR`, `HAS_ERROR`, `HAS_INFER`, `HAS_SELF`, `HAS_PROJECTION`
**Category flags:** `IS_PRIMITIVE`, `IS_CONTAINER`, `IS_FUNCTION`, `IS_COMPOSITE`
**Optimization flags:** `NEEDS_SUBST`, `IS_RESOLVED`, `IS_MONO`, `IS_COPYABLE`
**Capability flags:** `HAS_CAPABILITY`, `IS_PURE`, `HAS_IO`, `HAS_ASYNC`

Flags propagate from children to parents during construction via `PROPAGATE_MASK`, so checking whether a complex type contains any variables is O(1).

## ModuleChecker

The `ModuleChecker` orchestrates multi-pass type checking for a module:

```rust
pub struct ModuleChecker<'a> {
    arena: &'a ExprArena,
    interner: &'a StringInterner,

    pool: Pool,                             // Type storage
    types: TypeRegistry,                    // User-defined types
    traits: TraitRegistry,                  // Traits & impls
    methods: MethodRegistry,                // Method resolution

    import_env: TypeEnv,                    // Imported functions
    signatures: FxHashMap<Name, FunctionSig>,
    base_env: Option<TypeEnv>,              // Frozen after Pass 1

    expr_types: Vec<Idx>,                   // Output: type per expression
    errors: Vec<TypeCheckError>,            // Accumulated errors
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
}
```

### Multi-Pass Architecture

**Pass 0 — Registration:**
- 0a: Register built-in types (Ordering, etc.)
- 0b: Register user-defined types (structs, enums, newtypes)
- 0c: Register traits and implementations
- 0d: Register derived implementations
- 0e: Register config variables

**Pass 1 — Function Signatures:**
Collect all function signatures before checking bodies. This enables mutual recursion and forward references. The base environment is frozen after this pass.

**Pass 2 — Function Bodies:**
Type check each function body against its declared signature using `InferEngine`.

**Pass 3 — Test Bodies:**
Type check test function bodies (implicit `void` return type).

**Pass 4 — Impl Method Bodies:**
Type check implementation method bodies with `Self` type bound.

## Type Rules

### Literals

```
42      : int       3.14    : float
"hello" : str       true    : bool
'a'     : char      ()      : ()
[]      : [T]       5s      : Duration
```

### Binary Operations

```
int + int       → int       (primitive fast path)
float + float   → float     (primitive fast path)
str + str       → str       (concatenation)
int < int       → bool      (comparison)
T == T          → bool      (where T: Eq)
T + U           → T::Output (where T: Add<U>)
```

### Conditionals

```
if cond then t else e
  cond : bool
  t, e : T (branches unified)
  result : T
```

## Method Resolution

Method calls resolve through a three-level dispatch:

1. **Built-in methods** — Compiler-defined methods on primitive/container types (via `MethodRegistry`)
2. **Inherent methods** — `impl Type { ... }` blocks
3. **Trait methods** — `impl Trait for Type { ... }` blocks

The `MethodRegistry` stores built-in methods keyed by `(Tag, Name)` for O(1) lookup. Each built-in method declares its return type relationship to the receiver:

```rust
pub enum BuiltinMethodKind {
    Fixed(Idx),           // Fixed return type (e.g., len() → int)
    Element,              // Returns element type (e.g., list.first() → T?)
    Transform(MethodTransform),  // Transforms receiver type
}
```

## Salsa Compatibility

All exported types derive `Clone, Eq, PartialEq, Hash, Debug` for seamless integration with Salsa's memoization. Compile-time assertions verify compatibility:

```rust
assert_salsa_compatible!(Idx, Tag, TypeFlags, Rank);
assert_salsa_compatible!(TypedModule, FunctionSig, TypeCheckError);
```

## Related Documents

- [Pool Architecture](pool-architecture.md) — SoA storage, interning, type construction
- [Type Inference](type-inference.md) — InferEngine, expression inference
- [Unification](unification.md) — Union-find, rank system, occurs check
- [Type Environment](type-environment.md) — Scope chain, name resolution
- [Type Registry](type-registry.md) — User-defined types, traits, methods

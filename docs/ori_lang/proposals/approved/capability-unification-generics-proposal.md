# Proposal: Capability Unification & Generics Upgrade

**Status:** Approved
**Approved:** 2026-02-20
**Author:** Eric (with AI assistance)
**Created:** 2026-02-20
**Affects:** Compiler (all layers), type system, traits, capabilities, generics, grammar, spec, stdlib

## Errata (added 2026-02-24)

> **Monomorphization Architecture**: The monomorphization pipeline design for Phases 1-5 has been documented in `docs/ori_lang/v2026/design/monomorphization-architecture.md`. This design was informed by a study of Rust (`rustc`), Swift, Zig, and Lean 4 compilers. Key decision: a unified `GenericArg` enum (matching Rust's `GenericArgKind`) handles both type and const value substitution through all five phases. The current `MonoInstance.type_args: Vec<Idx>` will evolve to `MonoInstance.generic_args: Vec<GenericArg>` at the Phase 1-to-Phase 2 boundary. See the architecture document for full details including pipeline stages, ARC lowering integration, name mangling, and reference compiler comparison.

## Addendum (added 2026-03-04)

> **Decision: `:` replaces `with` for all structural bounds.** After reviewing prior art across 10 reference compilers (Rust, Swift, Kotlin, Haskell, Elm, Gleam, Roc, Koka, TypeScript, Zig), the `:` syntax was chosen over `with` for all structural capability positions: type declarations, generic bounds, where clauses, and supertrait declarations. Rationale:
>
> 1. **Universal familiarity** — Rust, Swift, and Kotlin all use `:` for bounds. Any developer from these languages reads `T: Comparable` instantly.
> 2. **Conciseness** — `T: Eq` (5 chars) vs `T with Eq` (9 chars). In complex signatures this compounds significantly.
> 3. **No migration for bounds** — `:` is already the compiler's syntax for generic bounds, where clauses, and supertraits. Only `#derive` → `:` on type declarations requires migration.
> 4. **`with` simplification** — `with` now has exactly ONE meaning: expression-level capability provision (`with Http = Mock in body`). No disambiguation rules needed.
>
> The `uses` keyword for environmental capabilities is unaffected. The conceptual unification remains: `:` = structural capabilities (what types *have*), `uses` = environmental capabilities (what functions *do*).
>
> | Before | After |
> |--------|-------|
> | `#derive(Eq, Hashable)` | `type Point: Eq, Hashable = { ... }` |
> | `T: Comparable` | `T: Comparable` (unchanged) |
> | `where T: Eq` | `where T: Eq` (unchanged) |
> | `trait Comparable: Eq` | `trait Comparable: Eq` (unchanged) |
> | `with Http = Mock in body` | `with Http = Mock in body` (unchanged) |
>
> **All examples in this proposal have been updated to reflect `:` syntax.** The original `with`-based framing is superseded.

---

## Summary

Establish a consistent vocabulary for capabilities — **`:`** for structural capabilities (what types *have*) and **`uses`** for environmental capabilities (what functions *do*) — replacing the disconnected `#derive` annotation with a grammar-level `:` clause on type declarations. Retain `:` for generic bounds, where clauses, and supertraits (already the existing syntax). Derive const generic eligibility from the capability model rather than a whitelist.

| Current | Proposed |
|---------|----------|
| `#derive(Eq, Hashable)` | `type Point: Eq, Hashable = { ... }` |
| `type Point = { x: int, y: int }` | `type Point: Eq, Hashable = { x: int, y: int }` |
| `T: Comparable` | `T: Comparable` (unchanged) |
| `where T: Eq, U: Clone` | `where T: Eq, U: Clone` (unchanged) |
| `impl<T: Eq> Eq for [T]` | `impl<T: Eq> Eq for [T]` (unchanged) |
| Const generics: `int` and `bool` only | Any type `: Eq, Hashable` |
| Three concepts (derive, bounds, effects) | Two concepts (`:` structural, `uses` environmental) |

This proposal is the foundation for Ori's generics upgrade (const generics, associated consts, const functions in type positions) and represents the language's conceptual unification of traits and effects under the term *capabilities*.

---

## Table of Contents

- [Part I: Vision & Motivation](#part-i-vision--motivation)
  - [1. The Problem: Three Vocabularies for One Concept](#1-the-problem-three-vocabularies-for-one-concept)
  - [2. The Model: with/uses](#2-the-model-withuses)
  - [3. Landscape Analysis](#3-landscape-analysis)
  - [4. Prior Art](#4-prior-art)
  - [5. What Makes Ori Unique](#5-what-makes-ori-unique)
- [Part II: Design](#part-ii-design)
  - [6. Grammar Changes](#6-grammar-changes)
  - [7. with on Type Declarations](#7-with-on-type-declarations)
  - [8. with on Generic Bounds](#8-with-on-generic-bounds)
  - [9. Const Generic Eligibility](#9-const-generic-eligibility)
  - [10. Associated Consts in Traits](#10-associated-consts-in-traits)
  - [11. Const Functions in Type Positions](#11-const-functions-in-type-positions)
  - [12. Interaction with Existing Features](#12-interaction-with-existing-features)
  - [13. Error Messages](#13-error-messages)
- [Part III: Implementation Phases](#part-iii-implementation-phases)
  - [14. Phase 1: with on Type Declarations](#14-phase-1-with-on-type-declarations)
  - [15. Phase 2: with on Generic Bounds](#15-phase-2-with-on-generic-bounds)
  - [16. Phase 3: Const Generic Eligibility](#16-phase-3-const-generic-eligibility)
  - [17. Phase 4: Associated Consts](#17-phase-4-associated-consts)
  - [18. Phase 5: Const Functions in Type Positions](#18-phase-5-const-functions-in-type-positions)
- [Part IV: Roadmap Impact](#part-iv-roadmap-impact)
  - [19. Cross-Cutting Roadmap Analysis](#19-cross-cutting-roadmap-analysis)
  - [20. Work That Should Be Pulled Forward](#20-work-that-should-be-pulled-forward)
  - [21. Suggested Roadmap Reordering](#21-suggested-roadmap-reordering)
  - [22. Risk Analysis](#22-risk-analysis)
- [Part V: Migration & Compatibility](#part-v-migration--compatibility)
  - [23. Spec Documents to Update](#23-spec-documents-to-update)
  - [24. Affected Proposals](#24-affected-proposals)
  - [25. Test Migration Plan](#25-test-migration-plan)
  - [26. Transition Period](#26-transition-period)
  - [27. Tooling Support](#27-tooling-support)
- [Part VI: Open Questions & Non-Goals](#part-vi-open-questions--non-goals)
  - [28. Open Questions](#28-open-questions)
  - [29. Honest Gaps](#29-honest-gaps)
  - [30. Non-Goals](#30-non-goals)
  - [31. Summary Table](#31-summary-table)

---

# Part I: Vision & Motivation

## 1. The Problem: Three Vocabularies for One Concept

Ori currently has three separate syntaxes for describing what entities can do:

### 1.1 Derive Annotations (`#derive`)

Types declare structural trait implementations via an annotation system:

```ori
#derive(Eq, Hashable, Comparable)
type Point = { x: int, y: int }
```

This uses `#derive(...)` syntax — an attribute bolted onto the type declaration from outside the grammar. The annotation tells the compiler to auto-generate method implementations (e.g., field-wise equality) from the type's structure.

### 1.2 Generic Bounds (`:`)

Generic type parameters declare required traits via colon syntax:

```ori
@sort<T: Comparable> (items: [T]) -> [T] = ...

@merge<T: Eq + Clone> (a: [T], b: [T]) -> [T]
    where T: Hashable
= ...
```

This uses `:` for inline bounds and `where T: Trait` for clause-form bounds. The colon says "T must implement this trait."

### 1.3 Environmental Capabilities (`uses`)

Functions declare required effects via the `uses` keyword:

```ori
@fetch (url: str) -> str uses Http = Http.get(url: url)

@cached_fetch (url: str) -> str uses Http, Cache = ...
```

This uses a purpose-built keyword with its own tracking infrastructure (`FxHashSet<Name>` in the type checker) separate from the trait registry.

### 1.4 Why Three Vocabularies Is Costly

These three mechanisms describe the same fundamental concept — **what an entity can do** — using three different syntaxes and conceptual frameworks:

| Mechanism | Syntax | Meaning | Infrastructure |
|-----------|--------|---------|----------------|
| Derive | `#derive(Eq)` | "This type *can* be compared" | Attribute parsing → derive strategy → eval/LLVM codegen |
| Bounds | `T: Comparable` | "This type *must be* comparable" | Generic param parsing → constraint checking |
| Capabilities | `uses Http` | "This function *can* talk to the network" | `uses` clause → `FxHashSet<Name>` tracking |

A new user must learn three mechanisms, three syntaxes, and three mental models. But the underlying question is always the same: **what capabilities does this entity have?**

### 1.5 The Cost of Inconsistent Vocabulary

The vocabulary inconsistency has concrete costs:

1. **Const generic eligibility needs a whitelist.** The current const-generics proposal restricts parameters to `int` and `bool` because there's no general way to ask "can this type be used as a compile-time value?" With a unified model, the answer is: "does it have `Eq + Hashable`?" — the same question the compiler needs to answer for any type used in a generic bound.

2. **Generic bounds and type declarations use different vocabularies.** `#derive(Eq)` on a type and `T: Eq` on a generic parameter mean the same thing ("Eq is available"), but they look completely different. A reader must map between two syntactic worlds.

3. **The `#derive` system is an annotation island.** It's the only significant use of `#attribute(...)` syntax in Ori's core type system. Every other type property (fields, variants, generics, where clauses) is part of the grammar. Derive stands alone as metadata bolted on top.

4. **Capabilities and traits have separate tracking.** The type checker uses `TraitRegistry` for trait dispatch and `FxHashSet<Name>` for capability tracking. Both answer the question "does this entity have X?" but through different code paths.

---

## 2. The Model: `:`/`uses`

This proposal introduces a unified model with two syntactic mechanisms:

### `:` — Structural Capabilities

"This entity **has** these capabilities, determined by its structure."

```ori
// On concrete types: compiler derives implementation from fields
type Point: Eq, Hashable = { x: int, y: int }

// On generic parameters: caller must guarantee the capability
@sort<T: Comparable> (items: [T]) -> [T] = ...

// On const generics: type is eligible because it has Eq + Hashable
type Color: Eq, Hashable = Red | Green | Blue;
@themed<$C: Color> () -> Style = ...
```

### `uses` — Environmental Capabilities

"This function **uses** these capabilities, provided by the caller's environment."

```ori
// On functions: declares required effects
@fetch (url: str) -> str uses Http = Http.get(url: url)

// Provided in scope
with Http = MockHttp in fetch(url: "/api/data")
```

### The Distinction

| | `:` (structural) | `uses` (environmental) |
|---|---|---|
| **Determined by** | The entity's shape (fields, structure) | The caller's context (environment) |
| **Who provides it** | The compiler (auto-derived) or the programmer (manual impl) | The caller (via `with...in` or `def impl`) |
| **Propagation** | None — local to the type | Through call chains (transitive) |
| **Mockable** | No (structural truth) | Yes (`with Http = Mock in`) |
| **Syntax position** | Type declarations, generic parameters, where clauses | Function signatures |
| **Example** | `type Point: Eq` / `T: Comparable` | `@fetch () uses Http` |

### `with` Is Exclusively for Expression-Level Capability Provision

The `with` keyword has exactly one meaning in Ori: expression-level capability provision.

```ori
with Http = MockHttp in fetch(url: "/api/data")
```

It always has the form `with Name = Expr in Expr`. There is no declaration-level `with` — type declarations and bounds use `:`.

### The Consistent Vocabulary

Both `:` and `uses` answer the same question: **what can this entity do?** The difference is *where the answer comes from*:

- Structural capabilities (`:`) come from the entity's definition — its fields, its structure
- Environmental capabilities (`uses`) come from the entity's context — its callers, its runtime

This is one concept (capabilities), two flavors (structural/environmental), two syntactic mechanisms (`:` for structural, `uses` for environmental). The compiler infrastructure remains appropriately separate — structural capabilities use the `TraitRegistry`, environmental capabilities use effect tracking — but the user-facing vocabulary is unified.

---

## 3. Landscape Analysis

No existing language combines general const generics, first-class effect tracking, and a unified conceptual model:

| Language | Const Generics | Effects | Unified Model | Ergonomics |
|----------|---------------|---------|---------------|------------|
| Lean 4 | Everything (dependent types) | Monads | No | Steep learning curve |
| Zig | Everything (comptime) | None | No | Good (but duck-typed) |
| Mojo | Everything (comptime) | Minimal | No | Good |
| Haskell | Promoted ADTs + type families | Monads/libraries | No | Notorious |
| Scala 3 | Literal singletons + match types | Experimental | No | Complex |
| Rust | `int`, `bool`, `char` (stable) — stuck | None | No | Good |
| Swift | Int only (just shipped) | async/throws | No | Good |
| Koka | None | Best (row-polymorphic) | No | Good |
| **Ori (proposed)** | **Any `Eq + Hashable` type** | **Capabilities** | **Yes** | **`:` + `uses`** |

### Key Observations

**Rust's const generics are stuck.** `generic_const_exprs` has been unstable since 2021. The team is now pursuing `min_generic_const_args` — a much narrower subset. Ori's "any type with `Eq, Hashable`" would be more powerful than both Rust's stable story *and* its planned extensions, while remaining type-safe (unlike Zig).

**Koka has the best effect system but no type-level computation.** Koka can track what functions do but not what size things are. It has no const generics, no type-level arithmetic, no compile-time computation.

**Nobody unifies traits and effects.** In every language, these are separate systems with separate syntax:
- Rust: `impl Trait` + no effects
- Haskell: type classes + monads (separate concepts, separate syntax)
- Koka: no traits + algebraic effects
- Scala 3: traits + experimental capabilities (separate systems)

---

## 4. Prior Art

### 4.1 Rust — `#[derive(...)]` + `:` bounds

Rust uses a proc-macro-based derive system (`#[derive(Eq, Hash)]`) and colon syntax for bounds (`T: Clone + Send`). These are completely separate mechanisms. Rust has no built-in effect tracking.

**What Ori takes:** The `:` bound syntax is retained (matching Rust). The key difference is replacing `#[derive]` attributes with a `:` trait clause on the type declaration itself.

**What Ori avoids:** Rust's derive is a macro system — arbitrary code execution at compile time. Ori's `:` trait clause is grammar-level, not macro-level.

### 4.2 Haskell — `deriving` + type classes

Haskell's `deriving (Eq, Show)` clause is part of the data declaration grammar:

```haskell
data Point = Point { x :: Int, y :: Int } deriving (Eq, Show)
```

**What Ori takes:** The idea that derivation should be part of the type declaration, not an annotation. Haskell's `deriving` is closer to Ori's `:` trait clause than Rust's `#[derive]`.

**What Ori avoids:** Haskell's `deriving` is a keyword specific to data declarations. Ori's `:` serves double duty — the same symbol used for both type declaration traits and generic bounds.

### 4.3 Zig — `comptime` parameters

Zig allows any type as a comptime parameter, including types themselves:

```zig
fn sort(comptime T: type, items: []T) []T { ... }
```

**What Ori takes:** The ambition of general const generics beyond just integers.

**What Ori avoids:** Duck typing. In Zig, if you pass a type that doesn't support `<`, the error appears deep inside the function body. In Ori, `T: Comparable` catches it at the call site.

### 4.4 Koka — Row-Polymorphic Effects

Koka tracks effects via row types:

```koka
fun fetch(url: string) : <http,exn> string { ... }
```

**What Ori takes:** First-class effect tracking as a core language feature, not a library.

**What Ori avoids:** Row-polymorphic formalism. Ori's capabilities are simpler (named traits with `uses`) at the cost of less formal compositionality.

### 4.5 Lean 4 — Type Classes for Everything

Lean 4 uses type classes as the universal mechanism for both traits and effects:

```lean
instance : BEq Point where
  beq a b := a.x == b.x && a.y == b.y
```

**What Ori takes:** The idea that one mechanism can handle both traits and effects.

**What Ori avoids:** Lean 4's approach requires dependent type theory. Ori achieves conceptual unification through keyword design (`with`/`uses`) without requiring the full power (and complexity) of dependent types.

### 4.6 Swift — Noncopyable Types + Value Generics

Swift recently shipped value generics (integers only) and has a constraint system using `:`:

```swift
struct Vector<T, let N: Int> { ... }
```

**What Ori takes:** Nothing directly. Swift's value generics are more limited than Ori's proposed model.

### 4.7 Scala 3 — Experimental Capabilities

Scala 3 has `CanThrow` as an experimental capability tracked via the type system, alongside its traditional trait system:

```scala
def parse(s: String)(using CanThrow[ParseError]): Int = ...
```

**What Ori takes:** The idea that capabilities can be trait-like. But Scala 3 doesn't unify them — traits and capabilities remain separate concepts.

---

## 5. What Makes Ori Unique

### 5.1 Const Generics That Are Both General and Type-Safe

Zig and Mojo let you use any type as a comptime parameter — but they're duck-typed. Pass the wrong thing and you get a compile error deep inside the implementation, not at the call site. Rust is type-safe but stuck on integers. Lean is both general and safe but requires dependent type theory.

Ori's rule — any type `: Eq, Hashable` — is general enough for real use (shapes, strings, enums, config structs) and type-safe (the compiler checks capability bounds at the call site, not inside the function body). It's the Goldilocks position: more powerful than Rust/Swift, more structured than Zig/Mojo, more accessible than Lean/Haskell.

### 5.2 First-Class Effect Tracking

Only Koka has a comparable built-in effect system. But Koka has no const generics — it can track what functions do but not what size things are. Ori would be the first language with both: track the shapes of your tensors AND the effects of your functions.

### 5.3 Consistent Vocabulary

This is the part no other language has. Every other language treats traits/type classes and effects as separate systems with separate syntax. Ori's `:`/`uses` vocabulary gives both a consistent framing: capabilities. Types have structural capabilities (`: Eq`). Functions have environmental capabilities (`uses Gpu`). Generic bounds are capability requirements (`T: Comparable`). Two mechanisms, one mental model — even though the compiler infrastructure remains appropriately separate (structural capabilities use `TraitRegistry`, environmental capabilities use effect tracking).

### 5.4 Honest Gaps

Ori would NOT match:

- **Lean 4 / Haskell on type-level expressiveness** — no HKTs, no type families, no full dependent types. You can't abstract over `Option`/`Result`/`List` as type constructors. This limits certain abstractions (no generic Functor/Monad).
- **Zig / Mojo on comptime generality** — types aren't values in Ori. You can't write `@create<$T: type>() -> T`. Zig can. This limits metaprogramming.
- **Koka on effect formalism** — Koka's row-polymorphic effects are mathematically more precise. Ori's capabilities are more practical but less formally grounded. You can't write a function generic over "any set of capabilities" — Ori's `uses` is a flat list, not a row variable.

### 5.5 Where Ori Would Differentiate

| Domain | Ori's Approach | Current Closest |
|--------|---------------|----------------|
| Shape-typed numeric code | Const list generics + compile-time shape checking | Dex (research, inactive) |
| Effect-tracked practical systems | Capabilities built into the type system, not a library | Koka (but no generics) |
| Vocabulary consistency | Two mechanisms (`:`/`uses`) instead of three syntaxes | No direct precedent |
| Const generic breadth with type safety | Any `Eq + Hashable` type, not just integers | Zig/Mojo (duck-typed) |

---

# Part II: Design

## 6. Grammar Changes

### 6.1 Type Declarations — Add `:` Trait Clause

**Current grammar:**

```ebnf
type_def    = "type" identifier [ generics ] [ where_clause ] "=" type_body [ ";" ] .
```

**Proposed grammar:**

```ebnf
type_def    = "type" identifier [ generics ] [ ":" trait_list ] [ where_clause ] "=" type_body [ ";" ] .
trait_list  = trait_ref { "," trait_ref } .
trait_ref   = type_path [ "(" assoc_bindings ")" ] .
```

The trait clause sits between generics and `where_clause`, before the `=`. This positions structural capabilities as part of the type's identity, after any type parameters but before any constraints on those parameters. Traits are comma-separated (matching Swift's `struct Point: Equatable, Hashable` pattern).

### 6.2 Generic Parameters — Unchanged

```ebnf
type_param  = identifier [ ":" bounds ] [ "=" type ] .
bounds      = type_path { "+" type_path } .
```

No change. `:` for bounds is already the existing syntax.

### 6.3 Where Clauses — Unchanged

```ebnf
where_clause    = "where" constraint { "," constraint } .
constraint      = type_constraint | const_constraint .
type_constraint = identifier [ "." identifier ] ":" bounds .
```

No change. `:` for constraints is already the existing syntax.

### 6.4 Trait Definitions — Unchanged

```ebnf
trait_def = "trait" identifier [ generics ] [ ":" bounds ] "{" { trait_item } "}" .
```

No change. `trait Comparable: Eq { ... }` is already the existing syntax.

### 6.5 Impl Blocks — Unchanged

```ebnf
impl_block = "impl" [ generics ] [ trait_path "for" ] type [ where_clause ] "{" { impl_item } "}" .
```

No change. Generic bounds in impl blocks already use `:`.

### 6.6 Expression-Level `with...in` — Unchanged

```ebnf
with_expr           = "with" capability_binding { "," capability_binding } "in" expression .
capability_binding  = identifier "=" expression .
```

No change. `with` is exclusively used for expression-level capability provision.

### 6.7 `#derive` Attribute — Removed

The `#derive(...)` attribute form is removed. The `:` trait clause on type declarations replaces it entirely.

### 6.8 Disambiguation

The `:` symbol appears in several contexts, all unambiguously parseable:

| Context | Pattern | Example |
|---------|---------|---------|
| Type declaration traits | `type Name [ generics ] ":" TraitList "="` | `type Point: Eq = { ... }` |
| Generic parameter bound | `Ident ":" Bounds` | `<T: Eq + Clone>` |
| Const generic type | `"$" Ident ":" Type` | `<$N: int>` |
| Where clause | `where Ident ":" Bounds` | `where T: Comparable` |
| Supertrait | `trait Name ":" Bounds` | `trait Comparable: Eq` |
| Let binding type | `let x ":" Type` | `let x: int = 5` |

The `$` sigil distinguishes const generics (`$N: int`) from trait bounds (`T: Eq`). On type declarations, the parser expects `:` after the optional generics — if present, it parses a comma-separated trait list until it hits `where` or `=`.

### 6.9 `with` Keyword Scope

The `with` keyword now has exactly one meaning in Ori: expression-level capability provision.

```ori
with Http = MockHttp in fetch(url: "/api")
```

This simplification eliminates the need for disambiguation rules between declaration-level and expression-level `with`.

---

## 7. `:` on Type Declarations

### 7.1 Basic Syntax

```ori
// Struct
type Point: Eq, Hashable = { x: int, y: int }

// Sum type
type Color: Eq, Hashable, Printable = Red | Green | Blue;

// Newtype
type UserId: Eq, Hashable = int;

// Multiple capabilities
type User: Eq, Hashable, Comparable, Clone, Debug = {
    id: int,
    name: str,
    email: str,
}
```

### 7.2 Semantics

`type T: Trait1, Trait2 = body` means:

1. **Validation**: The compiler checks that all fields of `T` implement `Trait1` and `Trait2`. If any field lacks a required trait, emit error E2032 ("field type does not implement required trait").

2. **Derivation**: The compiler generates method implementations for each listed trait using the same strategy-driven mechanism as the current `#derive` system:
   - `Eq` → field-wise equality (strategy: `ForEachField { Equals, AllTrue }`)
   - `Hashable` → FNV-1a hash combine over fields (strategy: `ForEachField { Hash, HashCombine }`)
   - `Comparable` → lexicographic field comparison (strategy: `ForEachField { Compare, Lexicographic }`)
   - `Clone` → field-wise clone (strategy: `CloneFields`)
   - `Default` → field-wise default (strategy: `DefaultConstruct`)
   - `Debug` → structural format with field names (strategy: `FormatFields { ... }`)
   - `Printable` → human-readable format (strategy: `FormatFields { ... }`)

3. **Registration**: The compiler registers impl entries in the `TraitRegistry`, exactly as `#derive` does today.

### 7.3 Supertrait Enforcement

When `:` includes a trait that requires a supertrait, the supertrait must also be listed:

```ori
// OK: Hashable requires Eq, and Eq is listed
type Point: Eq, Hashable = { x: int, y: int }

// ERROR E2029: Hashable requires Eq
type Point: Hashable = { x: int, y: int }
```

Error:
```
error[E2029]: `Hashable` requires supertrait `Eq`
  --> src/types.ori:1:13
   |
 1 | type Point: Hashable = { x: int, y: int }
   |             ^^^^^^^^ `Hashable` requires `Eq` to also be derived
   |
   = help: add `Eq` to the trait list: `type Point: Eq, Hashable`
```

### 7.4 Non-Derivable Traits

Only the 7 derivable traits can appear in a `:` trait clause on a type declaration:

| Trait | Derivable | Struct | Sum Type |
|-------|-----------|--------|----------|
| `Eq` | Yes | Yes | Yes |
| `Hashable` | Yes | Yes | Yes |
| `Comparable` | Yes | Yes | Yes |
| `Clone` | Yes | Yes | Yes |
| `Default` | Yes | Yes | No |
| `Debug` | Yes | Yes | Yes |
| `Printable` | Yes | Yes | Yes |

Attempting to use a non-derivable trait produces error E2033:

```ori
// ERROR E2033: Iterator cannot be derived
type MyIter: Iterator = { items: [int], pos: int }
```

```
error[E2033]: trait `Iterator` cannot be derived
  --> src/types.ori:1:14
   |
 1 | type MyIter: Iterator = { items: [int], pos: int }
   |              ^^^^^^^^ not derivable
   |
   = note: derivable traits: Eq, Hashable, Comparable, Clone, Default, Debug, Printable
   = help: implement `Iterator` manually: `impl Iterator for MyIter { ... }`
```

### 7.5 Generic Types with `:`

For generic types, `:` generates bounded implementations:

```ori
type Pair<T>: Eq, Clone, Debug = { first: T, second: T }

// Generates:
// impl<T: Eq> Eq for Pair<T> { ... }
// impl<T: Clone> Clone for Pair<T> { ... }
// impl<T: Debug> Debug for Pair<T> { ... }
```

This means `Pair<int>` has `Eq`, `Clone`, `Debug` (because `int` has all three), but `Pair<SomeOpaqueType>` only has the traits that `SomeOpaqueType` has.

### 7.6 Trait Clause Ordering

The `:` trait clause sits after generics, before `where`, before `=`:

```ori
type Matrix<T, $M: int, $N: int>: Eq, Clone
    where T: Eq + Clone
= {
    data: [T],
    rows: int,
    cols: int,
}
```

Order of elements: `type Name [generics] [: traits] [where constraints] = body`

---

## 8. Generic Bounds — Unchanged

Generic bound syntax is **not changed** by this proposal. The existing `:` syntax for bounds, where clauses, supertraits, and impl blocks is already the target syntax. Examples for reference:

### 8.1 Inline Bounds

```ori
@sort<T: Comparable> (items: [T]) -> [T] = ...
```

### 8.2 Multiple Bounds

```ori
@merge<T: Eq + Clone + Hashable> (a: [T], b: [T]) -> [T] = ...
```

### 8.3 Where Clauses

```ori
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default
= ...
```

### 8.4 Associated Type Constraints

```ori
@collect<I, C> (iter: I) -> C
    where I: Iterator, C: Collect, I.Item: Clone
= ...
```

### 8.5 Trait Bounds on Impl Blocks

```ori
impl<T: Printable> Printable for [T] {
    @to_str (self) -> str = ...;
}
```

### 8.6 Mixed Type and Const Parameters

```ori
@fill<T: Clone + Default, $N: int> () -> [T, max N]
    where N > 0
= for _ in 0..N yield T.default()
```

Both trait bounds (`T: Clone + Default`) and const parameter types (`$N: int`) use `:`. The `$` sigil disambiguates const generics from type parameters.

### 8.7 Reading the Syntax

The familiar `:` syntax reads naturally:

| Expression | Reading |
|------------|---------|
| `T: Comparable` | "T is Comparable" |
| `T: Eq + Clone` | "T is Eq and Clone" |
| `type Point: Eq` | "Point is Eq" (derives Eq) |
| `where T: Clone` | "where T is Clone" |
| `trait Hashable: Eq` | "Hashable extends Eq" |

---

## 9. Const Generic Eligibility

### 9.1 The Current Problem

The approved const-generics proposal restricts const generic types to `int` and `bool`:

> "Only `int` and `bool` are valid as const generic types."
> — const-generics-proposal.md

This is an arbitrary whitelist. The actual requirements for a type to work as a const generic parameter are:

1. **Equality** — the compiler must compare values to check type identity (`[int, max 5]` ≠ `[int, max 10]`)
2. **Hashing** — the compiler must hash values for type interning and monomorphization caching

These are exactly `Eq` and `Hashable`.

### 9.2 The Proposed Rule

**A type can be used as a const generic parameter if it has the `Eq` and `Hashable` capabilities.**

No whitelist. No special `ConstEligible` marker trait. Just: does the type have the capabilities the compiler needs?

```ori
// Primitives — all have Eq + Hashable, all const-eligible
@buffer<$N: int> () -> [byte, max N] = ...
@flag<$B: bool> () -> Config = ...
@label<$S: str> () -> Label = ...       // str: Eq, Hashable ✓
@code<$C: char> () -> Encoding = ...    // char: Eq, Hashable ✓

// User types — opt in by declaring : Eq, Hashable
type Color: Eq, Hashable = Red | Green | Blue;
@themed<$C: Color> () -> Style = ...    // Color: Eq, Hashable ✓

// Compound types — inherit capabilities from element types
@shaped<$S: [int]> () -> Tensor<float, S> = ...  // [int]: Eq, Hashable ✓

// Types without Eq + Hashable — NOT const-eligible
type Opaque = { data: [byte] }
@bad<$O: Opaque> () = ...  // ERROR: Opaque does not have Eq + Hashable
```

### 9.3 Error Messages

```
error[E1040]: type `Opaque` cannot be used as a const generic parameter
  --> src/main.ori:1:12
   |
 1 | @bad<$O: Opaque> () = ...
   |          ^^^^^^ not const-eligible
   |
   = note: const generic parameters require `Eq + Hashable`
   = note: `Opaque` does not implement `Eq` or `Hashable`
   = help: add `Eq, Hashable` to the type declaration:
   |        type Opaque: Eq, Hashable = { ... }
```

### 9.4 Why This Is Better Than a Whitelist

| Approach | `int` | `bool` | `str` | `char` | User enums | User structs | Lists |
|----------|-------|--------|-------|--------|------------|-------------|-------|
| Whitelist (`int`, `bool` only) | Yes | Yes | No | No | No | No | No |
| `Eq + Hashable` check | Yes | Yes | Yes | Yes | If declared | If declared | If elements are |

The whitelist approach forces every extension to be a language-level decision. The capability approach lets users opt in by declaring `: Eq, Hashable` on their types. New types become const-eligible without compiler changes.

### 9.5 Interaction with Current Const Generics

The current parser already accepts `$N: int` syntax. The change is:

1. **Expand allowed types** from `{int, bool}` to any type implementing `Eq + Hashable`
2. **Check at declaration site** that the type after `:` has `Eq + Hashable`
3. **Check at instantiation site** that the concrete value's type satisfies the bound

No changes to monomorphization, const evaluation, or const bounds — those remain as specified in the existing const-generics and const-generic-bounds proposals.

---

## 10. Associated Consts in Traits

### 10.1 Motivation

Traits can currently have associated types (`type Item`). The unified model extends this to associated consts:

```ori
trait Shaped {
    $rank: int;
    $shape: [int];
}
```

Associated consts are compile-time values that types provide as part of their trait implementation, just as associated types are runtime types that types provide.

### 10.2 Syntax

```ori
trait Shaped {
    $rank: int;                   // Required associated const
    $shape: [int];                // Required associated const
    $total: int = $product(Self.$shape);  // Default (computed from $shape)
}

impl Shaped for Matrix<float, 3, 4> {
    $rank = 2;
    $shape = [3, 4];
    // $total uses default: $product([3, 4]) = 12
}
```

### 10.3 Constraining Associated Consts

Associated consts can be constrained in `where` clauses:

```ori
@matrix_op<T: Shaped> (t: T) -> T
    where T.$rank == 2
= ...

@compatible<A: Shaped, B: Shaped> (a: A, b: B) -> bool
    where A.$shape == B.$shape
= true
```

### 10.4 Associated Const Bindings in `with` Clauses

When declaring a type with a trait that has associated consts, the values can be bound:

```ori
type Tensor<T: DType, $S: [int]>: Eq, Clone, Shaped($shape = S, $rank = $len(S))
= {
    data: [T],
}
```

This reads as: "Tensor is Eq, Clone, and Shaped where `$shape` is `S` and `$rank` is `$len(S)`."

### 10.5 This Is Phase 4

Associated consts require:
- Const evaluation infrastructure (existing proposal: const-evaluation-termination)
- Const expression unification in the type checker
- Const expression codegen in LLVM

This is substantial compiler work and is deliberately positioned as Phase 4.

---

## 11. Const Functions in Type Positions

### 11.1 Motivation

With associated consts and const generic types, it becomes natural to use const functions in type positions:

```ori
@reshape<T: DType, $FROM: [int], $TO: [int]> (
    t: Tensor<T, FROM>,
) -> Tensor<T, TO>
    where $product(FROM) == $product(TO)
= ...
```

### 11.2 Const Function Requirements

A function can be used in type positions if:
- It's marked as a const function (evaluable at compile time)
- All its parameters are const-eligible types
- Its return type is a const-eligible type

### 11.3 Built-in Const Functions

```ori
$len(list: [T]) -> int           // Length of a const list
$product(list: [int]) -> int     // Product of elements
$sum(list: [int]) -> int         // Sum of elements
$min(a: int, b: int) -> int     // Minimum
$max(a: int, b: int) -> int     // Maximum
```

### 11.4 Where Clause Examples

```ori
@reshape<$FROM: [int], $TO: [int]> (t: Tensor<float, FROM>) -> Tensor<float, TO>
    where $product(FROM) == $product(TO)
    where all(TO, d -> d > 0)
= ...

@concat<$A: [int], $B: [int]> (
    a: Tensor<float, A>,
    b: Tensor<float, B>,
) -> Tensor<float, $append(A, B)>
= ...
```

### 11.5 This Is Phase 5

Const functions in type positions require:
- Const function analysis (which functions are compile-time evaluable)
- Const expression evaluation in the type checker
- Unification of const expressions (`$product(FROM)` must unify with a concrete value)

This is the most complex phase and depends on all previous phases being complete.

---

## 12. Interaction with Existing Features

### 12.1 Manual Trait Implementations — Unchanged

`impl Trait for Type { ... }` is unaffected. `with` on types is specifically for auto-derivation. Manual implementations continue to work:

```ori
type Custom = { data: [byte] }

// Manual implementation — not derivable from fields
impl Eq for Custom {
    @eq (self, other: Custom) -> bool = self.data == other.data;
}
```

A type can have some traits from `:` (auto-derived) and others from `impl` (manual):

```ori
type Custom: Clone, Debug = { data: [byte] }

// Clone and Debug are auto-derived
// Eq is manually implemented
impl Eq for Custom {
    @eq (self, other: Custom) -> bool = custom_compare(a: self.data, b: other.data);
}
```

### 12.2 Capability Provision (`with...in`) — Unchanged

Expression-level capability provision is not affected:

```ori
with Http = MockHttp in fetch(url: "/api")
```

`with` is now exclusively used for this expression form. No disambiguation with type declarations needed.

### 12.3 Object Safety — Unchanged

Object safety rules remain the same. The `with` keyword on bounds doesn't change which traits are object-safe:

```ori
// Object-safe: no Self in return position
trait Printable {
    @to_str (self) -> str;
}

// NOT object-safe: Self in return position
trait Clone {
    @clone (self) -> Self;
}
```

### 12.4 Trait Dispatch Order — Unchanged

Method resolution order remains:
1. Inherent methods (`impl Type { ... }`)
2. Trait methods from explicit bounds
3. Trait methods from in-scope traits
4. Extension methods

### 12.5 Existential Types — Unchanged

```ori
@iter () -> impl Iterator<int>
@make_iter () -> impl Iterator<int> + Clone
```

No change. Existential type syntax doesn't use `:` bounds.

### 12.6 Extension Methods — Unchanged

```ori
extend<T: Printable> [T] {
    @show (self) -> void = print(msg: self.to_str());
}
```

No change. Extension method bounds already use `:`.

### 12.7 `capset` Declarations — Unchanged

```ori
capset Net = Http, Dns, Tls;
capset WebService = Net, Logger, Suspend;

@serve () -> void uses WebService = ...
```

Capsets are for environmental capabilities (`uses`), not structural capabilities (`with`). No change needed.

### 12.8 Default Implementations (`def impl`) — Unchanged

```ori
def impl Print {
    @write (text: str) -> void = _stdout_write(text: text);
}
```

Default implementations provide environmental capabilities. They are part of the `uses`/`with...in` system, not the `with`-on-types system.

### 12.9 The `uses` Keyword — Unchanged

```ori
@fetch (url: str) -> str uses Http = Http.get(url: url)
```

Environmental capabilities continue to use `uses`. No change to syntax, semantics, or tracking infrastructure.

---

## 13. Error Messages

### 13.1 Field Missing Required Trait (E2032)

```
error[E2032]: cannot derive `Eq` for `Container`
  --> src/types.ori:1:18
   |
 1 | type Container: Eq = { item: FileHandle }
   |                 ^^ `Eq` cannot be derived
   |                      ─────────────────── `FileHandle` does not implement `Eq`
   |
   = help: implement `Eq` for `FileHandle`, or remove `Eq` from the trait list
```

### 13.2 Non-Derivable Trait (E2033)

```
error[E2033]: trait `Iterator` cannot be derived
  --> src/types.ori:1:14
   |
 1 | type MyIter: Iterator = { ... }
   |              ^^^^^^^^ not derivable
   |
   = note: derivable traits: Eq, Hashable, Comparable, Clone, Default, Debug, Printable
   = help: implement `Iterator` manually: `impl Iterator for MyIter { ... }`
```

### 13.3 Supertrait Missing (E2029)

```
error[E2029]: `Hashable` requires supertrait `Eq`
  --> src/types.ori:1:13
   |
 1 | type Point: Hashable = { x: int, y: int }
   |             ^^^^^^^^ requires `Eq`
   |
   = help: add `Eq`: `type Point: Eq, Hashable = { ... }`
```

### 13.4 Default on Sum Type (E2028)

```
error[E2028]: cannot derive `Default` for sum type
  --> src/types.ori:1:13
   |
 1 | type Status: Default = Active | Inactive;
   |              ^^^^^^^ not derivable for sum types
   |
   = note: sum types have multiple variants; no unambiguous default
   = help: implement `Default` manually to specify which variant
```

### 13.5 Type Not Const-Eligible (E1040)

```
error[E1040]: type `Opaque` cannot be used as a const generic parameter
  --> src/main.ori:1:12
   |
 1 | @f<$O: Opaque> () = ...
   |        ^^^^^^ not const-eligible
   |
   = note: const generic parameters require `Eq` and `Hashable`
   = help: add to the type declaration: `type Opaque: Eq, Hashable = { ... }`
```

### 13.6 Missing Capability in Bound (E2020)

```
error[E2020]: `T` does not satisfy bound `Comparable`
  --> src/main.ori:3:12
   |
 1 | @process<T> (items: [T]) -> [T] = {
   |          - `T` declared here without bounds
   |
 3 |     sort(items: items)
   |          ^^^^^^^^^^^^^ `sort` requires `T: Comparable`
   |
   = help: add bound: `@process<T: Comparable>`
```

### 13.7 Old `#derive` Syntax Used (Migration Error)

```
error: `#derive` syntax has been replaced by `:` trait clause
  --> src/types.ori:1:1
   |
 1 | #derive(Eq, Hashable)
   | ^^^^^^^^^^^^^^^^^^^^^ old syntax
 2 | type Point = { x: int, y: int }
   |
   = help: use: `type Point: Eq, Hashable = { x: int, y: int }`
```

### ~~13.8 Old `:` Bound Syntax~~ — Removed

No migration error needed for `:` in bounds — `:` is retained as the bound syntax.

---

# Part III: Implementation Phases

## 14. Phase 1: `:` on Type Declarations

**Scope:** Replace `#derive(Trait)` with `type T: Trait = ...`

**Estimated impact:** Parser + IR + type checker registration. Evaluator and LLVM unchanged.

### 14.1 Parser Changes

1. **`parse_type_decl()`** — After parsing generics, check for `:` before `where`/`=`. Parse comma-separated trait names.
2. **`ParsedAttrs`** — Remove `derive_traits: Vec<Name>` field. Add `derive_traits: Vec<Name>` to the type declaration node instead.
3. **Remove** `parse_derive_attr()` — The `#derive(...)` attribute handler is no longer needed.
4. **Keep** `#derive(...)` as a migration error that suggests the `:` syntax.

### 14.2 IR Changes

1. **`TypeDef`** node — Add `derive_traits: Vec<DerivedTrait>` field (replacing attribute-sourced data).
2. **`DerivedTrait`** — Unchanged. The enum and strategy system remain as-is.
3. **Remove** derive-related fields from `ParsedAttrs`.

### 14.3 Type Checker Changes

1. **`register_derived_impls()`** — Change input source from `ParsedAttrs.derive_traits` to `TypeDef.derive_traits`. Processing logic unchanged.
2. **All validation** (E2028, E2029, E2032, E2033) — Unchanged. Only the source of trait names changes.

### 14.4 Evaluator Changes

None. The evaluator receives `DerivedMethodInfo` regardless of how the derive was declared.

### 14.5 LLVM Changes

None. The LLVM codegen receives `DerivedMethodInfo` regardless of how the derive was declared.

### 14.6 Test Changes

- Update all `#derive(...)` in spec tests to `:` syntax (~193 files)
- Add parser tests for new `:` trait clause syntax
- Add migration error tests for old `#derive` syntax
- Verify all existing derive tests pass with new syntax

### 14.7 Spec Changes

- Update `grammar.ebnf`: `type_def` production
- Update `06-types.md`: Derive section
- Update `07-properties-of-types.md`: All `#derive` examples
- Update `08-declarations.md`: All `#derive` examples
- Update `16-formatting.md`: `#derive` examples

---

## 15. Phase 2 — Eliminated

**Original scope:** Replace `T: Trait` with `T with Trait` in generic parameters and where clauses.

**Status:** This phase is **no longer needed**. The decision to retain `:` for bounds means generic parameters, where clauses, supertraits, and impl blocks all keep their existing syntax. No parser changes, no test migration, no spec updates for bound syntax.

The work originally allocated to Phase 2 (spec-wide syntax updates for ~28 proposals and ~100+ test files) is eliminated entirely.

---

## 16. Phase 3: Const Generic Eligibility

**Scope:** Replace the `{int, bool}` whitelist with "any type `: Eq, Hashable`" check.

**Estimated impact:** Type checker only. Small, targeted change.

### 16.1 Type Checker Changes

1. **`check_const_generic_type()`** — Replace hardcoded `matches!(type, Int | Bool)` with a trait registry lookup: "does this type have Eq and Hashable implementations?"
2. **Error message update** — E1040 now says "requires Eq + Hashable" instead of "only int and bool allowed."

### 16.2 New Const-Eligible Types

After this phase, the following types become const-eligible (assuming they have `Eq + Hashable`):

| Type | Currently | After Phase 3 |
|------|-----------|---------------|
| `int` | Eligible | Eligible |
| `bool` | Eligible | Eligible |
| `str` | Excluded | **Eligible** |
| `char` | Excluded | **Eligible** |
| `byte` | Excluded | **Eligible** |
| User enums `: Eq, Hashable` | Excluded | **Eligible** |
| User structs `: Eq, Hashable` | Excluded | **Eligible** |
| `[T]` where `T: Eq + Hashable` | Excluded | **Eligible** |
| Tuples of eligible types | Excluded | **Eligible** |

### 16.3 Spec Changes

- Update `const-generics-proposal.md` or supersede with this proposal
- Update `06-types.md`: Const generic section
- Update `grammar.ebnf`: `const_type` production (remove restriction)

---

## 17. Phase 4: Associated Consts

**Scope:** Add `$name: Type` syntax to trait definitions and impls.

**Estimated impact:** Parser, IR, type checker, evaluator, LLVM. Significant.

### 17.1 Prerequisites

- Const evaluation infrastructure (const-evaluation-termination proposal)
- Const expression unification in type checker
- Phases 1-3 complete

### 17.2 Parser Changes

1. **`parse_trait_item()`** — Accept `$name: Type` and `$name: Type = expr` as trait items.
2. **`parse_impl_item()`** — Accept `$name = expr` as impl items.

### 17.3 IR Changes

1. **`TraitItem`** — Add `AssocConst { name: Name, const_type: ParsedType, default: Option<ExprId> }`.
2. **`ImplItem`** — Add `AssocConst { name: Name, value: ExprId }`.

### 17.4 Type Checker Changes

1. **Trait registration** — Register associated consts alongside methods and associated types.
2. **Impl validation** — Verify associated const values match their declared types.
3. **Const expression unification** — When `T.$rank` appears in a where clause, resolve it via the trait registry and unify with the constraint.

### 17.5 Evaluator Changes

1. **Associated const resolution** — When evaluating `T.$rank`, look up the impl's associated const value.

### 17.6 LLVM Changes

1. **Const folding** — Associated consts are compile-time values; LLVM should inline them.

---

## 18. Phase 5: Const Functions in Type Positions

**Scope:** Allow `$product(S)`, `$len(S)` etc. in type positions and where clauses.

**Estimated impact:** Type checker, const evaluator. Most complex phase.

### 18.1 Prerequisites

- Phases 1-4 complete
- Const function analysis infrastructure
- Const expression evaluation in type checker

### 18.2 Const Function Identification

A function is const-evaluable if:
- All parameters are const-eligible types
- The body contains no effects (`uses` clause is empty)
- The body contains no mutable state
- The body terminates (checked by const evaluation termination analysis)

### 18.3 Type Checker Changes

1. **Const expression evaluation** — When a type position contains `$f(args)`, evaluate it at compile time and use the result for type checking.
2. **Const unification** — `$product(FROM)` in one position must unify with `$product(TO)` in another. This requires symbolic const expression comparison or eager evaluation.

### 18.4 Error Messages

```
error[E1045]: const expression mismatch in type position
  --> src/tensor.ori:5:5
   |
 3 | @reshape<$FROM: [int], $TO: [int]> (t: Tensor<float, FROM>) -> Tensor<float, TO>
   |                                                                               -- expected shape
 4 |     where $product(FROM) == $product(TO)
 5 |     = reshape_impl(t:)
   |       ^^^^^^^^^^^^^^^^ cannot verify that $product(FROM) == $product(TO) at this call site
   |
   = note: FROM = [2, 3] and TO = [3, 2] — $product([2, 3]) = 6, $product([3, 2]) = 6 ✓
```

---

# Part IV: Roadmap Impact

## 19. Cross-Cutting Roadmap Analysis

This proposal touches 6 roadmap sections directly and affects 4 more indirectly:

### 19.1 Section 3: Traits (DIRECT — Major Impact)

**Current status:** 70% complete. Core dispatch, derives, bounds, associated types all work.

**Impact:**
- **3.3 Trait Bounds** — Unchanged. `:` is retained for bounds.
- **3.5 Derived Traits** — `#derive` attribute replaced by `:` trait clause. Processing pipeline unchanged but input source changes (attribute → type declaration node).
- **3.1 Trait Declarations** — Supertrait syntax unchanged (`trait Foo: Bar` already correct).
- **3.4 Associated Types** — Existing feature. Phase 4 (associated consts) extends this pattern.
- **3.11 Object Safety** — Rules unchanged.
- **3.14 Comparable/Hashable** — These become the gatekeepers for const generic eligibility.

**Estimated effort:** Low (only `#derive` → `:` migration, no bound syntax changes).

### 19.2 Section 5: Type Declarations (DIRECT — Major Impact)

**Current status:** Evaluator complete, LLVM tests missing.

**Impact:**
- **5.4 Generic Types** — Bound syntax unchanged.
- **5.5 Compound Type Inference** — NOT STARTED. Lists, Maps, Sets, Tuples, Ranges have no type checker support. **This blocks Phase 3** (const generic eligibility for compound types like `[int]`).
- **5.7 Derive Attributes** — Replaced entirely by `:` trait clause.

**Estimated effort:** Low for syntax change; HIGH for 5.5 dependency.

### 19.3 Section 6: Capabilities (DIRECT — Conceptual Impact)

**Current status:** Core evaluator working. Composition, resolution, Unsafe pending.

**Impact:**
- **Conceptual reframing** — Capabilities become "environmental capabilities" under the unified model. No code changes, but all documentation reframed.
- **6.11 Composition** — `with...in` expression syntax unchanged. `with` is now exclusively for expression-level capability provision — no disambiguation needed.
- **6.2 Capability Traits** — These remain traits. The distinction is that capability traits are used with `uses` (environmental), while structural traits are declared with `:` (on types/bounds).

**Estimated effort:** Low (documentation and conceptual reframing, no code changes).

### 19.4 Section 15A: Attributes & Comments (DIRECT — Removal)

**Current status:** `#derive` is the primary attribute in the type system.

**Impact:**
- **`#derive` removal** — The `#derive(...)` attribute is removed, replaced by `:` trait clause on type declarations. Other attributes (`#test`, `#skip`, `#main`, `#cfg`) are unaffected.
- **`#[derive(...)]` bracket syntax** — Also removed (was kept for backward compatibility).
- **Parser** — Attribute parsing simplified; derive-specific handler removed.

**Estimated effort:** Low.

### 19.5 Section 18: Const Generics (DIRECT — Eligibility Change)

**Current status:** Parser done, type checking partial.

**Impact:**
- **18.1 Const Type Parameters** — Allowed types expand from `{int, bool}` to any type with `Eq + Hashable`.
- **18.0 Const Evaluation** — NOT STARTED. Needed for Phases 4-5 (associated consts, const functions).
- **Associated consts** — New feature added to traits (Phase 4).
- **Const functions in type positions** — New feature (Phase 5).

**Estimated effort:** Phase 3 (eligibility) is low. Phases 4-5 are high.

### 19.6 Section 19: Existential Types (INDIRECT — Bound Syntax)

**Current status:** Not started.

**Impact:**
- Bound syntax in `impl Trait + OtherTrait` positions unchanged (uses `+`, not `:`).
- Where clauses on existential return types change from `:` to `with`.

**Estimated effort:** Low.

### 19.7 Indirectly Affected Sections

- **Section 0 (Parser)** — Grammar changes for `with` in type declarations and generics.
- **Section 7A-D (Stdlib)** — All stdlib trait definitions and impls use bound syntax.
- **Section 8-9 (Patterns, Match)** — Exhaustiveness checking for sum types references trait bounds.
- **Section 21A (LLVM)** — No code changes, but LLVM test files reference trait syntax.

---

## 20. Work That Should Be Pulled Forward

### 20.1 Section 5.5: Compound Type Inference — CRITICAL

**Current state:** Entirely unimplemented. Lists, Maps, Sets, Tuples, Ranges all have evaluator support but NO type checker support.

**Why it blocks:** Phase 3 (const generic eligibility) needs `[int]: Eq, Hashable` to work. This requires the type checker to know that `[T]` has `Eq` when `T` has `Eq` — which requires compound type inference.

**Recommendation:** Pull forward to before Phase 3. Implement basic type inference for at least `[T]` (lists) and `(T, U)` (tuples), since these are the most common const generic compound types.

### 20.2 Section 18.0: Const Evaluation Foundations — IMPORTANT

**Current state:** Not started. Const evaluation termination analysis is specified but not implemented.

**Why it blocks:** Phases 4-5 (associated consts, const functions) require compile-time evaluation of expressions.

**Recommendation:** Pull forward to before Phase 4. Can be developed in parallel with Phases 1-3.

### 20.3 Section 6.11: Capability Composition — DESIRABLE

**Current state:** Not started. Multi-binding `with...in` syntax exists but validation is incomplete.

**Why it matters:** The proposal needs a clear understanding of how `with...in` (environmental) composes for documentation and error messages.

**Recommendation:** Complete during Phase 1, alongside the conceptual reframing.

---

## 21. Suggested Roadmap Reordering

### 21.1 Phase 1 Dependencies

```
Section 5.7 (Derive) ← REPLACED by Phase 1
Section 15A (Attributes) ← simplified by Phase 1
```

No blockers. Phase 1 can start immediately.

### 21.2 Phase 2 — Eliminated

Phase 2 (bound syntax migration from `:` to `with`) is no longer needed. `:` is retained for bounds.

### 21.3 Phase 3 Dependencies

```
Section 5.5 (Compound Type Inference) ← MUST COMPLETE FIRST
Section 3.14 (Comparable/Hashable) ← already complete
```

**Section 5.5 is the critical dependency.** It must be pulled forward.

### 21.4 Phase 4 Dependencies

```
Section 18.0 (Const Evaluation) ← MUST COMPLETE FIRST
Section 3.4 (Associated Types) ← already complete (pattern to follow)
```

**Section 18.0 is the critical dependency.** Can be developed in parallel with Phases 1-3.

### 21.5 Phase 5 Dependencies

```
Phase 4 (Associated Consts) ← MUST COMPLETE FIRST
Section 18 (Const Generics full) ← must be substantially complete
```

### 21.6 Proposed Timeline

```
Immediate:  Phase 1 (: on types, replacing #derive) + Section 5.5 (compound type inference)
            ↓
Then:       Phase 3 (const eligibility) + Section 18.0 (const evaluation) in parallel
            ↓
Later:      Phase 4 (associated consts)
            ↓
Future:     Phase 5 (const functions in type positions)
```

Phase 2 is eliminated — bound syntax already uses `:`.

---

## 22. Risk Analysis

### 22.1 Phase 1 Risks — LOW

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Parser ambiguity with `:` on type declarations | Low | Low | `:` after generics, before `where`/`=`; well-defined position |
| Test migration breaks tests | Medium | Low | Mechanical replacement; CI catches failures |
| Community confusion about syntax change | Low | Low | Migration errors guide users; `:` is universally familiar |

### 22.2 Phase 2 — Eliminated

Phase 2 risks are eliminated entirely because bound syntax is unchanged.

### 22.3 Phase 3 Risks — MEDIUM

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Compound type inference (5.5) takes longer than expected | Medium | High | Start 5.5 early; Phase 3 can wait |
| Performance regression from trait registry lookups vs. hardcoded whitelist | Low | Low | Cache eligibility results per type |
| User types as const generics expose monomorphization explosion | Medium | Medium | Warn on excessive instantiation count |

### 22.4 Phase 4 Risks — HIGH

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Const evaluation implementation is complex | High | High | Follow existing const-evaluation-termination proposal |
| Associated const unification in type checker is novel | High | High | Study Rust's approach (nightly `generic_const_exprs`) |
| Interaction with type inference is unpredictable | Medium | High | Conservative: require explicit annotations initially |

### 22.5 Phase 5 Risks — HIGH

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Const function analysis is hard to get right | High | High | Start with whitelisted built-in const functions |
| Symbolic const expression comparison is undecidable in general | High | High | Require explicit evaluation; no symbolic reasoning |
| This essentially approaches dependent types territory | Medium | High | Stay practical: no general dependent types, just const computation |

---

# Part V: Migration & Compatibility

## 23. Spec Documents to Update

| Document | Changes | Phase |
|----------|---------|-------|
| `grammar.ebnf` | `type_def` production (add `:` trait clause) | 1 |
| `06-types.md` | All `#derive` examples → `:` syntax, const generic section | 1, 3 |
| `07-properties-of-types.md` | All `#derive` examples → `:` syntax | 1 |
| `08-declarations.md` | `#derive` examples → `:` syntax | 1 |
| `14-capabilities.md` | Conceptual reframing | 1 |
| `16-formatting.md` | `#derive` examples → `:` syntax | 1 |
| `21-constant-expressions.md` | Const generic eligibility | 3 |
| `27-reflection.md` | `#derive(Reflect)` examples → `: Reflect` | 1 |

---

## 24. Affected Proposals

### 24.1 Proposals Superseded by This Proposal

| Proposal | Superseded Sections |
|----------|-------------------|
| `derived-traits-proposal.md` | `#derive` syntax (replaced by `:` trait clause). Derivation rules, field constraints, error messages remain valid. |
| `const-generics-proposal.md` | "Allowed Const Types" section (whitelist replaced by `Eq + Hashable` check). All other sections remain valid. |

### 24.2 Proposals Requiring Syntax Updates

Since bound syntax (`:`) is unchanged, only proposals containing `#derive(...)` need updating:

| Proposal | Update Needed |
|----------|--------------|
| `comparable-hashable-traits-proposal.md` | `#derive` examples → `:` trait clause |
| `clone-trait-proposal.md` | `#derive` examples → `:` trait clause |
| `debug-trait-proposal.md` | `#derive` examples → `:` trait clause |
| `reflection-api-proposal.md` | `#derive(Reflect)` → `: Reflect` |
| `capability-composition-proposal.md` | Documentation reframing only |
| `capset-proposal.md` | Documentation reframing only |
| `intrinsics-capability-proposal.md` | Documentation reframing only |

**No longer affected** (bound syntax unchanged): `drop-trait-proposal.md`, `additional-traits-proposal.md`, `formattable-trait-proposal.md`, `len-trait-proposal.md`, `iterator-traits-proposal.md`, `into-trait-proposal.md`, `index-trait-proposal.md`, `operator-traits-proposal.md`, `object-safety-rules-proposal.md`, `trait-resolution-conflicts-proposal.md`, `extension-methods-proposal.md`, `default-impl-proposal.md`, `default-impl-resolution-proposal.md`, `default-type-parameters-proposal.md`, `default-associated-types-proposal.md`, `associated-functions-proposal.md`, `existential-types-proposal.md`, `stateful-mock-testing-proposal.md`, `const-generic-bounds-proposal.md`, `sendable-channels-proposal.md`, `block-expression-syntax.md`.

---

## 25. Test Migration Plan

### 25.1 Phase 1: `#derive` → `:` (193+ files)

Mechanical replacement across all test files:

```
#derive(Eq)                        → (move to type declaration : clause)
#derive(Eq, Hashable)              → (move to type declaration : clause)
#derive(Eq, Hashable, Comparable)  → (move to type declaration : clause)
```

This is not a simple find-replace because `#derive` is on the line before the `type` declaration and must merge into it:

```ori
// Before
#derive(Eq, Clone)
type Point = { x: int, y: int }

// After
type Point: Eq, Clone = { x: int, y: int }
```

**Tooling needed:** A migration script that:
1. Finds `#derive(...)` lines
2. Extracts trait names
3. Removes the `#derive` line
4. Inserts `: Traits` into the following `type` declaration

### 25.2 Phase 2 — Eliminated

No bound syntax migration needed. `:` is retained for bounds.

### 25.3 Test File Counts by Category

| Category | Files | Phase |
|----------|-------|-------|
| `tests/spec/traits/derive/` | 12 | 1 |
| `tests/spec/declarations/` (attributes) | 5 | 1 |
| Files with `#derive` across all `tests/spec/` | ~193 | 1 |
| `compiler/oric/tests/` (Rust, derive-related) | ~20 | 1 |

---

## 26. Transition Period

### 26.1 Phase 1 Transition

During Phase 1, both `#derive` and `:` could be accepted temporarily:
- `#derive(Eq)` → accepted with deprecation warning pointing to `: Eq`
- `type Point: Eq = { ... }` → canonical form

**Recommendation:** No transition period. This is a pre-1.0 language. Make the change cleanly.

### 26.2 Phase 2 — Eliminated

No transition needed for bounds. `:` is retained as the bound syntax.

---

## 27. Tooling Support

### 27.1 Migration Script

A script (`scripts/migrate_derive_syntax.py` or similar) that:
1. Finds all `#derive(...)` annotations
2. Extracts trait names
3. Removes the `#derive` line
4. Inserts `: Traits` into the following `type` declaration

No bound syntax migration needed.

### 27.2 Editor Support

- LSP should auto-complete derivable trait names after `:` in type declarations
- LSP should show "available capabilities" on hover for types (structural) and functions (environmental)

### 27.3 `ori fmt`

- Format `:` trait clause on the same line as `type` if it fits
- Break to next line (indented) if the trait clause is long:

```ori
// Short — same line
type Point: Eq = { x: int, y: int }

// Long — next line
type User: Eq, Hashable, Comparable, Clone, Debug, Printable
= {
    id: int,
    name: str,
    email: str,
}
```

---

# Part VI: Open Questions & Non-Goals

## 28. Open Questions

### Q1: Should `:` on types allow non-derivable traits as markers?

**Question:** If `type Foo: Serializable` is written and `Serializable` isn't auto-derivable, should the compiler (a) error, (b) check for a manual impl, or (c) treat it as a constraint on the type?

**Current proposal:** Error (E2033). Only the 7 derivable traits are allowed in `:` trait clauses on type declarations.

**Alternative:** Allow any trait, with the compiler checking for a manual impl. This would make `:` truly mean "conforms to" rather than "derive." But it muddies the semantics — `:` would sometimes derive and sometimes just assert.

**Recommendation:** Start with derivable-only (simpler). Revisit if users request it.

### Q2: Should `:` on types support user-defined derivable traits?

**Question:** Can library authors create new derivable traits? E.g., `trait Serialize` with a derivation strategy that auto-generates from fields?

**Current proposal:** No. Only the 7 built-in derivable traits are supported.

**Future possibility:** A `#[derivable]` attribute on trait definitions, with a derivation strategy specified via a proc-macro-like mechanism. This is out of scope for this proposal.

### Q3: How do multi-param traits work in bounds?

**Question:** For traits with type parameters like `Add<int>`, how does the bound syntax read?

```ori
@f<T: Add<int>> (x: T) -> T.Output = ...
```

**Assessment:** Already works. `:` is the existing bound syntax. `T: Add<int>` parses unambiguously.

### Q4: Should `trait Foo: Bar` change? — RESOLVED

**Decision:** No. `trait Comparable: Eq` is retained. `:` is used for all structural capability declarations — type declarations, bounds, where clauses, and supertraits. This is consistent with Rust, Swift, and Kotlin.

### Q5: Can `:` appear in return type position?

**Question:** Does `@foo () -> T: Eq` make sense?

**Answer:** No. Return types describe what comes back, not what constraints it satisfies. Use `where` clauses or existential types instead:

```ori
@foo () -> impl Printable = ...
```

### Q6: How does `:` interact with trait objects?

**Question:** Ori removed the `dyn` keyword (see `remove-dyn-keyword-proposal.md`). Trait objects use `impl Trait` syntax. How do additional bounds work?

**Answer:** `impl Printable + Clone` (uses `+`). This is separate from `:` — `+` is for combining traits in existential/object position, `:` is for bounds on type parameters and type declarations.

### Q7: Interaction with `extend` blocks? — No Change

**Question:** Extension methods have generic bounds. Do they change?

**Answer:** No. Extension method bounds already use `:`:

```ori
extend<T: Printable> [T] { ... }
```

### Q8: Should capsets use `:`?

**Question:** Currently: `capset Net = Http, Dns, Tls`. Should this be `capset Net: Http, Dns, Tls`?

**Answer:** No. Capsets are aliases for environmental capability sets (used with `uses`). They don't declare structural capabilities on a type. The `=` syntax correctly expresses "Net is defined as the set {Http, Dns, Tls}."

### Q9: What about the Sendable auto-trait?

**Question:** `Sendable` is automatically derived by the compiler (not via `#derive`). How does it interact with `:`?

**Answer:** `Sendable` continues to be automatically computed by the compiler based on field types. It does not appear in `:` trait clauses on type declarations. Users can use it in bounds:

```ori
@spawn<T: Sendable> (task: () -> T) -> Future<T> = ...
```

### Q10: Does the `:` trait clause affect type identity?

**Question:** Are `type Point: Eq = { x: int, y: int }` and `type Point = { x: int, y: int }` the same type?

**Answer:** Yes. `:` generates trait implementations but does not change the type's identity. A `Point` is a `Point` regardless of which traits it derives.

### Q11: Ordering within `:` trait clause — does it matter?

**Question:** Is `type Point: Eq, Hashable` the same as `type Point: Hashable, Eq`?

**Answer:** Yes. Order is irrelevant, same as the current `#derive` behavior. Supertrait requirements are checked regardless of order.

### Q12: How does associated const syntax compose with `with` in bounds?

**Question:** For constraining associated consts:

```ori
@matrix_op<T: Shaped> (t: T) -> T
    where T.$rank == 2
```

Does `T.$rank` reference work in where clauses?

**Answer:** Yes. `T.$rank` is an associated const projection, analogous to `T.Item` for associated types. It resolves via the trait registry.

### Q13: What about `:` on function return types for documentation?

**Question:** Could `:` on return types serve as documentation?

```ori
@sort<T: Comparable> (items: [T]) -> [T]: Sorted
```

**Answer:** No. `: Sorted` on a return type is not a capability — it's an assertion about a property. This is better handled by post-conditions:

```ori
@sort<T: Comparable> (items: [T]) -> [T]
    post(result -> is_sorted(items: result))
```

### Q14: Migration of `Hashable without Eq` warning?

**Question:** Currently, deriving `Hashable` without `Eq` produces a warning (W0100). With `with`, this becomes:

```ori
type Foo: Hashable = { ... }  // Warning: Hashable without Eq
```

**Answer:** Same behavior. The warning is based on the trait list, not the syntax.

### Q15: How do compile-fail tests specify `with` errors?

**Question:** Compile-fail tests check for specific error codes. Do any error codes change?

**Answer:** Error codes are preserved. E2028, E2029, E2032, E2033 remain the same. Only error message text changes (e.g., "add `Eq` to the derive list" → "add `Eq` to the trait list").

---

## 29. Honest Gaps

This proposal deliberately does not provide:

1. **Higher-kinded types (HKTs):** You cannot write `T: Functor` where `Functor` abstracts over type constructors like `Option`, `Result`, `List`. This limits certain abstractions (no generic `map` over any container).

2. **Row-polymorphic effects:** Koka's effect system allows functions to be generic over "any set of effects." Ori's `uses` is a flat list of named capabilities. You cannot write `fn f<E>(x: () -E-> int)` where `E` is an effect variable.

3. **Types-as-values:** Zig's `comptime T: type` lets you pass types as values. Ori cannot. You cannot write `@create<$T: type>() -> T`.

4. **Full dependent types:** Lean 4 allows arbitrary term-level values in type positions. Ori restricts this to const-eligible types in const generic positions with const functions.

5. **Derivation strategies for user-defined traits:** Only the 7 built-in traits are auto-derivable. Library authors cannot define new derivation strategies.

These are deliberate scope limitations. Each could be a future proposal, but this proposal stays focused on the `with`/`uses` unification and its immediate consequences for the generics upgrade.

---

## 30. Non-Goals

1. **Changing how `uses` works.** Environmental capabilities are unchanged. This proposal only adds `:` trait clauses on type declarations and generalizes const generic eligibility.

2. **Changing `with...in` expression syntax.** The capability provision expression `with Http = Mock in body` is unchanged.

3. **Adding new derivable traits.** The set of 7 derivable traits is unchanged. Adding new ones (e.g., `Serialize`, `Deserialize`) is a separate future proposal.

4. **Changing trait dispatch order.** Method resolution remains: inherent → bounds → in-scope → extension.

5. **Implementing the full generics upgrade.** Phases 4-5 (associated consts, const functions in type positions) are future work described here for completeness but not implemented by this proposal. Only Phases 1-3 are proposed for immediate implementation.

6. **Backward compatibility with `#derive`**. As a pre-1.0 language, Ori does not maintain backward compatibility. The old `#derive` syntax is removed, with migration errors to guide users.

---

## 31. Summary Table

| Aspect | Current | Proposed | Phase |
|--------|---------|----------|-------|
| Derive on types | `#derive(Eq, Clone)` | `type T: Eq, Clone = { ... }` | 1 |
| Generic bounds | `T: Comparable` | `T: Comparable` (unchanged) | — |
| Where clauses | `where T: Eq` | `where T: Eq` (unchanged) | — |
| Supertrait syntax | `trait Foo: Bar` | `trait Foo: Bar` (unchanged) | — |
| Impl bounds | `impl<T: Eq>` | `impl<T: Eq>` (unchanged) | — |
| Const generic types | `int`, `bool` only | Any type `: Eq, Hashable` | 3 |
| Associated consts | Not supported | `$rank: int` in traits | 4 |
| Const fns in types | Not supported | `where $product(S) == N` | 5 |
| `uses` keyword | Environmental effects | Unchanged | — |
| `with...in` expression | Capability provision | Unchanged (only use of `with`) | — |
| Manual `impl` | `impl Trait for Type` | Unchanged | — |
| Trait dispatch | 4-tier resolution | Unchanged | — |
| 7 derivable traits | Eq, Hash, Cmp, Clone, Default, Debug, Print | Unchanged | — |
| `capset` | Named effect set | Unchanged | — |

---

## Supersedes

This proposal supersedes the following sections of existing proposals:

- **`derived-traits-proposal.md`** — `#derive(...)` syntax section. Derivation rules, field constraints, and error semantics remain valid.
- **`const-generics-proposal.md`** — "Allowed Const Types" section (`int` and `bool` whitelist). All other sections remain valid.

---

## Related Proposals

- **`derived-traits-proposal.md`** — Derivation rules and field constraints (semantics retained)
- **`const-generics-proposal.md`** — Const generic syntax and semantics (eligibility changed)
- **`const-generic-bounds-proposal.md`** — Where clause syntax for const bounds (unchanged)
- **`const-evaluation-termination-proposal.md`** — Compile-time evaluation limits (prerequisite for Phase 4)
- **`capability-composition-proposal.md`** — Environmental capability composition (unchanged, reframed)
- **`capset-proposal.md`** — Named capability sets (unchanged)
- **`comparable-hashable-traits-proposal.md`** — Eq + Hashable become const-eligibility gatekeepers
- **`block-expression-syntax.md`** — Block syntax (unaffected, sets the precedent for grammar-level changes)

---

## Origin

Discovered during design discussion (2026-02-20). The initial observation was that `#derive(Eq)` felt like it was "going around" Ori's capability system — granting abilities to types through a separate mechanism rather than through the capabilities model that is central to Ori's design philosophy.

Investigation revealed that while Eq and Http are mechanically different (structural vs. environmental), they are conceptually the same: capabilities that entities have. The `:`/`uses` split emerged as a clean way to express this distinction: `:` for structural capabilities (what types have), `uses` for environmental capabilities (what functions do).

The original proposal used `with` for structural capabilities, but after reviewing 10 reference compilers (2026-03-04), the decision was made to use `:` instead — matching the universal convention from Rust, Swift, and Kotlin. This simplified the proposal significantly: bound syntax, where clauses, and supertrait declarations all retain their existing `:` syntax. Only `#derive` → `:` trait clause on type declarations is a new change.

The const generic eligibility insight came from a side conversation: "what types can be const generic parameters?" is the same question as "what types have Eq + Hashable?" — and the capability model answers it without needing a whitelist.

The landscape analysis confirmed that no existing language combines general const generics, first-class effect tracking, and a consistent vocabulary for both. Ori's position — more powerful than Rust, more structured than Zig, more accessible than Lean, with effects that Koka lacks generics for — occupies a distinctive niche.

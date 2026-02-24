---
plan: "type_strategy_registry"
section: "06"
title: "Collection & Wrapper Type Definitions"
status: not-started
depends_on:
  - "01"  # Core Data Model Design (TypeTag, MethodDef, TypeDef, etc.)
  - "02"  # Crate Scaffolding & Purity Enforcement
blocks:
  - "08"  # Query API (needs all type defs)
  - "09"  # Wire Type Checker
  - "10"  # Wire Evaluator
  - "11"  # Wire ARC & Borrow Pass
  - "12"  # Wire LLVM Backend
estimated_lines: ~300
complexity: medium
---

# Section 06: Collection & Wrapper Type Definitions

## Purpose

Define the complete behavioral specification for all generic collection and wrapper
types: **List**, **Map**, **Set**, **Range**, **Tuple**, **Option**, **Result**.

These seven types are the `COLLECTION_TYPES` gap in `consistency.rs` (lines 13-25) --
the types with methods in the type checker (`ori_types`) and evaluator (`ori_eval`)
but **not** in the `ori_ir` builtin method registry. This section closes that gap by
declaring every method, its parameters, return type, ownership semantics, and trait
association as pure `const` data in `ori_registry`.

### Why This Is the Hardest Section

Unlike primitives (Section 03) and strings (Section 04), these types are **generic**.
A method like `List.first()` does not return a fixed type -- it returns `Option<T>`
where `T` is the list's element type. The registry's `ReturnSpec` must express these
relationships without importing any type-pool machinery. Section 01's data model must
provide the vocabulary; this section is the stress test.

## Current State Inventory

### Where Methods Are Defined Today

| Type | Typeck (`ori_types`) | Eval (`ori_eval`) | IR (`ori_ir`) |
|------|---------------------|-------------------|---------------|
| List | `resolve_list_method` (35 methods) | `EVAL_BUILTIN_METHODS` (13 methods) | Not present |
| Map | `resolve_map_method` (17 methods) | `EVAL_BUILTIN_METHODS` (10 methods) | Not present |
| Set | `resolve_set_method` (15 methods) | `EVAL_BUILTIN_METHODS` (6 methods) | Not present |
| Range | `resolve_range_method` (8 methods) | `EVAL_BUILTIN_METHODS` (3 methods) | Not present |
| Tuple | `resolve_tuple_method` (5 methods) | `EVAL_BUILTIN_METHODS` (6 methods) | Not present |
| Option | `resolve_option_method` (16 methods) | `EVAL_BUILTIN_METHODS` (12 methods) | Not present |
| Result | `resolve_result_method` (17 methods) | `EVAL_BUILTIN_METHODS` (11 methods) | Not present |

**Total: ~113 methods across 7 types, all absent from the IR registry.**

### Consistency Allowlists That Track These Gaps

From `compiler/oric/src/eval/tests/methods/consistency.rs`:

- `COLLECTION_TYPES` (line 13): Entire types skipped during IR consistency checks
- `TYPECK_METHODS_NOT_IN_EVAL` (line 374): 72 entries for collection types alone
  - `list`: 37 methods (lines 560-597)
  - `map`: 7 methods (lines 599-606)
  - `range`: 5 methods (lines 607-611)
  - `Set`: 9 methods (lines 460-468)
  - `Option`: 7 methods (lines 441-447)
  - `Result`: 10 methods (lines 449-458)

All of these allowlists will be **eliminated** once the registry is the single source
of truth.

---

## 06.1 The Generic Return Type Problem

### Problem Statement

Primitive methods return fixed types: `int.abs() -> int`, `str.len() -> int`. The
current `ReturnSpec` enum (defined in `ori_ir/src/builtin_methods/mod.rs`) has:

```rust
pub enum ReturnSpec {
    SelfType,        // returns same type as receiver
    Type(BuiltinType), // returns a specific fixed type
    Void,            // returns unit
    ElementType,     // returns the element type of the container
    OptionElement,   // returns Option<element>
    ListElement,     // returns [element]
    InnerType,       // returns the inner type (Option/Result unwrap)
}
```

This covers simple cases but **cannot express**:

| Method | Return Type | Missing From ReturnSpec |
|--------|-------------|------------------------|
| `Map.get(K)` | `Option<V>` | Option of **value** type, not element type |
| `Map.keys()` | `[K]` | List of **key** type |
| `Map.values()` | `[V]` | List of **value** type |
| `Map.entries()` | `[(K, V)]` | List of tuple of key and value |
| `Map.iter()` | `Iterator<(K, V)>` | Iterator of tuple |
| `List.iter()` | `DoubleEndedIterator<T>` | DEI, not plain Iterator |
| `Range.iter()` | `DoubleEndedIterator<T>` | DEI of element type |
| `List.enumerate()` | `[(int, T)]` | List of tuple with index |
| `List.zip(other)` | `[(T, U)]` | Tuple of two different element types |
| `Option.ok_or(E)` | `Result<T, E>` | Result wrapping inner type |
| `List.map(f)` | depends on closure | Higher-order return type |
| `Result.ok()` | `Option<T>` | Option of ok-type, not err-type |
| `Result.err()` | `Option<E>` | Option of err-type |

### Decision: Extended ReturnSpec

The registry's `ReturnSpec` (or its replacement in `ori_registry`) must grow to cover
these cases. The type checker still handles generic instantiation and unification --
the registry only needs to express the **shape** of the return type relationship.

#### Proposed Extensions

```rust
/// Return type specification for registry methods.
///
/// These describe the *relationship* between the return type and the
/// receiver's type parameters. The type checker maps these to concrete
/// `Idx` values; the registry never touches type pools.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ReturnSpec {
    // === Existing (from Section 01) ===
    /// Returns the same type as the receiver.
    SelfType,
    /// Returns a specific fixed builtin type.
    Fixed(TypeTag),
    /// Returns void/unit.
    Void,

    // === Generic container projections ===
    /// The element type of a single-param container (List<T> -> T, Set<T> -> T,
    /// Option<T> -> T, Iterator<T> -> T, Range<T> -> T, Channel<T> -> T).
    Element,
    /// The key type of a two-param container (Map<K, V> -> K).
    KeyType,
    /// The value type of a two-param container (Map<K, V> -> V).
    ValueType,
    /// The ok-type of Result<T, E> -> T.
    OkType,
    /// The err-type of Result<T, E> -> E.
    ErrType,

    // === Wrapped projections ===
    /// `Option<Element>` -- e.g., List.first(), List.get(), Map.get()
    OptionOf(TypeProjection),
    /// `[Element]` -- e.g., Map.keys(), Map.values()
    ListOf(TypeProjection),
    /// `Iterator<Element>` -- e.g., Set.iter()
    IteratorOf(TypeProjection),
    /// `DoubleEndedIterator<Element>` -- e.g., List.iter(), Range.iter()
    DoubleEndedIteratorOf(TypeProjection),
    /// `Result<T, E>` constructed from projections -- e.g., Option.ok_or()
    ResultOf { ok: TypeProjection, err: TypeProjection },

    // === Higher-order ===
    /// Return type depends on closure argument. The type checker resolves
    /// this via `unify_higher_order_constraints` in calls.rs. The registry
    /// uses `Fresh` to signal "type checker must infer via unification."
    /// Closure type inference logic stays in ori_types — it's behavioral,
    /// not declarative data.
    Fresh,

    // === Composite ===
    /// `(int, Element)` -- e.g., List.enumerate()
    TupleOfIntAndElement,
    /// `[(int, Element)]` -- List.enumerate() when on List
    ListOfTupleIntElement,
}

/// Which type parameter to project from the receiver.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TypeProjection {
    /// The single element type (T in List<T>, Option<T>, etc.)
    Element,
    /// The key type (K in Map<K, V>)
    Key,
    /// The value type (V in Map<K, V>)
    Value,
    /// The ok-type (T in Result<T, E>)
    Ok,
    /// The err-type (E in Result<T, E>)
    Err,
    /// A fixed type (not a projection)
    Fixed(TypeTag),
}

// NOTE: ClosureFlow was removed from the plan. Higher-order method type inference
// (how closure arguments constrain return types) is behavioral logic that belongs
// in the type checker's `unify_higher_order_constraints()`, not declarative data
// in the registry. Methods like map, flat_map, fold use `ReturnSpec::Fresh` to
// signal "the type checker must infer the return type via unification."
```

### Design Rationale

1. **No type pool dependency.** `ReturnSpec` and `TypeProjection` are
   plain enums -- `Copy + const`-constructible. The registry never allocates or
   touches `Idx`.

2. **Sufficient for the type checker.** `ori_types` maps each `ReturnSpec` variant
   to a concrete `Idx` using pool operations (`pool.option(elem)`, etc.). The mapping
   is mechanical and lives in one function in the wiring layer (Section 09).

3. **Sufficient for the evaluator.** `ori_eval` uses `ReturnSpec` to verify dispatch
   coverage. It does not need to resolve generic types -- the type checker already did.

4. **Sufficient for ARC/LLVM.** The ARC pass cares about `receiver: Ownership` (borrow
   vs owned). LLVM cares about operator strategies. Neither needs to resolve generic
   return types.

5. **Higher-order method inference stays in the type checker.** The type checker's
   `unify_higher_order_constraints()` (in `calls.rs` lines 700-764) handles
   map/flat_map/fold unification. The registry uses `ReturnSpec::Fresh` to signal
   "type checker must infer this." This keeps inference logic out of the pure-data
   registry.

---

## 06.2 List TypeDef

**Type:** `List<T>` (syntax: `[T]`)
**TypeTag:** `TypeTag::List`
**Memory:** `Arc` (always heap-allocated, reference-counted)
**Generics:** 1 type parameter (`T` = element type)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `len` | -- | `Fixed(Int)` | Borrow | -- | typeck, eval |
| `count` | -- | `Fixed(Int)` | Borrow | -- | typeck only |
| `is_empty` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `contains` | `(Element)` | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `first` | -- | `OptionOf(Element)` | Borrow | -- | typeck, eval |
| `last` | -- | `OptionOf(Element)` | Borrow | -- | typeck, eval |
| `get` | `(Int)` | `OptionOf(Element)` | Borrow | -- | typeck only |
| `pop` | -- | `OptionOf(Element)` | Borrow | -- | typeck only |
| `push` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `append` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `prepend` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `iter` | -- | `DoubleEndedIteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `reverse` | -- | `SelfType` | Borrow | -- | typeck only |
| `sort` | -- | `SelfType` | Borrow | -- | typeck only |
| `sorted` | -- | `SelfType` | Borrow | -- | typeck only |
| `unique` | -- | `SelfType` | Borrow | -- | typeck only |
| `flatten` | -- | `SelfType` | Borrow | -- | typeck only |
| `join` | `(Str)` | `Fixed(Str)` | Borrow | -- | typeck only |
| `enumerate` | -- | `ListOfTupleIntElement` | Borrow | -- | typeck only |
| `zip` | `(SelfType)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `map` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `filter` | `(Closure)` | `ClosureDriven(FilterLike)` | Borrow | -- | typeck only |
| `flat_map` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `fold` | `(Any, Closure)` | `ClosureDriven(FoldLike)` | Borrow | -- | typeck only |
| `reduce` | `(Closure)` | `ClosureDriven(ReduceLike)` | Borrow | -- | typeck only |
| `find` | `(Closure)` | `ClosureDriven(FindLike)` | Borrow | -- | typeck only |
| `any` | `(Closure)` | `ClosureDriven(PredicateLike)` | Borrow | -- | typeck only |
| `all` | `(Closure)` | `ClosureDriven(PredicateLike)` | Borrow | -- | typeck only |
| `for_each` | `(Closure)` | `ClosureDriven(ConsumerLike)` | Borrow | -- | typeck only |
| `take` | `(Int)` | `SelfType` | Borrow | -- | typeck only |
| `skip` | `(Int)` | `SelfType` | Borrow | -- | typeck only |
| `take_while` | `(Closure)` | `ClosureDriven(FilterLike)` | Borrow | -- | typeck only |
| `skip_while` | `(Closure)` | `ClosureDriven(FilterLike)` | Borrow | -- | typeck only |
| `chunk` | `(Int)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `window` | `(Int)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `min` | -- | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `max` | -- | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `min_by` | `(Closure)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `max_by` | `(Closure)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `sum` | -- | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `product` | -- | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `sort_by` | `(Closure)` | `SelfType` | Borrow | -- | typeck only |
| `group_by` | `(Closure)` | `ClosureDriven(...)` | Borrow | -- | typeck only |
| `partition` | `(Closure)` | `ClosureDriven(PartitionLike)` | Borrow | -- | typeck only |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Fixed(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |
| **Operator: Add** |
| `add` | `(SelfType)` | `SelfType` | Borrow | Add | eval only (typeck via ops) |
| `concat` | `(SelfType)` | `SelfType` | Borrow | -- | eval only |

### Rust Definition (Sketch)

```rust
pub const LIST: TypeDef = TypeDef {
    tag: TypeTag::List,
    name: "list",
    memory: MemoryStrategy::Arc,
    type_params: 1, // T = element
    operators: OpDefs {
        add: OpStrategy::RuntimeCall("ori_list_concat"),
        eq: OpStrategy::RuntimeCall("ori_list_eq"),
        cmp: OpStrategy::Unsupported,
        ..OpDefs::NONE
    },
    methods: &LIST_METHODS,
};

const LIST_METHODS: [MethodDef; 50] = [
    MethodDef::new("len", &[], ReturnSpec::Fixed(TypeTag::Int), Ownership::Borrow, None),
    MethodDef::new("is_empty", &[], ReturnSpec::Fixed(TypeTag::Bool), Ownership::Borrow, None),
    MethodDef::new("contains", &[ParamSpec::Element], ReturnSpec::Fixed(TypeTag::Bool),
                   Ownership::Borrow, None),
    MethodDef::new("first", &[], ReturnSpec::OptionOf(TypeProjection::Element),
                   Ownership::Borrow, None),
    MethodDef::new("last", &[], ReturnSpec::OptionOf(TypeProjection::Element),
                   Ownership::Borrow, None),
    MethodDef::new("iter", &[], ReturnSpec::DoubleEndedIteratorOf(TypeProjection::Element),
                   Ownership::Borrow, Some("Iterable")),
    MethodDef::new("clone", &[], ReturnSpec::SelfType, Ownership::Borrow, Some("Clone")),
    MethodDef::new("equals", &[ParamSpec::SelfType], ReturnSpec::Fixed(TypeTag::Bool),
                   Ownership::Borrow, Some("Eq")),
    MethodDef::new("compare", &[ParamSpec::SelfType], ReturnSpec::Fixed(TypeTag::Ordering),
                   Ownership::Borrow, Some("Comparable")),
    MethodDef::new("hash", &[], ReturnSpec::Fixed(TypeTag::Int),
                   Ownership::Borrow, Some("Hashable")),
    MethodDef::new("debug", &[], ReturnSpec::Fixed(TypeTag::Str),
                   Ownership::Borrow, Some("Debug")),
    // ... remaining methods
];
```

---

## 06.3 Map TypeDef

**Type:** `Map<K, V>` (syntax: `{K: V}`)
**TypeTag:** `TypeTag::Map`
**Memory:** `Arc`
**Generics:** 2 type parameters (`K` = key, `V` = value)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `len` | -- | `Fixed(Int)` | Borrow | -- | typeck, eval |
| `is_empty` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `get` | `(Key)` | `OptionOf(Value)` | Borrow | -- | typeck only |
| `contains_key` | `(Key)` | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `contains` | `(Key)` | `Fixed(Bool)` | Borrow | -- | typeck only |
| `insert` | `(Key, Value)` | `SelfType` | Borrow | -- | typeck only |
| `remove` | `(Key)` | `SelfType` | Borrow | -- | typeck only |
| `update` | `(Key, Value)` | `SelfType` | Borrow | -- | typeck only |
| `merge` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `keys` | -- | `ListOf(Key)` | Borrow | -- | typeck, eval |
| `values` | -- | `ListOf(Value)` | Borrow | -- | typeck, eval |
| `entries` | -- | `ListOfTupleKeyValue` | Borrow | -- | typeck only |
| `iter` | -- | `IteratorOfTupleKeyValue` | Borrow | Iterable | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |

### Map-Specific ReturnSpec Requirements

Map methods need `TypeProjection::Key` and `TypeProjection::Value` to distinguish
which type parameter they project. This is why `Map.keys()` uses `ListOf(Key)` while
`Map.values()` uses `ListOf(Value)`.

For `entries()` and `iter()`, the return type involves a tuple of key and value. Two
options:

**Option A: Add compound projections**
```rust
ReturnSpec::ListOfTuple(&[TypeProjection::Key, TypeProjection::Value])
ReturnSpec::IteratorOfTuple(&[TypeProjection::Key, TypeProjection::Value])
```

**Option B: Use a dedicated variant**
```rust
ReturnSpec::MapEntries,      // always [(K, V)]
ReturnSpec::MapIterator,     // always Iterator<(K, V)>
```

**Recommendation:** Option B. There are only two methods (entries, iter) that need
this pattern, and naming them explicitly is clearer than a generic tuple projection.
The type checker maps `MapEntries` to `pool.list(pool.tuple(&[key_ty, value_ty]))` --
a trivial translation.

---

## 06.4 Set TypeDef

**Type:** `Set<T>`
**TypeTag:** `TypeTag::Set`
**Memory:** `Arc`
**Generics:** 1 type parameter (`T` = element type)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `len` | -- | `Fixed(Int)` | Borrow | -- | typeck, eval |
| `is_empty` | -- | `Fixed(Bool)` | Borrow | -- | typeck only |
| `contains` | `(Element)` | `Fixed(Bool)` | Borrow | -- | typeck only |
| `insert` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `remove` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `iter` | -- | `IteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `union` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `intersection` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `difference` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `to_list` | -- | `ListOf(Element)` | Borrow | -- | typeck only |
| `into` | -- | `ListOf(Element)` | Borrow | Into | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck only |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |

### Note: Set.iter() Returns Iterator, Not DEI

Unlike List and Range, `Set.iter()` returns a plain `Iterator<T>`, not
`DoubleEndedIterator<T>`. This is because sets are unordered -- there is no
meaningful "back" to iterate from. The typeck source confirms this
(`resolve_set_method` line 579: `engine.pool_mut().iterator(elem)` vs
`resolve_list_method` line 475: `engine.pool_mut().double_ended_iterator(elem)`).

---

## 06.5 Range TypeDef

**Type:** `Range<T>`
**TypeTag:** `TypeTag::Range`
**Memory:** `Copy` (ranges are small value types: start + end + step)
**Generics:** 1 type parameter (`T` = element type, typically `int` or `float`)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `len` | -- | `Fixed(Int)` | Borrow | -- | typeck, eval |
| `count` | -- | `Fixed(Int)` | Borrow | -- | typeck only |
| `is_empty` | -- | `Fixed(Bool)` | Borrow | -- | typeck only |
| `contains` | `(Element)` | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `iter` | -- | `DoubleEndedIteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `to_list` | -- | `ListOf(Element)` | Borrow | -- | typeck only |
| `collect` | -- | `ListOf(Element)` | Borrow | -- | typeck only |
| `step_by` | `(Element)` | `SelfType` | Borrow | -- | typeck only |

### Special: Range<float> Iteration Guard

`Range<float>` rejects iteration methods (`iter`, `to_list`, `collect`). This is
enforced in `resolve_range_method` (lines 695-701):

```rust
let is_float = elem == Idx::FLOAT;
match method {
    "iter" | "to_list" | "collect" if is_float => None,
    // ...
}
```

The registry should capture this constraint. Two options:

**Option A: Conditional method availability (new MethodDef field)**
```rust
MethodDef {
    name: "iter",
    guard: Some(MethodGuard::ElementIsNot(TypeTag::Float)),
    // ...
}
```

**Option B: Document-only, type checker handles the guard**

The registry declares `iter` exists on Range. The type checker, when wiring to the
registry (Section 09), applies the Range<float> guard as a phase-specific concern
(since the guard depends on resolving the element type, which requires pool access).

**Recommendation:** Option B. The guard is inherently a type-checking concern --
it requires looking at the instantiated element type, which the pure-data registry
cannot do. The registry declares the method exists; the type checker conditionally
rejects it. The guard should be **documented** in the registry via a comment or a
`notes` field, but not encoded as executable logic.

---

## 06.6 Tuple TypeDef

**Type:** `(T1, T2, ..., Tn)` (variadic, 0-12 elements)
**TypeTag:** `TypeTag::Tuple`
**Memory:** Depends on contents (Copy if all elements Copy, otherwise stack/move)
**Generics:** Variadic (0-N type parameters)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `len` | -- | `Fixed(Int)` | Borrow | -- | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Fixed(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |

### Field Access: Not Methods

Tuple field access (`.0`, `.1`, etc.) is **not** a method -- it is a syntactic
construct handled by the parser and type checker. The registry does not need to
model it. From `resolve_tuple_method` (lines 868-877), only trait methods and `len`
are dispatched through the method resolution system.

### Memory Strategy: Structural

Tuples are unique among collection types: their memory strategy depends on their
contents. A `(int, bool)` is Copy; a `(str, [int])` contains Arc types. The registry
should declare `MemoryStrategy::Structural` -- a new variant meaning "determined by
the element types at instantiation time."

```rust
pub enum MemoryStrategy {
    Copy,         // value type, bitwise copy
    Arc,          // reference counted
    Structural,   // depends on contents (tuples, Option, Result)
}
```

---

## 06.7 Option TypeDef

**Type:** `Option<T>` (variants: `Some(T)` | `None`)
**TypeTag:** `TypeTag::Option`
**Memory:** `Structural` (Copy if T is Copy, otherwise move/clone)
**Generics:** 1 type parameter (`T` = inner type)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `is_some` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `is_none` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `unwrap` | -- | `Element` | Own | -- | typeck, eval |
| `expect` | `(Str)` | `Element` | Own | -- | typeck only |
| `unwrap_or` | `(Element)` | `Element` | Own | -- | typeck, eval |
| `ok_or` | `(Any)` | `ResultOf(Element, FreshVar)` | Own | -- | typeck, eval |
| `iter` | -- | `IteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `map` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `and_then` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `flat_map` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `filter` | `(Closure)` | `SelfType` | Borrow | -- | typeck only |
| `or` | `(SelfType)` | `SelfType` | Own | -- | typeck only |
| `or_else` | `(Closure)` | `ClosureDriven(...)` | Own | -- | typeck only |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Fixed(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |

### Ownership Semantics

Methods like `unwrap`, `expect`, `unwrap_or`, `or` consume the Option (ownership
transfer). Methods like `is_some`, `is_none`, `map`, `filter` borrow. This is
critical for the ARC pass: borrowing methods do not decrement the refcount.

---

## 06.8 Result TypeDef

**Type:** `Result<T, E>` (variants: `Ok(T)` | `Err(E)`)
**TypeTag:** `TypeTag::Result`
**Memory:** `Structural`
**Generics:** 2 type parameters (`T` = ok-type, `E` = err-type)

### Methods

| Method | Params | Returns | Receiver | Trait | Implemented In |
|--------|--------|---------|----------|-------|----------------|
| `is_ok` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `is_err` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `unwrap` | -- | `OkType` | Own | -- | typeck, eval |
| `expect` | `(Str)` | `OkType` | Own | -- | typeck only |
| `unwrap_or` | `(OkType)` | `OkType` | Own | -- | typeck only |
| `unwrap_err` | -- | `ErrType` | Own | -- | typeck only |
| `expect_err` | `(Str)` | `ErrType` | Own | -- | typeck only |
| `ok` | -- | `OptionOf(Ok)` | Borrow | -- | typeck only |
| `err` | -- | `OptionOf(Err)` | Borrow | -- | typeck only |
| `map` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `map_err` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `and_then` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `or_else` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `has_trace` | -- | `Fixed(Bool)` | Borrow | -- | typeck, eval |
| `trace` | -- | `Fixed(Str)` | Borrow | -- | typeck, eval |
| `trace_entries` | -- | `ClosureDriven(...)` | Borrow | -- | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Fixed(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Fixed(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Fixed(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Fixed(Str)` | Borrow | Debug | typeck, eval |

### Result-Specific Projections

Result is the only type with **two distinct projections** -- ok-type and err-type --
that both appear in method signatures. This drives the need for `TypeProjection::Ok`
and `TypeProjection::Err` in the data model.

`Result.ok()` returns `Option<T>` (wrapping the ok-type), while `Result.err()` returns
`Option<E>` (wrapping the err-type). Without separate projections, these two methods
would be indistinguishable.

`Result.map_err()` is particularly interesting: it transforms the **error** type while
preserving the **ok** type. This is the inverse of `Result.map()`. The registry
declares `map_err` with `ReturnSpec::Fresh` and the type checker's existing
`unify_higher_order_constraints` handles the inference (which type parameter
the closure transforms).

---

## 06.9 Handling Generic Types in the Registry

### Summary of Required Type-Projection Vocabulary

From the method tables above, the registry needs to express these return type patterns:

| Pattern | Example | ReturnSpec |
|---------|---------|------------|
| Fixed type | `List.len() -> int` | `Fixed(TypeTag::Int)` |
| Same as receiver | `List.reverse() -> [T]` | `SelfType` |
| Element type | `Option.unwrap() -> T` | `Element` |
| Key type | (not needed standalone for methods) | `KeyType` |
| Value type | (not needed standalone for methods) | `ValueType` |
| Ok type | `Result.unwrap() -> T` | `OkType` |
| Err type | `Result.unwrap_err() -> E` | `ErrType` |
| Option of element | `List.first() -> Option<T>` | `OptionOf(Element)` |
| Option of ok | `Result.ok() -> Option<T>` | `OptionOf(Ok)` |
| Option of err | `Result.err() -> Option<E>` | `OptionOf(Err)` |
| Option of value | `Map.get(K) -> Option<V>` | `OptionOf(Value)` |
| List of element | `Set.to_list() -> [T]` | `ListOf(Element)` |
| List of key | `Map.keys() -> [K]` | `ListOf(Key)` |
| List of value | `Map.values() -> [V]` | `ListOf(Value)` |
| Iterator of element | `Set.iter() -> Iterator<T>` | `IteratorOf(Element)` |
| DEI of element | `List.iter() -> DEI<T>` | `DoubleEndedIteratorOf(Element)` |
| Map entries | `Map.entries() -> [(K, V)]` | `MapEntries` |
| Map iterator | `Map.iter() -> Iterator<(K, V)>` | `MapIterator` |
| Enumerate | `List.enumerate() -> [(int, T)]` | `ListOfTupleIntElement` |
| Closure-driven | `List.map(f) -> [U]` | `ClosureDriven(MapLike)` |
| Result construction | `Option.ok_or(e) -> Result<T, E>` | Special |

### What We Do NOT Need

1. **TypeTag::Element, TypeTag::KeyType, TypeTag::ValueType** -- No. These are
   `ReturnSpec` / `TypeProjection` concepts, not type tags. TypeTag identifies the
   *kind* of a type (Int, List, Map, etc.), not a projection from a generic parameter.

2. **Full type algebra in the registry** -- No. The registry expresses the *shape* of
   the return type relationship. The type checker materializes it into `Idx` values
   using pool operations.

3. **Closure parameter types** -- No (mostly). The registry marks a parameter as
   `ParamSpec::Closure` without specifying `(T) -> U`. The type checker infers the
   closure type from context and uses `unify_higher_order_constraints`.

### TypeTag::SelfType

`SelfType` is already in `ReturnSpec` and means "the return type equals the receiver
type." This is correct for `List.reverse()`, `Option.clone()`, etc. No new TypeTag
needed.

### TypeTag::Iterator

Not a TypeTag concern -- `IteratorOf(Element)` in `ReturnSpec` handles this. The type
checker maps it to `pool.iterator(elem)`.

---

## 06.10 Cross-Reference & Validation

### Method Count by Type

| Type | Typeck Methods | Eval Methods | Delta | Registry Target |
|------|---------------|-------------|-------|----------------|
| List | 50 | 13 | 37 unimplemented | 50 |
| Map | 17 | 10 | 7 unimplemented | 17 |
| Set | 15 | 6 | 9 unimplemented | 15 |
| Range | 8 | 3 | 5 unimplemented | 8 |
| Tuple | 6 | 6 | 0 | 6 |
| Option | 16 | 12 | 7 unimplemented | 16 |
| Result | 17 | 11 | 10 unimplemented | 17 |
| **Total** | **129** | **61** | **75** | **129** |

### Allowlist Entries Eliminated by This Section

| Allowlist | Current Entries for Collection Types | After Section 06 |
|-----------|-------------------------------------|------------------|
| `COLLECTION_TYPES` | 11 entries (entire list) | Reduced by 7 (list, map, Set, range, tuple, Option, Result) |
| `TYPECK_METHODS_NOT_IN_EVAL` (collection portion) | 72 entries | **Eliminated** (tracked by registry status) |

Note: `COLLECTION_TYPES` also contains `Channel`, `DoubleEndedIterator`, `Iterator`,
and `error`. Channel is covered in Section 05 (Compound Types). Iterator/DEI are
covered in Section 07. Error is covered in Section 05. Once all sections are complete,
`COLLECTION_TYPES` is **fully eliminated**.

### Consistency Test Impact

The consistency tests in `consistency.rs` that currently skip `COLLECTION_TYPES`:

- `eval_primitive_methods_in_ir` (line 119): Will include collection types after registry wiring
- `typeck_primitive_methods_in_ir` (line 650): Will include collection types after registry wiring

### Source File Cross-Reference

| File | Lines Affected | Change |
|------|---------------|--------|
| `ori_types/src/infer/expr/methods.rs` | `resolve_list_method` (465-500) | Replaced by registry lookup (Section 09) |
| `ori_types/src/infer/expr/methods.rs` | `resolve_option_method` (502-525) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods.rs` | `resolve_result_method` (527-549) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods.rs` | `resolve_map_method` (551-572) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods.rs` | `resolve_set_method` (574-587) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods.rs` | `resolve_range_method` (689-707) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods.rs` | `resolve_tuple_method` (868-877) | Replaced by registry lookup |
| `ori_types/src/infer/expr/calls.rs` | `unify_higher_order_constraints` (700-764) | Stays in type checker (inference logic, not registry data) |
| `ori_eval/src/methods/helpers/mod.rs` | `EVAL_BUILTIN_METHODS` (collection entries) | Replaced by registry enumeration |
| `oric/src/eval/tests/methods/consistency.rs` | `COLLECTION_TYPES` (13-25) | **Eliminated** |
| `oric/src/eval/tests/methods/consistency.rs` | `TYPECK_METHODS_NOT_IN_EVAL` (374-633, collection portion) | **Eliminated** |

---

## Implementation Checklist

### 06.1 Data Model Extensions (prerequisite: Section 01 finalized)

- [ ] Add `MemoryStrategy::Structural` variant
- [ ] Add `TypeProjection` enum (Element, Key, Value, Ok, Err, Fixed)
- [ ] Extend `ReturnSpec` with generic projections (OptionOf, ListOf, IteratorOf, etc.)
- [ ] Add `ReturnSpec::Fresh` variant for higher-order methods (closure inference stays in type checker)
- [ ] Add `MapEntries` and `MapIterator` ReturnSpec variants
- [ ] Add `ListOfTupleIntElement` ReturnSpec variant
- [ ] Extend `ParamSpec` with `Element`, `Key`, `Value` for typed generic params
- [ ] Verify all variants are `const`-constructible (no allocations)
- [ ] Write unit tests for each new enum variant's Debug output

### 06.2 List TypeDef

- [ ] Define `LIST` const with all 50 methods
- [ ] Verify method names match `TYPECK_BUILTIN_METHODS` entries for `"list"`
- [ ] Verify trait methods match `EVAL_BUILTIN_METHODS` entries for `"list"`
- [ ] Document which methods use `ReturnSpec::Fresh` (higher-order inference)
- [ ] Test: `find_method(TypeTag::List, "len")` returns expected MethodDef
- [ ] Test: `LIST.methods.len() == 50`

### 06.3 Map TypeDef

- [ ] Define `MAP` const with all 17 methods
- [ ] Verify key/value projection usage is correct for each method
- [ ] Test: `find_method(TypeTag::Map, "get")` returns `OptionOf(Value)`
- [ ] Test: `find_method(TypeTag::Map, "keys")` returns `ListOf(Key)`

### 06.4 Set TypeDef

- [ ] Define `SET` const with all 15 methods
- [ ] Verify `iter` returns `IteratorOf` (not DEI)
- [ ] Test: `find_method(TypeTag::Set, "iter")` returns `IteratorOf(Element)`

### 06.5 Range TypeDef

- [ ] Define `RANGE` const with all 8 methods
- [ ] Document Range<float> iteration guard (not encoded in registry)
- [ ] Verify `iter` returns `DoubleEndedIteratorOf` (not plain Iterator)
- [ ] Test: `find_method(TypeTag::Range, "iter")` returns `DoubleEndedIteratorOf(Element)`

### 06.6 Tuple TypeDef

- [ ] Define `TUPLE` const with all 6 methods
- [ ] Set `MemoryStrategy::Structural`
- [ ] Document that field access (._0, ._1) is NOT modeled as methods
- [ ] Test: `TUPLE.methods.len() == 6`

### 06.7 Option TypeDef

- [ ] Define `OPTION` const with all 16 methods
- [ ] Set `MemoryStrategy::Structural`
- [ ] Verify ownership semantics: unwrap/expect/unwrap_or = Own, others = Borrow
- [ ] Test: `find_method(TypeTag::Option, "unwrap")` has `Ownership::Own`
- [ ] Test: `find_method(TypeTag::Option, "is_some")` has `Ownership::Borrow`

### 06.8 Result TypeDef

- [ ] Define `RESULT` const with all 17 methods
- [ ] Set `MemoryStrategy::Structural`
- [ ] Verify ok/err projection distinction: ok() -> OptionOf(Ok), err() -> OptionOf(Err)
- [ ] Verify `map_err` uses `ReturnSpec::Fresh` (closure inference in type checker)
- [ ] Test: `find_method(TypeTag::Result, "ok")` returns `OptionOf(Ok)`
- [ ] Test: `find_method(TypeTag::Result, "err")` returns `OptionOf(Err)`

### 06.9 Integration

- [ ] All 7 TypeDefs compile with `cargo c -p ori_registry`
- [ ] All TypeDefs are included in `BUILTIN_TYPES` array
- [ ] Total method count across all collection types matches expected (129)
- [ ] No duplicate method names within a single TypeDef
- [ ] All method names are sorted alphabetically within each TypeDef
- [ ] Purity test passes (no dependencies, no logic, all const)

---

## Exit Criteria

1. **Seven TypeDef constants** (`LIST`, `MAP`, `SET`, `RANGE`, `TUPLE`, `OPTION`,
   `RESULT`) are defined in `ori_registry` with complete method tables.

2. **Every method** currently in `TYPECK_BUILTIN_METHODS` for these types has a
   corresponding `MethodDef` in the registry. Count verification:
   - list: 50, map: 17, Set: 15, range: 8, tuple: 6, Option: 16, Result: 17

3. **ReturnSpec** is expressive enough to represent all return type patterns found in
   `resolve_*_method()` functions, including generic projections and higher-order flows.

4. **`MemoryStrategy::Structural`** exists for types whose memory strategy depends on
   their contents (Tuple, Option, Result).

5. **`type_params` field** on each TypeDef matches the arity: List=1, Map=2, Set=1,
   Range=1, Tuple=variadic, Option=1, Result=2.

6. **`cargo c -p ori_registry`** passes with no warnings.

7. **Unit tests** verify lookup by name for at least 3 representative methods per type.

---

## Open Questions for Section 01

These questions must be resolved in Section 01 (Core Data Model) before this section
can be implemented:

1. **How does `TypeDef` represent type parameter arity?** A simple `type_params: u8`
   field? Or something richer like `type_params: &[TypeParamDef]` with names?

2. ~~**Where does `ClosureFlow` live?**~~ **Resolved:** ClosureFlow was removed from scope. Higher-order method inference stays in the type checker. Methods use `ReturnSpec::Fresh`.

3. **How does `ReturnSpec::ResultOf` work?** For `Option.ok_or(E) -> Result<T, E>`,
   we need to construct a Result from the inner type (T) and a fresh/argument type (E).
   Does this require a new variant, or can it be expressed as a combination?

4. **`ParamSpec` for generic parameters:** Should `insert` on Map take
   `ParamSpec::Key, ParamSpec::Value` or just `ParamSpec::Any, ParamSpec::Any`? The
   former is more precise; the latter is simpler.

5. **Variadic tuples:** `type_params: u8` doesn't work for tuples (variable arity).
   Special case with `type_params: TypeParamArity::Variadic`?

6. ~~**`map_err` ClosureFlow**~~ **Resolved:** ClosureFlow removed. `map_err` uses `ReturnSpec::Fresh`; the type checker handles which type parameter the closure transforms.

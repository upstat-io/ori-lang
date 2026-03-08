---
plan: "type_strategy_registry"
section: "06"
title: "Collection & Wrapper Type Definitions"
status: complete
depends_on:
  - "01"  # Core Data Model Design (TypeTag, MethodDef, TypeDef, etc.)
  - "02"  # Crate Scaffolding & Purity Enforcement
blocks:
  - "08"  # Query API (needs all type defs)
  - "09"  # Wire Type Checker
  - "10"  # Wire Evaluator
  - "11"  # Wire ARC & Borrow Pass
  - "12"  # Wire LLVM Backend
estimated_lines: ~800
complexity: medium
---

# Section 06: Collection & Wrapper Type Definitions

## Purpose

Define the complete behavioral specification for all generic collection and wrapper
types: **List**, **Map**, **Set**, **Range**, **Tuple**, **Option**, **Result**, **Channel**.

These eight types are the `COLLECTION_TYPES` gap in `consistency.rs` (lines 13-25) --
the types with methods in the type checker (`ori_types`) and evaluator (`ori_eval`)
but **not** in the `ori_ir` builtin method registry. This section closes that gap by
declaring every method, its parameters, return type, ownership semantics, and trait
association as pure `const` data in `ori_registry`.

**Channel** was moved here from Section 05 (Compound Types) because it is structurally
a generic container — `Channel<T>` with `TypeParamArity::Fixed(1)`, using the same
single-child-in-data pool pattern as Option, Set, and Range. Its methods return
`Option<T>` via `ReturnTag::OptionOf(TypeProjection::Element)`, the same projection
machinery as the other generic types in this section.

### Why This Is the Hardest Section

Unlike primitives (Section 03) and strings (Section 04), these types are **generic**.
A method like `List.first()` does not return a fixed type -- it returns `Option<T>`
where `T` is the list's element type. The registry's `ReturnTag` must express these
relationships without importing any type-pool machinery. Section 01's data model must
provide the vocabulary; this section is the stress test.

**File size note:** List has 57 methods at ~10 lines per `MethodDef` literal = ~570 lines, exceeding the 500-line limit. Split `defs/list.rs` into `defs/list/mod.rs` (TypeDef, OpDefs) + `defs/list/methods.rs` (methods array). Same pattern may apply to Map (22+ methods).

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
| Channel | `resolve_channel_method` (9 methods) | Not present | Not present |

**Total: ~122 methods across 8 types, all absent from the IR registry.**

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
current `ReturnSpec` enum in ori_ir (defined in `ori_ir/src/builtin_methods/mod.rs`)
has a limited set of variants. **Note:** The registry uses `ReturnTag` (not
`ReturnSpec`) as the canonical name — see Section 01.5 for the frozen definition.
The ori_ir `ReturnSpec` is the legacy type being replaced. The current ori_ir enum:

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

| Method | Return Type | Missing From legacy ReturnSpec (ori_ir) |
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

### Decision: Extended ReturnTag (canonical name — see Section 01.5)

The Section 01 `ReturnTag` enum has been extended with all the variants needed for
generic container methods. **The canonical name is `ReturnTag`** — not `ReturnSpec`
or `ReturnType`. The `TypeProjection` enum is also defined in Section 01.

The following variants from Section 01 handle the collection cases:

```rust
// From Section 01.5 ReturnTag (relevant variants for collections):
ReturnTag::SelfType,                       // list.clone() -> [T]
ReturnTag::Concrete(TypeTag),              // list.len() -> int
ReturnTag::Unit,                           // list.for_each() -> void
ReturnTag::ElementType,                    // option.unwrap() -> T
ReturnTag::KeyType,                        // (reserved for map key access)
ReturnTag::ValueType,                      // (reserved for map value access)
ReturnTag::OkType,                         // result.unwrap() -> T
ReturnTag::ErrType,                        // result.unwrap_err() -> E
ReturnTag::OptionOf(TypeProjection),       // list.first() -> Option<T>
ReturnTag::ListOf(TypeProjection),         // map.keys() -> [K]
ReturnTag::IteratorOf(TypeProjection),     // set.iter() -> Iterator<T>
ReturnTag::DoubleEndedIteratorOf(TypeProjection), // list.iter() -> DEI<T>
ReturnTag::ListKeyValue,                   // map.entries() -> [(K, V)]
ReturnTag::MapIterator,                    // map.iter() -> Iterator<(K, V)>
ReturnTag::ListOfTupleIntElement,          // list.enumerate() -> [(int, T)]
ReturnTag::Fresh,                          // list.map(f) -> closure-driven

// TypeProjection (from Section 01.5):
TypeProjection::Element,   // T in List<T>, Option<T>, etc.
TypeProjection::Key,       // K in Map<K, V>
TypeProjection::Value,     // V in Map<K, V>
TypeProjection::Ok,        // T in Result<T, E>
TypeProjection::Err,       // E in Result<T, E>
TypeProjection::Fixed(TypeTag), // a known concrete type
```

**NOTE:** ClosureFlow was removed from the plan. Higher-order method type inference
(how closure arguments constrain return types) is behavioral logic that belongs
in the type checker's `unify_higher_order_constraints()`, not declarative data
in the registry. Methods like map, flat_map, fold use `ReturnTag::Fresh` to
signal "the type checker must infer the return type via unification."

### Design Rationale

1. **No type pool dependency.** `ReturnTag` and `TypeProjection` are
   plain enums -- `Copy + const`-constructible. The registry never allocates or
   touches `Idx`.

2. **Sufficient for the type checker.** `ori_types` maps each `ReturnTag` variant
   to a concrete `Idx` using pool operations (`pool.option(elem)`, etc.). The mapping
   is mechanical and lives in one function in the wiring layer (Section 09).

3. **Sufficient for the evaluator.** `ori_eval` uses `ReturnTag` to verify dispatch
   coverage. It does not need to resolve generic types -- the type checker already did.

4. **Sufficient for ARC/LLVM.** The ARC pass cares about `receiver: Ownership` (borrow
   vs owned). LLVM cares about operator strategies. Neither needs to resolve generic
   return types.

5. **Higher-order method inference stays in the type checker.** The type checker's
   `unify_higher_order_constraints()` (in `calls.rs` lines 700-764) handles
   map/flat_map/fold unification. The registry uses `ReturnTag::Fresh` to signal
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
| `len` | -- | `Concrete(Int)` | Borrow | -- | typeck, eval |
| `count` | -- | `Concrete(Int)` | Borrow | -- | typeck only |
| `is_empty` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `contains` | `(Element)` | `Concrete(Bool)` | Borrow | -- | typeck, eval |
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
| `join` | `(Str)` | `Concrete(Str)` | Borrow | -- | typeck only |
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
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Concrete(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |
| **Operator: Add** |
| `add` | `(SelfType)` | `SelfType` | Borrow | Add | eval only (typeck via ops) |
| `concat` | `(SelfType)` | `SelfType` | Borrow | -- | eval only |

### Rust Definition (Sketch)

```rust
pub const LIST: TypeDef = TypeDef {
    tag: TypeTag::List,
    name: "list",
    type_params: TypeParamArity::Fixed(1), // T = element
    memory: MemoryStrategy::Arc,
    operators: OpDefs {
        add: OpStrategy::RuntimeCall { fn_name: "ori_list_concat", returns_bool: false },
        eq:  OpStrategy::RuntimeCall { fn_name: "ori_list_eq", returns_bool: true },
        neq: OpStrategy::RuntimeCall { fn_name: "ori_list_ne", returns_bool: true },
        // No ordering operators for List
        ..OpDefs::UNSUPPORTED
    },
    methods: &LIST_METHODS,
};

// Note: MethodDef::new() is a sketch convenience constructor taking the 5 most-used fields.
// Implementation MUST use struct literals with ALL 10 MethodDef fields per frozen decision 13.
// All list methods share these defaults:
//   pure: true, backend_required: true, kind: MethodKind::Instance,
//   dei_only: false, dei_propagation: DeiPropagation::NotApplicable
const LIST_METHODS: [MethodDef; 50] = [
    MethodDef::new("len", &[], ReturnTag::Concrete(TypeTag::Int), Ownership::Borrow, None),
    MethodDef::new("is_empty", &[], ReturnTag::Concrete(TypeTag::Bool), Ownership::Borrow, None),
    MethodDef::new("contains", &[ParamDef { name: "value", ty: ReturnTag::ElementType, ownership: Ownership::Borrow }], ReturnTag::Concrete(TypeTag::Bool),
                   Ownership::Borrow, None),
    MethodDef::new("first", &[], ReturnTag::OptionOf(TypeProjection::Element),
                   Ownership::Borrow, None),
    MethodDef::new("last", &[], ReturnTag::OptionOf(TypeProjection::Element),
                   Ownership::Borrow, None),
    MethodDef::new("iter", &[], ReturnTag::DoubleEndedIteratorOf(TypeProjection::Element),
                   Ownership::Borrow, Some("Iterable")),
    MethodDef::new("clone", &[], ReturnTag::SelfType, Ownership::Borrow, Some("Clone")),
    MethodDef::new("equals", &[ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }], ReturnTag::Concrete(TypeTag::Bool),
                   Ownership::Borrow, Some("Eq")),
    MethodDef::new("compare", &[ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }], ReturnTag::Concrete(TypeTag::Ordering),
                   Ownership::Borrow, Some("Comparable")),
    MethodDef::new("hash", &[], ReturnTag::Concrete(TypeTag::Int),
                   Ownership::Borrow, Some("Hashable")),
    MethodDef::new("debug", &[], ReturnTag::Concrete(TypeTag::Str),
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
| `len` | -- | `Concrete(Int)` | Borrow | -- | typeck, eval |
| `is_empty` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `get` | `(Key)` | `OptionOf(Value)` | Borrow | -- | typeck only |
| `contains_key` | `(Key)` | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `contains` | `(Key)` | `Concrete(Bool)` | Borrow | -- | typeck only |
| `insert` | `(Key, Value)` | `SelfType` | Borrow | -- | typeck only |
| `remove` | `(Key)` | `SelfType` | Borrow | -- | typeck only |
| `update` | `(Key, Value)` | `SelfType` | Borrow | -- | typeck only |
| `merge` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `keys` | -- | `ListOf(Key)` | Borrow | -- | typeck, eval |
| `values` | -- | `ListOf(Value)` | Borrow | -- | typeck, eval |
| `entries` | -- | `ListKeyValue` | Borrow | -- | typeck only |
| `iter` | -- | `MapIterator` | Borrow | Iterable | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |

### Map-Specific ReturnTag Requirements

Map methods need `TypeProjection::Key` and `TypeProjection::Value` to distinguish
which type parameter they project. This is why `Map.keys()` uses `ListOf(Key)` while
`Map.values()` uses `ListOf(Value)`.

For `entries()` and `iter()`, the return type involves a tuple of key and value. Two
options:

**Option A: Add compound projections**
```rust
ReturnTag::ListKeyValue      // [(K, V)]
ReturnTag::MapIterator       // Iterator<(K, V)>
```

**Option B: Use a dedicated variant**
```rust
ReturnTag::ListKeyValue,     // always [(K, V)]
ReturnTag::MapIterator,      // always Iterator<(K, V)>
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
| `len` | -- | `Concrete(Int)` | Borrow | -- | typeck, eval |
| `is_empty` | -- | `Concrete(Bool)` | Borrow | -- | typeck only |
| `contains` | `(Element)` | `Concrete(Bool)` | Borrow | -- | typeck only |
| `insert` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `remove` | `(Element)` | `SelfType` | Borrow | -- | typeck only |
| `iter` | -- | `IteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `union` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `intersection` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `difference` | `(SelfType)` | `SelfType` | Borrow | -- | typeck only |
| `to_list` | -- | `ListOf(Element)` | Borrow | -- | typeck only |
| `into` | -- | `ListOf(Element)` | Borrow | Into | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck only |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |

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
| `len` | -- | `Concrete(Int)` | Borrow | -- | typeck, eval |
| `count` | -- | `Concrete(Int)` | Borrow | -- | typeck only |
| `is_empty` | -- | `Concrete(Bool)` | Borrow | -- | typeck only |
| `contains` | `(Element)` | `Concrete(Bool)` | Borrow | -- | typeck, eval |
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
| `len` | -- | `Concrete(Int)` | Borrow | -- | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Concrete(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |

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
| `is_some` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `is_none` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `unwrap` | -- | `Element` | Owned | -- | typeck, eval |
| `expect` | `(Str)` | `Element` | Owned | -- | typeck only |
| `unwrap_or` | `(Element)` | `Element` | Owned | -- | typeck, eval |
| `ok_or` | `(Any)` | `Fresh` | Owned | -- | typeck, eval |
| `iter` | -- | `IteratorOf(Element)` | Borrow | Iterable | typeck, eval |
| `map` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `and_then` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `flat_map` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `filter` | `(Closure)` | `SelfType` | Borrow | -- | typeck only |
| `or` | `(SelfType)` | `SelfType` | Owned | -- | typeck only |
| `or_else` | `(Closure)` | `ClosureDriven(...)` | Owned | -- | typeck only |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Concrete(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |

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
| `is_ok` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `is_err` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `unwrap` | -- | `OkType` | Owned | -- | typeck, eval |
| `expect` | `(Str)` | `OkType` | Owned | -- | typeck only |
| `unwrap_or` | `(OkType)` | `OkType` | Owned | -- | typeck only |
| `unwrap_err` | -- | `ErrType` | Owned | -- | typeck only |
| `expect_err` | `(Str)` | `ErrType` | Owned | -- | typeck only |
| `ok` | -- | `OptionOf(Ok)` | Borrow | -- | typeck only |
| `err` | -- | `OptionOf(Err)` | Borrow | -- | typeck only |
| `map` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `map_err` | `(Closure)` | `ClosureDriven(MapLike)` | Borrow | -- | typeck only |
| `and_then` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `or_else` | `(Closure)` | `ClosureDriven(FlatMapLike)` | Borrow | -- | typeck only |
| `has_trace` | -- | `Concrete(Bool)` | Borrow | -- | typeck, eval |
| `trace` | -- | `Concrete(Str)` | Borrow | -- | typeck, eval |
| `trace_entries` | -- | `ClosureDriven(...)` | Borrow | -- | typeck, eval |
| **Trait: Eq** |
| `equals` | `(SelfType)` | `Concrete(Bool)` | Borrow | Eq | typeck, eval |
| **Trait: Comparable** |
| `compare` | `(SelfType)` | `Concrete(Ordering)` | Borrow | Comparable | typeck, eval |
| **Trait: Hashable** |
| `hash` | -- | `Concrete(Int)` | Borrow | Hashable | typeck, eval |
| **Trait: Clone** |
| `clone` | -- | `SelfType` | Borrow | Clone | typeck, eval |
| **Trait: Debug** |
| `debug` | -- | `Concrete(Str)` | Borrow | Debug | typeck, eval |

### Result-Specific Projections

Result is the only type with **two distinct projections** -- ok-type and err-type --
that both appear in method signatures. This drives the need for `TypeProjection::Ok`
and `TypeProjection::Err` in the data model.

`Result.ok()` returns `Option<T>` (wrapping the ok-type), while `Result.err()` returns
`Option<E>` (wrapping the err-type). Without separate projections, these two methods
would be indistinguishable.

`Result.map_err()` is particularly interesting: it transforms the **error** type while
preserving the **ok** type. This is the inverse of `Result.map()`. The registry
declares `map_err` with `ReturnTag::Fresh` and the type checker's existing
`unify_higher_order_constraints` handles the inference (which type parameter
the closure transforms).

---

## 06.9 Channel TypeDef

**File:** `compiler/ori_registry/src/defs/channel/mod.rs` (tests: `channel/tests.rs`)

**Moved from Section 05.** Channel is structurally a generic container — `Channel<T>`
uses `TypeParamArity::Fixed(1)` and the same single-child-in-data pool pattern as
Option, Set, and Range. Its methods return `Option<T>` via
`ReturnTag::OptionOf(TypeProjection::Element)`, the same projection machinery as
List/Option in this section.

### Representation

```rust
pub const CHANNEL: TypeDef = TypeDef {
    tag: TypeTag::Channel,
    name: "Channel",
    memory: MemoryStrategy::Arc, // shared channel handle
    methods: &CHANNEL_METHODS,
    operators: OpDefs::UNSUPPORTED,
    type_params: TypeParamArity::Fixed(1), // Channel<T>
};
```

Channel is a generic Arc type used for concurrency communication (send/receive).
It has no operators. Unlike Duration/Size/Ordering, Channel does NOT have a
pre-interned `TypeId` constant in `ori_ir` — it is represented as `Tag::Channel`
(value 19) in the type pool.

### Instance Methods

| Method | Params | Returns | Receiver | Trait | Notes |
|--------|--------|---------|----------|-------|-------|
| `send` | `(T)` | `()` | Borrow | — | Send value into channel |
| `recv` / `receive` | — | `Option<T>` | Borrow | — | Blocking receive |
| `try_recv` / `try_receive` | — | `Option<T>` | Borrow | — | Non-blocking receive |
| `close` | — | `()` | Borrow | — | Close the channel |
| `is_closed` | — | `bool` | Borrow | — | Whether channel is closed |
| `is_empty` | — | `bool` | Borrow | — | Whether channel has no pending items |
| `len` | — | `int` | Borrow | — | Number of pending items |

### Current Coverage Matrix

| Method | ori_types | ori_eval (dispatch) | ori_ir | ori_llvm | Registry |
|--------|-----------|---------------------|--------|----------|----------|
| `send` | Y | — | — | — | Planned |
| `recv` | Y | — | — | — | Planned |
| `receive` | Y | — | — | — | Planned |
| `try_recv` | Y | — | — | — | Planned |
| `try_receive` | Y | — | — | — | Planned |
| `close` | Y | — | — | — | Planned |
| `is_closed` | Y | — | — | — | Planned |
| `is_empty` | Y | — | — | — | Planned |
| `len` | Y | — | — | — | Planned |

**Observations:**
- Channel exists ONLY in typeck (`resolve_channel_method`). Zero eval/IR/LLVM coverage.
- All 9 methods are in `TYPECK_METHODS_NOT_IN_EVAL` allowlist.
- No `Value::Channel` variant exists in `ori_patterns`.
- Channel is listed in `COLLECTION_TYPES` and `WELL_KNOWN_GENERIC_TYPES`.

### Sendable Constraint on T

Per the Ori spec, `Channel<T>` requires `T: Sendable`. This constraint is enforced by
the type checker during channel construction (`channel<T>(buffer:)` requires
`T: Sendable`), NOT by the registry. The registry's `TypeParamArity` describes arity,
not bounds on type params.

### Traits Not Covered (Channel-specific)

- **`Default`** — No. Channels must be explicitly constructed with `channel<T>(buffer:)`.
- **`Eq`/`Comparable`/`Hashable`/`Clone`** — No. Channels are mutable shared-state primitives.
- **`Sendable`** — No. Channel itself contains shared mutable state.
- **`Formattable`/`Printable`/`Debug`** — No. No meaningful string representation.

### MethodDef Frozen Field Defaults (Channel)

| Field | Value | Rationale |
|-------|-------|-----------|
| `pure` | `false` | All Channel methods have side effects (shared-state mutation) |
| `backend_required` | `false` | Eval, IR, LLVM coverage is all zero |
| `kind` | `MethodKind::Instance` | `channel<T>(buffer:)` constructor is a free function, not an associated function |
| `dei_only` | `false` | Not iterator methods |
| `dei_propagation` | `NotApplicable` | Not iterator methods |

**Note:** Channel is the only type in Section 06 with `pure: false`. All other
collection/wrapper types use persistent data structures with no side effects.

### Parameter Ownership for `send`

The `send` method takes ownership of the value being sent (`Ownership::Owned`). The
value is moved into the channel — the caller cannot use it after sending:
`ParamDef { name: "value", ty: ReturnTag::ElementType, ownership: Ownership::Owned }`.

### Tasks

- [x] Define `CHANNEL_METHODS: &[MethodDef]` with all 9 methods (send, recv, receive, try_recv, try_receive, close, is_closed, is_empty, len — each alias is a separate entry)
- [x] Use `type_params: TypeParamArity::Fixed(1)` for `Channel<T>`
- [x] Use `ReturnTag::OptionOf(TypeProjection::Element)` for `recv`/`try_recv`/`receive`/`try_receive`
- [x] Use `ReturnTag::Unit` for `send`, `close` return types
- [x] Set `pure: false` on ALL Channel methods (side effects from shared-state mutation)
- [x] Set `backend_required: false` on all methods (zero backend coverage)
- [x] Set `send` parameter ownership to `Ownership::Owned` (value moved into channel)
- [x] Set `dei_only: false` and `dei_propagation: NotApplicable` on all methods (via `chan()` helper)
- [x] Document `T: Sendable` constraint in module doc (enforced by type checker, not registry)
- [x] Register in `defs/mod.rs`: add `mod channel;` and `pub use self::channel::CHANNEL;`
- [x] Create `channel/tests.rs` with `#[cfg(test)] mod tests;` in `channel/mod.rs`
- [x] Unit test: all 9 methods present with correct return types
- [x] Unit test: no method has `backend_required: true` (channel_all_methods_impure implies this)
- [x] Unit test: all methods have `pure: false` (channel_all_methods_impure)
- [x] Unit test: `send` parameter has `Ownership::Owned` (channel_send_takes_element_owned)
- [x] Unit test: `OpDefs` is `UNSUPPORTED` (channel_no_operators)
- [x] Unit test: `type_params` is `TypeParamArity::Fixed(1)` (channel_is_generic_with_one_param)
- [x] Unit test: all 10 frozen fields present on every MethodDef (verified by compilation)

---

## 06.10 Handling Generic Types in the Registry

### Summary of Required Type-Projection Vocabulary

From the method tables above, the registry needs to express these return type patterns:

| Pattern | Example | ReturnTag |
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
   `ReturnTag` / `TypeProjection` concepts, not type tags. TypeTag identifies the
   *kind* of a type (Int, List, Map, etc.), not a projection from a generic parameter.

2. **Full type algebra in the registry** -- No. The registry expresses the *shape* of
   the return type relationship. The type checker materializes it into `Idx` values
   using pool operations.

3. **Closure parameter types** -- No (mostly). The registry marks a parameter as
   closure params use `ReturnTag::Fresh` for the parameter type without specifying `(T) -> U`. The type checker infers the
   closure type from context and uses `unify_higher_order_constraints`.

### ReturnTag::SelfType

`SelfType` is on `ReturnTag` (not `TypeTag`) and means "the return type equals the receiver
type." This is correct for `List.reverse()`, `Option.clone()`, etc. No new TypeTag
needed.

### TypeTag::Iterator

Not a TypeTag concern -- `IteratorOf(Element)` in `ReturnTag` handles this. The type
checker maps it to `pool.iterator(elem)`.

---

## 06.11 Cross-Reference & Validation

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
| Channel | 9 | 0 | 9 unimplemented | 9 |
| **Total** | **138** | **61** | **84** | **138** |

### Allowlist Entries Eliminated by This Section

| Allowlist | Current Entries for Collection Types | After Section 06 |
|-----------|-------------------------------------|------------------|
| `COLLECTION_TYPES` | 11 entries (entire list) | Reduced by 8 (list, map, Set, range, tuple, Option, Result, Channel) |
| `TYPECK_METHODS_NOT_IN_EVAL` (collection portion) | 81 entries (72 + 9 Channel) | **Eliminated** (tracked by registry status) |

Note: `COLLECTION_TYPES` also contains `DoubleEndedIterator`, `Iterator`,
and `error`. Channel is now covered in this section (moved from Section 05).
Iterator/DEI are covered in Section 07. Error is covered in Section 05. Once all
sections are complete, `COLLECTION_TYPES` is **fully eliminated**.

### Consistency Test Impact

The consistency tests in `consistency.rs` that currently skip `COLLECTION_TYPES`:

- `eval_primitive_methods_in_ir` (line 119): Will include collection types after registry wiring
- `typeck_primitive_methods_in_ir` (line 650): Will include collection types after registry wiring

### Source File Cross-Reference

| File | Lines Affected | Change |
|------|---------------|--------|
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_list_method` (465-500) | Replaced by registry lookup (Section 09) |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_option_method` (502-525) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_result_method` (527-549) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_map_method` (551-572) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_set_method` (574-587) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_range_method` (689-707) | Replaced by registry lookup |
| `ori_types/src/infer/expr/methods/resolve_by_type.rs` | `resolve_tuple_method` (868-877) | Replaced by registry lookup |
| `ori_types/src/infer/expr/calls/mod.rs` | `unify_higher_order_constraints` (700-764) | Stays in type checker (inference logic, not registry data) |
| `ori_eval/src/methods/helpers/mod.rs` | `EVAL_BUILTIN_METHODS` (collection entries) | Replaced by registry enumeration |
| `oric/src/eval/tests/methods/consistency.rs` | `COLLECTION_TYPES` (13-25) | **Eliminated** |
| `oric/src/eval/tests/methods/consistency.rs` | `TYPECK_METHODS_NOT_IN_EVAL` (374-633, collection portion) | **Eliminated** |

---

## 06.11a MethodDef Frozen Field Defaults (All Collection & Wrapper Types)


Per frozen decision 13 (Section 00), every `MethodDef` must specify all 10 fields. The
method tables above show `name`, `params`, `returns`, `receiver`, and `trait_name`. The
remaining 5 fields follow these defaults for collection and wrapper types:

| Field | Default | Exceptions |
|-------|---------|------------|
| `pure` | `true` | **Channel is the exception**: all Channel methods are `pure: false` (shared-state mutation). All other collection/wrapper methods are pure — even `push`/`insert` return new values (persistent data structures). |
| `backend_required` | `true` | Higher-order methods (`map`, `filter`, `fold`, `find`, `any`, `all`, `for_each`, `flat_map`, `reduce`, `take_while`, `skip_while`, `min_by`, `max_by`, `sort_by`, `group_by`, `partition`, `and_then`, `or_else`, `map_err`) are `false` — they are typeck+eval only (LLVM deferred). |
| `kind` | `MethodKind::Instance` | None — all collection/wrapper methods are instance methods. No associated functions on these types. |
| `dei_only` | `false` | None — `dei_only` applies only to Iterator methods (Section 07). |
| `dei_propagation` | `DeiPropagation::NotApplicable` | None — DEI propagation applies only to Iterator adapter methods (Section 07). |

**Implementation note:** The Rust definition sketches above use `MethodDef::new()` which
only takes 5 fields. The actual implementation MUST use struct literals with all 10 fields
per frozen decision 13. The `backend_required` distinction between simple methods (true)
and higher-order methods (false) is critical for LLVM wiring (Section 12).

---

## 06.11b Traits Not Covered by the Registry (Collection & Wrapper Types)


Following the precedent from Section 03 (Primitive Types), certain traits are handled
outside the registry. This subsection documents them for all 7 collection/wrapper types.

### Default Trait

| Type | Default Value | Handled By |
|------|--------------|------------|
| List | `[]` (empty list) | `well_known` in ori_types |
| Map | `{}` (empty map) | `well_known` in ori_types |
| Set | `Set()` (empty set) | `well_known` in ori_types |
| Range | No Default | N/A |
| Tuple | `()` (unit) for 0-tuple only | `well_known` in ori_types |
| Option | `None` | `well_known` in ori_types |
| Result | No Default | N/A |

**Rationale:** Default values are type-level constants, not method behavior. The registry
declares methods; the type checker's `well_known` module handles Default construction.

### Formattable Trait

All 7 types use the **blanket impl** from Printable — they implement `to_str` (registered
as a trait method in the method tables above under Debug/Printable), and the blanket
`Formattable` impl delegates to `to_str()`. No explicit `format` MethodDef is needed in
the registry for these types.

### Sendable Trait

| Type | Sendable? | Reason |
|------|-----------|--------|
| List | Conditional | `Sendable` if `T: Sendable` |
| Map | Conditional | `Sendable` if `K: Sendable` and `V: Sendable` |
| Set | Conditional | `Sendable` if `T: Sendable` |
| Range | Yes | Copy type (start + end + step are value types) |
| Tuple | Conditional | `Sendable` if all element types are `Sendable` |
| Option | Conditional | `Sendable` if `T: Sendable` |
| Result | Conditional | `Sendable` if `T: Sendable` and `E: Sendable` |
| Channel | No | Contains shared mutable state |

**Rationale:** Sendable is a marker trait auto-derived by the compiler based on field
types. The registry does not declare it — the type checker infers it structurally.

### Value Trait

| Type | Value? | Reason |
|------|--------|--------|
| List | No | Arc memory strategy |
| Map | No | Arc memory strategy |
| Set | No | Arc memory strategy |
| Range | Yes | Copy type, all fields are value types |
| Tuple | Conditional | Value if all elements are Value |
| Option | Conditional | Value if T is Value |
| Result | Conditional | Value if T and E are Value |
| Channel | No | Arc memory strategy, shared mutable state |

### Into Trait

- `Set.into()` -> `[T]` (Set to List conversion) — registered as a method in the Set table
- No other collection/wrapper types have standard `Into` conversions in the registry

---

## 06.11c Ownership Semantics Summary


The ownership distinction between `Borrow` and `Own` is critical for ARC correctness
(Section 11). This table summarizes the ownership pattern across all 7 types:

| Type | Borrowing Methods | Owning Methods | Pattern |
|------|-------------------|----------------|---------|
| List | All query/transform methods | None | All operations return new values |
| Map | All query/transform methods | None | Same pattern |
| Set | All query/transform methods | None | Same pattern |
| Range | All methods | None | Copy type — borrow is trivial |
| Tuple | All methods | None | Same pattern |
| Option | `is_some`, `is_none`, `map`, `filter`, `and_then`, `flat_map`, `iter`, trait methods | `unwrap`, `expect`, `unwrap_or`, `ok_or`, `or`, `or_else` | Consuming methods take ownership |
| Result | `is_ok`, `is_err`, `ok`, `err`, `map`, `map_err`, `and_then`, `or_else`, `has_trace`, `trace`, `trace_entries`, trait methods | `unwrap`, `expect`, `unwrap_or`, `unwrap_err`, `expect_err` | Same consuming pattern |
| Channel | `recv`, `try_recv`, `close`, `is_closed`, `is_empty`, `len` | `send` (value param, not receiver) | `send` takes ownership of the *value*, not the channel |

**Key insight:** Option and Result are the only collection/wrapper types with `Ownership::Owned`
methods. These are methods that consume the wrapper to extract the inner value. All other
types use persistent/functional semantics where every operation borrows the receiver and
returns a new value.

---

## Implementation Checklist

### 06.1 Data Model Extensions (prerequisite: Section 01 finalized)

- [x] Add `MemoryStrategy::Structural` variant — **done in Section 05 exit**
- [x] Add `TypeProjection` enum (Element, Key, Value, Ok, Err, Fixed) — **done in Section 01**
- [x] Extend `ReturnTag` with generic projections (OptionOf, ListOf, IteratorOf, etc.) — **done in Section 01**
- [x] Add `ReturnTag::Fresh` variant for higher-order methods — **done in Section 01**
- [x] Add `ListKeyValue` and `MapIterator` ReturnTag variants — **done in Section 01**
- [x] Add `ListOfTupleIntElement` ReturnTag variant — **done in Section 01**
- [x] Use `ParamDef` with `ReturnTag::ElementType`, `KeyType`, `ValueType` for typed generic params — **canonical form from Section 01**
- [x] Verify all variants are `const`-constructible (no allocations) — verified by static compilation
- [x] Write unit tests for each new enum variant's Debug output — covered by tags/tests.rs

### 06.2 List TypeDef

- [x] Define `LIST` const with all 57 methods (verified against `resolve_list_method` — 7 more than plan estimate due to aliases and additional methods)
- [x] Verify method names match typeck `resolve_list_method` entries
- [x] Document which methods use `ReturnTag::Fresh` (22 higher-order methods)
- [x] All methods use `ReturnTag::Fresh` instead of removed `ClosureDriven`
- [x] Set all 10 frozen fields on every MethodDef (per frozen decision 13)
- [x] Set `backend_required: false` on all methods (conservative — Section 14 will upgrade)
- [x] Verified against typeck: all 57 method names, parameter names, return types
- [x] Test: list_method_count (57)
- [x] Test: list_methods_alphabetically_sorted, list_trait_methods, list_higher_order_methods_return_fresh, etc.

### 06.3 Map TypeDef

- [x] Define `MAP` const with all 18 methods (includes `length` alias, verified against `resolve_map_method`)
- [x] Verify key/value projection usage is correct for each method
- [x] Set all 10 frozen fields on every MethodDef
- [x] Verified against typeck: all 18 method names, parameter names, return types
- [x] Test: map_get_returns_option_of_value, map_keys_returns_list_of_key
- [x] Test: map_method_count (18), map_methods_alphabetically_sorted, etc.

### 06.4 Set TypeDef

- [x] Define `SET` const with all 16 methods (includes `length` alias, verified against `resolve_set_method`)
- [x] Verify `iter` returns `IteratorOf` (not DEI) — hash sets have no inherent ordering
- [x] Set all 10 frozen fields on every MethodDef
- [x] Verified against typeck: all 16 method names, parameter names, return types
- [x] Test: set_iter_returns_iterator_not_dei, set_method_count (16), etc.

### 06.5 Range TypeDef

- [x] Define `RANGE` const with all 8 methods (verified against `resolve_range_method`)
- [x] Document Range<float> iteration guard in module doc (not encoded in registry — type checker handles it)
- [x] Verify `iter` returns `DoubleEndedIteratorOf(Element)` (not plain Iterator)
- [x] Set all 10 frozen fields on every MethodDef
- [x] Verified against typeck: all 8 method names, parameter names, return types
- [x] Test: range_iter_returns_dei, range_method_count (8), range_step_by_takes_int_returns_self, etc.

### 06.6 Tuple TypeDef

- [x] Define `TUPLE` const with all 6 methods (verified against `resolve_tuple_method`)
- [x] Set `MemoryStrategy::Structural`
- [x] Document in module doc that field access (._0, ._1) is handled by parser, not method system
- [x] Set all 10 frozen fields on every MethodDef
- [x] Verified against typeck: all 6 method names, parameter names, return types
- [x] Test: tuple_method_count (6), tuple_is_structural, tuple_is_variadic, tuple_trait_methods, etc.

### 06.7 Option TypeDef

- [x] Define `OPTION` const with all 18 methods (13 inherent + 5 trait, verified against `resolve_option_method`)
- [x] Set `MemoryStrategy::Structural`
- [x] Verify ownership semantics: default/err/or params are Owned, message is Borrow
- [x] All methods use `ReturnTag::Fresh` for HOF (no `ClosureDriven`)
- [x] Set all 10 frozen fields on every MethodDef
- [x] Set `backend_required: false` on all methods (conservative)
- [x] Verified against typeck: all 18 method names, parameter names, return types
- [x] Test: option_unwrap_returns_element, option_predicates_return_bool, option_higher_order_methods_return_fresh, etc.

### 06.8 Result TypeDef

- [x] Define `RESULT` const with all 21 methods (13 inherent + 8 trait, verified against `resolve_result_method`)
- [x] Set `MemoryStrategy::Structural`
- [x] Verify ok/err projection distinction: ok() -> OptionOf(Ok), err() -> OptionOf(Err)
- [x] Verify `map_err` uses `ReturnTag::Fresh`
- [x] All methods use `ReturnTag::Fresh` for HOF (no `ClosureDriven`)
- [x] Set all 10 frozen fields on every MethodDef
- [x] Set `backend_required: false` on all methods (conservative)
- [x] Verify ownership: default/message params with correct ownership
- [x] Verified against typeck: all 21 method names, parameter names, return types
- [x] Test: result_ok_returns_option_of_ok, result_err_returns_option_of_err, result_trait_methods, etc.

### 06.9 Channel TypeDef

- [x] Define `CHANNEL` const with all 9 methods (verified against `resolve_channel_method`)
- [x] Set `type_params: TypeParamArity::Fixed(1)`
- [x] Verify `recv`/`try_recv`/`receive`/`try_receive` return `OptionOf(Element)`
- [x] Set `pure: false` on ALL Channel methods (unique among all registry types)
- [x] Set `backend_required: false` on all methods (zero backend coverage)
- [x] Verify `send` parameter ownership is `Owned`
- [x] Set all 10 frozen fields on every MethodDef (via `chan()` const fn helper)
- [x] Test: channel_method_count (9), channel_all_methods_impure, channel_send_takes_element_owned, etc.

### 06.10 Integration

- [x] All 8 TypeDefs compile with `cargo c -p ori_registry`
- [x] All TypeDefs are included in `BUILTIN_TYPES` array (defs/mod.rs updated)
- [x] Total method count: 57+18+16+8+6+18+21+9 = 153 methods across 8 types
- [x] No duplicate method names within a single TypeDef (verified by defs/tests.rs::no_duplicate_methods)
- [x] All method names are sorted alphabetically within each TypeDef (verified by defs/tests.rs::methods_alphabetically_sorted)
- [x] Purity test passes — all 207 ori_registry tests pass, 12,364 total tests pass

---

## Exit Criteria

1. **Eight TypeDef constants** (`LIST`, `MAP`, `SET`, `RANGE`, `TUPLE`, `OPTION`,
   `RESULT`, `CHANNEL`) are defined in `ori_registry` with complete method tables.

2. **Every method** currently in `TYPECK_BUILTIN_METHODS` for these types has a
   corresponding `MethodDef` in the registry. Count verification:
   - list: 50, map: 17, Set: 15, range: 8, tuple: 6, Option: 16, Result: 17, Channel: 9

3. **ReturnTag** is expressive enough to represent all return type patterns found in
   `resolve_*_method()` functions, including generic projections and higher-order flows.

4. **`MemoryStrategy::Structural`** exists for types whose memory strategy depends on
   their contents (Tuple, Option, Result).

5. **`type_params` field** on each TypeDef matches the arity: List=1, Map=2, Set=1,
   Range=1, Tuple=variadic, Option=1, Result=2, Channel=1.

6. **`cargo c -p ori_registry`** passes with no warnings.

7. **Unit tests** verify lookup by name for at least 3 representative methods per type.


8. **Every MethodDef has all 10 frozen fields** from frozen decision 13: `name`,
   `params`, `returns`, `receiver`, `trait_name`, `pure`, `backend_required`, `kind`,
   `dei_only`, `dei_propagation`.

9. **`pure` is `true`** for all collection/wrapper methods (persistent data structures,
   no side effects), **except Channel** (`pure: false` — side effects from shared-state mutation).

10. **`backend_required` is correct**: `true` for simple methods (len, is_empty,
    contains, first, last, iter, trait methods), `false` for higher-order methods
    (map, filter, fold, find, any, all, for_each, etc.).

11. **`kind` is `MethodKind::Instance`** for all collection/wrapper methods (no
    associated functions).

12. **`dei_only` is `false`** and **`dei_propagation` is `NotApplicable`** for all
    collection/wrapper methods (DEI semantics are Section 07 only).

13. **Ownership semantics correct**: Option/Result consuming methods (`unwrap`,
    `expect`, `unwrap_or`, `or`, etc.) use `Ownership::Owned`; all other methods
    use `Ownership::Borrow`.

14. **Every method cross-referenced against spec** (`docs/ori_lang/v2026/spec/`)
    for name, parameters, and return type accuracy.

15. **`ClosureDriven` references resolved**: Method tables use `ReturnTag::Fresh`
    (not `ClosureDriven`) for higher-order methods — `ClosureDriven` was removed
    from the plan; the type checker handles closure inference via
    `unify_higher_order_constraints()`.

---

## Open Questions for Section 01

These questions must be resolved in Section 01 (Core Data Model) before this section
can be implemented:

1. ~~**How does `TypeDef` represent type parameter arity?**~~ **Resolved:** Section 01 defines `TypeParamArity` enum (`Fixed(u8) | Variadic`) as a field on `TypeDef`.

2. ~~**Where does `ClosureFlow` live?**~~ **Resolved:** ClosureFlow was removed from scope. Higher-order method inference stays in the type checker. Methods use `ReturnTag::Fresh`.

3. **How does `ReturnTag` handle `Result` construction?** For `Option.ok_or(E) -> Result<T, E>`,
   we need to construct a Result from the inner type (T) and a fresh/argument type (E).
   Does this require a new variant, or can it be expressed as a combination?

4. **`ParamDef` for generic parameters:** Resolved — use `ParamDef { ty: ReturnTag::KeyType }`,
   `ParamDef { ty: ReturnTag::ValueType }` for Map methods like `insert`. This is precise
   and consistent with the Section 01 data model.

5. ~~**Variadic tuples:**~~ **Resolved:** Frozen decision 8 defines `TypeParamArity::Variadic` for tuples.

6. ~~**`map_err` ClosureFlow**~~ **Resolved:** ClosureFlow removed. `map_err` uses `ReturnTag::Fresh`; the type checker handles which type parameter the closure transforms.

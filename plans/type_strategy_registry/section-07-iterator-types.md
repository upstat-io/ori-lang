---
section: "07"
title: "Iterator & DoubleEndedIterator Type Definitions"
status: not-started
goal: "Complete pure-data TypeDef declarations for Iterator<T> and DoubleEndedIterator<T>, including all adapter/consumer methods, DEI-only gating, and CollectionMethod enum mapping"
depends_on:
  - "01"  # Core Data Model (TypeDef, MethodDef)
  - "02"  # Crate Scaffolding (ori_registry exists, module structure in place)
files:
  # Registry (new)
  - ori_registry/src/defs/iterator.rs
  # Type checker (current — to be replaced)
  - compiler/ori_types/src/infer/expr/methods/mod.rs     # resolve_iterator_method(), DEI_ONLY_METHODS
  - compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs  # per-type method resolution
  - compiler/ori_types/src/infer/expr/calls/mod.rs      # unify_higher_order_constraints()
  - compiler/ori_types/src/infer/expr/calls/constraints.rs  # constraint solving
  # Evaluator (current — to be consumed)
  - compiler/ori_eval/src/interpreter/resolvers/mod.rs   # CollectionMethod enum, ITERATOR_METHOD_NAMES
  - compiler/ori_eval/src/interpreter/resolvers/collection/mod.rs  # CollectionMethodResolver
  - compiler/ori_eval/src/interpreter/method_dispatch/iterator/mod.rs  # eval_iterator_method()
  # LLVM (current — to be consumed)
  - compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator.rs  # declare_builtins! for Iterator
  # Patterns (runtime representation)
  - compiler/ori_patterns/src/value/iterator/mod.rs      # IteratorValue enum
  # Consistency tests
  - compiler/oric/src/eval/tests/methods/consistency.rs  # COLLECTION_TYPES, ITERATOR_METHOD_NAMES
  # ori_ir (current — to be consolidated)
  - compiler/ori_ir/src/builtin_methods/mod.rs           # MethodDef, BUILTIN_METHODS (no Iterator entries yet)
---

# Section 07: Iterator & DoubleEndedIterator Type Definitions

**Status:** Not Started
**Goal:** Declare the complete behavioral specification of `Iterator<T>` and `DoubleEndedIterator<T>` as `const` data in `ori_registry`, including every method's signature, receiver ownership, DEI-only gating, and double-endedness propagation semantics.

**Why this is the most complex section:** Iterator types are unique among builtins because:
1. They are **generic** (`Iterator<T>`) with type parameters that flow through adapter chains
2. They have **higher-order methods** that use `ReturnTag::Fresh` to signal type checker inference for closure-dependent return types
3. They have a **subtype relationship** (DEI extends Iterator) requiring gating logic
4. Adapter methods have **double-endedness propagation rules** (some preserve DEI, some downgrade)
5. The evaluator uses a **separate dispatch enum** (`CollectionMethod`) rather than string-based dispatch
6. The LLVM backend handles iterators through a **completely different code path** (opaque `ptr` + runtime calls)

---

## 07.1 Iterator TypeDef

### 07.1.1 Method Inventory

The complete set of Iterator methods, derived from `resolve_iterator_method()` in `methods.rs` (lines 804-866), `TYPECK_BUILTIN_METHODS` (lines 79-97), `ITERATOR_METHOD_NAMES` in `resolvers/mod.rs` (lines 232-257), and `CollectionMethod` enum variants.

**Adapter methods** (return Iterator or DoubleEndedIterator):

| Method | Signature | Params | Return | DEI Propagation | Notes |
|--------|-----------|--------|--------|-----------------|-------|
| `map` | `(f: (T) -> U) -> Iterator<U>` | 1 closure | Iterator(U) or DEI(U) | **Propagates** | Returns DEI if receiver is DEI |
| `filter` | `(f: (T) -> bool) -> Iterator<T>` | 1 closure | Same as receiver | **Propagates** | Returns exact receiver type |
| `take` | `(n: int) -> Iterator<T>` | 1 int | Iterator(T) | **Downgrades** | Always returns plain Iterator |
| `skip` | `(n: int) -> Iterator<T>` | 1 int | Iterator(T) | **Downgrades** | Always returns plain Iterator |
| `chain` | `(other: Iterator<T>) -> Iterator<T>` | 1 Iterator | Iterator(T) | **Downgrades** | Always returns plain Iterator |
| `zip` | `(other: Iterator<U>) -> Iterator<(T, U)>` | 1 Iterator | Iterator((T, U)) | **Downgrades** | Fresh var for U |
| `enumerate` | `() -> Iterator<(int, T)>` | 0 | Iterator((int, T)) | **Downgrades** | Element becomes tuple |
| `flatten` | `() -> Iterator<U>` | 0 | Iterator(U) | **Downgrades** | T must be Iterator<U> |
| `flat_map` | `(f: (T) -> Iterator<U>) -> Iterator<U>` | 1 closure | Iterator(U) | **Downgrades** | Desugars to .map(f).flatten() |
| `cycle` | `() -> Iterator<T>` | 0 | Iterator(T) | **Downgrades** | Infinite iterator |

**Consumer methods** (eagerly consume, return non-Iterator):

| Method | Signature | Params | Return | Notes |
|--------|-----------|--------|--------|-------|
| `collect` | `() -> [T]` | 0 | List(T) | Default collection target |
| `count` | `() -> int` | 0 | Int | Consumes all elements |
| `any` | `(f: (T) -> bool) -> bool` | 1 closure | Bool | Short-circuits on true |
| `all` | `(f: (T) -> bool) -> bool` | 1 closure | Bool | Short-circuits on false |
| `find` | `(f: (T) -> bool) -> Option<T>` | 1 closure | Option(T) | Short-circuits on match |
| `fold` | `(init: S, f: (S, T) -> S) -> S` | 1 any + 1 closure | S (fresh) | Return unifies with init + closure return |
| `for_each` | `(f: (T) -> void) -> void` | 1 closure | Unit | Side-effect consumer |
| `join` | `(sep: str) -> str` | 1 str | Str | T must implement Printable |
| `next` | `() -> (Option<T>, Iterator<T>)` | 0 | Tuple(Option(T), receiver_ty) | Functional next — returns updated iter |

**Internal/special methods:**

| Method | Signature | Notes |
|--------|-----------|-------|
| `__iter_next` | `() -> {tag: int, elem: T}` | ARC lowering protocol — NOT in TYPECK, only in LLVM |
| `__collect_set` | `() -> Set<T>` | Type-directed rewrite — NOT in TYPECK, only in eval |

### 07.1.2 Registry Definition (Proposed)

```rust
// ori_registry/src/defs/iterator.rs

use crate::{
    MethodDef, Ownership, ParamDef, TypeDef, TypeTag,
    ReturnType, GenericParam, DeiPropagation,
};

/// Iterator<T> — lazy sequence with adapter/consumer protocol.
pub const ITERATOR: TypeDef = TypeDef {
    tag: TypeTag::Iterator,
    name: "Iterator",
    display_name: "Iterator<T>",
    generic_params: &[GenericParam::Element],  // T
    memory: MemoryStrategy::Arc,               // Opaque heap-allocated handle
    operators: OpDefs::NONE,                   // No operators on Iterator
    methods: &ITERATOR_METHODS,
};

pub const ITERATOR_METHODS: &[MethodDef] = &[
    // ── Protocol ──
    MethodDef {
        name: "next",
        params: &[],
        returns: ReturnTag::TupleOf(&[
            ReturnTag::OptionOf(ReturnTag::Element),
            ReturnTag::ReceiverType,
        ]),
        receiver: Ownership::Borrow,
        dei_propagation: DeiPropagation::NotApplicable,
        dei_only: false,
        trait_name: None,
    },
    // ── Adapters ──
    MethodDef {
        name: "map",
        params: &[ParamDef::Closure],
        returns: ReturnTag::IteratorOf(ReturnTag::FreshVar),
        receiver: Ownership::Borrow,
        dei_propagation: DeiPropagation::Propagate,
        dei_only: false,
        trait_name: None,
    },
    MethodDef {
        name: "filter",
        params: &[ParamDef::Closure],
        returns: ReturnTag::ReceiverType,
        receiver: Ownership::Borrow,
        dei_propagation: DeiPropagation::Propagate,
        dei_only: false,
        trait_name: None,
    },
    MethodDef {
        name: "take",
        params: &[ParamDef::Type(TypeTag::Int)],
        returns: ReturnTag::IteratorOf(ReturnTag::Element),
        receiver: Ownership::Borrow,
        dei_propagation: DeiPropagation::Downgrade,
        dei_only: false,
        trait_name: None,
    },
    // ... (remaining methods follow same pattern)
];
```

### 07.1.3 All Methods Borrow Receiver

**Decision:** Every Iterator method borrows its receiver.

Justification from the codebase: In `builtins/iterator.rs`, every `declare_builtins!` entry uses `borrow: true`. In the evaluator, `eval_iterator_method()` takes `receiver: Value` by value but this is the Rust move semantics — the Ori-level semantic is always borrow because iterators are functional (calling `.map()` on an iterator does not consume the original; it wraps it).

For the registry, `receiver: Ownership::Borrow` on every Iterator method.

### 07.1.4 Receiver Type Decision

The current `ori_ir` `MethodDef` uses `BuiltinType` which has no `Iterator` variant. The COLLECTION_TYPES list in `consistency.rs` explicitly tracks Iterator and DoubleEndedIterator as types NOT YET in the IR registry.

**Required:** The registry's `TypeTag` enum must include `Iterator` and `DoubleEndedIterator` variants (or a unified variant with a DEI flag — see Section 07.2).

---

## 07.2 DoubleEndedIterator TypeDef

### 07.2.1 DEI-Only Methods

From `DEI_ONLY_METHODS` in `methods.rs` line 11:

| Method | Signature | Params | Return | Notes |
|--------|-----------|--------|--------|-------|
| `next_back` | `() -> (Option<T>, DEI<T>)` | 0 | Tuple(Option(T), receiver_ty) | Reverse iteration |
| `rev` | `() -> DEI<T>` | 0 | receiver_ty | Swaps next/next_back |
| `last` | `() -> Option<T>` | 0 | Option(T) | Seeks to end via next_back |
| `rfind` | `(f: (T) -> bool) -> Option<T>` | 1 closure | Option(T) | Reverse find |
| `rfold` | `(init: S, f: (S, T) -> S) -> S` | 1 any + 1 closure | S (fresh) | Reverse fold |

### 07.2.2 Design Decision: Separate TypeDef vs. Flag

**Option A: Two separate TypeDef declarations**

```rust
pub const ITERATOR: TypeDef = TypeDef { tag: TypeTag::Iterator, methods: &ITER_METHODS, ... };
pub const DOUBLE_ENDED_ITERATOR: TypeDef = TypeDef { tag: TypeTag::DoubleEndedIterator, methods: &DEI_METHODS, ... };
```

Where `DEI_METHODS` includes ALL Iterator methods plus the 5 DEI-only methods.

**Pros:** Simple lookup — `find_type(DoubleEndedIterator).methods` gives everything.
**Cons:** ~20 method entries duplicated across both TypeDefs. Updating an Iterator method requires updating both.

**Option B: Inheritance via `extends` field**

```rust
pub const DOUBLE_ENDED_ITERATOR: TypeDef = TypeDef {
    tag: TypeTag::DoubleEndedIterator,
    extends: Some(&ITERATOR),
    methods: &DEI_ONLY_METHODS_DEFS,  // Only the 5 extra methods
    ...
};
```

**Pros:** No duplication. Single source for shared methods.
**Cons:** Lookup requires walking `extends` chain. More complex query API. The `extends` field is only needed for this one type pair (no other builtin type has inheritance).

**Option C: Single TypeDef with `dei_only` flag on methods**

```rust
pub const ITERATOR: TypeDef = TypeDef {
    tag: TypeTag::Iterator,
    methods: &ALL_ITERATOR_METHODS,  // Includes DEI-only methods with flag
    ...
};
// No separate DEI TypeDef — the pool Tag::DoubleEndedIterator is handled by
// querying the same TypeDef and checking the dei_only flag.
```

Where each `MethodDef` has `dei_only: bool`. The query API provides:
- `methods_for(Iterator)` -> all methods where `!dei_only`
- `methods_for(DoubleEndedIterator)` -> all methods (no filter)
- `is_dei_only(method_name)` -> checks the flag

**Pros:** No duplication, no inheritance mechanism, simple query, flag is directly derivable.
**Cons:** Single TypeDef holds methods for two conceptual types. The `dei_only` flag is only meaningful for Iterator methods (wasted field elsewhere).

**Recommendation: Option C** — single TypeDef with `dei_only` flag.

Rationale:
1. The current codebase already treats Iterator and DEI as the same resolve function (`resolve_iterator_method`) with an `is_dei` guard.
2. The evaluator uses a single `CollectionMethod` enum for both Iterator and DEI methods.
3. The `DEI_ONLY_METHODS` constant is already a flat list of method names — a boolean flag is the natural registry equivalent.
4. The `extends` mechanism (Option B) adds complexity used by exactly one type pair. The Ori type system does not have general type inheritance.
5. A `dei_only: bool` field on `MethodDef` costs 1 byte per entry globally. For non-iterator methods, it is always `false` — a trivially documented default. Alternatively, this field can be made `Option<bool>` or gated behind a feature, but `bool` with a default of `false` is simpler.

**Implication for query API (Section 08):**
```rust
/// Get methods available on a specific tag.
pub fn methods_for_tag(tag: TypeTag) -> impl Iterator<Item = &'static MethodDef> {
    let type_def = find_type(tag.base_type());
    type_def.methods.iter().filter(move |m| {
        // For plain Iterator: skip DEI-only methods
        // For DEI: include all
        tag != TypeTag::Iterator || !m.dei_only
    })
}
```

### 07.2.3 DEI_ONLY_METHODS Must Be Derivable from Registry

Currently `DEI_ONLY_METHODS` is a `const &[&str]` in `methods.rs`. After migration:

```rust
// This constant becomes derivable:
// DEI_ONLY_METHODS = ITERATOR.methods.iter()
//     .filter(|m| m.dei_only)
//     .map(|m| m.name)
//     .collect()
```

The consistency test in Section 14 must verify that every method with `dei_only: true` in the registry corresponds to the current `DEI_ONLY_METHODS` list, and vice versa.

---

## 07.3 CollectionMethod Enum Mapping

### 07.3.1 Current CollectionMethod Enum

The evaluator's `CollectionMethod` enum in `resolvers/mod.rs` has 32 variants:

**List/Range/Map methods (non-iterator):**
- `Map`, `Filter`, `Fold`, `Find`, `Collect`, `MapEntries`, `FilterEntries`, `Any`, `All`, `Join`

**Iterator adapter methods:**
- `IterNext`, `IterMap`, `IterFilter`, `IterTake`, `IterSkip`, `IterEnumerate`, `IterZip`, `IterChain`, `IterFlatten`, `IterFlatMap`, `IterCycle`

**Iterator consumer methods:**
- `IterFold`, `IterCount`, `IterFind`, `IterAny`, `IterAll`, `IterForEach`, `IterCollect`, `IterCollectSet`, `IterJoin`

**DEI-only methods:**
- `IterNextBack`, `IterRev`, `IterLast`, `IterRFind`, `IterRFold`

**Other:**
- `OrderingThenWith`

### 07.3.2 Registry Entries to CollectionMethod Mapping

Each registry `MethodDef` for Iterator must map to exactly one `CollectionMethod` variant. This mapping is used by the evaluator wiring (Section 10).

| Registry Method Name | CollectionMethod Variant | Dispatch Path |
|---------------------|--------------------------|---------------|
| `next` | `IterNext` | `eval_iter_next_as_tuple()` |
| `next_back` | `IterNextBack` | `eval_iter_next_back_as_tuple()` |
| `map` | `IterMap` | `make_mapped()` |
| `filter` | `IterFilter` | `make_filtered()` |
| `take` | `IterTake` | `make_take()` |
| `skip` | `IterSkip` | `make_skip()` |
| `enumerate` | `IterEnumerate` | `make_enumerated()` |
| `zip` | `IterZip` | `make_zipped()` |
| `chain` | `IterChain` | `make_chained()` |
| `flatten` | `IterFlatten` | `make_flattened()` |
| `flat_map` | `IterFlatMap` | `make_flat_mapped()` |
| `cycle` | `IterCycle` | `make_cycled()` |
| `rev` | `IterRev` | `make_reversed()` |
| `fold` | `IterFold` | `eval_iter_fold()` |
| `rfold` | `IterRFold` | `eval_iter_rfold()` |
| `count` | `IterCount` | `eval_iter_count()` |
| `find` | `IterFind` | `eval_iter_find()` |
| `rfind` | `IterRFind` | `eval_iter_rfind()` |
| `any` | `IterAny` | `eval_iter_any()` |
| `all` | `IterAll` | `eval_iter_all()` |
| `for_each` | `IterForEach` | `eval_iter_for_each()` |
| `collect` | `IterCollect` | `eval_iter_collect()` |
| `join` | `IterJoin` | `eval_iter_join()` |
| `last` | `IterLast` | `eval_iter_last()` |

**Internal methods not in registry:**
- `__collect_set` -> `IterCollectSet` (type-directed rewrite, not user-callable)
- `__iter_next` (ARC lowering protocol, not user-callable)

### 07.3.3 Resolver vs. Direct Dispatch

All Iterator methods go through `CollectionMethodResolver` (priority 1), which means they are resolved by the collection resolver, not by `BuiltinMethodResolver` (priority 2). This is because Iterator methods take closures that need evaluator access to call.

Post-registry, the `CollectionMethodResolver` will still exist but will derive its method name set from the registry instead of from the `MethodNames` struct with 25 pre-interned fields. The resolver's `resolve_iterator_method()` will iterate over `ITERATOR.methods` and match interned names.

### 07.3.4 List/Collection Overlap

Some method names exist on both `List` and `Iterator`:
- `map`, `filter`, `fold`, `find`, `any`, `all`, `flat_map`, `flatten`, `enumerate`, `zip`, `take`, `skip`, `join`, `collect`, `count`

For `List`, these are dispatched as list-specific `CollectionMethod` variants (`Map`, `Filter`, `Fold`, `Find`, `Any`, `All`). For `Iterator`, they are `Iter*` variants. The registry encodes these as separate methods on separate TypeDefs (Section 06 for List, Section 07 for Iterator). The evaluator resolver distinguishes by checking `receiver` type.

---

## 07.4 DeiPropagation — Double-Endedness Semantics

### 07.4.1 Propagation Rules

When an adapter method is called on a `DoubleEndedIterator<T>`, the return type's double-endedness depends on the method:

| Category | Methods | Rule |
|----------|---------|------|
| **Propagate** | `map`, `filter` | Returns DEI if receiver is DEI, Iterator otherwise |
| **Downgrade** | `take`, `skip`, `chain`, `zip`, `enumerate`, `flatten`, `flat_map`, `cycle` | Always returns plain Iterator |
| **Not Applicable** | Consumers (`collect`, `fold`, `count`, ...) | Returns non-iterator type |
| **Not Applicable** | `next`, `next_back` | Returns tuple, not iterator adapter |
| **DEI-Only** | `rev` | Returns DEI (only callable on DEI) |
| **DEI-Only** | `last`, `rfind`, `rfold` | Consumer — returns non-iterator |

### 07.4.2 Registry Encoding

```rust
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeiPropagation {
    /// This method propagates double-endedness from receiver to return type.
    /// If receiver is DEI, return is DEI. If receiver is Iterator, return is Iterator.
    Propagate,

    /// This method always returns a plain Iterator, regardless of receiver.
    Downgrade,

    /// This method does not return an iterator type (consumer or protocol).
    NotApplicable,
}
```

The type checker uses this when constructing the return type:

```rust
// Current code in resolve_iterator_method() (methods.rs line 832-837):
"map" => {
    let new_elem = engine.pool_mut().fresh_var();
    if is_dei {
        Some(engine.pool_mut().double_ended_iterator(new_elem))
    } else {
        Some(engine.pool_mut().iterator(new_elem))
    }
}

// Registry-driven equivalent:
let method_def = registry::find_method(TypeTag::Iterator, "map");
match method_def.dei_propagation {
    DeiPropagation::Propagate => {
        if is_dei { engine.pool_mut().double_ended_iterator(new_elem) }
        else { engine.pool_mut().iterator(new_elem) }
    }
    DeiPropagation::Downgrade => engine.pool_mut().iterator(new_elem),
    DeiPropagation::NotApplicable => { /* return type is not iterator */ }
}
```

---

## 07.5 Cross-Reference & Validation Tables

### 07.5.1 Complete Method x Phase Matrix

This table tracks every iterator method across all compiler phases. Each cell indicates whether the method is implemented in that phase.

**Legend:** Y = implemented, N = not implemented, N/A = not applicable, R = via resolver (not direct dispatch)

| Method | typeck (methods.rs) | eval (CollectionMethod) | eval (IteratorValue) | llvm (builtins/iterator.rs) | ori_ir (BUILTIN_METHODS) | consistency.rs |
|--------|-------------------|------------------------|---------------------|---------------------------|------------------------|---------------|
| `next` | Y | Y (IterNext) | List/Range/Str/Map/Set + all adapters | N (uses `__iter_next`) | N (collection type) | Tracked |
| `next_back` | Y (DEI) | Y (IterNextBack) | List/Str (double-ended) | N | N | Tracked |
| `map` | Y | Y (IterMap) | Mapped variant | Y | N | Tracked |
| `filter` | Y | Y (IterFilter) | Filtered variant | Y | N | Tracked |
| `take` | Y | Y (IterTake) | TakeN variant | Y | N | Tracked |
| `skip` | Y | Y (IterSkip) | SkipN variant | Y | N | Tracked |
| `chain` | Y | Y (IterChain) | Chained variant | Y | N | Tracked |
| `zip` | Y | Y (IterZip) | Zipped variant | Y | N | Tracked |
| `enumerate` | Y | Y (IterEnumerate) | Enumerated variant | Y | N | Tracked |
| `flatten` | Y | Y (IterFlatten) | Flattened variant | N | N | Tracked |
| `flat_map` | Y | Y (IterFlatMap) | Mapped+Flattened | N | N | Tracked |
| `cycle` | Y | Y (IterCycle) | Cycled variant | N | N | Tracked |
| `rev` | Y (DEI) | Y (IterRev) | Reversed variant | N | N | Tracked |
| `fold` | Y | Y (IterFold) | consumer | Y | N | Tracked |
| `rfold` | Y (DEI) | Y (IterRFold) | consumer | N | N | Tracked |
| `count` | Y | Y (IterCount) | consumer | Y | N | Tracked |
| `find` | Y | Y (IterFind) | consumer | Y | N | Tracked |
| `rfind` | Y (DEI) | Y (IterRFind) | consumer | N | N | Tracked |
| `any` | Y | Y (IterAny) | consumer | Y | N | Tracked |
| `all` | Y | Y (IterAll) | consumer | Y | N | Tracked |
| `for_each` | Y | Y (IterForEach) | consumer | Y | N | Tracked |
| `collect` | Y | Y (IterCollect) | consumer | Y | N | Tracked |
| `join` | Y | Y (IterJoin) | consumer | N | N | Tracked |
| `last` | Y (DEI) | Y (IterLast) | consumer | N | N | Tracked |

### 07.5.2 LLVM Coverage Gaps

Methods NOT in `ori_llvm` but in typeck+eval:

| Method | Gap Reason | Impact |
|--------|-----------|--------|
| `next` | LLVM uses `__iter_next` protocol instead | N/A — different protocol |
| `next_back` | DEI not yet in LLVM | AOT doesn't support DEI iteration |
| `flatten` | Requires nested iterator runtime support | AOT limitation |
| `flat_map` | Same as flatten | AOT limitation |
| `cycle` | Infinite iterator — runtime support needed | AOT limitation |
| `rev` | DEI not in LLVM | AOT doesn't support DEI |
| `rfold` | DEI not in LLVM | AOT doesn't support DEI |
| `rfind` | DEI not in LLVM | AOT doesn't support DEI |
| `last` | DEI not in LLVM | AOT doesn't support DEI |
| `join` | String formatting in iterator pipeline | AOT limitation |

These gaps are pre-existing and tracked. The registry does not create them, but it makes them visible in a single table rather than scattered across allowlists.

### 07.5.3 Methods in typeck but NOT in ITERATOR_METHOD_NAMES (eval)

Comparing `TYPECK_BUILTIN_METHODS` Iterator entries with `ITERATOR_METHOD_NAMES`:

**TYPECK has these Iterator methods:** all, any, chain, collect, count, cycle, enumerate, filter, find, flat_map, flatten, fold, for_each, join, map, next, skip, take, zip (19 methods)

**ITERATOR_METHOD_NAMES has these:** all, any, chain, collect, count, cycle, enumerate, filter, find, flat_map, flatten, fold, for_each, join, last, map, next, next_back, rev, rfind, rfold, skip, take, zip (24 methods)

**Difference:** `ITERATOR_METHOD_NAMES` includes DEI methods (last, next_back, rev, rfind, rfold) because the eval resolver handles both Iterator and DEI through the same code path. The TYPECK list separates them into "Iterator" and "DoubleEndedIterator" type entries.

This is correct behavior, not a gap. The registry will unify this by having a single TypeDef with `dei_only` flags.

### 07.5.4 IteratorValue Variants vs. Methods

Each IteratorValue variant in `ori_patterns` corresponds to a source type or an adapter method:

| IteratorValue Variant | Created By | Double-Ended |
|----------------------|-----------|--------------|
| `List` | `list.iter()` | Yes |
| `Range` | `range.iter()` | Yes (bounded ranges) |
| `Map` | `map.iter()` | No |
| `Set` | `set.iter()` | No |
| `Str` | `str.iter()` | Yes |
| `Mapped` | `.map(f)` | Inherits from source |
| `Filtered` | `.filter(f)` | Inherits from source |
| `TakeN` | `.take(n)` | No (downgrades) |
| `SkipN` | `.skip(n)` | No (downgrades) |
| `Enumerated` | `.enumerate()` | No (downgrades) |
| `Zipped` | `.zip(other)` | No (downgrades) |
| `Chained` | `.chain(other)` | No (downgrades) |
| `Flattened` | `.flatten()` / `.flat_map(f)` | No (downgrades) |
| `Cycled` | `.cycle()` | No (downgrades) |
| `Reversed` | `.rev()` | Yes (DEI source required) |
| `Repeat` | `repeat(value)` prelude fn | No |

**No variant needed for consumers** (fold, collect, count, etc.) because they eagerly produce a final value, not a new iterator.

---

## 07.6 Implementation Steps

### Step 1: Define MethodDef entries

- [ ] Create `ori_registry/src/defs/iterator.rs`
- [ ] Define `ITERATOR_METHODS: &[MethodDef]` with all 24 user-callable methods
- [ ] Set `receiver: Ownership::Borrow` on all entries
- [ ] Set `dei_only: bool` on each entry (true for: next_back, rev, last, rfind, rfold)
- [ ] Set `dei_propagation: DeiPropagation` on each adapter entry
- [ ] Define `ITERATOR: TypeDef` referencing `ITERATOR_METHODS`

### Step 2: Define return type specifications

- [ ] Extend `ReturnType` enum (from Section 01) to handle:
  - `IteratorOf(inner)` — constructs `Iterator<inner>` in the type pool
  - `DeiOf(inner)` — constructs `DoubleEndedIterator<inner>` in the type pool
  - `OptionOf(inner)` — constructs `Option<inner>`
  - `ListOf(inner)` — constructs `[inner]`
  - `TupleOf(&[ReturnType])` — constructs tuple
  - `FreshVar` — creates a fresh type variable (for higher-order methods)
  - `Element` — the T in Iterator<T>
  - `ReceiverType` — same type as the receiver
- [ ] Verify all 24 methods' return types are expressible

### Step 3: Verify const-constructibility

- [ ] All `MethodDef` entries must be `const`-constructible
- [ ] `DeiPropagation`, `Ownership` must derive `Copy, Clone, Debug, Eq, PartialEq`
- [ ] `ITERATOR` TypeDef must compile as a `const` or `static`
- [ ] `cargo c -p ori_registry` must pass

### Step 4: Add query helpers

- [ ] `dei_only_methods() -> impl Iterator<Item = &str>` — replaces `DEI_ONLY_METHODS` constant
- [ ] `iterator_method_names() -> impl Iterator<Item = &str>` — replaces `ITERATOR_METHOD_NAMES`
- [ ] `is_dei_only(method: &str) -> bool` — replaces `DEI_ONLY_METHODS.contains()`

### Step 5: Validation tests

- [ ] Test: every method in `TYPECK_BUILTIN_METHODS` for "Iterator" has a registry entry
- [ ] Test: every method in `TYPECK_BUILTIN_METHODS` for "DoubleEndedIterator" has a registry entry with `dei_only: true`
- [ ] Test: every method in `ITERATOR_METHOD_NAMES` has a registry entry
- [ ] Test: `dei_only_methods()` returns exactly {"next_back", "rev", "last", "rfind", "rfold"}
- [ ] Test: every adapter method has a `dei_propagation` that is `Propagate` or `Downgrade`
- [ ] Test: every consumer method has `dei_propagation == NotApplicable`
- [ ] Test: `ITERATOR_METHODS` is sorted by name (for binary search / deterministic iteration)

---

## 07.7 Design Decision Log

### Decision 1: Single TypeDef with dei_only flag (Option C)

**Chosen over:** Separate TypeDefs (A) or inheritance chain (B).
**Rationale:** Matches current code structure. No method duplication. No new inheritance mechanism needed. The `dei_only` flag is cheap and directly queryable.

### Decision 2: DeiPropagation as explicit field, not derived from return type

**Chosen over:** Inferring propagation from `ReturnType` variants.
**Rationale:** The return type alone is ambiguous: `IteratorOf(Element)` could be either propagated or downgraded. The propagation rule depends on the method's semantics (e.g., `take` downgrades because bounded iteration breaks reversibility). Making it an explicit field makes the semantics clear and auditable.

### Decision 3: Internal methods (__iter_next, __collect_set) NOT in registry

**Chosen over:** Including them with an `internal: bool` flag.
**Rationale:** These are compiler-internal implementation details, not part of the user-facing type. `__iter_next` is the ARC lowering protocol; `__collect_set` is a type-directed rewrite. Neither should appear in diagnostics, documentation, or user-facing method lists. The LLVM backend can handle them through its own dispatch table without registry backing.

---

## Exit Criteria

- [ ] `ori_registry/src/defs/iterator.rs` exists and compiles (`cargo c -p ori_registry`)
- [ ] `ITERATOR` TypeDef contains exactly 24 user-callable method entries
- [ ] Every method entry has correct: `receiver`, `params`, `returns`, `dei_only`, `dei_propagation`
- [ ] `dei_only_methods()` returns exactly 5 method names matching `DEI_ONLY_METHODS`
- [ ] `iterator_method_names()` returns exactly 24 method names matching `ITERATOR_METHOD_NAMES` (including DEI methods)
- [ ] All validation tests in Step 5 pass
- [ ] No dependencies added to `ori_registry` (purity maintained)
- [ ] Return types for all methods are expressible in the `ReturnType` enum without hacks
- [ ] DeiPropagation assignments match the exact branching in current `resolve_iterator_method()`

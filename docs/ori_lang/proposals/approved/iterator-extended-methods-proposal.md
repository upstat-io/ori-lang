# Proposal: Extended Iterator Methods

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-22
**Approved:** 2026-03-22
**Affects:** Registry, type checker, evaluator, LLVM codegen, spec (Clause 13, Annex C)

---

## Summary

Add 19 commonly-needed methods to the `Iterator` and `DoubleEndedIterator` traits. These methods eliminate the most frequent `fold`-based workarounds and close the gap with Rust's standard iterator API — without adding complexity.

The additions fall into five categories:

| Category | Methods |
|----------|---------|
| **Selection** | `filter_map`, `take_while`, `skip_while`, `position`, `nth` |
| **Reduction** | `max`, `min`, `max_by`, `min_by`, `max_by_key`, `min_by_key`, `sum`, `sum_by`, `product`, `reduce` |
| **Inspection** | `inspect` |
| **Partitioning** | `partition` |
| **Stepping** | `step_by` |
| **DoubleEnded** | `rposition` |

---

## Motivation

### The Problem

Several extremely common iterator patterns require manual `fold` implementations today:

```ori
// Finding the maximum — 6 lines of boilerplate for a one-concept operation
let $max = scores.iter().fold(initial: None, op: (best, score) -> match best {
    None -> Some(score),
    Some(b) -> Some(if score > b then score else b),
})

// Filtering and transforming in one pass — requires two passes today
let $names = entries.iter()
    .filter(predicate: e -> match e { Active(_, _) -> true, _ -> false })
    .map(transform: e -> match e { Active(name, _) -> name, _ -> "" })

// Summing — trivial concept, non-trivial expression
let $total = scores.iter().fold(initial: 0, op: (acc, x) -> acc + x)

// Taking while a condition holds — impossible without manual loop
let first_positives = ???  // no take_while
```

### After This Proposal

```ori
let $max = scores.iter().max()
let $names = entries.iter().filter_map(transform: e -> match e {
    Active(name, _) -> Some(name), _ -> None,
})
let $total = scores.iter().sum()
let $first_positives = scores.iter().take_while(predicate: s -> s > 0)
```

### Design Principle

Every method in this proposal satisfies **at least one** of:
1. **Eliminates a `fold` pattern used in 3+ existing spec tests** (measured)
2. **Enables single-pass evaluation** where the current workaround requires multiple passes
3. **Has a direct counterpart in Rust, Python, Haskell, AND Kotlin** (cross-language consensus)

Methods that don't meet these criteria (e.g., `scan`, `group_by`, `windows`, `chunks`) are deferred to a future proposal.

---

## Design

### Tier 1 — Reduction Methods

These operate on the full iterator and return a single value. They are **consumers** (exhaust the iterator).

#### `max` / `min`

```ori
@max (self) -> Option<Self.Item> where Self.Item: Comparable
@min (self) -> Option<Self.Item> where Self.Item: Comparable
```

Returns the maximum/minimum element, or `None` if the iterator is empty.

```ori
[3, 1, 4, 1, 5].iter().max()   // Some(5)
[3, 1, 4, 1, 5].iter().min()   // Some(1)
let empty: [int] = []
empty.iter().max()               // None
```

**Default implementation:**
```ori
@max (self) -> Option<Self.Item> where Self.Item: Comparable =
    self.reduce(op: (a, b) -> if compare(left: a, right: b).is_greater_or_equal() then a else b)

@min (self) -> Option<Self.Item> where Self.Item: Comparable =
    self.reduce(op: (a, b) -> if compare(left: a, right: b).is_less_or_equal() then a else b)
```

**Trait bound:** Requires `Comparable` — not `Eq`. Total ordering is needed to define a meaningful maximum/minimum.

#### `max_by` / `min_by`

```ori
@max_by (self, compare: (Self.Item, Self.Item) -> Ordering) -> Option<Self.Item>
@min_by (self, compare: (Self.Item, Self.Item) -> Ordering) -> Option<Self.Item>
```

Returns the maximum/minimum element using a custom comparison function.

```ori
let $entries = [Active(name: "Alice", score: 95), Active(name: "Bob", score: 72)]
entries.iter().max_by(compare: (a, b) -> match (a, b) {
    (Active(_, s1), Active(_, s2)) -> compare(left: s1, right: s2),
    _ -> Equal,
})
// Some(Active(name: "Alice", score: 95))
```

**Default implementation:**
```ori
@max_by (self, compare: (Self.Item, Self.Item) -> Ordering) -> Option<Self.Item> =
    self.reduce(op: (a, b) -> if compare(a, b).is_greater_or_equal() then a else b)
```

#### `max_by_key` / `min_by_key`

```ori
@max_by_key<K: Comparable> (self, key: (Self.Item) -> K) -> Option<Self.Item>
@min_by_key<K: Comparable> (self, key: (Self.Item) -> K) -> Option<Self.Item>
```

Returns the element with the maximum/minimum extracted key. Mirrors List's `max(by:)` / `min(by:)` API for iterator chains.

```ori
let $entries = [Active(name: "Alice", score: 95), Active(name: "Bob", score: 72)]
entries.iter().max_by_key(key: e -> match e { Active(_, s) -> s, _ -> 0 })
// Some(Active(name: "Alice", score: 95))

// Equivalent to List's:
entries.max(by: e -> match e { Active(_, s) -> s, _ -> 0 })
```

**Default implementation:**
```ori
@max_by_key<K: Comparable> (self, key: (Self.Item) -> K) -> Option<Self.Item> =
    self.max_by(compare: (a, b) -> compare(left: key(a), right: key(b)))
```

NOTE  `max_by_key`/`min_by_key` complement the zero-arg `max()`/`min()` (for `Comparable` items) and the comparator-based `max_by`/`min_by` (for custom ordering). List has `max(by:)` which extracts a key; `max_by_key` is the lazy iterator equivalent.

#### `sum` / `sum_by`

```ori
@sum (self) -> Self.Item where Self.Item: Add + Default
@sum_by<N: Add + Default> (self, op: (Self.Item) -> N) -> N
```

`sum` reduces the iterator using addition, starting from the type's `Default` value.
`sum_by` extracts a numeric value from each element before summing. Mirrors List's `sum(op:)` API.

```ori
[1, 2, 3, 4, 5].iter().sum()                          // 15
let empty: [int] = []
empty.iter().sum()                                      // 0  (Default for int)

// sum_by — extract a field to sum
entries.iter().sum_by(op: e -> match e { Active(_, s) -> s, _ -> 0 })
// Equivalent to List's: entries.sum(op: e -> ...)
```

**Default implementation:**
```ori
@sum (self) -> Self.Item where Self.Item: Add + Default =
    self.fold(initial: Default.default(), op: (acc, x) -> acc + x)

@sum_by<N: Add + Default> (self, op: (Self.Item) -> N) -> N =
    self.map(transform: op).sum()
```

#### `product`

```ori
@product (self) -> Option<Self.Item> where Self.Item: Mul
```

Reduces the iterator using multiplication. Returns `Option` — `None` for empty iterators. This avoids the multiplicative identity problem (`Default` for `int` is 0, not 1) without requiring a `One` trait.

```ori
[1, 2, 3, 4, 5].iter().product()   // Some(120)
let empty: [int] = []
empty.iter().product()               // None
```

**Default implementation:**
```ori
@product (self) -> Option<Self.Item> where Self.Item: Mul =
    self.reduce(op: (acc, x) -> acc * x)
```

NOTE  `product` returns `Option` rather than a bare value because there is no universal multiplicative identity. For `int`, the identity is `1`, but `Default` returns `0`. Rather than introducing a `One` trait or hard-coding `1`, `product` delegates to `reduce` which naturally returns `None` for empty iterators.

#### `reduce`

```ori
@reduce (self, op: (Self.Item, Self.Item) -> Self.Item) -> Option<Self.Item>
```

Like `fold`, but uses the first element as the initial value. Returns `None` on empty iterator.

```ori
[1, 2, 3, 4].iter().reduce(op: (a, b) -> a + b)  // Some(10)
let empty: [int] = []
empty.iter().reduce(op: (a, b) -> a + b)            // None
```

**Default implementation:**
```ori
@reduce (self, op: (Self.Item, Self.Item) -> Self.Item) -> Option<Self.Item> = {
    let (first, iter) = self.next();
    match first {
        None -> None,
        Some(init) -> Some(iter.fold(initial: init, op:)),
    }
}
```

**Why both `fold` and `reduce`?** `fold` takes an initial value and can change the accumulator type (`fold<U>`). `reduce` requires no initial value but constrains the result to `Self.Item`. Different tools for different jobs: `fold(initial: 0, op: (acc, x) -> acc + x.len())` vs `reduce(op: (a, b) -> a + b)`.

---

### Tier 2 — Adapter Methods

These return new lazy iterators. They are **adapters** (do not consume).

#### `filter_map`

```ori
@filter_map<U> (self, transform: (Self.Item) -> Option<U>) -> FilterMapIterator<Self, U>
```

Combined filter + map in a single pass. The transform returns `Some(value)` to keep, `None` to skip.

```ori
let $entries = [Active(name: "Alice", score: 95), Inactive(name: "Bob"), Unknown]
entries.iter().filter_map(transform: e -> match e {
    Active(name, _) -> Some(name),
    _ -> None,
})
// Iterator yielding: "Alice"
```

**This is the single most requested missing method.** It replaces the two-pass `.filter().map()` pattern and eliminates the need for sentinel values (the `""` in the current workaround).

**Default implementation:**
```ori
@filter_map<U> (self, transform: (Self.Item) -> Option<U>) -> FilterMapIterator<Self, U> =
    FilterMapIterator { source: self, transform: transform }
```

**FilterMapIterator next:**
```ori
@next (self) -> (Option<U>, FilterMapIterator<Source, U>) = {
    let iter = self.source;
    loop {
        match iter.next() {
            (None, done) -> break (None, FilterMapIterator { source: done, transform: self.transform }),
            (Some(item), next) -> match self.transform(item) {
                Some(value) -> break (Some(value), FilterMapIterator { source: next, transform: self.transform }),
                None -> iter = next,
            },
        }
    }
}
```

#### `take_while` / `skip_while`

```ori
@take_while (self, predicate: (Self.Item) -> bool) -> TakeWhileIterator<Self>
@skip_while (self, predicate: (Self.Item) -> bool) -> SkipWhileIterator<Self>
```

`take_while` yields elements until the predicate returns `false`, then stops permanently.
`skip_while` skips elements until the predicate returns `false`, then yields everything after.

```ori
[1, 2, 3, 4, 5, 1, 2].iter().take_while(predicate: x -> x < 4).collect()
// [1, 2, 3]

[1, 2, 3, 4, 5, 1, 2].iter().skip_while(predicate: x -> x < 4).collect()
// [4, 5, 1, 2]
```

**Key semantic:** `take_while` is fused — once it stops, it never yields again, even if later elements would match. This matches Rust, Haskell, and Kotlin.

#### `step_by`

```ori
@step_by (self, step: int) -> StepByIterator<Self>
    pre(step > 0 | "step must be positive")
```

Yields every `step`-th element, starting with the first.

```ori
[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].iter().step_by(step: 3).collect()
// [0, 3, 6, 9]

(0..100).iter().step_by(step: 10).collect()
// [0, 10, 20, 30, 40, 50, 60, 70, 80, 90]
```

#### `inspect`

```ori
@inspect (self, f: (Self.Item) -> void) -> InspectIterator<Self>
```

Calls `f` on each element as it passes through, without modifying the iterator. The closure must be pure — no capability effects are permitted. This is intended for observing values during chain construction (e.g., accumulating into a captured mutable variable), not for performing IO.

```ori
let seen = [];
[1, 2, 3].iter()
    .inspect(f: x -> seen = [...seen, x])
    .filter(predicate: x -> x > 1)
    .collect()
// seen = [1, 2, 3]  (all elements observed, even filtered ones)
// result = [2, 3]
```

For debugging with output, use `dbg(value:)` at specific points or `for_each(f:)` as a terminal consumer.

---

### Tier 3 — Index and Partition Methods

These are consumers or adapters that deal with element positions.

#### `position`

```ori
@position (self, predicate: (Self.Item) -> bool) -> Option<int>
```

Returns the index of the first element matching the predicate.

```ori
["a", "b", "c", "d"].iter().position(predicate: x -> x == "c")  // Some(2)
["a", "b", "c"].iter().position(predicate: x -> x == "z")        // None
```

**Default implementation:**
```ori
@position (self, predicate: (Self.Item) -> bool) -> Option<int> =
    self.enumerate()
        .find(predicate: (i, item) -> predicate(item))
        .map(transform: (i, _) -> i)
```

#### `nth`

```ori
@nth (self, n: int) -> Option<Self.Item>
    pre(n >= 0 | "index must be non-negative")
```

Returns the `n`-th element (0-indexed), consuming all elements up to it.

```ori
[10, 20, 30, 40].iter().nth(n: 2)  // Some(30)
[10, 20].iter().nth(n: 5)           // None
```

**Default implementation:**
```ori
@nth (self, n: int) -> Option<Self.Item> = {
    let (value, _) = self.skip(count: n).next();
    value
}
```

#### `partition`

```ori
@partition (self, predicate: (Self.Item) -> bool) -> ([Self.Item], [Self.Item])
```

Splits the iterator into two lists: elements matching the predicate and elements that don't.

```ori
let (evens, odds) = [1, 2, 3, 4, 5, 6].iter().partition(predicate: x -> x % 2 == 0)
// evens = [2, 4, 6]
// odds  = [1, 3, 5]
```

**Default implementation:**
```ori
@partition (self, predicate: (Self.Item) -> bool) -> ([Self.Item], [Self.Item]) =
    self.fold(initial: ([], []), op: (acc, item) -> {
        let (yes, no) = acc;
        if predicate(item) then ([...yes, item], no)
        else (yes, [...no, item])
    })
```

---

### Tier 4 — DoubleEndedIterator Extension

#### `rposition`

```ori
@rposition (self, predicate: (Self.Item) -> bool) -> Option<int>
```

Like `position`, but searches from the back. Returns the index relative to the front (not the back).

```ori
["a", "b", "c", "b", "a"].iter().rposition(predicate: x -> x == "b")  // Some(3)
```

NOTE  `rposition` requires a `DoubleEndedIterator` with known length (e.g., list-backed). The implementation counts from the back and converts to a front-relative index using the iterator's `size_hint`.

---

## Trait Bounds Summary

| Method | Bound on `Self.Item` | Rationale |
|--------|----------------------|-----------|
| `max` / `min` | `Comparable` | Total ordering required |
| `max_by` / `min_by` | (none) | Custom comparator provided |
| `max_by_key` / `min_by_key` | (none on Item; `K: Comparable`) | Key extraction + ordering |
| `sum` | `Add + Default` | Needs `+` and zero value |
| `sum_by` | (none on Item; `N: Add + Default`) | Key extraction + addition |
| `product` | `Mul` | Returns `Option`, no identity needed |
| `reduce` | (none) | Uses first element as init |
| `filter_map` | (none) | Transform provides filtering |
| `take_while` / `skip_while` | (none) | Predicate only |
| `step_by` | (none) | Index-based |
| `inspect` | (none) | Pure observation only |
| `position` / `rposition` | (none) | Predicate only |
| `nth` | (none) | Index-based |
| `partition` | (none) | Predicate only |

---

## Relationship to List Methods

Several methods in this proposal have counterparts on the `List` type with different APIs:

| Iterator method | List method | Difference |
|-----------------|-------------|------------|
| `max()` (no args, `Comparable` bound) | `max(by: closure)` (key extraction) | Iterator: trait-bound, zero-arg. List: closure-based, flexible. |
| `max_by_key(key:)` | `max(by:)` | Equivalent functionality, different names |
| `min()` / `min_by_key(key:)` | `min(by:)` | Same as max/max_by_key |
| `sum()` (no args, `Add + Default` bound) | `sum(op: closure)` (key extraction) | Iterator: trait-bound. List: closure-based. |
| `sum_by(op:)` | `sum(op:)` | Equivalent functionality |
| `take_while(predicate:)` → lazy adapter | `take_while(predicate:)` → new list | Iterator: lazy. List: eager. |
| `skip_while(predicate:)` → lazy adapter | `skip_while(predicate:)` → new list | Iterator: lazy. List: eager. |
| `partition(predicate:)` | `partition(predicate:)` | Both materialize; semantically equivalent |

Both APIs coexist intentionally. List methods are convenient for direct collection operations; Iterator methods enable lazy pipeline composition. The `_by_key` and `_by` variants on Iterator mirror the key-extraction style of List's API.

---

## Implementation Plan

### Phase 1 — High-Impact Reductions (max, min, sum, reduce, product, max_by_key, min_by_key, sum_by)

These are the most commonly hand-rolled patterns in existing code.

1. Add `MethodDef` entries to `ori_registry/src/defs/iterator/mod.rs`
2. Add type signatures in `ori_types` method resolution
3. Add evaluation in `ori_eval/src/interpreter/method_dispatch/iterator/consumers.rs`
4. Tests: registry integrity, spec tests in `tests/spec/traits/iterator/`

**No new iterator variants needed** — these are all consumers that return values, not adapters.

### Phase 2 — filter_map, take_while, skip_while

These require new `IteratorValue` variants (lazy adapters).

1. Add `FilterMap`, `TakeWhile`, `SkipWhile` variants to `IteratorValue` enum
2. Implement `next()` for each in `ori_eval/src/interpreter/method_dispatch/iterator/next.rs`
3. Wire up `size_hint`: `filter_map` → `(0, source_upper)`, `take_while` → `(0, source_upper)`, `skip_while` → `(0, source_upper)`
4. Add `is_double_ended()`: all three return `false` (these adapters are forward-only)

### Phase 3 — step_by, inspect, position, nth, partition, rposition

Lower priority; each is straightforward given Phase 1-2 infrastructure.

- `step_by` → new `StepBy` adapter variant
- `inspect` → new `Inspect` adapter variant
- `position`, `nth`, `partition`, `rposition` → consumers only (no new variants)

### Phase 4 — LLVM Codegen

Add AOT support for all new methods, following existing patterns in `ori_llvm/src/codegen/`.

---

## Methods Explicitly NOT in This Proposal

| Method | Reason | Future? |
|--------|--------|---------|
| `scan` | Stateful map; niche usage; compose with `fold` + intermediate state | Maybe |
| `windows` / `chunks` | Requires borrowing semantics or list-of-lists allocation; design not settled | Separate proposal |
| `group_by` | Requires key extraction + grouping semantics; multiple valid designs | Separate proposal |
| `zip_longest` | Requires `Either` type or padding strategy; design not settled | After `Either` type |
| `peekable` | Wraps iterator with look-ahead; conflicts with immutable iterator model | Needs design work |
| `unzip` | Inverse of zip; niche | Maybe |
| `cloned` / `copied` | Ori has ARC+COW; these Rust-isms don't apply | Never |

---

## Interaction with Existing Features

### `for..yield` Comprehensions

These new methods do not affect `for..yield` desugaring. The relationship is:
- `for..yield` is sugar for `.map().collect()` (simple transforms)
- Iterator chains are for multi-step lazy pipelines
- Both remain valid; programmer chooses based on readability

### Parallel Iterators (draft proposal)

The parallel iterators proposal (`parallel-iterators-proposal.md`) will need parallel variants of `max`, `min`, `sum`, `product`, `reduce`, and `partition`. This proposal establishes the sequential API that the parallel proposal will mirror.

### `for..yield` with Guards

`for x in items if pred yield expr` is equivalent to `.filter(predicate: pred).map(transform: x -> expr)`. With `filter_map`, this pattern can also be expressed as a single-pass operation when the filter and map are entangled (e.g., extracting from sum type variants).

---

## Prior Art

| Method | Rust | Haskell | Kotlin | Python | Elixir |
|--------|------|---------|--------|--------|--------|
| `filter_map` | `filter_map` | `mapMaybe` | `mapNotNull` | — | — |
| `max` / `min` | `max` / `min` | `maximum` / `minimum` | `max` / `min` | `max()` / `min()` | `Enum.max` / `Enum.min` |
| `max_by_key` / `min_by_key` | `max_by_key` / `min_by_key` | `maximumBy . comparing` | `maxByOrNull` | `max(key=)` | `Enum.max_by` |
| `sum` / `product` | `sum` / `product` | `sum` / `product` | `sum()` | `sum()` | `Enum.sum` |
| `reduce` | `reduce` | `foldl1` | `reduce` | `functools.reduce` | `Enum.reduce` |
| `take_while` | `take_while` | `takeWhile` | `takeWhile` | `itertools.takewhile` | `Enum.take_while` |
| `skip_while` | `skip_while` | `dropWhile` | `dropWhile` | `itertools.dropwhile` | `Enum.drop_while` |
| `position` | `position` | `elemIndex` | `indexOfFirst` | — | `Enum.find_index` |
| `partition` | `partition` | `partition` | `partition` | — | `Enum.split_with` |
| `inspect` | `inspect` | `trace` | `onEach` | — | `Enum.each` (eager) |
| `step_by` | `step_by` | — | `step` (on ranges) | `range(0,n,step)` | — |
| `nth` | `nth` | `!!` | `elementAt` | `next(islice(it,n,n+1))` | `Enum.at` |

Every method in this proposal exists in **at least 3** of these 6 languages.

---

## Summary of Changes

**Registry:** 19 new `MethodDef` entries in `ori_registry/src/defs/iterator/mod.rs`
**New iterator variants:** 5 (`FilterMap`, `TakeWhile`, `SkipWhile`, `StepBy`, `Inspect`)
**New consumer functions:** 14 (`max`, `min`, `max_by`, `min_by`, `max_by_key`, `min_by_key`, `sum`, `sum_by`, `product`, `reduce`, `position`, `nth`, `partition`, `rposition`)
**Trait bounds required:** `Comparable` (for `max`/`min`/`*_by_key`), `Add + Default` (for `sum`/`sum_by`), `Mul` (for `product`)
**Tests:** ~100 new spec tests across `tests/spec/traits/iterator/`
**Spec updates:** Clause 13 (Iterator), Annex C (Built-ins)

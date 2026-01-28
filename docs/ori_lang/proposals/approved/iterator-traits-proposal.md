# Proposal: Iterator Traits

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-27
**Approved:** 2026-01-28

---

## Summary

Formalize iteration in Ori with four core traits: `Iterator`, `DoubleEndedIterator`, `Iterable`, and `Collect`. These traits enable generic code over any iterable, allow user types to participate in `for` loops, and provide the foundation for `.map()`, `.filter()`, and other transformation methods.

```ori
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}

trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}

trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}

trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```

---

## Motivation

### The Problem

Ori currently has iteration syntax and methods:

```ori
for item in items do process(item: item)
items.map(transform: x -> x * 2)
(0..10).collect()
```

But without formalized traits:
1. User-defined types can't participate in `for` loops
2. Generic functions can't accept "anything iterable"
3. `.map()`, `.filter()` must be duplicated per collection type
4. The relationship between iteration and collection is implicit

### The Ori Way

Ori values explicit contracts. Traits formalize what "iterable" and "collectable" mean:

```ori
// Generic function over any iterable
@sum<I: Iterable> (items: I) -> int where I.Item == int =
    items.iter().fold(initial: 0, op: (acc, x) -> acc + x)

// Works with any iterable
sum(items: [1, 2, 3])
sum(items: 1..10)
sum(items: my_custom_collection)
```

---

## Design

### Core Traits

#### Iterator

The fundamental trait for step-by-step iteration:

```ori
trait Iterator {
    type Item

    @next (self) -> (Option<Self.Item>, Self)

    // Default implementations (see below)
}
```

- `Item` — the type of values produced
- `next` — returns `(Some(value), updated_iterator)` or `(None, exhausted_iterator)`
- Returns tuple of value and updated iterator (functional, fits Ori's immutable parameter semantics)

**Fused Guarantee:** Once `next()` returns `(None, iter)`, all subsequent calls to `iter.next()` must return `(None, _)`. Iterators must track exhaustion state.

#### DoubleEndedIterator

Iterators that can traverse from both ends:

```ori
trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}
```

- Enables efficient `last()`, `rev()`, and reverse traversal
- Same fused guarantee applies to `next_back()`

#### Iterable

Types that can produce an iterator:

```ori
trait Iterable {
    type Item

    @iter (self) -> impl Iterator where Item == Self.Item
}
```

- Separates "being iterable" from "being an iterator"
- A collection can be iterated multiple times (produces fresh iterator each time)
- An iterator itself is iterable (returns self)

#### Collect

Types that can be built from an iterator:

```ori
trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```

- Enables `.collect()` method on iterators
- Type annotation determines target: `iter.collect() as [int]`

### Iterator Default Methods

The `Iterator` trait provides default implementations:

```ori
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)

    // Transformation
    @map<U> (self, transform: (Self.Item) -> U) -> MapIterator<Self, U> =
        MapIterator { source: self, transform: transform }

    @filter (self, predicate: (Self.Item) -> bool) -> FilterIterator<Self> =
        FilterIterator { source: self, predicate: predicate }

    // Reduction
    @fold<U> (self, initial: U, op: (U, Self.Item) -> U) -> U = run(
        let acc = initial,
        let iter = self,
        loop(
            match(
                iter.next(),
                (Some(item), next_iter) -> run(
                    acc = op(acc, item),
                    iter = next_iter,
                    continue,
                ),
                (None, _) -> break acc,
            ),
        ),
    )

    @find (self, predicate: (Self.Item) -> bool) -> Option<Self.Item> = run(
        let iter = self,
        loop(
            match(
                iter.next(),
                (Some(item), next_iter) ->
                    if predicate(item) then break Some(item)
                    else run(iter = next_iter, continue),
                (None, _) -> break None,
            ),
        ),
    )

    // Collection
    @collect<C: Collect<Self.Item>> (self) -> C =
        C.from_iter(iter: self)

    // Counting
    @count (self) -> int =
        self.fold(initial: 0, op: (acc, _) -> acc + 1)

    // Predicates
    @any (self, predicate: (Self.Item) -> bool) -> bool =
        self.find(predicate: predicate).is_some()

    @all (self, predicate: (Self.Item) -> bool) -> bool =
        !self.any(predicate: item -> !predicate(item))

    // Slicing
    @take (self, count: int) -> TakeIterator<Self> =
        TakeIterator { source: self, remaining: count }

    @skip (self, count: int) -> SkipIterator<Self> =
        SkipIterator { source: self, remaining: count }

    // Combining
    @enumerate (self) -> EnumerateIterator<Self> =
        EnumerateIterator { source: self, index: 0 }

    @zip<Other: Iterator> (self, other: Other) -> ZipIterator<Self, Other> =
        ZipIterator { first: self, second: other }

    @chain<Other: Iterator> (self, other: Other) -> ChainIterator<Self, Other>
        where Other.Item == Self.Item
    = ChainIterator { first: Some(self), second: other }

    // Flattening
    @flatten (self) -> FlattenIterator<Self>
        where Self.Item: Iterable
    = FlattenIterator { outer: self, inner: None }

    @flat_map<U, I: Iterable> (self, transform: (Self.Item) -> I) -> FlattenIterator<MapIterator<Self, I>>
        where I.Item == U
    = self.map(transform: transform).flatten()

    // Infinite iteration
    @cycle (self) -> CycleIterator<Self> where Self: Clone =
        CycleIterator { original: self.clone(), current: self }
}
```

### DoubleEndedIterator Default Methods

```ori
trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)

    @rev (self) -> RevIterator<Self> =
        RevIterator { source: self }

    @last (self) -> Option<Self.Item> = run(
        let result: Option<Self.Item> = None,
        let iter = self,
        loop(
            match(
                iter.next_back(),
                (Some(item), _) -> break Some(item),
                (None, _) -> break result,
            ),
        ),
    )

    @rfind (self, predicate: (Self.Item) -> bool) -> Option<Self.Item> = run(
        let iter = self,
        loop(
            match(
                iter.next_back(),
                (Some(item), next_iter) ->
                    if predicate(item) then break Some(item)
                    else run(iter = next_iter, continue),
                (None, _) -> break None,
            ),
        ),
    )

    @rfold<U> (self, initial: U, op: (U, Self.Item) -> U) -> U = run(
        let acc = initial,
        let iter = self,
        loop(
            match(
                iter.next_back(),
                (Some(item), next_iter) -> run(
                    acc = op(acc, item),
                    iter = next_iter,
                    continue,
                ),
                (None, _) -> break acc,
            ),
        ),
    )
}
```

### Infinite Iterator: repeat

A standalone function for creating infinite iterators:

```ori
@repeat<T: Clone> (value: T) -> RepeatIterator<T> =
    RepeatIterator { value: value }

type RepeatIterator<T> = { value: T }

impl<T: Clone> Iterator for RepeatIterator<T> {
    type Item = T
    @next (self) -> (Option<T>, RepeatIterator<T>) =
        (Some(self.value.clone()), self)
}
```

Usage:

```ori
let zeros = repeat(value: 0).take(count: 100).collect()  // [0, 0, ..., 0]
let pattern = repeat(value: "ab").take(count: 5).collect()  // ["ab", "ab", "ab", "ab", "ab"]
```

### For Loop Desugaring

The `for` loop desugars to use these traits:

```ori
// This:
for x in items do
    process(x: x)

// Desugars to:
run(
    let iter = items.iter(),
    loop(
        match(
            iter.next(),
            (Some(x), next_iter) -> run(
                process(x: x),
                iter = next_iter,
                continue,
            ),
            (None, _) -> break,
        ),
    ),
)
```

For `for...yield`:

```ori
// This:
for x in items yield x * 2

// Desugars to:
items.iter().map(transform: x -> x * 2).collect()
```

### Standard Implementations

#### Primitives and Built-ins

```ori
// Lists are iterable and double-ended
impl<T> Iterable for [T] {
    type Item = T
    @iter (self) -> ListIterator<T> = ListIterator { list: self, front: 0, back: len(collection: self) }
}

impl<T> Collect<T> for [T] {
    @from_iter (iter: impl Iterator where Item == T) -> [T] = /* intrinsic */
}

// Maps are iterable (yields key-value tuples, NOT double-ended — unordered)
impl<K, V> Iterable for {K: V} {
    type Item = (K, V)
    @iter (self) -> MapEntryIterator<K, V> = /* intrinsic */
}

// Sets are iterable (NOT double-ended — unordered)
impl<T> Iterable for Set<T> {
    type Item = T
    @iter (self) -> SetIterator<T> = /* intrinsic */
}

impl<T> Collect<T> for Set<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Set<T> = /* intrinsic */
}

// Strings are iterable and double-ended (yields characters)
impl Iterable for str {
    type Item = char
    @iter (self) -> CharIterator = /* intrinsic */
}

// Integer ranges are iterable and double-ended
// Note: Range<float> does NOT implement Iterable (precision issues)
impl Iterable for Range<int> {
    type Item = int
    @iter (self) -> RangeIterator<int> =
        RangeIterator { current: self.start, end: self.end, step: self.step }
}

// Option is iterable (zero or one element)
impl<T> Iterable for Option<T> {
    type Item = T
    @iter (self) -> OptionIterator<T> = OptionIterator { value: self }
}

// Iterators are iterable (return self)
impl<I: Iterator> Iterable for I {
    type Item = I.Item
    @iter (self) -> I = self
}
```

#### Helper Iterator Types

```ori
type ListIterator<T> = { list: [T], front: int, back: int }

impl<T> Iterator for ListIterator<T> {
    type Item = T
    @next (self) -> (Option<T>, ListIterator<T>) =
        if self.front < self.back then
            (Some(self.list[self.front]), ListIterator { list: self.list, front: self.front + 1, back: self.back })
        else
            (None, self)
}

impl<T> DoubleEndedIterator for ListIterator<T> {
    @next_back (self) -> (Option<T>, ListIterator<T>) =
        if self.front < self.back then
            (Some(self.list[self.back - 1]), ListIterator { list: self.list, front: self.front, back: self.back - 1 })
        else
            (None, self)
}

type RangeIterator<T> = { current: T, end: T, step: T }

impl Iterator for RangeIterator<int> {
    type Item = int
    @next (self) -> (Option<int>, RangeIterator<int>) =
        if (self.step > 0 && self.current < self.end) || (self.step < 0 && self.current > self.end) then
            (Some(self.current), RangeIterator { current: self.current + self.step, end: self.end, step: self.step })
        else
            (None, self)
}

impl DoubleEndedIterator for RangeIterator<int> {
    @next_back (self) -> (Option<int>, RangeIterator<int>) = run(
        let back_pos = if self.step > 0 then
            self.end - 1 - ((self.end - 1 - self.current) % self.step)
        else
            self.end + 1 - ((self.current - self.end - 1) % (-self.step)),
        if (self.step > 0 && back_pos >= self.current) || (self.step < 0 && back_pos <= self.current) then
            (Some(back_pos), RangeIterator { current: self.current, end: back_pos, step: self.step })
        else
            (None, self),
    )
}

type MapIterator<I: Iterator, U> = { source: I, transform: (I.Item) -> U }

impl<I: Iterator, U> Iterator for MapIterator<I, U> {
    type Item = U
    @next (self) -> (Option<U>, MapIterator<I, U>) =
        match(
            self.source.next(),
            (Some(item), next_source) ->
                (Some(self.transform(item)), MapIterator { source: next_source, transform: self.transform }),
            (None, exhausted) ->
                (None, MapIterator { source: exhausted, transform: self.transform }),
        )
}

impl<I: DoubleEndedIterator, U> DoubleEndedIterator for MapIterator<I, U> {
    @next_back (self) -> (Option<U>, MapIterator<I, U>) =
        match(
            self.source.next_back(),
            (Some(item), next_source) ->
                (Some(self.transform(item)), MapIterator { source: next_source, transform: self.transform }),
            (None, exhausted) ->
                (None, MapIterator { source: exhausted, transform: self.transform }),
        )
}

type FilterIterator<I: Iterator> = { source: I, predicate: (I.Item) -> bool }

impl<I: Iterator> Iterator for FilterIterator<I> {
    type Item = I.Item
    @next (self) -> (Option<I.Item>, FilterIterator<I>) = run(
        let source = self.source,
        loop(
            match(
                source.next(),
                (Some(item), next_source) ->
                    if self.predicate(item) then
                        break (Some(item), FilterIterator { source: next_source, predicate: self.predicate })
                    else run(source = next_source, continue),
                (None, exhausted) ->
                    break (None, FilterIterator { source: exhausted, predicate: self.predicate }),
            ),
        ),
    )
}

impl<I: DoubleEndedIterator> DoubleEndedIterator for FilterIterator<I> {
    @next_back (self) -> (Option<I.Item>, FilterIterator<I>) = run(
        let source = self.source,
        loop(
            match(
                source.next_back(),
                (Some(item), next_source) ->
                    if self.predicate(item) then
                        break (Some(item), FilterIterator { source: next_source, predicate: self.predicate })
                    else run(source = next_source, continue),
                (None, exhausted) ->
                    break (None, FilterIterator { source: exhausted, predicate: self.predicate }),
            ),
        ),
    )
}

type RevIterator<I: DoubleEndedIterator> = { source: I }

impl<I: DoubleEndedIterator> Iterator for RevIterator<I> {
    type Item = I.Item
    @next (self) -> (Option<I.Item>, RevIterator<I>) =
        match(
            self.source.next_back(),
            (Some(item), next_source) -> (Some(item), RevIterator { source: next_source }),
            (None, exhausted) -> (None, RevIterator { source: exhausted }),
        )
}

impl<I: DoubleEndedIterator> DoubleEndedIterator for RevIterator<I> {
    @next_back (self) -> (Option<I.Item>, RevIterator<I>) =
        match(
            self.source.next(),
            (Some(item), next_source) -> (Some(item), RevIterator { source: next_source }),
            (None, exhausted) -> (None, RevIterator { source: exhausted }),
        )
}

type CycleIterator<I: Iterator + Clone> = { original: I, current: I }

impl<I: Iterator + Clone> Iterator for CycleIterator<I> {
    type Item = I.Item
    @next (self) -> (Option<I.Item>, CycleIterator<I>) =
        match(
            self.current.next(),
            (Some(item), next_current) ->
                (Some(item), CycleIterator { original: self.original, current: next_current }),
            (None, _) ->
                // Restart from original
                match(
                    self.original.clone().next(),
                    (Some(item), next_current) ->
                        (Some(item), CycleIterator { original: self.original, current: next_current }),
                    (None, _) ->
                        // Original was empty, stay exhausted
                        (None, self),
                ),
        )
}

// ... additional iterator types for take, skip, enumerate, zip, chain, flatten, etc.
```

### Collect Type Inference

The target type for `.collect()` is inferred from context:

```ori
// Type annotation on binding
let list: [int] = (0..10).iter().map(transform: x -> x * 2).collect()

// Type annotation on expression
let set = (0..10).iter().collect() as Set<int>

// Inferred from usage
@process (items: [str]) -> void = ...
process(items: words.iter().map(transform: w -> w.upper()).collect())
```

---

## Examples

### Custom Iterable Type

```ori
type TreeNode<T> = {
    value: T,
    children: [TreeNode<T>],
}

type TreeIterator<T> = {
    stack: [TreeNode<T>],
}

impl<T> Iterable for TreeNode<T> {
    type Item = T
    @iter (self) -> TreeIterator<T> = TreeIterator { stack: [self] }
}

impl<T> Iterator for TreeIterator<T> {
    type Item = T
    @next (self) -> (Option<T>, TreeIterator<T>) =
        if is_empty(collection: self.stack) then
            (None, self)
        else run(
            let node = self.stack[# - 1],
            let new_stack = self.stack.take(n: # - 1) + node.children,
            (Some(node.value), TreeIterator { stack: new_stack }),
        )
}

// Now TreeNode works with for loops and iterator methods
let tree = TreeNode { value: 1, children: [...] }
for value in tree do print(msg: str(value))
let sum = tree.iter().fold(initial: 0, op: (a, b) -> a + b)
```

### Generic Functions

```ori
@find_first<I: Iterable, T> (items: I, predicate: (T) -> bool) -> Option<T>
    where I.Item == T
= items.iter().find(predicate: predicate)

@to_list<I: Iterable> (items: I) -> [I.Item] =
    items.iter().collect()

@group_by<I: Iterable, K: Eq + Hashable, V> (
    items: I,
    key: (V) -> K,
) -> {K: [V]}
    where I.Item == V
= items.iter().fold(
    initial: {},
    op: (groups, item) -> run(
        let k = key(item),
        let existing = groups[k].unwrap_or(default: []),
        groups.insert(key: k, value: existing + [item]),
    ),
)
```

### Chaining Operations

```ori
@process_logs (logs: [LogEntry]) -> [str] =
    logs.iter()
        .filter(predicate: log -> log.level == Level.Error)
        .map(transform: log -> log.message)
        .take(count: 100)
        .collect()

@word_frequencies (text: str) -> {str: int} =
    text.split(sep: " ")
        .iter()
        .map(transform: word -> word.lower().trim())
        .filter(predicate: word -> !is_empty(collection: word))
        .fold(
            initial: {},
            op: (counts, word) -> run(
                let count = counts[word].unwrap_or(default: 0),
                counts.insert(key: word, value: count + 1),
            ),
        )
```

### Reverse Iteration

```ori
@last_n<I: Iterable> (items: I, n: int) -> [I.Item]
    where I.Item: Iterator, I.Item: DoubleEndedIterator
= items.iter().rev().take(count: n).rev().collect()

@find_last_matching (items: [int], predicate: (int) -> bool) -> Option<int> =
    items.iter().rfind(predicate: predicate)

let reversed = [1, 2, 3, 4, 5].iter().rev().collect()  // [5, 4, 3, 2, 1]
let last = (1..100).iter().last()  // Some(99) — efficient O(1)
```

### Infinite Iterators

```ori
let zeros = repeat(value: 0).take(count: 10).collect()  // [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]

let pattern = [1, 2, 3].iter().cycle().take(count: 10).collect()  // [1, 2, 3, 1, 2, 3, 1, 2, 3, 1]

let alternating = [true, false].iter().cycle().zip(other: 0..10).collect()
// [(true, 0), (false, 1), (true, 2), (false, 3), ...]
```

---

## Design Rationale

### Why `Iterable` Instead of Rust's `IntoIterator`?

Rust's `IntoIterator` has three forms due to the borrow checker:
- `into_iter()` — consumes the collection
- `iter()` — borrows immutably
- `iter_mut()` — borrows mutably

Ori's ARC-based memory model doesn't need this complexity:
- Collections aren't "consumed" — ARC handles ownership
- No mutable borrows — single ownership of mutable data
- One method: `iter()` — simple and sufficient

### Why Functional `next()` Signature?

Ori removed the `mut` keyword. Function parameters are immutable. To advance iterator state, `next()` returns both the optional value and the updated iterator:

```ori
@next (self) -> (Option<Self.Item>, Self)
```

This is purely functional and fits Ori's semantics where callers rebind:

```ori
let (value, iter) = iter.next()
```

### Why Fused Iterators?

Once `next()` returns `None`, all subsequent calls must also return `None`. This:
- Matches user expectations — "empty means empty"
- Simplifies `for` loop reasoning
- Avoids needing a separate `FusedIterator` marker trait
- Most iterators naturally fuse anyway

### Why Separate `Collect` Trait?

The `Collect` trait:
1. Allows different collection types to define how they're built
2. Enables type-directed `.collect()` — the return type determines behavior
3. Keeps `Iterator` focused on producing values, not consuming them

### Why Associated Types?

Associated types (`type Item`) instead of generic parameters:
- Each iterator has exactly one item type
- Cleaner constraints: `where I.Item == int` vs `where I: Iterator<int>`
- Matches Ori's preference for clarity

### Why No Range<float> Iteration?

Float ranges are NOT iterable because:
- What's the "next" float after 0.1? Floating-point precision makes this ambiguous
- Different use cases need different steps
- `Range<float>.contains(value: x)` is still available for bounds checking
- For explicit iteration, use integer ranges with division: `for i in 0..10 do process(value: float(i) / 10.0)`

### Why DoubleEndedIterator?

- `rev()` and `last()` are commonly needed
- Without it, `last()` must iterate the entire collection (O(n))
- Natural extension of the functional model
- Only implemented for ordered collections (lists, ranges, strings)

---

## Performance Considerations

Each `next()` call returns a new iterator struct. These structs are typically small (a source reference plus a few state fields) and stack-allocated in normal use.

For a chain like `items.iter().map(...).filter(...).take(...)`, each iteration creates new wrapper structs. However:

- **Shallow copies**: Structs contain references (to closures, source iterators), not deep copies
- **ARC efficiency**: Reference count updates are cheap
- **Compiler optimization**: The compiler may inline iterator chains and eliminate intermediate allocations
- **No heap allocation**: Iterator structs live on the stack in typical usage

This functional approach is the right semantic fit for Ori. If specific use cases show performance issues, targeted compiler optimizations can address them without changing the language semantics.

---

## Future Work

### Parallel Iteration

A future proposal may add parallel iteration capabilities:

```ori
// Potential future syntax
items.iter()
    .parallel_map(transform: expensive_fn, max_concurrent: 10)
    .collect()
```

This would integrate with Ori's existing `parallel` pattern and capability system. The traits defined in this proposal provide the foundation for such extensions.

### Generator Syntax

A future proposal may add generator functions for easier iterator definition:

```ori
// Potential future syntax
@fibonacci () -> Iterator<int> = generate(
    let a = 0,
    let b = 1,
    loop(
        yield a,
        let next = a + b,
        a = b,
        b = next,
    ),
)
```

### Additional Infinite Iterators

Future additions may include:
- `successors(first: T, next: (T) -> Option<T>)` — generate from a function until `None`
- `from_fn(f: () -> Option<T>)` — generate from a closure

---

## Spec Changes Required

### `06-types.md`

Add Iterator traits section.

### `10-patterns.md`

Document `for` loop desugaring to `Iterable.iter()` and functional `next()`.

### `12-modules.md`

Add to prelude:

**Traits**: `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`

**Functions**: `repeat`

### `CLAUDE.md`

Update prelude traits list and add iterator documentation to the quick reference.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Core traits | `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect` |
| Iterator method | `@next (self) -> (Option<Self.Item>, Self)` (functional) |
| Double-ended method | `@next_back (self) -> (Option<Self.Item>, Self)` |
| Iterable method | `@iter (self) -> impl Iterator` |
| Collect method | `@from_iter (iter: impl Iterator) -> Self` |
| Fused guarantee | Required — once None, always None |
| For loop | Desugars to `.iter()` and functional `.next()` |
| Methods | `map`, `filter`, `fold`, `find`, `collect`, `rev`, `last`, `cycle`, etc. as default trait implementations |
| Infinite iterators | `repeat(value)` function, `Iterator.cycle()` method |
| User types | Implement traits to participate in iteration |
| Prelude | `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`, `repeat` |
| Float ranges | NOT iterable (precision issues) |
| Parallel iteration | Deferred to future proposal |
| Generators | Deferred to future proposal |

This proposal formalizes iteration as a first-class concept in Ori, enabling generic programming over any iterable while maintaining Ori's simplicity, explicitness, and functional semantics.

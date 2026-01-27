# Proposal: Iterator Traits

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Formalize iteration in Ori with three core traits: `Iterator`, `Iterable`, and `Collect`. These traits enable generic code over any iterable, allow user types to participate in `for` loops, and provide the foundation for `.map()`, `.filter()`, and other transformation methods.

```ori
trait Iterator {
    type Item
    @next (mut self) -> Option<Self.Item>
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

    @next (mut self) -> Option<Self.Item>
}
```

- `Item` — the type of values produced
- `next` — returns `Some(value)` or `None` when exhausted
- `mut self` — iterators are stateful, track position

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

### For Loop Desugaring

The `for` loop desugars to use these traits:

```ori
// This:
for x in items do
    process(x: x)

// Desugars to:
run(
    let mut iter = items.iter(),
    loop(
        match(
            iter.next(),
            Some(x) -> process(x: x),
            None -> break,
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

### Iterator Methods

The `Iterator` trait provides default method implementations via extension:

```ori
extend Iterator {
    @map<U> (self, transform: (Self.Item) -> U) -> MapIterator<Self, U> =
        MapIterator { source: self, transform: transform }

    @filter (self, predicate: (Self.Item) -> bool) -> FilterIterator<Self> =
        FilterIterator { source: self, predicate: predicate }

    @fold<U> (self, initial: U, op: (U, Self.Item) -> U) -> U = run(
        let mut acc = initial,
        let mut iter = self,
        loop(
            match(
                iter.next(),
                Some(item) -> { acc = op(acc, item); continue },
                None -> break acc,
            ),
        ),
    )

    @find (self, predicate: (Self.Item) -> bool) -> Option<Self.Item> = run(
        let mut iter = self,
        loop(
            match(
                iter.next(),
                Some(item) -> if predicate(item) then break Some(item) else continue,
                None -> break None,
            ),
        ),
    )

    @collect<C: Collect<Self.Item>> (self) -> C =
        C.from_iter(iter: self)

    @count (self) -> int =
        self.fold(initial: 0, op: (acc, _) -> acc + 1)

    @any (self, predicate: (Self.Item) -> bool) -> bool =
        self.find(predicate: predicate).is_some()

    @all (self, predicate: (Self.Item) -> bool) -> bool =
        !self.any(predicate: item -> !predicate(item))

    @take (self, count: int) -> TakeIterator<Self> =
        TakeIterator { source: self, remaining: count }

    @skip (self, count: int) -> SkipIterator<Self> =
        SkipIterator { source: self, remaining: count }

    @enumerate (self) -> EnumerateIterator<Self> =
        EnumerateIterator { source: self, index: 0 }

    @zip<Other: Iterator> (self, other: Other) -> ZipIterator<Self, Other> =
        ZipIterator { first: self, second: other }

    @chain<Other: Iterator> (self, other: Other) -> ChainIterator<Self, Other>
        where Other.Item == Self.Item
    = ChainIterator { first: Some(self), second: other }

    @flatten (self) -> FlattenIterator<Self>
        where Self.Item: Iterable
    = FlattenIterator { outer: self, inner: None }

    @flat_map<U, I: Iterable> (self, transform: (Self.Item) -> I) -> FlattenIterator<MapIterator<Self, I>>
        where I.Item == U
    = self.map(transform: transform).flatten()
}
```

### Standard Implementations

#### Primitives and Built-ins

```ori
// Lists are iterable
impl<T> Iterable for [T] {
    type Item = T
    @iter (self) -> ListIterator<T> = ListIterator { list: self, index: 0 }
}

impl<T> Collect<T> for [T] {
    @from_iter (iter: impl Iterator where Item == T) -> [T] = /* intrinsic */
}

// Maps are iterable (yields key-value tuples)
impl<K, V> Iterable for {K: V} {
    type Item = (K, V)
    @iter (self) -> MapIterator<K, V> = /* intrinsic */
}

// Sets are iterable
impl<T> Iterable for Set<T> {
    type Item = T
    @iter (self) -> SetIterator<T> = /* intrinsic */
}

impl<T> Collect<T> for Set<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Set<T> = /* intrinsic */
}

// Strings are iterable (yields characters)
impl Iterable for str {
    type Item = char
    @iter (self) -> CharIterator = /* intrinsic */
}

// Ranges are iterable
impl Iterable for Range<int> {
    type Item = int
    @iter (self) -> RangeIterator<int> = RangeIterator { current: self.start, end: self.end }
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
type ListIterator<T> = { list: [T], index: int }

impl<T> Iterator for ListIterator<T> {
    type Item = T
    @next (mut self) -> Option<T> = run(
        if self.index < self.list.len() then run(
            let item = self.list[self.index],
            self.index = self.index + 1,
            Some(item),
        ) else None,
    )
}

type RangeIterator<T> = { current: T, end: T }

impl Iterator for RangeIterator<int> {
    type Item = int
    @next (mut self) -> Option<int> = run(
        if self.current < self.end then run(
            let item = self.current,
            self.current = self.current + 1,
            Some(item),
        ) else None,
    )
}

type MapIterator<I: Iterator, U> = { source: I, transform: (I.Item) -> U }

impl<I: Iterator, U> Iterator for MapIterator<I, U> {
    type Item = U
    @next (mut self) -> Option<U> =
        self.source.next().map(transform: self.transform)
}

type FilterIterator<I: Iterator> = { source: I, predicate: (I.Item) -> bool }

impl<I: Iterator> Iterator for FilterIterator<I> {
    type Item = I.Item
    @next (mut self) -> Option<I.Item> = loop(
        match(
            self.source.next(),
            Some(item) -> if self.predicate(item) then break Some(item) else continue,
            None -> break None,
        ),
    )
}

// ... additional iterator types for take, skip, enumerate, zip, etc.
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
    @next (mut self) -> Option<T> = run(
        if self.stack.is_empty() then
            None
        else run(
            let node = self.stack.pop(),
            self.stack = self.stack + node.children,
            Some(node.value),
        ),
    )
}

// Now TreeNode works with for loops and iterator methods
let tree = TreeNode { value: 1, children: [...] }
for value in tree do print(msg: value as str)
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
        .filter(predicate: word -> !word.is_empty())
        .fold(
            initial: {},
            op: (counts, word) -> run(
                let count = counts[word].unwrap_or(default: 0),
                counts.insert(key: word, value: count + 1),
            ),
        )
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

---

## Spec Changes Required

### `06-types.md`

Add Iterator traits section:

```markdown
### Iterator Traits

```ori
trait Iterator {
    type Item
    @next (mut self) -> Option<Self.Item>
}

trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}

trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```
```

### `10-patterns.md`

Document `for` loop desugaring to `Iterable.iter()`.

### `12-modules.md`

Add `Iterator`, `Iterable`, `Collect` to prelude traits.

### `/CLAUDE.md`

Update prelude traits list and add iterator method documentation.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Core traits | `Iterator`, `Iterable`, `Collect` |
| Iterator method | `@next (mut self) -> Option<Self.Item>` |
| Iterable method | `@iter (self) -> impl Iterator` |
| Collect method | `@from_iter (iter: impl Iterator) -> Self` |
| For loop | Desugars to `.iter()` and `.next()` |
| Methods | `map`, `filter`, `fold`, `find`, `collect`, etc. via extension |
| User types | Implement traits to participate in iteration |

This proposal formalizes iteration as a first-class concept in Ori, enabling generic programming over any iterable while maintaining Ori's simplicity and explicitness.

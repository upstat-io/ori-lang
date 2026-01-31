# Proposal: Iterator Performance and Semantics

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Standard library, compiler optimizations, runtime, syntax (infinite ranges)

---

## Summary

This proposal formalizes the performance characteristics and precise semantics of Ori's functional iterator model, addressing the unusual `(Option<Item>, Self)` return type, state copying behavior, and composition guarantees. It also introduces infinite range syntax (`start..`) for creating unbounded integer sequences.

---

## Problem Statement

The approved Iterator Traits proposal defines:

```ori
@next (self) -> (Option<Self.Item>, Self)
```

This raises questions:

1. **State copying**: Is iterator state copied on every `next()` call?
2. **Performance**: What are the costs of this functional model?
3. **Composition**: How do chained iterators (`.map().filter()`) perform?
4. **Guarantees**: What can developers rely on for performance?

---

## Iterator State Model

### Functional Semantics

The `next()` method returns both the value and the "updated" iterator:

```ori
let (value, iter) = iter.next()
```

This is **functional** — the original `iter` is conceptually unchanged, and a new iterator state is returned.

### Implementation Reality

**Key insight**: The compiler optimizes this pattern. In practice:

1. **Single binding case**: When the result is immediately destructured and the old iterator discarded, no copy occurs:
   ```ori
   let (value, iter) = iter.next()  // iter is rebound, old value unused
   // Compiler: mutation in place, no copy
   ```

2. **Escape case**: If the old iterator escapes (used after next), copy occurs:
   ```ori
   let old_iter = iter
   let (value, iter) = iter.next()
   use(old_iter)  // old_iter still valid
   // Compiler: must copy to preserve old_iter
   ```

### Copy Elision Guarantee

The compiler guarantees that iterator chains in idiomatic usage patterns do NOT perform unnecessary copies:

```ori
// No copies in this chain:
items.iter()
    .map(transform: x -> x * 2)
    .filter(predicate: x -> x > 0)
    .take(count: 10)
    .collect()
```

---

## Iterator Size Guarantees

Iterators are designed for stack allocation with minimal overhead:

| Iterator Type | Size Guarantee |
|---------------|----------------|
| Base iterators (ListIterator, RangeIterator) | O(1) fixed size |
| Adaptor iterators (MapIterator, FilterIterator) | Source size + O(1) |
| Chained adaptors | O(depth) where depth = number of adaptors |

All iterator types are fixed-size at compile time. No iterator stores unbounded data.

**Example chain:**

```ori
items.iter()          // O(1)
    .map(...)         // O(1) additional
    .filter(...)      // O(1) additional
    .take(...)        // O(1) additional
// Total: O(4) = O(1) — four fixed-size structs
```

> **Note:** On typical 64-bit platforms, base iterators are approximately 24 bytes, and adaptor iterators add approximately 8-16 bytes each.

---

## Composition Semantics

### Lazy Evaluation

All iterator adaptors are lazy — they don't compute until `next()` is called:

```ori
let iter = huge_list.iter()
    .map(transform: expensive_computation)  // No computation yet
    .filter(predicate: another_computation)  // Still nothing

iter.next()  // NOW computation happens, for first element only
```

### Pull-Based Iteration

Elements are "pulled" through the chain one at a time:

```ori
// Conceptual flow for items.iter().map(f).filter(p).next():
// 1. filter.next() calls map.next()
// 2. map.next() calls items_iter.next()
// 3. items_iter returns (Some(item), items_iter')
// 4. map applies f(item), returns (Some(f(item)), map_iter')
// 5. filter checks p(f(item))
//    - if true: returns (Some(f(item)), filter_iter')
//    - if false: calls map.next() again (step 2)
```

### Short-Circuit Guarantee

Methods like `find`, `any`, `all` short-circuit:

```ori
huge_list.iter().any(predicate: x -> x > 100)
// Stops as soon as a match is found
// Does NOT iterate the entire list
```

---

## Fused Iterator Guarantee

### Specification

Once `next()` returns `(None, _)`, all subsequent calls return `(None, _)`:

```ori
let (None, iter) = iter.next()
let (None, _) = iter.next()  // Guaranteed None
let (None, _) = iter.next()  // Still None
```

### Implementation Requirement

Iterators must track exhaustion state. Simplest implementation:

```ori
type ListIterator<T> = { list: [T], front: int, back: int }

impl<T> Iterator for ListIterator<T> {
    @next (self) -> (Option<T>, ListIterator<T>) =
        if self.front >= self.back then
            (None, self)  // Returns same state, None forever
        else
            (Some(self.list[self.front]),
             ListIterator { ...self, front: self.front + 1 })
}
```

---

## Performance Guarantees

### Algorithmic Complexity

| Operation | Complexity |
|-----------|------------|
| `next()` | O(1) amortized for most iterators |
| `map(f).next()` | O(1) + cost of f |
| `filter(p).next()` | O(k) where k = elements until predicate matches |
| `take(n).next()` | O(1) |
| `skip(n).next()` (first call) | O(n) |
| `skip(n).next()` (subsequent) | O(1) |
| `collect()` | O(n) where n = iterator length |

### Memory Complexity

| Operation | Memory |
|-----------|--------|
| Iterator chain | O(depth) stack space |
| `collect()` | O(n) for result collection |
| `fold()` | O(1) beyond accumulator |

### No Hidden Allocation

Iterator operations do NOT allocate heap memory (except `collect()` which must build the result):

```ori
// No heap allocation:
items.iter()
    .map(transform: x -> x * 2)
    .filter(predicate: x -> x > 0)
    .fold(initial: 0, op: (a, b) -> a + b)
```

---

## DoubleEndedIterator Semantics

### Bidirectional Access

`DoubleEndedIterator` supports both `next()` and `next_back()`:

```ori
let iter = [1, 2, 3, 4, 5].iter()
let (Some(1), iter) = iter.next()
let (Some(5), iter) = iter.next_back()
let (Some(2), iter) = iter.next()
let (Some(4), iter) = iter.next_back()
let (Some(3), iter) = iter.next()
let (None, iter) = iter.next()       // Exhausted
let (None, iter) = iter.next_back()  // Also exhausted
```

### Meeting in the Middle

Front and back indices track separately until they meet:

```ori
type ListIterator<T> = { list: [T], front: int, back: int }
// Exhausted when front >= back
```

### rev() Efficiency

`rev()` is O(1) — it wraps the iterator and swaps next/next_back:

```ori
let reversed = [1, 2, 3].iter().rev()
// No copying, no reversal of underlying data
// Just calls next_back() instead of next()
```

---

## Compiler Optimizations

### Guaranteed Optimizations

The compiler MUST perform these optimizations:

| Optimization | Description |
|--------------|-------------|
| Copy elision | No copy when iterator rebound immediately |
| Inline expansion | Iterator methods inlined for small chains |
| Deforestation | Intermediate iterators eliminated |
| Loop fusion | Adjacent maps/filters combined |

### Example: Optimized Code

```ori
// Source:
let sum = (0..1000).iter()
    .map(transform: x -> x * 2)
    .filter(predicate: x -> x % 4 == 0)
    .fold(initial: 0, op: (a, b) -> a + b)

// Conceptually compiles to:
let sum = 0
for x in 0..1000 do
    let mapped = x * 2
    if mapped % 4 == 0 then
        sum = sum + mapped
```

### Not Guaranteed

- Parallelization of sequential iteration
- Vectorization (SIMD) for numeric operations
- Custom optimizations for specific patterns

These may be added in future compiler versions but are not part of the language specification.

---

## Infinite Ranges

### Syntax

The `start..` syntax creates an unbounded ascending integer range:

```ori
0..       // 0, 1, 2, 3, ... (infinite ascending)
100..     // 100, 101, 102, ... (infinite ascending from 100)
```

Infinite ranges always ascend with step +1 by default. For descending infinite sequences, use an explicit step:

```ori
0.. by -1     // 0, -1, -2, -3, ... (infinite descending)
100.. by -2   // 100, 98, 96, ... (infinite descending by 2)
```

### Constraints

- Infinite ranges are supported only for `int`
- The step must be explicitly positive for ascending or negative for descending
- `start.. by 0` panics (zero step is invalid for any range)

### Type

An infinite range has type `Range<int>` and implements `Iterable` but NOT `DoubleEndedIterator` (no end to iterate from).

---

## Infinite Iterator Handling

### Creating Infinite Iterators

```ori
let zeros = repeat(value: 0)          // Infinite zeros
let naturals = (0..).iter()           // 0, 1, 2, 3, ...
let evens = (0.. by 2).iter()         // 0, 2, 4, 6, ...
let cycling = [1, 2, 3].iter().cycle() // 1, 2, 3, 1, 2, 3, ...
```

### Safe Consumption

Infinite iterators must be bounded before consumption:

```ori
// OK: bounded by take
repeat(value: 0).take(count: 100).collect()

// OK: short-circuits
repeat(value: 0).find(predicate: x -> x > 0)  // Never finds, runs forever
// But:
repeat(value: 1).find(predicate: x -> x > 0)  // Returns Some(1) immediately

// DANGEROUS: infinite loop
repeat(value: 0).collect()  // Never terminates, eventually OOM
```

### Infinite Iteration Detection

The compiler does not automatically prevent infinite iteration. However, implementations SHOULD warn on obvious infinite patterns:

**Recommended warnings:**
- `repeat(...).collect()` without `take`
- `repeat(...).fold(...)` without bounds
- `iter.cycle().collect()` without `take`
- Unbounded range (e.g., `(0..).collect()`) without `take`

These warnings are advisory. Developers may intentionally use infinite iteration with short-circuiting operations like `find` or `any`.

```ori
// Warning: unbounded collect on infinite iterator
repeat(value: 0).collect()

// OK: bounded by take
repeat(value: 0).take(count: 100).collect()

// OK: short-circuits (intentional infinite iteration)
repeat(value: 0).find(predicate: x -> x > 0)
```

---

## Examples

### Efficient File Processing

```ori
@count_errors (lines: [str]) -> int =
    lines.iter()
        .filter(predicate: line -> line.starts_with(prefix: "ERROR"))
        .count()
// O(n) time, O(1) space (not counting input)
```

### Parallel-Ready Iteration

```ori
@process_batch (items: [Item]) -> [Result] uses Suspend =
    items.iter()
        .map(transform: item -> () -> process(item))  // Create tasks
        .collect()
        |> tasks -> parallel(tasks: tasks)             // Execute in parallel
```

### Avoiding Intermediate Collections

```ori
// GOOD: no intermediate list
let sum = numbers.iter()
    .filter(predicate: is_positive)
    .map(transform: square)
    .fold(initial: 0, op: add)

// BAD: creates intermediate lists
let positives = numbers.iter().filter(predicate: is_positive).collect()
let squared = positives.iter().map(transform: square).collect()
let sum = squared.iter().fold(initial: 0, op: add)
```

---

## Spec Changes Required

### `06-types.md`

Add infinite range type:
- `Range<int>` unbounded variant for `start..` syntax
- Document that unbounded ranges are only iterable, not finite

### `09-expressions.md`

Add infinite range literal syntax:
- `start..` creates unbounded ascending range with step 1
- `start.. by step` creates unbounded range with explicit step (step must be non-zero)

### `12-modules.md` (Prelude)

No changes needed (Iterator traits already in prelude from Iterator Traits proposal).

### `grammar.ebnf`

Update range expression to allow omitted end:
```ebnf
range_expr = expr ".." [ expr ] [ "by" expr ] .
```

### Performance Appendix (new section or in existing spec)

1. Copy elision guarantee
2. Lazy evaluation guarantee
3. Fused iterator requirement (already in Iterator Traits, cross-reference)
4. Performance complexity table
5. Optimization guarantees
6. Infinite iterator lint recommendations

---

## Summary

| Aspect | Guarantee |
|--------|-----------|
| State copying | Elided when immediately rebound |
| Lazy evaluation | Yes, always |
| Short-circuit | Yes, for find/any/all/take |
| Fused | Yes, once None always None |
| Memory | O(chain depth) stack, no heap |
| Complexity | O(1) per next() for most adaptors |
| Optimizations | Copy elision, inlining, fusion guaranteed |
| Infinite ranges | `start..` syntax for unbounded ascending int ranges |
| Infinite iterators | Developer responsibility to bound; lint warnings recommended |
| Double-ended | O(1) for rev(), efficient last() |

# Proposal: Iterator Performance and Semantics

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Standard library, compiler optimizations, runtime

---

## Summary

This proposal formalizes the performance characteristics and precise semantics of Ori's functional iterator model, addressing the unusual `(Option<Item>, Self)` return type, state copying behavior, and composition guarantees.

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

## Iterator Structure Sizes

### Typical Iterator Sizes

| Iterator Type | Fields | Approximate Size |
|---------------|--------|------------------|
| `ListIterator<T>` | list ref + front index + back index | 24 bytes |
| `RangeIterator<int>` | current + end + step | 24 bytes |
| `MapIterator<I, F>` | source iter + transform fn | source + 16 bytes |
| `FilterIterator<I, F>` | source iter + predicate fn | source + 16 bytes |
| `TakeIterator<I>` | source iter + remaining count | source + 8 bytes |

### Chain Size

Iterator chains grow linearly with the number of adaptors:

```ori
items.iter()          // ~24 bytes
    .map(...)         // +16 bytes
    .filter(...)      // +16 bytes
    .take(...)        // +8 bytes
// Total: ~64 bytes
```

All stack-allocated in normal usage.

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

The compiler performs these optimizations:

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

## Infinite Iterator Handling

### Creating Infinite Iterators

```ori
let zeros = repeat(value: 0)          // Infinite zeros
let naturals = (0..).iter()            // 0, 1, 2, 3, ... (if supported)
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

### No Automatic Detection

The compiler does NOT detect infinite iteration — this is the developer's responsibility:

```ori
// Compiles but doesn't terminate:
repeat(value: 0).fold(initial: 0, op: (a, b) -> a + b)
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
@process_batch (items: [Item]) -> [Result] uses Async =
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

### Update Iterator Traits Section

Add:
1. Copy elision guarantee
2. Lazy evaluation guarantee
3. Fused iterator requirement
4. Performance complexity table

### Add Performance Appendix

Document:
1. Iterator structure sizes
2. Optimization guarantees
3. Infinite iterator warnings

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
| Infinite iterators | Developer responsibility to bound |
| Double-ended | O(1) for rev(), efficient last() |

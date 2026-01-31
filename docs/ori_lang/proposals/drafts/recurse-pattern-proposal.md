# Proposal: Recurse Pattern

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, patterns, optimization

---

## Summary

This proposal formalizes the `recurse` pattern semantics, including memoization, parallel options, termination guarantees, and the `self` keyword.

---

## Problem Statement

The spec shows `recurse(condition:, base:, step:, memo:)` but leaves unclear:

1. **Self semantics**: How does `self(...)` work?
2. **Memoization**: How does `memo: true` behave?
3. **Parallel recursion**: When can recursion parallelize?
4. **Termination**: What termination guarantees exist?
5. **Stack limits**: How are deep recursions handled?

---

## Syntax

```ori
recurse(
    condition: bool_expr,
    base: expr,
    step: expr_with_self,
    memo: bool = false,
    parallel: bool = false,
)
```

---

## Basic Semantics

### Evaluation

1. Evaluate `condition`
2. If true: return `base` expression
3. If false: evaluate `step` expression (may contain `self(...)` calls)

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)
```

### Self Keyword

`self(...)` within `step` represents a recursive call:

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),  // Two recursive calls
)
```

### Argument Binding

`self(...)` arguments must match the enclosing function's parameters:

```ori
@gcd (a: int, b: int) -> int = recurse(
    condition: b == 0,
    base: a,
    step: self(b, a % b),  // Same arity as gcd
)
```

---

## Memoization

### memo: true

Caches results for the duration of the top-level call:

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,  // O(n) instead of O(2^n)
)
```

### Memo Scope

Memoization cache is:
- Created at top-level `recurse` entry
- Shared across all recursive calls
- Discarded when top-level returns

```ori
fibonacci(n: 10)  // Cache created, populated, discarded
fibonacci(n: 10)  // Fresh cache created (no persistence)
```

### Key Generation

Memo keys are the `self(...)` arguments:

```ori
@f (a: int, b: str) -> int = recurse(
    condition: ...,
    base: ...,
    step: self(a - 1, b),  // Key: (a - 1, b)
    memo: true,
)
// Arguments must be Hashable + Eq
```

### Memo Requirements

With `memo: true`:
- All parameters must be `Hashable + Eq`
- Return type must be `Clone`

---

## Parallel Recursion

### parallel: true

Enables parallel evaluation of independent recursive calls:

```ori
@parallel_fib (n: int) -> int uses Async = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),  // Evaluated in parallel
    parallel: true,
)
```

### Parallel Requirements

- Requires `uses Async` capability
- Captured values must be `Sendable`
- Multiple `self(...)` calls execute concurrently

### Parallel + Memo

Both can be combined:

```ori
@fast_fib (n: int) -> int uses Async = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
    parallel: true,
)
// Memo prevents redundant work; parallel speeds independent calls
```

### Parallel Overhead

Parallel recursion has overhead. For small operations, sequential may be faster:

```ori
// Sequential may be faster for simple arithmetic
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
    parallel: false,  // Sequential is fine here
)
```

---

## Tail Call Optimization

### When Applicable

`recurse` enables tail call optimization when `self(...)` is in tail position:

```ori
@sum_to (n: int, acc: int = 0) -> int = recurse(
    condition: n == 0,
    base: acc,
    step: self(n - 1, acc + n),  // Tail position
)
// Compiled to loop, O(1) stack space
```

### Non-Tail Recursion

When `self(...)` is not in tail position, stack space is O(depth):

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),  // NOT tail: multiplication after self
)
// Stack space O(n)
```

---

## Stack Limits

### Default Limit

The runtime enforces a recursion depth limit (default: 1000):

```ori
@deep (n: int) -> int = recurse(
    condition: n == 0,
    base: 0,
    step: 1 + self(n - 1),
)

deep(n: 10000)  // panic: recursion limit exceeded
```

### With Tail Optimization

Tail-recursive patterns don't hit stack limits:

```ori
@deep_tail (n: int, acc: int = 0) -> int = recurse(
    condition: n == 0,
    base: acc,
    step: self(n - 1, acc + 1),
)

deep_tail(n: 10000)  // OK: compiled to loop
```

---

## Termination

### No Static Guarantee

The compiler does NOT verify termination:

```ori
@infinite (n: int) -> int = recurse(
    condition: false,  // Never terminates
    base: 0,
    step: self(n),
)
// Compiles, but loops forever at runtime
```

### Runtime Limit

The recursion limit catches unintentional infinite recursion:

```ori
@bad (n: int) -> int = recurse(
    condition: n == 0,
    base: 0,
    step: self(n + 1),  // Wrong direction!
)

bad(n: 5)  // panic: recursion limit exceeded
```

---

## Common Patterns

### Tree Traversal

```ori
type Tree<T> = Leaf(T) | Node(left: Tree<T>, right: Tree<T>)

@sum_tree (tree: Tree<int>) -> int = match(tree,
    Leaf(v) -> v,
    Node(l, r) -> recurse(
        condition: false,  // Always step
        base: 0,           // Unused
        step: sum_tree(tree: l) + sum_tree(tree: r),
    ),
)
```

### Divide and Conquer

```ori
@merge_sort<T: Comparable> (items: [T]) -> [T] = recurse(
    condition: len(collection: items) <= 1,
    base: items,
    step: run(
        let mid = len(collection: items) / 2,
        let left = self(items.take(count: mid)),
        let right = self(items.skip(count: mid)),
        merge(left, right),
    ),
    parallel: true,
)
```

### Dynamic Programming

```ori
@longest_common_subsequence (a: str, b: str) -> int = run(
    @lcs (i: int, j: int) -> int = recurse(
        condition: i == 0 || j == 0,
        base: 0,
        step: if a[i - 1] == b[j - 1] then
            1 + self(i - 1, j - 1)
        else
            max(left: self(i - 1, j), right: self(i, j - 1)),
        memo: true,
    ),
    lcs(i: len(collection: a), j: len(collection: b)),
)
```

---

## Error Messages

### Non-Hashable with Memo

```
error[E1000]: `memo: true` requires `Hashable` parameters
  --> src/main.ori:5:5
   |
 5 |     @f (data: [int]) -> int = recurse(
   |                ^^^^^ `[int]` is `Hashable` but check other params
   |
   = note: all parameters must implement `Hashable + Eq` for memoization
```

### Self Outside Step

```
error[E1001]: `self` can only appear in `step` expression
  --> src/main.ori:5:10
   |
 5 |     condition: self(n) == 0,
   |                ^^^^ invalid use of `self`
   |
   = help: `self` represents recursive calls and can only appear in `step`
```

### Wrong Self Arity

```
error[E1002]: `self` call has wrong number of arguments
  --> src/main.ori:7:10
   |
 3 | @f (a: int, b: int) -> int = recurse(
   |     -------- expects 2 arguments
 7 |     step: self(a),
   |           ^^^^^^^ provided 1 argument
```

---

## Spec Changes Required

### Update `10-patterns.md`

Expand recurse section with:
1. Self keyword semantics
2. Memoization behavior
3. Parallel execution rules
4. Tail call optimization
5. Stack limit behavior

---

## Summary

| Aspect | Details |
|--------|---------|
| Syntax | `recurse(condition:, base:, step:, memo:, parallel:)` |
| Self | `self(...)` represents recursive call within `step` |
| Memo | Caches results for current call tree |
| Memo requires | Parameters: `Hashable + Eq`, Return: `Clone` |
| Parallel | Execute multiple `self()` calls concurrently |
| Parallel requires | `uses Async`, `Sendable` captures |
| Tail optimization | When `self()` is in tail position |
| Stack limit | Default 1000, bypassed with tail optimization |

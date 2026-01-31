# Proposal: For-Yield Comprehensions

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, expressions, type inference

---

## Summary

This proposal formalizes `for...yield` comprehension semantics, including type inference, filtering, nesting, and interaction with break/continue.

---

## Problem Statement

The spec shows `for...yield` syntax but leaves unclear:

1. **Type inference**: How is the result type determined?
2. **Filtering**: How does `if` filtering work?
3. **Nesting**: How do nested comprehensions behave?
4. **Break/Continue**: What do these mean in yield context?
5. **Empty results**: What happens when nothing is yielded?

---

## Syntax

### Basic Form

```ori
for element in iterable yield expression
```

### With Filter

```ori
for element in iterable if condition yield expression
```

### With Binding Pattern

```ori
for (key, value) in map yield expression
for { x, y } in points yield expression
```

---

## Semantics

### Desugaring

`for...yield` desugars to iterator methods:

```ori
// This:
for x in items yield x * 2

// Desugars to:
items.iter().map(transform: x -> x * 2).collect()
```

With filter:

```ori
// This:
for x in items if x > 0 yield x * 2

// Desugars to:
items.iter().filter(predicate: x -> x > 0).map(transform: x -> x * 2).collect()
```

### Type Inference

The result type is inferred from context:

```ori
let numbers: [int] = for x in items yield x.id  // -> [int]
let set: Set<str> = for x in items yield x.name  // -> Set<str>
```

Without context, defaults to list:

```ori
let result = for x in 0..5 yield x * 2  // [int] inferred
```

### Collect Target

Any type implementing `Collect<T>` can be the target:

```ori
// Collect into list
let list: [int] = for x in items yield x

// Collect into set
let set: Set<int> = for x in items yield x

// Collect into map (yielding tuples)
let map: {str: int} = for x in items yield (x.name, x.value)
```

---

## Filtering

### Single Condition

```ori
for x in numbers if x > 0 yield x
```

### Multiple Conditions

Chain conditions with `&&`:

```ori
for x in numbers if x > 0 && x < 100 yield x
```

Or use multiple `if` clauses (equivalent):

```ori
for x in numbers if x > 0 if x < 100 yield x
```

### Filter Position

Filter comes after the binding, before `yield`:

```ori
for x in items if predicate(x) yield transform(x)
//              ^^^^^^^^^^^^^^^^ filter
//                               ^^^^^^^^^^^^^^^^ yield expression
```

---

## Nested Comprehensions

### Flat Nesting

Nested `for` clauses produce a flat result:

```ori
for x in xs for y in ys yield (x, y)
// Equivalent to:
// [(x, y) for each x in xs, for each y in ys]
```

### With Filters

```ori
for x in xs if x > 0 for y in ys if y > 0 yield x * y
```

### Desugaring

```ori
// This:
for x in xs for y in ys yield (x, y)

// Desugars to:
xs.iter().flat_map(transform: x -> ys.iter().map(transform: y -> (x, y))).collect()
```

---

## Break and Continue

### Continue Without Value

Skips the current element:

```ori
for x in items yield
    if skip(x) then continue,  // Don't add anything
    transform(x),
```

Equivalent to filtering:

```ori
for x in items if !skip(x) yield transform(x)
```

### Continue With Value

Uses the value instead of the yield expression:

```ori
for x in items yield
    if special(x) then continue x * 10,  // Use this value
    transform(x),  // Otherwise use this
```

### Break Without Value

Stops iteration, collects results so far:

```ori
for x in items yield
    if done(x) then break,  // Stop here
    transform(x),
```

### Break With Value

Stops iteration and adds a final value:

```ori
for x in items yield
    if done(x) then break x,  // Add x and stop
    transform(x),
```

---

## Empty Results

### No Elements

If the source is empty, the result is empty:

```ori
for x in [] yield x * 2  // []
```

### All Filtered

If all elements are filtered out:

```ori
for x in [1, 2, 3] if x > 10 yield x  // []
```

### Break Immediately

If break occurs before any yield:

```ori
for x in items yield
    break,
    x,
// []
```

---

## Type Constraints

### Iterable Source

The source must implement `Iterable`:

```ori
for x in items yield x  // items must be Iterable
```

### Collect Target

The result must be `Collect<T>` where `T` is the yield type:

```ori
let list: [int] = for x in items yield x.count  // OK: [int]: Collect<int>
let bad: int = for x in items yield x.count  // ERROR: int is not Collect<int>
```

---

## Interaction with Patterns

### In Run

```ori
run(
    let data = prepare(),
    let results = for x in data yield process(x),
    summarize(results),
)
```

### In Match Arms

```ori
match(source,
    Some(items) -> for x in items yield x * 2,
    None -> [],
)
```

---

## Performance

### Lazy Evaluation

The desugared iterator chain is lazy:

```ori
for x in items if expensive_filter(x) yield transform(x)
// Only calls expensive_filter and transform as needed
```

### Short-Circuit on Break

Break stops iteration immediately:

```ori
for x in large_list yield
    if found(x) then break x,
    x,
// Stops at first found, doesn't traverse entire list
```

---

## Error Messages

### Non-Iterable Source

```
error[E0890]: `for` requires `Iterable` source
  --> src/main.ori:5:10
   |
 5 | for x in 42 yield x
   |          ^^ `int` does not implement `Iterable`
   |
   = help: use a range: `0..42`
```

### Non-Collectible Target

```
error[E0891]: cannot collect into `int`
  --> src/main.ori:5:1
   |
 5 | let n: int = for x in items yield x
   |              ^^^^^^^^^^^^^^^^^^^^^^ produces collection, not `int`
   |
   = note: `for...yield` produces a collection type
   = help: use `.fold()` or `.count()` for a single value
```

### Type Mismatch in Yield

```
error[E0892]: mismatched types in `yield`
  --> src/main.ori:5:30
   |
 5 | let list: [int] = for x in items yield x.name
   |                                        ^^^^^^ expected `int`, found `str`
   |
   = note: expected element type `int` for `[int]`
```

---

## Examples

### Basic Transformation

```ori
let squares = for x in 0..10 yield x * x
// [0, 1, 4, 9, 16, 25, 36, 49, 64, 81]
```

### Filtering

```ori
let evens = for x in 0..10 if x % 2 == 0 yield x
// [0, 2, 4, 6, 8]
```

### Nested

```ori
let pairs = for x in 0..3 for y in 0..3 yield (x, y)
// [(0,0), (0,1), (0,2), (1,0), (1,1), (1,2), (2,0), (2,1), (2,2)]
```

### With Complex Logic

```ori
let processed = for item in items yield
    if item.skip then continue,
    if item.stop then break,
    match(item.transform(),
        Ok(v) -> v,
        Err(_) -> continue,
    ),
```

### Into Set

```ori
let unique_names: Set<str> = for user in users yield user.name
```

### Into Map

```ori
let by_id: {int: User} = for user in users yield (user.id, user)
```

---

## Spec Changes Required

### Update `10-patterns.md`

Add For-Yield section covering:
1. Desugaring semantics
2. Type inference rules
3. Filter syntax
4. Nesting behavior
5. Break/continue interaction

### Update `09-expressions.md`

Cross-reference to for-yield comprehensions.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Basic syntax | `for x in items yield expr` |
| Filter syntax | `for x in items if cond yield expr` |
| Desugars to | `.iter().map().collect()` or `.filter().map().collect()` |
| Result type | Inferred from context or defaults to `[T]` |
| Empty source | Empty result |
| All filtered | Empty result |
| Continue | Skip element (no value) or substitute value |
| Break | Stop iteration, optionally add final value |
| Nesting | Flat result via `flat_map` |

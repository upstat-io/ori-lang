# Proposal: Range with Step

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-25
**Approved:** 2026-01-28

---

## Summary

Extend range syntax to support a step value for non-unit increments.

```ori
0..10 by 2      // 0, 2, 4, 6, 8
10..0 by -1     // 10, 9, 8, ..., 1
0..100 by 10    // 0, 10, 20, ..., 90
```

---

## Motivation

### The Problem

Currently, Ori supports basic ranges:

```ori
0..10       // 0, 1, 2, ..., 9 (exclusive)
0..=10      // 0, 1, 2, ..., 10 (inclusive)
```

Common use cases require non-unit steps:

```ori
// Every other element
for i in [0, 2, 4, 6, 8] do ...  // Manual list

// Countdown
for i in [10, 9, 8, 7, 6, 5, 4, 3, 2, 1] do ...  // Manual list

// Pagination offsets
for offset in [0, 20, 40, 60, 80] do ...  // Manual list
```

Without step support, users must either:
1. Build lists manually
2. Use filter with modulo (inefficient)
3. Write recursive helper functions

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Python | `range(0, 10, 2)` | Function with step arg |
| Ruby | `(0..10).step(2)` | Method on range |
| Kotlin | `0..10 step 2` | Infix keyword |
| Rust | `(0..10).step_by(2)` | Method on iterator |
| Haskell | `[0, 2..10]` | List syntax with step |
| Go | `for i := 0; i < 10; i += 2` | C-style loop |

### The Ori Way

Kotlin's `step` keyword is clean but conflicts with potential variable names. Using `by` is:
- Readable: "0 to 10 by 2"
- Unlikely to conflict (not a common variable name)
- Consistent with natural language

---

## Design

### Syntax

The `by` keyword specifies the step value following a range expression:

```
range_expr = shift_expr [ ( ".." | "..=" ) shift_expr [ "by" shift_expr ] ] .
```

### Basic Usage

```ori
// Positive step (ascending)
0..10 by 2      // 0, 2, 4, 6, 8
1..10 by 3      // 1, 4, 7

// Inclusive end
0..=10 by 2     // 0, 2, 4, 6, 8, 10

// Negative step (descending)
10..0 by -1     // 10, 9, 8, 7, 6, 5, 4, 3, 2, 1
10..=0 by -2    // 10, 8, 6, 4, 2, 0

// Variable step
let step = 5
0..100 by step  // 0, 5, 10, 15, ..., 95
```

### In For Loops

```ori
// Every other index
for i in 0..len(collection: items) by 2 do
    process(items[i])

// Countdown
for i in 10..=1 by -1 do
    print(msg: `{i}...`)
print(msg: "Liftoff!")

// Pagination
for offset in 0..total by page_size do
    let page = fetch_page(offset: offset, limit: page_size)
    process(page)
```

### With Collect

```ori
// Generate list of even numbers
let evens = for i in 0..20 by 2 yield i
// [0, 2, 4, 6, 8, 10, 12, 14, 16, 18]

// Countdown list
let countdown = for i in 5..=1 by -1 yield i
// [5, 4, 3, 2, 1]
```

### Edge Cases

**Step of zero:**
```ori
0..10 by 0  // panic: step cannot be zero
```

**Mismatched direction:**
```ori
0..10 by -1   // Empty range (can't go from 0 to 10 with negative step)
10..0 by 1    // Empty range (can't go from 10 to 0 with positive step)
```

### Type Constraints

- Range with step is supported only for `int` ranges
- Start, end, and step must all be `int`
- Step must be non-zero (runtime panic if zero)
- It is a compile-time error to use `by` with non-integer ranges

### Keywords

`by` is added as a context-sensitive keyword. It is only recognized following a range expression.

Variable names `by` remain valid:
```ori
let by = 2
let range = 0..10 by by  // Valid: second `by` is the keyword, third is the variable
```

---

## Examples

### Matrix Diagonal

```ori
@diagonal<T> (matrix: [[T]]) -> [T] = run(
    let size = len(collection: matrix),
    for i in 0..size yield matrix[i][i],
)
```

### Sampling Every Nth Element

```ori
@sample<T> (items: [T], every: int) -> [T] =
    for i in 0..len(collection: items) by every yield items[i]

let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
sample(items: data, every: 3)  // [1, 4, 7, 10]
```

### Batch Processing

```ori
@process_in_batches<T> (items: [T], batch_size: int) -> void =
    for start in 0..len(collection: items) by batch_size do run(
        let end = min(left: start + batch_size, right: len(collection: items)),
        let batch = items[start..end],
        process_batch(batch),
    )
```

### Animation Frames

```ori
@animate (from: float, to: float, frames: int) -> [float] = run(
    let step = (to - from) / float(frames),
    for i in 0..=frames yield from + float(i) * step,
)

animate(from: 0.0, to: 1.0, frames: 5)
// [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
```

---

## Design Rationale

### Why `by` Keyword?

Alternatives considered:

| Syntax | Precedent | Problem |
|--------|-----------|---------|
| `0..10:2` | None | Colon overloaded |
| `0..10..2` | None | Confusing with existing `..` |
| `0..10 step 2` | Kotlin | `step` common variable name |
| `(0..10).step(2)` | Ruby/Rust | Verbose, method chaining |
| **`0..10 by 2`** | Natural language | Clear, reads naturally |

`by` reads naturally: "zero to ten by two" = "0, 2, 4, 6, 8".

### Why Not a Method?

```ori
(0..10).by(2)  // Alternative
```

This works but:
1. Requires parentheses around the range
2. Less readable than infix syntax
3. Doesn't match Ori's expression-oriented style

### Why Allow Negative Steps?

Descending ranges are common:
- Countdowns
- Reverse iteration
- Stack unwinding

Rather than separate syntax for descending ranges, allowing negative steps is more general and intuitive.

### Why Empty Range for Mismatched Direction?

```ori
0..10 by -1  // Empty, not error
```

This matches the principle of least surprise:
- The range "0 to 10 stepping by -1" contains no valid values
- Returning empty is consistent with other empty ranges
- Allows safe use in generic code without direction checks

Alternative (panic) was rejected as too strict for dynamic step values.

### Why Integer-Only?

Float iteration is inherently error-prone due to IEEE 754 precision. For example, `0.0..1.0 by 0.1` may or may not include values near 0.9 depending on accumulated floating-point error.

Users can iterate with `int` and convert:
```ori
for i in 0..10 yield float(i) * 0.1
```

This keeps the feature simple and avoids a class of subtle bugs.

---

## Implementation Notes

The `by` clause extends range expressions. Implementation details:

- `by` becomes a contextual keyword following range expressions
- The Range type gains an optional step field (defaults to 1)
- Iterator implementation handles ascending/descending based on step sign
- Zero step is a runtime panic

### Parser Changes

Add `by` as a contextual keyword following range expressions. Update the grammar:

```
range_expr = shift_expr [ ( ".." | "..=" ) shift_expr [ "by" shift_expr ] ] .
```

---

## Summary

| Syntax | Meaning |
|--------|---------|
| `0..10` | 0, 1, 2, ..., 9 |
| `0..=10` | 0, 1, 2, ..., 10 |
| `0..10 by 2` | 0, 2, 4, 6, 8 |
| `0..=10 by 2` | 0, 2, 4, 6, 8, 10 |
| `10..0 by -1` | 10, 9, 8, ..., 1 |
| `10..=0 by -1` | 10, 9, 8, ..., 0 |

The `by` keyword provides a natural, readable way to specify range steps, enabling common iteration patterns without manual list construction or helper functions.

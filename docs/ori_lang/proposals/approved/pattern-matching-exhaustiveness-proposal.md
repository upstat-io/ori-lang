# Proposal: Pattern Matching Exhaustiveness

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Approved:** 2026-01-30
**Affects:** Compiler, type system, diagnostics

---

## Summary

This proposal specifies the algorithm and rules for pattern matching exhaustiveness checking, including how the compiler determines if all cases are covered, when errors are issued, and how pattern features (guards, or-patterns, at-patterns) interact with exhaustiveness.

---

## Problem Statement

The spec mentions pattern matching but lacks:

1. **Exhaustiveness algorithm**: How does the compiler verify all cases are covered?
2. **Error policy**: When is non-exhaustiveness an error?
3. **Or-pattern semantics**: How do `A | B` patterns affect exhaustiveness?
4. **Guard interaction**: How do guards affect exhaustiveness checking?
5. **Refutability rules**: Which patterns can fail to match?

---

## Exhaustiveness Checking

### Definition

A pattern match is **exhaustive** if every possible value of the scrutinee type matches at least one pattern arm.

```ori
// Exhaustive: covers all Option<T> values
match opt {
    Some(x) -> use(x)
    None -> default
}

// Non-exhaustive: missing None case
match opt {
    Some(x) -> use(x)
    // Missing: None
}
```

### Algorithm

The compiler uses **pattern matrix decomposition** (based on Maranget's algorithm):

1. Build a matrix of pattern columns (one column per scrutinee component)
2. Check if the matrix covers the "wildcard row" (matches any value)
3. Recursively decompose by constructors until base cases

For each type, the compiler knows its constructors:
- `bool`: `true`, `false`
- `Option<T>`: `Some(_)`, `None`
- `Result<T, E>`: `Ok(_)`, `Err(_)`
- Sum types: all declared variants
- Integers: infinite (requires wildcard)
- Strings: infinite (requires wildcard)

### Error Policy

| Context | Non-Exhaustive | Rationale |
|---------|---------------|-----------|
| `match` expression | **Error** | Must handle all cases to return a value |
| `let` binding destructure | **Error** | Must match to bind |
| Function clause patterns | **Error** | All clauses together must be exhaustive |

Non-exhaustiveness is always an error in Ori — there is no "partial match" construct.

---

## Pattern Types and Refutability

### Irrefutable Patterns

Patterns that always match:

| Pattern | Example | Always matches? |
|---------|---------|-----------------|
| Wildcard | `_` | Yes |
| Variable binding | `x` | Yes |
| Struct (all fields irrefutable) | `Point { x, y }` | Yes |
| Tuple (all elements irrefutable) | `(a, b)` | Yes |

### Refutable Patterns

Patterns that may fail to match:

| Pattern | Example | May fail? |
|---------|---------|-----------|
| Literal | `42`, `"hello"` | Yes |
| Variant | `Some(x)`, `None` | Yes |
| Range | `0..10` | Yes |
| List with length | `[a, b]` | Yes |
| Guard | `x.match(x > 0)` | Yes |

### Refutability Requirements

| Context | Requirement |
|---------|-------------|
| `match` arm | Any pattern (refutable OK) |
| `let` binding | Must be irrefutable |
| Function parameter | Must be irrefutable |
| `for` loop variable | Must be irrefutable |

```ori
// OK: irrefutable in let
let (x, y) = get_point()
let Point { x, y } = get_point()

// ERROR: refutable in let
let Some(x) = maybe_value  // Error: pattern may not match
let [a, b] = get_list()    // Error: list may have wrong length
```

---

## Or-Patterns

### Syntax

```ori
match color {
    Red | Green | Blue -> "primary"
    _ -> "other"
}
```

### Exhaustiveness with Or-Patterns

Or-patterns contribute their combined coverage:

```ori
type Light = Red | Yellow | Green

// Exhaustive via or-pattern
match light {
    Red | Yellow -> "stop"
    Green -> "go"
}
```

### Binding Rules

Bindings in or-patterns must:
1. Appear in all alternatives
2. Have the same type in all alternatives

```ori
// OK: x bound in both alternatives
match result {
    Ok(x) | Err(x) -> print(x),  // x: same type in both
}

// ERROR: x not bound in all alternatives
match opt {
    Some(x) | None -> x,  // Error: x not bound in None
}
```

---

## Guards

### Syntax

Guards use the `.match(condition)` syntax on bindings:

```ori
match n {
    x.match(x > 0) -> "positive"
    x.match(x < 0) -> "negative"
    0 -> "zero"
}
```

### Exhaustiveness with Guards

**Guards are not considered for exhaustiveness checking.** The compiler cannot statically verify guard conditions, so matches with guards must include a catch-all pattern.

```ori
// ERROR: guards require catch-all
match n {
    x.match(x > 0) -> "positive"
    x.match(x < 0) -> "negative"
    // Error: patterns not exhaustive due to guards
    // Add a wildcard pattern to ensure all cases are covered
}

// OK: catch-all ensures exhaustiveness
match n {
    x.match(x > 0) -> "positive"
    x.match(x < 0) -> "negative"
    _ -> "zero"
}
```

### Guard Evaluation

Guards are evaluated only if the structural pattern matches:

```ori
match (x, y) {
    (0, _) -> "x is zero"
    (_, y).match(y > 10) -> "y is large",  // Guard only checked if first pattern fails
    _ -> "other"
}
```

Guards have access to bindings from the pattern:

```ori
match point {
    Point { x, y }.match(x == y) -> "diagonal"
    Point { x, y }.match(x > y) -> "above"
    _ -> "below or on"
}
```

---

## At-Patterns

### Syntax

```ori
match opt {
    whole @ Some(x) -> use_both(whole, x)
    None -> default
}
```

### Semantics

At-patterns bind the whole matched value AND destructure it:

- `whole` binds to the entire `Option<T>` value
- `x` binds to the inner value (if `Some`)

### Exhaustiveness

At-patterns contribute same exhaustiveness as their inner pattern:

```ori
// whole @ Some(x) covers same cases as Some(x)
match opt {
    whole @ Some(x) -> ...
    None -> ...,  // Still need this for exhaustiveness
}
```

---

## List Patterns

### Syntax

```ori
match list {
    [] -> "empty"
    [x] -> "singleton"
    [x, y] -> "pair"
    [x, ..rest] -> "at least one"
}
```

### Exhaustiveness

List patterns match by length and structure:

| Pattern | Matches |
|---------|---------|
| `[]` | Empty list only |
| `[x]` | Exactly one element |
| `[x, y]` | Exactly two elements |
| `[x, ..rest]` | One or more elements |
| `[..rest]` | Any list (including empty) |

To be exhaustive, must cover all lengths:

```ori
// Exhaustive
match list {
    [] -> "empty"
    [x, ..rest] -> "non-empty"
}

// Also exhaustive
match list {
    [..rest] -> "any"
}

// Non-exhaustive
match list {
    [x] -> "one"
    [x, y] -> "two"
    // Error: doesn't cover empty or 3+ elements
}
```

---

## Range Patterns

### Syntax

```ori
match n {
    0..10 -> "small"
    10..100 -> "medium"
    _ -> "large"
}
```

### Exhaustiveness

Integer ranges cannot be exhaustive without a wildcard (infinite domain):

```ori
// Non-exhaustive (even with many ranges)
match n {
    0..100 -> "small"
    100..1000 -> "medium"
    // Error: doesn't cover negative or >= 1000
}

// Exhaustive with wildcard
match n {
    0..100 -> "small"
    100..1000 -> "medium"
    _ -> "other"
}
```

### Range Overlap

The compiler warns about overlapping ranges:

```ori
match n {
    0..10 -> "a"
    5..15 -> "b",  // Warning: overlaps with previous pattern (5..10 unreachable)
    _ -> "c"
}
```

---

## Unreachable Pattern Detection

The compiler warns about patterns that can never match:

```ori
match opt {
    Some(x) -> use(x)
    None -> default
    Some(y) -> other,  // Warning: unreachable pattern (already covered)
}

match color {
    Red -> "red"
    _ -> "other"
    Blue -> "blue",  // Warning: unreachable pattern (covered by _)
}
```

### Diagnostic Levels

| Situation | Diagnostic |
|-----------|------------|
| Completely unreachable pattern | Warning |
| Partially unreachable (overlap) | Warning |
| Redundant wildcard | Warning |
| Missing cases | Error |
| Guards without catch-all | Error |

---

## Struct Patterns

### Exhaustiveness

Struct patterns are exhaustive if they match all fields:

```ori
type Point = { x: int, y: int }

// Exhaustive (only one constructor)
match point {
    Point { x, y } -> use(x, y)
}

// Also exhaustive (wildcard for fields)
match point {
    Point { .. } -> "a point"
}
```

### Partial Field Matching

```ori
match point {
    Point { x: 0, .. } -> "on y-axis"
    Point { y: 0, .. } -> "on x-axis"
    Point { .. } -> "elsewhere"
}
```

---

## Error Messages

### Non-Exhaustive Match

```
error[E0123]: non-exhaustive patterns
  --> src/main.ori:10:5
   |
10 |     match(opt,
   |     ^^^^^ patterns `None` not covered
   |
   = help: add a pattern for `None` or use a wildcard `_`
```

### Unreachable Pattern

```
warning[W0456]: unreachable pattern
  --> src/main.ori:14:5
   |
12 |     Some(x) -> use(x),
   |     ------- first matching pattern
13 |     None -> default,
14 |     Some(y) -> other,
   |     ^^^^^^ this pattern is unreachable
   |
   = note: this arm will never be executed
```

### Guard Coverage Error

```
error[E0124]: patterns not exhaustive due to guards
  --> src/main.ori:10:5
   |
10 |     match(n,
   |     ^^^^^ guards prevent exhaustiveness verification
   |
   = help: add a wildcard pattern `_ ->` to cover remaining cases
```

---

## Spec Changes Required

### Extend `10-patterns.md`

Add the following sections:
1. Exhaustiveness algorithm description
2. Refutability rules
3. Or-pattern semantics
4. Guard interaction with exhaustiveness
5. At-pattern semantics
6. List pattern exhaustiveness
7. Error policy

### Update Diagnostics

Specify error codes and message formats for:
- Non-exhaustive match (E0123)
- Unreachable pattern (W0456)
- Guard coverage error (E0124)
- Overlapping ranges (W0457)

---

## Summary

| Aspect | Specification |
|--------|--------------|
| Algorithm | Pattern matrix decomposition |
| Non-exhaustive match | Compile error |
| Refutable in `let` | Compile error |
| Guards | Not considered for exhaustiveness; require catch-all |
| Or-patterns | Combined coverage, consistent bindings |
| At-patterns | Same coverage as inner pattern |
| List patterns | Must cover all lengths |
| Range patterns | Need wildcard for integers |
| Unreachable | Warning |
| Overlapping | Warning |

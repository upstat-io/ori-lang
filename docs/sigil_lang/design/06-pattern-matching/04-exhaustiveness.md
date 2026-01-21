# Exhaustiveness

This document covers Sigil's exhaustiveness checking: compiler enforcement of complete pattern matching, wildcard warnings, and suppression with `#[allow]`.

---

## Overview

Exhaustiveness checking ensures that match expressions handle all possible cases. The compiler analyzes patterns and reports missing cases at compile time, preventing runtime errors from unhandled values.

```sigil
type Status = Pending | Running | Completed | Failed(str)

// ERROR: non-exhaustive match
@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running -> "in progress",
    Completed -> "done"
    // Missing: Failed
)
```

---

## How Exhaustiveness Works

### Sum Type Coverage

For sum types, every variant must be matched:

```sigil
type Option<T> = Some(T) | None

// Complete: all variants covered
@unwrap_or (opt: Option<int>, default: int) -> int = match(opt,
    Some(value) -> value,
    None -> default
)
```

### Pattern Overlap

The compiler tracks which values each pattern covers:

```sigil
type Color = Red | Green | Blue | Yellow | Cyan | Magenta

// Each pattern covers one variant
@to_hex (c: Color) -> str = match(c,
    Red -> "#FF0000",
    Green -> "#00FF00",
    Blue -> "#0000FF",
    Yellow -> "#FFFF00",
    Cyan -> "#00FFFF",
    Magenta -> "#FF00FF"
)
```

### Nested Types

Exhaustiveness extends to nested structures:

```sigil
type Result<T, E> = Ok(T) | Err(E)
type Option<T> = Some(T) | None

@process (r: Result<Option<int>, str>) -> int = match(r,
    Ok(Some(n)) -> n,
    Ok(None) -> 0,
    Err(_) -> -1
)
// Complete: covers Ok(Some), Ok(None), and Err
```

---

## Missing Case Errors

### Basic Missing Variant

```
error[E0400]: non-exhaustive match
  |
5 | @describe (s: Status) -> str = match(s,
6 |     Pending -> "waiting",
7 |     Running -> "in progress"
8 | )
  | ^ missing variants: Completed, Failed

help: add missing patterns or use wildcard `_`
```

### Missing Nested Cases

```
error[E0400]: non-exhaustive match
   |
10 | @process (r: Result<Option<int>, str>) -> int = match(r,
11 |     Ok(Some(n)) -> n,
12 |     Err(_) -> -1
13 | )
   | ^ missing: Ok(None)
```

### Missing Literal Cases

For literal patterns, the compiler cannot enumerate all possibilities:

```sigil
// ERROR: int has infinite values
@describe (n: int) -> str = match(n,
    0 -> "zero",
    1 -> "one"
)
// Missing: all other integers

// Fixed with wildcard
@describe (n: int) -> str = match(n,
    0 -> "zero",
    1 -> "one",
    _ -> "other"
)
```

---

## Wildcard Pattern

The `_` pattern matches any value, making the match exhaustive.

### Basic Usage

```sigil
@is_success (s: Status) -> bool = match(s,
    Completed -> true,
    _ -> false  // matches Pending, Running, Failed
)
```

### Must Be Last

Wildcards must be the final pattern:

```sigil
// ERROR: unreachable pattern after wildcard
@bad (s: Status) -> str = match(s,
    _ -> "unknown",
    Completed -> "done"  // never reached
)
```

---

## Wildcard Warnings

The compiler warns when wildcards hide specific variants, helping you catch missing handlers when types evolve.

### Warning on Hidden Variants

```
warning[W0140]: wildcard pattern hides variants
   |
10 | @is_active (s: Status) -> bool = match(s,
11 |     Running -> true,
12 |     _ -> false
   |     ^ hides: Pending, Completed, Failed

help: consider explicit handling or #[allow(wildcard_enum)]
```

### Why This Matters

When a new variant is added to a type:

```sigil
// Before: Status = Pending | Running | Completed | Failed(str)
// After:  Status = Pending | Running | Completed | Failed(str) | Cancelled

@is_active (s: Status) -> bool = match(s,
    Running -> true,
    _ -> false  // silently handles new Cancelled
)
```

The warning alerts you that `Cancelled` is now handled by the wildcard, when it might need explicit treatment.

---

## Suppressing Warnings

When wildcard behavior is intentional, suppress warnings with attributes.

### Per-Match Suppression

```sigil
#[allow(wildcard_enum)]
@is_running (s: Status) -> bool = match(s,
    Running -> true,
    _ -> false  // intentionally catch-all
)
```

### Per-Function Suppression

```sigil
#[allow(wildcard_enum)]
@status_handlers (s: Status) -> Handler = match(s,
    Running -> active_handler,
    Failed(_) -> error_handler,
    _ -> default_handler  // intentional fallback
)
```

### File-Level Suppression

At the top of a file:

```sigil
#![allow(wildcard_enum)]

// All matches in this file can use wildcards without warning
```

---

## Guards and Exhaustiveness

Guards do not affect exhaustiveness. Patterns must cover all structural cases regardless of guards.

### Guards Don't Count as Coverage

```sigil
// ERROR: non-exhaustive
@classify (n: int) -> str = match(n,
    x if x < 0 -> "negative",
    x if x > 0 -> "positive"
)
// Missing: x == 0

// Fixed
@classify (n: int) -> str = match(n,
    x if x < 0 -> "negative",
    x if x > 0 -> "positive",
    _ -> "zero"
)
```

### Overlapping Guards

```sigil
// Complete: _ catches any value not matched by guards
@grade (score: int) -> str = match(score,
    s if s >= 90 -> "A",
    s if s >= 80 -> "B",
    s if s >= 70 -> "C",
    _ -> "below C"
)
```

---

## Or Patterns and Coverage

Or patterns combine coverage from multiple alternatives.

### Combined Coverage

```sigil
type Day = Mon | Tue | Wed | Thu | Fri | Sat | Sun

// Complete: all days covered by two or patterns
@day_type (d: Day) -> str = match(d,
    Mon | Tue | Wed | Thu | Fri -> "weekday",
    Sat | Sun -> "weekend"
)
```

### Partial Or Patterns

```sigil
// ERROR: non-exhaustive
@day_type (d: Day) -> str = match(d,
    Mon | Tue | Wed -> "early week",
    Sat | Sun -> "weekend"
)
// Missing: Thu, Fri
```

---

## Boolean Exhaustiveness

Booleans must cover both cases:

```sigil
@describe (b: bool) -> str = match(b,
    true -> "yes",
    false -> "no"
)

// Or with wildcard
@describe (b: bool) -> str = match(b,
    true -> "yes",
    _ -> "no"
)
```

---

## Tuple and Struct Exhaustiveness

### Tuples

Must cover all combinations:

```sigil
@both_true (a: bool, b: bool) -> bool = match((a, b),
    (true, true) -> true,
    (true, false) -> false,
    (false, true) -> false,
    (false, false) -> false
)

// Or simplified
@both_true (a: bool, b: bool) -> bool = match((a, b),
    (true, true) -> true,
    _ -> false
)
```

### Structs

Struct patterns are always irrefutable (all fields exist):

```sigil
type Point = { x: int, y: int }

// Always exhaustive: struct always has x and y
@quadrant (p: Point) -> int = match(p,
    { x, y } if x >= 0 && y >= 0 -> 1,
    { x, y } if x < 0 && y >= 0 -> 2,
    { x, y } if x < 0 && y < 0 -> 3,
    _ -> 4
)
```

---

## List Pattern Exhaustiveness

Lists require careful handling of all lengths:

```sigil
// Complete: covers empty, single, and multiple
@describe (items: [int]) -> str = match(items,
    [] -> "empty",
    [_] -> "single",
    [_, _, ..] -> "multiple"
)

// Incomplete: missing empty
@bad (items: [int]) -> str = match(items,
    [x, ..rest] -> "has items"
)
// ERROR: missing: []
```

---

## Exhaustiveness with Generics

Generic types require exhaustiveness over all type parameters:

```sigil
type Option<T> = Some(T) | None

// Works for any T
@is_some<T> (opt: Option<T>) -> bool = match(opt,
    Some(_) -> true,
    None -> false
)
```

### Constrained Generics

```sigil
@process<T> (opt: Option<T>) -> str where T: Printable = match(opt,
    Some(value) -> value.to_string(),
    None -> "nothing"
)
```

---

## Unreachable Patterns

The compiler detects patterns that can never match.

### After Wildcard

```
error[E0401]: unreachable pattern
  |
5 | @bad (n: int) -> str = match(n,
6 |     _ -> "any",
7 |     0 -> "zero"
  |     ^ this pattern is never reached
```

### Subsumed by Earlier Pattern

```
error[E0401]: unreachable pattern
  |
5 | @bad (s: Status) -> str = match(s,
6 |     Pending | Running | Completed | Failed(_) -> "any status",
7 |     Running -> "running"
  |     ^^^^^^^ this pattern is never reached (already covered above)
```

### Contradictory Guards

```
error[E0401]: unreachable pattern
  |
5 | @bad (n: int) -> str = match(n,
6 |     x if x > 0 && x < 0 -> "impossible"
  |                           ^^^^^^^^^^^^ guard is always false
```

---

## Best Practices

### Prefer Explicit Over Wildcard

```sigil
// Preferred: explicit handling
@handle (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running -> "active",
    Completed -> "done",
    Failed(msg) -> "error: " + msg
)

// Less safe: wildcard hides cases
@handle (s: Status) -> str = match(s,
    Running -> "active",
    _ -> "not running"
)
```

### Use Wildcard for Intentional Catch-All

```sigil
// Good: explicit about catch-all behavior
#[allow(wildcard_enum)]
@is_error (code: int) -> bool = match(code,
    400..500 -> true,   // client errors
    500..600 -> true,   // server errors
    _ -> false          // all other codes
)
```

### Review Warnings During Type Changes

When modifying sum types, check all matches:

```sigil
// Adding new variant to Status
type Status = ... | Paused

// Compiler warns about all wildcards that now hide Paused
// Review each one to decide if Paused needs explicit handling
```

---

## Configuration

### Compiler Flags

```bash
# Treat wildcard warnings as errors
sigil build --deny wildcard-enum

# Suppress all wildcard warnings
sigil build --allow wildcard-enum

# Show additional exhaustiveness info
sigil build --explain-exhaustiveness
```

### Project Configuration

In `sigil.toml`:

```toml
[warnings]
wildcard_enum = "warn"  # "allow", "warn", or "deny"
```

---

## Error Reference

| Error | Description |
|-------|-------------|
| E0400 | Non-exhaustive match |
| E0401 | Unreachable pattern |
| W0140 | Wildcard hides variants |
| W0141 | Overlapping patterns |

---

## See Also

- [Match Pattern](01-match-pattern.md) — Basic match syntax
- [Guards and Bindings](03-guards-and-bindings.md) — Guards and or patterns
- [Type Narrowing](05-type-narrowing.md) — Flow-sensitive typing
- [User-Defined Types](../03-type-system/03-user-defined-types.md) — Sum types

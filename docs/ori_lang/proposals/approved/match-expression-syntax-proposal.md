# Proposal: Match Expression Syntax

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Compiler, parser, type system, spec
**Depends On:** Pattern Matching Exhaustiveness Proposal (approved)

---

## Summary

This proposal formalizes the syntax and semantics of match expressions in Ori. Match expressions provide pattern matching with exhaustiveness checking, enabling concise and safe destructuring of values.

---

## Problem Statement

The match expression syntax has been implemented but lacks formal specification. This proposal documents:

1. **Match expression syntax**: The `match(scrutinee, arms...)` form
2. **Match arm syntax**: Pattern with optional guard and body
3. **Guard syntax**: The `.match(condition)` form
4. **Pattern types**: All supported pattern forms
5. **Evaluation semantics**: Top-to-bottom, first-match-wins
6. **Type inference**: How match expressions are typed

---

## Match Expression Syntax

### Basic Form

```ori
match scrutinee { pattern -> expression, ...}
```

A match expression consists of:
1. The `match` keyword
2. Opening parenthesis `(`
3. A **scrutinee** expression (the value being matched)
4. A comma `,`
5. One or more **match arms** (comma-separated)
6. Closing parenthesis `)`

### Examples

```ori
// Simple match on Option
match opt {
    Some(x) -> x * 2
    None -> 0
}

// Match on literals
match n {
    0 -> "zero"
    1 -> "one"
    _ -> "many"
}

// Match with computation in arms
match status {
    Pending -> calculate_pending()
    Running(progress) -> progress * 100
    Done(result) -> result.value
}
```

---

## Match Arm Syntax

### Structure

```
pattern [guard] -> expression
```

A match arm consists of:
1. A **pattern** (see Pattern Types below)
2. An optional **guard** (`.match(condition)`)
3. An arrow `->`
4. A **body expression**

### Pattern Binding Scope

Bindings introduced in the pattern are in scope for:
1. The guard expression (if present)
2. The body expression

```ori
match point {
    Point { x, y }.match(x == y) -> "diagonal: " + str(x)
    //       ^ x,y bound here        ^ used in guard    ^ used in body
    Point { x, y } -> str(x) + "," + str(y)
}
```

---

## Guard Syntax

### Form

```
pattern.match(condition)
```

Guards use the `.match(condition)` suffix on a pattern:

1. The pattern is evaluated first
2. If the pattern matches, the guard condition is evaluated
3. The arm matches only if both pattern and guard succeed

### Semantics

- Guards have access to bindings from the pattern
- Guard expression must be of type `bool`
- Guards are **not** considered for exhaustiveness (see Exhaustiveness Proposal)
- Matches using guards require a catch-all pattern

### Examples

```ori
// Basic guard usage
match n {
    x.match(x > 0) -> "positive"
    x.match(x < 0) -> "negative"
    _ -> "zero",  // Required: guards don't ensure exhaustiveness
}

// Guard with struct destructuring
match rect {
    Rectangle { width, height }.match(width == height) -> "square"
    Rectangle { width, height } -> "rectangle"
}

// Multiple conditions in guard
match point {
    Point { x, y }.match(x > 0 && y > 0) -> "quadrant I"
    Point { x, y }.match(x < 0 && y > 0) -> "quadrant II"
    _ -> "other"
}
```

---

## Pattern Types

### Wildcard Pattern

```ori
_
```

Matches any value without binding it.

```ori
match opt {
    Some(_) -> "has value"
    None -> "empty"
}
```

### Binding Pattern

```ori
identifier
```

Matches any value and binds it to the identifier.

```ori
match opt {
    Some(x) -> x * 2,  // x bound to inner value
    None -> 0
}
```

### Literal Pattern

```ori
42
-1
"hello"
'a'
true
false
```

Matches exact literal values. Numeric literals must be integers; float literals are not supported in patterns. Negative integer literals are supported.

```ori
match n {
    0 -> "zero"
    1 -> "one"
    -1 -> "negative one"
    _ -> "other"
}
```

### Variant Pattern

```ori
VariantName
VariantName(inner_pattern)
VariantName(pattern1, pattern2)
```

Matches enum/sum type variants, optionally destructuring payloads.

```ori
// Built-in variants
match opt { Some(x) -> x, None -> 0}
match res { Ok(v) -> v, Err(e) -> handle(e)}

// User-defined variants
type Color = Red | Green | Blue | Rgb(int, int, int)

match color {
    Red -> "#FF0000"
    Green -> "#00FF00"
    Blue -> "#0000FF"
    Rgb(r, g, b) -> format_rgb(r, g, b)
}
```

### Struct Pattern

```ori
{ field1, field2 }
{ field1: pattern1, field2: pattern2 }
TypeName { field1, field2 }
{ field1, .. }
```

Matches struct values, with optional type name and field patterns.

```ori
// Field shorthand (binds to field name)
match point { Point { x, y } -> x + y}

// Field with pattern
match point { Point { x: 0, y } -> "on y-axis"}

// Partial match with rest
match point { Point { x, .. } -> x}

// Anonymous struct pattern
match data { { name, age } -> name + str(age)}
```

### Tuple Pattern

```ori
(pattern1, pattern2)
()
```

Matches tuple values.

```ori
match pair {
    (0, 0) -> "origin"
    (x, 0) -> "on x-axis"
    (0, y) -> "on y-axis"
    (x, y) -> "point"
}
```

### List Pattern

```ori
[]
[pattern]
[pattern1, pattern2]
[head, ..tail]
[..rest]
[first, ..middle, last]
```

Matches lists by length and element patterns.

```ori
match list {
    [] -> "empty"
    [x] -> "singleton: " + str(x)
    [x, y] -> "pair: " + str(x) + ", " + str(y)
    [head, ..tail] -> "head: " + str(head) + ", rest length: " + str(len(tail))
}
```

### Range Pattern

```ori
start..end      // Exclusive end
start..=end     // Inclusive end
```

Matches values within a range. Only integer literals are supported.

```ori
match score {
    0..60 -> "F"
    60..70 -> "D"
    70..80 -> "C"
    80..90 -> "B"
    90..=100 -> "A"
    _ -> "invalid"
}
```

### Or-Pattern

```ori
pattern1 | pattern2
```

Matches if any alternative matches. Bindings must be consistent across alternatives.

```ori
match light {
    Red | Yellow -> "stop"
    Green -> "go"
}

// With bindings (must appear in all alternatives with same type)
match result {
    Ok(x) | Err(x) -> process(x),  // x bound in both
}
```

### At-Pattern

```ori
name @ pattern
```

Binds the whole matched value while also destructuring.

```ori
match opt {
    whole @ Some(inner) -> use_both(whole, inner)
    None -> default
}
```

---

## Evaluation Semantics

### Order of Evaluation

1. The scrutinee expression is evaluated exactly once
2. Arms are tried in **top-to-bottom** order
3. For each arm:
   a. The pattern is matched against the scrutinee
   b. If the pattern matches and a guard is present, the guard is evaluated
   c. If both pattern and guard (if present) succeed, the arm body is evaluated
4. The first matching arm's body is the result
5. Subsequent arms are not evaluated (short-circuit)

### First-Match-Wins

```ori
match n {
    x.match(x > 0) -> "positive",  // Checked first
    1 -> "one",                     // Never reached for positive numbers
    _ -> "other"
}
```

### Exhaustiveness

Match expressions must be exhaustive. See the Pattern Matching Exhaustiveness proposal for details.

---

## Type Inference

### Scrutinee Type

The scrutinee expression is inferred independently, then used to validate patterns.

### Arm Body Types

All arm bodies must have compatible types. The match expression's type is the unified type of all arm bodies.

```ori
// All arms return int
let x: int = match opt {
    Some(n) -> n
    None -> 0
}

// Mixed types unify to common type
let s: str = match opt {
    Some(n) -> str(n)
    None -> "none"
}
```

### Pattern Type Checking

Patterns are checked against the scrutinee type:

| Pattern | Scrutinee Type Constraint |
|---------|--------------------------|
| Literal `42` | Must be `int` |
| `Some(x)` | Must be `Option<T>`, `x: T` |
| `Ok(x)` / `Err(e)` | Must be `Result<T, E>` |
| `{ x, y }` | Must have fields `x` and `y` |
| `[a, b]` | Must be list type `[T]` |

---

## Grammar

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § PATTERNS

---

## Comparison with Other Languages

| Feature | Ori | Rust | OCaml | Haskell |
|---------|-----|------|-------|---------|
| Syntax | `match(x, ...)` | `match x { }` | `match x with` | `case x of` |
| Guards | `.match(cond)` | `if cond` | `when cond` | `\| cond` |
| Or-patterns | `A \| B` | `A \| B` | `A \| B` | No |
| At-patterns | `x @ pat` | `x @ pat` | `x as pat` | `x@pat` |
| Exhaustive | Required | Required | Required | Required |

### Design Rationale

1. **Function-call syntax**: `match(x, ...)` is consistent with other sequential patterns (`run`, `try`)
2. **Guard syntax**: `.match(cond)` reads naturally as "x where x matches condition"
3. **Comma-separated arms**: Consistent with other multi-element constructs in Ori
4. **Required exhaustiveness**: Ensures all cases are handled, preventing runtime errors

---

## Implementation Status

| Component | Status |
|-----------|--------|
| Parser | Implemented |
| IR representation | Implemented |
| Type checking | Implemented |
| Exhaustiveness checking | Implemented |
| Code generation | Implemented |
| Test coverage | Comprehensive |

---

## Spec Changes Required

This proposal formalizes existing behavior. The spec at `docs/ori_lang/0.1-alpha/spec/10-patterns.md` already documents match expressions. Verify that:

1. All pattern types are documented
2. Guard syntax is specified
3. Evaluation order is clear
4. Grammar matches implementation

---

## Open Questions

None. This proposal documents implemented behavior.

---

## Summary

Match expressions in Ori provide pattern matching with:

- **Function-call syntax**: `match(scrutinee, arms...)`
- **Rich patterns**: literals, bindings, variants, structs, tuples, lists, ranges, or, at
- **Guards**: `.match(condition)` suffix for conditional matching
- **Exhaustiveness**: Compile-time verification that all cases are handled
- **First-match-wins**: Top-to-bottom evaluation with short-circuit
- **Type unification**: All arm bodies must have compatible types

---

## Errata (added 2026-02-20)

> **Superseded by [block-expression-syntax](block-expression-syntax.md)**: Match syntax changed from function-call `match(scrutinee, arms...)` to block `match expr { arms }`.
>
> **Superseded by [match-arm-comma-separator-proposal](match-arm-comma-separator-proposal.md)**: Guard syntax changed from `.match(condition)` to `if condition`. Arms are comma-separated (not newline-separated). `.match()` now exclusively refers to method-style pattern matching.

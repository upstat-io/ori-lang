# Proposal: Simplified Bindings with `$` for Immutability

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-25
**Supersedes:** mutable-by-default-proposal.md, const-keyword-proposal.md, rename-config-to-constants-proposal.md

---

## Summary

Simplify Sigil's binding model to two forms:

```sigil
let x = 5       // mutable — can reassign
let $x = 5      // immutable — cannot reassign
```

This removes three keywords (`mut`, `readonly`, `const`) and unifies compile-time constants with immutable bindings under a single syntax.

---

## Motivation

### The Problem

Sigil currently has multiple overlapping concepts:

| Concept | Syntax | Purpose |
|---------|--------|---------|
| Mutable binding | `let mut x = 5` | Can reassign |
| Immutable binding | `let x = 5` | Cannot reassign |
| Compile-time constant | `$x = 5` | Module-level, compile-time |

This creates confusion:
- `mut` is borrowed from Rust but provides no safety benefit without a borrow checker
- `$x = 5` uses a sigil but only at module level
- The distinction between "immutable binding" and "constant" is unclear to most developers

### The Solution

Unify everything under `let` with a simple rule:

- **`let x`** — mutable (can reassign)
- **`let $x`** — immutable (cannot reassign)

The `$` prefix becomes part of the identifier and signals immutability. This works at any scope — module-level or local.

### Benefits

1. **One keyword** — Just `let`, no `mut`/`readonly`/`const`
2. **Visual distinction** — `$` in the name signals "this won't change"
3. **Consistent** — Same syntax at module level and local scope
4. **Familiar default** — Mutable by default like Python/JS/Go
5. **Opt-in immutability** — Add `$` when you want to lock a binding

---

## Design

### Grammar

```
binding = "let" pattern [ ":" type ] "=" expression .
pattern = "$" identifier | identifier | ... .
```

The `$` prefix is part of the identifier. An identifier starting with `$` creates an immutable binding.

### Mutable Bindings (Default)

```sigil
let x = 5
x = 6           // ok

let name = "Alice"
name = "Bob"    // ok
```

### Immutable Bindings (`$` prefix)

```sigil
let $x = 5
$x = 6          // error: cannot assign to immutable binding '$x'

let $name = "Alice"
$name = "Bob"   // error: cannot assign to immutable binding '$name'
```

### Module-Level Constants

```sigil
let $timeout = 30s
let $api_base = "https://api.example.com"
let $max_retries = 3

pub let $default_limit = 100
```

Usage:
```sigil
@fetch () -> Result<Data, Error> uses Http =
    timeout(op: Http.get($api_base + "/data"), after: $timeout)
```

The `$` appears at both definition and usage sites, providing visual distinction.

### Local Immutable Bindings

```sigil
@process (input: int) -> int = run(
    let $base = expensive_calculation(input),

    // ... lots of code ...

    $base = 0,  // error: cannot assign to immutable binding '$base'

    $base * 2,
)
```

### Compile-Time Evaluation

The compiler determines if a `$`-prefixed binding can be evaluated at compile time:

```sigil
let $a = 5                    // compile-time: literal
let $b = $a * 2               // compile-time: references constant
let $c = square(n: 10)        // compile-time if square is pure and args are const

let $d = compute()            // runtime: non-const function (still immutable)
```

Immutability is guaranteed. Compile-time evaluation is an optimization the compiler performs when possible.

### Const Functions

Pure functions that can be evaluated at compile time:

```sigil
let $square = (x: int) -> int = x * x
let $factorial = (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1)

// Evaluated at compile time
let $fact_10 = $factorial(n: 10)  // 3628800
```

A function bound to a `$` name must be pure (no capabilities, no side effects).

### What Remains Always Immutable

Some bindings are always immutable regardless of `$`:

- **Function parameters** — Cannot reassign within function body
- **Loop variables** — `for item in items` — `item` cannot be reassigned

```sigil
@add (a: int, b: int) -> int = run(
    a = 10,  // error: cannot assign to parameter
    a + b,
)

for item in items do
    item = other  // error: cannot assign to loop variable
```

### Destructuring

The `$` prefix applies to individual bindings in destructuring:

```sigil
let { $x, y } = point      // x is immutable, y is mutable
let ($a, $b) = pair        // both immutable
let [$head, ..tail] = list // head is immutable, tail is mutable
```

---

## Examples

### Configuration Module

```sigil
// config.si
pub let $api_base = "https://api.example.com"
pub let $timeout = 30s
pub let $max_retries = 3
pub let $page_size = 20
```

```sigil
// client.si
use './config' { $api_base, $timeout }

@fetch_users (page: int) -> Result<[User], Error> uses Http = try(
    let url = $api_base + "/users?page=" + str(page),
    let response = timeout(op: Http.get(url), after: $timeout)?,
    parse_users(response.body),
)
```

### Mutable Counter

```sigil
@count_evens (numbers: [int]) -> int = run(
    let count = 0,
    for n in numbers do
        if n % 2 == 0 then count = count + 1 else (),
    count,
)
```

### Immutable Intermediate Result

```sigil
@transform (data: Data) -> Result<Output, Error> = run(
    let $validated = validate(data),  // lock this — shouldn't change

    // ... processing ...

    Ok(finalize($validated)),
)
```

### Mathematical Constants

```sigil
let $pi = 3.14159265358979
let $e = 2.71828182845904
let $golden_ratio = 1.61803398874989

@circle_area (radius: float) -> float = $pi * radius * radius
```

---

## Migration

### From Current Syntax

| Current | New |
|---------|-----|
| `let x = 5` | `let $x = 5` (if you want immutable) |
| `let x = 5` | `let x = 5` (if mutable is fine) |
| `let mut x = 5` | `let x = 5` |
| `$timeout = 30s` | `let $timeout = 30s` |
| `pub $timeout = 30s` | `pub let $timeout = 30s` |

### Keyword Changes

| Removed | Reason |
|---------|--------|
| `mut` | Default is now mutable |
| `const` | Replaced by `let $name` |
| `readonly` | Never needed — use `$` prefix |

### Spec Changes

| File | Change |
|------|--------|
| `03-lexical-elements.md` | Remove `mut` from keywords, update `$` description |
| `04-constants.md` | Rewrite for `let $name` syntax |
| `05-variables.md` | Flip default, document `$` prefix for immutability |
| `09-expressions.md` | Update `let_expr` grammar |

---

## Rationale

### Why Mutable by Default?

Sigil uses ARC, not borrowing. The `mut` keyword in Rust enables mutable borrows, which is critical for memory safety. In Sigil, `mut` only controlled reassignment — minor benefit, significant friction.

Higher-level languages (Python, JS, Go, Ruby) are mutable by default. Sigil aims to be approachable.

### Why `$` for Immutability?

1. **Visual distinction** — `$timeout` is clearly different from `timeout`
2. **Concise** — One character vs `readonly` or `const`
3. **At usage site** — You see `$` where the value is used, not just where it's defined
4. **Unified** — Same syntax for module-level constants and local immutables

### Why Not Separate `const` Keyword?

A separate `const` keyword creates two concepts (compile-time constant vs runtime immutable) that most developers don't distinguish. JavaScript's `const` is really just "can't reassign." Sigil follows this simpler mental model.

### Why Not `SCREAMING_CASE` Convention?

Conventions are not enforced by the compiler. The `$` prefix is syntactically enforced — you cannot reassign a `$`-prefixed binding.

---

## Summary

| Binding | Syntax | Reassignable |
|---------|--------|--------------|
| Mutable | `let x = 5` | Yes |
| Immutable | `let $x = 5` | No |

Three keywords removed: `mut`, `const`, `readonly`

One simple rule: **`$` in the name means immutable.**

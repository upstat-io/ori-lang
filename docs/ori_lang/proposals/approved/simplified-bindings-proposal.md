# Proposal: Simplified Bindings with `$` for Immutability

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-25
**Approved:** 2026-01-28
**Supersedes:** mutable-by-default-proposal.md, const-keyword-proposal.md, rename-config-to-constants-proposal.md

---

## Summary

Simplify Ori's binding model to two forms:

```ori
let x = 5       // mutable — can reassign
let $x = 5      // immutable — cannot reassign
```

This removes three keywords (`mut`, `readonly`, `const`) and unifies compile-time constants with immutable bindings under a single syntax.

---

## Motivation

### The Problem

Ori currently has multiple overlapping concepts:

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

The `$` prefix signals immutability. This works at any scope — module-level or local.

### Benefits

1. **One keyword** — Just `let`, no `mut`/`readonly`/`const`
2. **Visual distinction** — `$` in the name signals "this won't change"
3. **Consistent** — Same syntax at module level and local scope
4. **Familiar default** — Mutable by default like Python/JS/Go
5. **Opt-in immutability** — Add `$` when you want to lock a binding

---

## Design

### Grammar

```ebnf
// Module-level constant (immutable required)
constant_decl = [ "pub" ] "let" "$" identifier [ ":" type ] "=" expression .

// Local binding (mutable or immutable)
let_expr = "let" binding_pattern [ ":" type ] "=" expression .
binding_pattern = "$" identifier | identifier | destructure_pattern .

// Destructuring
destructure_pattern = "{" [ "$" ] field_binding { "," [ "$" ] field_binding } "}"
                    | "(" [ "$" ] identifier { "," [ "$" ] identifier } ")"
                    | "[" [ "$" ] identifier { "," [ "$" ] identifier } [ ".." identifier ] "]" .
```

### Identifier Rules

The `$` prefix is a modifier, not part of the identifier name. A binding for `$x` and a binding for `x` refer to the same name — they cannot coexist in the same scope.

```ori
let x = 5
let $x = 10  // error: 'x' is already defined in this scope
```

The `$` affects mutability, not the identifier itself. At both definition and usage sites, the prefix must match:

```ori
let $timeout = 30s
$timeout       // OK
timeout        // error: undefined variable 'timeout'
```

### Mutable Bindings (Default)

```ori
let x = 5
x = 6           // ok

let name = "Alice"
name = "Bob"    // ok
```

### Immutable Bindings (`$` prefix)

```ori
let $x = 5
$x = 6          // error: cannot assign to immutable binding '$x'

let $name = "Alice"
$name = "Bob"   // error: cannot assign to immutable binding '$name'
```

### Module-Level Bindings

All module-level bindings must be immutable (use `$` prefix). Mutable state must be contained within functions or passed as arguments.

```ori
let $timeout = 30s      // OK: immutable
pub let $api_base = "https://api.example.com"  // OK: public, immutable

let counter = 0         // error: module-level bindings must be immutable
```

Usage:
```ori
@fetch () -> Result<Data, Error> uses Http =
    timeout(op: Http.get($api_base + "/data"), after: $timeout)
```

The `$` appears at both definition and usage sites, providing visual distinction.

### Local Immutable Bindings

```ori
@process (input: int) -> int = run(
    let $base = expensive_calculation(input),

    // ... lots of code ...

    $base = 0,  // error: cannot assign to immutable binding '$base'

    $base * 2,
)
```

### Compile-Time Evaluation

Any expression can initialize a `$`-prefixed binding. The compiler evaluates expressions at compile time when possible:

```ori
let $a = 5                    // compile-time: literal
let $b = $a * 2               // compile-time: references constant
let $c = $square(x: 10)       // compile-time if $square is pure and args are const

let $d = compute()            // runtime: non-pure function (still immutable)
```

Immutability is guaranteed. Compile-time evaluation is an optimization the compiler performs when possible.

### Const Functions

The previous syntax `$name (params) -> Type = expr` is replaced by binding a lambda to a `$`-prefixed name:

```ori
// Old syntax (removed)
$square (x: int) -> int = x * x

// New syntax
let $square = (x: int) -> int = x * x
```

A function bound to a `$` name must be pure (no capabilities, no side effects). The compiler may evaluate calls to such functions at compile time when all arguments are constant:

```ori
let $square = (x: int) -> int = x * x
let $factorial = (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1)

// Evaluated at compile time
let $fact_10 = $factorial(n: 10)  // 3628800
```

If arguments are not constant, the call is evaluated at runtime but the function remains immutable.

### What Remains Always Immutable

Some bindings are always immutable regardless of `$`:

- **Function parameters** — Cannot reassign within function body
- **Loop variables** — `for item in items` — `item` cannot be reassigned

```ori
@add (a: int, b: int) -> int = run(
    a = 10,  // error: cannot assign to parameter
    a + b,
)

for item in items do
    item = other  // error: cannot assign to loop variable
```

### Destructuring

The `$` prefix applies to individual bindings in destructuring:

```ori
let { $x, y } = point      // x is immutable, y is mutable
let ($a, $b) = pair        // both immutable
let [$head, ..tail] = list // head is immutable, tail is mutable
```

### Shadowing

Shadowing follows standard rules — a new binding in an inner scope hides the outer binding. Mutability can differ between shadowed bindings:

```ori
run(
    let x = 5,          // mutable
    run(
        let $x = x * 2,  // immutable, shadows outer x
        $x,              // 10
    ),
    x,                   // still 5 (outer binding)
)
```

The `$` prefix must match between definition and usage within the same binding scope.

### Imports

When importing `$`-prefixed bindings, the `$` must be included:

```ori
// config.ori
pub let $timeout = 30s

// client.ori
use './config' { $timeout }  // OK
use './config' { timeout }   // error: 'timeout' not found (no such export)
```

This maintains consistency: the `$` appears at definition, import, and usage sites.

---

## Examples

### Configuration Module

```ori
// config.ori
pub let $api_base = "https://api.example.com"
pub let $timeout = 30s
pub let $max_retries = 3
pub let $page_size = 20
```

```ori
// client.ori
use './config' { $api_base, $timeout }

@fetch_users (page: int) -> Result<[User], Error> uses Http = try(
    let url = $api_base + "/users?page=" + str(page),
    let response = timeout(op: Http.get(url), after: $timeout)?,
    parse_users(response.body),
)
```

### Mutable Counter

```ori
@count_evens (numbers: [int]) -> int = run(
    let count = 0,
    for n in numbers do
        if n % 2 == 0 then count = count + 1 else (),
    count,
)
```

### Immutable Intermediate Result

```ori
@transform (data: Data) -> Result<Output, Error> = run(
    let $validated = validate(data),  // lock this — shouldn't change

    // ... processing ...

    Ok(finalize($validated)),
)
```

### Mathematical Constants

```ori
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
| `$square (x: int) -> int = x * x` | `let $square = (x: int) -> int = x * x` |

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
| `04-constants.md` | Rewrite for `let $name` syntax, remove `$name()` const function syntax |
| `05-variables.md` | Flip default, document `$` prefix for immutability |
| `09-expressions.md` | Update `let_expr` grammar |
| `grammar.ebnf` | Update binding and constant productions |

---

## Rationale

### Why Mutable by Default?

Ori uses ARC, not borrowing. The `mut` keyword in Rust enables mutable borrows, which is critical for memory safety. In Ori, `mut` only controlled reassignment — minor benefit, significant friction.

Higher-level languages (Python, JS, Go, Ruby) are mutable by default. Ori aims to be approachable.

### Why `$` for Immutability?

1. **Visual distinction** — `$timeout` is clearly different from `timeout`
2. **Concise** — One character vs `readonly` or `const`
3. **At usage site** — You see `$` where the value is used, not just where it's defined
4. **Unified** — Same syntax for module-level constants and local immutables

### Why Not Separate `const` Keyword?

A separate `const` keyword creates two concepts (compile-time constant vs runtime immutable) that most developers don't distinguish. JavaScript's `const` is really just "can't reassign." Ori follows this simpler mental model.

### Why Not `SCREAMING_CASE` Convention?

Conventions are not enforced by the compiler. The `$` prefix is syntactically enforced — you cannot reassign a `$`-prefixed binding.

### Why Module-Level Bindings Must Be Immutable?

Module-level mutable bindings would introduce global mutable state, which conflicts with Ori's philosophy of explicit effects and testability. All mutable state should be contained within functions or passed as arguments.

---

## Implementation

### Compiler Phases Affected

1. **Lexer** — No change (`$` already tokenized)
2. **Parser** — Update binding grammar, remove `mut` keyword, update constant syntax
3. **Name Resolution** — Track `$` modifier separately from identifier name
4. **Type Checker** — Enforce module-level immutability, check const function purity
5. **Code Generation** — No significant change (mutability is semantic, not runtime)

### Implementation Tasks

- [ ] Remove `mut` keyword from lexer/parser
- [ ] Update `let` expression parsing for `$` prefix patterns
- [ ] Update constant declaration parsing (`let $name = expr`)
- [ ] Remove old const function syntax (`$name (params) -> Type`)
- [ ] Add semantic check: module-level bindings must have `$` prefix
- [ ] Add semantic check: `$`-prefixed bindings cannot be reassigned
- [ ] Add semantic check: `$` and non-`$` same-name conflict detection
- [ ] Update error messages for immutability violations
- [ ] Update import/export handling for `$`-prefixed names

---

## Summary

| Binding | Syntax | Reassignable |
|---------|--------|--------------|
| Mutable | `let x = 5` | Yes |
| Immutable | `let $x = 5` | No |

Three keywords removed: `mut`, `const`, `readonly`

One simple rule: **`$` in the name means immutable.**

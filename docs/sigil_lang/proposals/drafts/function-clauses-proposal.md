# Proposal: Multiple Function Clauses

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-25

---

## Summary

Allow functions to be defined with multiple clauses that pattern match on arguments, enabling cleaner recursive and conditional logic.

```sigil
@factorial (0) -> int = 1
@factorial (n: int) -> int = n * factorial(n - 1)

@fib (0) -> int = 0
@fib (1) -> int = 1
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)
```

---

## Motivation

### The Problem

Currently, functions with pattern-dependent logic require explicit `match`:

```sigil
@factorial (n: int) -> int = match(n,
    0 -> 1,
    _ -> n * factorial(n - 1),
)

@describe (opt: Option<str>) -> str = match(opt,
    Some(s) -> `Value: {s}`,
    None -> "No value",
)

@len<T> (list: [T]) -> int = match(list,
    [] -> 0,
    [_, ..tail] -> 1 + len(tail),
)
```

This works but:
1. Adds nesting for simple cases
2. Obscures the base case vs recursive case structure
3. Requires repeating the function body structure

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Elixir | `def foo(0), do: 1` | Multiple `def` clauses |
| Erlang | `foo(0) -> 1;` | Semicolon-separated clauses |
| Haskell | `foo 0 = 1` | Pattern in parameter position |
| OCaml | `let rec foo = function 0 -> 1` | Match in function |
| Scala | `case` in partial functions | Different syntax |
| Rust | No multi-clause | Must use `match` |

### The Sigil Way

Multiple clauses with the same function name, each with patterns in parameter position. Clauses are tried top-to-bottom until one matches.

---

## Design

### Syntax

```
function = [ "pub" ] "@" identifier [ generics ] clause_params "->" type [ uses ] [ where ] "=" expression .
clause_params = "(" [ clause_param { "," clause_param } ] ")" .
clause_param = pattern [ ":" type ] .
```

A function can have multiple definitions. All must have:
- Same name
- Same number of parameters
- Same return type
- Same capabilities (`uses`)

### Basic Patterns

**Literal patterns:**
```sigil
@factorial (0) -> int = 1
@factorial (n: int) -> int = n * factorial(n - 1)
```

**Constructor patterns:**
```sigil
@unwrap (Some(x): Option<int>) -> int = x
@unwrap (None: Option<int>) -> int = 0
```

**List patterns:**
```sigil
@head ([first, ..]: [int]) -> int = first
@head ([]: [int]) -> int = panic("empty list")

@len ([]: [T]) -> int = 0
@len ([_, ..tail]: [T]) -> int = 1 + len(tail)
```

**Struct patterns:**
```sigil
@origin ({ x: 0, y: 0 }: Point) -> bool = true
@origin (_: Point) -> bool = false
```

### Clause Ordering

Clauses are matched top-to-bottom. More specific patterns should come first:

```sigil
// Correct: specific before general
@fib (0) -> int = 0
@fib (1) -> int = 1
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)

// Wrong: general catches everything
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)  // Always matches!
@fib (0) -> int = 0  // Never reached
@fib (1) -> int = 1  // Never reached
```

The compiler warns about unreachable clauses.

### Exhaustiveness

All clauses together must be exhaustive:

```sigil
// Error: non-exhaustive clauses
@describe (Some(x): Option<int>) -> str = str(x)
// Missing: None case

// Complete:
@describe (Some(x): Option<int>) -> str = str(x)
@describe (None: Option<int>) -> str = "none"
```

### Guards

Pattern guards using `.match()`:

```sigil
@classify (n: int).match(n < 0) -> str = "negative"
@classify (0) -> str = "zero"
@classify (n: int).match(n > 0) -> str = "positive"

@abs (n: int).match(n < 0) -> int = -n
@abs (n: int) -> int = n
```

### Multiple Parameters

```sigil
@gcd (a: int, 0) -> int = a
@gcd (a: int, b: int) -> int = gcd(b, a % b)

@zip ([], _: [U]) -> [(T, U)] = []
@zip (_: [T], []) -> [(T, U)] = []
@zip ([x, ..xs]: [T], [y, ..ys]: [U]) -> [(T, U)] = [(x, y)] + zip(xs, ys)
```

### With Named Arguments

Callers still use named arguments:

```sigil
@power (base: int, 0) -> int = 1
@power (base: int, exp: int) -> int = base * power(base: base, exp: exp - 1)

// Call site
power(base: 2, exp: 10)  // 1024
```

---

## Examples

### List Operations

```sigil
@sum ([]: [int]) -> int = 0
@sum ([x, ..xs]: [int]) -> int = x + sum(xs)

@reverse ([]: [T]) -> [T] = []
@reverse ([x, ..xs]: [T]) -> [T] = reverse(xs) + [x]

@take (0, _: [T]) -> [T] = []
@take (_: int, []: [T]) -> [T] = []
@take (n: int, [x, ..xs]: [T]) -> [T] = [x] + take(n - 1, xs)
```

### Option/Result Handling

```sigil
@unwrap_or (Some(x): Option<T>, _: T) -> T = x
@unwrap_or (None: Option<T>, default: T) -> T = default

@map_option (Some(x): Option<T>, f: (T) -> U) -> Option<U> = Some(f(x))
@map_option (None: Option<T>, _: (T) -> U) -> Option<U> = None

@and_then (Ok(x): Result<T, E>, f: (T) -> Result<U, E>) -> Result<U, E> = f(x)
@and_then (Err(e): Result<T, E>, _: (T) -> Result<U, E>) -> Result<U, E> = Err(e)
```

### Tree Traversal

```sigil
type Tree<T> = Leaf(value: T) | Branch(left: Tree<T>, right: Tree<T>)

@depth (Leaf(_): Tree<T>) -> int = 1
@depth (Branch(left, right): Tree<T>) -> int =
    1 + max(left: depth(left), right: depth(right))

@flatten (Leaf(v): Tree<T>) -> [T] = [v]
@flatten (Branch(left, right): Tree<T>) -> [T] =
    flatten(left) + flatten(right)
```

### State Machines

```sigil
type State = Idle | Running(progress: int) | Done | Error(msg: str)

@transition (Idle, "start": str) -> State = Running(progress: 0)
@transition (Running(p), "progress": str).match(p < 100) -> State = Running(progress: p + 10)
@transition (Running(p), "progress": str).match(p >= 100) -> State = Done
@transition (_, "reset": str) -> State = Idle
@transition (state: State, _: str) -> State = state  // Unknown command: no change
```

### Mathematical Functions

```sigil
@sign (n: int).match(n < 0) -> int = -1
@sign (0) -> int = 0
@sign (n: int).match(n > 0) -> int = 1

@ackermann (0, n: int) -> int = n + 1
@ackermann (m: int, 0).match(m > 0) -> int = ackermann(m - 1, 1)
@ackermann (m: int, n: int).match(m > 0 && n > 0) -> int =
    ackermann(m - 1, ackermann(m, n - 1))
```

---

## Design Rationale

### Why Not Just Use `match`?

`match` works, but multiple clauses are better when:

1. **Base cases are prominent** — Recursive functions naturally show base vs recursive cases
2. **Patterns are simple** — One pattern per clause is cleaner than nested match
3. **Functions are short** — Each clause is a simple expression

For complex logic with multiple matches on different values, `match` is still appropriate.

### Why Top-to-Bottom Matching?

Matches Sigil's explicit philosophy:
- Order matters and is visible
- No "best match" heuristics
- Predictable behavior
- Consistent with `match` arm ordering

### Why Require Exhaustiveness?

Partial functions are error-prone. If you want a partial function, use the last clause as a catch-all or return `Option`.

```sigil
// Partial (allowed with catch-all)
@head ([x, ..]: [T]) -> T = x
@head ([]: [T]) -> T = panic("empty list")

// Total (returns Option)
@safe_head ([x, ..]: [T]) -> Option<T> = Some(x)
@safe_head ([]: [T]) -> Option<T> = None
```

### Why Keep Named Arguments at Call Site?

Function clauses use positional patterns, but callers still use named arguments:

```sigil
@power (base: int, 0) -> int = 1
@power (base: int, exp: int) -> int = ...

power(base: 2, exp: 10)  // Named at call site
```

This maintains Sigil's readability at call sites while allowing concise pattern syntax in definitions.

---

## Interaction with Other Features

### With `recurse` Pattern

`recurse` is still useful for memoization and parallel recursion:

```sigil
// Using clauses (no memoization)
@fib (0) -> int = 0
@fib (1) -> int = 1
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)

// Using recurse (with memoization)
@fib_memo (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
)
```

Both are valid. Use clauses for clarity, `recurse` for memoization/parallelism.

### With Tests

Tests target the function name, covering all clauses:

```sigil
@test_factorial tests @factorial () -> void = run(
    assert_eq(actual: factorial(0), expected: 1),
    assert_eq(actual: factorial(5), expected: 120),
)
```

### With Capabilities

All clauses must have the same `uses` declaration:

```sigil
@fetch (None: Option<str>) -> str uses Http = Http.get("/default")
@fetch (Some(url): Option<str>) -> str uses Http = Http.get(url)
```

---

## Implementation Notes

### Parser Changes

Function declarations allow patterns in parameter position. Multiple declarations with the same name are grouped.

### Desugaring

Multiple clauses desugar to a single function with match:

```sigil
// Source
@factorial (0) -> int = 1
@factorial (n: int) -> int = n * factorial(n - 1)

// Desugars to
@factorial (__arg0: int) -> int = match(__arg0,
    0 -> 1,
    n -> n * factorial(n - 1),
)
```

### Exhaustiveness Checking

Reuse existing pattern exhaustiveness checker from `match`.

### Unreachable Clause Detection

Warn if a clause can never match due to earlier clauses:

```
warning: unreachable function clause
  --> src/math.si:5:1
  |
5 | @factorial (0) -> int = 1
  | ^^^^^^^^^^^^^^^^^^^^^^^^^ this clause is unreachable
  |
  = note: previous clause at line 4 matches all values
```

---

## Summary

| Feature | Syntax |
|---------|--------|
| Literal pattern | `@f (0) -> T = ...` |
| Constructor pattern | `@f (Some(x): Option<T>) -> T = ...` |
| List pattern | `@f ([x, ..xs]: [T]) -> T = ...` |
| Wildcard | `@f (_: T) -> T = ...` |
| Guard | `@f (n: int).match(n > 0) -> T = ...` |

Multiple function clauses enable pattern matching directly in function definitions, making recursive functions and conditional logic cleaner and more readable.

# Proposal: Multiple Function Clauses

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-25
**Approved:** 2026-01-28

---

## Summary

Allow functions to be defined with multiple clauses that pattern match on arguments, enabling cleaner recursive and conditional logic.

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@fib (0: int) -> int = 0
@fib (1) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)
```

---

## Motivation

### The Problem

Currently, functions with pattern-dependent logic require explicit `match`:

```ori
@factorial (n: int) -> int = match n {
    0 -> 1
    _ -> n * factorial(n - 1)
}

@describe (opt: Option<str>) -> str = match opt {
    Some(s) -> `Value: {s}`
    None -> "No value"
}

@len<T> (list: [T]) -> int = match list {
    [] -> 0
    [_, ..tail] -> 1 + len(tail)
}
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

### The Ori Way

Multiple clauses with the same function name, each with patterns in parameter position. Clauses are tried top-to-bottom until one matches.

---

## Design

### Syntax

```ebnf
function      = [ "pub" ] "@" identifier [ generics ] clause_params "->" type
                [ uses_clause ] [ where_clause ] [ guard_clause ] "=" expression .
clause_params = "(" [ clause_param { "," clause_param } ] ")" .
clause_param  = match_pattern [ ":" type ] .
guard_clause  = "if" expression .
```

A function can have multiple definitions (clauses). All clauses share:
- Same name
- Same number of parameters
- Same return type
- Same capabilities (`uses`)
- Same generics (declared on first clause only)
- Same visibility (declared on first clause only)

### First Clause Rules

The first clause establishes the function signature:
- **Visibility**: `pub` only on first clause; error if repeated
- **Generics**: Type parameters declared on first clause; in scope for all clauses
- **Type annotations**: Required on first clause parameters; optional on subsequent clauses

```ori
// First clause: full signature
pub @len<T> ([]: [T]) -> int = 0
// Subsequent: types optional, generics in scope
@len ([_, ..tail]) -> int = 1 + len(tail)
```

### Basic Patterns

**Literal patterns:**
```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)
```

**Constructor patterns:**
```ori
@unwrap<T> (Some(x): Option<T>) -> T = x
@unwrap (None) -> T = panic("called unwrap on None")
```

**List patterns:**
```ori
@head<T> ([first, ..]: [T]) -> T = first
@head ([]) -> T = panic("empty list")

@sum ([]: [int]) -> int = 0
@sum ([x, ..xs]) -> int = x + sum(xs)
```

**Struct patterns:**
```ori
@origin ({ x: 0, y: 0 }: Point) -> bool = true
@origin (_) -> bool = false
```

### Guards

Guards use `if` before `=`, consistent with `for x in items if cond` syntax:

```ori
@classify (n: int) -> str if n < 0 = "negative"
@classify (0) -> str = "zero"
@classify (n: int) -> str if n > 0 = "positive"

@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n
```

### Clause Ordering

Clauses are matched top-to-bottom. More specific patterns should come first:

```ori
// Correct: specific before general
@fib (0: int) -> int = 0
@fib (1) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)

// Wrong: general catches everything
@fib (n: int) -> int = fib(n - 1) + fib(n - 2)  // Always matches!
@fib (0) -> int = 0  // Never reached
@fib (1) -> int = 1  // Never reached
```

The compiler warns about unreachable clauses.

### Exhaustiveness

All clauses together must be exhaustive:

```ori
// Error: non-exhaustive clauses
@describe (Some(x): Option<int>) -> str = str(x)
// Missing: None case

// Complete:
@describe (Some(x): Option<int>) -> str = str(x)
@describe (None) -> str = "none"
```

### Multiple Parameters

```ori
@gcd (a: int, 0) -> int = a
@gcd (a, b) -> int = gcd(a: b, b: a % b)

@zip<T, U> ([]: [T], _: [U]) -> [(T, U)] = []
@zip (_, []) -> [(T, U)] = []
@zip ([x, ..xs], [y, ..ys]) -> [(T, U)] = [(x, y)] + zip(xs, ys)
```

### Named Arguments at Call Site

Callers use named arguments. Arguments are reordered to definition order before pattern matching:

```ori
@power (base: int, 0) -> int = 1
@power (base, exp: int) -> int = base * power(base: base, exp: exp - 1)

// Both equivalent — reordered to definition order before matching:
power(base: 2, exp: 10)  // (base=2, exp=10)
power(exp: 10, base: 2)  // reordered to (base=2, exp=10)
```

### Default Parameters

Default parameter values are filled in before pattern matching:

```ori
@connect (host: str, 443) -> Connection = secure_connect(host)
@connect (host, port: int = 80) -> Connection = plain_connect(host, port)

connect(host: "example.com")             // port=80 (default), matches second
connect(host: "example.com", port: 443)  // matches first (literal 443)
connect(host: "example.com", port: 8080) // matches second (8080 ≠ 443)
```

---

## Examples

### List Operations

```ori
@sum ([]: [int]) -> int = 0
@sum ([x, ..xs]) -> int = x + sum(xs)

@reverse<T> ([]: [T]) -> [T] = []
@reverse ([x, ..xs]) -> [T] = reverse(xs) + [x]

@take<T> (0: int, _: [T]) -> [T] = []
@take (_, []) -> [T] = []
@take (n, [x, ..xs]) -> [T] = [x] + take(n - 1, xs)
```

### Option/Result Handling

```ori
@unwrap_or<T> (Some(x): Option<T>, _: T) -> T = x
@unwrap_or (None, default) -> T = default

@map_option<T, U> (Some(x): Option<T>, f: (T) -> U) -> Option<U> = Some(f(x))
@map_option (None, _) -> Option<U> = None

@and_then<T, U, E> (Ok(x): Result<T, E>, f: (T) -> Result<U, E>) -> Result<U, E> = f(x)
@and_then (Err(e), _) -> Result<U, E> = Err(e)
```

### Tree Traversal

```ori
type Tree<T> = Leaf(value: T) | Branch(left: Tree<T>, right: Tree<T>)

@depth<T> (Leaf(_): Tree<T>) -> int = 1
@depth (Branch(left, right)) -> int =
    1 + max(left: depth(left), right: depth(right))

@flatten<T> (Leaf(v): Tree<T>) -> [T] = [v]
@flatten (Branch(left, right)) -> [T] =
    flatten(left) + flatten(right)
```

### State Machines

```ori
type State = Idle | Running(progress: int) | Done | Error(msg: str)

@transition (Idle, "start": str) -> State = Running(progress: 0)
@transition (Running(p), "progress") -> State if p < 100 = Running(progress: p + 10)
@transition (Running(p), "progress") -> State if p >= 100 = Done
@transition (_, "reset") -> State = Idle
@transition (state, _) -> State = state  // Unknown command: no change
```

### Mathematical Functions

```ori
@sign (n: int) -> int if n < 0 = -1
@sign (0) -> int = 0
@sign (n: int) -> int if n > 0 = 1

@ackermann (0: int, n: int) -> int = n + 1
@ackermann (m, 0) -> int if m > 0 = ackermann(m - 1, 1)
@ackermann (m, n) -> int if m > 0 && n > 0 =
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

Matches Ori's explicit philosophy:
- Order matters and is visible
- No "best match" heuristics
- Predictable behavior
- Consistent with `match` arm ordering

### Why Require Exhaustiveness?

Partial functions are error-prone. If you want a partial function, use the last clause as a catch-all or return `Option`.

```ori
// Partial (allowed with catch-all)
@head<T> ([x, ..]: [T]) -> T = x
@head ([]) -> T = panic("empty list")

// Total (returns Option)
@safe_head<T> ([x, ..]: [T]) -> Option<T> = Some(x)
@safe_head ([]) -> Option<T> = None
```

### Why `if` for Guards?

The `if` guard syntax mirrors existing `for x in items if cond` syntax in Ori:

```ori
// for loop with guard
for x in items if x > 0 yield x * 2

// function clause with guard
@abs (n: int) -> int if n < 0 = -n
```

Both read naturally: "for x if condition" / "abs of n if condition".

### Why First Clause Establishes Signature?

Reduces repetition while maintaining clarity:
- Visibility, generics, and types declared once
- Subsequent clauses focus on patterns
- Consistent with "define once, use many" principle

---

## Interaction with Other Features

### With `recurse` Pattern

`recurse` is still useful for memoization and parallel recursion:

```ori
// Using clauses (no memoization)
@fib (0: int) -> int = 0
@fib (1) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)

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

```ori
@test_factorial tests @factorial () -> void = {
    assert_eq(actual: factorial(0), expected: 1)
    assert_eq(actual: factorial(5), expected: 120)
}
```

### With Capabilities

All clauses must have the same `uses` declaration (on first clause):

```ori
@fetch (None: Option<str>) -> str uses Http = Http.get("/default")
@fetch (Some(url)) -> str = Http.get(url)
```

---

## Implementation Notes

### Parser Changes

- Function declarations allow `match_pattern` in parameter position
- Multiple declarations with same name are grouped into single function
- First clause parsed with full signature; subsequent validated for consistency
- `if` guard parsed between `where_clause` and `=`

### Desugaring

Multiple clauses desugar to a single function with match:

```ori
// Source
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

// Desugars to
@factorial (__arg0: int) -> int = match __arg0 {
    0 -> 1
    n -> n * factorial(n - 1)
}
```

With guards:

```ori
// Source
@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n

// Desugars to
@abs (__arg0: int) -> int = match __arg0 {
    n.match(n < 0) -> -n
    n -> n
}
```

### Exhaustiveness Checking

Reuse existing pattern exhaustiveness checker from `match`.

### Unreachable Clause Detection

Warn if a clause can never match due to earlier clauses:

```
warning: unreachable function clause
  --> src/math.ori:5:1
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
| Literal pattern | `@f (0: int) -> T = ...` |
| Constructor pattern | `@f (Some(x): Option<T>) -> T = ...` |
| List pattern | `@f ([x, ..xs]: [T]) -> T = ...` |
| Wildcard | `@f (_: T) -> T = ...` |
| Guard | `@f (n: int) -> T if n > 0 = ...` |
| Type inference | `@f (n) -> T = ...` (after first clause) |

Multiple function clauses enable pattern matching directly in function definitions, making recursive functions and conditional logic cleaner and more readable.

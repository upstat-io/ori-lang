---
title: "Declarations"
description: "Ori Language Specification — Declarations"
order: 8
section: "Declarations"
---

# Declarations

Functions, types, traits, and implementations.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS

## Functions

```ori
@add (a: int, b: int) -> int = a + b

pub @identity<T> (x: T) -> T = x

@sort<T: Comparable> (items: [T]) -> [T] = ...

@fetch (url: str) -> Result<str, Error> uses Http = Http.get(url)
```

- `@` prefix required
- Return type required (`void` for no value)
- Parameters are immutable
- Private by default; `pub` exports
- `uses` declares capability dependencies

### Multiple Clauses

A function may have multiple definitions (clauses) with patterns in parameter position:

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@fib (0: int) -> int = 0
@fib (1) -> int = 1
@fib (n) -> int = fib(n - 1) + fib(n - 2)
```

Clauses are matched top-to-bottom. All clauses must have:
- Same name
- Same number of parameters
- Same return type
- Same capabilities

The first clause establishes the function signature:
- **Visibility**: `pub` only on first clause
- **Generics**: Type parameters declared on first clause; in scope for all clauses
- **Type annotations**: Required on first clause parameters; optional on subsequent clauses

```ori
pub @len<T> ([]: [T]) -> int = 0
@len ([_, ..tail]) -> int = 1 + len(tail)
```

Guards use `if` before `=`:

```ori
@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n
```

All clauses together must be exhaustive. The compiler warns about unreachable clauses.

### Default Parameter Values

Parameters may specify default values:

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

@connect (host: str, port: int = 8080, timeout: Duration = 30s) -> Connection
```

- Callers may omit parameters with defaults
- Named arguments allow any defaulted parameter to be omitted, not just trailing ones
- Default expressions are evaluated at call time, not definition time
- Default expressions must not reference other parameters

```ori
greet()                        // "Hello, World!"
greet(name: "Alice")           // "Hello, Alice!"
connect(host: "localhost")     // uses default port and timeout
connect(host: "localhost", timeout: 60s)  // override timeout only
```

See [Expressions § Function Call](09-expressions.md#function-call) for call semantics.

## Types

```ori
type Point = { x: int, y: int }

type Status = Pending | Running | Done | Failed(reason: str)

type UserId = int

#derive(Eq, Clone)
type User = { id: int, name: str }
```

## Traits

```ori
trait Printable {
    @to_str (self) -> str
}

trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}

trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}
```

- `self` — instance
- `Self` — implementing type

## Implementations

```ori
impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }
}

impl Printable for Point {
    @to_str (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
}

impl<T: Printable> Printable for [T] {
    @to_str (self) -> str = ...
}
```

## Tests

```ori
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)
```

See [Testing](13-testing.md).

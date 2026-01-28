---
title: "Declarations"
description: "Ori Language Specification — Declarations"
order: 8
---

# Declarations

Functions, types, traits, and implementations.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § DECLARATIONS

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

#[derive(Eq, Clone)]
type User = { id: int, name: str }
```

## Traits

```ori
trait Printable {
    @to_string (self) -> str
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
    @to_string (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
}

impl<T: Printable> Printable for [T] {
    @to_string (self) -> str = ...
}
```

## Tests

```ori
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 2, b: 3), expected: 5),
)
```

See [Testing](13-testing.md).

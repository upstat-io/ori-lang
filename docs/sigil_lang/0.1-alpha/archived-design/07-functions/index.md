# Functions

This section covers Sigil's function system: definitions, first-class functions, lambdas, and closures.

---

## Documents

| Document | Description |
|----------|-------------|
| [Function Definitions](01-function-definitions.md) | @name syntax, parameters, return types |
| [First-Class Functions](02-first-class-functions.md) | Functions as values |
| [Lambdas](03-lambdas.md) | Anonymous functions, closures |
| [Higher-Order Functions](04-higher-order.md) | Functions that take/return functions |

---

## Overview

Functions are first-class values in Sigil:

```sigil
// Definition
@double (number: int) -> int = number * 2

// Reference (without calling)
transform = double

// Call via variable
// 10
result = transform(5)

// Pass to higher-order pattern
// [2, 4, 6]
doubled = map(
    .over: [1, 2, 3],
    .transform: double,
)
```

### Function Syntax

```sigil
// Basic function
@add (left: int, right: int) -> int = left + right

// Generic function
@identity<T> (value: T) -> T = value

// Public function
pub @multiply (left: int, right: int) -> int = left * right
```

### Function Types

```sigil
type Transform = (int) -> int
type Predicate = (int) -> bool
type BinaryOp = (int, int) -> int
type Curried = (int) -> (int) -> int
```

### Lambdas and Closures

```sigil
// Lambda syntax
squared = map(
    .over: [1, 2, 3],
    .transform: item -> item * item,
)

// Closure captures values
@make_adder (amount: int) -> (int) -> int =
    // amount is captured
    value -> value + amount

add5 = make_adder(5)
// 15
add5(10)
```

---

## See Also

- [Main Index](../00-index.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Type System](../03-type-system/index.md)

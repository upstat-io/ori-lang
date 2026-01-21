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
@double (n: int) -> int = n * 2

// Reference (without calling)
f = double

// Call via variable
result = f(5)  // 10

// Pass to higher-order function
doubled = map([1, 2, 3], double)  // [2, 4, 6]
```

### Function Syntax

```sigil
// Basic function
@add (a: int, b: int) -> int = a + b

// Generic function
@identity<T> (x: T) -> T = x

// Public function
pub @multiply (a: int, b: int) -> int = a * b
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
squared = map([1, 2, 3], x -> x * x)

// Closure captures values
@make_adder (n: int) -> (int) -> int =
    x -> x + n  // n is captured

add5 = make_adder(5)
add5(10)  // 15
```

---

## See Also

- [Main Index](../00-index.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Type System](../03-type-system/index.md)

# Basic Syntax

This document covers Sigil's fundamental syntax: functions, config variables, comments, and basic types.

---

## Functions

Functions are defined with the `@` prefix:

```sigil
@function_name (param: type, ...) -> return_type = expression
```

### Examples

```sigil
// Simple function
@add (left: int, right: int) -> int = left + right

// No parameters
@get_pi () -> float = 3.14159

// Void return
@greet (name: str) -> void = print(
    .message: "Hello, " + name,
)

// Using patterns
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1),
)
```

### Public Functions

Use `pub` to export from a module:

```sigil
pub @add (left: int, right: int) -> int = left + right
```

### Generic Functions

Type parameters use angle brackets:

```sigil
@identity<T> (value: T) -> T = value

@transform<T, U> (items: [T], transformer: (T) -> U) -> [U] = ...
```

---

## Config Variables

Config variables use the `$` prefix:

```sigil
$config_name = value
```

### Examples

```sigil
$max_retries = 3
$timeout = 30s
$api_base = "https://api.example.com"
$debug_mode = false
```

### With Units

Unit suffixes create `Duration` and `Size` typed values:

```sigil
// Duration literals (type: Duration)
// 30 seconds
$timeout = 30s
// 5 minutes
$cache_ttl = 5m
// 2 hours
$max_wait = 2h
// 100 milliseconds
$delay = 100ms

// Size literals (type: Size)
// 1 kilobyte
$buffer_size = 1kb
// 10 megabytes
$max_upload = 10mb
```

See [Primitive Types](../03-type-system/01-primitive-types.md) for full details on Duration and Size.

### Usage

Reference config values with the `$` prefix:

```sigil
@fetch (url: str) -> Result<Data, Error> = retry(
    .operation: http_get(
        .url: url,
    ),
    .attempts: $max_retries,
    .timeout: $timeout,
)
```

### Public Config

```sigil
pub $default_timeout = 30s
```

---

## Comments

Single-line comments use `//`. Comments must appear on their own line; inline comments (comments following code on the same line) are not allowed.

```sigil
// This is a comment
@add (left: int, right: int) -> int = left + right
```

### Documentation Comments

See [Doc Comments](../13-documentation/01-doc-comments.md) for documentation syntax.

```sigil
// #Adds two integers
// @param left first operand
// @param right second operand
@add (left: int, right: int) -> int = left + right
```

---

## Basic Types

### Primitives

| Type | Description | Examples |
|------|-------------|----------|
| `int` | 64-bit signed integer | `42`, `-1`, `0` |
| `float` | 64-bit floating point | `3.14`, `-0.5` |
| `bool` | Boolean | `true`, `false` |
| `str` | UTF-8 string | `"hello"`, `""` |
| `void` | No value | (for side-effect functions) |

### Collections

| Type | Description | Examples |
|------|-------------|----------|
| `[T]` | List of T | `[1, 2, 3]`, `["a", "b"]` |
| `{K: V}` | Map from K to V | `{"a": 1, "b": 2}` |

### Tuples

| Type | Description | Examples |
|------|-------------|----------|
| `(T, U)` | Pair | `(1, "hello")` |
| `(T, U, V)` | Triple | `(1, "a", true)` |

### Optional and Result

| Type | Description |
|------|-------------|
| `Option<T>` | Value or nothing (Some/None) |
| `Result<T, E>` | Success or error (Ok/Err) |

---

## String Literals

### Basic Strings

```sigil
let name = "Alice"
let empty = ""
let with_quotes = "She said \"hello\""
```

### Escape Sequences

| Escape | Meaning |
|--------|---------|
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | Newline |
| `\t` | Tab |
| `\r` | Carriage return |

### String Interpolation

String concatenation uses `+`:

```sigil
let greeting = "Hello, " + name + "!"
```

---

## List Literals

```sigil
let numbers = [1, 2, 3, 4, 5]
let names = ["Alice", "Bob", "Carol"]
let empty = []
let nested = [[1, 2], [3, 4]]
```

### List Operations

```sigil
// First element
let first = numbers[0]
// Last element (# = length)
let last = numbers[# - 1]
// Middle element
let middle = numbers[# / 2]
```

---

## Map Literals

```sigil
let ages = {"Alice": 30, "Bob": 25}
let empty = {}
```

### Map Operations

```sigil
// returns Option, use ?? for default
let alice_age = ages["Alice"] ?? 0
let has_alice = ages.has(
    .key: "Alice",
)
```

---

## Type Annotations

### Variable Bindings

```sigil
@process () -> int = run(
    // explicit type
    let x: int = 5,
    // inferred
    let y = 10,
    let result = x + y,
    result,
)
```

### Function Types

```sigil
type Transform = (int) -> int
type Predicate = (int) -> bool
type BinaryOp = (int, int) -> int
```

---

## Tests

Tests use the `tests` keyword:

```sigil
@test_name tests @target_function () -> void = run(
    assert(
        .condition: some_condition,
    ),
    assert_eq(
        .actual: actual_value,
        .expected: expected_value,
    ),
)
```

### Example

```sigil
@add (left: int, right: int) -> int = left + right

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(
            .left: 2,
            .right: 3,
        ),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(
            .left: -1,
            .right: 1,
        ),
        .expected: 0,
    ),
    assert_eq(
        .actual: add(
            .left: 0,
            .right: 0,
        ),
        .expected: 0,
    ),
)
```

---

## Imports

```sigil
use module { item1, item2 }
use module { item as alias }
use path.to.module
use path.to.module as alias
```

### Examples

```sigil
use std.math { sqrt, abs, pow }
use std.string { split, join }
use http.client as http
```

Imports must be at the top of the file.

---

## See Also

- [Expressions](02-expressions.md)
- [Patterns Overview](03-patterns-overview.md)
- [Type System](../03-type-system/index.md)

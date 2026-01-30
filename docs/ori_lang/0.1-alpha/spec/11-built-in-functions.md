---
title: "Built-in Functions"
description: "Ori Language Specification — Built-in Functions"
order: 11
section: "Expressions"
---

# Built-in Functions

Core functions provided by the language. All built-in functions require named arguments, except type conversions.

## Reserved Names

Built-in names cannot be used for function definitions. Reserved in call position only; may be used as variables.

```ori
let min = 5           // OK: variable
min(left: a, right: b) // OK: calls built-in
@min (...) = ...      // error: reserved name
```

## Type Conversions (function_val)

Type conversions are the sole exception to named argument requirements. Positional syntax is allowed.

| Function | From | Behavior |
|----------|------|----------|
| `int(x)` | `float` | Truncates toward zero |
| | `str` | Parses decimal; panics on invalid |
| | `bool` | `true`→1, `false`→0 |
| | `byte` | Zero-extends |
| `float(x)` | `int` | Exact (may lose precision) |
| | `str` | Parses; panics on invalid |
| `str(x)` | `int`, `float`, `bool` | Decimal representation |
| `byte(x)` | `int` | Truncates to 8 bits |
| | `str` | First UTF-8 byte |

## Collection Functions

```
len(collection: T) -> int
is_empty(collection: T) -> bool
```

Works on `[T]`, `{K: V}`, `str`. For strings, returns code point count.

## Option Functions

```
is_some(opt: Option<T>) -> bool
is_none(opt: Option<T>) -> bool
```

### Option Methods

```
Option<T>.map(transform: T -> U) -> Option<U>
Option<T>.unwrap_or(default: T) -> T
Option<T>.ok_or(err: E) -> Result<T, E>
Option<T>.and_then(then: T -> Option<U>) -> Option<U>
Option<T>.filter(predicate: T -> bool) -> Option<T>
```

## Result Functions

```
is_ok(result: Result<T, E>) -> bool
is_err(result: Result<T, E>) -> bool
```

### Result Methods

```
Result<T, E>.map(transform: T -> U) -> Result<U, E>
Result<T, E>.map_err(transform: E -> F) -> Result<T, F>
Result<T, E>.unwrap_or(default: T) -> T
Result<T, E>.ok() -> Option<T>
Result<T, E>.err() -> Option<E>
Result<T, E>.and_then(then: T -> Result<U, E>) -> Result<U, E>
```

## Assertions

```
assert(condition: bool) -> void
assert_eq(actual: T, expected: T) -> void
assert_ne(actual: T, unexpected: T) -> void
assert_some(opt: Option<T>) -> void
assert_none(opt: Option<T>) -> void
assert_ok(result: Result<T, E>) -> void
assert_err(result: Result<T, E>) -> void
assert_panics(expr: T) -> void
assert_panics_with(expr: T, message: str) -> void
```

Panics on failure.

## Comparison

```
compare(left: T, right: T) -> Ordering where T: Comparable
min(left: T, right: T) -> T where T: Comparable
max(left: T, right: T) -> T where T: Comparable
```

## I/O

```
print(msg: str) -> void
```

## Control

```
panic(msg: str) -> Never
```

## Prelude

Available without import:
- `int`, `float`, `str`, `byte`
- `len`, `is_empty`
- `is_some`, `is_none`, `Some`, `None`
- `is_ok`, `is_err`, `Ok`, `Err`
- All assertions
- `print`, `panic`, `compare`, `min`, `max`

## Collection Methods

Data transformation via method calls on collections.

### List Methods

```
[T].map(transform: T -> U) -> [U]
[T].filter(predicate: T -> bool) -> [T]
[T].fold(initial: U, op: (U, T) -> U) -> U
[T].find(where: T -> bool) -> Option<T>
[T].any(predicate: T -> bool) -> bool
[T].all(predicate: T -> bool) -> bool
[T].first() -> Option<T>
[T].last() -> Option<T>
[T].take(n: int) -> [T]
[T].skip(n: int) -> [T]
[T].reverse() -> [T]
[T].sort() -> [T] where T: Comparable
[T].contains(value: T) -> bool where T: Eq
```

### Range Methods

```
Range<T>.map(transform: T -> U) -> [U]
Range<T>.filter(predicate: T -> bool) -> [T]
Range<T>.fold(initial: U, op: (U, T) -> U) -> U
Range<T>.collect() -> [T]
Range<T>.contains(value: T) -> bool
```

### String Methods

```
str.split(sep: str) -> [str]
str.trim() -> str
str.upper() -> str
str.lower() -> str
str.starts_with(prefix: str) -> bool
str.ends_with(suffix: str) -> bool
str.contains(substr: str) -> bool
```

## Standard Library

| Module | Contents |
|--------|----------|
| `std.math` | `sqrt`, `abs`, `pow`, `sin`, `cos`, `tan` |
| `std.string` | `split`, `join`, `trim`, `upper`, `lower` |
| `std.list` | `sort`, `reverse`, `unique` |
| `std.io` | File I/O |
| `std.net` | Network |
| `std.resilience` | `retry`, `exponential`, `linear` |
| `std.validate` | `validate` |

### std.resilience

```ori
use std.resilience { retry, exponential, linear }

retry(op: fetch(url), attempts: 3, backoff: exponential(base: 100ms))
retry(op: fetch(url), attempts: 5, backoff: linear(delay: 100ms))
```

### std.validate

```ori
use std.validate { validate }

validate(
    rules: [
        age >= 0 | "age must be non-negative",
        name != "" | "name required",
    ],
    value: User { name, age },
)
```

Returns `Result<T, [str]>`.

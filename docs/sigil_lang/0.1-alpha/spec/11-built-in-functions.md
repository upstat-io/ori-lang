# Built-in Functions

This section defines the core functions provided by the language. Built-in functions are function_exp constructs and use named argument syntax, with the exception of type conversion functions which allow positional syntax.

## Type Conversion Functions

Type conversion functions are the only built-in functions that allow positional argument syntax.

### int

Convert a value to `int`.

```
@int (value: T) -> int
```

| Input Type | Behavior |
|------------|----------|
| `float` | Truncates toward zero |
| `str` | Parses as decimal integer; panics on invalid |
| `bool` | `true` → 1, `false` → 0 |
| `byte` | Zero-extends to int |

```sigil
// Returns 3
int(3.7)

// Returns -3
int(-3.7)

// Returns 42
int("42")

// Returns 1
int(true)
```

It is a run-time error if a string cannot be parsed as an integer.

### float

Convert a value to `float`.

```
@float (value: T) -> float
```

| Input Type | Behavior |
|------------|----------|
| `int` | Exact conversion (may lose precision for large values) |
| `str` | Parses as floating-point; panics on invalid |

```sigil
// Returns 42.0
float(42)

// Returns 3.14
float("3.14")
```

### str

Convert a value to `str`.

```
@str (value: T) -> str
```

| Input Type | Behavior |
|------------|----------|
| `int` | Decimal representation |
| `float` | Decimal representation |
| `bool` | "true" or "false" |

```sigil
// Returns "42"
str(42)

// Returns "3.14"
str(3.14)

// Returns "true"
str(true)
```

### byte

Convert a value to `byte`.

```
@byte (value: T) -> byte
```

| Input Type | Behavior |
|------------|----------|
| `int` | Truncates to least significant 8 bits |
| `str` | First byte of UTF-8 encoding |

## Collection Functions

### len

Return the length of a collection.

```
@len (.collection: T) -> int
```

| Input Type | Result |
|------------|--------|
| `[T]` | Number of elements |
| `{K: V}` | Number of entries |
| `str` | Number of Unicode code points |

```sigil
// Returns 3
len(
    .collection: [1, 2, 3],
)

// Returns 1
len(
    .collection: {"a": 1},
)

// Returns 5
len(
    .collection: "hello",
)
```

### is_empty

Test if a collection is empty.

```
@is_empty (.collection: T) -> bool
```

Equivalent to `len(.collection: collection) == 0`.

```sigil
// Returns true
is_empty(
    .collection: [],
)

// Returns false
is_empty(
    .collection: [1, 2],
)
```

## Option Functions

### is_some

Test if an `Option` contains a value.

```
@is_some (.opt: Option<T>) -> bool
```

```sigil
// Returns true
is_some(
    .opt: Some(42),
)

// Returns false
is_some(
    .opt: None,
)
```

### is_none

Test if an `Option` is empty.

```
@is_none (.opt: Option<T>) -> bool
```

```sigil
// Returns true
is_none(
    .opt: None,
)

// Returns false
is_none(
    .opt: Some(42),
)
```

## Option Methods

Methods available on `Option<T>` values.

### map

Transform the value if present.

```
Option<T>.map(.transform: T -> U) -> Option<U>
```

```sigil
// Returns Some(4)
Some(2).map(
    .transform: x -> x * 2,
)

// Returns None
None.map(
    .transform: x -> x * 2,
)
```

### unwrap_or

Return the value if present, or a default.

```
Option<T>.unwrap_or(.default: T) -> T
```

```sigil
// Returns 42
Some(42).unwrap_or(
    .default: 0,
)

// Returns 0
None.unwrap_or(
    .default: 0,
)
```

### ok_or

Convert to `Result<T, E>`, using the provided error if `None`.

```
Option<T>.ok_or(.err: E) -> Result<T, E>
```

```sigil
// Returns Ok(42)
Some(42).ok_or(
    .err: "missing",
)

// Returns Err("missing")
None.ok_or(
    .err: "missing",
)
```

### and_then

Chain operations that may return `None`.

```
Option<T>.and_then(.then: T -> Option<U>) -> Option<U>
```

```sigil
// Returns Some(4)
Some(2).and_then(
    .then: x -> Some(x * 2),
)

// Returns None
Some(2).and_then(
    .then: x -> None,
)

// Returns None
None.and_then(
    .then: x -> Some(x * 2),
)
```

### filter

Keep the value only if it satisfies a predicate.

```
Option<T>.filter(.predicate: T -> bool) -> Option<T>
```

```sigil
// Returns Some(4)
Some(4).filter(
    .predicate: x -> x > 0,
)

// Returns None
Some(-1).filter(
    .predicate: x -> x > 0,
)

// Returns None
None.filter(
    .predicate: x -> x > 0,
)
```

## Result Functions

### is_ok

Test if a `Result` is successful.

```
@is_ok (.result: Result<T, E>) -> bool
```

```sigil
// Returns true
is_ok(
    .result: Ok(42),
)

// Returns false
is_ok(
    .result: Err("fail"),
)
```

### is_err

Test if a `Result` is an error.

```
@is_err (.result: Result<T, E>) -> bool
```

```sigil
// Returns true
is_err(
    .result: Err("fail"),
)

// Returns false
is_err(
    .result: Ok(42),
)
```

## Result Methods

Methods available on `Result<T, E>` values.

### map

Transform the success value, leaving errors unchanged.

```
Result<T, E>.map(.transform: T -> U) -> Result<U, E>
```

```sigil
// Returns Ok(4)
Ok(2).map(
    .transform: x -> x * 2,
)

// Returns Err("fail")
Err("fail").map(
    .transform: x -> x * 2,
)
```

### map_err

Transform the error value, leaving successes unchanged.

```
Result<T, E>.map_err(.transform: E -> F) -> Result<T, F>
```

```sigil
// Returns Err(AppError.Parse("fail"))
Err("fail").map_err(
    .transform: e -> AppError.Parse(e),
)

// Returns Ok(42)
Ok(42).map_err(
    .transform: e -> AppError.Parse(e),
)
```

Use `.map_err()` with `?` to convert and propagate errors:

```sigil
let content = read_file(
    .path: path,
).map_err(
    .transform: e -> AppError.Io(e),
)?
```

### unwrap_or

Return the success value, or a default if error.

```
Result<T, E>.unwrap_or(.default: T) -> T
```

```sigil
// Returns 42
Ok(42).unwrap_or(
    .default: 0,
)

// Returns 0
Err("fail").unwrap_or(
    .default: 0,
)
```

### ok

Convert to `Option<T>`, discarding error information.

```
Result<T, E>.ok() -> Option<T>
```

```sigil
// Returns Some(42)
Ok(42).ok()

// Returns None
Err("fail").ok()
```

### err

Convert to `Option<E>`, discarding success value.

```
Result<T, E>.err() -> Option<E>
```

```sigil
// Returns Some("fail")
Err("fail").err()

// Returns None
Ok(42).err()
```

### and_then

Chain operations that may fail.

```
Result<T, E>.and_then(.then: T -> Result<U, E>) -> Result<U, E>
```

```sigil
// Returns Ok(4)
Ok(2).and_then(
    .then: x -> Ok(x * 2),
)

// Returns Err("fail")
Ok(2).and_then(
    .then: x -> Err("fail"),
)

// Returns Err("fail")
Err("fail").and_then(
    .then: x -> Ok(x * 2),
)
```

## Assertion Functions

### assert

Assert that a condition is true.

```
@assert (.cond: bool) -> void
```

If `cond` is false, the program panics with an assertion error.

```sigil
assert(
    .cond: x > 0,
)

assert(
    .cond: list.len() > 0,
)
```

### assert_eq

Assert that two values are equal.

```
@assert_eq (.actual: T, .expected: T) -> void
```

If `actual != expected`, the program panics with a diagnostic showing both values.

```sigil
assert_eq(
    .actual: add(
        .a: 2,
        .b: 3,
    ),
    .expected: 5,
)
assert_eq(
    .actual: result,
    .expected: expected,
)
```

### assert_ne

Assert that two values are not equal.

```
@assert_ne (.actual: T, .unexpected: T) -> void
```

If `actual == unexpected`, the program panics with a diagnostic showing both values.

### assert_some

Assert that an `Option` is `Some`.

```
@assert_some (.opt: Option<T>) -> void
```

### assert_none

Assert that an `Option` is `None`.

```
@assert_none (.opt: Option<T>) -> void
```

### assert_ok

Assert that a `Result` is `Ok`.

```
@assert_ok (.result: Result<T, E>) -> void
```

### assert_err

Assert that a `Result` is `Err`.

```
@assert_err (.result: Result<T, E>) -> void
```

### assert_panics

Assert that evaluating an expression panics.

```
@assert_panics (.expr: T) -> void
```

The argument expression is evaluated and must panic; otherwise the assertion fails.

### assert_panics_with

Assert that evaluating an expression panics with a specific message.

```
@assert_panics_with (.expr: T, .message: str) -> void
```

The argument expression is evaluated and must panic with an error message equal to `message`.

## I/O Functions

### print

Print a message to standard output.

```
@print (.msg: str) -> void
```

```sigil
print(
    .msg: "Hello, World!",
)

print(
    .msg: "Value: " + str(x),
)
```

## Comparison Functions

### compare

Compare two values.

```
@compare (.left: T, .right: T) -> Ordering where T: Comparable
```

Returns `Less`, `Equal`, or `Greater`.

### min

Return the minimum of two values.

```
@min (.left: T, .right: T) -> T where T: Comparable
```

### max

Return the maximum of two values.

```
@max (.left: T, .right: T) -> T where T: Comparable
```

## Panic

### panic

Terminate execution with an error message.

```
@panic (.msg: str) -> Never
```

The return type `Never` indicates that this function never returns normally.

```sigil
panic(
    .msg: "Unexpected state",
)
```

## Prelude

The following functions are available without import (prelude):

- Type conversions: `int`, `float`, `str`, `byte`
- Collection: `len`, `is_empty`
- Option: `is_some`, `is_none`, `Some`, `None`
- Result: `is_ok`, `is_err`, `Ok`, `Err`
- Assertion: `assert`, `assert_eq`, `assert_ne`, `assert_some`, `assert_none`, `assert_ok`, `assert_err`, `assert_panics`, `assert_panics_with`
- I/O: `print`
- Control: `panic`

Additional functions are available via `use std { ... }`.

## Standard Library Modules

The standard library provides additional functions in modules:

| Module | Contents |
|--------|----------|
| `std.math` | `sqrt`, `abs`, `pow`, `sin`, `cos`, `tan`, etc. |
| `std.string` | `split`, `join`, `trim`, `upper`, `lower`, etc. |
| `std.list` | `sort`, `reverse`, `unique`, etc. |
| `std.io` | File I/O functions |
| `std.net` | Network functions |

See the standard library documentation for complete reference.

# Built-in Functions

This section defines the core functions provided by the language.

## Type Conversion Functions

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
int(3.7)      // 3
int(-3.7)     // -3
int("42")     // 42
int(true)     // 1
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
float(42)       // 42.0
float("3.14")   // 3.14
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
str(42)       // "42"
str(3.14)     // "3.14"
str(true)     // "true"
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
@len (collection: T) -> int
```

| Input Type | Result |
|------------|--------|
| `[T]` | Number of elements |
| `{K: V}` | Number of entries |
| `str` | Number of Unicode code points |

```sigil
len([1, 2, 3])     // 3
len({"a": 1})      // 1
len("hello")       // 5
```

### is_empty

Test if a collection is empty.

```
@is_empty (collection: T) -> bool
```

Equivalent to `len(collection) == 0`.

## Option Functions

### is_some

Test if an `Option` contains a value.

```
@is_some (opt: Option<T>) -> bool
```

```sigil
is_some(Some(42))   // true
is_some(None)       // false
```

### is_none

Test if an `Option` is empty.

```
@is_none (opt: Option<T>) -> bool
```

```sigil
is_none(None)       // true
is_none(Some(42))   // false
```

## Option Methods

Methods available on `Option<T>` values.

### map

Transform the value if present.

```
Option<T>.map(T -> U) -> Option<U>
```

```sigil
Some(2).map(x -> x * 2)  // Some(4)
None.map(x -> x * 2)     // None
```

### unwrap_or

Return the value if present, or a default.

```
Option<T>.unwrap_or(T) -> T
```

```sigil
Some(42).unwrap_or(0)  // 42
None.unwrap_or(0)      // 0
```

### ok_or

Convert to `Result<T, E>`, using the provided error if `None`.

```
Option<T>.ok_or(E) -> Result<T, E>
```

```sigil
Some(42).ok_or("missing")  // Ok(42)
None.ok_or("missing")      // Err("missing")
```

### and_then

Chain operations that may return `None`.

```
Option<T>.and_then(T -> Option<U>) -> Option<U>
```

```sigil
Some(2).and_then(x -> Some(x * 2))  // Some(4)
Some(2).and_then(x -> None)         // None
None.and_then(x -> Some(x * 2))     // None
```

### filter

Keep the value only if it satisfies a predicate.

```
Option<T>.filter(T -> bool) -> Option<T>
```

```sigil
Some(4).filter(x -> x > 0)   // Some(4)
Some(-1).filter(x -> x > 0)  // None
None.filter(x -> x > 0)      // None
```

## Result Functions

### is_ok

Test if a `Result` is successful.

```
@is_ok (result: Result<T, E>) -> bool
```

```sigil
is_ok(Ok(42))        // true
is_ok(Err("fail"))   // false
```

### is_err

Test if a `Result` is an error.

```
@is_err (result: Result<T, E>) -> bool
```

```sigil
is_err(Err("fail"))  // true
is_err(Ok(42))       // false
```

## Result Methods

Methods available on `Result<T, E>` values.

### map

Transform the success value, leaving errors unchanged.

```
Result<T, E>.map(T -> U) -> Result<U, E>
```

```sigil
Ok(2).map(x -> x * 2)       // Ok(4)
Err("fail").map(x -> x * 2) // Err("fail")
```

### map_err

Transform the error value, leaving successes unchanged.

```
Result<T, E>.map_err(E -> F) -> Result<T, F>
```

```sigil
Err("fail").map_err(e -> AppError.Parse(e))  // Err(AppError.Parse("fail"))
Ok(42).map_err(e -> AppError.Parse(e))       // Ok(42)
```

Use `.map_err()` with `?` to convert and propagate errors:

```sigil
let content = read_file(path).map_err(e -> AppError.Io(e))?
```

### unwrap_or

Return the success value, or a default if error.

```
Result<T, E>.unwrap_or(T) -> T
```

```sigil
Ok(42).unwrap_or(0)        // 42
Err("fail").unwrap_or(0)   // 0
```

### ok

Convert to `Option<T>`, discarding error information.

```
Result<T, E>.ok() -> Option<T>
```

```sigil
Ok(42).ok()       // Some(42)
Err("fail").ok()  // None
```

### err

Convert to `Option<E>`, discarding success value.

```
Result<T, E>.err() -> Option<E>
```

```sigil
Err("fail").err()  // Some("fail")
Ok(42).err()       // None
```

### and_then

Chain operations that may fail.

```
Result<T, E>.and_then(T -> Result<U, E>) -> Result<U, E>
```

```sigil
Ok(2).and_then(x -> Ok(x * 2))      // Ok(4)
Ok(2).and_then(x -> Err("fail"))    // Err("fail")
Err("fail").and_then(x -> Ok(x * 2)) // Err("fail")
```

## Assertion Functions

### assert

Assert that a condition is true.

```
@assert (cond: bool) -> void
```

If `cond` is false, the program panics with an assertion error.

```sigil
assert(x > 0)
assert(list.len() > 0)
```

### assert_eq

Assert that two values are equal.

```
@assert_eq (actual: T, expected: T) -> void
```

If `actual != expected`, the program panics with a diagnostic showing both values.

```sigil
assert_eq(add(2, 3), 5)
assert_eq(result, expected)
```

### assert_ne

Assert that two values are not equal.

```
@assert_ne (actual: T, unexpected: T) -> void
```

If `actual == unexpected`, the program panics with a diagnostic showing both values.

### assert_some

Assert that an `Option` is `Some`.

```
@assert_some (option: Option<T>) -> void
```

### assert_none

Assert that an `Option` is `None`.

```
@assert_none (option: Option<T>) -> void
```

### assert_ok

Assert that a `Result` is `Ok`.

```
@assert_ok (result: Result<T, E>) -> void
```

### assert_err

Assert that a `Result` is `Err`.

```
@assert_err (result: Result<T, E>) -> void
```

### assert_panics

Assert that evaluating an expression panics.

```
@assert_panics (expr: T) -> void
```

The argument expression is evaluated and must panic; otherwise the assertion fails.

### assert_panics_with

Assert that evaluating an expression panics with a specific message.

```
@assert_panics_with (expr: T, msg: str) -> void
```

The argument expression is evaluated and must panic with an error message equal to `msg`.

## I/O Functions

### print

Print a message to standard output.

```
@print (msg: str) -> void
```

```sigil
print("Hello, World!")
print("Value: " + str(x))
```

## Comparison Functions

### compare

Compare two values.

```
@compare (a: T, b: T) -> Ordering where T: Comparable
```

Returns `Less`, `Equal`, or `Greater`.

### min

Return the minimum of two values.

```
@min (a: T, b: T) -> T where T: Comparable
```

### max

Return the maximum of two values.

```
@max (a: T, b: T) -> T where T: Comparable
```

## Panic

### panic

Terminate execution with an error message.

```
@panic (msg: str) -> Never
```

The return type `Never` indicates that this function never returns normally.

```sigil
panic("Unexpected state")
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

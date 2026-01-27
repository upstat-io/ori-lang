# Prelude

This document covers Ori's prelude: items that are automatically available without explicit imports, and when to use explicit stdlib imports.

> **Reference:** See [Standard Library § Prelude](../../modules/prelude.md) for complete API documentation.

---

## Overview

The prelude is a small set of items automatically imported into every Ori module. These are fundamental types and functions that are used constantly.

```ori
// No imports needed for these:
@find (items: [int], target: int) -> Option<int> = ...
@divide (dividend: int, divisor: int) -> Result<int, str> = ...
@main () -> void = print("Hello, " + str(42))
```

### Design Rationale

| Principle | Rationale |
|-----------|-----------|
| Minimal prelude | Explicit is better than implicit |
| Ubiquitous items only | Error types and basic I/O are everywhere |
| Clear boundaries | Prelude should be memorizable |
| Explicit stdlib | Everything else requires `use` |

---

## Auto-Imported Items

These items are available in every module without any `use` statement.

### Primitive Types

| Type | Description |
|------|-------------|
| `int`, `float`, `bool`, `str` | Basic types |
| `char` | Unicode scalar value |
| `byte` | 8-bit unsigned integer |
| `void` | Unit type |
| `Never` | Bottom type |
| `Duration` | Time span |
| `Size` | Byte size |

### Error Handling Types

| Item | Description |
|------|-------------|
| `Option<T>` | Value that may not exist |
| `Some(T)` | Constructor for present value |
| `None` | Constructor for absent value |
| `Result<T, E>` | Operation that may fail |
| `Ok(T)` | Constructor for success |
| `Err(E)` | Constructor for failure |
| `Error` | Standard error type |
| `Ordering` | Comparison result (Less, Equal, Greater) |

### Collection Types

| Item | Description |
|------|-------------|
| `[T]` | List |
| `{K: V}` | Map |
| `Set<T>` | Unique collection |
| `Range<T>` | Range (from `..` operator) |
| `Channel<T>` | Async communication |

### Core Traits

| Trait | Description |
|-------|-------------|
| `Eq` | Equality comparison |
| `Comparable` | Ordering comparison |
| `Hashable` | Can be map/set key |
| `Printable` | String conversion |
| `Clone` | Explicit copying |
| `Default` | Default value |

### Basic Functions

| Item | Signature | Description |
|------|-----------|-------------|
| `print` | `(Printable) -> void` | Output to stdout |
| `len` | `(Collection) -> int` | Length of collection |
| `str` | `(Printable) -> str` | Convert to string |
| `int` | `(T) -> int` | Convert to integer |
| `float` | `(T) -> float` | Convert to float |
| `compare` | `(Comparable, Comparable) -> Ordering` | Compare values |
| `panic` | `(str) -> Never` | Terminate with error |
| `assert` | `(bool) -> void` | Runtime assertion |
| `assert_eq` | `(T, T) -> void` | Equality assertion |

### Built-in Function Name Resolution

Built-in function names (`min`, `max`, `len`, `str`, etc.) are reserved **only in call position**. You can use these names freely as variables:

```ori
// OK: variable binding
let min = 5
// OK: uses variable
let result = min + 1
// OK: calls built-in function
let y = min(.left: a, .right: b)
```

The parser distinguishes by context:
- `name(` → built-in function call
- `name` alone → variable reference

#### Why This Design?

| Consideration | Resolution |
|---------------|------------|
| Common variable names | `min`, `max`, `str` are natural names for local values |
| Built-in accessibility | Built-ins remain callable regardless of local variables |
| No shadowing ambiguity | `min(` always means the built-in, never a variable |
| Function reservation | User can't define `@min`, preventing confusion |

This approach balances flexibility (use `min` as a variable) with predictability (built-in functions are always accessible via call syntax).

#### What's Not Allowed

Defining a function with a built-in name is an error:

```ori
// Error: 'min' is a reserved built-in function name
@min (left: int, right: int) -> int = if left < right then left else right
```

This prevents ambiguity about what `min(` means—it's always the built-in.

---

## Option Type

### Definition

```ori
// Conceptually defined as:
type Option<T> = Some(T) | None
```

### Usage

```ori
// No import needed
@find_first (items: [int], predicate: (int) -> bool) -> Option<int> =
    match(filter(items, predicate),
        [] -> None,
        [first, ..] -> Some(first),
    )

@safe_head (items: [int]) -> Option<int> =
    if len(items) == 0 then None
    else Some(items[0])
```

### Why in Prelude?

Every operation that might not return a value uses `Option`:

```ori
// List lookup - returns Option<T>
items.get(index)

// Map lookup - returns Option<V>
map.get(key)

// String search - returns Option<int>
string.find(substring)
```

Without `Option` in the prelude, virtually every module would need:

```ori
// tedious
use std { Option, Some, None }
```

---

## Result Type

### Definition

```ori
// Conceptually defined as:
type Result<T, E> = Ok(T) | Err(E)
```

### Usage

```ori
// No import needed
@divide (dividend: int, divisor: int) -> Result<int, str> =
    if divisor == 0 then Err("division by zero")
    else Ok(dividend / divisor)

@validate_age (input: str) -> Result<int, Error> = try(
    // parse_int is from std, returns Result
    let age = parse_int(input)?,
    if age < 0 then Err(Error { message: "age cannot be negative", cause: None })
    else Ok(age),
)
```

### Why in Prelude?

Every operation that can fail uses `Result`:

```ori
// File operations
// Result<str, Error>
read_file(path)

// Network operations
// Result<Response, Error>
http.get(url)

// Parsing
// Result<Value, Error>
json.parse(data)
```

---

## Error Type

### Definition

```ori
// Standard error type
type Error = { message: str, cause: Option<Error> }
```

### Usage

```ori
// Creating errors
error = Error { message: "something went wrong", cause: None }
error = Error { message: "failed to process", cause: Some(inner_error) }

// In results
@process () -> Result<Data, Error> =
    if invalid then Err(Error { message: "invalid input", cause: None })
    else Ok(compute())
```

### Why in Prelude?

`Error` is the default error type when you don't need something more specific:

```ori
// Application code often uses generic Error
@main () -> Result<void, Error> = try(
    let config = load_config()?,
    let data = fetch_data(config)?,
    process(data),
    Ok(()),
)
```

For library code, define specific error types:

```ori
type ParseError = InvalidSyntax(line: int) | UnexpectedEof
type NetworkError = Timeout | ConnectionRefused | InvalidUrl(str)
```

---

## Basic Functions

### print

```ori
// Output to stdout
print("Hello, world!")
print("Value: " + str(42))
// blank line
print("")
```

**Type:** `(str) -> void`

### len

```ori
// Length of lists (function or method)
// 3
len([1, 2, 3])
// 3
[1, 2, 3].len()

// Length of strings (in characters)
// 5
len("hello")
// 5
"hello".len()
```

**Type:** `([T]) -> int` and `(str) -> int`

**Note:** Available both as a prelude function `len(x)` and as a method `x.len()`.

### str

```ori
// Convert anything to string
// "42"
str(42)
// "3.14"
str(3.14)
// "true"
str(true)
// "[1, 2, 3]"
str([1, 2, 3])
// "Some(5)"
str(Some(5))
```

**Type:** `(T) -> str` (generic)

### int

```ori
// Convert to integer
// truncates to 3
int(3.14)
// parses to 42
int("42")
// true becomes 1
int(true)
// false becomes 0
int(false)
```

**Type:** `(T) -> int`

**Note:** Parsing invalid strings panics at runtime. Invalid string literals (like `int("abc")`) are caught at compile time. Use `parse_int` from std for safe parsing that returns `Result`.

### float

```ori
// Convert to float
// converts to 42.0
float(42)
// parses to 3.14
float("3.14")
```

**Type:** `(T) -> float`

---

## Explicit Stdlib Imports

Everything beyond the prelude requires explicit import.

### std.math

```ori
use std.math { sqrt, abs, pow, sin, cos, tan, log, exp, floor, ceil, round }
use std.math { min, max, clamp }
use std.math { pi, e }

@hypotenuse (side_a: float, side_b: float) -> float = sqrt(pow(side_a, 2) + pow(side_b, 2))
```

### String Methods

String methods are available on all `str` values:

```ori
// Transformation
// "HELLO"
text.upper()
// "hello"
text.lower()
// "Hello"
text.capitalize()
// removes whitespace
text.trim()

// Queries
// bool
text.starts_with("h")
// bool
text.ends_with("o")
// bool
text.contains("ell")
// Option<int> - index of first occurrence
text.find("sub")

// Splitting/joining
// [str]
text.split(" ")
// str
parts.join(", ")

// Slicing
// str - substring from index 0 to 5
text.slice(0, 5)

@format_name (first: str, last: str) -> str =
    first.capitalize() + " " + last.upper()
```

### List Methods

List methods are available on all `[T]` values:

```ori
// Core transformations
// [U] - transform each element
items.map(item -> item * 2)
// [T] - keep matching elements
items.filter(item -> item > 0)
// U - reduce to single value
items.fold(0, (accumulator, item) -> accumulator + item)

// Other transformations
// sorted copy
items.sort()
// reversed copy
items.reverse()
// deduplicated
items.unique()

// Slicing
// first 5
items.take(5)
// skip first 2
items.drop(2)
// sublist
items.slice(1, 4)

// Queries
// Option<T>
items.find(item -> item > 0)
// Option<int>
items.find_index(item -> item > 0)
// bool
items.any(item -> item > 0)
// bool
items.all(item -> item > 0)

// Result/Option combinators
// [U] - map to Option, keep Some
items.filter_map(item -> try_parse(item))
// Result<[U], E> - fail if any fails
items.traverse(item -> validate(item))

@process_items (items: [int]) -> [int] =
    items.filter(item -> item > 0).unique().sort()

@parse_all (strings: [str]) -> Result<[int], Error> =
    strings.traverse(text -> parse_int(text))

@parse_valid (strings: [str]) -> [int] =
    strings.filter_map(text -> parse_int(text).ok())
```

### std.io

```ori
use std.io { read_file, write_file, append_file }
use std.io { read_lines, write_lines }
use std.io { exists, delete, rename }

@backup (path: str) -> Result<void, Error> = try(
    let content = read_file(path)?,
    write_file(path + ".bak", content)?,
    Ok(()),
)
```

### std.json

```ori
use std.json { parse, stringify }
use std.json { Value, Object, Array }

@decode (json_str: str) -> Result<Data, Error> = try(
    let value = parse(json_str)?,
    Ok(convert_to_data(value)),
)
```

### std.http

```ori
use std.http { get, post, put, delete }
use std.http { Request, Response, Headers }

@fetch_api (url: str) -> Result<Response, Error> =
    get(url, .headers: {"Accept": "application/json"})
```

### std.time

```ori
use std.time { now, Duration, Instant }
use std.time { sleep, timeout }

@measure<T> (operation: () -> T) -> (T, Duration) = run(
    let start = now(),
    let result = operation(),
    let elapsed = now() - start,
    (result, elapsed),
)
```

---

## Why Explicit Imports?

### Clarity

```ori
// Clear: these functions come from std.regex
use std.regex { compile, match_all }

@extract_emails (text: str) -> [str] = match_all(compile(r"\S+@\S+"), text)
```

### No Hidden Dependencies

```ori
// Every external dependency is visible at the top
use std.math { sqrt }
use std.io { read_file }
use http { get }

// Scanning imports tells you what this module uses
```

### AI Benefits

| Benefit | Explanation |
|---------|-------------|
| Context | AI sees exactly what's available |
| No guessing | "Where did `split` come from?" is answered |
| Error prevention | Can't accidentally use unimported function |
| Refactoring | Clear dependency graph |

---

## Prelude vs Import Decision

### In Prelude (No Import)

Items are in the prelude if they are:

1. **Ubiquitous** - Used in almost every module
2. **Fundamental** - Part of basic program structure
3. **Few** - Small enough set to memorize

```ori
// These work everywhere without imports:
Option, Some, None
Result, Ok, Err
Error
print, len, str, int, float
```

### Requires Import

Everything else:

```ori
// Math beyond basic types
use std.math { sqrt, sin, cos }

// String utilities beyond methods
use std.string { pad_left, pad_right, repeat }

// Collections beyond methods
use std.list { zip, flatten, chunk, group_by }

// I/O operations
use std.io { read_file, write_file }

// Networking
use std.http { get, post }

// Serialization
use std.json { parse, stringify }

// Time operations
use std.time { now, sleep }
```

---

## Prelude Implementation

### Conceptual Definition

The prelude is conceptually equivalent to:

```ori
// File: std/prelude.ori

pub type Option<T> = Some(T) | None
pub type Result<T, E> = Ok(T) | Err(E)
pub type Error = { message: str, cause: Option<Error> }

pub @print (message: str) -> void = ...
pub @len<T> (items: [T]) -> int = ...
pub @str<T> (value: T) -> str = ...
pub @int (value: T) -> int = ...
pub @float (value: T) -> float = ...
```

### Implicit Import

Every module implicitly has:

```ori
use std.prelude { Option, Some, None, Result, Ok, Err, Error, print, len, str, int, float }
```

This import is automatic and cannot be disabled.

---

## Common Patterns

### Option Handling

```ori
// Using prelude Option
@get_or_default (opt: Option<int>, default: int) -> int =
    match(opt,
        Some(value) -> value,
        None -> default,
    )

@map_option<T, U> (opt: Option<T>, transform: (T) -> U) -> Option<U> =
    match(opt,
        Some(value) -> Some(transform(value)),
        None -> None,
    )
```

### Result Handling

```ori
// Using prelude Result
@safe_divide (dividend: int, divisor: int) -> Result<int, Error> =
    if divisor == 0 then Err(Error("division by zero"))
    else Ok(dividend / divisor)

@process () -> Result<int, Error> = try(
    let first_result = safe_divide(10, 2)?,
    // propagates Err
    let second_result = safe_divide(first_result, 0)?,
    Ok(second_result),
)
```

### Type Conversion

```ori
// Using prelude conversion functions
@format_values (number: int, decimal: float, flag: bool) -> str =
    "int=" + str(number) + ", float=" + str(decimal) + ", bool=" + str(flag)

@parse_args (args: [str]) -> (int, str) = run(
    let count = int(args[0]),
    let name = args[1],
    (count, name),
)
```

---

## Best Practices

### Use Prelude Naturally

```ori
// Good: prelude items used directly
@find_user (users: [User], id: int) -> Option<User> =
    match(filter(users, user -> user.id == id),
        [found, ..] -> Some(found),
        [] -> None,
    )
```

### Don't Re-Import Prelude

```ori
// Unnecessary - these are already available
// use std { Option, Some, None, Result, Ok, Err }

// Just use them directly
@process () -> Option<int> = Some(42)
```

### Import Stdlib as Needed

```ori
// Good: import what you need from stdlib
use std.math { sqrt }
use std.string { split }

@distance (delta_x: float, delta_y: float) -> float = sqrt(delta_x*delta_x + delta_y*delta_y)
@parse_csv (line: str) -> [str] = split(line, ",")
```

---

## See Also

- [Standard Library § Prelude](../../modules/prelude.md) — Complete API documentation
- [Module System](01-module-system.md)
- [Imports](02-imports.md)
- [Re-exports](04-re-exports.md)
- [Result and Option](../05-error-handling/01-result-and-option.md)
- [Types Specification](../../spec/06-types.md)

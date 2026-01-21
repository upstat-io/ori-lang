# Prelude

This document covers Sigil's prelude: items that are automatically available without explicit imports, and when to use explicit stdlib imports.

---

## Overview

The prelude is a small set of items automatically imported into every Sigil module. These are fundamental types and functions that are used constantly.

```sigil
// No imports needed for these:
@find (items: [int], target: int) -> Option<int> = ...
@divide (a: int, b: int) -> Result<int, str> = ...
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

### Basic Functions

| Item | Signature | Description |
|------|-----------|-------------|
| `print` | `(str) -> void` | Output to stdout |
| `len` | `([T]) -> int` | Length of list/string |
| `str` | `(T) -> str` | Convert to string |
| `int` | `(T) -> int` | Convert to integer |
| `float` | `(T) -> float` | Convert to float |

---

## Option Type

### Definition

```sigil
// Conceptually defined as:
type Option<T> = Some(T) | None
```

### Usage

```sigil
// No import needed
@find_first (items: [int], predicate: (int) -> bool) -> Option<int> =
    match(filter(items, predicate),
        [] -> None,
        [first, ..] -> Some(first)
    )

@safe_head (items: [int]) -> Option<int> =
    if len(items) == 0 then None
    else Some(items[0])
```

### Why in Prelude?

Every operation that might not return a value uses `Option`:

```sigil
// List lookup - returns Option<T>
items.get(index)

// Map lookup - returns Option<V>
map.get(key)

// String search - returns Option<int>
string.find(substring)
```

Without `Option` in the prelude, virtually every module would need:

```sigil
use std { Option, Some, None }  // tedious
```

---

## Result Type

### Definition

```sigil
// Conceptually defined as:
type Result<T, E> = Ok(T) | Err(E)
```

### Usage

```sigil
// No import needed
@divide (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("division by zero")
    else Ok(a / b)

@parse_int (s: str) -> Result<int, Error> = try(
    validated = if s == "" then Err(Error("empty string")) else Ok(s),
    Ok(int(validated))
)
```

### Why in Prelude?

Every operation that can fail uses `Result`:

```sigil
// File operations
read_file(path)         // Result<str, Error>

// Network operations
http.get(url)           // Result<Response, Error>

// Parsing
json.parse(data)        // Result<Value, Error>
```

---

## Error Type

### Definition

```sigil
// Standard error type
type Error = { message: str, cause: Option<Error> }
```

### Usage

```sigil
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

```sigil
// Application code often uses generic Error
@main () -> Result<void, Error> = try(
    config = load_config(),
    data = fetch_data(config),
    process(data),
    Ok(())
)
```

For library code, define specific error types:

```sigil
type ParseError = InvalidSyntax(line: int) | UnexpectedEof
type NetworkError = Timeout | ConnectionRefused | InvalidUrl(str)
```

---

## Basic Functions

### print

```sigil
// Output to stdout
print("Hello, world!")
print("Value: " + str(42))
print("")  // blank line
```

**Type:** `(str) -> void`

### len

```sigil
// Length of lists (function or method)
len([1, 2, 3])        // 3
[1, 2, 3].len()       // 3

// Length of strings (in characters)
len("hello")          // 5
"hello".len()         // 5
```

**Type:** `([T]) -> int` and `(str) -> int`

**Note:** Available both as a prelude function `len(x)` and as a method `x.len()`.

### str

```sigil
// Convert anything to string
str(42)               // "42"
str(3.14)             // "3.14"
str(true)             // "true"
str([1, 2, 3])        // "[1, 2, 3]"
str(Some(5))          // "Some(5)"
```

**Type:** `(T) -> str` (generic)

### int

```sigil
// Convert to integer
int(3.14)             // 3 (truncates)
int("42")             // 42
int(true)             // 1
int(false)            // 0
```

**Type:** `(T) -> int`

**Note:** Parsing invalid strings panics at runtime. Invalid string literals (like `int("abc")`) are caught at compile time. Use `parse_int` from std for safe parsing that returns `Result`.

### float

```sigil
// Convert to float
float(42)             // 42.0
float("3.14")         // 3.14
```

**Type:** `(T) -> float`

---

## Explicit Stdlib Imports

Everything beyond the prelude requires explicit import.

### std.math

```sigil
use std.math { sqrt, abs, pow, sin, cos, tan, log, exp, floor, ceil, round }
use std.math { min, max, clamp }
use std.math { pi, e }

@hypotenuse (a: float, b: float) -> float = sqrt(pow(a, 2) + pow(b, 2))
```

### String Methods

String methods are available on all `str` values:

```sigil
// Transformation
text.upper()          // "HELLO"
text.lower()          // "hello"
text.capitalize()     // "Hello"
text.trim()           // removes whitespace

// Queries
text.starts_with("h") // bool
text.ends_with("o")   // bool
text.contains("ell")  // bool
text.find("sub")      // Option<int> - index of first occurrence

// Splitting/joining
text.split(" ")       // [str]
parts.join(", ")      // str

// Slicing
text.slice(0, 5)      // str - substring from index 0 to 5

@format_name (first: str, last: str) -> str =
    first.capitalize() + " " + last.upper()
```

### List Methods

List methods are available on all `[T]` values:

```sigil
// Core transformations
items.map(x -> x * 2)        // [U] - transform each element
items.filter(x -> x > 0)     // [T] - keep matching elements
items.fold(0, (acc, x) -> acc + x)  // U - reduce to single value

// Other transformations
items.sort()          // sorted copy
items.reverse()       // reversed copy
items.unique()        // deduplicated

// Slicing
items.take(5)         // first 5
items.drop(2)         // skip first 2
items.slice(1, 4)     // sublist

// Queries
items.find(x -> x > 0)       // Option<T>
items.find_index(x -> x > 0) // Option<int>
items.any(x -> x > 0)        // bool
items.all(x -> x > 0)        // bool

// Result/Option combinators
items.filter_map(x -> try_parse(x))  // [U] - map to Option, keep Some
items.traverse(x -> validate(x))     // Result<[U], E> - fail if any fails

@process_items (items: [int]) -> [int] =
    items.filter(x -> x > 0).unique().sort()

@parse_all (strings: [str]) -> Result<[int], Error> =
    strings.traverse(s -> parse_int(s))

@parse_valid (strings: [str]) -> [int] =
    strings.filter_map(s -> parse_int(s).ok())
```

### std.io

```sigil
use std.io { read_file, write_file, append_file }
use std.io { read_lines, write_lines }
use std.io { exists, delete, rename }

@backup (path: str) -> Result<void, Error> = try(
    content = read_file(path),
    write_file(path + ".bak", content),
    Ok(())
)
```

### std.json

```sigil
use std.json { parse, stringify }
use std.json { Value, Object, Array }

@decode (json_str: str) -> Result<Data, Error> = try(
    value = parse(json_str),
    Ok(convert_to_data(value))
)
```

### std.http

```sigil
use std.http { get, post, put, delete }
use std.http { Request, Response, Headers }

@fetch_api (url: str) -> Result<Response, Error> =
    get(url, .headers: {"Accept": "application/json"})
```

### std.time

```sigil
use std.time { now, Duration, Instant }
use std.time { sleep, timeout }

@measure<T> (f: () -> T) -> (T, Duration) = run(
    start = now(),
    result = f(),
    elapsed = now() - start,
    (result, elapsed)
)
```

---

## Why Explicit Imports?

### Clarity

```sigil
// Clear: these functions come from std.regex
use std.regex { compile, match_all }

@extract_emails (s: str) -> [str] = match_all(compile(r"\S+@\S+"), s)
```

### No Hidden Dependencies

```sigil
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

```sigil
// These work everywhere without imports:
Option, Some, None
Result, Ok, Err
Error
print, len, str, int, float
```

### Requires Import

Everything else:

```sigil
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

```sigil
// File: std/prelude.si

pub type Option<T> = Some(T) | None
pub type Result<T, E> = Ok(T) | Err(E)
pub type Error = { message: str, cause: Option<Error> }

pub @print (s: str) -> void = ...
pub @len<T> (items: [T]) -> int = ...
pub @str<T> (value: T) -> str = ...
pub @int (value: T) -> int = ...
pub @float (value: T) -> float = ...
```

### Implicit Import

Every module implicitly has:

```sigil
use std.prelude { Option, Some, None, Result, Ok, Err, Error, print, len, str, int, float }
```

This import is automatic and cannot be disabled.

---

## Common Patterns

### Option Handling

```sigil
// Using prelude Option
@get_or_default (opt: Option<int>, default: int) -> int =
    match(opt,
        .Some: v -> v,
        .None: default
    )

@map_option<T, U> (opt: Option<T>, f: (T) -> U) -> Option<U> =
    match(opt,
        .Some: v -> Some(f(v)),
        .None: None
    )
```

### Result Handling

```sigil
// Using prelude Result
@safe_divide (a: int, b: int) -> Result<int, Error> =
    if b == 0 then Err(Error("division by zero"))
    else Ok(a / b)

@process () -> Result<int, Error> = try(
    x = safe_divide(10, 2),
    y = safe_divide(x, 0),   // propagates Err
    Ok(y)
)
```

### Type Conversion

```sigil
// Using prelude conversion functions
@format_values (n: int, f: float, b: bool) -> str =
    "int=" + str(n) + ", float=" + str(f) + ", bool=" + str(b)

@parse_args (args: [str]) -> (int, str) = run(
    count = int(args[0]),
    name = args[1],
    (count, name)
)
```

---

## Best Practices

### Use Prelude Naturally

```sigil
// Good: prelude items used directly
@find_user (users: [User], id: int) -> Option<User> =
    match(filter(users, u -> u.id == id),
        [user, ..] -> Some(user),
        [] -> None
    )
```

### Don't Re-Import Prelude

```sigil
// Unnecessary - these are already available
// use std { Option, Some, None, Result, Ok, Err }

// Just use them directly
@process () -> Option<int> = Some(42)
```

### Import Stdlib as Needed

```sigil
// Good: import what you need from stdlib
use std.math { sqrt }
use std.string { split }

@distance (x: float, y: float) -> float = sqrt(x*x + y*y)
@parse_csv (line: str) -> [str] = split(line, ",")
```

---

## See Also

- [Module System](01-module-system.md)
- [Imports](02-imports.md)
- [Re-exports](04-re-exports.md)
- [Result and Option](../05-error-handling/01-result-and-option.md)

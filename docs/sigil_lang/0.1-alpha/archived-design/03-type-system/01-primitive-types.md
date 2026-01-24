# Primitive Types

This document covers Sigil's primitive types: int, float, bool, str, char, byte, void, Never, Duration, and Size.

---

## Overview

| Type | Description | Size |
|------|-------------|------|
| `int` | Signed integer | 64 bits |
| `float` | Floating point | 64 bits |
| `bool` | Boolean | 1 bit (stored as byte) |
| `str` | UTF-8 string | Variable |
| `char` | Unicode scalar value | 32 bits |
| `byte` | Unsigned byte | 8 bits |
| `void` | No value | 0 bits |
| `Never` | Bottom type (never returns) | N/A |
| `Duration` | Time duration | 64 bits |
| `Size` | Byte size | 64 bits |

---

## `int` — Integer

64-bit signed integer.

### Range

- Minimum: `-9223372036854775808`
- Maximum: `9223372036854775807`

### Literals

```sigil
let x = 42
let y = -17
let z = 0
// underscores for readability
let large = 1_000_000
```

### Operations

```sigil
// Arithmetic
// addition
a + b
// subtraction
a - b
// multiplication
a * b
// division (truncates toward zero: -7 / 3 = -2)
a / b
// modulo (sign follows dividend)
a % b
// floor division (toward -infinity: -7 div 3 = -3)
a div b

// Comparison
// equal
a == b
// not equal
a != b
// less than
a < b
// greater than
a > b
// less or equal
a <= b
// greater or equal
a >= b
```

### Conversion

```sigil
// -> 3 (truncates)
int(3.14)
// -> 42 (parses)
int("42")
// -> 1
int(true)
// -> 0
int(false)
```

**String parsing behavior:**
- `int("42")` succeeds → `42`
- `int("abc")` **panics** at runtime
- `int("abc")` with string literal → **compile error** (detected statically)

For safe parsing, use `parse_int` from std which returns `Result<int, Error>`:
```sigil
use std { parse_int }
// -> Ok(42)
parse_int("42")
// -> Err(ParseError)
parse_int("abc")
```

---

## `float` — Floating Point

64-bit IEEE 754 floating point (double precision).

### Literals

```sigil
let pi = 3.14159
let negative = -0.5
let scientific = 1.5e10
let small = 2.5e-8
```

### Operations

```sigil
// Arithmetic
// addition
a + b
// subtraction
a - b
// multiplication
a * b
// division
a / b

// Comparison
// equal (caution: floating point equality)
a == b
// not equal
a != b
// less than
a < b
// greater than
a > b
```

### Conversion

```sigil
// -> 42.0
float(42)
// -> 3.14
float("3.14")
```

### Special Values

```sigil
use std.math { inf, nan, is_nan, is_inf }

infinity = inf()
not_a_number = nan()
// true
is_nan(.value: not_a_number)
// true
is_inf(.value: infinity)
```

---

## `bool` — Boolean

True or false.

### Literals

```sigil
let yes = true
let no = false
```

### Operations

```sigil
// Logical
// and (short-circuit)
a && b
// or (short-circuit)
a || b
// not
!a

// Comparison
// equal
a == b
// not equal
a != b
```

### Short-Circuit Evaluation

```sigil
// b is only evaluated if a is true
a && b

// b is only evaluated if a is false
a || b
```

### Conversion

```sigil
// -> false
bool(0)
// -> true
bool(1)
// -> false
bool("")
// -> true
bool("x")
```

---

## `byte` — Unsigned Byte

8-bit unsigned integer for binary data.

### Range

- Minimum: `0`
- Maximum: `255`

### Literals

```sigil
// hex literal -> 65 (ASCII 'A')
let b = 0x41
// decimal
let b = 255
```

### Operations

```sigil
// Arithmetic
// addition (wraps on overflow)
a + b
// subtraction (wraps on underflow)
a - b
// multiplication
a * b

// Bitwise
// and
a & b
// or
a | b
// xor
a ^ b
// not
~a

// Comparison
// equal
a == b
// not equal
a != b
// less than
a < b
// greater than
a > b
```

### Common Usage

```sigil
// Binary data
type Body = { data: [byte], encoding: str }

// Reading files as bytes
@read_binary (path: str) -> Result<[byte], Error> = ...

// Network packets
@parse_packet (data: [byte]) -> Result<Packet, ParseError> = ...
```

### Conversion

```sigil
// -> 65
byte(65)
// -> 65 (first byte of UTF-8)
byte("A")
// byte to int
int(b)
```

---

## `char` — Character

A single Unicode scalar value, stored as 32 bits.

### Range

- Minimum: U+0000
- Maximum: U+10FFFF
- Excludes surrogate code points: U+D800 to U+DFFF

### Literals

Character literals use single quotes:

```sigil
let letter = 'a'
let unicode = 'λ'
let emoji = '🦀'
let newline = '\n'
let tab = '\t'
let backslash = '\\'
let single_quote = '\''
```

### Escape Sequences

| Escape | Character |
|--------|-----------|
| `\\` | Backslash |
| `\'` | Single quote |
| `\n` | Newline |
| `\t` | Tab |
| `\r` | Carriage return |
| `\0` | Null |

### Operations

```sigil
// Comparison
// equal
a == b
// not equal
a != b
// less than (by code point)
a < b
// greater than
a > b

// Checking
// true for letters
c.is_alphabetic()
// true for digits
c.is_numeric()
// true for spaces, tabs, newlines
c.is_whitespace()
// true for 0..=127
c.is_ascii()
```

### Relationship to `str`

A `char` is a single Unicode scalar value. A `str` is a sequence of UTF-8 encoded bytes that may represent multiple code points.

```sigil
let c: char = 'a'
let s: str = "a"

// char and str are distinct types
// Explicit conversion required
// char to str: "a"
str(c)
// str to [char]
s.chars()
// first char of str
s.chars()[0]
```

**Important:** `char` indexing of strings is O(n) because UTF-8 is variable-width. For character-by-character processing, convert to `[char]` first:

```sigil
let text = "hello"
// [char]
let chars = text.chars()
// 'h' - O(1) access
chars[0]
```

### Conversion

```sigil
// -> 'A' (from code point)
char(65)
// -> 65 (to code point)
int('A')
// -> "a"
str('a')
```

---

## `str` — String

UTF-8 encoded string.

### Literals

```sigil
let name = "Alice"
let empty = ""
let with_newline = "line1\nline2"
let with_quote = "She said \"hello\""
```

### Escape Sequences

| Escape | Character |
|--------|-----------|
| `\\` | Backslash |
| `\"` | Double quote |
| `\n` | Newline |
| `\t` | Tab |
| `\r` | Carriage return |

### Operations

```sigil
// Concatenation
let greeting = "Hello, " + name + "!"

// Length (function or method)
// -> 5
len(.collection: name)
// -> 5
name.len()

// Comparison
// equal
a == b
// not equal
a != b
// lexicographic less than
a < b

// Methods
// "ALICE"
name.upper()
// "alice"
name.lower()
// true
name.contains("li")
// true
name.starts_with("Al")
// true
name.ends_with("ce")
// removes whitespace
name.trim()
// -> [str]
name.split(",")
```

### Indexing

```sigil
// Note: indexes UTF-8 code points, not bytes
// "A"
let first = name[0]
// "e"
let last = name[# - 1]
```

### Conversion

```sigil
// -> "42"
str(42)
// -> "3.14"
str(3.14)
// -> "true"
str(true)
```

---

## `void` — No Meaningful Value

Used for functions that don't return a meaningful value. `void` is an alias for the unit type `()`.

### Usage

```sigil
@print_greeting (name: str) -> void = print(.message: "Hello, " + name)

@log (message: str) -> void = run(
    let timestamp = now(),
    print(.message: "[" + str(timestamp) + "] " + message),
)
```

### Relationship to Unit Type `()`

`void` and `()` are **the same type** — `void` is simply a type alias for `()`:

```sigil
// These are exactly equivalent:
@log1 (msg: str) -> void = print(.message: msg)
@log2 (msg: str) -> () = print(.message: msg)
```

**When to use which:**

| Use `void` | Use `()` |
|------------|----------|
| Return types for side-effecting functions | When unit is a value (generics, data structures) |
| Makes intent clear: "no meaningful return" | When you need `()` as a literal value |

```sigil
// void for "returns nothing meaningful"
@side_effect () -> void = do_something(.input: ())

// () when used as a value
// returns the unit value
@placeholder () -> () = ()
// unit as accumulator
items.fold((), (_, item) -> process(item))
```

### void/() in Data Structures

Since `void` and `()` are the same type, both can be used in data structures:

```sigil
// Result with no success value
@delete_user (id: int) -> Result<void, Error> = try(
    db.delete(id),
    // return unit value wrapped in Ok
    Ok(()),
)

// Option<()> is valid but unusual
// Some(()) or None
type Signal = Option<()>

// In generics
type Void = ()
@ignore<T> (input: T) -> () = ()
```

**Note:** Using `void` as a type parameter is valid but uncommon. Prefer `void` for return type annotations and `()` elsewhere.

### Discouraging Storage

While technically valid, storing void/unit values is discouraged and may trigger a warning:

```sigil
// Discouraged: storing a void value
// x has type (), but why store it?
let x = print(.message: "hello")

// Better: don't bind side effects
print(.message: "hello")
```

---

## `Never` — Bottom Type

The type of computations that never produce a value.

### Definition

`Never` is an uninhabited type—it has no values. A function returning `Never` can never return normally.

### Use Cases

**1. Functions that always panic:**

```sigil
@panic (message: str) -> Never = ...

@unreachable () -> Never = panic(.message: "unreachable code")

@todo (message: str) -> Never = panic(.message: "TODO: " + message)
```

**2. Functions that loop forever:**

```sigil
@run_forever (server: Server) -> Never = loop(
    accept_connection(server),
)
```

**3. Exhaustive match arms that are unreachable:**

```sigil
@process (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "in progress",
    Done -> "finished",
    // if match is exhaustive, no unreachable arm needed
)
```

### Coercion

`Never` can be coerced to any other type. This is safe because a `Never` value can never exist:

```sigil
@example () -> int = if condition then 42 else panic("failed")
// panic returns Never, which coerces to int
```

### In Result Types

`Result<T, Never>` indicates an infallible operation—one that cannot fail:

```sigil
@safe_parse (text: str) -> Result<int, Never> = ...

// Can safely unwrap without handling error case
// cannot panic
let value = safe_parse(input).unwrap()
```

Similarly, `Result<Never, E>` indicates an operation that always fails.

### Relationship to void

| Type | Meaning | Has values? |
|------|---------|-------------|
| `void` / `()` | Returns successfully with no data | Yes: `()` |
| `Never` | Never returns | No (uninhabited) |

```sigil
// void: returns successfully, no meaningful value
@log (message: str) -> void = print(message)

// Never: does not return
@abort () -> Never = panic("abort")
```

---

## `Duration` — Time Duration

Represents a span of time. Used for timeouts, delays, and time measurements.

### Literals

Duration literals use unit suffixes:

```sigil
// milliseconds
let delay = 100ms
// seconds
let timeout = 30s
// minutes
let interval = 5m
// hours
let max_wait = 2h
```

### Units

| Suffix | Unit |
|--------|------|
| `ms` | Milliseconds |
| `s` | Seconds |
| `m` | Minutes |
| `h` | Hours |

### Operations

```sigil
// Arithmetic
// add durations
a + b
// subtract durations
a - b
// scale duration
a * 2
// divide duration
a / 2

// Comparison
// equal
a == b
// not equal
a != b
// less than
a < b
// greater than
a > b
```

### Conversion

```sigil
// -> int (milliseconds)
d.as_ms()
// -> int (seconds)
d.as_secs()
// -> float (seconds as float)
d.as_float()

// -> Duration
Duration.from_ms(100)
// -> Duration
Duration.from_secs(30)
```

### Usage

```sigil
// timeout returns Result<T, TimeoutError>
@fetch_data (url: str) -> Result<Data, TimeoutError> uses Http, Async =
    timeout(
        .operation: Http.get(url),
        .after: 30s,
    )

// Periodic task using loop
@periodic (interval: Duration, action: () -> void) -> void uses Clock, Async =
    loop(
        Clock.sleep(interval),
        action()
    )

// Common usage with config
$request_timeout = 30s
$cache_ttl = 5m
$retry_delay = 100ms
```

---

## `Size` — Byte Size

Represents a number of bytes. Used for buffer sizes, file sizes, and memory limits.

### Literals

Size literals use unit suffixes:

```sigil
// kilobytes
let buffer = 4kb
// megabytes
let max_file = 10mb
// gigabytes
let memory = 2gb
```

### Units

| Suffix | Unit | Bytes |
|--------|------|-------|
| `b` | Bytes | 1 |
| `kb` | Kilobytes | 1,024 |
| `mb` | Megabytes | 1,048,576 |
| `gb` | Gigabytes | 1,073,741,824 |

### Operations

```sigil
// Arithmetic
// add sizes
a + b
// subtract sizes
a - b
// scale size
a * 2
// divide size
a / 2

// Comparison
// equal
a == b
// not equal
a != b
// less than
a < b
// greater than
a > b
```

### Conversion

```sigil
// -> int
s.as_bytes()
// -> float
s.as_kb()
// -> float
s.as_mb()

// -> Size
Size.from_bytes(1024)
```

### Usage

```sigil
@read_chunked (path: str, chunk_size: Size) -> Result<[str], Error> uses FileSystem, Async = ...

// Common usage with config
$buffer_size = 4kb
$max_upload = 10mb
$cache_limit = 100mb
```

---

## Type Relationships

### No Implicit Conversions

```sigil
// ERROR: type mismatch
// float != int
value: int = 3.14
// int != float
number: float = 42

// Must be explicit
// OK
value: int = int(3.14)
// OK
number: float = float(42)
```

### Numeric Tower

There is no implicit numeric hierarchy. Each conversion must be explicit:

```sigil
let integer: int = 42
// explicit conversion
let decimal: float = float(integer)
```

---

## See Also

- [Compound Types](02-compound-types.md)
- [Type Inference](05-type-inference.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
- [Async via Capabilities](../10-async/01-async-await.md) — Duration usage with timeouts
- [Patterns Reference](../02-syntax/04-patterns-reference.md) — timeout and cache patterns

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
let large = 1_000_000  // underscores for readability
```

### Operations

```sigil
// Arithmetic
a + b    // addition
a - b    // subtraction
a * b    // multiplication
a / b    // division (truncates toward zero: -7 / 3 = -2)
a % b    // modulo (sign follows dividend)
a div b  // floor division (toward -∞: -7 div 3 = -3)

// Comparison
a == b   // equal
a != b   // not equal
a < b    // less than
a > b    // greater than
a <= b   // less or equal
a >= b   // greater or equal
```

### Conversion

```sigil
int(3.14)     // -> 3 (truncates)
int("42")     // -> 42 (parses)
int(true)     // -> 1
int(false)    // -> 0
```

**String parsing behavior:**
- `int("42")` succeeds → `42`
- `int("abc")` **panics** at runtime
- `int("abc")` with string literal → **compile error** (detected statically)

For safe parsing, use `parse_int` from std which returns `Result<int, Error>`:
```sigil
use std { parse_int }
parse_int("42")   // -> Ok(42)
parse_int("abc")  // -> Err(ParseError)
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
a + b    // addition
a - b    // subtraction
a * b    // multiplication
a / b    // division

// Comparison
a == b   // equal (caution: floating point equality)
a != b   // not equal
a < b    // less than
a > b    // greater than
```

### Conversion

```sigil
float(42)      // -> 42.0
float("3.14")  // -> 3.14
```

### Special Values

```sigil
use std.math { inf, nan, is_nan, is_inf }

infinity = inf()
not_a_number = nan()
is_nan(not_a_number)  // true
is_inf(infinity)       // true
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
a && b   // and (short-circuit)
a || b   // or (short-circuit)
!a       // not

// Comparison
a == b   // equal
a != b   // not equal
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
bool(0)    // -> false
bool(1)    // -> true
bool("")   // -> false
bool("x")  // -> true
```

---

## `byte` — Unsigned Byte

8-bit unsigned integer for binary data.

### Range

- Minimum: `0`
- Maximum: `255`

### Literals

```sigil
let b = 0x41          // hex literal -> 65 (ASCII 'A')
let b = 255           // decimal
```

### Operations

```sigil
// Arithmetic
a + b    // addition (wraps on overflow)
a - b    // subtraction (wraps on underflow)
a * b    // multiplication

// Bitwise
a & b    // and
a | b    // or
a ^ b    // xor
~a       // not

// Comparison
a == b   // equal
a != b   // not equal
a < b    // less than
a > b    // greater than
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
byte(65)      // -> 65
byte("A")     // -> 65 (first byte of UTF-8)
int(b)        // byte to int
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
a == b   // equal
a != b   // not equal
a < b    // less than (by code point)
a > b    // greater than

// Checking
c.is_alphabetic()  // true for letters
c.is_numeric()     // true for digits
c.is_whitespace()  // true for spaces, tabs, newlines
c.is_ascii()       // true for 0..=127
```

### Relationship to `str`

A `char` is a single Unicode scalar value. A `str` is a sequence of UTF-8 encoded bytes that may represent multiple code points.

```sigil
let c: char = 'a'
let s: str = "a"

// char and str are distinct types
// Explicit conversion required
str(c)          // char to str: "a"
s.chars()       // str to [char]
s.chars()[0]    // first char of str
```

**Important:** `char` indexing of strings is O(n) because UTF-8 is variable-width. For character-by-character processing, convert to `[char]` first:

```sigil
let text = "hello"
let chars = text.chars()  // [char]
chars[0]  // 'h' - O(1) access
```

### Conversion

```sigil
char(65)       // -> 'A' (from code point)
int('A')       // -> 65 (to code point)
str('a')       // -> "a"
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
len(name)   // -> 5
name.len()  // -> 5

// Comparison
a == b     // equal
a != b     // not equal
a < b      // lexicographic less than

// Methods
name.upper()           // "ALICE"
name.lower()           // "alice"
name.contains("li")    // true
name.starts_with("Al") // true
name.ends_with("ce")   // true
name.trim()            // removes whitespace
name.split(",")        // -> [str]
```

### Indexing

```sigil
// Note: indexes UTF-8 code points, not bytes
let first = name[0]       // "A"
let last = name[# - 1]    // "e"
```

### Conversion

```sigil
str(42)      // -> "42"
str(3.14)    // -> "3.14"
str(true)    // -> "true"
```

---

## `void` — No Meaningful Value

Used for functions that don't return a meaningful value. `void` is an alias for the unit type `()`.

### Usage

```sigil
@print_greeting (name: str) -> void = print("Hello, " + name)

@log (message: str) -> void = run(
    let timestamp = now(),
    print("[" + str(timestamp) + "] " + message),
)
```

### Relationship to Unit Type `()`

`void` and `()` are **the same type** — `void` is simply a type alias for `()`:

```sigil
// These are exactly equivalent:
@log1 (msg: str) -> void = print(msg)
@log2 (msg: str) -> () = print(msg)
```

**When to use which:**

| Use `void` | Use `()` |
|------------|----------|
| Return types for side-effecting functions | When unit is a value (generics, data structures) |
| Makes intent clear: "no meaningful return" | When you need `()` as a literal value |

```sigil
// void for "returns nothing meaningful"
@side_effect () -> void = do_something()

// () when used as a value
@placeholder () -> () = ()            // returns the unit value
items.fold((), (_, x) -> process(x))  // unit as accumulator
```

### void/() in Data Structures

Since `void` and `()` are the same type, both can be used in data structures:

```sigil
// Result with no success value
@delete_user (id: int) -> Result<void, Error> = try(
    db.delete(id),
    Ok(())  // return unit value wrapped in Ok
)

// Option<()> is valid but unusual
type Signal = Option<()>  // Some(()) or None

// In generics
type Void = ()
@ignore<T> (value: T) -> () = ()
```

**Note:** Using `void` as a type parameter is valid but uncommon. Prefer `void` for return type annotations and `()` elsewhere.

### Discouraging Storage

While technically valid, storing void/unit values is discouraged and may trigger a warning:

```sigil
// Discouraged: storing a void value
let x = print("hello")  // x has type (), but why store it?

// Better: don't bind side effects
print("hello")
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

@unreachable () -> Never = panic("unreachable code")

@todo (message: str) -> Never = panic("TODO: " + message)
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
@safe_parse (s: str) -> Result<int, Never> = ...

// Can safely unwrap without handling error case
let value = safe_parse(input).unwrap()  // cannot panic
```

Similarly, `Result<Never, E>` indicates an operation that always fails.

### Relationship to void

| Type | Meaning | Has values? |
|------|---------|-------------|
| `void` / `()` | Returns successfully with no data | Yes: `()` |
| `Never` | Never returns | No (uninhabited) |

```sigil
// void: returns successfully, no meaningful value
@log (msg: str) -> void = print(msg)

// Never: does not return
@abort () -> Never = panic("abort")
```

---

## `Duration` — Time Duration

Represents a span of time. Used for timeouts, delays, and time measurements.

### Literals

Duration literals use unit suffixes:

```sigil
let delay = 100ms       // milliseconds
let timeout = 30s       // seconds
let interval = 5m       // minutes
let max_wait = 2h       // hours
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
a + b          // add durations
a - b          // subtract durations
a * 2          // scale duration
a / 2          // divide duration

// Comparison
a == b         // equal
a != b         // not equal
a < b          // less than
a > b          // greater than
```

### Conversion

```sigil
d.as_ms()      // -> int (milliseconds)
d.as_secs()    // -> int (seconds)
d.as_float()   // -> float (seconds as float)

Duration.from_ms(100)    // -> Duration
Duration.from_secs(30)   // -> Duration
```

### Usage

```sigil
// timeout returns Result<T, TimeoutError>
@fetch_data (url: str) -> async Result<Data, TimeoutError> =
    timeout(http_get(url), 30s)

// Periodic task using loop
@periodic (interval: Duration, action: () -> void) -> async void =
    loop(
        sleep(interval).await,
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
let buffer = 4kb        // kilobytes
let max_file = 10mb     // megabytes
let memory = 2gb        // gigabytes
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
a + b          // add sizes
a - b          // subtract sizes
a * 2          // scale size
a / 2          // divide size

// Comparison
a == b         // equal
a != b         // not equal
a < b          // less than
a > b          // greater than
```

### Conversion

```sigil
s.as_bytes()   // -> int
s.as_kb()      // -> float
s.as_mb()      // -> float

Size.from_bytes(1024)    // -> Size
```

### Usage

```sigil
@read_chunked (path: str, chunk_size: Size) -> async Result<[str], Error> = ...

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
x: int = 3.14        // float != int
y: float = 42        // int != float

// Must be explicit
x: int = int(3.14)   // OK
y: float = float(42) // OK
```

### Numeric Tower

There is no implicit numeric hierarchy. Each conversion must be explicit:

```sigil
let i: int = 42
let f: float = float(i)  // explicit conversion
```

---

## See Also

- [Compound Types](02-compound-types.md)
- [Type Inference](05-type-inference.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
- [Async/Await](../10-async/01-async-await.md) — Duration usage with timeouts
- [Patterns Reference](../02-syntax/04-patterns-reference.md) — timeout and cache patterns

# Primitive Types

This document covers Sigil's primitive types: int, float, bool, str, byte, void, Duration, and Size.

---

## Overview

| Type | Description | Size |
|------|-------------|------|
| `int` | Signed integer | 64 bits |
| `float` | Floating point | 64 bits |
| `bool` | Boolean | 1 bit (stored as byte) |
| `str` | UTF-8 string | Variable |
| `byte` | Unsigned byte | 8 bits |
| `void` | No value | 0 bits |
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
x = 42
y = -17
z = 0
large = 1_000_000  // underscores for readability
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
pi = 3.14159
negative = -0.5
scientific = 1.5e10
small = 2.5e-8
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
yes = true
no = false
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
b = 0x41          // hex literal -> 65 (ASCII 'A')
b = 255           // decimal
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

## `str` — String

UTF-8 encoded string.

### Literals

```sigil
name = "Alice"
empty = ""
with_newline = "line1\nline2"
with_quote = "She said \"hello\""
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
greeting = "Hello, " + name + "!"

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
first = name[0]       // "A"
last = name[# - 1]    // "e"
```

### Conversion

```sigil
str(42)      // -> "42"
str(3.14)    // -> "3.14"
str(true)    // -> "true"
```

---

## `void` — No Value

Used for functions that don't return a meaningful value.

### Usage

```sigil
@print_greeting (name: str) -> void = print("Hello, " + name)

@log (message: str) -> void = run(
    timestamp = now(),
    print("[" + str(timestamp) + "] " + message)
)
```

### Cannot Be Stored

```sigil
// ERROR: cannot store void
x = print("hello")
```

---

## `Duration` — Time Duration

Represents a span of time. Used for timeouts, delays, and time measurements.

### Literals

Duration literals use unit suffixes:

```sigil
delay = 100ms       // milliseconds
timeout = 30s       // seconds
interval = 5m       // minutes
max_wait = 2h       // hours
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
@with_timeout (op: async T, limit: Duration) -> async Result<T, TimeoutError> =
    timeout(.op: op.await, .after: limit, .on_timeout: Err(TimeoutError))

@periodic (interval: Duration, action: () -> void) -> async void =
    for _ in forever() do run(
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
buffer = 4kb        // kilobytes
max_file = 10mb     // megabytes
memory = 2gb        // gigabytes
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
i: int = 42
f: float = float(i)  // explicit conversion
```

---

## See Also

- [Compound Types](02-compound-types.md)
- [Type Inference](05-type-inference.md)
- [Basic Syntax](../02-syntax/01-basic-syntax.md)
- [Async/Await](../10-async/01-async-await.md) — Duration usage with timeouts
- [Patterns Reference](../02-syntax/04-patterns-reference.md) — timeout and cache patterns

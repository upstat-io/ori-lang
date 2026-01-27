# std.fmt

String formatting utilities.

```ori
use std.fmt { format, pad_left, pad_right }
```

**No capability required**

---

## Overview

The `std.fmt` module provides:

- String interpolation and formatting
- Padding and alignment
- Number formatting

---

## Functions

### @format

```ori
@format (template: str, args: ...) -> str
```

Formats a string with placeholders.

```ori
use std.fmt { format }

format("Hello, {}!", "world")
// "Hello, world!"

format("{} + {} = {}", 2, 3, 5)
// "2 + 3 = 5"

format("Name: {name}, Age: {age}", name: "Alice", age: 30)
// "Name: Alice, Age: 30"
```

---

### @pad_left

```ori
@pad_left (s: str, width: int, fill: char) -> str
```

Pads string on the left to reach width.

```ori
use std.fmt { pad_left }

pad_left("42", 5, '0')   // "00042"
pad_left("hi", 10, ' ')  // "        hi"
```

---

### @pad_right

```ori
@pad_right (s: str, width: int, fill: char) -> str
```

Pads string on the right to reach width.

```ori
use std.fmt { pad_right }

pad_right("hi", 10, ' ')  // "hi        "
```

---

### @center

```ori
@center (s: str, width: int, fill: char) -> str
```

Centers string within width.

```ori
use std.fmt { center }

center("hi", 10, '-')  // "----hi----"
```

---

### @truncate

```ori
@truncate (s: str, max_len: int) -> str
@truncate (s: str, max_len: int, suffix: str) -> str
```

Truncates string to maximum length.

```ori
use std.fmt { truncate }

truncate("Hello, world!", 5)         // "Hello"
truncate("Hello, world!", 8, "...")  // "Hello..."
```

---

## Number Formatting

### @format_int

```ori
@format_int (n: int, base: int) -> str
```

Formats integer in given base.

```ori
use std.fmt { format_int }

format_int(255, 16)  // "ff"
format_int(255, 2)   // "11111111"
format_int(255, 10)  // "255"
```

---

### @format_float

```ori
@format_float (n: float, precision: int) -> str
```

Formats float with given decimal places.

```ori
use std.fmt { format_float }

format_float(3.14159, 2)  // "3.14"
format_float(3.14159, 4)  // "3.1416"
```

---

### @format_size

```ori
@format_size (s: Size) -> str
```

Formats byte size with appropriate unit.

```ori
use std.fmt { format_size }

format_size(1024b)       // "1 KB"
format_size(1500000b)    // "1.43 MB"
```

---

### @format_duration

```ori
@format_duration (d: Duration) -> str
```

Formats duration in human-readable form.

```ori
use std.fmt { format_duration }

format_duration(90s)      // "1m 30s"
format_duration(3661s)    // "1h 1m 1s"
```

---

## Examples

### Table formatting

```ori
use std.fmt { pad_right, pad_left }

@format_table (rows: [(str, int)]) -> str = run(
    let header = pad_right("Name", 20, ' ') + pad_left("Score", 10, ' '),
    let separator = "-".repeat(30),
    let body = map(rows, (name, score) ->
        pad_right(name, 20, ' ') + pad_left(str(score), 10, ' ')
    ),
    [header, separator] + body | join("\n"),
)
```

---

## See Also

- [str](../prelude.md#str) — Built-in str function
- [std.text](../std.text/) — Text processing

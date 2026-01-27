# std.text

Text processing utilities.

```ori
use std.text { split, join, replace, trim }
use std.text.regex { Regex, match }
```

**No capability required**

---

## Overview

The `std.text` module provides:

- String manipulation functions
- Text searching and replacing
- Regular expressions (in `std.text.regex`)

---

## Submodules

| Module | Description |
|--------|-------------|
| [std.text.regex](regex.md) | Regular expressions |

---

## Functions

### @split

```ori
@split (s: str, delimiter: str) -> [str]
```

Splits string by delimiter.

```ori
use std.text { split }

split("a,b,c", ",")        // ["a", "b", "c"]
split("hello world", " ")  // ["hello", "world"]
```

---

### @join

```ori
@join (parts: [str], separator: str) -> str
```

Joins strings with separator.

```ori
use std.text { join }

join(["a", "b", "c"], ", ")  // "a, b, c"
join(["hello", "world"], " ")  // "hello world"
```

---

### @replace

```ori
@replace (s: str, old: str, new: str) -> str
```

Replaces all occurrences.

```ori
use std.text { replace }

replace("hello world", "world", "ori")  // "hello ori"
```

---

### @replace_first

```ori
@replace_first (s: str, old: str, new: str) -> str
```

Replaces first occurrence only.

---

### @trim

```ori
@trim (s: str) -> str
```

Removes leading and trailing whitespace.

```ori
use std.text { trim }

trim("  hello  ")  // "hello"
```

---

### @trim_start

```ori
@trim_start (s: str) -> str
```

Removes leading whitespace.

---

### @trim_end

```ori
@trim_end (s: str) -> str
```

Removes trailing whitespace.

---

### @contains

```ori
@contains (s: str, substring: str) -> bool
```

Checks if string contains substring.

```ori
use std.text { contains }

contains("hello world", "world")  // true
contains("hello world", "ori")  // false
```

---

### @starts_with

```ori
@starts_with (s: str, prefix: str) -> bool
```

Checks if string starts with prefix.

---

### @ends_with

```ori
@ends_with (s: str, suffix: str) -> bool
```

Checks if string ends with suffix.

---

### @index_of

```ori
@index_of (s: str, substring: str) -> Option<int>
```

Finds first occurrence of substring.

```ori
use std.text { index_of }

index_of("hello", "l")   // Some(2)
index_of("hello", "x")   // None
```

---

### @repeat

```ori
@repeat (s: str, count: int) -> str
```

Repeats string n times.

```ori
use std.text { repeat }

repeat("ab", 3)  // "ababab"
repeat("-", 10)  // "----------"
```

---

### @reverse

```ori
@reverse (s: str) -> str
```

Reverses string.

```ori
use std.text { reverse }

reverse("hello")  // "olleh"
```

---

### @lines

```ori
@lines (s: str) -> [str]
```

Splits string into lines.

```ori
use std.text { lines }

lines("a\nb\nc")  // ["a", "b", "c"]
```

---

### @words

```ori
@words (s: str) -> [str]
```

Splits string into words (whitespace-separated).

```ori
use std.text { words }

words("hello  world")  // ["hello", "world"]
```

---

## Case Conversion

### @upper

```ori
@upper (s: str) -> str
```

Converts to uppercase.

---

### @lower

```ori
@lower (s: str) -> str
```

Converts to lowercase.

---

### @capitalize

```ori
@capitalize (s: str) -> str
```

Capitalizes first character.

```ori
use std.text { capitalize }

capitalize("hello")  // "Hello"
```

---

### @title_case

```ori
@title_case (s: str) -> str
```

Capitalizes first character of each word.

```ori
use std.text { title_case }

title_case("hello world")  // "Hello World"
```

---

## Examples

### Parsing CSV line

```ori
use std.text { split, trim }

@parse_csv_line (line: str) -> [str] =
    split(line, ",") | map(_, trim)
```

### Building a slug

```ori
use std.text { lower, replace, trim }

@slugify (title: str) -> str =
    title
    | trim(_)
    | lower(_)
    | replace(_, " ", "-")
    | replace(_, "'", "")
```

---

## See Also

- [std.text.regex](regex.md) — Regular expressions
- [std.fmt](../std.fmt/) — String formatting

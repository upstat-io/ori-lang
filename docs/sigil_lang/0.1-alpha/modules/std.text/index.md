# std.text

Text processing utilities.

```sigil
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

```sigil
@split (s: str, delimiter: str) -> [str]
```

Splits string by delimiter.

```sigil
use std.text { split }

split("a,b,c", ",")        // ["a", "b", "c"]
split("hello world", " ")  // ["hello", "world"]
```

---

### @join

```sigil
@join (parts: [str], separator: str) -> str
```

Joins strings with separator.

```sigil
use std.text { join }

join(["a", "b", "c"], ", ")  // "a, b, c"
join(["hello", "world"], " ")  // "hello world"
```

---

### @replace

```sigil
@replace (s: str, old: str, new: str) -> str
```

Replaces all occurrences.

```sigil
use std.text { replace }

replace("hello world", "world", "sigil")  // "hello sigil"
```

---

### @replace_first

```sigil
@replace_first (s: str, old: str, new: str) -> str
```

Replaces first occurrence only.

---

### @trim

```sigil
@trim (s: str) -> str
```

Removes leading and trailing whitespace.

```sigil
use std.text { trim }

trim("  hello  ")  // "hello"
```

---

### @trim_start

```sigil
@trim_start (s: str) -> str
```

Removes leading whitespace.

---

### @trim_end

```sigil
@trim_end (s: str) -> str
```

Removes trailing whitespace.

---

### @contains

```sigil
@contains (s: str, substring: str) -> bool
```

Checks if string contains substring.

```sigil
use std.text { contains }

contains("hello world", "world")  // true
contains("hello world", "sigil")  // false
```

---

### @starts_with

```sigil
@starts_with (s: str, prefix: str) -> bool
```

Checks if string starts with prefix.

---

### @ends_with

```sigil
@ends_with (s: str, suffix: str) -> bool
```

Checks if string ends with suffix.

---

### @index_of

```sigil
@index_of (s: str, substring: str) -> Option<int>
```

Finds first occurrence of substring.

```sigil
use std.text { index_of }

index_of("hello", "l")   // Some(2)
index_of("hello", "x")   // None
```

---

### @repeat

```sigil
@repeat (s: str, count: int) -> str
```

Repeats string n times.

```sigil
use std.text { repeat }

repeat("ab", 3)  // "ababab"
repeat("-", 10)  // "----------"
```

---

### @reverse

```sigil
@reverse (s: str) -> str
```

Reverses string.

```sigil
use std.text { reverse }

reverse("hello")  // "olleh"
```

---

### @lines

```sigil
@lines (s: str) -> [str]
```

Splits string into lines.

```sigil
use std.text { lines }

lines("a\nb\nc")  // ["a", "b", "c"]
```

---

### @words

```sigil
@words (s: str) -> [str]
```

Splits string into words (whitespace-separated).

```sigil
use std.text { words }

words("hello  world")  // ["hello", "world"]
```

---

## Case Conversion

### @upper

```sigil
@upper (s: str) -> str
```

Converts to uppercase.

---

### @lower

```sigil
@lower (s: str) -> str
```

Converts to lowercase.

---

### @capitalize

```sigil
@capitalize (s: str) -> str
```

Capitalizes first character.

```sigil
use std.text { capitalize }

capitalize("hello")  // "Hello"
```

---

### @title_case

```sigil
@title_case (s: str) -> str
```

Capitalizes first character of each word.

```sigil
use std.text { title_case }

title_case("hello world")  // "Hello World"
```

---

## Examples

### Parsing CSV line

```sigil
use std.text { split, trim }

@parse_csv_line (line: str) -> [str] =
    split(line, ",") | map(_, trim)
```

### Building a slug

```sigil
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

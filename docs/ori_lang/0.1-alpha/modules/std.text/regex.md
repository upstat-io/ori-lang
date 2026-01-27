# std.text.regex

Regular expression matching.

```ori
use std.text.regex { Regex, match, find, replace }
```

**No capability required**

---

## Overview

The `std.text.regex` module provides:

- Regular expression compilation and matching
- Find and replace operations
- Capture groups

---

## Types

### Regex

```ori
type Regex
```

A compiled regular expression.

```ori
use std.text.regex { Regex }

let re = Regex.new(r"\d+")?
re.is_match("abc123")  // true
re.find("abc123")      // Some("123")
```

**Methods:**
- `new(pattern: str) -> Result<Regex, RegexError>` — Compile pattern
- `is_match(s: str) -> bool` — Test if string matches
- `find(s: str) -> Option<str>` — Find first match
- `find_all(s: str) -> [str]` — Find all matches
- `captures(s: str) -> Option<Captures>` — Get capture groups
- `replace(s: str, replacement: str) -> str` — Replace first match
- `replace_all(s: str, replacement: str) -> str` — Replace all matches
- `split(s: str) -> [str]` — Split by pattern

---

### Captures

```ori
type Captures = {
    full: str,
    groups: [Option<str>],
    named: {str: str},
}
```

Captured groups from a match.

```ori
use std.text.regex { Regex }

let re = Regex.new(r"(\d{4})-(\d{2})-(\d{2})")?
let caps = re.captures("2024-12-25")?

caps.full       // "2024-12-25"
caps.groups[0]  // Some("2024")
caps.groups[1]  // Some("12")
caps.groups[2]  // Some("25")
```

**Named captures:**

```ori
let re = Regex.new(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})")?
let caps = re.captures("2024-12-25")?

caps.named["year"]   // "2024"
caps.named["month"]  // "12"
caps.named["day"]    // "25"
```

---

### RegexError

```ori
type RegexError =
    | InvalidPattern(message: str, position: int)
```

---

## Functions

### @match

```ori
@match (pattern: str, s: str) -> bool
```

Quick match test without compiling.

```ori
use std.text.regex { match }

match(r"\d+", "abc123")  // true
match(r"^\d+$", "abc")   // false
```

---

### @find

```ori
@find (pattern: str, s: str) -> Option<str>
```

Quick find without compiling.

```ori
use std.text.regex { find }

find(r"\d+", "abc123def")  // Some("123")
```

---

### @replace

```ori
@replace (pattern: str, s: str, replacement: str) -> str
```

Quick replace without compiling.

```ori
use std.text.regex { replace }

replace(r"\s+", "hello   world", " ")  // "hello world"
```

---

## Pattern Syntax

| Pattern | Meaning |
|---------|---------|
| `.` | Any character |
| `\d` | Digit [0-9] |
| `\w` | Word character [a-zA-Z0-9_] |
| `\s` | Whitespace |
| `^` | Start of string |
| `$` | End of string |
| `*` | Zero or more |
| `+` | One or more |
| `?` | Zero or one |
| `{n}` | Exactly n |
| `{n,m}` | Between n and m |
| `[abc]` | Character class |
| `[^abc]` | Negated class |
| `(...)` | Capture group |
| `(?:...)` | Non-capturing group |
| `(?<name>...)` | Named capture |
| `a\|b` | Alternation |

---

## Examples

### Email validation

```ori
use std.text.regex { Regex }

let email_re = Regex.new(r"^[\w.+-]+@[\w.-]+\.[a-zA-Z]{2,}$")?

@is_valid_email (email: str) -> bool =
    email_re.is_match(email)
```

### Parsing log lines

```ori
use std.text.regex { Regex }

type LogEntry = { timestamp: str, level: str, message: str }

let log_re = Regex.new(
    r"(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) " +
    r"(?<level>INFO|WARN|ERROR) " +
    r"(?<message>.*)"
)?

@parse_log (line: str) -> Option<LogEntry> = run(
    let caps = log_re.captures(line)?,
    Some(LogEntry {
        timestamp: caps.named["timestamp"],
        level: caps.named["level"],
        message: caps.named["message"],
    }),
)
```

### Find all URLs

```ori
use std.text.regex { Regex }

let url_re = Regex.new(r"https?://[^\s]+")?

@extract_urls (text: str) -> [str] =
    url_re.find_all(text)
```

### Template replacement

```ori
use std.text.regex { Regex }

let var_re = Regex.new(r"\{\{(\w+)\}\}")?

@render_template (template: str, vars: {str: str}) -> str = run(
    var_re.replace_all(template, |caps| ->
        vars[caps.groups[0] ?? ""] ?? caps.full
    ),
)

render_template("Hello, {{name}}!", {"name": "Alice"})
// "Hello, Alice!"
```

---

## Performance

For repeated matching, compile the regex once:

```ori
// Good: compile once
let re = Regex.new(r"\d+")?
for line in lines do
    if re.is_match(line) then process(line)

// Bad: recompiles each iteration
for line in lines do
    if match(r"\d+", line) then process(line)
```

---

## See Also

- [std.text](index.md) — String utilities
- [std.fmt](../std.fmt/) — String formatting

# Style Guide: What ori fmt Enforces

The Ori formatter produces a single canonical format with minimal configuration. Only line width is configurable (default 100). This guide describes the style enforced by `ori fmt`.

## Philosophy

Like Go's `gofmt`, Ori's formatter eliminates style debates by design:

- **Minimal configuration** — Only line width is configurable (`--width=N`). All other style choices are fixed.
- **Deterministic** — Same input always produces same output.
- **Idempotent** — `format(format(code)) == format(code)`.
- **Width-driven** — Lines break only when exceeding the configured width (default 100).

## Core Rules

| Rule | Value |
|------|-------|
| Indentation | 4 spaces (tabs converted) |
| Line width | 100 characters default (`--width=N` to change) |
| Trailing commas | Required in multi-line, forbidden in single-line |
| Trailing newline | Required (exactly one) |
| Blank lines | One between top-level items, no consecutive |

## Spacing Rules

### Binary Operators

Space around binary operators:

```ori
// Formatted
a + b * c
x == y && z > 0
value | mask
```

### Arrows

Space around arrows:

```ori
// Formatted
x -> x + 1
(a: int, b: int) -> int
@func () -> Result<T, E>
```

### Colons

Space after colons in type annotations and arguments:

```ori
// Formatted
x: int
param: value
{ name: "Alice", age: 30 }
```

### Commas

Space after commas:

```ori
// Formatted
[1, 2, 3]
f(a: 1, b: 2)
(x, y, z)
```

### Delimiters

| Delimiter | Rule | Example |
|-----------|------|---------|
| Parentheses `()` | No space inside | `f(x)`, `(a, b)` |
| Brackets `[]` | No space inside | `[1, 2]`, `items[0]` |
| Struct braces `{}` | Space inside | `Point { x, y }` |
| Empty delimiters | No space | `[]`, `{}`, `()` |

### Other Spacing

| Context | Rule | Example |
|---------|------|---------|
| Field access `.` | No space | `point.x` |
| Range `..`/`..=` | No space | `0..10`, `0..=100` |
| Range step `by` | Space around | `0..100 by 5` |
| Spread `...` | No space after | `[...a, ...b]` |
| Unary operators | No space after | `-x`, `!valid` |
| Error propagation `?` | No space before | `fetch()?` |
| Labels | No space around `:` | `loop:outer` |
| Type conversion `as` | Space around | `42 as float` |
| Visibility `pub` | Space after | `pub @add` |
| Generic bounds | Space after `:`, around `+` | `<T: Clone + Debug>` |
| Sum type variants | Space around `\|` | `Red \| Green \| Blue` |

## Width-Based Breaking

The core principle: **inline if ≤100 characters, break otherwise**.

### Function Signatures

Inline when fits:

```ori
@add (a: int, b: int) -> int = a + b
```

Parameters break one-per-line when signature exceeds 100 characters:

```ori
@send_notification (
    user_id: int,
    notification: Notification,
    preferences: NotificationPreferences,
) -> Result<void, Error> = do_notify()
```

### Function Calls

Inline when fits:

```ori
let result = add(a: 1, b: 2)
```

Arguments break one-per-line when call exceeds 100 characters:

```ori
let result = send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
)
```

### Collections

**Lists** with simple items wrap multiple per line:

```ori
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
]
```

**Lists** with complex items (structs, calls) go one-per-line:

```ori
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
]
```

**Maps**, **tuples**, and **struct literals** break one-per-line:

```ori
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
}
```

### Binary Expressions

Break before operator when exceeding 100 characters:

```ori
let result = first_value + second_value
    - third_value * fourth_value
    + fifth_value / sixth_value
```

### Method Chains

Break at every `.` once any break is needed:

```ori
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b)
```

### Conditionals

Keep `if cond then expr` together, `else` on new line:

```ori
let category =
    if value > 100 then "large"
    else "small"
```

## Always-Stacked Constructs

These are always stacked regardless of width:

| Construct | Example |
|-----------|---------|
| `run` / `try` | Sequential execution |
| `match` | Pattern matching arms |
| `recurse` | Recursive definition |
| `parallel` / `spawn` | Concurrency |
| `nursery` | Structured concurrency |

```ori
let result = run(
    let x = compute(),
    let y = transform(x),
    x + y,
)

let label = match(status,
    Pending -> "waiting",
    Running -> "in progress",
    Complete -> "done",
)
```

## Type Definitions

### Structs

Inline when fits:

```ori
type Point = { x: int, y: int }
```

One field per line when exceeds 100 characters:

```ori
type User = {
    id: int,
    name: str,
    email: str,
    created_at: Timestamp,
}
```

### Sum Types

Inline when fits:

```ori
type Color = Red | Green | Blue
```

One variant per line with leading `|` when exceeds 100 characters:

```ori
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: Modifiers)
    | Scroll(delta_x: float, delta_y: float)
```

## Comments

Comments must appear on their own line. The formatter normalizes spacing:

| Input | Output |
|-------|--------|
| `//comment` | `// comment` |
| `//  comment` | `// comment` |

### Doc Comments

Required order (formatter reorders if wrong):

1. `#` — Description
2. `@param`/`@field` — Parameters/fields
3. `!` — Warning
4. `>` — Example

`@param` order matches function signature order. `@field` order matches struct field order.

```ori
// #Computes the sum of two integers.
// @param a The first operand.
// @param b The second operand.
// !Panics if overflow occurs.
// >add(a: 2, b: 3) -> 5
@add (a: int, b: int) -> int = a + b
```

## Imports

Stdlib first, relative second, blank line between. Items sorted alphabetically:

```ori
use std.collections { HashMap, Set }
use std.math { abs, sqrt }

use "../utils" { format }
use "./helpers" { compute, validate }
```

Break to multiple lines if exceeds 100 characters:

```ori
use std.collections {
    BTreeMap,
    BTreeSet,
    HashMap,
    HashSet,
    LinkedList,
}
```

## Blank Lines

- One after imports block
- One after constants block
- One between top-level declarations
- One between trait/impl methods (except single-method blocks)
- No consecutive blank lines

## Lambdas

No parens for single untyped parameter:

```ori
x -> x + 1
items.map(x -> x * 2)
```

Parens for zero, multiple, or typed parameters:

```ori
() -> 42
(a, b) -> a + b
(x: int) -> int = x * 2
```

## Strings

Never break inside strings. Break the binding instead:

```ori
let message =
    "This is a very long string that exceeds 100 characters but we never break inside"
```

## Summary

The formatter's style is:

1. **Minimal whitespace** — No extra spaces inside delimiters
2. **Consistent spacing** — Space after `:` and `,`, around binary operators
3. **Width-driven breaking** — Break at 100 characters, not arbitrary counts
4. **Logical grouping** — Always-stacked for sequential/concurrent patterns
5. **Trailing commas** — Required in multi-line to simplify diffs

## Normative Reference

The authoritative formatting rules are defined in the [Formatting Specification](../../ori_lang/0.1-alpha/spec/16-formatting.md).

## See Also

- [User Guide](user-guide.md) — Command-line usage
- [Integration Guide](integration.md) — Editor setup
- [Troubleshooting](troubleshooting.md) — Common issues

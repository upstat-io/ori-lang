---
title: "Formatting Rules"
description: "Zero-config code formatting for consistent style."
order: 21
part: "Advanced Patterns"
---

# Formatting Rules

Ori has one canonical code style, enforced by `ori fmt`. No configuration, no debates — just consistent code.

## Core Principles

1. **Zero config** — no options, no style guides to maintain
2. **Width-based** — inline if fits, break otherwise
3. **100 character limit** — hard limit for all lines
4. **Deterministic** — same input always produces same output

## Indentation and Spacing

### Indentation

Four spaces per level, no tabs:

```ori
@example () -> void = {
    let x = 10
    if x > 0 then {
        let y = x * 2
        print(msg: `{y}`)
    }
}
```

### Spacing Around Operators

```ori
// Binary operators
a + b
x == y
m && n

// No space for unary
!condition
-value

// No space around . and ?
point.x
result?
```

### Spacing Around Arrows

```ori
// Lambda arrows
x -> x + 1
(a, b) -> a + b

// Return type arrows
@fn () -> int = ...
```

### Spacing in Delimiters

```ori
// Space after colon
x: int
key: value

// Space after comma
f(a, b, c)
[1, 2, 3]

// No space inside parens/brackets
f(x)
[1, 2]

// Space inside braces (structs)
Point { x, y }

// No space in empty delimiters
[]
{}
()
```

## Width-Based Breaking

Core principle: **inline if ≤100 characters, break otherwise**.

### Inline (Fits)

```ori
assert_eq(actual: result, expected: 10)
Point { x: 0, y: 0 }
let short = [1, 2, 3]
```

### Broken (Exceeds Width)

```ori
send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
)

Point {
    x: some_long_expression,
    y: another_long_expression,
}

let long_list = [
    first_long_item,
    second_long_item,
    third_long_item,
]
```

### Nested Constructs

Each nested construct breaks independently based on its own width:

```ori
// Outer breaks, inner fits
send_email(
    to: recipient,
    subject: "Hello",
    body: format_body(name: user.name, date: today()),
)

// Both break
send_email(
    to: recipient,
    subject: create_subject(
        prefix: notification_type,
        content: message_content,
    ),
    body: body_text,
)
```

## Always-Stacked Constructs

Some constructs always stack, regardless of width:

### run and try

```ori
// Always stacked
{
    let x = compute()
    let y = process(input: x)
    y
}

try {
    let data = fetch()?
    let result = parse(data: data)?
    Ok(result)
}
```

### match Arms

```ori
// One arm per line
match value {
    Some(x) -> process(x: x)
    None -> default_value
}
```

### recurse, parallel, spawn, nursery

```ori
// Always stacked
recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n: n - 1),
)

parallel(
    tasks: task_list,
    max_concurrent: 10,
    timeout: 30s,
)
```

## Breaking Rules by Construct

### Function Parameters/Arguments

One per line when broken:

```ori
// Inline
@add (a: int, b: int) -> int = a + b

// Broken
@send_email (
    recipient: str,
    subject: str,
    body: str,
    attachments: [Attachment],
) -> Result<void, Error> = ...
```

### Generics and Where Clauses

```ori
// Inline
@process<T: Clone> (x: T) -> T = ...

// Broken
@complex<T, U, V> (
    input: T,
    transformer: (T) -> U,
    validator: (U) -> Result<V, Error>,
) -> Result<V, Error>
    where T: Clone + Debug,
          U: Sendable,
          V: Default = ...
```

### Struct Fields

```ori
// Inline
type Point = { x: int, y: int }

// Broken
type User = {
    id: int,
    name: str,
    email: str,
    created_at: DateTime,
}
```

### Sum Type Variants

Always one per line with leading `|`:

```ori
type Status =
    | Pending
    | Active(since: DateTime)
    | Inactive(reason: str)
    | Terminated
```

### Lists

Simple items wrap, complex items stack:

```ori
// Simple items wrap
let numbers = [
    1, 2, 3, 4, 5,
    6, 7, 8, 9, 10,
]

// Complex items stack
let users = [
    User { name: "Alice", age: 30 },
    User { name: "Bob", age: 25 },
    User { name: "Charlie", age: 35 },
]
```

### Chains

Each method on its own line when broken:

```ori
// Inline
numbers.iter().map(transform: x -> x * 2).collect()

// Broken
numbers.iter()
    .filter(predicate: x -> x > 0)
    .map(transform: x -> x * 2)
    .take(count: 10)
    .collect()
```

### Binary Expressions

Break before operator:

```ori
// Inline
let total = a + b + c

// Broken
let total = very_long_first_operand
    + very_long_second_operand
    + very_long_third_operand
```

### Conditionals

```ori
// Inline
if condition then result else alternative

// Broken
if some_complex_condition then
    first_result
else if another_condition then
    second_result
else
    default_result
```

### Lambdas

Break after `->` only for always-stacked patterns:

```ori
// Inline
x -> x + 1

// With run (always stacked)
x -> {
    let y = compute(input: x)
    let z = process(input: y)
    z
}
```

## Trailing Commas

Always on multi-line, forbidden on single-line:

```ori
// Single line — no trailing comma
Point { x: 0, y: 0 }
[1, 2, 3]

// Multi-line — trailing comma required
Point {
    x: 0,
    y: 0,
}

[
    first,
    second,
    third,
]
```

## Blank Lines

### After Import Block

```ori
use std.math { sqrt, abs }
use "./utils" { helper }

@main () -> void = ...
```

### After Constants Block

```ori
let $MAX_SIZE = 100
let $TIMEOUT = 30s

@process () -> void = ...
```

### Between Functions

```ori
@first () -> void = ...

@second () -> void = ...

@third () -> void = ...
```

### Between Trait/Impl Methods

```ori
impl Displayable for Point {
    @display (self) -> str = `({self.x}, {self.y})`

    @format (self, spec: FormatSpec) -> str = ...
}
```

Exception: single-method blocks don't need blank lines.

### No Consecutive Blank Lines

```ori
// BAD
@first () -> void = ...


@second () -> void = ...

// GOOD
@first () -> void = ...

@second () -> void = ...
```

## Comments

Comments must be on their own line:

```ori
// This is valid
let x = 42

// This is NOT valid
let y = 42  // inline comment
```

Space after `//`:

```ori
// Good
//Bad
```

## Specific Constructs

### Type Aliases

```ori
type UserId = int
type Point = { x: int, y: int }
```

### Trait Definitions

```ori
trait Displayable {
    @display (self) -> str
}

trait Container {
    type Item

    @get (self, index: int) -> Option<Self.Item>
    @len (self) -> int
}
```

### Implementations

```ori
impl Displayable for Point {
    @display (self) -> str = `({self.x}, {self.y})`
}

impl<T: Displayable> Displayable for [T] {
    @display (self) -> str = {
        let items = self.iter().map(transform: x -> x.display()).collect()
        `[{items.join(sep: ", ")}]`
    }
}
```

### Tests

```ori
@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5)
    assert_eq(actual: add(a: -1, b: 1), expected: 0)
}

@test_with_mock tests @fetch () -> void =
    with Http = MockHttp { responses: {} } in {
        let result = fetch(url: "/test")
        assert_err(result: result)
    }
```

## Running the Formatter

### Format Files

```bash
ori fmt src/
ori fmt main.ori
```

### Check Without Modifying

```bash
ori fmt --check src/
```

### In CI

```bash
ori fmt --check src/ || exit 1
```

## Quick Reference

### Spacing

| Context | Rule |
|---------|------|
| Binary operators | `a + b` |
| Arrows | `x -> y`, `-> Type` |
| After colon | `x: int` |
| After comma | `a, b, c` |
| Inside parens | `f(x)` |
| Inside struct braces | `Point { x, y }` |
| Empty delimiters | `[]`, `{}`, `()` |

### Breaking

| Width | Action |
|-------|--------|
| ≤100 chars | Inline |
| >100 chars | Break |

### Always Stack

- `run`, `try`
- `match` arms
- `recurse`, `parallel`, `spawn`, `nursery`

### Trailing Commas

| Format | Rule |
|--------|------|
| Single-line | No comma |
| Multi-line | Always comma |

### Blank Lines

| Context | Lines |
|---------|-------|
| After imports | 1 |
| After constants | 1 |
| Between functions | 1 |
| Consecutive max | 1 |


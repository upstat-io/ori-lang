---
title: "Indentation"
description: "Ori Formatter Design — Indentation Rules"
order: 3
---

# Indentation

Ori uses 4-space indentation with no tabs. This document specifies when indentation increases.

## Core Rule

**Indentation increases by 4 spaces when entering a nested scope or broken construct.**

## Indentation Contexts

### Top Level (0 spaces)

```ori
use std.math { sqrt }

let $timeout = 30

type Point = { x: int, y: int }

@add (a: int, b: int) -> int = a + b
```

### Broken Parameters/Arguments (+4 spaces)

When a parameter or argument list breaks, each item is indented:

```ori
@send_notification (
    user_id: int,          // +4
    notification: Notification,
    preferences: Preferences,
) -> Result<void, Error> = do_notify()

let result = process(
    data: input,           // +4
    options: config,
)
```

### Broken Collections (+4 spaces)

Collection contents are indented when broken:

```ori
let numbers = [
    1, 2, 3, 4, 5,         // +4
    6, 7, 8, 9, 10,
]

let config = {
    "timeout": 30,         // +4
    "retries": 3,
}
```

### Struct Fields (+4 spaces)

```ori
type User = {
    id: int,               // +4
    name: str,
    email: str,
}
```

### Sum Type Variants (+4 spaces)

```ori
type Event =
    | Click(x: int, y: int)      // +4
    | KeyPress(key: char)
    | Scroll(delta: float)
```

### Trait/Impl Bodies (+4 spaces)

```ori
trait Printable {
    @to_str (self) -> str        // +4
}

impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }    // +4

    @distance (self, other: Point) -> float = run(
        let dx = self.x - other.x,                      // +8 (nested)
        let dy = self.y - other.y,
        sqrt(float(dx * dx + dy * dy)),
    )
}
```

### Pattern Bodies (+4 spaces)

`run`, `try`, `match`, and other patterns indent their contents:

```ori
let result = run(
    let x = compute(),     // +4
    let y = transform(x),
    x + y,
)

let label = match(status,
    Pending -> "waiting",  // +4
    Running -> "in progress",
    Complete -> "done",
)
```

### Broken Chains (+4 spaces)

```ori
let result = items
    .filter(x -> x > 0)    // +4
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b)
```

### Broken Conditionals (+4 spaces)

```ori
let category =
    if value > 100 then "large"   // +4
    else "small"
```

### Broken Binary Expressions (+4 spaces)

```ori
let result = first_value
    + second_value         // +4
    - third_value
```

### Lambda Bodies (+4 spaces)

When a lambda body breaks after the arrow:

```ori
let process = x ->
    run(                   // +4
        let y = x * 2,     // +8 (nested run)
        validate(y),
    )
```

### Where Clauses (+4 spaces)

```ori
@process<T, U> (items: [T]) -> [U]
    where T: Clone,        // +4
          U: Default = do_it()
```

### Uses Clauses (+4 spaces)

```ori
@complex_op (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger = do_it()   // +4
```

## Nested Indentation

Indentation accumulates with nesting:

```ori
impl Calculator {                              // 0
    @compute (self, input: Data) -> int = run( // +4
        let validated = validate(              // +8
            data: input,                       // +12
            rules: self.rules,
        ),
        let result = process(                  // +8
            input: validated,
            options: Options {                 // +12
                timeout: 30s,                  // +16
                retries: 3,
            },
        ),
        result,                                // +8
    )
}
```

## Alignment

The formatter does **not** align on specific characters. All indentation is based on nesting level only.

```ori
// NO - do not align colons or arrows
let short:   int = 1
let longer:  int = 2

// YES - consistent indentation
let short: int = 1
let longer: int = 2
```

```ori
// NO - do not align match arms
match(value,
    Some(x) -> x,
    None    -> 0,
)

// YES - consistent indentation
match(value,
    Some(x) -> x,
    None -> 0,
)
```

## Continuation Lines

When a single statement spans multiple lines (not due to entering a nested construct), continuation lines are indented +4:

```ori
// Return type on continuation line
@long_function (params: Params)
    -> Result<VeryLongTypeName, Error> = body

// Binary expression continuation
let total = base_amount
    + tax_amount
    + shipping_amount

// Condition continuation (rare - conditions should be short)
let valid = is_authenticated(user)
    && has_permission(user, resource)
    && is_not_expired(token)
```

## Tab Characters

Tabs are **never** produced by the formatter. If input contains tabs, they are converted to 4 spaces.

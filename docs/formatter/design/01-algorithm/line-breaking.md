---
title: "Line Breaking"
description: "Ori Formatter Design — Line Breaking Rules"
order: 2
---

# Line Breaking

This document specifies when and how the formatter breaks lines.

## Core Principle

**Break only when exceeding 100 characters.** There are no arbitrary thresholds based on item count.

## Breaking Points by Construct

### Function Signatures

**Trigger**: Signature exceeds 100 characters.

**Breaking points** (in order of application):
1. Parameters — one per line
2. Return type — own line if `) -> Type =` still exceeds 100

```ori
// Inline (fits)
@add (a: int, b: int) -> int = a + b

// Break parameters
@send_notification (
    user_id: int,
    notification: Notification,
    preferences: NotificationPreferences,
) -> Result<void, Error> = do_notify()

// Break parameters AND return type
@very_long_function_name (
    first_parameter: int,
    second_parameter: str,
) -> Result<HashMap<UserId, NotificationPreferences>, NotificationServiceError> =
    do_work()
```

### Function Calls

**Trigger**: Call exceeds 100 characters.

**Breaking point**: Arguments — one per line.

```ori
// Inline
let result = add(a: 1, b: 2)

// Broken
let result = send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
)
```

Even single-argument calls break if they exceed 100:

```ori
let result = process(
    data: some_very_long_variable_name_that_pushes_past_the_limit,
)
```

### Generics

**Trigger**: Generic parameter list exceeds 100 characters.

**Breaking point**: Type parameters — one per line.

```ori
// Inline
@transform<T, U> (a: T) -> U = do_it()

// Broken
@complex_transform<
    InputType,
    OutputType,
    ErrorType,
    ConfigurationType,
> (input: InputType) -> Result<OutputType, ErrorType> = do_it()
```

### Where Clauses

**Trigger**: Signature with where clause exceeds 100 characters.

**Breaking points**:
1. `where` keyword — moves to new line
2. Constraints — one per line if multiple

```ori
// Inline (fits)
@sort<T> (items: [T]) -> [T] where T: Comparable = do_sort()

// Broken
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone + Debug,
          U: Default + Printable = do_it()
```

### Capabilities

**Trigger**: Signature with `uses` clause exceeds 100 characters.

**Breaking points**:
1. `uses` keyword — moves to new line
2. Capabilities — one per line only if capability list itself exceeds 100 (rare)

```ori
// Inline
@fetch (url: str) -> Result<str, Error> uses Http = http_get(url)

// uses on new line
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache = do_it()
```

### Collections

#### Lists

**Trigger**: List exceeds 100 characters.

**Breaking behavior**: Depends on item complexity.

**Simple items** (literals, identifiers) — wrap multiple per line:

```ori
// Inline
let nums = [1, 2, 3, 4, 5]

// Wrapped (fill as many as fit per line)
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
]
```

**Complex items** (structs, calls, nested collections) — one per line:

```ori
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
    User { id: 3, name: "Charlie" },
]

let tasks = [
    fetch_user(id: 1),
    fetch_user(id: 2),
    fetch_user(id: 3),
]
```

#### Maps

**Trigger**: Map exceeds 100 characters.

**Breaking behavior**: One entry per line.

```ori
// Inline
let m = {"a": 1, "b": 2}

// Broken
let m = {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
}
```

#### Tuples

**Trigger**: Tuple exceeds 100 characters.

**Breaking behavior**: One element per line.

```ori
// Inline
let pair = (1, "hello")

// Broken
let data = (
    first_very_long_value,
    second_very_long_value,
)
```

### Struct Literals

**Trigger**: Struct literal exceeds 100 characters.

**Breaking behavior**: One field per line.

```ori
// Inline
let p = Point { x: 0, y: 0 }

// Broken
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
}
```

### Type Definitions

#### Structs

**Trigger**: Struct definition exceeds 100 characters.

**Breaking behavior**: One field per line.

```ori
// Inline
type Point = { x: int, y: int }

// Broken
type UserProfile = {
    id: int,
    name: str,
    email: str,
    created_at: Timestamp,
}
```

#### Sum Types

**Trigger**: Sum type exceeds 100 characters.

**Breaking behavior**: One variant per line with leading `|`.

```ori
// Inline
type Color = Red | Green | Blue

// Broken
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: Modifiers)
    | Scroll(delta_x: float, delta_y: float)
```

### Chains

**Trigger**: Chain exceeds 100 characters.

**Breaking behavior**: Each `.method()` on its own line.

```ori
// Inline
let result = items.filter(x -> x > 0).map(x -> x * 2)

// Broken (once any break needed, break all)
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b)
```

### Conditionals

**Trigger**: Conditional exceeds 100 characters.

**Breaking behavior**: Keep `if cond then expr` together, break at `else`.

```ori
// Inline
let sign = if x > 0 then "positive" else "negative"

// Broken
let category =
    if value > 100 then "large"
    else "small"

// Chained
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

Branch bodies break independently:

```ori
let result =
    if condition then compute_simple(x: value)
    else compute_with_many_args(
        input: data,
        fallback: default,
        options: config,
    )
```

### Binary Expressions

**Trigger**: Expression exceeds 100 characters.

**Breaking point**: Before the operator.

```ori
// Inline
let result = a + b * c - d

// Broken
let result = first_value + second_value
    - third_value * fourth_value
    + fifth_value / sixth_value
```

### Lambdas

**Trigger**: Lambda has always-stacked body (`run`, `try`, `match`).

**Breaking point**: After the arrow `->`, but only for always-stacked patterns.

```ori
// Inline - fits
items.map(x -> x * 2)

// Long lambda - break the CALL, not the lambda
items.map(
    x -> compute_something_with_long_name(input: x, options: defaults),
)

// Always-stacked body - break after arrow
let process = x ->
    run(
        let doubled = x * 2,
        let validated = validate(doubled),
        validated,
    )

// In call context with always-stacked body
items.map(
    x ->
        run(
            let y = x * 2,
            validate(y),
        ),
)
```

### Imports

**Trigger**: Import exceeds 100 characters.

**Breaking behavior**: One item per line.

```ori
// Inline
use std.math { abs, sqrt }

// Broken
use std.collections {
    HashMap,
    HashSet,
    BTreeMap,
    BTreeSet,
}
```

## Non-Breaking Constructs

These never break internally regardless of width:

| Construct | Behavior |
|-----------|----------|
| String literals | Never break inside; break the binding if needed |
| Template strings | Never break inside; break the binding if needed |
| Identifiers | Never break |
| Literals | Never break |

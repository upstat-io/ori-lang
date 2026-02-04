---
title: "Expressions"
description: "Ori Formatter Design — Expression Formatting"
order: 3
section: "Constructs"
---

# Expressions

Formatting rules for expressions: function calls, method chains, conditionals, lambdas, binary expressions, and bindings.

## Function Calls

### Inline Format

Used when the call fits in 100 characters:

```ori
let result = add(a: 1, b: 2)
let user = fetch_user(id: 42)
let msg = format(template: "Hello, {}", value: name)
let result = send_email(to: recipient, subject: title, body: content)
```

### Broken Arguments

When the call exceeds 100 characters, break arguments one per line:

```ori
let result = send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
    retry_count: 3,
)
```

Even single-argument calls break if exceeding 100:

```ori
let result = process(
    data: some_very_long_variable_name_that_pushes_past_the_limit,
)
```

### Nested Calls

Nested calls break independently based on their own width:

```ori
// Inner call fits - stays inline
let result = process(
    data: transform(input: fetch(url: endpoint), options: defaults),
    config: settings,
)

// Inner call exceeds 100 - it breaks too
let result = process(
    data: transform(
        input: fetch(url: api_endpoint),
        options: default_transform_options,
        validator: schema_validator,
    ),
    config: settings,
)
```

## Method Chains

> **Note:** The `MethodChainRule` infrastructure exists in `ori_fmt/src/rules/method_chain.rs` but is not yet invoked by the formatter. The patterns below show the intended behavior.

### Inline Format

Used when the chain fits in 100 characters:

```ori
let result = items.filter(x -> x > 0).map(x -> x * 2)
```

### Broken Chain

When a chain exceeds 100 characters, break at every `.`:

```ori
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b)
```

**Important**: Once any break is needed, *all* calls in the chain break. No partial breaking (all-or-nothing rule).

### Chains Starting from Calls

```ori
let users = fetch_all_users()
    .filter(user -> user.is_active)
    .map(user -> user.name)
    .collect()
```

### Mixed Access and Calls

```ori
let name = user.profile
    .preferences
    .display_name
    .unwrap_or(default: "Anonymous")
```

## Conditionals

### Inline Format

Used when the conditional fits in 100 characters:

```ori
let sign = if x > 0 then "positive" else "negative"
let abs = if n >= 0 then n else -n
let max = if a > b then a else b
```

### Broken Format

Keep `if cond then expr` together, break at `else`:

```ori
let category =
    if value > 100 then "large"
    else "small"
```

### Chained Else-If

```ori
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

### Branch Bodies Break Independently

```ori
let result =
    if condition then compute_simple(x: value)
    else compute_with_many_args(
        input: data,
        fallback: default,
        options: config,
    )
```

### Complex Conditions

Long conditions break before operators:

```ori
let valid =
    if is_authenticated(user)
        && has_permission(user, resource)
        && is_not_expired(token)
    then allow_access()
    else deny_access()
```

## Lambdas

### Single Parameter (No Parens)

```ori
x -> x + 1
items.map(x -> x * 2)
users.filter(user -> user.is_active)
```

### Multiple Parameters (Parens Required)

```ori
(a, b) -> a + b
items.fold(0, (acc, x) -> acc + x)
```

### Zero Parameters

```ori
() -> 42
() -> generate_id()
```

### Typed Parameters

```ori
(x: int) -> int = x * 2
(input: str, config: Config) -> Result<Output, Error> = process(input, config)
```

### Always-Stacked Body - Break After Arrow

Break after `->` **only** when the body is an always-stacked pattern (`run`, `try`, `match`):

```ori
let process = x ->
    run(
        let doubled = x * 2,
        let validated = validate(doubled),
        validated,
    )
```

### Lambda in Call Context

If the lambda fits, inline:

```ori
items.map(x -> x * 2)
```

If the lambda is too long, break the **call argument**, not the lambda itself:

```ori
// Long lambda - break the call
items.map(
    x -> compute_something_complex(input: x, options: defaults),
)
```

Break after `->` only for always-stacked bodies:

```ori
// Always-stacked body - break after arrow
items.map(
    x ->
        run(
            let y = x * 2,
            validate(y),
        ),
)
```

### Multiple Params with Complex Body

```ori
items.fold(
    0,
    (acc, x) ->
        run(
            let computed = compute(x),
            acc + computed,
        ),
)
```

## Binary Expressions

### Inline Format

Used when the expression fits in 100 characters:

```ori
let result = a + b * c - d
let valid = x > 0 && x < 100
let combined = first || second && third
```

### Break Before Operator

When exceeding 100 characters, break before the operator:

```ori
let result = first_value + second_value
    - third_value * fourth_value
    + fifth_value / sixth_value

let valid = is_authenticated(user)
    && has_permission(user, resource)
    && is_not_expired(token)
```

### Precedence Preserved

Breaking doesn't change precedence. Parentheses are preserved:

```ori
let result = (first_value + second_value)
    * (third_value - fourth_value)
```

## Bindings

### Simple Bindings

```ori
let x = 42
let $constant = 100
let name: str = "Alice"
```

### Destructuring - Inline

```ori
let { x, y } = point
let (first, second) = pair
let [$head, ..tail] = items
let { position: { x, y }, velocity } = entity
```

### Destructuring - Broken

When exceeding 100 characters:

```ori
let {
    id,
    name,
    email,
    preferences,
    created_at,
} = user
```

### Binding with Long Value

If the value is long, it may break:

```ori
let result = compute_something_with_many_parameters(
    input: data,
    options: config,
    fallback: default_value,
)
```

### Binding with Long Type Annotation

```ori
let handler: (Request, Context) -> Result<Response, Error> =
    create_handler(config: settings)
```

## Indexing

Always inline:

```ori
let first = items[0]
let last = items[# - 1]
let value = map["key"]
```

## Field Access

Always inline:

```ori
let x = point.x
let name = user.profile.name
```

## Type Conversions

Always inline:

```ori
let f = 42 as float
let n = "42" as? int
let s = value as str
```

---
title: "Patterns"
description: "Ori Formatter Design — Pattern Formatting"
order: 4
section: "Constructs"
---

# Patterns

Formatting rules for compiler-recognized patterns: blocks, `try`, `match`, `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`, and `nursery`.

## Blocks and try

### Block Expressions

Block expressions use `{ }` with newline-separated expressions. The last expression is the block's value. Multi-line blocks are **always** stacked regardless of width.

```ori
// ALWAYS this format for multi-line blocks
let result = {
    let x = compute()
    let y = transform(x)
    x + y
}

// Short blocks may use comma-separated one-liner
let result = { let x = 1, x + 1 }
```

### Contracts on Functions

Contracts (`pre()`/`post()`) appear on the **function declaration**, between the signature and `=`:

```ori
@divide (a: int, b: int) -> int
    pre(b != 0) = a / b

@divide (a: int, b: int) -> int
    pre(b != 0 | "divisor cannot be zero") = a / b

@compute (n: int) -> int
    post(result -> result >= 0) = {
    let value = compute_inner(n)
    value
}

@compute_sqrt (n: float) -> float
    pre(n >= 0.0 | "cannot take sqrt of negative")
    post(result -> result >= 0.0) = {
    let result = sqrt(n)
    result
}
```

### Long Conditions Break

Binary expressions in contracts break before operators:

```ori
@process (data: Data) -> Output
    pre(data.is_valid()
        && data.size() > 0
        && data.size() < max_size) = process_inner(data)
```

### try Blocks

`try` uses block syntax with `?` propagation:

```ori
let result = try {
    let data = fetch(url: endpoint)?
    let parsed = parse(input: data)?
    Ok(parsed)
}
```

## match

### Scrutinee Before Block, Arms Below

Multi-line `match` has arms stacked with trailing commas. Short matches (all simple expressions, no guards, fits in line width) may be single-line:

```ori
// Single-line (short, simple)
let label = match b { true -> "yes", false -> "no" };

// Multi-line
let label = match status {
    Pending -> "waiting",
    Running -> "in progress",
    Complete -> "done",
    Failed -> "error",
}
```

### Arms with Longer Bodies

```ori
let message = match event {
    Click(x, y) -> format(template: "Clicked at ({}, {})", x, y),
    KeyPress(key, mods) -> format(template: "Key: {} with {:?}", key, mods),
    _ -> "unknown event",
}
```

### Arms with Long Calls

When an arm body has a long function call, break the call **arguments** (not after `->`):

```ori
let result = match event {
    Click(x, y) -> handle_click_with_long_name(
        x: x,
        y: y,
        options: defaults,
    ),
    KeyPress(key) -> handle_key(key),
}
```

### Arms with Always-Stacked Bodies

Break after `->` only when the body is an always-stacked pattern (block, `try`, `match`):

```ori
let result = match input {
    Some(data) -> {
        let validated = validate(data);
        transform(validated)
    },
    None -> default_value,
}
```

### Guards

```ori
let category = match n {
    x if x < 0 -> "negative",
    0 -> "zero",
    x if x < 10 -> "small",
    _ -> "large",
}
```

## recurse

### Always Stacked

`recurse` is always stacked with named parameters:

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)

@fib (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
)

@parallel_fib (n: int) -> int = recurse(
    condition: n <= 20,
    base: sequential_fib(n),
    step: self(n - 1) + self(n - 2),
    parallel: 20,
)
```

## parallel and spawn

### Always Stacked

```ori
let results = parallel(
    tasks: [fetch(url: "/a"), fetch(url: "/b"), fetch(url: "/c")],
)

let results = parallel(
    tasks: [fetch(url: "/a"), fetch(url: "/b"), fetch(url: "/c")],
    max_concurrent: 2,
    timeout: 30s,
)
```

### Task List Follows List Rules

Short task list stays inline; long list wraps:

```ori
let results = parallel(
    tasks: [
        fetch_user(id: 1),
        fetch_user(id: 2),
        fetch_user(id: 3),
        fetch_user(id: 4),
        fetch_user(id: 5),
    ],
    max_concurrent: 3,
)
```

### spawn

Fire-and-forget, same formatting:

```ori
spawn(
    tasks: [send_email(to: user), log_event(event: action)],
)
```

## timeout and cache

### Stacked When Multiple Params

```ori
let result = timeout(
    op: fetch(url: slow_endpoint),
    after: 5s,
)

let user = cache(
    key: `user:{user_id}`,
    op: fetch_user(id: user_id),
    ttl: 5m,
)
```

### Inline Only If Very Short (Rare)

```ori
let value = cache(key: "k", op: get(), ttl: 1m)
```

## with Expressions

### Inline If Short

```ori
let result = with Http = mock_http in fetch(url: "/api")
```

### Break at in

When exceeding 100 characters:

```ori
let result =
    with Http = MockHttp { responses: default_responses }
    in fetch_user_data(user_id: current_user)
```

### Multiple Capabilities

```ori
let result =
    with Http = mock_http,
         Logger = mock_logger
    in perform_operation(input: data)
```

### Nested with

```ori
let result =
    with Http = mock_http in
        with Cache = mock_cache in
            fetch_cached(url: endpoint)
```

## for Loops

### Inline If Short

```ori
for x in items do print(msg: x)

let doubled = for x in items yield x * 2

let positives = for x in items if x > 0 yield x
```

### Break for Long Body

```ori
for user in users do
    process_user(user: user, options: default_options)

let results = for item in items yield
    transform(input: item, config: default_config)
```

### Nested for

```ori
for x in 0..10 do
    for y in 0..10 do
        plot(x: x, y: y)
```

### Labeled Loops

```ori
loop:outer {
    loop:inner {
        if condition then break:outer result
        else continue:inner
    }
}
```

## nursery

### Always Stacked

```ori
let results = nursery(
    body: n -> {
        n.spawn(task: fetch(url: "/a"))
        n.spawn(task: fetch(url: "/b"))
        n.spawn(task: fetch(url: "/c"))
    },
    on_error: CancelRemaining,
    timeout: 30s,
)
```

### Complex Body

When the body lambda is complex, break after arrow:

```ori
let results = nursery(
    body: n -> {
        let urls = generate_urls(count: 10)
        for url in urls do n.spawn(task: fetch(url: url))
    },
    on_error: CollectAll,
)
```

## Channel Constructors

### Typically Inline

Channel constructors are typically short:

```ori
let (tx, rx) = channel<int>(buffer: 10)
let (tx, rx) = channel_in<Message>(buffer: 100)
let (tx, rx) = channel_out<Event>(buffer: 50)
let (tx, rx) = channel_all<Data>(buffer: 25)
```

## catch

### Stacked Format

```ori
let result = catch(
    expr: potentially_panicking_operation(),
)
```

---
title: "Patterns"
description: "Ori Formatter Design — Pattern Formatting"
order: 4
section: "Constructs"
---

# Patterns

Formatting rules for compiler-recognized patterns: `run`, `try`, `match`, `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`, and `nursery`.

## run and try

### Always Stacked

`run` and `try` are **always** stacked regardless of width. They never appear inline.

```ori
// ALWAYS this format
let result = run(
    let x = compute(),
    let y = transform(x),
    x + y,
)

// NEVER inline (even if short)
// let result = run(let x = 1, x + 1)  // NO
```

### With Contracts (Future)

> **Note:** Pre-check and post-check formatting is defined in the spec but not yet implemented in the formatter. The patterns below show the intended behavior.

Pre-check and post-check appear as named arguments:

```ori
let result = run(
    pre_check: b != 0,
    a / b,
)

let result = run(
    pre_check: b != 0 | "divisor cannot be zero",
    a / b,
)

let result = run(
    let value = compute(),
    post_check: result -> result >= 0,
    value,
)

let result = run(
    pre_check: n >= 0.0 | "cannot take sqrt of negative",
    let result = compute_sqrt(n),
    post_check: result -> result >= 0.0,
    result,
)
```

### Long Conditions Break

Binary expressions in contracts break before operators:

```ori
let result = run(
    pre_check: data.is_valid()
        && data.size() > 0
        && data.size() < max_size,
    process(data),
)
```

### try Pattern

Same rules as `run`, with `?` propagation:

```ori
let result = try(
    let data = fetch(url: endpoint)?,
    let parsed = parse(input: data)?,
    Ok(parsed),
)
```

## match

### Scrutinee on First Line, Arms Below

`match` always has arms stacked, never inline:

```ori
let label = match(status,
    Pending -> "waiting",
    Running -> "in progress",
    Complete -> "done",
    Failed -> "error",
)
```

### Arms with Longer Bodies

```ori
let message = match(event,
    Click(x, y) -> format(template: "Clicked at ({}, {})", x, y),
    KeyPress(key, mods) -> format(template: "Key: {} with {:?}", key, mods),
    _ -> "unknown event",
)
```

### Arms with Long Calls

When an arm body has a long function call, break the call **arguments** (not after `->`):

```ori
let result = match(event,
    Click(x, y) -> handle_click_with_long_name(
        x: x,
        y: y,
        options: defaults,
    ),
    KeyPress(key) -> handle_key(key),
)
```

### Arms with Always-Stacked Bodies

Break after `->` only when the body is an always-stacked pattern (`run`, `try`, `match`):

```ori
let result = match(input,
    Some(data) ->
        run(
            let validated = validate(data),
            transform(validated),
        ),
    None -> default_value,
)
```

### Guards

```ori
let category = match(n,
    x if x < 0 -> "negative",
    0 -> "zero",
    x if x < 10 -> "small",
    _ -> "large",
)
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
loop:outer(0, n ->
    loop:inner(0, m ->
        if condition then break:outer(result)
        else continue:inner,
    ),
)
```

## nursery

### Always Stacked

```ori
let results = nursery(
    body: n -> run(
        n.spawn(task: fetch(url: "/a")),
        n.spawn(task: fetch(url: "/b")),
        n.spawn(task: fetch(url: "/c")),
    ),
    on_error: CancelRemaining,
    timeout: 30s,
)
```

### Complex Body

When the body lambda is complex, break after arrow:

```ori
let results = nursery(
    body: n ->
        run(
            let urls = generate_urls(count: 10),
            for url in urls do n.spawn(task: fetch(url: url)),
        ),
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

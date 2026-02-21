---
title: "Concurrency"
description: "Parallel execution, spawn, timeout, and structured concurrency."
order: 14
part: "Effects and Concurrency"
---

# Concurrency

Ori provides structured concurrency primitives that are safe by design. This guide covers parallel execution patterns, timeouts, and nurseries.

## The parallel Pattern

Run multiple tasks concurrently and collect all results:

```ori
@fetch_all_users (ids: [int]) -> [Result<User, Error>] uses Http, Async =
    parallel(
        tasks: for id in ids yield () -> fetch_user(id: id),
        max_concurrent: 10,
        timeout: 30s,
    )
```

Breaking this down:
- `tasks:` — a list of functions to execute (note: functions, not values)
- `max_concurrent:` — at most 10 tasks run simultaneously
- `timeout:` — if not done in 30 seconds, cancel remaining and return

The result is `[Result<T, E>]` — each task's result is captured individually.

### Creating Task Lists

Tasks are zero-argument functions that will be called concurrently:

```ori
// From a for expression
parallel(
    tasks: for url in urls yield () -> fetch(url: url),
    max_concurrent: 5,
    timeout: 60s,
)

// Explicit list
parallel(
    tasks: [
        () -> fetch_user(id: 1),
        () -> fetch_user(id: 2),
        () -> fetch_user(id: 3),
    ],
    max_concurrent: 3,
    timeout: 30s,
)
```

### Working with Results

```ori
@fetch_users_summary (ids: [int]) -> UsersSummary uses Http, Async, Logger = {
    Logger.info(msg: `Fetching {len(collection: ids)} users`);

    let results = parallel(
        tasks: for id in ids yield () -> fetch_user(id: id)
        max_concurrent: 20
        timeout: 60s
    );

    // Count successes and failures
    let successes = results.iter().filter(predicate: r -> is_ok(result: r)).count();
    let failures = len(collection: results) - successes;

    Logger.info(msg: `Fetched {successes} users, {failures} failed`);

    // Extract successful users
    let users = for result in results
        if is_ok(result: result)
        yield match result {
            Ok(u) -> u
            Err(_) -> continue
        };

    UsersSummary {
        users
        total_requested: len(collection: ids)
        total_fetched: successes
        total_failed: failures
    }
}
```

### Parallel with Different Task Types

When tasks return different types, unify them:

```ori
type DataBundle = { user: User, orders: [Order], preferences: Preferences }

@fetch_user_bundle (user_id: int) -> Result<DataBundle, Error> uses Http, Async = {
    // Fetch all data in parallel
    let results = parallel(
        tasks: [
            () -> fetch_user(id: user_id).map(transform: u -> UserData(u))
            () -> fetch_orders(user_id: user_id).map(transform: o -> OrderData(o))
            () -> fetch_preferences(user_id: user_id).map(transform: p -> PrefData(p))
        ]
        max_concurrent: 3
        timeout: 10s
    );

    // Extract results
    let user = match results[0] { Ok(UserData(u)) -> u, _ -> Err(Error { message: "Failed to fetch user" })? };
    let orders = match results[1] { Ok(OrderData(o)) -> o, _ -> Err(Error { message: "Failed to fetch orders" })? };
    let prefs = match results[2] { Ok(PrefData(p)) -> p, _ -> Err(Error { message: "Failed to fetch preferences" })? };

    Ok(DataBundle { user, orders, preferences: prefs })
}
```

## The spawn Pattern

For fire-and-forget operations:

```ori
@send_notifications (events: [Event]) -> void uses Http, Async, Logger =
    spawn(
        tasks: for event in events yield () -> {
            let result = send_notification(event: event);
            match result {
                Ok(_) -> ()
                Err(e) -> Logger.warn(msg: `Failed to send notification: {e}`)
            }
        },
        max_concurrent: 50,
    )
```

`spawn` returns `void` — you don't wait for results. Useful for:
- Sending emails
- Logging to external services
- Cache warming
- Any background work where you don't need the result

### spawn vs parallel

| Feature | `parallel` | `spawn` |
|---------|-----------|---------|
| Returns | `[Result<T, E>]` | `void` |
| Waits for completion | Yes | No |
| Has timeout | Yes | No |
| Use case | Need results | Fire and forget |

## The timeout Pattern

Limit how long an operation can take:

```ori
@fetch_with_fallback (url: str, fallback: str) -> str uses Http, Async = {
    let result = timeout(
        op: Http.get(url: url)
        after: 5s
    );

    match result {
        Ok(response) -> response.body
        Err(_) -> fallback
    }
}
```

If the operation takes longer than the specified duration, it's cancelled and returns a timeout error.

### Timeout with Complex Operations

```ori
@fetch_with_retries (url: str) -> Result<str, Error> uses Http, Async = {
    let attempts = [1, 2, 3];

    for attempt in attempts do {
        let result = timeout(
            op: Http.get(url: url)
            after: 5s
        )

        match result {
            Ok(response) -> return Ok(response.body)
            Err(_) -> Logger.warn(msg: `Attempt {attempt} timed out`)
        }
    }

    Err(Error { message: "All attempts failed" })
}
```

## Structured Concurrency with nursery

For more control over concurrent tasks, use `nursery`:

```ori
@process_batch (items: [Item]) -> BatchResult uses Async, Logger = {
    let results = nursery(
        body: n -> {
            for item in items do
                n.spawn(task: () -> process_item(item: item))
        }
        on_error: CollectAll
        timeout: 60s
    );

    BatchResult {
        processed: results.iter().filter(predicate: r -> is_ok(result: r)).count()
        failed: results.iter().filter(predicate: r -> is_err(result: r)).count()
        results
    }
}
```

### The Nursery Body

The `body` parameter receives a nursery handle `n` that you use to spawn tasks:

```ori
nursery(
    body: n -> {
        // Spawn tasks dynamically
        n.spawn(task: () -> fetch_user(id: 1))
        n.spawn(task: () -> fetch_user(id: 2))

        // Spawn based on data
        for id in user_ids do
            n.spawn(task: () -> fetch_user(id: id))

        // Conditional spawning
        if include_admin then
            n.spawn(task: () -> fetch_admin_data())
    },
    on_error: CollectAll,
    timeout: 30s,
)
```

### Error Modes

The `on_error` parameter controls what happens when a task fails:

#### CancelRemaining

Cancel pending tasks, let running tasks finish:

```ori
nursery(
    body: n -> for item in items do n.spawn(task: () -> process(item: item)),
    on_error: CancelRemaining,
    timeout: 30s,
)
```

Use when: One failure means the rest of the batch isn't useful.

#### CollectAll

Wait for all tasks regardless of errors:

```ori
nursery(
    body: n -> for item in items do n.spawn(task: () -> process(item: item)),
    on_error: CollectAll,
    timeout: 30s,
)
```

Use when: You want to process as much as possible and collect errors.

#### FailFast

Cancel everything immediately on first error:

```ori
nursery(
    body: n -> for item in items do n.spawn(task: () -> process(item: item)),
    on_error: FailFast,
    timeout: 30s,
)
```

Use when: Any error is critical and continuing is dangerous.

### Error Mode Comparison

| Mode | On First Error | Running Tasks | Pending Tasks |
|------|---------------|---------------|---------------|
| `CancelRemaining` | Continue | Finish | Cancel |
| `CollectAll` | Continue | Finish | Continue |
| `FailFast` | Stop | Cancel | Cancel |

### Nursery Guarantees

Nurseries provide structured concurrency guarantees:

1. **No orphan tasks** — All spawned tasks complete before the nursery returns
2. **Proper cleanup** — Cancelled tasks are properly cleaned up
3. **Error propagation** — Errors are collected and returned
4. **Timeout safety** — Timeouts cancel all tasks cleanly

### Dynamic Task Spawning

Unlike `parallel`, nurseries allow spawning tasks based on intermediate results:

```ori
@crawl_pages (start_url: str, max_depth: int) -> [Page] uses Http, Async = {
    let visited: Set<str> = Set.new();
    let pages: [Page] = [];

    nursery(
        body: n -> {
            @crawl (url: str, depth: int) -> void = {
                if depth > max_depth || visited.contains(value: url) then return ()

                visited = visited.insert(value: url);
                let page = fetch_page(url: url)?;
                pages = [...pages, page];

                // Spawn child tasks for links
                for link in page.links do
                    n.spawn(task: () -> crawl(url: link, depth: depth + 1))
            }

            n.spawn(task: () -> crawl(url: start_url, depth: 0))
        }
        on_error: CollectAll
        timeout: 300s
    );

    pages
}
```

## Combining Patterns

### Parallel with Timeout

```ori
@fetch_with_individual_timeouts (urls: [str]) -> [Result<str, Error>] uses Http, Async =
    parallel(
        tasks: for url in urls yield () -> timeout(
            op: Http.get(url: url),
            after: 5s,
        ),
        max_concurrent: 10,
        timeout: 60s,  // Overall timeout
    )
```

### Nursery with Parallel Subtasks

```ori
@process_batches (batches: [[Item]]) -> [[Result<Output, Error>]] uses Async = {
    nursery(
        body: n -> for batch in batches do
            n.spawn(task: () -> parallel(
                tasks: for item in batch yield () -> process_item(item: item)
                max_concurrent: 5
                timeout: 30s
            ))
        on_error: CollectAll
        timeout: 300s
    )
}
```

## Testing Concurrent Code

### Testing parallel

```ori
@test_fetch_all_users tests @fetch_all_users () -> void =
    with Http = MockHttp {
        responses: {
            "/api/users/1": `{"id": 1, "name": "Alice"}`,
            "/api/users/2": `{"id": 2, "name": "Bob"}`,
        },
    } in {
        let results = fetch_all_users(ids: [1, 2]);
        assert_eq(actual: len(collection: results), expected: 2);
        assert_ok(result: results[0]);
        assert_ok(result: results[1])
    }
```

### Testing Timeout Behavior

```ori
@test_timeout_returns_error tests @fetch_with_timeout () -> void =
    with Http = MockHttp {
        responses: {},
        delay: 10s,  // Simulate slow response
    } in {
        let result = fetch_with_timeout(url: "/slow", timeout_duration: 1s);
        assert_err(result: result)
    }
```

### Testing Error Modes

```ori
@test_fail_fast_cancels tests @process_batch () -> void =
    with Processor = MockProcessor {
        responses: {
            1: Ok("success"),
            2: Err("failure"),
            3: Ok("success"),
        },
    } in {
        let result = process_batch_fail_fast(ids: [1, 2, 3]);
        // With FailFast, we might not process all items
        assert(condition: len(collection: result.results) <= 3)
    }
```

## Best Practices

### Set Reasonable Timeouts

Always set timeouts for concurrent operations:

```ori
// BAD: No timeout — can hang forever
parallel(
    tasks: [...],
    max_concurrent: 10,
)

// GOOD: Explicit timeout
parallel(
    tasks: [...],
    max_concurrent: 10,
    timeout: 30s,
)
```

### Limit Concurrency

Don't overwhelm resources:

```ori
// BAD: Unbounded concurrency
parallel(
    tasks: for url in thousand_urls yield () -> fetch(url: url),
    max_concurrent: 1000,  // Too many!
    timeout: 60s,
)

// GOOD: Reasonable limit
parallel(
    tasks: for url in thousand_urls yield () -> fetch(url: url),
    max_concurrent: 20,  // Controlled
    timeout: 60s,
)
```

### Prefer parallel Over spawn

Use `parallel` when you need results, `spawn` only for true fire-and-forget:

```ori
// Use parallel — you need results
let user_data = parallel(
    tasks: for id in ids yield () -> fetch_user(id: id),
    max_concurrent: 10,
    timeout: 30s,
)

// Use spawn — you don't need results
spawn(
    tasks: for event in events yield () -> log_to_analytics(event: event),
    max_concurrent: 100,
)
```

### Handle Partial Failures

When using `CollectAll`, expect some failures:

```ori
@fetch_with_summary (ids: [int]) -> FetchSummary uses Http, Async = {
    let results = parallel(
        tasks: for id in ids yield () -> fetch_data(id: id)
        max_concurrent: 10
        timeout: 30s
    );

    let successes = for r in results if is_ok(result: r) yield match r { Ok(v) -> v, _ -> continue};
    let failures = for r in results if is_err(result: r) yield match r { Err(e) -> e, _ -> continue};

    FetchSummary {
        data: successes
        errors: failures
        success_rate: len(collection: successes) as float / len(collection: results) as float
    }
}
```

## Complete Example

```ori
type PageResult = {
    url: str,
    status: int,
    load_time: Duration,
    error: Option<str>,
}

type HealthCheck = {
    timestamp: str,
    results: [PageResult],
    healthy_count: int,
    unhealthy_count: int,
    avg_load_time: Duration,
}

@check_page (url: str) -> PageResult uses Http, Clock, Async = {
    let start = Clock.now();

    let result = timeout(
        op: Http.get(url: url)
        after: 10s
    );

    let load_time = Clock.elapsed_since(start: start);

    match result {
        Ok(response) -> PageResult {
            url
            status: response.status
            load_time
            error: if response.status >= 400 then
                Some(`HTTP {response.status}`)
            else
                None
        }
        Err(e) -> PageResult {
            url
            status: 0
            load_time
            error: Some(e.to_str())
        }
    }
}

@test_check_page tests @check_page () -> void =
    with Http = MockHttp {
        responses: {
            "https://example.com": MockResponse { status: 200, body: "OK" },
        },
    },
    Clock = handler(state: Instant.parse(s: "2024-01-15T10:00:00")) {
        now: (s) -> (s, s),
    } in {
        let result = check_page(url: "https://example.com");
        assert_eq(actual: result.status, expected: 200);
        assert_none(option: result.error)
    }

@health_check (urls: [str]) -> HealthCheck uses Http, Clock, Async, Logger = {
    Logger.info(msg: `Starting health check for {len(collection: urls)} URLs`);

    let results = parallel(
        tasks: for url in urls yield () -> check_page(url: url)
        max_concurrent: 10
        timeout: 60s
    );

    // Extract PageResults from Results
    let page_results = for r in results yield match r {
        Ok(pr) -> pr
        Err(e) -> PageResult {
            url: "unknown"
            status: 0
            load_time: 0ms
            error: Some(e.to_str())
        }
    };

    let healthy = page_results.iter()
        .filter(predicate: pr -> is_none(option: pr.error))
        .count();
    let unhealthy = len(collection: page_results) - healthy;

    let total_time = page_results.iter()
        .map(transform: pr -> pr.load_time)
        .fold(initial: 0ms, op: (a, b) -> a + b);
    let avg_time = total_time / len(collection: page_results);

    Logger.info(msg: `Health check complete: {healthy} healthy, {unhealthy} unhealthy`);

    HealthCheck {
        timestamp: Clock.now() as str
        results: page_results
        healthy_count: healthy
        unhealthy_count: unhealthy
        avg_load_time: avg_time
    }
}

@test_health_check tests @health_check () -> void =
    with Http = MockHttp {
        responses: {
            "https://good.com": MockResponse { status: 200, body: "OK" },
            "https://bad.com": MockResponse { status: 500, body: "Error" },
        },
    },
    Clock = MockClock { time: "2024-01-15T10:00:00" },
    Logger = MockLogger {} in {
        let result = health_check(urls: ["https://good.com", "https://bad.com"]);
        assert_eq(actual: result.healthy_count, expected: 1);
        assert_eq(actual: result.unhealthy_count, expected: 1)
    }

// Continuous monitoring with nursery
@monitor_continuously (urls: [str], interval: Duration) -> void
    uses Http, Clock, Async, Logger = {
    nursery(
        body: n -> {
            loop {
                let check = health_check(urls: urls);
                Logger.info(msg: `Check at {check.timestamp}: {check.healthy_count}/{len(collection: urls)} healthy`);

                if check.unhealthy_count > 0 then
                    for result in check.results do
                        if is_some(option: result.error) then
                            Logger.warn(msg: `{result.url}: {result.error.unwrap_or(default: "unknown error")}`)

                sleep(duration: interval)
            }
        }
        on_error: CollectAll
        timeout: 24h
    )
}

// Placeholder
@sleep (duration: Duration) -> void uses Async = ();
```

## Quick Reference

### parallel

```ori
parallel(
    tasks: [() -> T],
    max_concurrent: int,
    timeout: Duration,
) -> [Result<T, E>]
```

### spawn

```ori
spawn(
    tasks: [() -> T],
    max_concurrent: int,
) -> void
```

### timeout

```ori
timeout(
    op: expression,
    after: Duration,
) -> Result<T, TimeoutError>
```

### nursery

```ori
nursery(
    body: n -> for x in items do n.spawn(task: () -> ...),
    on_error: CancelRemaining | CollectAll | FailFast,
    timeout: Duration,
) -> [Result<T, E>]
```

### Error Modes

| Mode | Description |
|------|-------------|
| `CancelRemaining` | Cancel pending, finish running |
| `CollectAll` | Wait for all tasks |
| `FailFast` | Cancel everything on first error |

## What's Next

Now that you understand concurrency:

- **[Channels](/guide/15-channels)** — Communication between tasks
- **[Traits](/guide/16-traits)** — Shared behavior definitions


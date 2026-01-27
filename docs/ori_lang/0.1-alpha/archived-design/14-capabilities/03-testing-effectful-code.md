# Testing Effectful Code

This document covers how to test functions that use capabilities — mocking side effects for reliable, fast, deterministic tests.

---

## Overview

Functions with `uses` clauses require capabilities to be provided. In tests, you provide mock implementations:

```ori
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get(
        .url: "/users/" + id,
    )?,
    Ok(parse(
        .input: json,
    )),
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        let result = get_user(
            .id: "1",
        ),
        assert_eq(
            .actual: result,
            .expected: Ok(User { name: "Alice" }),
        ),
    )
```

---

## The Testing Pattern

### 1. Create Mock Implementation

```ori
type MockHttp = {
    responses: {str: str},
    errors: {str: Error},
}

impl Http for MockHttp {
    @get (url: str) -> Result<str, Error> =
        match(
            self.errors.get(
                .key: url,
            ),
            Some(err) -> Err(err),
            None -> match(
                self.responses.get(
                    .key: url,
                ),
                Some(body) -> Ok(body),
                None -> Err(Error { message: "Not found: " + url, cause: None }),
            ),
        )

    @post (url: str, body: str) -> Result<str, Error> =
        self.get(
            .url: url,
        )
}
```

### 2. Provide Mock in Test

```ori
@test_get_user_success tests @get_user () -> void =
    with Http = MockHttp {
        responses: {"/users/123": "{\"id\": \"123\", \"name\": \"Alice\"}"},
        errors: {},
    } in
    run(
        let result = get_user(
            .id: "123",
        ),
        assert(
            .condition: is_ok(
                .result: result,
            ),
        ),
        let user = unwrap(
            .result: result,
        ),
        assert_eq(
            .actual: user.name,
            .expected: "Alice",
        ),
    )
```

### 3. Test Error Cases

```ori
@test_get_user_not_found tests @get_user () -> void =
    with Http = MockHttp {
        responses: {},
        errors: {"/users/999": Error { message: "Not found", cause: None }},
    } in
    run(
        let result = get_user(
            .id: "999",
        ),
        assert(
            .condition: is_err(
                .result: result,
            ),
        ),
    )

@test_get_user_network_error tests @get_user () -> void =
    with Http = MockHttp {
        responses: {},
        errors: {"/users/123": Error { message: "Connection refused", cause: None }},
    } in
    run(
        let result = get_user(
            .id: "123",
        ),
        assert(
            .condition: is_err(
                .result: result,
            ),
        ),
        let err = unwrap_err(
            .result: result,
        ),
        assert(
            .condition: err.message.contains(
                .substring: "Connection",
            ),
        ),
    )
```

---

## Common Mock Patterns

### Recording Mock

Track what was called:

```ori
type RecordingHttp = {
    responses: {str: str},
    calls: [str],
}

// For testing, use a simpler approach:
type HttpCall = Get(url: str) | Post(url: str, body: str)

@test_api_calls tests @sync_data () -> void =
    with Http = MockHttp { responses: {"/api/sync": "{}"}, errors: {} } in
    with CallLog = MockCallLog { calls: [] } in
    run(
        sync_data(),
        let calls = CallLog.get_calls(),
        assert(
            .condition: calls.contains(
                .element: "/api/sync",
            ),
        ),
    )
```

### Sequence Mock

Return different responses for successive calls:

```ori
type SequenceMockHttp = {
    responses: [str],
}

impl Http for SequenceMockHttp {
    // Implementation tracks call count internally
}
```

### Conditional Mock

```ori
type ConditionalMockHttp = {
    handler: (str) -> Result<str, Error>,
}

impl Http for ConditionalMockHttp {
    @get (url: str) -> Result<str, Error> = self.handler(url)
    @post (url: str, body: str) -> Result<str, Error> = self.handler(url)
}

@test_conditional tests @fetch () -> void =
    with Http = ConditionalMockHttp {
        handler: requestUrl -> if requestUrl.contains(
            .substring: "v1",
        ) then Ok("v1 response") else Ok("default"),
    } in
    run(...)
```

---

## Testing Multiple Capabilities

### All Mocked

```ori
@fetch_and_cache (key: str) -> Result<Data, Error> uses Http, Cache = ...

@test_fetch_and_cache tests @fetch_and_cache () -> void =
    with Http = MockHttp { responses: {"/data/key1": "{...}"}, errors: {} } in
    with Cache = MockCache { data: {} } in
    run(
        let result = fetch_and_cache(
            .key: "key1",
        ),
        assert(
            .condition: is_ok(
                .result: result,
            ),
        ),
    )
```

### Test Cache Hit

```ori
@test_fetch_cache_hit tests @fetch_and_cache () -> void =
    // No HTTP responses needed
    with Http = MockHttp { responses: {}, errors: {} } in
    with Cache = MockCache { data: {"key1": "{\"cached\": true}"} } in
    run(
        let result = fetch_and_cache(
            .key: "key1",
        ),
        assert(
            .condition: is_ok(
                .result: result,
            ),
        ),
        let data = unwrap(
            .result: result,
        ),
        // Got cached data, HTTP not called
        assert(
            .condition: data.cached,
        ),
    )
```

### Test Cache Miss

```ori
@test_fetch_cache_miss tests @fetch_and_cache () -> void =
    with Http = MockHttp { responses: {"/data/key1": "{\"fresh\": true}"}, errors: {} } in
    // Empty cache
    with Cache = MockCache { data: {} } in
    run(
        let result = fetch_and_cache(
            .key: "key1",
        ),
        assert(
            .condition: is_ok(
                .result: result,
            ),
        ),
        let data = unwrap(
            .result: result,
        ),
        // Got fresh data from HTTP
        assert(
            .condition: data.fresh,
        ),
    )
```

---

## Testing Time-Dependent Code

### Mock Clock

```ori
trait Clock {
    @now () -> Timestamp
}

type MockClock = {
    fixed_time: Timestamp,
}

impl Clock for MockClock {
    @now () -> Timestamp = self.fixed_time
}
```

### Testing Expiry

```ori
@is_token_expired (token: Token) -> bool uses Clock =
    Clock.now().seconds > token.expires_at.seconds

@test_token_not_expired tests @is_token_expired () -> void =
    with Clock = MockClock { fixed_time: Timestamp { seconds: 1000 } } in
    run(
        let token = Token { expires_at: Timestamp { seconds: 2000 } },
        assert(
            .condition: not(
                .value: is_token_expired(
                    .token: token,
                ),
            ),
        ),
    )

@test_token_expired tests @is_token_expired () -> void =
    with Clock = MockClock { fixed_time: Timestamp { seconds: 3000 } } in
    run(
        let token = Token { expires_at: Timestamp { seconds: 2000 } },
        assert(
            .condition: is_token_expired(
                .token: token,
            ),
        ),
    )
```

---

## Testing Random Code

### Mock Random

```ori
trait Random {
    @int (min: int, max: int) -> int
}

type MockRandom = {
    values: [int],
}

impl Random for MockRandom {
    // Returns values in sequence
}

type FixedRandom = {
    value: int,
}

impl Random for FixedRandom {
    @int (min: int, max: int) -> int = self.value
}
```

### Testing with Fixed Random

```ori
@roll_dice () -> int uses Random = Random.int(
    .min: 1,
    .max: 6,
)

@test_roll_dice tests @roll_dice () -> void =
    with Random = FixedRandom { value: 4 } in
    run(
        let result = roll_dice(),
        assert_eq(
            .actual: result,
            .expected: 4,
        ),
    )
```

---

## Testing Async Code

### Testing Async Code with Sync Mocks

The biggest benefit of capability-based async: **tests use sync mocks and don't need the Async capability**.

```ori
// Production code uses Http + Async (non-blocking)
@fetch_user (id: str) -> Result<User, Error> uses Http, Async =
    Http.get(
        .url: "/users/" + id,
    )?.parse()

// MockHttp is synchronous - returns immediately without suspension
type MockHttp = {
    responses: {str: str},
}

impl Http for MockHttp {
    @get (url: str) -> Result<str, Error> =
        match(
            self.responses.get(
                .key: url,
            ),
            Some(body) -> Ok(body),
            None -> Err(Error { message: "Not found", cause: None }),
        )
}
```

### Sync Tests for Async Code

```ori
// Test doesn't need Async because MockHttp is synchronous
@test_fetch tests @fetch_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{...}"} } in
    run(
        // No .await needed!
        let result = fetch_user(
            .id: "1",
        ),
        assert(
            .condition: is_ok(
                .result: result,
            ),
        ),
    )
```

Note: The test doesn't declare `uses Async` because MockHttp returns immediately without suspending. This is a key advantage of capability-based async over traditional async/await - tests run synchronously.

---

## Testing Logging

### Capture Log Messages

```ori
trait Logger {
    @info (message: str) -> void
    @error (message: str) -> void
}

type CapturingLogger = {
    messages: [str],
}

impl Logger for CapturingLogger {
    @info (message: str) -> void =
        self.messages = self.messages + ["INFO: " + message]
    @error (message: str) -> void =
        self.messages = self.messages + ["ERROR: " + message]
}
```

### Verify Logging

```ori
@process_with_logging (value: int) -> int uses Logger = run(
    Logger.info(
        .message: "Processing: " + str(value),
    ),
    let result = value * 2,
    Logger.info(
        .message: "Result: " + str(result),
    ),
    result,
)

@test_logging tests @process_with_logging () -> void =
    with Logger = CapturingLogger { messages: [] } in
    run(
        let result = process_with_logging(
            .value: 5,
        ),
        assert_eq(
            .actual: result,
            .expected: 10,
        ),
        // Verify logs were written (implementation detail)
    )
```

---

## Best Practices

### Keep Mocks Simple

```ori
// Good: simple, focused mock
type MockHttp = {
    responses: {str: str},
}

// Avoid: overly complex mock with too much logic
type ComplexMock = {
    // ... many fields
    // ... complex matching logic
}
```

### Test One Thing at a Time

```ori
// Good: focused tests
@test_user_found tests @get_user () -> void = ...
@test_user_not_found tests @get_user () -> void = ...
@test_network_error tests @get_user () -> void = ...

// Avoid: one test doing everything
@test_all_cases tests @get_user () -> void = ...
```

### Use Descriptive Test Names

```ori
// Good: describes the scenario
@test_get_user_returns_user_when_found tests @get_user () -> void = ...
@test_get_user_returns_error_when_network_fails tests @get_user () -> void = ...

// Avoid: vague names
@test1 tests @get_user () -> void = ...
@test2 tests @get_user () -> void = ...
```

### Test Edge Cases

```ori
@test_empty_response tests @parse_users () -> void =
    with Http = MockHttp { responses: {"/users": "[]"} } in
    run(
        let result = parse_users(),
        assert_eq(
            .actual: result,
            .expected: Ok([]),
        ),
    )

@test_malformed_json tests @parse_users () -> void =
    with Http = MockHttp { responses: {"/users": "not json"} } in
    run(
        let result = parse_users(),
        assert(
            .condition: is_err(
                .result: result,
            ),
        ),
    )
```

---

## Common Testing Errors

### Forgot to Provide Capability

```
error[E0510]: capability `Http` not provided
  --> src/test.ori:5:18
   |
5  |     let result = get_user(
   |                  ^^^^^^^^ `get_user` requires `Http` capability
6  |         .id: "123",
7  |     ),
   |
   = help: add `with Http = MockHttp { ... } in` before the test body
```

### Mock Missing Expected URL

```
Test failed: test_get_user
  assertion failed at line 12:
    assert(
        .condition: is_ok(
            .result: result,
        ),
    )
  result was: Err(Error { message: "Not found: /users/123", cause: None })

  hint: MockHttp.responses may be missing "/users/123"
```

---

## See Also

- [Capability Traits](01-capability-traits.md) — Defining mock-able interfaces
- [Uses Clause](02-uses-clause.md) — Declaring dependencies
- [Mandatory Tests](../11-testing/01-mandatory-tests.md) — Test requirements
- [Test Syntax](../11-testing/02-test-syntax.md) — Writing tests

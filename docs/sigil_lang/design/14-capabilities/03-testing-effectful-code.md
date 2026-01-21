# Testing Effectful Code

This document covers how to test functions that use capabilities — mocking side effects for reliable, fast, deterministic tests.

---

## Overview

Functions with `uses` clauses require capabilities to be provided. In tests, you provide mock implementations:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    json = Http.get("/users/" + id),
    Ok(parse(json))
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        result = get_user("1"),
        assert_eq(result, Ok(User { name: "Alice" }))
    )
```

---

## The Testing Pattern

### 1. Create Mock Implementation

```sigil
type MockHttp = {
    responses: {str: str},
    errors: {str: Error}
}

impl Http for MockHttp {
    @get (url: str) -> Result<str, Error> =
        match(self.errors.get(url),
            Some(err) -> Err(err),
            None -> match(self.responses.get(url),
                Some(body) -> Ok(body),
                None -> Err(Error { message: "Not found: " + url, cause: None })
            )
        )

    @post (url: str, body: str) -> Result<str, Error> =
        self.get(url)
}
```

### 2. Provide Mock in Test

```sigil
@test_get_user_success tests @get_user () -> void =
    with Http = MockHttp {
        responses: {"/users/123": "{\"id\": \"123\", \"name\": \"Alice\"}"},
        errors: {}
    } in
    run(
        result = get_user("123"),
        assert(is_ok(result)),
        user = unwrap(result),
        assert_eq(user.name, "Alice")
    )
```

### 3. Test Error Cases

```sigil
@test_get_user_not_found tests @get_user () -> void =
    with Http = MockHttp {
        responses: {},
        errors: {"/users/999": Error { message: "Not found", cause: None }}
    } in
    run(
        result = get_user("999"),
        assert(is_err(result))
    )

@test_get_user_network_error tests @get_user () -> void =
    with Http = MockHttp {
        responses: {},
        errors: {"/users/123": Error { message: "Connection refused", cause: None }}
    } in
    run(
        result = get_user("123"),
        assert(is_err(result)),
        err = unwrap_err(result),
        assert(err.message.contains("Connection"))
    )
```

---

## Common Mock Patterns

### Recording Mock

Track what was called:

```sigil
type RecordingHttp = {
    responses: {str: str},
    calls: [str]  // Mutable in tests only via special handling
}

// For testing, use a simpler approach:
type HttpCall = Get(url: str) | Post(url: str, body: str)

@test_api_calls tests @sync_data () -> void =
    with Http = MockHttp { responses: {"/api/sync": "{}"}, errors: {} } in
    with CallLog = MockCallLog { calls: [] } in
    run(
        sync_data(),
        calls = CallLog.get_calls(),
        assert(calls.contains("/api/sync"))
    )
```

### Sequence Mock

Return different responses for successive calls:

```sigil
type SequenceMockHttp = {
    responses: [str]  // Returns responses in order
}

impl Http for SequenceMockHttp {
    // Implementation tracks call count internally
}
```

### Conditional Mock

```sigil
type ConditionalMockHttp = {
    handler: (str) -> Result<str, Error>
}

impl Http for ConditionalMockHttp {
    @get (url: str) -> Result<str, Error> = self.handler(url)
    @post (url: str, body: str) -> Result<str, Error> = self.handler(url)
}

@test_conditional tests @fetch () -> void =
    with Http = ConditionalMockHttp {
        handler: url -> if url.contains("v1") then Ok("v1 response") else Ok("default")
    } in
    run(...)
```

---

## Testing Multiple Capabilities

### All Mocked

```sigil
@fetch_and_cache (key: str) -> Result<Data, Error> uses Http, Cache = ...

@test_fetch_and_cache tests @fetch_and_cache () -> void =
    with Http = MockHttp { responses: {"/data/key1": "{...}"}, errors: {} } in
    with Cache = MockCache { data: {} } in
    run(
        result = fetch_and_cache("key1"),
        assert(is_ok(result))
    )
```

### Test Cache Hit

```sigil
@test_fetch_cache_hit tests @fetch_and_cache () -> void =
    with Http = MockHttp { responses: {}, errors: {} } in  // No HTTP responses needed
    with Cache = MockCache { data: {"key1": "{\"cached\": true}"} } in
    run(
        result = fetch_and_cache("key1"),
        assert(is_ok(result)),
        data = unwrap(result),
        assert(data.cached)  // Got cached data, HTTP not called
    )
```

### Test Cache Miss

```sigil
@test_fetch_cache_miss tests @fetch_and_cache () -> void =
    with Http = MockHttp { responses: {"/data/key1": "{\"fresh\": true}"}, errors: {} } in
    with Cache = MockCache { data: {} } in  // Empty cache
    run(
        result = fetch_and_cache("key1"),
        assert(is_ok(result)),
        data = unwrap(result),
        assert(data.fresh)  // Got fresh data from HTTP
    )
```

---

## Testing Time-Dependent Code

### Mock Clock

```sigil
trait Clock {
    @now () -> Timestamp
}

type MockClock = {
    fixed_time: Timestamp
}

impl Clock for MockClock {
    @now () -> Timestamp = self.fixed_time
}
```

### Testing Expiry

```sigil
@is_token_expired (token: Token) -> bool uses Clock =
    Clock.now().seconds > token.expires_at.seconds

@test_token_not_expired tests @is_token_expired () -> void =
    with Clock = MockClock { fixed_time: Timestamp { seconds: 1000 } } in
    run(
        token = Token { expires_at: Timestamp { seconds: 2000 } },
        assert(not(is_token_expired(token)))
    )

@test_token_expired tests @is_token_expired () -> void =
    with Clock = MockClock { fixed_time: Timestamp { seconds: 3000 } } in
    run(
        token = Token { expires_at: Timestamp { seconds: 2000 } },
        assert(is_token_expired(token))
    )
```

---

## Testing Random Code

### Mock Random

```sigil
trait Random {
    @int (min: int, max: int) -> int
}

type MockRandom = {
    values: [int]  // Predetermined sequence
}

impl Random for MockRandom {
    // Returns values in sequence
}

type FixedRandom = {
    value: int
}

impl Random for FixedRandom {
    @int (min: int, max: int) -> int = self.value
}
```

### Testing with Fixed Random

```sigil
@roll_dice () -> int uses Random = Random.int(.min: 1, .max: 6)

@test_roll_dice tests @roll_dice () -> void =
    with Random = FixedRandom { value: 4 } in
    run(
        result = roll_dice(),
        assert_eq(result, 4)
    )
```

---

## Testing Async Code

### Async Mock

```sigil
trait AsyncHttp {
    @get (url: str) -> async Result<str, Error>
}

type MockAsyncHttp = {
    responses: {str: str}
}

impl AsyncHttp for MockAsyncHttp {
    @get (url: str) -> async Result<str, Error> = async run(
        match(self.responses.get(url),
            Some(body) -> Ok(body),
            None -> Err(Error { message: "Not found", cause: None })
        )
    )
}
```

### Async Test

```sigil
@fetch_user_async (id: str) -> async Result<User, Error> uses AsyncHttp = ...

@test_fetch_async tests @fetch_user_async () -> void =
    with AsyncHttp = MockAsyncHttp { responses: {"/users/1": "{...}"} } in
    run(
        result = fetch_user_async("1").await,
        assert(is_ok(result))
    )
```

---

## Testing Logging

### Capture Log Messages

```sigil
trait Logger {
    @info (message: str) -> void
    @error (message: str) -> void
}

type CapturingLogger = {
    messages: [str]
}

impl Logger for CapturingLogger {
    @info (message: str) -> void =
        self.messages = self.messages + ["INFO: " + message]
    @error (message: str) -> void =
        self.messages = self.messages + ["ERROR: " + message]
}
```

### Verify Logging

```sigil
@process_with_logging (x: int) -> int uses Logger = run(
    Logger.info("Processing: " + str(x)),
    result = x * 2,
    Logger.info("Result: " + str(result)),
    result
)

@test_logging tests @process_with_logging () -> void =
    with Logger = CapturingLogger { messages: [] } in
    run(
        result = process_with_logging(5),
        assert_eq(result, 10),
        // Verify logs were written (implementation detail)
    )
```

---

## Best Practices

### Keep Mocks Simple

```sigil
// Good: simple, focused mock
type MockHttp = {
    responses: {str: str}
}

// Avoid: overly complex mock with too much logic
type ComplexMock = {
    // ... many fields
    // ... complex matching logic
}
```

### Test One Thing at a Time

```sigil
// Good: focused tests
@test_user_found tests @get_user () -> void = ...
@test_user_not_found tests @get_user () -> void = ...
@test_network_error tests @get_user () -> void = ...

// Avoid: one test doing everything
@test_all_cases tests @get_user () -> void = ...
```

### Use Descriptive Test Names

```sigil
// Good: describes the scenario
@test_get_user_returns_user_when_found tests @get_user () -> void = ...
@test_get_user_returns_error_when_network_fails tests @get_user () -> void = ...

// Avoid: vague names
@test1 tests @get_user () -> void = ...
@test2 tests @get_user () -> void = ...
```

### Test Edge Cases

```sigil
@test_empty_response tests @parse_users () -> void =
    with Http = MockHttp { responses: {"/users": "[]"} } in
    run(
        result = parse_users(),
        assert_eq(result, Ok([]))
    )

@test_malformed_json tests @parse_users () -> void =
    with Http = MockHttp { responses: {"/users": "not json"} } in
    run(
        result = parse_users(),
        assert(is_err(result))
    )
```

---

## Common Testing Errors

### Forgot to Provide Capability

```
error[E0510]: capability `Http` not provided
  --> src/test.si:5:5
   |
5  |     result = get_user("123"),
   |              ^^^^^^^^^^^^^^^ `get_user` requires `Http` capability
   |
   = help: add `with Http = MockHttp { ... } in` before the test body
```

### Mock Missing Expected URL

```
Test failed: test_get_user
  assertion failed: is_ok(result)
  result was: Err(Error { message: "Not found: /users/123", cause: None })

  hint: MockHttp.responses may be missing "/users/123"
```

---

## See Also

- [Capability Traits](01-capability-traits.md) — Defining mock-able interfaces
- [Uses Clause](02-uses-clause.md) — Declaring dependencies
- [Mandatory Tests](../11-testing/01-mandatory-tests.md) — Test requirements
- [Test Syntax](../11-testing/02-test-syntax.md) — Writing tests

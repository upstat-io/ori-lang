# Proposal: Standard Library Resilience API

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Standard library, capabilities, `std.resilience` module

---

## Summary

This proposal defines the `std.resilience` module, providing retry logic with configurable backoff strategies. The module enables robust error handling for transient failures in network calls, database operations, and other fallible operations.

---

## Motivation

### The Problem

Transient failures are common in distributed systems:

- Network timeouts
- Rate limiting (HTTP 429)
- Database connection drops
- Service unavailability

Naive retry loops are error-prone:

```ori
// Anti-pattern: manual retry with poor semantics
@fetch_with_retry (url: str) -> Result<Data, Error> uses Http, Suspend =
    run(
        let mut attempts = 0,
        loop(
            match(fetch(url),
                Ok(data) -> break Ok(data),
                Err(e) -> run(
                    attempts = attempts + 1,
                    if attempts >= 3 then break Err(e),
                    sleep(duration: 1s),  // Fixed delay, no backoff
                    continue,
                ),
            ),
        ),
    )
```

Problems with manual retries:
- **Boilerplate**: Same pattern repeated across codebase
- **No exponential backoff**: Fixed delays cause thundering herd
- **No jitter**: Synchronized retries overwhelm servers
- **Hard to test**: Retry logic mixed with business logic

### The Solution

A declarative retry API with configurable strategies:

```ori
use std.resilience { retry, exponential }

@fetch_reliable (url: str) -> Result<Data, Error> uses Http, Suspend =
    retry(
        op: () -> fetch(url),
        attempts: 3,
        backoff: exponential(base: 100ms),
    )
```

---

## Design Principles

Following [stdlib-philosophy-proposal.md](../approved/stdlib-philosophy-proposal.md):

1. **Pure Ori implementation**: No FFI needed for retry logic
2. **Composable**: Backoff strategies are values, not magic strings
3. **Testable**: Strategies work with `MockClock` for deterministic tests
4. **Capability-aware**: Requires `Suspend` for delays, `Clock` for jitter

---

## API Design

### retry Function

The core retry function:

```ori
@retry<T, E> (
    op: () -> Result<T, E> uses Suspend,
    attempts: int,
    backoff: BackoffStrategy = none(),
    should_retry: (E) -> bool = _ -> true,
) -> Result<T, E> uses Suspend, Clock
```

#### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `op` | `() -> Result<T, E> uses Suspend` | Operation to retry |
| `attempts` | `int` | Maximum attempts (must be > 0) |
| `backoff` | `BackoffStrategy` | Delay strategy between attempts |
| `should_retry` | `(E) -> bool` | Predicate for retryable errors |

#### Semantics

1. Execute `op()`
2. If `Ok(value)`: return immediately
3. If `Err(error)`:
   a. If `attempts` exhausted: return `Err(error)`
   b. If `should_retry(error)` is `false`: return `Err(error)`
   c. Wait for `backoff.delay(attempt: current_attempt)`
   d. Increment attempt counter, goto 1

#### Return Type

Returns `Result<T, E>` where:
- `Ok(value)` from successful attempt
- `Err(error)` from final failed attempt

The error is always the last error encountered, not accumulated.

#### Capability Requirements

- `Suspend`: Required for waiting between attempts
- `Clock`: Required for backoff delay timing

### BackoffStrategy Type

An opaque type representing delay calculation:

```ori
type BackoffStrategy = {
    base: Duration,
    factor: float,
    max: Option<Duration>,
    jitter: float,
}
```

#### Methods

```ori
impl BackoffStrategy {
    // Calculate delay for given attempt (1-indexed)
    @delay (self, attempt: int) -> Duration uses Clock
}
```

### Backoff Strategy Constructors

#### none

No delay between attempts:

```ori
@none () -> BackoffStrategy =
    BackoffStrategy { base: 0s, factor: 1.0, max: None, jitter: 0.0 }
```

```ori
retry(op: op, attempts: 5, backoff: none())
// Attempts: immediate, immediate, immediate, immediate, immediate
```

#### constant

Fixed delay between attempts:

```ori
@constant (delay: Duration) -> BackoffStrategy =
    BackoffStrategy { base: delay, factor: 1.0, max: None, jitter: 0.0 }
```

```ori
retry(op: op, attempts: 3, backoff: constant(delay: 1s))
// Attempts: immediate, wait 1s, wait 1s
```

#### linear

Linearly increasing delay:

```ori
@linear (
    base: Duration,
    increment: Duration = base,
    max: Option<Duration> = None,
) -> BackoffStrategy
```

```ori
retry(op: op, attempts: 5, backoff: linear(base: 100ms, increment: 100ms))
// Attempts: immediate, wait 100ms, wait 200ms, wait 300ms, wait 400ms

retry(op: op, attempts: 5, backoff: linear(base: 100ms, max: Some(250ms)))
// Attempts: immediate, wait 100ms, wait 200ms, wait 250ms, wait 250ms
```

Formula: `delay(n) = min(base + (n-1) * increment, max)`

#### exponential

Exponentially increasing delay (recommended for distributed systems):

```ori
@exponential (
    base: Duration,
    factor: float = 2.0,
    max: Option<Duration> = None,
) -> BackoffStrategy
```

```ori
retry(op: op, attempts: 5, backoff: exponential(base: 100ms))
// Attempts: immediate, wait 100ms, wait 200ms, wait 400ms, wait 800ms

retry(op: op, attempts: 5, backoff: exponential(base: 100ms, factor: 3.0))
// Attempts: immediate, wait 100ms, wait 300ms, wait 900ms, wait 2700ms

retry(op: op, attempts: 5, backoff: exponential(base: 100ms, max: Some(500ms)))
// Attempts: immediate, wait 100ms, wait 200ms, wait 400ms, wait 500ms
```

Formula: `delay(n) = min(base * factor^(n-1), max)`

### Jitter

All strategies support jitter to prevent thundering herd:

```ori
extend BackoffStrategy {
    @with_jitter (self, amount: float) -> BackoffStrategy
}
```

```ori
exponential(base: 100ms).with_jitter(amount: 0.2)
// Adds +/- 20% random variation to each delay
```

Jitter is calculated as: `delay * (1 + random(-jitter, jitter))`

Jitter requires `Clock` capability (for access to random source).

---

## should_retry Predicate

The `should_retry` parameter enables selective retry:

```ori
@is_retryable (e: HttpError) -> bool = match(e.status,
    429 -> true,  // Rate limited
    500..599 -> true,  // Server errors
    _ -> false,
)

retry(
    op: () -> fetch(url),
    attempts: 3,
    backoff: exponential(base: 100ms),
    should_retry: is_retryable,
)
// Only retries on 5xx or 429, not 4xx client errors
```

Default: `_ -> true` (retry all errors)

---

## Error Handling

### Last Error Wins

Only the final error is returned:

```ori
retry(op: sometimes_fails, attempts: 3)
// If all 3 attempts fail, returns Err from attempt 3
```

### Collecting All Errors

For diagnostics, wrap in a collector:

```ori
@retry_with_trace<T, E> (
    op: () -> Result<T, E> uses Suspend,
    attempts: int,
    backoff: BackoffStrategy,
) -> Result<T, { errors: [E], final: E }> uses Suspend, Clock =
    run(
        let mut errors: [E] = [],
        retry(
            op: () -> match(op(),
                Ok(v) -> Ok(v),
                Err(e) -> run(
                    errors = [...errors, e],
                    Err(e),
                ),
            ),
            attempts: attempts,
            backoff: backoff,
        ).map_err(transform: e -> { errors: errors, final: e }),
    )
```

---

## Cancellation

Retry respects cancellation:

```ori
timeout(
    op: retry(op: slow_op, attempts: 10, backoff: exponential(base: 1s)),
    after: 5s,
)
// Cancels retry loop when timeout expires
```

Between attempts, the retry loop checks for cancellation at the sleep point. If cancelled:
- Current sleep is interrupted
- `Err(CancellationError { reason: Timeout, ... })` propagates

---

## Examples

### HTTP with Exponential Backoff

```ori
use std.resilience { retry, exponential }
use std.http { get, HttpError }

@fetch_api (endpoint: str) -> Result<Data, HttpError> uses Http, Suspend, Clock =
    retry(
        op: () -> get(url: `https://api.example.com{endpoint}`),
        attempts: 5,
        backoff: exponential(base: 100ms, max: Some(10s)).with_jitter(amount: 0.1),
        should_retry: e -> e.status >= 500 || e.status == 429,
    )
```

### Database Reconnection

```ori
use std.resilience { retry, exponential }

@query_with_retry<T> (
    db: Database,
    sql: str,
) -> Result<T, DbError> uses Suspend, Clock =
    retry(
        op: () -> db.query(sql: sql),
        attempts: 3,
        backoff: exponential(base: 500ms),
        should_retry: e -> e.is_connection_error(),
    )
```

### Immediate Retry (No Backoff)

```ori
use std.resilience { retry, none }

@read_sensor () -> Result<Reading, SensorError> uses Suspend, Clock =
    retry(
        op: sensor.read,
        attempts: 3,
        backoff: none(),  // Retry immediately
    )
```

### Custom Retry Logic

```ori
use std.resilience { retry, linear }

@upload_with_progress (
    file: File,
    on_attempt: (int) -> void,
) -> Result<void, UploadError> uses Suspend, Clock =
    run(
        let mut attempt = 0,
        retry(
            op: () -> run(
                attempt = attempt + 1,
                on_attempt(attempt),
                upload(file: file),
            ),
            attempts: 5,
            backoff: linear(base: 1s),
        ),
    )
```

---

## Testing

### With MockClock

```ori
use std.resilience { retry, exponential }
use std.time { MockClock }

@test_exponential_backoff tests @retry () -> void =
    with Clock = MockClock.new(now: 0s) in run(
        let mut calls = 0,
        let op = () -> run(
            calls = calls + 1,
            if calls < 3 then Err("fail") else Ok("success"),
        ),

        let result = retry(
            op: op,
            attempts: 5,
            backoff: exponential(base: 100ms),
        ),

        assert_eq(actual: result, expected: Ok("success")),
        assert_eq(actual: calls, expected: 3),
        // MockClock.elapsed() shows: 100ms + 200ms = 300ms total delay
    )
```

### Verifying should_retry

```ori
@test_should_retry_predicate tests @retry () -> void =
    run(
        let op = () -> Err(HttpError { status: 400 }),
        let result = retry(
            op: op,
            attempts: 5,
            backoff: none(),
            should_retry: e -> e.status >= 500,
        ),

        // Should not retry 400 errors
        assert(condition: is_err(result: result)),
        // Only 1 attempt made (no retries)
    )
```

---

## Error Messages

### Invalid attempts

```
error[E1100]: `retry` attempts must be positive
  --> src/main.ori:5:5
   |
 5 |     retry(op: op, attempts: 0, backoff: none())
   |                   ^^^^^^^^^^^ must be > 0
   |
   = help: use at least 1 attempt
```

### Missing Suspend Capability

```
error[E1101]: `retry` requires `Suspend` capability
  --> src/main.ori:5:5
   |
 5 | @fetch () -> Result<Data, Error> = retry(...)
   |                                    ^^^^^ requires `uses Suspend`
   |
   = help: add `uses Suspend` to the function signature
```

### Negative Backoff Duration

```
error[E1102]: backoff duration must be non-negative
  --> src/main.ori:5:20
   |
 5 |     exponential(base: -100ms)
   |                       ^^^^^^ negative duration
```

### Invalid Jitter

```
error[E1103]: jitter amount must be between 0.0 and 1.0
  --> src/main.ori:5:20
   |
 5 |     exponential(base: 100ms).with_jitter(amount: 1.5)
   |                                                  ^^^ must be 0.0 <= amount <= 1.0
```

---

## Relationship to Other Patterns

### retry vs timeout

| Aspect | `retry` | `timeout` |
|--------|---------|-----------|
| Purpose | Retry failed operations | Bound operation time |
| Returns | `Result<T, E>` | `Result<T, CancellationError>` |
| On failure | Retries with backoff | Cancels and returns error |
| Combination | Often used together | Often used together |

```ori
// Combine both: timeout per attempt, retry on timeout
retry(
    op: () -> timeout(op: slow_fetch(), after: 5s),
    attempts: 3,
    backoff: exponential(base: 100ms),
)
```

### retry vs catch

| Aspect | `retry` | `catch` |
|--------|---------|---------|
| Purpose | Retry on `Err` | Catch panics |
| Handles | `Result<T, E>` errors | Runtime panics |
| Returns | `Result<T, E>` | `Result<T, str>` |

For panicking operations, combine both:

```ori
retry(
    op: () -> catch(expr: risky_operation()),
    attempts: 3,
    backoff: exponential(base: 100ms),
)
```

---

## Spec Changes Required

### Update `11-built-in-functions.md`

Add reference to `std.resilience` module for retry functionality.

### Create `modules/std.resilience/index.md`

Document the full module API.

---

## Implementation Notes

### Pure Ori Implementation

The module can be implemented entirely in Ori:

```ori
// Simplified implementation sketch
@retry<T, E> (
    op: () -> Result<T, E> uses Suspend,
    attempts: int,
    backoff: BackoffStrategy = none(),
    should_retry: (E) -> bool = _ -> true,
) -> Result<T, E> uses Suspend, Clock =
    run(
        pre_check: attempts > 0 | "attempts must be positive",
        let mut current = 1,
        loop(
            match(op(),
                Ok(value) -> break Ok(value),
                Err(error) -> run(
                    if current >= attempts then break Err(error),
                    if !should_retry(error) then break Err(error),
                    sleep(duration: backoff.delay(attempt: current)),
                    current = current + 1,
                    continue,
                ),
            ),
        ),
    )
```

### Clock Capability Usage

The `Clock` capability provides:
- `sleep(duration:)` for delays
- Random source for jitter calculation

---

## Summary

| Aspect | Details |
|--------|---------|
| Module | `std.resilience` |
| Core function | `retry(op:, attempts:, backoff:, should_retry:)` |
| Strategies | `none()`, `constant()`, `linear()`, `exponential()` |
| Jitter | `.with_jitter(amount:)` method |
| Capabilities | `Suspend` (required), `Clock` (required) |
| Cancellation | Respects cooperative cancellation |
| Testing | Works with `MockClock` |
| Implementation | Pure Ori (no FFI) |

---

## Future Considerations

### Circuit Breaker

A future extension could add circuit breaker pattern:

```ori
// Not in this proposal
circuit_breaker(
    op: op,
    failure_threshold: 5,
    reset_timeout: 30s,
)
```

### Retry Policies

Named policies for common scenarios:

```ori
// Not in this proposal
retry(op: op, policy: Policy.http_default)
```

These are deferred to future proposals to keep this focused on the core retry functionality.

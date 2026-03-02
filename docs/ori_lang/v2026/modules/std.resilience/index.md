# std.resilience

Resilience and retry utilities.

```ori
use std.resilience { retry, exponential, linear }
```

---

## Functions

### @retry

```ori
@retry<T, E> (
    operation: () -> Result<T, E>,
    attempts: int,
    backoff: BackoffStrategy,
) -> Result<T, E>
```

Retries an operation with backoff between attempts.

```ori
use std.resilience { retry, exponential }

let result = retry(
    operation: () -> fetch(url),
    attempts: 3,
    backoff: exponential(base: 100ms),
)
```

**Parameters:**
- `operation` — Function to retry (must return `Result`)
- `attempts` — Maximum number of attempts
- `backoff` — Strategy for delays between attempts

**Returns:** `Result<T, E>` — Success if any attempt succeeds, last error if all fail.

---

### @exponential

```ori
@exponential (base: Duration) -> BackoffStrategy
```

Exponential backoff: delays double each attempt.

```ori
exponential(base: 100ms)
// Delays: 100ms, 200ms, 400ms, 800ms, ...
```

---

### @linear

```ori
@linear (delay: Duration) -> BackoffStrategy
```

Linear backoff: constant delay between attempts.

```ori
linear(delay: 500ms)
// Delays: 500ms, 500ms, 500ms, ...
```

---

## See Also

- [std.validate](../std.validate/) — Input validation
- [Patterns](../spec/10-patterns.md) — Compiler patterns

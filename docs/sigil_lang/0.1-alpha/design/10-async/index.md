# Async

This section covers Sigil's concurrency model: async/await, structured concurrency, cancellation, and channels.

---

## Documents

| Document | Description |
|----------|-------------|
| [Async/Await](01-async-await.md) | Basic async syntax |
| [Structured Concurrency](02-structured-concurrency.md) | No detached tasks |
| [Cancellation](03-cancellation.md) | Context-based cancellation |
| [Channels](04-channels.md) | Typed channels for communication |

---

## Overview

Sigil uses async/await with explicit suspension points:

```sigil
@fetch_user (id: int) -> async Result<User, Error> =
    http_get($api_url + "/users/" + str(id)).await

@fetch_all (ids: [int]) -> async [User] = parallel(
    .tasks: map(ids, id -> fetch_user(id)),
    .max_concurrent: 10
)
```

### Key Concepts

| Concept | Description |
|---------|-------------|
| `async` | In return type, marks function as async |
| `.await` | Postfix syntax, marks suspension points |
| `parallel` | Run async tasks concurrently |
| `Context` | Carries timeout and cancellation |
| `Channel<T>` | Typed communication between tasks |

### Structured Concurrency

All async must be structured - no fire-and-forget:

```sigil
// Allowed: parallel waits for all
data = parallel(
    .tasks: [fetch_a(), fetch_b()]
).await

// NOT allowed: detached tasks
spawn(background_task())  // ERROR
```

### No Shared Mutable State

```sigil
// NOT allowed
shared = Mutex<int>.new(0)  // No Mutex type

// Use message passing or functional patterns
counts = parallel(.tasks: [...]).await
total = fold(counts, 0, +)
```

---

## See Also

- [Main Index](../00-index.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)

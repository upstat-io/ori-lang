# Async

This section covers Sigil's concurrency model: capability-based async tracking, structured concurrency, cancellation, and channels.

---

## Documents

| Document | Description |
|----------|-------------|
| [Async via Capabilities](01-async-await.md) | How capabilities track async |
| [Structured Concurrency](02-structured-concurrency.md) | No detached tasks |
| [Cancellation](03-cancellation.md) | Context-based cancellation |
| [Channels](04-channels.md) | Typed channels for communication |

---

## Overview

Sigil takes a unique approach to async: **an explicit `Async` capability instead of async/await syntax**.

```sigil
// Async capability explicitly declares "may suspend"
@fetch_user (user_id: int) -> Result<User, Error> uses Http, Async =
    Http.get("/users/" + str(user_id))?.parse()

@fetch_all (user_ids: [int]) -> [User] uses Http, Async = parallel(
    .tasks: map(
        .over: user_ids,
        .transform: user_id -> fetch_user(
            .user_id: user_id,
        ),
    ),
    .max_concurrent: 10,
)
```

### Why the Async Capability?

Traditional async/await has a "propagation tax" - `async` bubbles up the call stack. But you get nothing for this cost except the ability to suspend.

Sigil's insight: **you're paying the propagation cost anyway**. The `uses Async` propagates just like `async` would. But with capabilities you also get:

- Easy testing via sync mock implementations
- Explicit dependency tracking
- Cleaner return types (no `async T` wrapper)
- Clear sync vs async distinction

| Approach | Propagates | Testing | Clean Types | Sync/Async Clear |
|----------|------------|---------|-------------|------------------|
| `async/await` | Yes | Hard | No | Yes |
| `uses Async` | Yes | Easy | Yes | Yes |

### Key Concepts

| Concept | Description |
|---------|-------------|
| `uses Async` | Explicitly declares function may suspend |
| `uses Http` | Uses HTTP (blocking if no Async) |
| `uses Http, Async` | Uses HTTP (non-blocking, may suspend) |
| `parallel` | Pattern for concurrent execution |
| `Context` | Carries timeout and cancellation |
| `Channel<T>` | Typed communication between tasks |

### Sequential vs Concurrent

```sigil
// Sequential - one at a time, but still async (may suspend)
@fetch_seq (user_ids: [int]) -> [User] uses Http, Async = run(
    let users = [],
    for user_id in user_ids do
        users = users + [fetch_user(
            .user_id: user_id,
        )?],
    users,
)

// Concurrent - all at once
@fetch_par (user_ids: [int]) -> [User] uses Http, Async = parallel(
    .tasks: map(
        .over: user_ids,
        .transform: user_id -> fetch_user(
            .user_id: user_id,
        ),
    ),
)
```

### Structured Concurrency

All async must be structured - no fire-and-forget:

```sigil
// Allowed: parallel waits for all
data = parallel(
    .tasks: [fetch_a(), fetch_b()]
)

// NOT allowed: detached tasks
// ERROR
spawn(background_task())
```

### No Shared Mutable State

```sigil
// NOT allowed: No Mutex type
shared = Mutex<int>.new(0)

// Use message passing or functional patterns
counts = parallel(
    .tasks: [...],
)
total = fold(
    .over: counts,
    .init: 0,
    .op: +,
)
```

---

## See Also

- [Capabilities](../14-capabilities/index.md)
- [Main Index](../00-index.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)

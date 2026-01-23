# std.cache

Caching utilities.

```sigil
use std.cache { get, set, delete, Cache }
```

**Capability required:** `Cache`

---

## Overview

The `std.cache` module provides:

- Key-value caching with TTL support
- Cache implementations (in-memory, Redis, etc.)
- Cache patterns and utilities

---

## The Cache Capability

```sigil
trait Cache {
    @get (key: str) -> Option<str>
    @set (key: str, value: str) -> void
    @set_with_ttl (key: str, value: str, ttl: Duration) -> void
    @delete (key: str) -> void
    @exists (key: str) -> bool
    @clear () -> void
}
```

The `Cache` capability represents access to a caching layer. Functions that read from or write to a cache must declare `uses Cache` in their signature.

```sigil
@get_user_cached (id: str) -> Result<User, Error> uses Cache, Http, Async = run(
    match(Cache.get("user:" + id),
        Some(json) -> Ok(parse(json)?),
        None -> run(
            let user = fetch_user(id)?,
            Cache.set_with_ttl("user:" + id, stringify(user), 5m),
            Ok(user),
        ),
    ),
)
```

**Implementations:**

| Type | Description |
|------|-------------|
| `MemoryCache` | In-process memory cache |
| `RedisCache` | Redis-backed cache |
| `MockCache` | Configurable mock for testing |

---

## In-Memory Cache

### MemoryCache

```sigil
type MemoryCache = {
    max_size: int,
    default_ttl: Duration,
}
```

Simple in-process cache. Data is lost when the process exits.

```sigil
use std.cache { MemoryCache }

let cache = MemoryCache {
    max_size: 1000,
    default_ttl: 10m,
}
```

**Methods:**
- `new() -> MemoryCache` — Create with defaults
- `max_size(n: int) -> MemoryCache` — Set maximum entries
- `default_ttl(d: Duration) -> MemoryCache` — Set default TTL

---

## Redis Cache

### RedisCache

```sigil
type RedisCache = {
    host: str,
    port: int,
    prefix: str,
}
```

Redis-backed distributed cache.

```sigil
use std.cache { RedisCache }

let cache = RedisCache {
    host: "localhost",
    port: 6379,
    prefix: "myapp:",
}
```

---

## Testing with MockCache

### MockCache

For testing cache-dependent code:

```sigil
type MockCache = {
    data: {str: str},
}

impl Cache for MockCache {
    @get (key: str) -> Option<str> = self.data.get(key)

    @set (key: str, value: str) -> void =
        self.data = self.data.insert(key, value)

    @set_with_ttl (key: str, value: str, ttl: Duration) -> void =
        self.set(key, value)  // Ignores TTL in mock

    @delete (key: str) -> void =
        self.data = self.data.remove(key)

    @exists (key: str) -> bool = self.data.contains_key(key)

    @clear () -> void = self.data = {}
}
```

```sigil
@test_cache_hit tests @get_user_cached () -> void =
    with Cache = MockCache { data: {"user:123": "{\"name\": \"Alice\"}"} } in
    with Http = MockHttp { responses: {} } in  // HTTP not called on cache hit
    run(
        let user = get_user_cached("123")?,
        assert_eq(
            .actual: user.name,
            .expected: "Alice",
        ),
    )

@test_cache_miss tests @get_user_cached () -> void =
    with Cache = MockCache { data: {} } in
    with Http = MockHttp { responses: {"/users/123": "{\"name\": \"Bob\"}"} } in
    run(
        let user = get_user_cached("123")?,
        assert_eq(
            .actual: user.name,
            .expected: "Bob",
        ),
        // Verify it was cached
        assert(Cache.exists("user:123")),
    )
```

---

## Functions

### @get

```sigil
@get (key: str) -> Option<str> uses Cache
```

Retrieves a value from the cache.

```sigil
use std.cache { get }

match(get("session:" + session_id),
    Some(data) -> parse(data),
    None -> create_session(),
)
```

---

### @set

```sigil
@set (key: str, value: str) -> void uses Cache
```

Stores a value in the cache with default TTL.

```sigil
use std.cache { set }

set("config", stringify(config))
```

---

### @set_with_ttl

```sigil
@set_with_ttl (key: str, value: str, ttl: Duration) -> void uses Cache
```

Stores a value with explicit TTL.

```sigil
use std.cache { set_with_ttl }

set_with_ttl("token:" + user_id, token, 1h)
set_with_ttl("rate_limit:" + ip, "1", 1m)
```

---

### @delete

```sigil
@delete (key: str) -> void uses Cache
```

Removes a value from the cache.

```sigil
use std.cache { delete }

delete("session:" + session_id)
```

---

### @exists

```sigil
@exists (key: str) -> bool uses Cache
```

Checks if a key exists in the cache.

```sigil
use std.cache { exists }

if exists("rate_limit:" + ip) then
    Err(RateLimited)
else
    process_request()
```

---

## Patterns

### Cache-Aside Pattern

```sigil
@cached<T> (
    key: str,
    fetch: () -> Result<T, Error>,
    ttl: Duration,
) -> Result<T, Error> uses Cache = match(Cache.get(key),
    Some(json) -> Ok(parse(json)?),
    None -> run(
        let value = fetch()?,
        Cache.set_with_ttl(key, stringify(value), ttl),
        Ok(value),
    ),
)

// Usage
@get_user (id: str) -> Result<User, Error> uses Cache, Http, Async =
    cached(
        "user:" + id,
        () -> fetch_user_from_api(id),
        5m,
    )
```

### Write-Through Pattern

```sigil
@update_user (user: User) -> Result<void, Error> uses Cache, Database = run(
    Database.execute("UPDATE users SET name = ? WHERE id = ?", [user.name, user.id])?,
    Cache.set("user:" + user.id, stringify(user)),
    Ok(()),
)
```

### Cache Invalidation

```sigil
@invalidate_user_cache (id: str) -> void uses Cache = run(
    Cache.delete("user:" + id),
    Cache.delete("user_posts:" + id),
    Cache.delete("user_followers:" + id),
)
```

---

## Examples

### Rate limiting

```sigil
use std.cache { get, set_with_ttl }

@check_rate_limit (ip: str, limit: int, window: Duration) -> Result<void, Error> uses Cache = run(
    let key = "rate:" + ip,
    let count = get(key).and_then(parse_int).unwrap_or(0),

    if count >= limit then
        Err(Error { message: "Rate limit exceeded", source: None })
    else run(
        set_with_ttl(key, str(count + 1), window),
        Ok(()),
    ),
)
```

### Session management

```sigil
use std.cache { get, set_with_ttl, delete }
use std.math.rand { random_bytes }

@create_session (user_id: str) -> str uses Cache, Random = run(
    let token = random_bytes(32).to_hex(),
    set_with_ttl("session:" + token, user_id, 24h),
    token,
)

@get_session_user (token: str) -> Option<str> uses Cache =
    get("session:" + token)

@logout (token: str) -> void uses Cache =
    delete("session:" + token)
```

---

## See Also

- [Capabilities](../../spec/14-capabilities.md) — Capability system
- [std.async](../std.async/) — Async patterns
- [Testing Effectful Code](../../design/14-capabilities/03-testing-effectful-code.md) — Mocking patterns

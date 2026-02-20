# Proposal: Cache Pattern

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, patterns, capabilities

---

## Summary

This proposal formalizes the `cache` pattern semantics, including TTL behavior, key requirements, cache invalidation, and capability interaction.

---

## Problem Statement

The spec shows `cache(key:, op:, ttl:)` but leaves unclear:

1. **Key semantics**: What types can be keys?
2. **TTL behavior**: What happens on expiration?
3. **Cache scope**: Where is the cache stored?
4. **Concurrent access**: How do parallel requests behave?
5. **Invalidation**: How to clear cached values?

---

## Syntax

```ori
cache(
    key: expression,
    op: expression,
    ttl: Duration,
)
```

---

## Semantics

### Basic Behavior

1. Compute `key` expression
2. Check cache for existing unexpired entry
3. If hit: return cached value
4. If miss: evaluate `op`, store result, return it

```ori
@fetch_user (id: int) -> User uses Cache =
    cache(
        key: `user-{id}`,
        op: db.query(id: id),
        ttl: 5m,
    )
```

### Return Type

The `cache` pattern returns the same type as `op`:

```ori
cache(key: k, op: get_data(), ttl: 1m)  // Returns type of get_data()
```

---

## Key Requirements

### Hashable + Eq

Keys must implement `Hashable` and `Eq`:

```ori
cache(key: "string-key", op: ..., ttl: 1m)  // OK: str is Hashable + Eq
cache(key: 42, op: ..., ttl: 1m)            // OK: int is Hashable + Eq
cache(key: (user_id, "profile"), op: ..., ttl: 1m)  // OK: tuple of hashables
```

### Composite Keys

Tuples and structs can be keys if all components are Hashable + Eq:

```ori
#derive(Eq, Hashable)
type CacheKey = { user_id: int, resource: str }

cache(key: CacheKey { user_id: id, resource: "profile" }, op: ..., ttl: 1m)
```

---

## TTL (Time To Live)

### Expiration

Entries expire after TTL from creation:

```ori
cache(key: k, op: compute(), ttl: 5m)
// Entry valid for 5 minutes from first computation
```

### TTL = 0

Zero TTL means no caching (always recompute):

```ori
cache(key: k, op: compute(), ttl: 0s)  // Always executes op
```

### No Negative TTL

Negative TTL is a compile error:

```ori
cache(key: k, op: compute(), ttl: -1s)  // ERROR
```

---

## Cache Scope

### Capability-Provided

The cache storage is provided by the `Cache` capability:

```ori
@cached_fetch (url: str) -> Data uses Cache =
    cache(key: url, op: fetch(url), ttl: 10m)

// At call site:
with Cache = InMemoryCache { max_size: 1000 } in
    cached_fetch(url: "https://api.example.com/data")
```

### Cache Implementations

Different Cache capability implementations provide different behaviors:

| Implementation | Description |
|----------------|-------------|
| `InMemoryCache` | Process-local, fastest |
| `DistributedCache` | Shared across nodes |
| `NoOpCache` | Disable caching (always miss) |

### Suspension Behavior

Cache operations may suspend depending on the implementation:

| Implementation | Suspends |
|----------------|----------|
| `InMemoryCache` | No |
| `DistributedCache` | Yes (network I/O) |
| `NoOpCache` | No |

When using a suspending cache implementation, the calling function must have `uses Suspend` or be called from an async context. The cache pattern handles this transparently — the suspension is internal to the capability.

```ori
// In async context, distributed cache suspension is transparent
@fetch_data () -> Data uses Cache, Async =
    cache(key: "data", op: compute(), ttl: 5m)
```

---

## Concurrent Access

### Stampede Prevention

When multiple tasks request the same key simultaneously:

1. First request computes the value
2. Other requests wait for computation
3. All receive the same result

```ori
parallel(
    tasks: [
        () -> cache(key: "shared", op: expensive(), ttl: 1m),
        () -> cache(key: "shared", op: expensive(), ttl: 1m),
        () -> cache(key: "shared", op: expensive(), ttl: 1m),
    ],
)
// expensive() called only once
```

### Error Behavior

If `op` fails during stampede:
- Waiting requests also receive the error
- Entry is NOT cached
- Next request will retry

---

## Error Handling

### Op Failure

If `op` returns `Err` or panics, the result is NOT cached:

```ori
cache(
    key: url,
    op: fetch(url),  // Returns Result<Data, Error>
    ttl: 5m,
)
// Only Ok values are cached; Err values are not
```

### Caching Errors

To cache error results, wrap in a non-error type:

```ori
cache(
    key: url,
    op: match fetch(url) { r -> r},  // Cache the Result itself
    ttl: 5m,
)
```

---

## Invalidation

### Time-Based

Entries automatically expire after TTL.

### Manual Invalidation

Use Cache capability methods:

```ori
@invalidate_user (id: int) -> void uses Cache =
    Cache.invalidate(key: `user-{id}`)

@clear_all_cache () -> void uses Cache =
    Cache.clear()
```

### Cache Interface

```ori
trait Cache {
    @get<K: Hashable + Eq, V: Clone> (self, key: K) -> Option<V>
    @set<K: Hashable + Eq, V: Clone> (self, key: K, value: V, ttl: Duration) -> void
    @invalidate<K: Hashable + Eq> (self, key: K) -> void
    @clear (self) -> void
}
```

---

## Value Requirements

### Clone

Cached values must implement `Clone`:

```ori
cache(key: k, op: get_user(), ttl: 1m)  // User must be Clone
```

The cache returns a clone of the stored value. For distributed caches, the capability implementation handles serialization internally — this is not exposed at the type level.

---

## Cache vs Memoization

The `cache` pattern and `recurse(..., memo: true)` serve different purposes:

| Aspect | `cache(...)` | `recurse(..., memo: true)` |
|--------|--------------|---------------------------|
| Persistence | TTL-based, may persist across calls | Call-duration only |
| Capability | Requires `Cache` | Pure, no capability |
| Scope | Shared across function calls | Private to single recurse |
| Use case | API responses, config, expensive I/O | Pure recursive algorithms |

For pure recursive functions like fibonacci, prefer `recurse(..., memo: true)`:

```ori
// Preferred for pure memoization
@fibonacci (n: int) -> int =
    recurse(
        condition: n <= 1,
        base: n,
        step: self(n - 1) + self(n - 2),
        memo: true,
    )

// Use cache for persistent/TTL scenarios
@get_exchange_rate (from: str, to: str) -> float uses Cache, Http =
    cache(
        key: (from, to),
        op: fetch_rate(from: from, to: to),
        ttl: 1h,
    )
```

---

## Examples

### API Response Caching

```ori
@get_exchange_rate (from: str, to: str) -> float uses Cache, Http =
    cache(
        key: (from, to),
        op: fetch_rate(from: from, to: to),
        ttl: 1h,
    )
```

### Computed Value Caching

> **Note:** For pure recursive functions, prefer `recurse(..., memo: true)`. Use `cache` when you need TTL-based persistence across calls.

```ori
@fibonacci (n: int) -> int uses Cache =
    if n <= 1 then n
    else cache(
        key: n,
        op: fibonacci(n: n - 1) + fibonacci(n: n - 2),
        ttl: 24h,
    )
```

### Configuration Caching

```ori
@get_config () -> Config uses Cache, FileSystem =
    cache(
        key: "app-config",
        op: load_config_file(),
        ttl: 5m,
    )
```

---

## Error Messages

### Non-Hashable Key

```
error[E0990]: cache key must be `Hashable`
  --> src/main.ori:5:10
   |
 5 |     cache(key: my_closure, op: compute(), ttl: 1m)
   |                ^^^^^^^^^^^ `(int) -> int` does not implement `Hashable`
   |
   = help: use a hashable key type like `str`, `int`, or derive `Hashable`
```

### Missing Cache Capability

```
error[E0991]: `cache` requires `Cache` capability
  --> src/main.ori:5:5
   |
 5 |     cache(key: k, op: compute(), ttl: 1m)
   |     ^^^^^ requires `uses Cache`
   |
   = help: add `uses Cache` to the function signature
```

### Negative TTL

```
error[E0992]: TTL must be non-negative
  --> src/main.ori:5:35
   |
 5 |     cache(key: k, op: compute(), ttl: -5m)
   |                                       ^^^ negative duration
```

---

## Spec Changes Required

### Update `10-patterns.md`

Expand cache section with:
1. Complete semantics
2. Key requirements
3. TTL behavior
4. Concurrent access rules
5. Cache capability interface

---

## Summary

| Aspect | Details |
|--------|---------|
| Syntax | `cache(key:, op:, ttl:)` |
| Key requirement | `Hashable + Eq` |
| Value requirement | `Clone` |
| TTL | Duration until expiration |
| Scope | Provided by `Cache` capability |
| Concurrent | Stampede prevention (one computation) |
| Errors | Not cached by default |
| Invalidation | Time-based or manual via capability |

# Proposal: LRU Cache in Standard Library

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-26

---

## Summary

Add `LRUCache<K, V>` to `std.collections` for bounded caching with least-recently-used eviction. Uses timestamp-based tracking instead of a doubly-linked list to maintain ARC safety.

```sigil
use std.collections { LRUCache }

let cache = LRUCache.new(capacity: 100)
let cache = cache.put(key: "user:1", value: user_data)
let (cache, result) = cache.get(key: "user:1")  // Updates access time
```

---

## Motivation

### The Problem

LRU caches traditionally use a doubly-linked list for O(1) reordering:

```typescript
class LRUCache<K, V> {
  private map = new Map<K, Node>();
  private head: Node | null = null;  // Most recent
  private tail: Node | null = null;  // Least recent

  get(key: K) {
    const node = this.map.get(key);
    if (node) {
      this.moveToFront(node);  // O(1) via prev/next pointers
      return node.value;
    }
  }
}
```

This creates cycles: `head ↔ node ↔ node ↔ tail`

### The Solution

Replace the linked list with timestamps:

```sigil
type LRUCache<K, V> = {
    entries: {K: (V, int)},  // key → (value, access_time)
    access_counter: int,
    capacity: int,
}
```

Each entry stores its last access time. Eviction finds the minimum timestamp.

### Complexity Trade-off

| Operation | Linked List LRU | Timestamp LRU |
|-----------|-----------------|---------------|
| Get | O(1) | O(1) |
| Put | O(1) | O(1) |
| Evict | O(1) | O(n) |
| Memory | 2 pointers/entry | 1 int/entry |

The O(n) eviction sounds bad, but:
- Eviction only happens when cache is full
- Good caches have high hit rates (eviction is rare)
- Batch eviction amortizes the cost
- Simpler code = fewer bugs

---

## Design

### Core Type

```sigil
type LRUCache<K, V> = {
    entries: {K: (V, int)},
    access_counter: int,
    capacity: int,
}
```

### Construction

```sigil
@new<K, V> (capacity: int) -> LRUCache<K, V> =
    LRUCache {
        entries: {},
        access_counter: 0,
        capacity: capacity,
    }

@with_entries<K, V> (capacity: int, entries: [(K, V)]) -> LRUCache<K, V> =
    entries.fold(
        initial: LRUCache.new(capacity: capacity),
        op: (cache, (k, v)) -> cache.put(key: k, value: v),
    )
```

### Core Operations

```sigil
// Get value, updating access time
// Returns updated cache (access time changed) and optional value
@get<K: Eq + Hashable, V> (self, key: K) -> (LRUCache<K, V>, Option<V>) =
    match(
        self.entries[key],
        None -> (self, None),
        Some((value, _)) -> run(
            let new_counter = self.access_counter + 1,
            let updated = LRUCache {
                entries: self.entries.insert(key: key, value: (value, new_counter)),
                access_counter: new_counter,
                capacity: self.capacity,
            },
            (updated, Some(value)),
        ),
    )

// Put value, evicting if necessary
@put<K: Eq + Hashable, V> (self, key: K, value: V) -> LRUCache<K, V> = run(
    let new_counter = self.access_counter + 1,
    let updated = LRUCache {
        entries: self.entries.insert(key: key, value: (value, new_counter)),
        access_counter: new_counter,
        capacity: self.capacity,
    },
    maybe_evict(cache: updated),
)

// Remove entry
@remove<K: Eq + Hashable, V> (self, key: K) -> LRUCache<K, V> =
    LRUCache {
        entries: self.entries.remove(key: key),
        access_counter: self.access_counter,
        capacity: self.capacity,
    }

// Clear all entries
@clear<K, V> (self) -> LRUCache<K, V> =
    LRUCache {
        entries: {},
        access_counter: 0,
        capacity: self.capacity,
    }
```

### Eviction

```sigil
// Evict oldest entry if over capacity
@maybe_evict<K: Eq + Hashable, V> (cache: LRUCache<K, V>) -> LRUCache<K, V> =
    if len(collection: cache.entries) <= cache.capacity
    then cache
    else run(
        let oldest = cache.entries
            .to_list()
            .fold(
                initial: None,
                op: (acc, (k, (_, time))) -> match(
                    acc,
                    None -> Some((k, time)),
                    Some((_, min_time)) -> if time < min_time
                        then Some((k, time))
                        else acc,
                ),
            ),
        match(
            oldest,
            None -> cache,
            Some((evict_key, _)) -> LRUCache {
                entries: cache.entries.remove(key: evict_key),
                access_counter: cache.access_counter,
                capacity: cache.capacity,
            },
        ),
    )

// Batch eviction for better amortization
@evict_batch<K: Eq + Hashable, V> (cache: LRUCache<K, V>, count: int) -> LRUCache<K, V> = run(
    let sorted = cache.entries
        .to_list()
        .sort_by(key: (_, (_, time)) -> time),
    let to_remove = sorted
        .take(n: count)
        .map(transform: (k, _) -> k),
    LRUCache {
        entries: to_remove.fold(
            initial: cache.entries,
            op: (m, k) -> m.remove(key: k),
        ),
        access_counter: cache.access_counter,
        capacity: cache.capacity,
    },
)

// Evict entries older than threshold
@evict_older_than<K: Eq + Hashable, V> (cache: LRUCache<K, V>, threshold: int) -> LRUCache<K, V> =
    LRUCache {
        entries: cache.entries.filter(predicate: (_, (_, time)) -> time >= threshold),
        access_counter: cache.access_counter,
        capacity: cache.capacity,
    }
```

### Query Operations

```sigil
@contains<K: Eq + Hashable, V> (self, key: K) -> bool =
    self.entries[key].is_some()

@len<K, V> (self) -> int =
    len(collection: self.entries)

@is_empty<K, V> (self) -> bool =
    self.entries.is_empty()

@is_full<K, V> (self) -> bool =
    len(collection: self.entries) >= self.capacity

@keys<K, V> (self) -> [K] =
    self.entries.keys()

@values<K, V> (self) -> [V] =
    self.entries.to_list().map(transform: (_, (v, _)) -> v)

// Get without updating access time (peek)
@peek<K: Eq + Hashable, V> (self, key: K) -> Option<V> =
    self.entries[key].map(transform: (v, _) -> v)
```

### Get-or-Compute Pattern

```sigil
// Get existing or compute and cache
@get_or_insert<K: Eq + Hashable, V> (
    self,
    key: K,
    compute: () -> V,
) -> (LRUCache<K, V>, V) =
    match(
        self.entries[key],
        Some((value, _)) -> run(
            let (updated, _) = self.get(key: key),  // Update access time
            (updated, value),
        ),
        None -> run(
            let value = compute(),
            let updated = self.put(key: key, value: value),
            (updated, value),
        ),
    )

// Async version
@get_or_insert_async<K: Eq + Hashable, V> (
    self,
    key: K,
    compute: () -> V uses Async,
) -> (LRUCache<K, V>, V) uses Async =
    match(
        self.entries[key],
        Some((value, _)) -> run(
            let (updated, _) = self.get(key: key),
            (updated, value),
        ),
        None -> run(
            let value = compute(),
            let updated = self.put(key: key, value: value),
            (updated, value),
        ),
    )
```

---

## Examples

### Basic Usage

```sigil
use std.collections { LRUCache }

@example_basic () -> void = run(
    let cache = LRUCache.new(capacity: 3),

    // Add entries
    let cache = cache.put(key: "a", value: 1),
    let cache = cache.put(key: "b", value: 2),
    let cache = cache.put(key: "c", value: 3),

    // Access "a" to make it recent
    let (cache, _) = cache.get(key: "a"),

    // Add "d" - evicts "b" (least recently used)
    let cache = cache.put(key: "d", value: 4),

    assert(condition: cache.contains(key: "a")),  // Still there (accessed)
    assert(condition: !cache.contains(key: "b")), // Evicted
    assert(condition: cache.contains(key: "c")),
    assert(condition: cache.contains(key: "d")),
)
```

### Memoization Cache

```sigil
use std.collections { LRUCache }

type MemoCache = LRUCache<int, int>

@fibonacci_cached (n: int, cache: MemoCache) -> (MemoCache, int) =
    if n <= 1
    then (cache, n)
    else cache.get_or_insert(
        key: n,
        compute: () -> run(
            let (cache, a) = fibonacci_cached(n: n - 1, cache: cache),
            let (cache, b) = fibonacci_cached(n: n - 2, cache: cache),
            a + b,
        ),
    )
```

### HTTP Response Cache

```sigil
use std.collections { LRUCache }
use std.time { now, Duration }

type CachedResponse = {
    body: str,
    status: int,
    cached_at: Duration,
}

type ResponseCache = LRUCache<str, CachedResponse>

@fetch_cached (
    cache: ResponseCache,
    url: str,
    max_age: Duration,
) -> (ResponseCache, Result<str, Error>) uses Http = run(
    // Check cache
    let (cache, cached) = cache.get(key: url),

    match(
        cached,
        Some(resp) if now() - resp.cached_at < max_age ->
            (cache, Ok(resp.body)),
        _ -> run(
            // Fetch fresh
            let result = Http.get(url: url),
            match(
                result,
                Ok(response) -> run(
                    let cached_resp = CachedResponse {
                        body: response.body,
                        status: response.status,
                        cached_at: now(),
                    },
                    let cache = cache.put(key: url, value: cached_resp),
                    (cache, Ok(response.body)),
                ),
                Err(e) -> (cache, Err(e)),
            ),
        ),
    ),
)
```

### Database Query Cache

```sigil
use std.collections { LRUCache }

type QueryCache = LRUCache<str, [Row]>

@query_cached (
    cache: QueryCache,
    sql: str,
) -> (QueryCache, [Row]) uses Database =
    cache.get_or_insert(
        key: sql,
        compute: () -> Database.query(sql: sql),
    )

@invalidate_table (cache: QueryCache, table: str) -> QueryCache =
    LRUCache {
        entries: cache.entries.filter(
            predicate: (sql, _) -> !sql.contains(substring: table),
        ),
        access_counter: cache.access_counter,
        capacity: cache.capacity,
    }
```

---

## Advanced: TTL Support

For time-based expiration in addition to LRU:

```sigil
type TTLCache<K, V> = {
    entries: {K: (V, int, Duration)},  // value, access_time, expires_at
    access_counter: int,
    capacity: int,
    default_ttl: Duration,
}

@get_ttl<K: Eq + Hashable, V> (self, key: K) -> (TTLCache<K, V>, Option<V>) uses Clock =
    match(
        self.entries[key],
        None -> (self, None),
        Some((value, _, expires_at)) ->
            if Clock.now() > expires_at
            then (self.remove(key: key), None)  // Expired
            else run(
                let new_counter = self.access_counter + 1,
                let updated = TTLCache {
                    entries: self.entries.insert(
                        key: key,
                        value: (value, new_counter, expires_at),
                    ),
                    access_counter: new_counter,
                    capacity: self.capacity,
                    default_ttl: self.default_ttl,
                },
                (updated, Some(value)),
            ),
    )

@put_ttl<K: Eq + Hashable, V> (self, key: K, value: V, ttl: Duration) -> TTLCache<K, V> uses Clock = run(
    let new_counter = self.access_counter + 1,
    let expires_at = Clock.now() + ttl,
    let updated = TTLCache {
        entries: self.entries.insert(key: key, value: (value, new_counter, expires_at)),
        access_counter: new_counter,
        capacity: self.capacity,
        default_ttl: self.default_ttl,
    },
    maybe_evict_ttl(cache: updated),
)
```

---

## ARC Safety

The LRU cache is ARC-safe because:

1. **No linked list** — Entries are stored in a flat map, not a doubly-linked structure.

2. **Timestamps are values** — Access times are integers, not references.

3. **No internal pointers** — No `prev`/`next` fields creating cycles.

4. **Reference structure:**
   ```
   LRUCache
     └── entries: {K: (V, int)}  // Just data, no cycles
   ```

---

## Performance Considerations

### When Timestamp LRU is Good

- **High hit rate** — Eviction is rare
- **Moderate capacity** — O(n) eviction is fast for n < 10,000
- **Simple code preferred** — Fewer bugs, easier debugging

### When to Use Index-Based Linked List

For high-churn caches (constant eviction), an index-based linked list provides O(1) eviction:

```sigil
type FastLRUCache<K, V> = {
    entries: {K: int},           // key → index
    nodes: [Option<CacheNode<K, V>>],
    head: Option<int>,
    tail: Option<int>,
    free: [int],
    capacity: int,
}

type CacheNode<K, V> = {
    key: K,
    value: V,
    prev: Option<int>,  // Index, not pointer
    next: Option<int>,  // Index, not pointer
}
```

This is more complex but provides O(1) for all operations. Could be added as `FastLRUCache` if needed.

---

## Comparison

| Implementation | Get | Put | Evict | Complexity | ARC Safe |
|----------------|-----|-----|-------|------------|----------|
| Linked list + map | O(1) | O(1) | O(1) | High | No |
| **Timestamp map** | O(1) | O(1) | O(n) | Low | Yes |
| Index-based list | O(1) | O(1) | O(1) | Medium | Yes |
| Ordered map* | O(1) | O(1) | O(1) | Low | Yes |

*If language provides insertion-ordered map with move-to-end operation.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Storage | Map with timestamps |
| Eviction | O(n) scan for minimum |
| Get/Put | O(1) |
| TTL support | Optional extension |
| ARC safety | Yes — no linked list |
| Language changes | None — stdlib only |

Simple, correct, and ARC-safe. Covers the common case well.

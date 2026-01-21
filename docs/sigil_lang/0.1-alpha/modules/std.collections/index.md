# std.collections

Additional collection types.

```sigil
use std.collections { Deque, BTreeMap, BTreeSet, LinkedList }
```

**No capability required**

---

## Overview

The `std.collections` module provides collection types beyond the built-in `[T]`, `{K: V}`, and `Set<T>`:

- `Deque<T>` — Double-ended queue
- `BTreeMap<K, V>` — Sorted map
- `BTreeSet<T>` — Sorted set
- `LinkedList<T>` — Doubly-linked list
- `PriorityQueue<T>` — Heap-based priority queue

---

## Deque<T>

Double-ended queue with O(1) push/pop at both ends.

```sigil
use std.collections { Deque }

let q = Deque<int>.new()
q.push_back(1)
q.push_front(0)
// [0, 1]

q.pop_front()  // Some(0)
q.pop_back()   // Some(1)
```

**Methods:**
- `new() -> Deque<T>` — Create empty deque
- `push_front(value: T)` — Add to front
- `push_back(value: T)` — Add to back
- `pop_front() -> Option<T>` — Remove from front
- `pop_back() -> Option<T>` — Remove from back
- `front() -> Option<T>` — Peek front
- `back() -> Option<T>` — Peek back
- `len() -> int` — Number of elements
- `is_empty() -> bool`

### Use Cases

- Sliding window algorithms
- BFS traversal
- Work-stealing queues

---

## BTreeMap<K, V>

Sorted map using B-tree. Keys must be `Comparable`.

```sigil
use std.collections { BTreeMap }

let map = BTreeMap<str, int>.new()
map.insert("apple", 1)
map.insert("banana", 2)
map.insert("cherry", 3)

// Iteration is in sorted key order
for (key, value) in map.entries() do
    print(key + ": " + str(value))
// apple: 1
// banana: 2
// cherry: 3
```

**Methods:**
- `new() -> BTreeMap<K, V>` — Create empty map
- `insert(key: K, value: V) -> Option<V>` — Insert, returns old value
- `get(key: K) -> Option<V>` — Get value
- `remove(key: K) -> Option<V>` — Remove entry
- `contains(key: K) -> bool` — Check key exists
- `keys() -> [K]` — Sorted keys
- `values() -> [V]` — Values in key order
- `entries() -> [(K, V)]` — Sorted entries
- `range(start: K, end: K) -> [(K, V)]` — Entries in range
- `len() -> int`

### Use Cases

- When you need sorted iteration
- Range queries
- When keys aren't `Hashable`

---

## BTreeSet<T>

Sorted set using B-tree. Elements must be `Comparable`.

```sigil
use std.collections { BTreeSet }

let set = BTreeSet<int>.new()
set.insert(3)
set.insert(1)
set.insert(2)

set.to_list()  // [1, 2, 3] (sorted)
```

**Methods:**
- `new() -> BTreeSet<T>` — Create empty set
- `insert(value: T) -> bool` — Insert, returns true if new
- `remove(value: T) -> bool` — Remove, returns true if existed
- `contains(value: T) -> bool` — Check membership
- `range(start: T, end: T) -> [T]` — Elements in range
- `min() -> Option<T>` — Smallest element
- `max() -> Option<T>` — Largest element
- `len() -> int`

### Set Operations

```sigil
let a = BTreeSet.from([1, 2, 3])
let b = BTreeSet.from([2, 3, 4])

a.union(b)        // [1, 2, 3, 4]
a.intersection(b) // [2, 3]
```

---

## LinkedList<T>

Doubly-linked list with O(1) insertion/removal at known positions.

```sigil
use std.collections { LinkedList }

let list = LinkedList<int>.new()
list.push_front(1)
list.push_back(2)
list.push_back(3)
// [1, 2, 3]
```

**Methods:**
- `new() -> LinkedList<T>` — Create empty list
- `push_front(value: T)` — Add to front
- `push_back(value: T)` — Add to back
- `pop_front() -> Option<T>` — Remove from front
- `pop_back() -> Option<T>` — Remove from back
- `front() -> Option<T>` — Peek front
- `back() -> Option<T>` — Peek back
- `len() -> int`

### Use Cases

- When you need O(1) insertion/removal in middle
- LRU cache implementation
- Rarely needed (usually `[T]` or `Deque<T>` is better)

---

## PriorityQueue<T>

Max-heap priority queue. Elements must be `Comparable`.

```sigil
use std.collections { PriorityQueue }

let pq = PriorityQueue<int>.new()
pq.push(3)
pq.push(1)
pq.push(4)
pq.push(1)
pq.push(5)

pq.pop()  // Some(5) - largest first
pq.pop()  // Some(4)
pq.pop()  // Some(3)
```

**Methods:**
- `new() -> PriorityQueue<T>` — Create max-heap
- `new_min() -> PriorityQueue<T>` — Create min-heap
- `push(value: T)` — Add element
- `pop() -> Option<T>` — Remove highest priority
- `peek() -> Option<T>` — View highest priority
- `len() -> int`

### Use Cases

- Dijkstra's algorithm
- Task scheduling
- K-largest/smallest elements

---

## Choosing a Collection

| Need | Use |
|------|-----|
| Ordered sequence | `[T]` (built-in) |
| Key-value, fast lookup | `{K: V}` (built-in) |
| Unique elements, fast lookup | `Set<T>` (built-in) |
| FIFO queue | `Deque<T>` |
| Sorted keys | `BTreeMap<K, V>` |
| Sorted unique elements | `BTreeSet<T>` |
| Priority-based retrieval | `PriorityQueue<T>` |
| Frequent mid-list modifications | `LinkedList<T>` |

---

## Examples

### LRU Cache

```sigil
use std.collections { LinkedList }

type LRUCache<K, V> = {
    capacity: int,
    map: {K: (V, Node)},
    order: LinkedList<K>,
}

impl<K: Eq + Hashable, V> LRUCache<K, V> {
    @get (self, key: K) -> Option<V> = run(
        let entry = self.map[key]?,
        self.order.move_to_front(entry.1),
        Some(entry.0),
    )

    @put (self, key: K, value: V) -> void = run(
        if self.map.has(key) then
            self.order.remove(self.map[key].1)
        else if self.map.len() >= self.capacity then
            let oldest = self.order.pop_back()?,
            self.map.remove(oldest),

        let node = self.order.push_front(key),
        self.map.insert(key, (value, node)),
    )
}
```

### Top K elements

```sigil
use std.collections { PriorityQueue }

@top_k<T: Comparable> (items: [T], k: int) -> [T] = run(
    let pq = PriorityQueue<T>.new_min(),  // min-heap
    for item in items do run(
        pq.push(item),
        if pq.len() > k then pq.pop(),
    ),
    pq.to_list().reverse(),
)
```

---

## See Also

- [Prelude Collections](../prelude.md) — Built-in `[T]`, `{K: V}`, `Set<T>`
- [Compound Types](../../design/03-type-system/02-compound-types.md)

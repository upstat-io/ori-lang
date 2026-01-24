# Proposal: Hive Container

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-22

---

## Summary

Add `Hive<T>` to the standard library — a container optimized for frequent insertion and erasure while maintaining stable references. Based on C++26's `std::hive`.

```sigil
use std.collections { Hive }

let entities: Hive<Entity> = Hive.new()

// Insert returns a stable handle
let player = entities.insert(Entity { name: "Player", health: 100 })
let enemy = entities.insert(Entity { name: "Enemy", health: 50 })

// Handles remain valid after other insertions/erasures
entities.erase(enemy)
entities[player].health = 80  // Still valid!

// Iteration skips erased slots automatically
for entity in entities do print(entity.name)  // Prints "Player"
```

---

## Motivation

### The Problem

Common collection types have tradeoffs for dynamic element management:

| Collection | Insert | Erase | Stable References |
|------------|--------|-------|-------------------|
| `[T]` (list) | O(1) amortized | O(n) | No |
| `{K: V}` (map) | O(1) amortized | O(1) | Via key only |
| `Set<T>` | O(1) amortized | O(1) | No |

For applications with frequent insertions and deletions where elements reference each other, none of these work well:

- **Lists**: Erase is O(n), and indices shift after erasure
- **Maps**: Require generating/managing keys, overhead for key storage
- **Sets**: Elements can't be modified, no stable handles

### Use Cases

1. **Entity-Component Systems (ECS)**: Game entities frequently spawn/despawn, components reference entities
2. **Graph structures**: Nodes reference other nodes, nodes are added/removed
3. **Particle systems**: Many particles created/destroyed per frame
4. **Object pools**: Reusing slots for objects with similar lifetimes
5. **Document editors**: Paragraphs/elements with references, frequent add/delete

### Prior Art

| Language/Library | Container | Notes |
|-----------------|-----------|-------|
| C++26 | `std::hive` | Standardized "colony" pattern |
| C++ plf | `plf::colony` | Original implementation |
| Rust | `slotmap` crate | Generational indices |
| Rust | `slab` crate | Simpler slot reuse |

C++26's `std::hive` was standardized after years of usage as `plf::colony`. It's proven valuable for high-performance applications.

---

## Design

### Core Concepts

**Hive** is a container where:

1. **Insertion is O(1)** — Reuses erased slots or allocates new
2. **Erasure is O(1)** — Marks slot as free, doesn't move elements
3. **References stay valid** — Other elements don't move on insert/erase
4. **Iteration skips gaps** — Only visits live elements

### Type Definition

```sigil
type Handle<T> = { generation: int, index: int }

type Hive<T> = {
    // Internal: blocks of elements with free-list
    // Implementation details hidden
}
```

### API

#### Creation

```sigil
Hive.new<T>() -> Hive<T>                    // Empty hive
Hive.with_capacity<T>(n: int) -> Hive<T>    // Pre-allocate space
Hive.from<T>(items: [T]) -> Hive<T>         // From list
```

#### Insertion

```sigil
.insert(item: T) -> Handle<T>               // Insert, get handle
.insert_all(items: [T]) -> [Handle<T>]      // Bulk insert
```

#### Access

```sigil
.[handle] -> T                              // Get by handle (panics if invalid)
.get(handle: Handle<T>) -> Option<T>        // Safe get
.contains(handle: Handle<T>) -> bool        // Check if handle valid
```

#### Erasure

```sigil
.erase(handle: Handle<T>) -> void           // Remove element
.erase(handle: Handle<T>) -> Option<T>      // Remove and return
.clear() -> void                            // Remove all
```

#### Iteration

```sigil
for item in hive do ...                     // Iterate values
for (handle, item) in hive.entries() do ... // Iterate with handles
```

#### Size

```sigil
len(hive) -> int                            // Number of live elements
.capacity -> int                            // Total slots (live + free)
is_empty(hive) -> bool                      // No live elements
```

### Handle Safety

Handles include a **generation counter** to detect use-after-erase:

```sigil
let entities: Hive<Entity> = Hive.new()

let h1 = entities.insert(Entity { id: 1 })
entities.erase(h1)

let h2 = entities.insert(Entity { id: 2 })  // Might reuse h1's slot

// h1 is stale, h2 is valid
entities.get(h1)  // Returns None (generation mismatch)
entities.get(h2)  // Returns Some(Entity { id: 2 })

entities[h1]      // PANIC: stale handle
entities[h2]      // Returns Entity { id: 2 }
```

### Memory Layout

Hive uses blocks of elements with a free-list:

```
Hive<T>:
+------------------+------------------+------------------+
| Block 0          | Block 1          | Block 2          |
| [E][_][E][E][_]  | [E][E][E][_][E]  | [_][_][_][_][_]  |
+------------------+------------------+------------------+
  ^   ^       ^
  |   |       +-- free slot (in free-list)
  |   +---------- free slot
  +-------------- live element

Free-list links free slots for O(1) allocation
```

---

## Examples

### Entity-Component System

```sigil
use std.collections { Hive }

type Entity = {
    name: str,
    position: Vec2,
    health: int
}

type GameWorld = {
    entities: Hive<Entity>
}

@spawn_entity (world: GameWorld, name: str, pos: Vec2) -> Handle<Entity> = run(
    world.entities.insert(Entity {
        name,
        position: pos,
        health: 100
    })
)

@despawn_entity (world: GameWorld, handle: Handle<Entity>) -> void = run(
    world.entities.erase(handle)
)

@update_all (world: GameWorld) -> void = run(
    for entity in world.entities do run(
        entity.position = entity.position + Vec2 { x: 1.0, y: 0.0 }
    )
)
```

### Graph with Dynamic Nodes

```sigil
use std.collections { Hive }

type Node = {
    value: int,
    neighbors: [Handle<Node>]
}

type Graph = {
    nodes: Hive<Node>
}

@add_node (g: Graph, value: int) -> Handle<Node> = run(
    g.nodes.insert(Node { value, neighbors: [] })
)

@connect (g: Graph, from: Handle<Node>, to: Handle<Node>) -> void = run(
    g.nodes[from].neighbors.push(to)
)

@remove_node (g: Graph, handle: Handle<Node>) -> void = run(
    // Remove references to this node from all neighbors
    for node in g.nodes do run(
        node.neighbors = filter(
            over: node.neighbors,
            predicate: h -> h != handle
        )
    ),
    g.nodes.erase(handle)
)
```

### Particle System

```sigil
use std.collections { Hive }

type Particle = {
    position: Vec2,
    velocity: Vec2,
    lifetime: float
}

type ParticleSystem = {
    particles: Hive<Particle>
}

@spawn (sys: ParticleSystem, pos: Vec2, vel: Vec2) -> Handle<Particle> = run(
    sys.particles.insert(Particle {
        position: pos,
        velocity: vel,
        lifetime: 2.0
    })
)

@update (sys: ParticleSystem, dt: float) -> void = run(
    let to_remove: [Handle<Particle>] = [],

    for (handle, p) in sys.particles.entries() do run(
        p.position = p.position + p.velocity * dt,
        p.lifetime = p.lifetime - dt,
        if p.lifetime <= 0.0 then to_remove.push(handle)
    ),

    for handle in to_remove do sys.particles.erase(handle)
)
```

### Object Pool Pattern

```sigil
use std.collections { Hive }

type Connection = {
    socket: Socket,
    last_active: Duration
}

type ConnectionPool = {
    connections: Hive<Connection>
}

@acquire (pool: ConnectionPool) -> Handle<Connection> = run(
    pool.connections.insert(Connection {
        socket: Socket.connect("server:8080"),
        last_active: now()
    })
)

@release (pool: ConnectionPool, handle: Handle<Connection>) -> void = run(
    pool.connections[handle].socket.close(),
    pool.connections.erase(handle)
)

@cleanup_stale (pool: ConnectionPool, timeout: Duration) -> void = run(
    let now_time = now(),
    let stale: [Handle<Connection>] = [],

    for (handle, conn) in pool.connections.entries() do run(
        if now_time - conn.last_active > timeout then stale.push(handle)
    ),

    for handle in stale do release(pool, handle)
)
```

---

## API Reference

### Creation

```sigil
Hive.new<T>() -> Hive<T>
Hive.with_capacity<T>(n: int) -> Hive<T>
Hive.from<T>(items: [T]) -> Hive<T>
```

### Insertion

```sigil
.insert(item: T) -> Handle<T>
.insert_all(items: [T]) -> [Handle<T>]
```

### Access

```sigil
.[handle: Handle<T>] -> T                    // Panics if invalid
.get(handle: Handle<T>) -> Option<T>
.contains(handle: Handle<T>) -> bool
```

### Modification

```sigil
.[handle: Handle<T>] = value                 // Update element
.update(handle: Handle<T>, f: T -> T) -> void
```

### Erasure

```sigil
.erase(handle: Handle<T>) -> void
.remove(handle: Handle<T>) -> Option<T>      // Erase and return
.clear() -> void
```

### Iteration

```sigil
for item in hive do ...                      // Values only
for (handle, item) in hive.entries() do ...  // Handle + value
hive.handles() -> [Handle<T>]                // All valid handles
```

### Size & Capacity

```sigil
len(hive) -> int
.capacity -> int
is_empty(hive) -> bool
.shrink_to_fit() -> void                     // Release unused blocks
```

---

## Design Rationale

### Why Generational Handles?

Simple indices can become stale:

```sigil
let h = hive.insert(x)  // h.index = 5
hive.erase(h)
let h2 = hive.insert(y) // Reuses slot 5

hive[h]  // Would return y, not x! BAD
```

Generational handles solve this:

```sigil
let h = hive.insert(x)  // h = { generation: 1, index: 5 }
hive.erase(h)           // Increment generation at slot 5
let h2 = hive.insert(y) // h2 = { generation: 2, index: 5 }

hive[h]   // Generation mismatch (1 != 2), panic
hive[h2]  // Correct generation, returns y
```

### Why Not Expose Indices?

Exposing raw indices would:

1. Allow stale index bugs
2. Break encapsulation
3. Couple user code to implementation

Handles are opaque for safety.

### Why Blocks Instead of Single Array?

Hive uses multiple blocks because:

1. **No reallocation**: Adding blocks doesn't move existing elements
2. **Incremental growth**: Only allocate as needed
3. **Better cache behavior**: Smaller working sets

### Panic vs. Option for Access

| Operation | Returns | Rationale |
|-----------|---------|-----------|
| `hive[h]` | `T` (panics) | Like list indexing, use when handle known valid |
| `hive.get(h)` | `Option<T>` | Safe access when validity uncertain |

This matches Sigil's pattern for lists: `list[i]` panics, safe alternatives available.

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `insert` | O(1) | Reuses free slot or allocates |
| `erase` | O(1) | Adds to free-list |
| `[handle]` | O(1) | Direct index + generation check |
| Iteration | O(n) | n = live elements, skips gaps |
| `len` | O(1) | Maintained counter |

Memory overhead:
- Per-element: generation counter (8 bytes) + next-free pointer when erased
- Per-block: block header with size/free-list head
- Total: ~10-20% overhead typical

---

## Comparison to Alternatives

### vs. `[T]` with Index

```sigil
// List approach
let entities: [Entity] = []
entities.push(e1)  // index 0
entities.push(e2)  // index 1
entities.remove(0) // Now e2 is at index 0! References broken.
```

Hive doesn't shift elements, so references stay valid.

### vs. `{int: T}` Map

```sigil
// Map approach
let entities: {int: Entity} = {}
let next_id = 0

let id1 = next_id
next_id = next_id + 1
entities[id1] = e1

// Works, but:
// - Must manage ID generation
// - Map has hashing overhead
// - Iteration order undefined
```

Hive handles ID generation internally and has less overhead.

### vs. Rust's SlotMap

Very similar! Sigil's Hive is essentially a SlotMap:
- Generational indices for safety
- O(1) insert/erase
- Stable references

Differences:
- Sigil integrates with standard collection patterns
- Naming follows C++26 standard

---

## Implementation Notes

### Internal Structure

```sigil
type Block<T> = {
    elements: [Option<T>],    // Some = live, None = free
    generations: [int],       // Generation per slot
    free_head: Option<int>    // Head of free-list within block
}

type Hive<T> = {
    blocks: [Block<T>],
    len: int,
    global_free: Option<(int, int)>  // (block_idx, slot_idx)
}
```

### Block Sizing

Blocks grow geometrically (like vector capacity):
- Block 0: 64 elements
- Block 1: 128 elements
- Block 2: 256 elements
- etc.

### Iteration Skip-Field

For efficient iteration over sparse hives, use a skip-field:

```
Block: [E][_][_][E][E][_][E][_]
Skip:  [0][2][ ][0][0][1][0][ ]
         ^   ^       ^
         |   |       +-- skip 1 to next live
         |   +---------- (part of skip)
         +-------------- skip 2 to next live
```

This allows O(live elements) iteration, not O(capacity).

---

## Future Extensions

### Typed Handles

Prevent mixing handles from different hives:

```sigil
type EntityHandle = Handle<Entity>    // Distinct from Handle<Component>
```

### Parallel Iteration

```sigil
parallel(over: hive, op: entity -> update(entity))
```

### Serialization

```sigil
// Serialize/deserialize hive state
hive.serialize() -> [byte]
Hive.deserialize<T>(bytes: [byte]) -> Hive<T>
```

---

## Summary

`Hive<T>` provides:

1. **O(1) insert and erase** — No element shifting
2. **Stable references** — Handles remain valid across mutations
3. **Safe handles** — Generational indices detect stale access
4. **Efficient iteration** — Skips erased slots automatically
5. **Proven design** — Based on C++26's `std::hive`

Ideal for:
- Entity-component systems
- Graphs with dynamic nodes
- Particle systems
- Object pools
- Any collection with frequent add/remove and inter-element references

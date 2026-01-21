# Automatic Reference Counting (ARC)

This document covers Sigil's memory management strategy: Automatic Reference Counting with backup cycle collection.

---

## Overview

Sigil uses Automatic Reference Counting (ARC) as its primary memory management strategy. This provides:

- **Simple mental model** - Values live until nothing references them
- **Deterministic destruction** - Objects are destroyed immediately when unreferenced
- **No lifetime annotations** - AI and developers don't reason about borrows
- **Predictable performance** - No garbage collection pauses

```sigil
@process_data (input: str) -> Result<Data, Error> = run(
    let data = parse(input),      // data created, refcount = 1
    let validated = validate(data), // data still referenced
    let result = transform(validated),
    Ok(result),
    // data destroyed here when function returns
)
```

---

## Why ARC?

### Memory Management Strategy Comparison

| Strategy | AI Complexity | Runtime Cost | Determinism | Learning Curve |
|----------|---------------|--------------|-------------|----------------|
| Garbage Collection | Low | Medium (pauses) | No | Low |
| **ARC** | **Low** | **Low** | **Yes** | **Low** |
| Ownership/Borrowing | High | None | Yes | High |
| Manual | Very High | None | Yes | Very High |
| Arena/Region | Medium | Low | Scoped | Medium |

### Why Not Garbage Collection?

Garbage collection (GC) is the default choice for many modern languages (Java, Go, JavaScript). However, GC has drawbacks for Sigil's goals:

**Non-deterministic destruction:**
```sigil
// With GC, when is 'file' cleaned up?
@read_all (path: str) -> str = run(
    let file = open(path),
    let content = read(file),
    close(file),  // Must explicitly close!
    content,
)
```

With GC, the `file` handle might not be cleaned up until an unpredictable later time. This forces explicit resource management, adding boilerplate and opportunities for leaks.

**Pause times:**
- GC pauses can be problematic for latency-sensitive applications
- Even "low-pause" collectors have some impact
- Harder to reason about performance characteristics

**Memory overhead:**
- GC requires tracking metadata for all allocations
- Often needs 2-3x the working set for efficient collection
- Heap fragmentation can increase memory usage

### Why Not Ownership (Rust-style)?

Rust's ownership system eliminates runtime overhead but introduces significant complexity:

**Borrow checker errors are common:**
```rust
// Rust code that AI might generate
fn process(data: &mut Vec<i32>) {
    let first = &data[0];  // immutable borrow
    data.push(42);          // ERROR: mutable borrow while immutable exists
    println!("{}", first);
}
```

**Lifetime annotations add complexity:**
```rust
// Complex lifetime annotations
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}
```

**AI impact:**
- Borrow checker errors are the #1 pain point for Rust developers
- AI would generate lifetime errors constantly
- Requires complex reasoning about references, moves, borrows
- Iteration loops to fix borrow errors are expensive in AI token usage

### Why ARC Wins for Sigil

**Simple mental model:**
```sigil
// AI can reason: "value lives until nothing references it"
@example () -> int = run(
    let x = create_value(),  // x has refcount 1
    let y = x,               // refcount now 2
    use(y),              // both x and y valid
    let result = compute(x),
    result,
    // x, y destroyed when function returns
)
```

**No lifetime reasoning:**
```sigil
// No annotations needed - just works
@longest (a: str, b: str) -> str =
    if len(a) > len(b) then a else b
```

**Deterministic cleanup:**
```sigil
// Resources released immediately when unreferenced
@safe_file_read (path: str) -> Result<str, Error> = try(
    let handle = open(path)?,
    let content = read(handle)?,
    // handle destroyed here - file closed automatically
    Ok(content),
)
```

---

## How ARC Works

### Reference Counting Basics

Every heap-allocated value has an associated reference count:

```
Value on heap:
+----------------+
| refcount: 2    |  <- Number of references to this value
+----------------+
| actual data    |
+----------------+
```

**Operations:**
1. **Create** - New value starts with refcount = 1
2. **Copy reference** - Increment refcount
3. **Drop reference** - Decrement refcount
4. **Destroy** - When refcount reaches 0, free memory

### Reference Count Operations

```sigil
@demonstrate_refcount () -> void = run(
    // 1. Create: list allocated, refcount = 1
    let original = [1, 2, 3],

    // 2. Copy reference: refcount = 2
    let alias = original,

    // 3. Pass to function: refcount = 3 during call
    process(original),

    // 4. After call returns: refcount back to 2
    // 5. Function ends: both references dropped, refcount = 0
    // 6. Destroy: memory freed
    void,
)
```

### When References Are Created

References are created (refcount incremented) when:

| Operation | Example | Effect |
|-----------|---------|--------|
| Assignment | `y = x` | +1 refcount |
| Function argument | `f(x)` | +1 during call |
| Struct field | `{ data: x }` | +1 refcount |
| List element | `[x, y, z]` | +1 per element |
| Return value | `return x` | +1 (caller's reference) |
| Closure capture | `n -> n + x` | +1 captured |

### When References Are Dropped

References are dropped (refcount decremented) when:

| Operation | Example | Effect |
|-----------|---------|--------|
| Variable goes out of scope | End of `run` block | -1 refcount |
| Reassignment (shadowing) | `x = new_value` | -1 for old |
| Function return | Caller doesn't use result | -1 refcount |
| Struct destruction | Containing struct freed | -1 per field |
| List destruction | List freed | -1 per element |

---

## Destruction Timing

### Immediate Destruction

When a reference count reaches zero, the value is destroyed immediately:

```sigil
@immediate_destruction () -> void = run(
    // Value created
    let temp = expensive_computation(),

    // Value used
    let result = process(temp),

    // 'temp' no longer referenced after this point
    // Destroyed HERE, not at function end

    more_work(result),

    let final_result = finish(result),
    // 'result' destroyed here
)
```

### Scope-Based Destruction

Values are destroyed when their containing scope ends:

```sigil
@scope_destruction () -> void = run(
    let outer = create_outer(),

    let inner_result = run(
        let inner = create_inner(),
        compute(inner, outer),
        // 'inner' destroyed here - end of inner run
    ),

    use(inner_result, outer),
    // 'outer' and 'inner_result' destroyed here
)
```

### Resource Management

ARC enables automatic resource cleanup without explicit close/dispose calls:

```sigil
type FileHandle = {
    fd: int,
    path: str
}

// Destructor runs when refcount hits 0
impl Drop for FileHandle {
    @drop (self) -> void = close_fd(self.fd)
}

@safe_read (path: str) -> Result<str, Error> = try(
    let handle = open(path)?,       // FileHandle created
    let content = read_all(handle)?, // Used
    Ok(content),
    // handle's refcount -> 0, drop() called automatically
    // File descriptor closed
)
```

### Destruction Order

When multiple values are destroyed at scope end, destruction happens in reverse creation order:

```sigil
@destruction_order () -> void = run(
    let first = create_a(),   // Created 1st
    let second = create_b(),  // Created 2nd
    let third = create_c(),   // Created 3rd
    compute(first, second, third),
    // Destroyed: third, second, first (reverse order)
)
```

This ensures values don't reference already-destroyed values.

---

## Cycle Handling

### The Cycle Problem

ARC cannot handle reference cycles - values that reference each other:

```sigil
// Hypothetical cycle - this would leak memory
type Node = {
    value: int,
    next: Option<Node>  // Reference to another Node
}

// If A -> B -> A, neither can reach refcount 0
```

Without cycle handling:
- A references B (B's refcount >= 1)
- B references A (A's refcount >= 1)
- Neither can be freed, even when unreachable from program

### Sigil's Solution: Backup Cycle Collector

Sigil uses a backup cycle collector that runs periodically:

**How it works:**
1. Normal ARC handles most allocations efficiently
2. Cycle detector runs when triggered (memory pressure, explicit call)
3. Identifies unreachable cycles using mark-sweep algorithm
4. Frees cyclic garbage

**When cycle collection runs:**
- Memory allocation exceeds threshold
- Explicit call to `std.memory.collect_cycles()`
- Program idle time
- Before program termination

```sigil
use std.memory { collect_cycles }

// Force cycle collection (rarely needed)
collect_cycles()
```

### Why Cycles Are Rare in Sigil

Sigil's design minimizes cycles:

**Immutable data structures:**
```sigil
// Immutable data naturally forms trees, not graphs
type Tree<T> = Leaf(T) | Branch(left: Tree<T>, right: Tree<T>)

// Children don't reference parents - no cycles possible
```

**Functional patterns:**
```sigil
// Fold, map, filter create new values, don't mutate
@transform (items: [int]) -> [int] =
    map(filter(items, n -> n > 0), n -> n * 2)
// No cycles created
```

**Value semantics:**
```sigil
// Creating new values instead of mutating
@process (data: Data) -> Data =
    Data { id: data.id, value: data.value, processed: true }
    // New value, original unchanged
```

### When Cycles Can Occur

Cycles are possible with certain patterns:

**Graph structures:**
```sigil
type GraphNode = {
    id: int,
    edges: [GraphNode]  // Can form cycles
}
```

**Parent-child relationships:**
```sigil
type TreeNode = {
    value: int,
    parent: Option<TreeNode>,  // Parent reference
    children: [TreeNode]
}
```

**Callback patterns:**
```sigil
type EventHandler = {
    owner: Widget,
    callback: (Event) -> void  // May capture owner
}
```

### Designing Around Cycles

When possible, design to avoid cycles:

**Use IDs instead of references:**
```sigil
type GraphNode = {
    id: int,
    edge_ids: [int]  // IDs, not references
}

type Graph = {
    nodes: Map<int, GraphNode>
}
```

**Use weak references for back-pointers:**
```sigil
type TreeNode = {
    value: int,
    parent_id: Option<int>,  // ID, not reference
    children: [TreeNode]
}
```

**Break cycles explicitly:**
```sigil
@cleanup_graph (graph: Graph) -> void = run(
    // Clear references before dropping
    map(graph.nodes, node -> GraphNode { id: node.id, edges: [] }),
    void,
)
```

---

## Performance Characteristics

### Reference Counting Overhead

Every reference copy and drop has a small cost:

| Operation | Cost |
|-----------|------|
| Increment refcount | ~1-3 CPU cycles |
| Decrement refcount | ~1-3 CPU cycles |
| Check for zero | ~1 CPU cycle |
| Destroy value | Proportional to value size |

### When ARC Overhead Matters

**Tight loops with many copies:**
```sigil
// Potentially expensive: many refcount operations
@inefficient (items: [Data]) -> int =
    fold(items, 0, (acc, item) ->
        // 'item' copied each iteration
        acc + compute(item)
    )
```

**Better: minimize copies:**
```sigil
// Compiler can optimize to avoid unnecessary copies
@efficient (items: [Data]) -> int =
    fold(items, 0, (acc, item) -> acc + item.value)
```

### Compiler Optimizations

The Sigil compiler applies several optimizations:

**Elision of increments:**
```sigil
// Compiler knows 'x' won't be used after passing to 'f'
@example () -> int = run(
    let x = create(),
    f(x),  // No refcount increment needed - ownership transfer
)
```

**Inline small values:**
```sigil
// Small structs copied by value, no refcounting
type Point = { x: int, y: int }  // 16 bytes - value type

@move (p: Point, dx: int, dy: int) -> Point =
    { x: p.x + dx, y: p.y + dy }  // No heap allocation
```

**Copy-on-write for unique references:**
```sigil
// If refcount == 1, modify in place instead of copying
let list = [1, 2, 3]
let list2 = list + [4]  // If 'list' unique, reuse allocation
```

---

## Comparison with Other Languages

### Swift

Swift uses ARC similarly to Sigil:

| Feature | Swift | Sigil |
|---------|-------|-------|
| Core strategy | ARC | ARC |
| Cycle handling | Weak/unowned references | Backup cycle collector |
| Value types | Explicit `struct` | Automatic for small types |
| Destruction | Immediate | Immediate |

**Key difference:** Swift requires manual `weak` and `unowned` annotations for cycles. Sigil handles this automatically.

### Python (CPython)

CPython uses reference counting plus cycle GC:

| Feature | CPython | Sigil |
|---------|---------|-------|
| Core strategy | Reference counting | ARC |
| Cycle handling | Generational GC | Backup cycle collector |
| Performance | Interpreted, slower | Compiled, faster |
| Memory safety | Runtime checks | Compile-time types |

**Key difference:** Sigil is compiled with static typing, enabling better optimizations.

### Objective-C

Objective-C (with ARC enabled) pioneered compiler-managed reference counting:

| Feature | Objective-C | Sigil |
|---------|-------------|-------|
| Core strategy | ARC | ARC |
| Cycle handling | Manual weak | Automatic |
| Type system | Dynamic | Static |
| Nil handling | Nil messaging | Option types |

**Key difference:** Sigil uses static typing and `Option<T>` instead of nullable types.

---

## Memory Layout

### Value Types

Small types are stored inline (no heap allocation):

```
Stack frame:
+------------------+
| x: int = 42      |  <- 8 bytes inline
+------------------+
| y: bool = true   |  <- 1 byte (padded)
+------------------+
| p: Point         |  <- 16 bytes inline
|   x: 10          |
|   y: 20          |
+------------------+
```

### Reference Types

Larger types store a pointer to heap data:

```
Stack frame:                     Heap:
+------------------+             +------------------+
| list: ref -------|------------>| refcount: 1      |
+------------------+             | length: 3        |
                                 | data: [1, 2, 3]  |
                                 +------------------+
```

### Threshold for Reference Types

Sigil uses these heuristics:

| Type | Storage | Threshold |
|------|---------|-----------|
| `int`, `float`, `bool` | Value | Always |
| Small structs | Value | <= 32 bytes, primitives only |
| `str` | Reference (with SSO) | See [Strings and Lists](03-strings-and-lists.md) |
| `[T]` | Reference | Always |
| Large structs | Reference | > 32 bytes or contains references |

---

## Implementation Notes

### Reference Count Storage

Reference counts are stored adjacent to the data:

```c
// Generated C code structure
typedef struct {
    size_t refcount;
    // ... actual data follows
} SigilHeapObject;
```

### Thread Safety

For single-threaded code:
- Simple increment/decrement operations
- No synchronization overhead

For multi-threaded code:
- Atomic reference count operations
- Small overhead per operation (~10-20 cycles)

### Debug Mode

In debug builds, Sigil tracks:
- Allocation source locations
- Reference count history
- Leak detection at program end

```sigil
// Running with --debug-memory
@main () -> void = run(
    let data = create_data(),
    // Intentionally don't use 'data'
    void,
)
// Warning: value created at line 2 was never used
```

---

## Best Practices

### 1. Let the Compiler Optimize

Don't manually manage references:

```sigil
// Don't do this - unnecessary complexity
@manual () -> int = run(
    let x = create(),
    let x_copy = x,      // Why copy? Just use x
    use(x_copy),
    // x still exists but unused - compiler handles cleanup
    result,
)

// Do this - simple and clear
@automatic () -> int = run(
    let x = create(),
    use(x),
    result,
    // x automatically cleaned up at scope end
)
```

### 2. Prefer Small Value Types

Design structs to be value types when possible:

```sigil
// Good: small, immutable, value type
type Point = { x: int, y: int }
type Color = { r: int, g: int, b: int, a: int }

// Acceptable: larger, becomes reference type
type Matrix4x4 = {
    m00: float, m01: float, m02: float, m03: float,
    m10: float, m11: float, m12: float, m13: float,
    m20: float, m21: float, m22: float, m23: float,
    m30: float, m31: float, m32: float, m33: float
}
```

### 3. Avoid Unnecessary Aliasing

Create new values instead of sharing references:

```sigil
// Less efficient: creates alias
@share (data: Data) -> { a: Data, b: Data } = run(
    { a: data, b: data },  // Both reference same data
)

// Often better: explicit copy if needed
@copy (data: Data) -> { a: Data, b: Data } = run(
    { a: data, b: data.clone() },
)
```

### 4. Use Structured Patterns

Patterns like `map`, `fold`, `filter` are optimized for ARC:

```sigil
// Good: pattern handles memory efficiently
@process (items: [int]) -> [int] =
    map(filter(items, n -> n > 0), n -> n * 2)

// Avoid: manual iteration with accumulation
@manual (items: [int]) -> [int] =
    fold(items, [], (acc, n) ->
        if n > 0 then acc + [n * 2] else acc
    )  // Creates many intermediate lists
```

---

## See Also

- [Value Semantics](02-value-semantics.md) - Immutability and bindings
- [Strings and Lists](03-strings-and-lists.md) - SSO and structural sharing
- [Type System](../03-type-system/index.md) - Type definitions
- [Primitive Types](../03-type-system/01-primitive-types.md) - Value types

# Proposal: Fixed-Capacity Lists

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-22

---

## Summary

Introduce fixed-capacity lists — stack-allocated lists with a compile-time maximum size. Useful for embedded systems, performance-critical code, and bounded buffers.

```ori
// Type syntax: list of int with maximum capacity 10
let buffer: [int, max 10] = []

// Can hold 0 to 10 elements
buffer.push(1)   // length: 1
buffer.push(2)   // length: 2
// ... up to 10 elements

buffer.push(11)  // PANIC: capacity exceeded
```

---

## Motivation

### The Problem

Regular Ori lists (`[T]`) are heap-allocated and grow dynamically. This is flexible but has costs:

1. **Heap allocation**: Each list requires heap memory management
2. **Unbounded growth**: No compile-time guarantee on memory usage
3. **Indirection**: Pointer to heap adds cache miss potential

For some use cases, these tradeoffs are unacceptable:

- **Embedded systems**: Limited memory, no heap allocator
- **Real-time code**: Heap allocation has unpredictable latency
- **Bounded buffers**: Protocol requires fixed maximum size
- **Performance-critical inner loops**: Stack allocation is faster

### Prior Art

| Language | Feature | Syntax |
|----------|---------|--------|
| C | Fixed arrays | `int arr[10]` |
| C++ | `std::array` | `std::array<int, 10>` |
| C++26 | `std::inplace_vector` | `std::inplace_vector<int, 10>` |
| Rust | Fixed arrays | `[i32; 10]` |
| Rust | `ArrayVec` (crate) | `ArrayVec<i32, 10>` |

C++26's `inplace_vector` is particularly relevant — it's a stack-allocated vector with dynamic length up to a fixed capacity. This is exactly what we want.

### Why Not Rust's Syntax?

Rust uses `[T; N]` for fixed-size arrays:

```rust
let arr: [i32; 10] = [0; 10];
```

Ori doesn't use semicolons, so this syntax is unavailable and would be confusing. We need a different approach.

---

## Design

### Type Syntax

```ori
[T, max N]
```

Where:
- `T` is the element type
- `N` is a compile-time constant (positive integer literal or `$config` value)

**Examples:**

```ori
[int, max 10]         // List of int, max 10 elements
[str, max 256]        // List of str, max 256 elements
[Point, max 100]      // List of Point, max 100 elements

$buffer_size = 64
[byte, max $buffer_size]  // Using config constant
```

**Reading it naturally:** "list of int, max 10" — reads like English.

### Literal Syntax

Fixed-capacity list literals include the capacity:

```ori
// Empty fixed-capacity list
let buffer: [int, max 10] = []

// Pre-populated
let coords: [int, max 3] = [1, 2, 3]

// Type inference from literal with explicit capacity
let items = [int, max 5] [1, 2, 3]  // Type: [int, max 5], length: 3
```

### Runtime Behavior

Fixed-capacity lists have two size concepts:

- **Capacity**: Maximum elements, fixed at compile time
- **Length**: Current number of elements, dynamic at runtime

```ori
let buffer: [int, max 10] = [1, 2, 3]
len(buffer)       // 3 (current length)
buffer.capacity   // 10 (maximum capacity)
buffer.is_full    // false
```

### Operations

All standard list operations work, with capacity checks:

```ori
let buffer: [int, max 5] = []

// Adding elements
buffer.push(1)           // OK: length 1
buffer.push(2)           // OK: length 2
buffer.push_all([3, 4])  // OK: length 4
buffer.push(5)           // OK: length 5 (at capacity)
buffer.push(6)           // PANIC: capacity exceeded

// Safe alternatives
buffer.try_push(6)           // Returns false, no panic
buffer.push_or_drop(6)       // Silently drops if full

// Removing elements
buffer.pop()             // Returns Some(5), length 4
buffer.clear()           // Length 0, capacity still 5

// Iteration (same as regular lists)
for x in buffer do print(x)
map(over: buffer, transform: x -> x * 2)
```

### Type Compatibility

Fixed-capacity lists are a subtype of regular lists:

```ori
@process (items: [int]) -> int = fold(over: items, init: 0, op: +)

let fixed: [int, max 10] = [1, 2, 3]
process(fixed)  // OK: [int, max 10] is assignable to [int]
```

The reverse is not true:

```ori
@process_fixed (items: [int, max 10]) -> int = ...

let dynamic: [int] = [1, 2, 3]
process_fixed(dynamic)  // ERROR: cannot guarantee capacity
```

### Generic Functions

Functions can be generic over capacity:

```ori
@swap_ends<T, N> (items: [T, max N]) -> [T, max N] = run(
    .pre_check: len(items) >= 2,
    let first = items[0],
    let last = items[# - 1],
    let result = items.clone(),
    result[0] = last,
    result[# - 1] = first,
    result
)
```

### Memory Layout

Fixed-capacity lists are stored inline (stack or within containing struct):

```ori
type Packet = {
    header: Header,
    payload: [byte, max 1500],  // Stored inline, not on heap
    checksum: int
}

// sizeof(Packet) includes space for 1500 bytes
```

---

## Examples

### Network Packet Buffer

```ori
type UdpPacket = {
    source_port: int,
    dest_port: int,
    payload: [byte, max 65507]  // UDP max payload
}

@parse_udp (raw: [byte]) -> Result<UdpPacket, ParseError> = run(
    .pre_check: len(raw) >= 8,
    let source_port = int(raw[0]) << 8 | int(raw[1]),
    let dest_port = int(raw[2]) << 8 | int(raw[3]),
    let payload_len = int(raw[4]) << 8 | int(raw[5]),

    if payload_len > 65507 then Err(ParseError.PayloadTooLarge),

    let payload: [byte, max 65507] = raw[8..8 + payload_len].to_fixed(),
    Ok(UdpPacket { source_port, dest_port, payload })
)
```

### Ring Buffer

```ori
type RingBuffer<T, N> = {
    data: [T, max N],
    head: int,
    tail: int
}

@push<T, N> (rb: RingBuffer<T, N>, item: T) -> RingBuffer<T, N> = run(
    let new_tail = (rb.tail + 1) % N,
    if new_tail == rb.head then panic("ring buffer full"),

    let new_data = rb.data.clone(),
    new_data[rb.tail] = item,
    RingBuffer { data: new_data, head: rb.head, tail: new_tail }
)
```

### Small Vector Optimization

```ori
// Common case: small lists inline, large lists on heap
type SmallVec<T> =
    | Inline(data: [T, max 8])
    | Heap(data: [T])

@push<T> (sv: SmallVec<T>, item: T) -> SmallVec<T> = match(sv,
    Inline(data) ->
        if len(data) < 8
        then run(data.push(item), Inline(data: data))
        else run(
            let heap = data.to_dynamic(),
            heap.push(item),
            Heap(data: heap)
        ),
    Heap(data) -> run(
        data.push(item),
        Heap(data: data)
    )
)
```

### Embedded Systems

```ori
$max_sensors = 16

type SensorArray = {
    sensors: [Sensor, max $max_sensors],
    active_count: int
}

@read_all (arr: SensorArray) -> [Reading, max $max_sensors] = run(
    collect(
        range: 0..arr.active_count,
        transform: i -> arr.sensors[i].read()
    )
)
```

---

## API Reference

### Type Properties

```ori
// For type [T, max N]
.capacity: int           // The compile-time capacity N
.is_full: bool          // len(self) == capacity
.remaining: int         // capacity - len(self)
```

### Methods

```ori
// Mutation (panics if capacity exceeded)
.push(item: T) -> void
.push_all(items: [T]) -> void

// Safe mutation (returns success/failure)
.try_push(item: T) -> bool
.try_push_all(items: [T]) -> bool

// Dropping behavior
.push_or_drop(item: T) -> void    // Drops if full
.push_or_oldest(item: T) -> void  // Drops oldest if full

// Conversion
.to_dynamic() -> [T]              // Convert to heap-allocated
[T].to_fixed<N>() -> [T, max N]   // Convert, panics if too large
[T].try_to_fixed<N>() -> Option<[T, max N]>
```

---

## Design Rationale

### Why `max` Keyword?

Alternatives considered:

| Syntax | Problem |
|--------|---------|
| `[int; 10]` | Semicolons not used in Ori |
| `[int: 10]` | Confusing with type annotations |
| `[int 10]` | Ambiguous parsing |
| `[int x 10]` | Reads oddly |
| `[int, cap 10]` | Abbreviation less clear |
| **`[int, max 10]`** | Reads naturally, clear meaning |

The `max` keyword explicitly communicates that this is a maximum capacity, not a fixed size.

### Fixed Size vs. Fixed Capacity

Two possible semantics:

1. **Fixed size**: Always exactly N elements
2. **Fixed capacity**: 0 to N elements (C++26 `inplace_vector`)

We choose **fixed capacity** because:

- More flexible (can represent empty, partial, or full)
- Matches common use cases (buffers, dynamic collections)
- Fixed-size can be expressed as "must always be full"

### Panic vs. Error on Overflow

When capacity is exceeded:

| Approach | Code | Problem |
|----------|------|---------|
| Panic | `buffer.push(x)` | Simple, but crashes |
| Result | `buffer.push(x)?` | Every push needs error handling |
| **Both** | `push()` panics, `try_push()` returns bool | Flexibility |

We provide both: `push()` panics (like index out of bounds), `try_push()` returns `bool` for graceful handling.

### Subtyping Relationship

`[T, max N]` is a subtype of `[T]` because:

- Any operation valid on `[T]` is valid on `[T, max N]`
- Fixed-capacity list can always be treated as dynamic

The reverse doesn't hold because we can't guarantee a dynamic list fits the capacity.

---

## Implementation Notes

### Memory Representation

```
[T, max N] in memory:
+----------+--------+--------+-----+--------+
| length   | elem 0 | elem 1 | ... | elem N-1 |
| (int)    |   T    |   T    |     |    T     |
+----------+--------+--------+-----+--------+
```

- Length stored inline
- N elements worth of space always allocated
- Uninitialized elements beyond length are undefined

### Compiler Changes

1. New type constructor `FixedList<T, N>`
2. Capacity `N` must be a compile-time constant
3. Generate capacity checks for `push`/`push_all`
4. Implement subtyping from `[T, max N]` to `[T]`

### Generic Capacity

Capacity can be a generic parameter:

```ori
@copy_n<T, N> (src: [T], n: int) -> [T, max N] = ...
```

The compiler must track `N` as a const-generic parameter.

---

## Comparison to C++26 `inplace_vector`

| Aspect | C++26 `inplace_vector` | Ori `[T, max N]` |
|--------|----------------------|-------------------|
| Syntax | `inplace_vector<T, N>` | `[T, max N]` |
| Stack allocated | Yes | Yes |
| Dynamic length | Yes | Yes |
| Overflow behavior | Throws/UB | Panic (or `try_push`) |
| Subtype of `vector` | No | Yes (`[T]`) |
| Generic capacity | Yes | Yes |

---

## Future Extensions

### Fixed-Size Arrays

If true fixed-size arrays are needed (always exactly N elements):

```ori
// Possible future syntax
[int, size 10]  // Exactly 10 elements, always
```

This is separate from fixed-capacity lists.

### Compile-Time Capacity Arithmetic

```ori
$header_size = 8
$payload_size = 1500
$packet_size = $header_size + $payload_size  // 1508

[byte, max $packet_size]
```

---

## Summary

Fixed-capacity lists provide:

1. **Stack allocation** — No heap, predictable memory
2. **Bounded size** — Compile-time guarantee on maximum
3. **Dynamic length** — 0 to N elements at runtime
4. **Natural syntax** — `[T, max N]` reads clearly
5. **Type compatibility** — Subtype of regular lists

Use cases:

- Embedded systems without heap
- Performance-critical code
- Network protocol buffers
- Any bounded collection with known maximum

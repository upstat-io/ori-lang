# Proposal: Sendable Trait and Interior Mutability Definition

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, type system, concurrency

---

## Summary

This proposal clarifies the `Sendable` trait by defining what "interior mutability" means in Ori's context, specifying exactly which types are Sendable, and documenting verification rules for closures.

---

## Problem Statement

The approved Sendable proposal states that types are Sendable when they have "no interior mutability," but:

1. Ori has no `Mutex`, `RefCell`, or similar types — what IS interior mutability?
2. Closure Sendability depends on captured values — how is this verified?
3. The rule "all fields are Sendable" is recursive — what are the base cases?
4. Custom types need clear guidelines for Sendable auto-implementation

---

## Interior Mutability Defined

### In Other Languages

In Rust, "interior mutability" means mutating data through a shared reference via:
- `RefCell<T>` — runtime borrow checking
- `Mutex<T>` — mutual exclusion
- `Cell<T>` — single-threaded mutation
- Atomic types — lock-free mutation

### In Ori

Ori's memory model prohibits shared mutable references entirely:

> "No shared mutable references — single ownership of mutable data"

Therefore, **interior mutability does not exist in user-defined Ori types** by language design.

### Where "Interior Mutability" Matters

The only types with interior mutability are **runtime-provided resources**:

| Type | Description | Sendable? |
|------|-------------|-----------|
| `FileHandle` | OS file descriptor | No |
| `Socket` | Network connection | No |
| `DatabaseConnection` | DB session state | No |
| `ThreadLocalStorage` | Thread-specific data | No |

These types represent external resources with identity semantics — sending them to another task would violate their invariants.

---

## Sendable Base Cases

### Primitive Types (Always Sendable)

| Type | Sendable | Reason |
|------|----------|--------|
| `int` | Yes | Pure value, no references |
| `float` | Yes | Pure value |
| `bool` | Yes | Pure value |
| `str` | Yes | Immutable, reference-counted |
| `char` | Yes | Pure value |
| `byte` | Yes | Pure value |
| `Duration` | Yes | Pure value |
| `Size` | Yes | Pure value |
| `void` | Yes | No data |
| `Never` | Yes | Never instantiated |

### Built-in Collections (Conditionally Sendable)

| Type | Sendable When |
|------|---------------|
| `[T]` | `T: Sendable` |
| `{K: V}` | `K: Sendable` and `V: Sendable` |
| `Set<T>` | `T: Sendable` |
| `Option<T>` | `T: Sendable` |
| `Result<T, E>` | `T: Sendable` and `E: Sendable` |
| `(T1, T2, ...)` | All `Ti: Sendable` |

### Function Types (Conditionally Sendable)

| Type | Sendable When |
|------|---------------|
| `(T) -> U` (no captures) | Always (pure function pointer) |
| Closure | All captured values are `Sendable` |

### Non-Sendable Types

| Type | Reason |
|------|--------|
| `FileHandle` | OS resource with thread affinity |
| `Socket` | OS resource, not safely movable |
| `DatabaseConnection` | Session state, not safely movable |
| Channel endpoints | Tied to specific async context |
| Nursery handles | Scoped to specific execution context |

---

## User-Defined Type Sendability

### Automatic Implementation

`Sendable` is automatically implemented for user-defined types when all fields are Sendable:

```ori
// Automatically Sendable (all fields are Sendable)
type Point = { x: int, y: int }
type User = { name: str, age: int, active: bool }
type Tree<T: Sendable> = { value: T, children: [Tree<T>] }

// NOT Sendable (contains non-Sendable field)
type Connection = { handle: Socket, timeout: Duration }
// Socket is not Sendable, so Connection is not Sendable
```

### No Manual Implementation

Users CANNOT manually implement `Sendable`:

```ori
// ERROR: cannot implement Sendable manually
impl Sendable for MyType { }
// Sendable is automatically derived or not available
```

**Rationale**: Sendable is a safety property verified by the compiler. Manual implementation could break thread safety.

### Opting Out

There is no way to make a type "not Sendable" if all its fields are Sendable. If you need a non-Sendable type, include a non-Sendable field:

```ori
type ThreadLocal<T> = {
    value: T,
    _marker: NonSendableMarker,  // Hypothetical marker type
}
```

---

## Closure Sendability Verification

### Capture Analysis

The compiler analyzes closure captures to determine Sendability:

```ori
let x: int = 10              // int: Sendable
let y: str = "hello"         // str: Sendable
let z: FileHandle = open()   // FileHandle: NOT Sendable

let f = () -> x + 1          // f is Sendable (captures only x: int)
let g = () -> y.len()        // g is Sendable (captures only y: str)
let h = () -> z.read()       // h is NOT Sendable (captures z: FileHandle)
```

### Transitive Capture

Closures capturing other closures inherit their Sendability:

```ori
let x: int = 10
let inner = () -> x * 2      // inner is Sendable
let outer = () -> inner()    // outer captures inner, which is Sendable
                             // outer is Sendable
```

### Task Boundary Verification

When closures cross task boundaries, the compiler verifies Sendability:

```ori
@spawn_tasks () -> void uses Async = run(
    let data = create_data(),     // data: Sendable
    let handle = open_file(),     // handle: NOT Sendable

    parallel(
        tasks: [
            () -> process(data),   // OK: captures Sendable
            () -> read(handle),    // ERROR: captures non-Sendable
        ],
    ),
)
```

Error message:
```
error[E0900]: closure is not `Sendable`
  --> src/main.ori:8:13
   |
8  |             () -> read(handle),
   |             ^^^^^^^^^^^^^^^^^^ closure captures non-Sendable value
   |
   = note: captured variable `handle` of type `FileHandle` is not Sendable
   = note: closures passed to `parallel` must be Sendable
```

---

## Channel Type Requirements

### Why Channels Require Sendable

Channel types (`Producer<T>`, `Consumer<T>`) require `T: Sendable`:

```ori
let (producer, consumer) = channel<int>(buffer: 10)      // OK
let (producer, consumer) = channel<FileHandle>(buffer: 10)  // ERROR
```

**Rationale**: Values sent through channels cross task boundaries. Non-Sendable values would violate their invariants.

### Ownership Transfer

Sending a value through a channel transfers ownership:

```ori
let data = create_data()
producer.send(value: data)  // data moved into channel
// data is no longer accessible here
```

This ensures no shared mutable access even without explicit Sendable checks.

---

## Reference Counting and Sendability

### ARC is Thread-Safe

Ori's reference counting is atomic (thread-safe) for all types:

- Incrementing refcount uses atomic operations
- Decrementing refcount uses atomic operations
- This is an implementation requirement, not user-visible

### Why This Matters

Because refcounts are atomic, sharing immutable data across tasks is safe:

```ori
let big_data = load_data()  // Reference-counted
parallel(
    tasks: [
        () -> read(big_data),   // Shares reference
        () -> analyze(big_data), // Shares reference
    ],
)
// Both tasks share the same data (reference counted atomically)
```

---

## Generic Sendable Bounds

### Constraining Generics

Generic types can require Sendable bounds:

```ori
@spawn_with<T: Sendable> (value: T, action: (T) -> void) -> void uses Async =
    parallel(tasks: [() -> action(value)])

// OK: int is Sendable
spawn_with(value: 42, action: x -> print(msg: str(x)))

// ERROR: FileHandle is not Sendable
spawn_with(value: file_handle, action: h -> h.read())
```

### Conditional Sendable

Container types often have conditional Sendability:

```ori
// Box is Sendable when T is Sendable
impl<T: Sendable> Sendable for Box<T> { }

// This enables:
let boxed: Box<int> = Box(42)  // Sendable
let boxed_handle: Box<FileHandle> = Box(h)  // NOT Sendable
```

---

## Examples

### Sendable Data Structure

```ori
type Message = {
    id: int,
    content: str,
    timestamp: Duration,
    metadata: {str: str},
}
// All fields are Sendable, so Message is Sendable

@send_messages (messages: [Message]) -> void uses Async =
    parallel(
        tasks: messages.map(m -> () -> deliver(m)),  // OK
    )
```

### Non-Sendable Resource Wrapper

```ori
type DatabasePool = {
    connections: [DatabaseConnection],  // DatabaseConnection: NOT Sendable
}
// DatabasePool is NOT Sendable

// Must use within single task:
@query_all (pool: DatabasePool, queries: [str]) -> [Result] =
    queries.map(q -> pool.query(q)).collect()  // Sequential, same task
```

### Sendable Check Error

```ori
type CacheEntry = {
    key: str,
    value: str,
    file_cache: FileHandle,  // Oops!
}
// CacheEntry is NOT Sendable due to file_cache

@parallel_cache_lookup (entries: [CacheEntry]) -> void uses Async =
    parallel(
        tasks: entries.map(e -> () -> lookup(e)),  // ERROR
    )
// Error: CacheEntry is not Sendable
```

---

## Spec Changes Required

### Update `06-types.md`

Clarify Sendable:
1. Define interior mutability in Ori context
2. List base Sendable types
3. Auto-implementation rules

### Update `14-capabilities.md`

Add:
1. Channel Sendable requirements
2. Task closure verification rules

### Add to Type Reference

Document Sendable status for all standard types.

---

## Summary

| Aspect | Specification |
|--------|--------------|
| Interior mutability | Does not exist in user code; only runtime resources |
| Base Sendable | All primitives (`int`, `float`, `bool`, `str`, etc.) |
| Collections | Sendable when elements are Sendable |
| User types | Auto-Sendable when all fields are Sendable |
| Manual impl | Not allowed |
| Closures | Sendable when all captures are Sendable |
| Verification | Compiler checks at task boundaries |
| Non-Sendable | Runtime resources (files, sockets, connections) |
| ARC | Thread-safe (atomic refcounts) |

# Memory Model

This section defines the memory management semantics of Sigil.

## Overview

Sigil uses Automatic Reference Counting (ARC) for memory management. Values are automatically deallocated when they become unreachable.

## Reference Counting

### Definition

Every heap-allocated value has an associated reference count. The reference count tracks the number of references to that value.

### Reference Count Operations

Reference counts are modified as follows:

| Operation | Effect |
|-----------|--------|
| Value creation | Reference count initialized to 1 |
| Reference copy | Reference count incremented |
| Reference drop | Reference count decremented |
| Reference count reaches 0 | Value is destroyed |

### When References Are Copied

A reference is copied (incrementing the reference count) when:

1. A value is assigned to a variable
2. A value is passed as a function argument
3. A value is stored in a struct field
4. A value is stored in a list element
5. A value is returned from a function
6. A value is captured by a closure

### When References Are Dropped

A reference is dropped (decrementing the reference count) when:

1. A variable goes out of scope
2. A variable is reassigned (the old value's reference is dropped)
3. A struct containing the reference is destroyed
4. A list containing the reference is destroyed

## Deterministic Destruction

### Destruction Timing

Values are destroyed when they become unreachable. Destruction occurs no later than the end of the enclosing scope.

### Destruction Order

When multiple values are destroyed at the end of a scope, destruction occurs in reverse order of creation.

```sigil
@example () -> void = run(
    let first = create_a(),   // Created 1st
    let second = create_b(),  // Created 2nd
    let third = create_c(),   // Created 3rd
    compute(first, second, third),
    // Destroyed: third, second, first (reverse order)
)
```

### Resource Cleanup

Destruction guarantees enable deterministic resource cleanup. When a value with associated resources (file handles, network connections, etc.) is destroyed, those resources are released.

## Cycle Prevention

### The Cycle Problem

Reference counting alone cannot reclaim cyclic data structures—values that reference each other—because their reference counts never reach zero.

### Compile-Time Cycle Prevention

Sigil prevents reference cycles at compile time. It is a compile-time error to create a structure that could form a reference cycle.

Cycles are prevented through these rules:

1. **Immutable data cannot form cycles** — A value can only reference values that existed before it was created.
2. **Mutable references are restricted** — Mutable fields cannot hold references that could complete a cycle back to the containing structure.
3. **Self-referential types are forbidden** — A type `T` cannot contain a field of type `T`, `[T]`, `Option<T>`, or any type that transitively contains `T`.

> **Note:** These restrictions align with Sigil's functional programming model. Data structures requiring cycles (graphs, doubly-linked lists) should use indices into a collection rather than direct references.

### Acyclic Data Patterns

```sigil
// Valid: reference to separately-created value
let a = create_node()
let b = Node { parent: a }  // b references a, no cycle possible

// Valid: graph via indices
type Graph = {
    nodes: [NodeData],
    edges: [(int, int)],  // indices into nodes
}

// Invalid: self-referential type (compile error)
// type LinkedNode = { next: Option<LinkedNode> }
```

## Value and Reference Semantics

### Value Types

A type uses value semantics if:

1. Its size is 32 bytes or less, AND
2. It contains only primitive fields (no reference types)

Value types are copied by value on assignment. No reference counting is performed.

### Reference Types

A type uses reference semantics if:

1. Its size exceeds 32 bytes, OR
2. It contains reference types (lists, maps, strings, or other reference-counted types)

Reference types are shared on assignment. Reference counting tracks the number of references.

### Classification

| Type | Semantics | Reason |
|------|-----------|--------|
| `int`, `float`, `bool`, `char`, `byte` | Value | Primitive, ≤8 bytes |
| `Duration`, `Size` | Value | Primitive, 8 bytes |
| `void` / `()` | Value | Zero-size |
| Small structs (≤32 bytes, primitives only) | Value | Size and composition |
| `str` | Reference | Variable size |
| `[T]` | Reference | Variable size |
| `{K: V}` | Reference | Variable size |
| `Set<T>` | Reference | Variable size |
| Large structs or structs with references | Reference | Size or composition |

### Examples

```sigil
// Value type: 16 bytes, only primitives
type Point = { x: int, y: int }

// Value type: 32 bytes, only primitives
type Color = { r: int, g: int, b: int, a: int }

// Reference type: contains str (reference type)
type User = { id: int, name: str }

// Reference type: exceeds 32 bytes
type Transform = {
    m00: float, m01: float, m02: float, m03: float,
    m10: float, m11: float, m12: float, m13: float,
}
```

## Constraints

1. It is a compile-time error to define a self-referential type.
2. It is a compile-time error to create a structure that could form a reference cycle.
3. Destruction order within a scope is guaranteed to be reverse creation order.
4. All heap-allocated values must be destroyed when their reference count reaches zero.

## Implementation Notes

> **Note:** The following are informative implementation guidelines.

### Cycle Detection

Implementations must detect potential cycles at compile time by:

1. **Type graph analysis** — Build a graph of type references and reject types with cycles.
2. **Field type validation** — Ensure no field's type transitively contains the parent type.

### Optimization

Implementations may optimize reference counting through:

1. **Elision of increments** — When the compiler can prove a value's lifetime, reference count operations may be omitted.
2. **Inline small values** — Value types should be stored inline without heap allocation.
3. **Copy-on-write** — When a reference count is 1, mutation may occur in place rather than copying.

### Debug Support

Implementations should provide:

1. Thread-safe reference counting for multi-threaded code (atomic operations)
2. Leak detection in debug builds
3. Allocation source tracking in debug builds

## See Also

- [Types](06-types.md) — Type definitions
- [Variables](05-variables.md) — Variable bindings and scoping
- [Design: Memory Model](../design/08-memory-model/01-arc-overview.md) — Rationale and detailed explanation

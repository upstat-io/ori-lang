---
title: "Memory Model"
description: "Ori Language Specification — Memory Model"
order: 15
section: "Execution"
---

# Memory Model

Ori uses Automatic Reference Counting (ARC) without cycle detection. This is made possible by language design choices that structurally prevent reference cycles.

## Why Pure ARC Works

Most languages using ARC require either cycle detection (Python, PHP) or manual weak reference annotations (Swift, Objective-C). Ori requires neither because its execution model produces directed acyclic graphs (DAGs) by construction.

### The Problem: Object Graphs

Traditional object-oriented languages build **reference graphs** where objects hold references to other objects. Cycles form naturally:

```
     ┌──────────┐
     │  Parent  │
     └────┬─────┘
          │ children
          ▼
     ┌──────────┐
     │  Child   │──── parent ───┐
     └──────────┘               │
          ▲                     │
          └─────────────────────┘
```

Common cycle sources in other languages:
- Closures capturing `self`/`this`
- Parent-child bidirectional references
- Observer/delegate callback patterns
- Event emitter subscriptions

### The Solution: Sequential Data Flow

Ori's function sequences (`run`, `try`, `match`) enforce **linear data flow**:

```
input ──▶ step A ──▶ step B ──▶ step C ──▶ output
```

Each binding in a sequence:
1. Is immutable after creation
2. Can only reference earlier bindings (forward-only)
3. Is destroyed when the sequence ends

```ori
@process (input: Data) -> Result<Output, Error> = try(
    let validated = validate(data: input)?,   // A: sees input
    let enriched = enrich(data: validated)?,  // B: sees input, validated
    let saved = save(data: enriched)?,        // C: sees input, validated, enriched
    Ok(saved),
)
```

There is no mechanism for `saved` to reference the function `process`, or for `enriched` and `validated` to reference each other bidirectionally. Data flows forward through transformations.

### Structural Guarantees

| Pattern | Data Flow | Cycle Prevention |
|---------|-----------|------------------|
| `run(a, b, c)` | Linear sequence | Each step sees only prior bindings |
| `try(a?, b?, c?)` | Linear with early exit | Same as `run` |
| `match(x, ...)` | Branching | Each branch is independent |
| `recurse(...)` | Iteration | State passed explicitly, no self-reference |
| `parallel(...)` | Fan-out/fan-in | Results collected, no cross-task references |

### Closures Capture by Value

In languages where closures capture by reference, cycles form when a closure captures `self`:

```
Object ──▶ callback field ──▶ closure ──▶ captured self ──▶ Object
```

Ori closures capture by value. The closure receives a copy of captured data, not a reference back to the containing scope:

```ori
let x = 5
let f = () -> x + 1  // f contains a copy of 5, not a reference to x
```

This eliminates the most common source of cycles in functional-style code.

### Closure Representation

A closure is represented as a struct containing captured values:

```ori
let x = 10
let y = "hello"
let f = () -> `{y}: {x}`

// f is approximately:
// type _Closure_f = { captured_x: int, captured_y: str }
```

For reference-counted types (lists, maps, custom types), the closure stores the reference (incrementing the reference count), not a deep copy of the data.

### Self-Referential Types Forbidden

The one place cycles could form is in user-defined recursive types:

```ori
// Compile error: self-referential type
type Node = { next: Option<Node> }
```

If permitted, this would allow:
```
node1.next = Some(node2)
node2.next = Some(node1)  // cycle
```

Ori forbids this at the type level. Recursive structures use indices into collections:

```ori
// Valid: indices for relationships
type Graph = { nodes: [NodeData], edges: [(int, int)] }
```

### Summary

Pure ARC works in Ori because:

1. **Sequences enforce DAGs** — Data flows forward through `run`/`try`/`match`
2. **Value capture prevents closure cycles** — No reference back to enclosing scope
3. **Type restrictions prevent structural cycles** — Self-referential types forbidden
4. **No shared mutable references** — Single ownership of mutable data

These are not conventions — they are language invariants enforced by the compiler.

## Reference Counting

| Operation | Effect |
|-----------|--------|
| Value creation | Count = 1 |
| Reference copy | Count + 1 |
| Reference drop | Count - 1 |
| Count = 0 | Value destroyed |

References are copied on assignment, argument passing, field storage, return, closure capture.

References are dropped when variables go out of scope or are reassigned.

## Destruction

Destruction occurs when values become unreachable, no later than scope end.

Reverse creation order within a scope:

```ori
run(
    let a = create_a(),  // 1st
    let b = create_b(),  // 2nd
    // destroyed: b, a
)
```

## Cycle Prevention

Cycles prevented at compile time:

1. Immutable data cannot form cycles
2. Mutable references restricted from completing cycles
3. Self-referential types forbidden

```ori
// Valid: indices
type Graph = { nodes: [Node], edges: [(int, int)] }

// Error: self-referential
type Node = { next: Option<Node> }  // compile error
```

## Value vs Reference Types

| Semantics | Criteria |
|-----------|----------|
| Value | ≤32 bytes AND primitives only |
| Reference | >32 bytes OR contains references |

**Value types** (copied):
- Primitives: `int`, `float`, `bool`, `char`, `byte`, `Duration`, `Size`
- Small structs with only primitives

**Reference types** (shared, counted):
- `str`, `[T]`, `{K: V}`, `Set<T>`
- Structs containing references
- Structs >32 bytes

```ori
type Point = { x: int, y: int }      // Value: 16 bytes, primitives
type User = { id: int, name: str }   // Reference: contains str
```

## Constraints

- Self-referential types are compile errors
- Cyclic structures are compile errors
- Destruction in reverse creation order
- Values destroyed when count reaches zero

## ARC Safety Invariants

Ori uses ARC without cycle detection. The following invariants must be maintained by all language features to ensure ARC remains viable.

### Invariant 1: Value Capture

Closures must capture variables by value. Reference captures are prohibited.

```ori
let x = 5
let f = () -> x + 1  // captures copy of x, not reference to x
```

This prevents cycles through closure environments.

### Invariant 2: No Implicit Back-References

Structures must not implicitly reference their containers. Bidirectional relationships require explicit weak references or indices.

```ori
// Valid: indices for back-navigation
type Tree = { nodes: [Node], parent_indices: [Option<int>] }

// Invalid: implicit parent reference would create cycle
type Node = { children: [Node], parent: Node }  // error
```

### Invariant 3: No Shared Mutable References

Multiple mutable references to the same value are prohibited. Shared access requires either:
- Copy-on-write semantics
- Explicit synchronization primitives with single ownership

### Invariant 4: Value Semantics Default

Types have value semantics unless explicitly boxed. Reference types require explicit opt-in through container types or `Box<T>`.

### Invariant 5: Explicit Weak References

If weak references are added to the language, they must:
- Use distinct syntax (`Weak<T>`)
- Require explicit upgrade operations returning `Option<T>`
- Never be implicitly created

### Feature Evaluation

New language features must be evaluated against these invariants. A feature that violates any invariant must either:
1. Be redesigned to maintain the invariant
2. Provide equivalent cycle prevention guarantees
3. Be rejected

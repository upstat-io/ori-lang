# Memory Model

Sigil uses Automatic Reference Counting (ARC).

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

```sigil
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

```sigil
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

```sigil
type Point = { x: int, y: int }      // Value: 16 bytes, primitives
type User = { id: int, name: str }   // Reference: contains str
```

## Constraints

- Self-referential types are compile errors
- Cyclic structures are compile errors
- Destruction in reverse creation order
- Values destroyed when count reaches zero

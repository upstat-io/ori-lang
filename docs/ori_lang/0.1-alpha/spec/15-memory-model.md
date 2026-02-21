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
1. Holds a value that is never mutated in place (reassignment replaces the value, it does not modify it)
2. Can only reference earlier bindings (forward-only)
3. Is destroyed when the sequence ends

```ori
@process (input: Data) -> Result<Output, Error> = try {
    let validated = validate(data: input)?;   // A: sees input
    let enriched = enrich(data: validated)?;  // B: sees input, validated
    let saved = save(data: enriched)?;        // C: sees input, validated, enriched

    Ok(saved)
}
```

There is no mechanism for `saved` to reference the function `process`, or for `enriched` and `validated` to reference each other bidirectionally. Data flows forward through transformations.

### Structural Guarantees

| Pattern | Data Flow | Cycle Prevention |
|---------|-----------|------------------|
| `{ a \n b \n c }` | Linear sequence | Each step sees only prior bindings |
| `try { a? \n b? \n c? }` | Linear with early exit | Same as blocks |
| `match x { ... }` | Branching | Each branch is independent |
| `recurse(...)` | Iteration | State passed explicitly, no self-reference |
| `parallel(...)` | Fan-out/fan-in | Results collected, no cross-task references |

### Closures Capture by Value

In languages where closures capture by reference, cycles form when a closure captures `self`:

```
Object ──▶ callback field ──▶ closure ──▶ captured self ──▶ Object
```

Ori closures capture by value. The closure receives a copy of captured data, not a reference back to the containing scope:

```ori
let x = 5;
let f = () -> x + 1;  // f contains a copy of 5, not a reference to x
```

This eliminates the most common source of cycles in functional-style code.

### Closure Representation

A closure is represented as a struct containing captured values:

```ori
let x = 10;
let y = "hello";
let f = () -> `{y}: {x}`;

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

### Atomicity

All reference count operations are atomic. This ensures correct deallocation when values are shared across concurrent tasks.

| Operation | Atomic Instruction | Memory Ordering |
|-----------|-------------------|-----------------|
| Increment | Fetch-add | Acquire |
| Decrement | Fetch-sub | Release |
| Deallocation check | Fence before free | Acquire |

The acquire fence before deallocation ensures the deallocating task observes all prior writes to the object from other tasks.

> **Note:** An implementation may use non-atomic operations for values that provably do not escape the current task. This is an optimization; the observable behavior must be identical.

## Destruction

Destruction occurs when values become unreachable, no later than scope end.

### The Drop Trait

The `Drop` trait enables custom destruction logic:

```ori
trait Drop {
    @drop (self) -> void;
}
```

When a value's reference count reaches zero, its `Drop.drop` method is called if implemented. Drop is called before memory is reclaimed.

`Drop` is included in the prelude.

### Destructor Timing

Destructors run when reference count reaches zero:

| Context | Timing |
|---------|--------|
| Local binding out of scope | Immediately at scope end |
| Last reference dropped | Immediately after drop |
| Field of struct dropped | After struct destructor |
| Collection element | When removed or collection dropped |

Values may be dropped before scope end if no longer referenced (compiler optimization).

### Destruction Order

Reverse creation order within a scope:

```ori
{
    let a = create_a();  // Destroyed 3rd
    let b = create_b();  // Destroyed 2nd
    let c = create_c();  // Destroyed 1st
    // destroyed: c, b, a
}
```

Struct fields are destroyed in reverse declaration order:

```ori
type Container = {
    first: Resource,   // Destroyed 3rd
    second: Resource,  // Destroyed 2nd
    third: Resource,   // Destroyed 1st
}
```

List elements are destroyed back-to-front:

```ori
let items = [a, b, c];
// When dropped: c, then b, then a
```

Tuple elements are destroyed right-to-left:

```ori
let tuple = (first, second, third);
// When dropped: third, then second, then first
```

Map entries have no guaranteed destruction order (hash-based).

### Panic During Destruction

If a destructor panics during normal execution (not already unwinding):
1. That panic propagates normally
2. Other values in scope still have their destructors run
3. Each destructor runs in isolation

If a destructor panics while already unwinding from another panic (double panic):
1. The program **aborts** immediately
2. No further destructors run
3. Exit code indicates abnormal termination

### Async Destructors

Destructors cannot be async:

```ori
impl Drop for Resource {
    @drop (self) -> void uses Async = ...;  // ERROR: drop cannot be async
}
```

For async cleanup, use explicit methods:

```ori
impl AsyncResource {
    @close (self) -> void uses Async = ...;  // Explicit async cleanup
}

impl Drop for AsyncResource {
    @drop (self) -> void = ();  // Synchronous no-op
}
```

### Destructors and Task Cancellation

When a task is cancelled, destructors still run during unwinding.

## Reference Counting Optimizations

An implementation may optimize reference counting operations provided the following observable behavior is preserved:

1. Every reference-counted value is deallocated no later than the end of the scope in which it becomes unreachable
2. `Drop.drop` is called exactly once per value, in the order specified by [§ Destruction Order](#destruction-order)
3. No value is accessed after deallocation

The following optimizations are permitted:

| Optimization | Description |
|-------------|-------------|
| Scalar elision | No reference counting operations for scalar types (see [§ Type Classification](#type-classification)) |
| Borrow inference | Omit increment/decrement for parameters that are borrowed and do not outlive the callee |
| Move optimization | Elide the increment/decrement pair when a value is transferred on last use |
| Redundant pair elimination | Remove an increment immediately followed by a decrement on the same value |
| Constructor reuse | Reuse the existing allocation when the reference count is one (requires a runtime uniqueness check) |
| Early drop | Deallocate a value before scope end when it is provably unreferenced for the remainder of the scope |

These are permissions, not requirements. A conforming implementation may perform all, some, or none of these optimizations.

## Ownership and Borrowing

Every reference-counted value has exactly one _owner_. The owner is the binding, field, or container element that holds the value.

### Ownership Transfer

Ownership transfers on:

- Assignment to a new binding
- Passing as a function argument
- Returning from a function
- Storage in a container element or struct field

On transfer, the previous owner relinquishes access. The reference count does not change; ownership moves without an increment/decrement pair.

### Borrowed References

A _borrowed reference_ provides temporary read access to a value without incrementing the reference count. A borrowed reference must not outlive its owner.

The compiler infers ownership and borrowing. There is no user-visible syntax for ownership annotations or borrow markers.

## Cycle Prevention

Cycles prevented at compile time:

1. Values are never mutated in place — reassignment produces new values, preventing in-place cycle formation
2. No shared mutable references — single ownership of mutable data
3. Self-referential types forbidden

```ori
// Valid: indices
type Graph = { nodes: [Node], edges: [(int, int)] }

// Error: self-referential
type Node = { next: Option<Node> }  // compile error
```

## Type Classification

Every type is classified as either _scalar_ or _reference_ for the purpose of reference counting. Classification is determined by type containment, not by representation size.

### Scalar Types

A type is scalar if it requires no reference counting. The following types are scalar:

- Primitive types: `int`, `float`, `bool`, `char`, `byte`, `Duration`, `Size`, `Ordering`
- `unit` and `never`
- Compound types (structs, enums, tuples, `Option<T>`, `Result<T, E>`, `Range<T>`) whose fields are all scalar

### Reference Types

A type is a reference type if it requires reference counting. The following types are reference types:

- Heap-allocated types: `str`, `[T]`, `{K: V}`, `Set<T>`, `Channel<T>`
- Function types and iterator types
- Compound types containing at least one reference type field

### Transitive Rule

Classification is transitive: if any field of a compound type is a reference type, the compound type is a reference type.

| Type | Classification | Reason |
|------|---------------|--------|
| `int` | Scalar | Primitive |
| `(int, float, bool)` | Scalar | All fields scalar |
| `{ x: int, y: int }` | Scalar | All fields scalar |
| `str` | Reference | Heap-allocated |
| `{ id: int, name: str }` | Reference | `name` is reference |
| `Option<str>` | Reference | Inner type is reference |
| `Option<int>` | Scalar | Inner type is scalar |
| `[int]` | Reference | List is heap-allocated |
| `Result<int, str>` | Reference | `str` is reference |

Classification is independent of type size. A struct with ten `int` fields is scalar. A struct with one `str` field is a reference type regardless of its total size.

### Generic Type Parameters

Unresolved type parameters are conservatively treated as reference types. After monomorphization, all type parameters are concrete and classification is exact.

## Constraints

- Self-referential types are compile errors
- Destruction in reverse creation order
- Values destroyed when reference count reaches zero

## ARC Safety Invariants

Ori uses ARC without cycle detection. The following invariants must be maintained by all language features to ensure ARC remains viable.

### Invariant 1: Value Capture

Closures must capture variables by value. Reference captures are prohibited.

```ori
let x = 5;
let f = () -> x + 1;  // captures copy of x, not reference to x
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

### Task Isolation

Values shared across task boundaries are reference-counted. Each task may independently increment and decrement the reference count of a shared value. Atomic reference count operations (see [§ Atomicity](#atomicity)) ensure that deallocation occurs exactly once, regardless of which task drops the last reference.

A task must not hold a borrowed reference to a value owned by another task. All cross-task value sharing uses ownership transfer or reference count increment.

See [Concurrency Model § Task Isolation](23-concurrency-model.md#task-isolation) for task isolation rules.

### Handler Frame State

Stateful handlers (see [Capabilities § Stateful Handlers](14-capabilities.md#stateful-handlers)) maintain frame-local mutable state within a `with...in` scope. This state is analogous to mutable loop variables: it is local to the handler frame, not aliased, and not accessible outside the `with...in` scope. Handler frame state does not violate Invariant 3 (no shared mutable references) because the state has a single owner (the handler frame) and is never shared.

### Feature Evaluation

New language features must be evaluated against these invariants. A feature that violates any invariant must either:
1. Be redesigned to maintain the invariant
2. Provide equivalent cycle prevention guarantees
3. Be rejected

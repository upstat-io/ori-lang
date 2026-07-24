---
title: "Memory Model"
description: "Clause 21: Ori Language Specification — Memory Model"
order: 21
section: "Language"
---

# 21 Memory model

Ori specifies automatic, exactly-once ownership cleanup without tracing cycle detection. Language design choices structurally prevent reference cycles, and every admitted executor shall preserve the same value, lifetime, drop, and error behavior.

NOTE  The reference compiler's current compiled projection uses Automatic
Reference Counting (ARC). The backend-neutral compile-time ownership calculus
is documented in Annex E §AIMS. The historical “ARC Intelligent Memory System”
expansion is non-normative and does not restrict AIMS to that projection.

AIMS spans a product lattice over Access, Consumption, Cardinality, Uniqueness, Locality, Shape, and Effect dimensions, with interprocedural contracts and a layered verification stack; it does not mandate a counter-based physical realization.

## 21.1 Why cycle-free automatic ownership works

Most languages using ARC require either cycle detection (Python, PHP) or manual weak reference annotations (Swift, Objective-C). Ori requires neither because its execution model produces directed acyclic graphs (DAGs) by construction.

### 21.1.1 The problem: object graphs

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

### 21.1.2 The solution: sequential data flow

Ori's block expressions enforce **linear data flow**:

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

### 21.1.3 Structural guarantees

| Pattern | Data Flow | Cycle Prevention |
|---------|-----------|------------------|
| `{ a; b; c }` | Linear sequence | Each step sees only prior bindings |
| `try { a?; b?; c? }` | Linear with early exit | Same as blocks |
| `match x { ... }` | Branching | Each branch is independent |
| `for x in xs do ...` | Iteration | Loop variable rebinding, no self-reference |
| `loop { ... }` | Iteration | State via mutable bindings, no self-reference |
| `parallel(...)` | Fan-out/fan-in | Results collected, no cross-task references |

### 21.1.4 Closures capture by value

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

### 21.1.5 Closure capture semantics

A closure semantically carries a capture aggregate containing its captured values. The selected physical plan may encode that aggregate as inline fields, an environment object or handle, a struct, or another validated representation. The following sketch shows logical capture fields, not an ABI or machine layout:

```ori
let x = 10;
let y = "hello";
let f = () -> `{y}: {x}`;

// f logically carries:
// type _Closure_f = { captured_x: int, captured_y: str }
```

For ownership-bearing types (lists, maps, custom types), the closure stores a value sharing the same logical storage identity and receives its own owner credit rather than deep-copying the data. A counter-based projection may realize that credit with an increment; the counter is not language semantics.

### 21.1.6 Self-referential types forbidden

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

### 21.1.7 Summary

Cycle-free automatic ownership works in Ori because:

1. **Blocks enforce DAGs** — Data flows forward through `{ }` blocks, `try`, `match`
2. **Value capture prevents closure cycles** — No reference back to enclosing scope
3. **Type restrictions prevent structural cycles** — Self-referential types forbidden
4. **No shared mutable references** — Single ownership of mutable data

These are not conventions — they are language invariants enforced by the compiler.

## 21.2 Logical ownership credits

| Operation | Logical effect |
|-----------|----------------|
| Owned value creation | Establish one owner credit |
| Owned duplication | Establish one additional owner credit |
| Ownership transfer | Move one credit without creating or discharging it |
| Owner death | Discharge one credit |
| Final owner death | Run the exact drop plan and reclaim the selected physical storage |

Owned values may be duplicated or transferred by assignment, argument passing, field storage, return, and closure capture according to the compiler's inferred ownership contract.

Owner credits are discharged when their bindings die or are replaced and have not transferred the credit elsewhere.

### 21.2.1 Thread-safe physical realization

An executor shall use a physical ownership mechanism safe for the value's proved thread reachability. A potentially shared value requires a shared-safe capability; a proven thread-confined value may use a confined specialization.

Permitted mechanisms include atomic or non-atomic counters, ownership transfer, regions, static storage, locks, tags, side tables, or another validated realization. The mechanism shall preserve every logical owner-credit event, exactly-once cleanup, visibility, and observable behavior.

NOTE  The current compiled ARC runtime uses atomic fetch-add/fetch-sub operations and a fence before final reclamation for potentially shared objects. That is an informative projection detail documented in Annex E §ARC Runtime, not a language requirement or an AIMS fact.

The observable behavior shall be identical for every admitted mechanism.

## 21.3 Destruction

Destruction occurs when values become unreachable, no later than scope end.

### 21.3.1 The Drop trait

The `Drop` trait enables custom destruction logic:

```ori
trait Drop {
    @drop (self) -> void;
}
```

When a value reaches its exactly-once logical death point, its `Drop.drop` method is called if implemented. Drop runs before the value's selected storage is reclaimed.

A physical projection may discover the final-owner event with a reference count, but that counter is not part of the language semantics and counter-free values still obey the same rule.

`Drop` is included in the prelude.

### 21.3.2 Destructor timing

Destructors run at the final logical owner/cleanup event:

| Context | Timing |
|---------|--------|
| Local binding out of scope | Immediately at scope end |
| Final logical owner relinquished | Immediately after the ownership release |
| Field of struct dropped | After struct destructor |
| Collection element | When removed or collection dropped |

Values may be dropped before scope end if no longer referenced (compiler optimization).

### 21.3.3 Destruction order

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

### 21.3.4 Panic during destruction

If a destructor panics during normal execution (not already unwinding):
1. That panic propagates normally
2. Other values in scope still have their destructors run
3. Each destructor runs in isolation

If a destructor panics while already unwinding from another panic (double panic):
1. The program **aborts** immediately
2. No further destructors run
3. Exit code indicates abnormal termination

### 21.3.5 Async destructors

Destructors cannot be async:

```ori
impl Resource: Drop {
    @drop (self) -> void uses Suspend = ...;  // ERROR: drop cannot be async
}
```

For async cleanup, use explicit methods:

```ori
impl AsyncResource {
    @close (self) -> void uses Suspend = ...;  // Explicit async cleanup
}

impl AsyncResource: Drop {
    @drop (self) -> void = ();  // Synchronous no-op
}
```

### 21.3.6 Destructors and task cancellation

When a task is cancelled, destructors still run during unwinding.

## 21.4 Ownership and storage optimizations

An implementation may optimize logical ownership events and physical storage operations provided the following observable behavior is preserved:

1. Every ownership-bearing value completes cleanup no later than the end of the scope in which it becomes unreachable
2. `Drop.drop` is called exactly once per value, in the order specified by [§ Destruction Order](#2133-destruction-order)
3. No value is accessed after deallocation

### 21.4.1 Permitted optimizations

The following optimizations are permitted:

| Optimization | Description |
|-------------|-------------|
| Scalar elision | No ownership bookkeeping for scalar types without cleanup obligations (see [§ Type Classification](#217-type-classification)) |
| Borrow inference | Omit owner-credit creation/discharge for parameters that are borrowed and do not outlive the callee |
| Move optimization | Transfer the existing owner credit on last use without a create/discharge pair |
| Redundant pair elimination | Remove an adjacent logical credit/debit pair on the same value when no observation intervenes |
| Constructor reuse | Reuse selected physical storage when there is one logical owner and the physical plan satisfies every cleanup and extent obligation |
| Copy-on-write | Mutating operations on collections and strings may mutate in place only when logical sharing permits it and any required physical sharing observation succeeds; otherwise they copy before mutation |
| Seamless slicing | Slice operations (`take`, `skip`, `slice`, `substring`, `trim`) may return a zero-copy view sharing the original logical storage identity and carrying the required ownership credit |
| Small value inlining | Small values (e.g., short strings ≤23 bytes) may be stored inline without heap allocation. The threshold is implementation-defined. |
| Early drop | Deallocate a value before scope end when it is provably unreferenced for the remainder of the scope |
| Tail-modulo-cons (TRMC) | Tail-recursive functions that construct values can rewrite allocation patterns to build results in-place, avoiding intermediate allocations. |
| Functional in-place (FIP) | Functions that consume a uniquely-owned argument and produce a structurally similar result may be certified as functional-in-place, enabling allocation-free execution. |

These are permissions, not requirements. A conforming implementation may perform all, some, or none of these optimizations.

NOTE  Copy-on-write preserves value semantics: `let b = a; b = b.push(value: x)` shall not modify `a`, regardless of whether the implementation copies or mutates in place. The optimization is transparent to user code.

### 21.4.2 AIMS — backend-neutral ownership calculus

The reference compiler uses AIMS, one backend-neutral logical ownership calculus over a multi-dimensional product lattice. It freezes ownership, consumption, usage, uniqueness, lifetime, reuse eligibility, cleanup, unwind, and effect facts once.

VM, LLVM, native, compiled-WebAssembly, and JIT paths consume the same frozen facts and independently validate their physical mechanisms; no backend may rerun AIMS or reconstruct its policy.

NOTE  AIMS is an implementation strategy, not a language requirement. A conforming implementation may use any optimization approach that preserves the observable behavior specified above.

## 21.5 Ownership and borrowing

Every live owned handle carries one logical _owner credit_. A binding, field, or container element may carry such a credit; an explicit logical duplication creates another owner, while a borrow creates none.

### 21.5.1 Ownership transfer

Ownership transfers on:

- Assignment to a new binding
- Passing as a function argument
- Returning from a function
- Storage in a container element or struct field

On transfer, the previous owner relinquishes access and the same logical credit moves to the recipient. No credit is created or discharged.

### 21.5.2 Borrowed references

A _borrowed reference_ provides temporary read access without creating an owner credit. A borrowed reference shall not outlive its governing owner.

The compiler infers ownership and borrowing. There is no user-visible syntax for ownership annotations or borrow markers.

## 21.6 Cycle prevention

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

## 21.7 Type classification

Every type is classified as either _scalar_ or _ownership-bearing_ for logical cleanup. Classification is determined by type containment and declared behavior, not by representation size or physical placement.

### 21.7.1 Scalar types

A type is scalar if it carries no owner-credit or cleanup obligation. The following types are scalar:

- Primitive types: `int`, `float`, `bool`, `char`, `byte`, `Duration`, `Size`, `Ordering`
- `unit` and `never`
- Compound types (structs, enums, tuples, `Option<T>`, `Result<T, E>`, `Range<T>`) whose fields are all scalar

### 21.7.2 Ownership-bearing types

A type is ownership-bearing if it carries shared identity, owner-credit, or cleanup obligations. The following types are ownership-bearing:

- Identity-bearing built-ins: `str`, `[T]`, `{K: V}`, `Set<T>`, `Channel<T>`
- Function and iterator types with captured or service-owned state
- Compound types containing at least one ownership-bearing field

### 21.7.3 Transitive rule

Classification is transitive: if any field of a compound type is ownership-bearing, the compound type is ownership-bearing.

| Type | Classification | Reason |
|------|---------------|--------|
| `int` | Scalar | Primitive |
| `(int, float, bool)` | Scalar | All fields scalar |
| `{ x: int, y: int }` | Scalar | All fields scalar |
| `str` | Ownership-bearing | Carries string storage identity |
| `{ id: int, name: str }` | Ownership-bearing | `name` carries an ownership obligation |
| `Option<str>` | Ownership-bearing | Inner type is ownership-bearing |
| `Option<int>` | Scalar | Inner type is scalar |
| `[int]` | Ownership-bearing | Carries list storage identity |
| `Result<int, str>` | Ownership-bearing | `str` carries an ownership obligation |

Classification is independent of type size and storage. A struct with ten `int` fields is scalar.

A struct with one `str` field is ownership-bearing regardless of its total size or a physical plan's inline, stack, region, or heap choice.

### 21.7.4 Value types

A type that implements the `Value` trait has bitwise-copy value semantics with no owner-credit or `Drop` obligation. All fields of a `Value` type shall themselves be `Value`.

The `Value` trait implies `Clone` and `Sendable`; physical placement remains target-owned.

Primitive scalar types (`int`, `float`, `bool`, `char`, `byte`, `void`, `Duration`, `Size`, `Ordering`) implicitly satisfy `Value`. User-defined types opt in via the type declaration:

```ori
type Point: Value, Eq = { x: float, y: float }
```

A `Value` type shall not exceed 512 bytes. Types exceeding 256 bytes produce a warning.

### 21.7.5 Generic type parameters

Unresolved type parameters are conservatively treated as ownership-bearing. After monomorphization, all type parameters are concrete and classification is exact.

## 21.8 Constraints

- Self-referential types are compile errors
- Destruction in reverse creation order
- Values destroyed exactly once at their logical death point

## 21.9 Automatic ownership safety invariants

The following invariants shall be maintained by all language features so automatic ownership cleanup remains cycle-free and exactly once under every admitted physical realization.

### 21.9.1 Invariant 1: value capture

Closures shall capture variables by value. Reference captures are prohibited.

```ori
let x = 5;
let f = () -> x + 1;  // captures copy of x, not reference to x
```

This prevents cycles through closure environments.

### 21.9.2 Invariant 2: no implicit back-references

Structures shall not implicitly reference their containers. Bidirectional relationships require explicit weak references or indices.

```ori
// Valid: indices for back-navigation
type Tree = { nodes: [Node], parent_indices: [Option<int>] }

// Invalid: implicit parent reference would create cycle
type Node = { children: [Node], parent: Node }  // error
```

### 21.9.3 Invariant 3: no shared mutable references

Multiple mutable references to the same value are prohibited. Shared access requires either:
- Copy-on-write semantics
- Explicit synchronization primitives with single ownership

### 21.9.4 Invariant 4: value semantics default

Types have value semantics unless explicitly boxed. Reference types require explicit opt-in through container types or `Box<T>`.

### 21.9.5 Invariant 5: explicit weak references

If weak references are added to the language, they shall:
- Use distinct syntax (`Weak<T>`)
- Require explicit upgrade operations returning `Option<T>`
- Never be implicitly created

### 21.9.6 Task isolation

Values crossing task boundaries use ownership transfer or explicit logical sharing. Each receiving task carries the exact owner credit required by the shared plan, and the selected physical mechanism shall provide a shared-safe capability and exactly-once final cleanup regardless of which task discharges the last credit.

A task shall not hold a borrowed reference to a value owned by another task. All cross-task value sharing uses ownership transfer or an explicit shared owner credit.

See [Concurrency Model § Task Isolation](22-concurrency-model.md#2213-task-isolation) for task isolation rules.

### 21.9.7 Handler frame state

Stateful handlers (see [Capabilities § Stateful Handlers](20-capabilities.md#205-stateful-handlers)) maintain frame-local mutable state within a `with...in` scope. This state is analogous to mutable loop variables: it is local to the handler frame, not aliased, and not accessible outside the `with...in` scope. Handler frame state does not violate Invariant 3 (no shared mutable references) because the state has a single owner (the handler frame) and is never shared.

### 21.9.8 Feature evaluation

New language features shall be evaluated against these invariants. A feature that violates any invariant shall either:
1. Be redesigned to maintain the invariant
2. Provide equivalent cycle prevention guarantees
3. Be rejected

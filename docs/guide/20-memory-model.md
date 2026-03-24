---
title: "Memory Model"
description: "AIMS memory management — value semantics, reference counting, copy-on-write, and cycle prevention."
order: 20
part: "Advanced Patterns"
---

# Memory Model

Ori uses **AIMS** (ARC Intelligent Memory System) for memory management — no garbage collector pauses, no manual memory management, and no borrow checker. You write code with value semantics (every assignment is a logical copy), and AIMS makes it fast by automatically managing reference counts, reusing allocations, and applying copy-on-write.

## How It Works

Under the hood, AIMS uses reference counting. Every heap-allocated value has a reference count. When you assign a value, the count increases. When a reference goes out of scope, the count decreases. When it hits zero, the memory is freed.

```ori
let a = [1, 2, 3];      // ref count = 1
let b = a;              // ref count = 2 (a and b share the data)
// b goes out of scope  // ref count = 1
// a goes out of scope  // ref count = 0, memory freed
```

### Deterministic Cleanup

Unlike garbage collection, ARC frees memory immediately when the last reference is gone:

```ori
@process_file (path: str) -> void uses FileSystem = {
    let data = FileSystem.read(path: path);  // Memory allocated
    let result = process(data: data);
    print(msg: result)
}  // data freed exactly here, not "sometime later"
```

This predictability is valuable for resource-constrained environments and real-time applications.

## Why Not a Garbage Collector?

| Feature | GC | AIMS |
|---------|----|----|
| Pause times | Unpredictable | None |
| Memory overhead | Higher | Lower |
| Cleanup timing | Eventually | Immediate |
| Performance | Variable | Consistent |
| In-place mutation | Requires barriers | COW (automatic) |

Ori chose AIMS for:
- Predictable performance — no stop-the-world pauses
- Lower memory overhead — no GC headroom needed
- Immediate cleanup — resources freed when last reference drops
- Copy-on-write — value semantics at near-mutating performance

## Preventing Reference Cycles

Reference counting can't handle reference cycles. If A references B and B references A, neither can ever be freed. Ori's design prevents cycles structurally — no cycle detector needed:

### 1. Sequential Data Flow

Data flows forward through blocks:

```ori
{
    let a = create_a();
    let b = create_b(input: a);   // b can reference a
    let c = create_c(input: b);   // c can reference b
    // No way for a to reference c (c doesn't exist when a is created)
}
```

### 2. Capture by Value

Closures capture variables by value, not reference:

```ori
let x = 10;
let f = () -> x + 1;  // f captures a COPY of x = 10

// Even if we reassign x, f still has 10
f();  // Always returns 11
```

This means closures can't create cycles by capturing "self" references.

### 3. No Self-Referential Types

You can't create types that reference themselves through the same instance:

```ori
// This pattern is NOT possible in Ori:
type Node = {
    value: int,
    parent: Option<Node>,  // Can't point back to containing instance
}
```

Instead, use:
- Indices into collections
- Separate parent/child structures
- Tree patterns where children don't reference parents

## Value Types vs Reference Types

Ori classifies every type as either _scalar_ (no reference counting) or _reference_ (reference counted).

### Scalar Types

Copied directly — no reference counting overhead:

```ori
let x = 42;
let y = x;  // y is an independent copy
```

Scalar types include:
- `int`, `float`, `bool`, `char`, `byte`
- `Duration`, `Size`, `Ordering`
- `void`, `Never`
- Structs/enums/tuples where ALL fields are scalar

### Reference Types

Shared with reference counting:

```ori
let a = [1, 2, 3];
let b = a;  // b and a share the same underlying data
```

Reference types include:
- `str` (heap strings; short strings ≤23 bytes use Small String Optimization)
- `[T]` (lists), `{K: V}` (maps), `Set<T>`
- Function types, iterator types
- Any struct/enum containing at least one reference field

### The Value Trait

Types that implement `Value` are stored inline with bitwise copy — no heap allocation, no reference counting, no `Drop`. All fields must also be `Value`:

```ori
type Point: Value, Eq = { x: float, y: float }

let a = Point { x: 1.0, y: 2.0 };
let b = a;  // Bitwise copy, zero overhead
```

All primitives implicitly satisfy `Value`. User types opt in via the type declaration. Maximum size: 512 bytes (warning above 256).

### How to Know Which Is Which

| Type | Classification | Reason |
|------|---------------|--------|
| `int`, `float`, `bool` | Scalar | Primitive |
| `(int, float)` | Scalar | All fields scalar |
| `{ x: int, y: int }` | Scalar | All fields scalar |
| `str` | Reference | Heap-allocated (or SSO) |
| `{ id: int, name: str }` | Reference | `name` is reference |
| `Option<str>` | Reference | Inner type is reference |
| `Option<int>` | Scalar | Inner type is scalar |
| `[int]` | Reference | List is heap-allocated |

## Copy-on-Write (COW)

Ori has value semantics — every assignment is a logical copy. But the compiler optimizes this via copy-on-write: when you "modify" a shared collection, the runtime checks if the reference count is 1. If unique, it mutates in place. If shared, it copies first.

```ori
let a = [1, 2, 3];
let b = a;           // a and b share data (ref count = 2)

// This triggers a copy because a is shared
a = a.push(value: 4);  // a gets its own copy: [1, 2, 3, 4]
                        // b still has [1, 2, 3]
```

COW is transparent — you write code as if every value is independent, and the compiler avoids copies when it can prove safety.

### Seamless Slices

Slice operations (`take`, `skip`, `slice`, `substring`, `trim`) return zero-copy views into the original allocation:

```ori
let text = "hello world";
let word = text.substring(start: 0, end: 5);  // "hello" — no copy
```

The slice shares the parent's reference count. When the parent is dropped and only the slice remains, subsequent mutations materialize the slice into an independent allocation.

### Small String Optimization (SSO)

Strings of 23 bytes or fewer are stored inline — no heap allocation, no reference counting. This covers most identifiers, short messages, and single-line strings.

```ori
let short = "hello";  // Stored inline (5 bytes ≤ 23)
let long = "this is a string that definitely exceeds twenty-three bytes";  // Heap allocated
```

## The Clone Trait

To get an explicit independent copy of a reference type, use `.clone()`:

```ori
let a = [1, 2, 3];
let b = a.clone();  // b has its own copy of the data
```

Clone is recursive — cloning a container clones its elements:

```ori
let lists = [[1, 2], [3, 4]];
let copy = lists.clone();  // Both outer and inner lists are cloned
```

### What Implements Clone

- All primitives
- All collections (when element types implement Clone)
- `Option<T>` and `Result<T, E>` (when inner types implement Clone)
- Derivable for user types:

```ori
type Point: Clone, Eq = { x: int, y: int }

let p1 = Point { x: 10, y: 20 };
let p2 = p1.clone();  // Independent copy
```

## The Drop Trait

Custom cleanup logic when a value's reference count reaches zero:

```ori
trait Drop {
    @drop (self) -> void
}
```

Drop is called before memory is reclaimed. Destructors cannot be async — for async cleanup, use explicit methods.

### Destruction Order

Values are destroyed in reverse creation order within a scope:

```ori
{
    let a = create_a();  // Destroyed 3rd
    let b = create_b();  // Destroyed 2nd
    let c = create_c();  // Destroyed 1st
}
```

Struct fields: reverse declaration order. List elements: back-to-front. Map entries: no guaranteed order.

### Early Drop

The compiler may drop a value before scope end when it is provably unreferenced:

```ori
@example () -> int = {
    let big_data = load_data();
    let result = process(data: big_data);
    // big_data may be freed here (no longer used)
    expensive_computation(input: result)
}
```

You can also request early drop explicitly:

```ori
let data = load_data();
let result = process(data: data);
drop_early(value: data);  // Free now, don't wait for scope end
expensive_computation(input: result)
```

## Closures and Capture

Closures capture variables by value at creation time:

```ori
@make_adder (n: int) -> (int) -> int = {
    let add_n = x -> x + n;  // Captures n by value
    add_n
}

let add_5 = make_adder(n: 5);
let add_10 = make_adder(n: 10);

add_5(3);   // 8
add_10(3);  // 13
```

### Snapshot Semantics

The closure sees a snapshot of values at creation:

```ori
let x = 10;
let f = () -> x;  // Captures x = 10

let x = 20;  // Shadowing, creates new binding
f();  // Still returns 10 (captured value)
```

For reference types (lists, maps, strings), the closure stores the reference (incrementing the reference count), not a deep copy. This is efficient — you share the data, and COW ensures independence if either side mutates.

## AIMS Safety Invariants

The Ori language maintains these invariants to ensure memory safety without a garbage collector:

1. **No shared mutable references** — Only one reference can mutate data at a time. COW enforces this at runtime.
2. **Closures capture by value** — No closure can hold a mutable reference to outer scope.
3. **No self-referential structures** — Types cannot contain references to their own instances.
4. **Immutable module-level bindings** — Module-level bindings must use `$` (immutable).
5. **Value semantics by default** — Assignment is a logical copy; the compiler optimizes via COW.

## Quick Reference

### AIMS Reference Counting

```ori
let a = value;         // ref count = 1
let b = a;             // ref count = 2
// b drops            // ref count = 1
// a drops            // ref count = 0, freed
```

### Clone

```ori
let copy = original.clone();  // Independent copy
```

### Value vs Reference

| Scalar (no RC) | Reference (RC) |
|----------------|----------------|
| `int`, `float`, `bool` | `str`, `[T]`, `{K: V}` |
| `char`, `byte`, `Duration` | `Set<T>`, function types |
| All-scalar structs/tuples | Structs with any ref field |
| `Value` trait types | Iterator types |

### Copy-on-Write

```ori
let a = [1, 2, 3];
let b = a;                    // Shared (cheap)
a = a.push(value: 4);        // COW: copies because shared
```

### Closure Capture

```ori
let x = 10;
let f = () -> x;  // Captures x by value (snapshot)
```

### AIMS Safety Invariants

1. No shared mutable references (COW enforces)
2. Closures capture by value
3. No self-referential structures
4. Immutable module-level bindings
5. Value semantics by default

## What's Next

Now that you understand the memory model:

- **[Formatting Rules](/guide/21-formatting)** — Code style guidelines

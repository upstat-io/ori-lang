---
title: "Memory Model"
description: "AIMS memory management — value semantics, ownership, deterministic cleanup, copy-on-write, and cycle prevention."
order: 20
part: "Advanced Patterns"
---

# Memory Model

Ori uses **AIMS**, one backend-neutral ownership calculus, for memory management
— no required tracing-collector pauses, manual memory management, or
source-level borrow checker. You write code with value semantics (every
assignment is a logical copy), and AIMS proves the ownership, lifetime, cleanup,
sharing, copy-on-write, and reuse obligations once. The historical “ARC
Intelligent Memory System” expansion names the first compiled projection; it
does not require LLVM or a reference counter.

AIMS is backend-neutral. It does not require LLVM, choose heap versus stack storage, prescribe an object header, or mandate a particular reference-counting instruction.

The VM and compiled targets choose their own physical layouts and mechanisms, then validate them against the same frozen AIMS facts. The tree-walking evaluator remains a representation-independent behavior oracle and does not implement a second memory calculus.

## How It Works

Conceptually, AIMS tracks logical ownership obligations. A conservative physical plan can realize those obligations with reference counting: sharing a managed value retains it, ending an ownership obligation releases it, and the last release permits cleanup.

When AIMS proves sharing impossible or a lifetime tightly bounded, a physical plan may omit the counter or use inline, stack, arena, region, or another validated representation.

```ori
let a = [1, 2, 3];      // one logical owner
let b = a;              // two logical values may share physical storage
// b's ownership ends   // one logical owner remains
// a's ownership ends   // cleanup obligation becomes due
```

### Deterministic Cleanup

Unlike tracing garbage collection, an admitted Ori executor performs cleanup at the lifetime end proved by the shared plan. A reference-counted projection commonly reclaims storage on its last release; another projection may use an equally deterministic mechanism:

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
| Memory overhead | Requires tracing headroom | Selected and proven per target |
| Cleanup timing | Eventually | At the proved lifetime end |
| Performance | Variable | Consistent |
| In-place mutation | Requires barriers | COW (automatic) |

Ori chose AIMS for:
- Predictable performance — no stop-the-world pauses
- Lower memory overhead — no tracing-GC headroom needed
- Deterministic cleanup — resources are released at their proved lifetime end
- Copy-on-write — value semantics at near-mutating performance

## Preventing Reference Cycles

Reference counting cannot reclaim an isolated reference cycle: if A references B and B references A, neither count reaches zero. Ori prevents cycles structurally, so no cycle detector is needed:

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

## Value Types vs Managed Types

Ori records whether a value is scalar or carries logical ownership/drop obligations. That classification is semantic; a target's physical planner decides whether satisfying it needs a counter, inline bits, an arena handle, or another representation.

### Scalar Types

Copied directly — no managed-ownership overhead:

```ori
let x = 42;
let y = x;  // y is an independent copy
```

Scalar types include:
- `int`, `float`, `bool`, `char`, `byte`
- `Duration`, `Size`, `Ordering`
- `void`, `Never`
- Structs/enums/tuples where ALL fields are scalar

### Managed Types

May share physical storage under the selected plan:

```ori
let a = [1, 2, 3];
let b = a;  // b and a share the same underlying data
```

Managed types include:
- `str` (a physical plan may inline short strings)
- `[T]` (lists), `{K: V}` (maps), `Set<T>`
- Function types, iterator types
- Any struct/enum containing at least one managed field

### The Value Trait

Types that implement `Value` admit a direct value representation with no logical drop burden. A physical plan may store them inline and copy their bits.

All fields must also be `Value`:

```ori
type Point: Value, Eq = { x: float, y: float }

let a = Point { x: 1.0, y: 2.0 };
let b = a;  // Bitwise copy, zero overhead
```

All primitives implicitly satisfy `Value`, while user types opt in through the type declaration. The maximum size is 512 bytes, with a warning above 256 bytes.

### How to Know Which Is Which

| Type | Classification | Reason |
|------|---------------|--------|
| `int`, `float`, `bool` | Scalar | Primitive |
| `(int, float)` | Scalar | All fields scalar |
| `{ x: int, y: int }` | Scalar | All fields scalar |
| `str` | Managed | Carries string storage semantics |
| `{ id: int, name: str }` | Managed | `name` carries a burden |
| `Option<str>` | Managed | Inner type carries a burden |
| `Option<int>` | Scalar | Inner type is scalar |
| `[int]` | Managed | Carries list storage semantics |

## Copy-on-Write (COW)

Ori has value semantics — every assignment is a logical copy. The compiler optimizes this through copy-on-write.

AIMS may prove a collection unique statically; otherwise the selected physical plan uses its sharing mechanism to choose safe in-place reuse or a copy. Reference count `1` is one possible compiled probe, not an AIMS rule.

```ori
let a = [1, 2, 3];
let b = a;           // a and b may share physical data

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

The slice shares the parent's logical storage identity. Its physical plan keeps that storage alive.

A later mutation materializes independent storage when required to preserve value semantics.

### Small String Optimization (SSO)

Physical plans may store short strings inline, avoiding separate storage and count metadata. The threshold is target- and plan-specific rather than part of Ori's language semantics.

```ori
let short = "hello";  // Eligible for an inline representation
let long = "this is a string long enough to require managed storage on many targets";
```

## The Clone Trait

To request an explicit independent copy of a managed type, use `.clone()`:

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

Custom cleanup logic when the value's logical cleanup obligation becomes due:

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

Destruction order for nested values is:

- struct fields in reverse declaration order;
- list elements from back to front;
- map entries in no guaranteed order.

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

For managed values such as lists, maps, and strings, closure capture is still a logical value copy rather than an eager deep copy. AIMS freezes the capture's ownership and cleanup obligations; each executor realizes them using its selected closure and storage plan.

COW preserves independence if either side later changes.

## AIMS Safety Invariants

The Ori language maintains these invariants to ensure memory safety without a garbage collector:

1. **No shared mutable references** — Only one reference can mutate data at a time. COW enforces this at runtime.
2. **Closures capture by value** — No closure can hold a mutable reference to outer scope.
3. **No self-referential structures** — Types cannot contain references to their own instances.
4. **Immutable module-level bindings** — Module-level bindings must use `$` (immutable).
5. **Value semantics by default** — Assignment is a logical copy; the compiler optimizes via COW.

## Quick Reference

### AIMS Logical Ownership

```ori
let a = value;         // one logical owner
let b = a;             // another logical value
// b drops             // its ownership obligation ends
// a drops             // final cleanup obligation becomes due
```

### Clone

```ori
let copy = original.clone();  // Independent copy
```

### Value vs Managed

| Scalar/value | Managed burden |
|--------------|----------------|
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

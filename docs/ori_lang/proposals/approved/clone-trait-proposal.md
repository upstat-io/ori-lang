# Proposal: Clone Trait

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-01-26
**Approved:** 2026-01-28

---

## Summary

Define the `Clone` trait that enables explicit value duplication. This trait is already referenced in the prelude and derivable traits list but lacks a formal definition.

```ori
trait Clone {
    @clone (self) -> Self
}
```

---

## Motivation

### The Problem

The spec currently:
- Lists `Clone` as a prelude trait (`12-modules.md`, line 119)
- Lists `Clone` as derivable (`06-types.md`, line 152)
- Uses `Clone` in type constraints (`07-properties-of-types.md`, line 63)

But nowhere defines what `Clone` actually provides. This is a gap that blocks:
- Channel sharing (parallel-concurrency-proposal uses `.clone()`)
- Explicit copying of reference types
- Generic functions that need to duplicate values

### Why Explicit Clone?

Ori distinguishes between:
- **Value types** — copied implicitly (primitives, small structs)
- **Reference types** — shared via ARC, explicit clone to duplicate

This matches Rust's model and prevents accidental expensive copies:

```ori
let a = large_data()
let b = a          // b shares reference to same data (cheap)
let c = a.clone()  // c is independent copy (explicit, potentially expensive)
```

---

## Design

### Trait Definition

```ori
trait Clone {
    @clone (self) -> Self
}
```

Single method, returns owned copy of `self`.

### Prelude Implementations

All primitive and built-in types implement `Clone`:

```ori
impl Clone for int      { @clone (self) -> int = self }
impl Clone for float    { @clone (self) -> float = self }
impl Clone for bool     { @clone (self) -> bool = self }
impl Clone for str      { @clone (self) -> str = self }
impl Clone for char     { @clone (self) -> char = self }
impl Clone for byte     { @clone (self) -> byte = self }
impl Clone for Duration { @clone (self) -> Duration = self }
impl Clone for Size     { @clone (self) -> Size = self }

impl<T: Clone> Clone for [T] {
    @clone (self) -> [T] = self.map(transform: x -> x.clone())
}

impl<K: Clone, V: Clone> Clone for {K: V} {
    @clone (self) -> {K: V} = ...
}

impl<T: Clone> Clone for Set<T> {
    @clone (self) -> Set<T> = ...
}

impl<T: Clone> Clone for Option<T> {
    @clone (self) -> Option<T> = match(
        self,
        Some(v) -> Some(v.clone()),
        None -> None,
    )
}

impl<T: Clone, E: Clone> Clone for Result<T, E> {
    @clone (self) -> Result<T, E> = match(
        self,
        Ok(v) -> Ok(v.clone()),
        Err(e) -> Err(e.clone()),
    )
}

impl<A: Clone, B: Clone> Clone for (A, B) {
    @clone (self) -> (A, B) = (self.0.clone(), self.1.clone())
}
// ... extends to all tuple arities
```

### Derivable

For user-defined types, `Clone` can be derived if all fields implement `Clone`:

```ori
#[derive(Clone)]
type Point = { x: int, y: int }

#[derive(Clone)]
type Tree<T: Clone> = Leaf(value: T) | Branch(left: Tree<T>, right: Tree<T>)
```

Derived implementation clones each field:

```ori
// Generated for Point:
impl Clone for Point {
    @clone (self) -> Point = Point { x: self.x.clone(), y: self.y.clone() }
}
```

### Manual Implementation

Types can implement `Clone` manually for custom behavior:

```ori
type CachedData = {
    data: [byte],
    cache: {str: Result},  // Don't clone cache
}

impl Clone for CachedData {
    @clone (self) -> CachedData = CachedData {
        data: self.data.clone(),
        cache: {},  // Start with empty cache
    }
}
```

### Non-Cloneable Types

Some types should not implement `Clone`:

- **Unique resources** — file handles, network connections
- **Channel endpoints** — unless sharing mode permits (see below)
- **Types with identity** — where duplicates would be semantically wrong

```ori
type FileHandle = { fd: int }
// No Clone implementation — cannot duplicate file handle
```

---

## Channel Integration (Future)

This section describes how Clone will interact with the channel sharing modes proposed in the parallel-concurrency-proposal. Implementation details are defined in that proposal.

The parallel-concurrency-proposal defines sharing modes for channels:

```ori
type Sharing = Exclusive | Producers | Consumers | Both
```

Channel endpoints will implement `Clone` conditionally based on their sharing mode:
- `Exclusive` — neither Producer nor Consumer implements Clone
- `Producers` — Producer implements Clone
- `Consumers` — Consumer implements Clone
- `Both` — both implement Clone

The conditional implementation mechanism is specified in the parallel-concurrency-proposal.

**Example usage (when both proposals are implemented):**

```ori
// Exclusive channel — clone fails at compile time
let { producer, consumer } = channel<int>(.buffer: 10)
let p2 = producer.clone()  // ERROR: Producer<int> does not implement Clone

// Shared producers — clone works
let { producer, consumer } = channel<int>(.buffer: 10, .share: Producers)
let p2 = producer.clone()  // OK
```

---

## Interaction with ARC

`Clone` is ARC-safe because:

1. **Creates independent copy** — no shared mutable state between original and clone
2. **No cycles** — clone doesn't create references back to original
3. **Explicit** — developer consciously chooses to duplicate

```ori
let a = create_data()  // refcount: 1
let b = a              // refcount: 2 (shared reference)
let c = a.clone()      // a's refcount still 2, c has refcount 1 (independent)
```

The clone operation:
- For value types: returns a copy of the value
- For reference types: allocates new memory with refcount 1
- Element-wise recursive: cloning a container clones each element via `.clone()`

After cloning:
- Original and clone have independent reference counts
- Modifying the clone does not affect the original
- Cloned elements are themselves clones (no shared mutable references)

---

## Examples

### Defensive Copy

```ori
@process (data: Data) -> Result<Output, Error> = run(
    let working_copy = data.clone(),  // Don't modify original
    mutate_in_place(data: working_copy),
    Ok(transform(data: working_copy)),
)
```

### Cloning in Parallel Work

```ori
@parallel_process (config: Config, items: [Item]) -> [Result] uses Suspend = run(
    let { producer, consumer } = channel<Result>(
        .buffer: 100,
        .share: Producers,
    ),

    parallel(
        .workers: (0..4).map(transform: i -> worker(
            config: config.clone(),  // Each worker gets own copy
            producer: producer.clone(),
        )),
        .collector: collect(consumer: consumer),
    ),
)
```

### Clone Constraint in Generics

```ori
@duplicate<T: Clone> (value: T, count: int) -> [T] =
    (0..count).map(transform: _ -> value.clone()).collect()

@cache_result<K: Eq + Hashable, V: Clone> (
    cache: {K: V},
    key: K,
    compute: () -> V,
) -> (V, {K: V}) = match(
    cache[key],
    Some(v) -> (v.clone(), cache),  // Return clone, keep original in cache
    None -> run(
        let v = compute(),
        (v.clone(), cache.insert(key: key, value: v)),
    ),
)
```

---

## Spec Changes Required

### `06-types.md`

Add to Built-in Types section:

```markdown
### Clone Trait

```ori
trait Clone {
    @clone (self) -> Self
}
```

Creates an independent copy of a value. Implemented by all primitive types and derivable for user-defined types where all fields implement `Clone`.
```

### `08-declarations.md`

Add Clone to trait examples:

```ori
trait Clone {
    @clone (self) -> Self
}
```

### `12-modules.md`

Update prelude traits description to note Clone provides `.clone()` method.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Method | `@clone (self) -> Self` |
| Primitives | All implement Clone |
| Collections | Clone if element types Clone |
| Derivable | Yes, if all fields Clone |
| Channels | Conditional based on Sharing mode |
| ARC impact | None — creates independent copy |

This proposal fills a spec gap that's blocking other features. Implementation is straightforward.

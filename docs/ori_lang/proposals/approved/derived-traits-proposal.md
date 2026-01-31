# Proposal: Derived Traits

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, type system, traits

---

## Summary

This proposal formalizes the `#derive` attribute semantics, including derivable traits, derivation rules, field constraints, and error handling.

---

## Problem Statement

The spec mentions `#derive(Eq, Hashable, Clone)` syntax but leaves unclear:

1. **Complete list**: Which traits are derivable?
2. **Field requirements**: What constraints apply to fields?
3. **Derivation rules**: How is each trait derived?
4. **Ordering**: Does derive order matter?
5. **Errors**: What happens when derivation fails?

---

## Syntax

```ori
#derive(Trait1, Trait2, ...)
type TypeName = { ... }
```

Multiple traits can be derived in a single attribute. The attribute must immediately precede the type definition.

---

## Derivable Traits

### Core Traits

| Trait | Requirement | Generated Implementation |
|-------|-------------|-------------------------|
| `Eq` | All fields implement `Eq` | Field-wise equality |
| `Hashable` | All fields implement `Hashable` | Combined field hashes |
| `Comparable` | All fields implement `Comparable` | Lexicographic comparison |
| `Clone` | All fields implement `Clone` | Field-wise clone |
| `Default` | All fields implement `Default` | Field-wise default |
| `Debug` | All fields implement `Debug` | Formatted struct representation |
| `Printable` | All fields implement `Printable` | Human-readable representation |

---

## Derivation Rules

### Eq

Field-wise equality comparison:

```ori
#derive(Eq)
type Point = { x: int, y: int }

// Generated:
impl Eq for Point {
    @equals (self, other: Point) -> bool =
        self.x == other.x && self.y == other.y
}
```

For sum types:

```ori
#derive(Eq)
type Status = Pending | Running(progress: int) | Done

// Generated:
impl Eq for Status {
    @equals (self, other: Status) -> bool = match((self, other),
        (Pending, Pending) -> true,
        (Running(a), Running(b)) -> a == b,
        (Done, Done) -> true,
        _ -> false,
    )
}
```

### Hashable

Combined hash from all fields:

```ori
#derive(Hashable)
type Point = { x: int, y: int }

// Generated:
impl Hashable for Point {
    @hash (self) -> int = run(
        let h = 0,
        h = hash_combine(seed: h, value: self.x.hash()),
        h = hash_combine(seed: h, value: self.y.hash()),
        h,
    )
}
```

**Invariant:** If `a == b`, then `a.hash() == b.hash()`. Deriving `Hashable` without `Eq` is a warning.

### Comparable

Lexicographic field comparison (declaration order):

```ori
#derive(Comparable)
type Point = { x: int, y: int }

// Generated:
impl Comparable for Point {
    @compare (self, other: Point) -> Ordering = match(compare(left: self.x, right: other.x),
        Equal -> compare(left: self.y, right: other.y),
        result -> result,
    )
}
```

For sum types, variants compare by declaration order:

```ori
#derive(Comparable)
type Priority = Low | Medium | High

// Low < Medium < High (declaration order)
```

### Clone

Field-wise cloning:

```ori
#derive(Clone)
type Config = { host: str, port: int, options: Options }

// Generated:
impl Clone for Config {
    @clone (self) -> Config = Config {
        host: self.host.clone(),
        port: self.port.clone(),
        options: self.options.clone(),
    }
}
```

### Default

Field-wise default construction:

```ori
#derive(Default)
type Config = { host: str, port: int, debug: bool }

// Generated:
impl Default for Config {
    @default () -> Config = Config {
        host: str.default(),    // ""
        port: int.default(),    // 0
        debug: bool.default(),  // false
    }
}
```

Sum types cannot derive `Default` (which variant?).

### Debug

Structural representation with type name:

```ori
#derive(Debug)
type Point = { x: int, y: int }

// Generated:
impl Debug for Point {
    @debug (self) -> str = `Point \{ x: {self.x.debug()}, y: {self.y.debug()} \}`
}

Point { x: 1, y: 2 }.debug()  // "Point { x: 1, y: 2 }"
```

### Printable

Human-readable representation with type name:

```ori
#derive(Printable)
type Point = { x: int, y: int }

// Generated:
impl Printable for Point {
    @to_str (self) -> str = `Point({self.x}, {self.y})`
}

Point { x: 1, y: 2 }.to_str()  // "Point(1, 2)"
```

**Note:** Types implementing `Printable` automatically implement `Formattable` via a blanket implementation. Deriving `Printable` therefore provides `Formattable` as well.

---

## Field Constraints

### All Fields Must Implement

Derivation fails if any field doesn't implement the required trait:

```ori
type NonHashable = { data: FileHandle }  // FileHandle: !Hashable

#derive(Hashable)
type Container = { item: NonHashable }  // ERROR
```

### Recursive Types

Recursive types can derive traits:

```ori
#derive(Eq, Clone, Debug)
type Tree = Leaf(value: int) | Node(left: Tree, right: Tree)
```

The generated implementation handles recursion correctly.

---

## Multiple Derives

### Single Attribute

```ori
#derive(Eq, Hashable, Clone, Debug)
type Point = { x: int, y: int }
```

### Multiple Attributes

```ori
#derive(Eq, Hashable)
#derive(Clone, Debug)
type Point = { x: int, y: int }
```

Both forms are equivalent.

### Order Independence

Derive order does not affect behavior:

```ori
#derive(Hashable, Eq)  // Same as #derive(Eq, Hashable)
```

---

## Generic Types

### Conditional Derivation

Generic types derive traits when type parameters satisfy constraints:

```ori
#derive(Eq, Clone, Debug)
type Pair<T> = { first: T, second: T }

// Pair<int> implements Eq, Clone, Debug (int implements all)
// Pair<FileHandle> implements Debug only (FileHandle: !Eq, !Clone)
```

The compiler generates bounded implementations:

```ori
impl<T: Eq> Eq for Pair<T> { ... }
impl<T: Clone> Clone for Pair<T> { ... }
impl<T: Debug> Debug for Pair<T> { ... }
```

---

## Cannot Derive

### Traits Not Derivable

Some traits cannot be derived:

| Trait | Reason |
|-------|--------|
| `Iterator` | Requires custom `next` logic |
| `Iterable` | Requires custom `iter` logic |
| `Into` | Requires custom conversion logic |
| `Drop` | Requires custom cleanup logic |
| `Sendable` | Automatically derived by compiler |

### Manual Implementation Required

```ori
// Iterator cannot be derived
impl Iterator for MyIter {
    type Item = int
    @next (self) -> (Option<int>, Self) = ...  // Custom logic
}
```

---

## Error Messages

### Field Missing Trait

```
error[E0880]: cannot derive `Eq` for `Container`
  --> src/types.ori:2:10
   |
 2 | #derive(Eq)
   |         ^^ `Eq` cannot be derived
 3 | type Container = { item: FileHandle }
   |                          ---------- `FileHandle` does not implement `Eq`
   |
   = help: implement `Eq` manually or use a different field type
```

### Non-Derivable Trait

```
error[E0881]: trait `Iterator` cannot be derived
  --> src/types.ori:1:10
   |
 1 | #derive(Iterator)
   |         ^^^^^^^^ not derivable
   |
   = note: derivable traits: Eq, Hashable, Comparable, Clone, Default, Debug, Printable
   = help: implement `Iterator` manually
```

### Default for Sum Type

```
error[E0882]: cannot derive `Default` for sum type
  --> src/types.ori:1:10
   |
 1 | #derive(Default)
   |         ^^^^^^^ not derivable for sum types
 2 | type Status = Pending | Running | Done
   |
   = note: sum types have multiple variants; no unambiguous default
   = help: implement `Default` manually to specify which variant
```

### Hashable Without Eq Warning

```
warning[W0100]: `Hashable` derived without `Eq`
  --> src/types.ori:1:10
   |
 1 | #derive(Hashable)
   |         ^^^^^^^^
   |
   = note: hash equality invariant: a == b implies a.hash() == b.hash()
   = help: also derive `Eq`: #derive(Eq, Hashable)
```

---

## Examples

### Complete Data Type

```ori
#derive(Eq, Hashable, Comparable, Clone, Debug)
type User = {
    id: int,
    name: str,
    email: str,
    created_at: Duration,
}
```

### Sum Type

```ori
#derive(Eq, Clone, Debug)
type JsonValue =
    | Null
    | Bool(bool)
    | Number(float)
    | String(str)
    | Array([JsonValue])
    | Object({str: JsonValue})
```

### Generic Container

```ori
#derive(Eq, Clone, Debug)
type Box<T> = { value: T }

// Box<int>: Eq + Clone + Debug
// Box<[int]>: Eq + Clone + Debug
```

---

## Spec Changes Required

### Update `06-types.md`

Expand Derive section with:
1. Complete list of derivable traits
2. Derivation rules for each trait
3. Field constraint requirements
4. Generic type conditional derivation

### Update `07-properties-of-types.md`

Add cross-reference to derive semantics.

---

## Summary

| Trait | Derivable | Struct | Sum Type | Requirement |
|-------|-----------|--------|----------|-------------|
| `Eq` | Yes | Yes | Yes | All fields: `Eq` |
| `Hashable` | Yes | Yes | Yes | All fields: `Hashable` |
| `Comparable` | Yes | Yes | Yes | All fields: `Comparable` |
| `Clone` | Yes | Yes | Yes | All fields: `Clone` |
| `Default` | Yes | Yes | No | All fields: `Default` |
| `Debug` | Yes | Yes | Yes | All fields: `Debug` |
| `Printable` | Yes | Yes | Yes | All fields: `Printable` |
| `Iterator` | No | — | — | — |

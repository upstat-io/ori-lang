# Proposal: Custom Subscripting for User-Defined Types

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Approved:** 2026-01-30
**Affects:** Language design, type system, standard library

---

## Executive Summary

This proposal introduces trait-based custom subscripting, allowing user-defined types to implement the `[]` operator with compile-time type safety. This enables natural syntax for matrices, custom containers, database abstractions, and domain-specific indexed access.

**Key features:**
1. **Index trait** for indexed read access: `value[key]`
2. **Multi-dimensional indexing** via tuple keys: `matrix[(row, col)]`
3. **Return type flexibility** - can return `T`, `Option<T>`, or `Result<T, E>`

---

## Motivation

### Current State

Ori has built-in subscripting for core types:

```ori
list[0]         // [T] -> T (panics on out-of-bounds)
map["key"]      // {K: V} -> Option<V>
str[0]          // str -> str (single codepoint, panics on out-of-bounds)
```

### Problem

User-defined types cannot use `[]` syntax. This forces verbose method calls:

```ori
// Current: verbose
let value = matrix.get(row: row, col: col)

// Desired: natural
let value = matrix[(row, col)]
```

### Use Cases

| Domain | Type | Index | Returns |
|--------|------|-------|---------|
| Linear algebra | `Matrix<T>` | `(int, int)` | `T` |
| Sparse data | `SparseVector<T>` | `int` | `Option<T>` |
| Database | `Row` | `str` | `Result<Value, ColumnError>` |
| Ring buffer | `RingBuffer<T>` | `int` | `T` |
| Bitset | `BitSet` | `int` | `bool` |
| JSON | `JsonValue` | `str` or `int` | `Option<JsonValue>` |
| Cache | `Cache<K, V>` | `K` | `Option<V>` |

---

## Proposed Design

### Core Trait

```ori
// Read-only indexed access
trait Index<Key, Value> {
    @index (self, key: Key) -> Value
}
```

### Desugaring

The compiler transforms subscript syntax into trait method calls:

```ori
// Read access
x[key]
// Desugars to:
x.index(key: key)
```

### Basic Example: Matrix

```ori
type Matrix<T> = {
    data: [T],
    rows: int,
    cols: int,
}

impl<T> Index<(int, int), T> for Matrix<T> {
    @index (self, key: (int, int)) -> T = run(
        let (row, col) = key,
        assert(condition: row >= 0 && row < self.rows, msg: "row out of bounds"),
        assert(condition: col >= 0 && col < self.cols, msg: "col out of bounds"),
        self.data[row * self.cols + col],
    )
}

// Usage
@example () -> void = run(
    let m = Matrix.identity(size: 3),
    print(msg: m[(1, 1)] as str),  // prints: 1
)
```

### Fallible Indexing: Option and Result

Return types can encode failure:

```ori
// Sparse vector - returns Option<T>
type SparseVector<T> = {
    entries: {int: T},
    size: int,
}

impl<T> Index<int, Option<T>> for SparseVector<T> {
    @index (self, key: int) -> Option<T> =
        if key < 0 || key >= self.size
        then None
        else self.entries[key]
}

// Database row - returns Result<Value, Error>
type Row = {
    columns: {str: Value},
}

impl Index<str, Result<Value, ColumnError>> for Row {
    @index (self, key: str) -> Result<Value, ColumnError> =
        match(self.columns[key],
            Some(v) -> Ok(v),
            None -> Err(ColumnError.NotFound(key)),
        )
}

// Usage with ? operator
@query_example (row: Row) -> Result<int, ColumnError> = run(
    let age = row["age"]?,
    Ok(age.as_int()),
)
```

### Multiple Index Types

A single type can implement multiple Index traits:

```ori
type JsonValue =
    | Null
    | Bool(bool)
    | Number(float)
    | String(str)
    | Array([JsonValue])
    | Object({str: JsonValue})

// Index by string (object access)
impl Index<str, Option<JsonValue>> for JsonValue {
    @index (self, key: str) -> Option<JsonValue> =
        match(self,
            Object(map) -> map[key],
            _ -> None,
        )
}

// Index by int (array access)
impl Index<int, Option<JsonValue>> for JsonValue {
    @index (self, key: int) -> Option<JsonValue> =
        match(self,
            Array(arr) -> if key >= 0 && key < len(collection: arr) then Some(arr[key]) else None,
            _ -> None,
        )
}

// Usage
@json_example (json: JsonValue) -> void = run(
    let name = json["user"]["name"],      // Object path
    let first_item = json["items"][0],    // Mixed access
)
```

---

## Interaction with Existing Features

### The `#` Length Shorthand

Ori's `#` shorthand for length inside brackets is supported only for built-in types (`[T]`, `str`). Custom types use `len()` explicitly:

```ori
// Built-in: # works
list[# - 1]

// Custom: use explicit len()
let size = len(collection: matrix.data)
matrix[(size - 1, size - 1)]
```

This keeps the semantics simple and avoids ambiguity for multi-dimensional containers where "length" could mean different things.

---

## Comparison with Other Languages

| Language | Mechanism | Type Safety | Multiple Index Types |
|----------|-----------|-------------|---------------------|
| Obj-C | `objectAtIndexedSubscript:` | Runtime | Yes (separate methods) |
| Swift | `subscript` keyword | Compile-time | Yes |
| Rust | `Index`/`IndexMut` traits | Compile-time | Yes (via generics) |
| Python | `__getitem__`/`__setitem__` | Runtime | Yes |
| C++ | `operator[]` | Compile-time | No (single signature) |
| **Ori** | `Index` trait | Compile-time | Yes (via generics) |

Ori's approach is similar to Rust's `Index` trait, which has proven effective.

---

## Standard Library Implementations

These built-in types would have explicit trait implementations:

```ori
// List
impl<T> Index<int, T> for [T] { ... }

// Map - read returns Option
impl<K: Hashable, V> Index<K, Option<V>> for {K: V} { ... }

// String - read only, returns str (single codepoint)
impl Index<int, str> for str { ... }
```

---

## Error Handling Strategies

Different types can choose appropriate error handling:

| Strategy | Return Type | Use Case |
|----------|-------------|----------|
| Panic | `T` | Fixed-size containers with programmer-controlled indices |
| Option | `Option<T>` | Sparse data, optional lookup |
| Result | `Result<T, E>` | External data, detailed error info needed |

```ori
// Panic strategy (Matrix - programmer error if out of bounds)
impl Index<(int, int), T> for Matrix<T> {
    @index (self, key: (int, int)) -> T = ...  // panics on invalid
}

// Option strategy (SparseVector - missing is normal)
impl Index<int, Option<T>> for SparseVector<T> {
    @index (self, key: int) -> Option<T> = ...
}

// Result strategy (DatabaseRow - need error details)
impl Index<str, Result<Value, DbError>> for Row {
    @index (self, key: str) -> Result<Value, DbError> = ...
}
```

---

## Implementation Notes

### Compiler Changes

1. **Trait definition**: Add `Index` to prelude
2. **Desugaring pass**: Transform `x[k]` to `x.index(key: k)`
3. **Type inference**: Resolve which `Index` impl based on key type

### Ambiguity Resolution

If multiple `Index` impls could apply, the key type must be unambiguous:

```ori
// JsonValue implements Index<str, ...> and Index<int, ...>
json["key"]   // Unambiguous: str literal
json[0]       // Unambiguous: int literal
json[x]       // Depends on type of x
```

---

## Examples

### Ring Buffer

```ori
type RingBuffer<T> = {
    data: [T],
    head: int,
    len: int,
}

impl<T> Index<int, T> for RingBuffer<T> {
    @index (self, key: int) -> T = run(
        assert(condition: key >= 0 && key < self.len, msg: "index out of bounds"),
        let actual_idx = (self.head + key) % len(collection: self.data),
        self.data[actual_idx],
    )
}
```

### BitSet

```ori
type BitSet = {
    bits: [byte],
    size: int,
}

impl Index<int, bool> for BitSet {
    @index (self, key: int) -> bool = run(
        assert(condition: key >= 0 && key < self.size, msg: "index out of bounds"),
        let byte_idx = key / 8,
        let bit_idx = key % 8,
        (self.bits[byte_idx] >> bit_idx) & 1 == 1,
    )
}

// Usage
@bitset_example (bs: BitSet) -> void = run(
    print(msg: bs[0] as str),   // read bit at index 0
    print(msg: bs[1] as str),   // read bit at index 1
)
```

### 2D Game Grid

```ori
type Grid<T> = {
    cells: [T],
    width: int,
    height: int,
}

impl<T> Index<(int, int), Option<T>> for Grid<T> {
    @index (self, key: (int, int)) -> Option<T> = run(
        let (x, y) = key,
        if x < 0 || x >= self.width || y < 0 || y >= self.height
        then None
        else Some(self.cells[y * self.width + x]),
    )
}

// Game usage - safe neighbor checking
@count_neighbors (grid: Grid<bool>, x: int, y: int) -> int =
    [(-1, -1), (0, -1), (1, -1),
     (-1,  0),          (1,  0),
     (-1,  1), (0,  1), (1,  1)]
    .filter(d -> grid[(x + d.0, y + d.1)] == Some(true))
    .count()
```

---

## Design Decisions

### Why No IndexMut?

Ori uses value semantics without shared mutable references. The `mut self` pattern required for in-place mutation does not exist in the language. For types that need mutation, use explicit methods:

```ori
// Instead of: matrix[(0, 0)] = 5
// Use:
let matrix = matrix.set(row: 0, col: 0, value: 5)

// Or provide a method that returns a modified copy
impl<T> Matrix<T> {
    @set (self, row: int, col: int, value: T) -> Matrix<T> = ...
}
```

This aligns with Ori's functional approach and ARC-based memory model.

### Why No Slicing in This Proposal?

Slicing (`x[0..5]`) has different semantics and return types than point indexing. It deserves its own proposal to properly design the `Slice` trait and its interactions with ranges.

---

## Migration / Compatibility

This is a new feature with no breaking changes:
- Existing code continues to work
- Built-in subscripting gains explicit trait implementations
- New types can opt into subscripting by implementing the `Index` trait

---

## References

- [Rust `Index` trait](https://doc.rust-lang.org/std/ops/trait.Index.html)
- [Swift Subscripts](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/subscripts/)
- [Python Data Model - `__getitem__`](https://docs.python.org/3/reference/datamodel.html#object.__getitem__)

---

## Changelog

- 2026-01-22: Initial draft
- 2026-01-30: Approved — Removed IndexMut (Ori has no `mut`), removed Slice (deferred), removed `#`/Sized discussion, added to prelude

---

## Errata (2026-02-17)

> **Superseded by [index-assignment-proposal](index-assignment-proposal.md)**: The "Why No IndexMut?" section above is stale. Index and field assignment have been approved via copy-on-write desugaring using the `IndexSet` trait with an `updated(key:, value:)` method. The original rejection reasoning — that `mut self` is required — was incorrect. Ori's mutable bindings and copy-on-write semantics enable `list[i] = x` to desugar to `list = list.updated(key: i, value: x)` without mutable references. The recommended alternative `matrix.set(row:, col:, value:)` was never implemented.

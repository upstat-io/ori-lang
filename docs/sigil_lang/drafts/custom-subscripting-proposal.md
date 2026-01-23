# Proposal: Custom Subscripting for User-Defined Types

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Language design, type system, standard library

---

## Executive Summary

This proposal introduces trait-based custom subscripting, allowing user-defined types to implement the `[]` operator with compile-time type safety. This enables natural syntax for matrices, custom containers, database abstractions, and domain-specific indexed access.

**Key features:**
1. **Index trait** for read access: `value[key]`
2. **IndexMut trait** for write access: `value[key] = x`
3. **Multi-dimensional indexing** via tuple keys: `matrix[(row, col)]`
4. **Return type flexibility** - can return `T`, `Option<T>`, or `Result<T, E>`

---

## Motivation

### Current State

Sigil has built-in subscripting for core types:

```sigil
list[0]         // [T] -> T (panics on out-of-bounds)
map["key"]      // {K: V} -> Option<V>
str[0]          // str -> str (single codepoint, panics on out-of-bounds)
```

### Problem

User-defined types cannot use `[]` syntax. This forces verbose method calls:

```sigil
// Current: verbose
let value = matrix.get(row, col)
matrix.set(row, col, 42)

// Desired: natural
let value = matrix[(row, col)]
matrix[(row, col)] = 42
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

### Core Traits

```sigil
// Read-only indexed access
trait Index<Key, Value> {
    @index (self, key: Key) -> Value
}

// Read-write indexed access (extends Index)
trait IndexMut<Key, Value>: Index<Key, Value> {
    @index_mut (mut self, key: Key, value: Value) -> void
}
```

### Desugaring

The compiler transforms subscript syntax into trait method calls:

```sigil
// Read access
x[key]
// Desugars to:
x.index(key)

// Write access
x[key] = value
// Desugars to:
x.index_mut(key, value)
```

### Basic Example: Matrix

```sigil
type Matrix<T> = {
    data: [T],
    rows: int,
    cols: int,
}

impl<T> Index<(int, int), T> for Matrix<T> {
    @index (self, key: (int, int)) -> T = run(
        let (row, col) = key,
        assert(row >= 0 && row < self.rows, "row out of bounds"),
        assert(col >= 0 && col < self.cols, "col out of bounds"),
        self.data[row * self.cols + col],
    )
}

impl<T> IndexMut<(int, int), T> for Matrix<T> {
    @index_mut (mut self, key: (int, int), value: T) -> void = run(
        let (row, col) = key,
        assert(row >= 0 && row < self.rows, "row out of bounds"),
        assert(col >= 0 && col < self.cols, "col out of bounds"),
        self.data[row * self.cols + col] = value,
    )
}

// Usage
@example () -> void = run(
    let mut m = Matrix.new(3, 3, 0),
    m[(0, 0)] = 1,
    m[(1, 1)] = 1,
    m[(2, 2)] = 1,
    print(m[(1, 1)]),  // prints: 1
)
```

### Fallible Indexing: Option and Result

Return types can encode failure:

```sigil
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

```sigil
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
            Array(arr) -> if key >= 0 && key < len(arr) then Some(arr[key]) else None,
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

Sigil's `#` shorthand for length inside brackets should work with custom types that implement `Sized`:

```sigil
trait Sized {
    @len (self) -> int
}

// If Matrix implements Sized:
impl<T> Sized for Matrix<T> {
    @len (self) -> int = self.rows * self.cols
}

// Then # works inside brackets
matrix[(# - 1, # - 1)]  // Last element (assumes square, # refers to len)
```

**Open question:** Should `#` refer to the container's length, or should we require explicit `len(matrix)` for custom types? The built-in behavior for lists (`list[# - 1]`) is convenient but the semantics for multi-dimensional containers are ambiguous.

**Recommendation:** `#` only works for built-in types. Custom types use explicit `len()`.

### Range Indexing (Slicing)

Should custom types support range indexing?

```sigil
list[0..5]      // Built-in: returns [T]
matrix[0..2]    // Custom: what does this return?
```

**Proposal:** Separate trait for slicing:

```sigil
trait Slice<RangeType, Output> {
    @slice (self, range: RangeType) -> Output
}

impl<T> Slice<Range<int>, [T]> for [T] {
    @slice (self, range: Range<int>) -> [T] = ...
}

// Matrix row slice
impl<T> Slice<int, [T]> for Matrix<T> {
    @slice (self, row: int) -> [T] = ...  // Returns entire row
}
```

This keeps `Index` simple while allowing rich slicing behavior.

---

## Comparison with Other Languages

| Language | Mechanism | Type Safety | Multiple Index Types |
|----------|-----------|-------------|---------------------|
| Obj-C | `objectAtIndexedSubscript:` | Runtime | Yes (separate methods) |
| Swift | `subscript` keyword | Compile-time | Yes |
| Rust | `Index`/`IndexMut` traits | Compile-time | Yes (via generics) |
| Python | `__getitem__`/`__setitem__` | Runtime | Yes |
| C++ | `operator[]` | Compile-time | No (single signature) |
| **Sigil** | `Index`/`IndexMut` traits | Compile-time | Yes (via generics) |

Sigil's approach is most similar to Rust, which has proven effective.

---

## Standard Library Implementations

These built-in types would have explicit trait implementations:

```sigil
// List
impl<T> Index<int, T> for [T] { ... }
impl<T> IndexMut<int, T> for [T] { ... }

// Map - read returns Option, write inserts
impl<K: Hashable, V> Index<K, Option<V>> for {K: V} { ... }
impl<K: Hashable, V> IndexMut<K, V> for {K: V} { ... }  // Inserts or updates

// String - read only, returns str (single codepoint)
impl Index<int, str> for str { ... }
// Note: strings are immutable, no IndexMut
```

---

## Error Handling Strategies

Different types can choose appropriate error handling:

| Strategy | Return Type | Use Case |
|----------|-------------|----------|
| Panic | `T` | Fixed-size containers with programmer-controlled indices |
| Option | `Option<T>` | Sparse data, optional lookup |
| Result | `Result<T, E>` | External data, detailed error info needed |

```sigil
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

1. **Trait definitions**: Add `Index` and `IndexMut` to prelude
2. **Desugaring pass**: Transform `x[k]` to `x.index(k)` and `x[k] = v` to `x.index_mut(k, v)`
3. **Type inference**: Resolve which `Index` impl based on key type
4. **Mutability check**: `x[k] = v` requires `x` to be `mut` and `IndexMut` to be implemented

### Ambiguity Resolution

If multiple `Index` impls could apply, the key type must be unambiguous:

```sigil
// JsonValue implements Index<str, ...> and Index<int, ...>
json["key"]   // Unambiguous: str literal
json[0]       // Unambiguous: int literal
json[x]       // Depends on type of x
```

---

## Examples

### Ring Buffer

```sigil
type RingBuffer<T> = {
    data: [T],
    head: int,
    len: int,
}

impl<T> Index<int, T> for RingBuffer<T> {
    @index (self, key: int) -> T = run(
        assert(key >= 0 && key < self.len, "index out of bounds"),
        let actual_idx = (self.head + key) % len(self.data),
        self.data[actual_idx],
    )
}

impl<T> IndexMut<int, T> for RingBuffer<T> {
    @index_mut (mut self, key: int, value: T) -> void = run(
        assert(key >= 0 && key < self.len, "index out of bounds"),
        let actual_idx = (self.head + key) % len(self.data),
        self.data[actual_idx] = value,
    )
}
```

### BitSet

```sigil
type BitSet = {
    bits: [byte],
    size: int,
}

impl Index<int, bool> for BitSet {
    @index (self, key: int) -> bool = run(
        assert(key >= 0 && key < self.size, "index out of bounds"),
        let byte_idx = key / 8,
        let bit_idx = key % 8,
        (self.bits[byte_idx] >> bit_idx) & 1 == 1,
    )
}

impl IndexMut<int, bool> for BitSet {
    @index_mut (mut self, key: int, value: bool) -> void = run(
        assert(key >= 0 && key < self.size, "index out of bounds"),
        let byte_idx = key / 8,
        let bit_idx = key % 8,
        if value
        then self.bits[byte_idx] = self.bits[byte_idx] | (1 << bit_idx)
        else self.bits[byte_idx] = self.bits[byte_idx] & ~(1 << bit_idx),
    )
}

// Usage
@bitset_example () -> void = run(
    let mut bs = BitSet.new(64),
    bs[0] = true,
    bs[63] = true,
    print(bs[0]),   // true
    print(bs[1]),   // false
)
```

### 2D Game Grid

```sigil
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
    |> filter(d -> grid[(x + d.0, y + d.1)] == Some(true))
    |> len()
```

---

## Open Questions

### Q1: Should `#` work in custom subscripts?

**Options:**
- A) Yes, `#` always means `len(container)` - consistent but may be confusing for multi-dimensional
- B) No, `#` only for built-in types - explicit but less convenient
- C) `#` works if type implements `Sized` trait - flexible but adds complexity

**Recommendation:** Option B for simplicity.

### Q2: Chained mutable indexing?

```sigil
matrix[(0, 0)][(1, 1)] = 5  // Is this allowed?
```

**Options:**
- A) Allow if intermediate returns mutable reference
- B) Disallow, require explicit temp variable
- C) Allow for specific patterns only

**Recommendation:** Option B initially - keep semantics simple.

### Q3: Compound assignment operators?

```sigil
matrix[(0, 0)] += 5  // Desugar to what?
```

**Proposal:** Desugar to read-modify-write:
```sigil
// matrix[(0, 0)] += 5 becomes:
matrix.index_mut((0, 0), matrix.index((0, 0)) + 5)
```

---

## Migration / Compatibility

This is a new feature with no breaking changes:
- Existing code continues to work
- Built-in subscripting gains explicit trait implementations
- New types can opt into subscripting by implementing traits

---

## References

- [Rust `Index` trait](https://doc.rust-lang.org/std/ops/trait.Index.html)
- [Swift Subscripts](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/subscripts/)
- [Python Data Model - `__getitem__`](https://docs.python.org/3/reference/datamodel.html#object.__getitem__)

---

## Changelog

- 2026-01-22: Initial draft

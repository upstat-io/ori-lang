# Proposal: Index Trait

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, expressions

---

## Summary

This proposal formalizes the `Index` trait for custom subscripting, including return type variants, multiple key types, and error handling patterns.

---

## Problem Statement

The spec shows `Index` trait usage but leaves unclear:

1. **Return type variants**: What return types are valid?
2. **Multiple keys**: How to support different key types?
3. **Error handling**: When should Index panic vs return Option?
4. **Hash shorthand**: Does `#` work with custom Index?
5. **Assignment**: Can Index support `x[k] = v`?

---

## Definition

```ori
trait Index<Key, Value> {
    @index (self, key: Key) -> Value
}
```

The `Index` trait enables subscript syntax `container[key]`.

---

## Desugaring

```ori
x[key]
// Desugars to:
x.index(key: key)
```

---

## Return Type Variants

### Direct Value (Panic on Missing)

For containers where access should always succeed:

```ori
impl Index<int, T> for [T] {
    @index (self, key: int) -> T  // Panics if out of bounds
}

let list = [1, 2, 3]
list[0]   // 1
list[10]  // panic: index out of bounds
```

### Option (Missing Returns None)

For containers where keys may be absent:

```ori
impl Index<K, Option<V>> for {K: V} {
    @index (self, key: K) -> Option<V>  // None if not present
}

let map = {"a": 1, "b": 2}
map["a"]  // Some(1)
map["z"]  // None
```

### Result (Detailed Error)

For containers where access can fail with details:

```ori
// Example: user-defined JSON type with Result-based indexing
type JsonError = KeyNotFound { key: str } | NotAnObject

impl Index<str, Result<JsonValue, JsonError>> for JsonValue {
    @index (self, key: str) -> Result<JsonValue, JsonError>
}

let json = parse_json(text)?
json["field"]?  // Propagate JsonError if field missing
```

---

## Multiple Key Types

A type can implement `Index` for multiple key types:

```ori
impl Index<int, Option<JsonValue>> for JsonValue {
    @index (self, key: int) -> Option<JsonValue> = ...
}

impl Index<str, Option<JsonValue>> for JsonValue {
    @index (self, key: str) -> Option<JsonValue> = ...
}

let json = get_json()
json[0]       // Array access
json["key"]   // Object access
```

### Type Inference

The key type must be unambiguous:

```ori
json[key]  // OK if key has known type
json[0]    // OK: int literal
json["x"]  // OK: str literal
```

Ambiguous cases are compile errors:

```ori
let key = ???  // Unknown type
json[key]  // ERROR: ambiguous Index implementation
```

---

## Standard Implementations

### List

```ori
impl<T> Index<int, T> for [T] {
    @index (self, key: int) -> T =
        if key < 0 || key >= len(collection: self) then
            panic(msg: "index out of bounds")
        else
            // intrinsic: compiler-provided
}
```

### Fixed-Capacity List

```ori
impl<T, $N: int> Index<int, T> for [T, max N] {
    @index (self, key: int) -> T = ...  // Same as [T]
}
```

### Map

```ori
impl<K: Eq + Hashable, V> Index<K, Option<V>> for {K: V} {
    @index (self, key: K) -> Option<V> = ...
}
```

### String

```ori
impl Index<int, str> for str {
    @index (self, key: int) -> str =  // Returns single-codepoint str
        if key < 0 || key >= len(collection: self) then
            panic(msg: "index out of bounds")
        else
            // intrinsic: compiler-provided
}
```

---

## Hash Shorthand

The `#` shorthand for length is built-in and does NOT work with custom Index:

```ori
let list = [1, 2, 3]
list[# - 1]  // OK: built-in list indexing

type MyContainer = { ... }
impl Index<int, int> for MyContainer { ... }

let c = MyContainer { ... }
c[# - 1]  // ERROR: # not available for custom Index
c[len(collection: c) - 1]  // Use explicit len()
```

### Rationale

`#` is syntactic sugar that requires compiler knowledge of container length semantics. Custom containers should use `len()` explicitly.

---

## No Index Assignment

Ori does not support index assignment syntax:

```ori
list[0] = 42  // ERROR: assignment via index not supported
```

Instead, use methods that return modified collections:

```ori
let new_list = list.set(index: 0, value: 42)
```

### Rationale

Ori's memory model prefers immutable updates. Index assignment would require mutable references, which Ori avoids.

---

## Custom Index Examples

### Matrix

```ori
type Matrix = { rows: [[float]] }

impl Index<(int, int), float> for Matrix {
    @index (self, key: (int, int)) -> float = run(
        let (row, col) = key,
        self.rows[row][col],
    )
}

let m = Matrix { rows: [[1.0, 2.0], [3.0, 4.0]] }
m[(0, 1)]  // 2.0
```

### Sparse Array

```ori
type SparseArray<T> = { data: {int: T}, default: T }

impl<T: Clone> Index<int, T> for SparseArray<T> {
    @index (self, key: int) -> T = match(self.data[key],
        Some(v) -> v,
        None -> self.default.clone(),
    )
}
```

### Config with Path

```ori
type Config = { data: {str: JsonValue} }

impl Index<str, Option<JsonValue>> for Config {
    @index (self, key: str) -> Option<JsonValue> = run(
        let parts = key.split(sep: "."),
        parts.fold(
            initial: Some(JsonValue.Object(self.data)),
            combine: (acc, part) -> match(acc,
                Some(JsonValue.Object(obj)) -> obj[part],
                _ -> None,
            ),
        ),
    )
}

let config = load_config()
config["database.host"]  // Navigate nested path
```

---

## Type Errors

### Wrong Key Type

```
error[E0950]: mismatched types in index expression
  --> src/main.ori:5:10
   |
 5 | let x = list["key"]
   |              ^^^^^ expected `int`, found `str`
   |
   = note: `[int]` implements `Index<int, int>`, not `Index<str, _>`
```

### No Index Implementation

```
error[E0951]: `MyType` cannot be indexed
  --> src/main.ori:5:10
   |
 5 | let x = my_value[0]
   |         ^^^^^^^^^^^ `Index` not implemented
   |
   = help: implement `Index<int, _>` for `MyType`
```

### Ambiguous Key Type

```
error[E0952]: ambiguous index key type
  --> src/main.ori:5:10
   |
 5 | let x = json[key]
   |              ^^^ cannot infer key type
   |
   = note: `JsonValue` implements both `Index<int, _>` and `Index<str, _>`
   = help: add type annotation: `let key: int = ...`
```

---

## Spec Changes Required

### Update `09-expressions.md`

Expand Index Trait section with:
1. Return type variants
2. Multiple key type support
3. Standard implementations
4. Hash shorthand limitation

### Update `06-types.md`

Cross-reference to Index trait.

---

## Summary

| Aspect | Details |
|--------|---------|
| Syntax | `x[key]` desugars to `x.index(key: key)` |
| Trait | `trait Index<Key, Value> { @index (self, key: Key) -> Value }` |
| Return types | `T` (panic), `Option<T>` (none), `Result<T, E>` (error) |
| Multiple keys | Implement Index for each key type |
| Hash shorthand | Built-in only, not for custom Index |
| Assignment | Not supported (`x[k] = v` is error) — see Errata |

---

## Errata (2026-02-17)

1. **"No Index Assignment" reasoning is stale.** Section "No Index Assignment" states *"Ori's memory model prefers immutable updates. Index assignment would require mutable references."* This conflates in-place mutation with reassignment. Per `spec/05-variables.md`, Ori supports mutable bindings (`let x = 0; x = x + 1`). Index assignment could desugar to `list = list.set(index: i, value: x)` (copy-on-write reassignment) without requiring mutable references. The decision to reject index assignment should be revisited.

2. **`list.set(index:, value:)` does not exist.** The alternative recommended in the "No Index Assignment" section references a method that was never implemented. No `set` method is registered for lists, maps, or any built-in type.

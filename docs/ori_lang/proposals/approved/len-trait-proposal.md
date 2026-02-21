# Proposal: Len Trait for Collection Length

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-02-05
**Approved:** 2026-02-18
**Affects:** Type system, prelude, standard library

---

## Executive Summary

This proposal formalizes the `Len` trait for types that have a countable size. The trait enables generic programming over collections and sequences, allowing functions like `len()` to work with any type that implements `Len`.

**Key features:**
1. **Len trait** with `.len() -> int` method
2. **Built-in implementations** for `[T]`, `str`, `{K: V}`, `Set<T>`, `Range<int>`, `(T₁, T₂, ...)`, `[T, max N]`
3. **Generic `len()` function** in prelude using `Len` bound

---

## Motivation

### Current State

The prelude defines `len` only for lists:

```ori
pub @len<T> (collection: [T]) -> int = collection.len()
```

The `.len()` method works on multiple types via built-in method dispatch:

```ori
[1, 2, 3].len()      // 3 - works
"hello".len()        // 5 - works
{"a": 1}.len()       // 1 - works
len(collection: [1, 2, 3])  // 3 - works
len(collection: "hello")    // ERROR: expected [T], found str
```

### Problem

1. **Inconsistency**: Method `.len()` works on strings, but function `len()` doesn't
2. **No generic programming**: Cannot write `@process<T: Len> (x: T) -> int`
3. **Missing specification**: No spec section defines the `Len` trait
4. **Test failures**: Tests reference non-existent `07-properties-of-types.md § Len Trait`

### Use Cases

| Type | `.len()` Returns |
|------|------------------|
| `[T]` | Number of elements |
| `str` | Number of bytes (not codepoints) |
| `{K: V}` | Number of key-value pairs |
| `Set<T>` | Number of elements |
| `Range<int>` | Number of values in range |
| `(T₁, T₂, ...)` | Number of elements |
| `[T, max N]` | Current length (0 to N) |

---

## Proposed Design

### Trait Definition

```ori
/// Trait for types with a countable length.
///
/// Implementations must return a non-negative integer representing
/// the number of elements, bytes, or items in the collection.
pub trait Len {
    /// Returns the length of this collection.
    ///
    /// For strings, returns byte count (not codepoint count).
    /// For ranges, returns the number of values in the range.
    @len (self) -> int
}
```

### Built-in Implementations

The compiler provides built-in implementations for core types:

| Type | Implementation |
|------|----------------|
| `[T]` | Number of elements |
| `str` | Number of bytes |
| `{K: V}` | Number of entries |
| `Set<T>` | Number of elements |
| `Range<int>` | `end - start` for exclusive, `end - start + 1` for inclusive |
| `(T₁, T₂, ...)` | Number of elements (statically known) |
| `[T, max N]` | Current element count |

These are implemented in the evaluator's method registry, not in Ori source.

### Updated Prelude Function

```ori
/// Get the length of any collection implementing Len.
pub @len<T: Len> (collection: T) -> int = collection.len()
```

### Generic Programming

With the `Len` trait, users can write generic functions:

```ori
@is_empty<T: Len> (x: T) -> bool = x.len() == 0

@at_least<T: Len> (x: T, n: int) -> bool = x.len() >= n

@process_if_nonempty<T: Len> (x: T, f: (T) -> void) -> void =
    if x.len() > 0 then f(x) else ()
```

### Distinction from Iterator.count()

The `Len` trait is distinct from `Iterator.count()`:

| | `Len.len()` | `Iterator.count()` |
|--|------------|-------------------|
| **Complexity** | O(1) for built-in types | O(n) — consumes the iterator |
| **Side effects** | None — non-consuming | Consuming — iterator is exhausted |
| **Semantics** | Current size of collection | Number of remaining elements |

Iterators do **not** implement `Len`. To count iterator elements, use `.count()`.

Collections that implement `Len` may also be iterable, but `len()` provides direct size access without iteration.

---

## Specification Text

Add to `07-properties-of-types.md` after the Default Trait section:

### Len Trait

The `Len` trait provides length/size information for collections.

```ori
trait Len {
    @len (self) -> int
}
```

#### Semantic Requirements

Implementations must satisfy:
- **Non-negative**: `x.len() >= 0` for all `x`
- **Deterministic**: `x.len()` returns the same value for unchanged `x`

#### String Length

For `str`, `.len()` returns the **byte count**, not the codepoint or grapheme count:

```ori
"hello".len()  // 5
"café".len()   // 5 (é is 2 bytes in UTF-8)
"日本".len()   // 6 (each character is 3 bytes)
```

For codepoint iteration, use `.chars()`. For grapheme clusters, use the `unicode` module.

#### Standard Implementations

| Type | Implements `Len` |
|------|------------------|
| `[T]` | Yes |
| `str` | Yes |
| `{K: V}` | Yes |
| `Set<T>` | Yes |
| `Range<int>` | Yes |
| `(T₁, T₂, ...)` | Yes |
| `[T, max N]` | Yes |

#### Derivation

`Len` cannot be derived. Types must implement it explicitly or be built-in.

---

## Implementation Plan

### Phase 1: Type Checker Support

1. Add `Len` to recognized built-in traits in V2 type checker
2. Implement `type_implements_len()` for `[T]`, `str`, `{K: V}`, `Set<T>`, `Range<int>`, `(T₁, T₂, ...)`, `[T, max N]`
3. Ensure trait bound `<T: Len>` resolves correctly

**Files:**
- `compiler/ori_types/src/check/bounds.rs` (or equivalent)
- `compiler/ori_types/src/registry/traits.rs`

### Phase 2: Prelude Update

1. Add `Len` trait declaration to prelude
2. Update `len` function to use `<T: Len>` bound

**Files:**
- `library/std/prelude.ori`

### Phase 3: Specification

1. Add `Len Trait` section to `07-properties-of-types.md`
2. Update test file comment to reference correct spec section

**Files:**
- `docs/ori_lang/0.1-alpha/spec/07-properties-of-types.md`
- `tests/spec/traits/core/len.ori`

### Phase 4: Enable Tests

1. Remove `#skip` attributes from `len.ori` tests
2. Verify all tests pass

---

## Alternatives Considered

### 1. Keep `len()` List-Only

**Rejected.** Creates inconsistency between method and function syntax. Users expect `len(x)` to work when `x.len()` works.

### 2. Overloaded Functions Instead of Trait

Define multiple `len` functions for each type.

**Rejected.** Doesn't enable generic programming. Cannot write `<T: Len>`.

### 3. Return `uint` Instead of `int`

**Rejected.** Ori doesn't have `uint`. Using `int` with non-negative semantic requirement matches other methods.

---

## Compatibility

- **Backward compatible**: Existing code using `.len()` methods continues to work
- **Existing `len([T])` calls**: Continue to work (more specific signature available)
- **New capability**: `len(str)` and `len({K: V})` now work

---

## Related Work

| Language | Approach |
|----------|----------|
| Rust | `len()` method, no trait (each type defines separately) |
| Go | `len()` built-in function, works on arrays, slices, maps, strings, channels |
| Python | `__len__` dunder method, `len()` built-in calls it |
| Haskell | `length` in `Foldable` typeclass |

Ori follows Python's model: a trait defines the capability, a function provides uniform syntax.

---

## Resolved Questions

1. **Should `Len` extend another trait?** **No.** `Len` stands alone with no supertrait. Requiring `Eq` would be unnecessary coupling — not all measurable types need equality.

2. **Range length for non-int ranges?** **`Range<int>` only for now.** `Range<char>` would require codepoint distance semantics, which is complex. Deferred to a future proposal.

3. **Channel length?** **No.** Channel buffer occupancy is inherently racy — the value is stale before it can be used. This violates the determinism semantic requirement. Deferred to a future `BufferedChannel` API if needed.

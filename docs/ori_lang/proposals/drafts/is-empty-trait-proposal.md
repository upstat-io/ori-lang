# Proposal: IsEmpty Trait for Collection Emptiness

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-05
**Affects:** Type system, prelude, standard library
**Related:** [Len Trait Proposal](len-trait-proposal.md)

---

## Executive Summary

This proposal formalizes the `IsEmpty` trait for types that can be checked for emptiness. The trait enables generic programming over collections, allowing functions like `is_empty()` to work with any type that implements `IsEmpty`.

**Key features:**
1. **IsEmpty trait** with `.is_empty() -> bool` method
2. **Built-in implementations** for `[T]`, `str`, `{K: V}`, `Set<T>`
3. **Generic `is_empty()` function** in prelude using `IsEmpty` bound

---

## Motivation

### Current State

The prelude defines `is_empty` only for lists:

```ori
pub @is_empty<T> (collection: [T]) -> bool = collection.len() == 0
```

The `.is_empty()` method works on multiple types via built-in method dispatch:

```ori
[].is_empty()           // true - works
"".is_empty()           // true - works
{}.is_empty()           // true - works
is_empty(collection: [])      // true - works
is_empty(collection: "")      // ERROR: expected [T], found str
```

### Problem

1. **Inconsistency**: Method `.is_empty()` works on strings, but function `is_empty()` doesn't
2. **No generic programming**: Cannot write `@process<T: IsEmpty> (x: T) -> bool`
3. **Missing specification**: No spec section defines the `IsEmpty` trait
4. **Test failures**: Tests reference non-existent `07-properties-of-types.md § IsEmpty Trait`

### Use Cases

| Type | `.is_empty()` Returns |
|------|----------------------|
| `[T]` | `true` if no elements |
| `str` | `true` if zero bytes |
| `{K: V}` | `true` if no entries |
| `Set<T>` | `true` if no elements |
| `[T, max N]` | `true` if current length is 0 |

---

## Proposed Design

### Trait Definition

```ori
/// Trait for types that can be checked for emptiness.
///
/// Implementations must return `true` if and only if the collection
/// contains no elements, bytes, or items.
pub trait IsEmpty {
    /// Returns true if this collection is empty.
    @is_empty (self) -> bool
}
```

### Relationship to Len

`IsEmpty` is independent of `Len`. While `is_empty()` can be implemented as `len() == 0`, this is not required:

- Some types may implement `IsEmpty` without `Len` (e.g., lazy iterators where counting is expensive)
- The semantic requirement is: `is_empty()` returns `true` iff the collection has no items

### Built-in Implementations

The compiler provides built-in implementations for core types:

| Type | Implementation |
|------|----------------|
| `[T]` | `true` if no elements |
| `str` | `true` if zero bytes |
| `{K: V}` | `true` if no entries |
| `Set<T>` | `true` if no elements |
| `[T, max N]` | `true` if length is 0 |

These are implemented in the evaluator's method registry, not in Ori source.

### Updated Prelude Function

```ori
/// Check if any collection implementing IsEmpty is empty.
pub @is_empty<T: IsEmpty> (collection: T) -> bool = collection.is_empty()
```

### Generic Programming

With the `IsEmpty` trait, users can write generic functions:

```ori
@require_non_empty<T: IsEmpty> (x: T) -> Result<T, str> =
    if x.is_empty() then Err("collection is empty") else Ok(x)

@first_or_default<T: IsEmpty + Len> (x: [T], default: T) -> T =
    if x.is_empty() then default else x[0]
```

---

## Specification Text

Add to `07-properties-of-types.md` after the Len Trait section:

### IsEmpty Trait

The `IsEmpty` trait provides emptiness checking for collections.

```ori
trait IsEmpty {
    @is_empty (self) -> bool
}
```

#### Semantic Requirements

Implementations must satisfy:
- **Consistency**: `is_empty()` returns `true` iff the collection has no items
- **Deterministic**: `x.is_empty()` returns the same value for unchanged `x`
- **Relationship to Len**: If a type implements both `Len` and `IsEmpty`, then `x.is_empty() == (x.len() == 0)`

#### Standard Implementations

| Type | Implements `IsEmpty` |
|------|---------------------|
| `[T]` | Yes |
| `str` | Yes |
| `{K: V}` | Yes |
| `Set<T>` | Yes |
| `[T, max N]` | Yes |

Note: `Range<T>` does not implement `IsEmpty` because ranges are never "empty" in the collection sense—they represent a mathematical interval.

#### Derivation

`IsEmpty` cannot be derived. Types must implement it explicitly or be built-in.

---

## Implementation Plan

### Phase 1: Type Checker Support

1. Add `IsEmpty` to recognized built-in traits in V2 type checker
2. Implement `type_implements_is_empty()` for `[T]`, `str`, `{K: V}`, `Set<T>`
3. Ensure trait bound `<T: IsEmpty>` resolves correctly

**Files:**
- `compiler/ori_types/src/check/bounds.rs` (or equivalent)
- `compiler/ori_types/src/registry/traits.rs`

### Phase 2: Prelude Update

1. Add `IsEmpty` trait declaration to prelude
2. Update `is_empty` function to use `<T: IsEmpty>` bound

**Files:**
- `library/std/prelude.ori`

### Phase 3: Specification

1. Add `IsEmpty Trait` section to `07-properties-of-types.md`
2. Update test file comment to reference correct spec section

**Files:**
- `docs/ori_lang/0.1-alpha/spec/07-properties-of-types.md`
- `tests/spec/traits/core/is_empty.ori`

### Phase 4: Enable Tests

1. Remove comments from `is_empty.ori` tests
2. Verify all tests pass

---

## Alternatives Considered

### 1. Keep `is_empty()` List-Only

**Rejected.** Creates inconsistency between method and function syntax.

### 2. Derive IsEmpty from Len

Make `IsEmpty` a supertrait requirement or blanket implementation from `Len`.

**Rejected.** Some types may want `IsEmpty` without the cost of `Len` (e.g., checking if a stream has items without counting all of them).

### 3. Combine with Len Trait

Have a single `Collection` trait with both `len()` and `is_empty()`.

**Rejected.** Less flexible. Types should be able to implement traits independently.

---

## Compatibility

- **Backward compatible**: Existing code using `.is_empty()` methods continues to work
- **Existing `is_empty([T])` calls**: Continue to work
- **New capability**: `is_empty(str)` and `is_empty({K: V})` now work

---

## Related Work

| Language | Approach |
|----------|----------|
| Rust | `is_empty()` method on each collection type, no unified trait |
| Go | No `is_empty`; use `len(x) == 0` |
| Python | `__bool__` / `__len__`; empty collections are falsy |
| Haskell | `null` function in `Foldable` |

---

## Checklist

- [ ] Specification text approved
- [ ] Type checker implementation complete
- [ ] Prelude updated
- [ ] Tests enabled and passing
- [ ] Documentation updated

# Bounds and Constraints

This document covers constraining generic types with trait bounds.

---

## Why Bounds?

Generic functions often need to use methods on their type parameters:

```ori
// ERROR: can't call to_string on arbitrary T
@print_all<T> (items: [T]) -> void =
    map(items, item -> print(item.to_string()))
```

Bounds specify what capabilities `T` must have:

```ori
// OK: T must be Printable
@print_all<T> (items: [T]) -> void where T: Printable =
    map(items, item -> print(item.to_string()))
```

---

## Where Clause Syntax

```ori
@function_name<T> (params...) -> ReturnType where T: Trait = ...
```

### Single Bound

```ori
@sort<T> (items: [T]) -> [T] where T: Comparable = ...

@hash_all<T> (items: [T]) -> [int] where T: Hashable =
    map(items, item -> item.hash())
```

### Multiple Bounds

Use `+` to require multiple traits:

```ori
@sort_and_print<T> (items: [T]) -> void where T: Comparable + Printable = run(
    let sorted = sort(items),
    map(sorted, item -> print(item.to_string())),
)
```

### Multiple Type Parameters

```ori
@convert<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default = ...

@merge<K, V> (a: {K: V}, b: {K: V}) -> {K: V}
    where K: Hashable + Eq, V: Clone = ...
```

---

## Bound Placement

### In Where Clause (Preferred)

```ori
@process<T> (value: T) -> str where T: Printable = value.to_string()
```

### Inline (Short Bounds)

```ori
@process<T: Printable> (value: T) -> str = value.to_string()
```

Both are equivalent. Use `where` for longer bounds.

---

## Common Bounds

### Eq — Equality

```ori
@contains<T> (items: [T], target: T) -> bool where T: Eq =
    fold(items, false, (found, item) -> found || item.equals(target))
```

### Comparable — Ordering

```ori
@max<T> (items: [T]) -> Option<T> where T: Comparable = ...

@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

### Hashable — Hashing

```ori
@to_set<T> (items: [T]) -> {T: bool} where T: Hashable + Eq = ...
```

### Clone — Copying

```ori
@duplicate<T> (item: T) -> [T] where T: Clone =
    [item.clone(), item.clone()]
```

### Printable — String Conversion

```ori
@describe<T> (value: T) -> str where T: Printable = value.to_string()
```

### Default — Default Values

```ori
@get_or_default<T> (opt: Option<T>) -> T where T: Default = match(opt,
    Some(value) -> value,
    None -> T.default(),
)
```

---

## Trait Inheritance in Bounds

When a trait requires another trait:

```ori
trait Comparable: Eq { ... }

// Comparable bound implies Eq
@find_max<T> (items: [T]) -> Option<T> where T: Comparable = ...
// Can use both equals() and compare() on T
```

---

## Associated Type Bounds

Constrain associated types:

```ori
@sum_items<I> (iter: I) -> int
    where I: Iterator, I.Item: Addable = ...

@print_items<I> (iter: I) -> void
    where I: Iterator, I.Item: Printable = ...
```

---

## Bounds on Implementations

### Conditional Implementation

```ori
// Only implement Printable for Option<T> when T is Printable
impl<T> Printable for Option<T> where T: Printable {
    @to_string (self) -> str = match(self,
        Some(value) -> "Some(" + value.to_string() + ")",
        None -> "None",
    )
}
```

### Multiple Conditional Implementations

```ori
// Different implementations based on bounds
impl<T> Summarize for [T] where T: Printable {
    @summarize (self) -> str = ...
}

impl<T> Summarize for [T] where T: Numeric {
    @summarize (self) -> str = "Sum: " + str(sum(self))
}
```

---

## Type Bounds on Structs

```ori
// T must be Hashable to be used in this struct
type Cache<T> where T: Hashable = {
    items: {T: Data},
    ...
}
```

---

## Negative Bounds

Ori doesn't support negative bounds (`where T: !Trait`). Design around positive constraints instead.

---

## Bound Errors

### Missing Bound

```
error[E0277]: the trait bound `T: Printable` is not satisfied
  --> src/main.ori:3:10
   |
 3 |     x.to_string()
   |       ^^^^^^^^^ T doesn't implement Printable
   |
   = help: add bound: where T: Printable
```

### Conflicting Bounds

```
error[E0277]: conflicting trait bounds
  --> src/main.ori:5:1
   |
 5 | where T: A + B
   |          ^^^^^ A and B have conflicting associated types
```

---

## Best Practices

### Use Minimal Bounds

```ori
// Good: only requires what's needed
@find<T> (items: [T], target: T) -> Option<int> where T: Eq = ...

// Bad: over-constrained
@find<T> (items: [T], target: T) -> Option<int>
    where T: Eq + Hashable + Clone + Printable = ...
```

### Document Bound Requirements

```ori
// #Sorts items in ascending order
// Requires T to be Comparable for ordering
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

### Prefer Trait Bounds Over Concrete Types

```ori
// Good: works with any Printable
@log<T> (value: T) -> void where T: Printable = print(value.to_string())

// Less flexible: only works with str
@log (message: str) -> void = print(message)
```

---

## See Also

- [Trait Definitions](01-trait-definitions.md)
- [Implementations](02-implementations.md)
- [Generics](../03-type-system/04-generics.md)

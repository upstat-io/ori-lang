# Bounds and Constraints

This document covers constraining generic types with trait bounds.

---

## Why Bounds?

Generic functions often need to use methods on their type parameters:

```sigil
// ERROR: can't call to_string on arbitrary T
@print_all<T> (items: [T]) -> void =
    map(items, x -> print(x.to_string()))
```

Bounds specify what capabilities `T` must have:

```sigil
// OK: T must be Printable
@print_all<T> (items: [T]) -> void where T: Printable =
    map(items, x -> print(x.to_string()))
```

---

## Where Clause Syntax

```sigil
@function_name<T> (params...) -> ReturnType where T: Trait = ...
```

### Single Bound

```sigil
@sort<T> (items: [T]) -> [T] where T: Comparable = ...

@hash_all<T> (items: [T]) -> [int] where T: Hashable =
    map(items, x -> x.hash())
```

### Multiple Bounds

Use `+` to require multiple traits:

```sigil
@sort_and_print<T> (items: [T]) -> void where T: Comparable + Printable = run(
    let sorted = sort(items),
    map(sorted, x -> print(x.to_string())),
)
```

### Multiple Type Parameters

```sigil
@convert<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default = ...

@merge<K, V> (a: {K: V}, b: {K: V}) -> {K: V}
    where K: Hashable + Eq, V: Clone = ...
```

---

## Bound Placement

### In Where Clause (Preferred)

```sigil
@process<T> (x: T) -> str where T: Printable = x.to_string()
```

### Inline (Short Bounds)

```sigil
@process<T: Printable> (x: T) -> str = x.to_string()
```

Both are equivalent. Use `where` for longer bounds.

---

## Common Bounds

### Eq — Equality

```sigil
@contains<T> (items: [T], target: T) -> bool where T: Eq =
    fold(items, false, (found, x) -> found || x.equals(target))
```

### Comparable — Ordering

```sigil
@max<T> (items: [T]) -> Option<T> where T: Comparable = ...

@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

### Hashable — Hashing

```sigil
@to_set<T> (items: [T]) -> {T: bool} where T: Hashable + Eq = ...
```

### Clone — Copying

```sigil
@duplicate<T> (item: T) -> [T] where T: Clone =
    [item.clone(), item.clone()]
```

### Printable — String Conversion

```sigil
@describe<T> (x: T) -> str where T: Printable = x.to_string()
```

### Default — Default Values

```sigil
@get_or_default<T> (opt: Option<T>) -> T where T: Default = match(opt,
    Some(x) -> x,
    None -> T.default(),
)
```

---

## Trait Inheritance in Bounds

When a trait requires another trait:

```sigil
trait Comparable: Eq { ... }

// Comparable bound implies Eq
@find_max<T> (items: [T]) -> Option<T> where T: Comparable = ...
// Can use both equals() and compare() on T
```

---

## Associated Type Bounds

Constrain associated types:

```sigil
@sum_items<I> (iter: I) -> int
    where I: Iterator, I.Item: Addable = ...

@print_items<I> (iter: I) -> void
    where I: Iterator, I.Item: Printable = ...
```

---

## Bounds on Implementations

### Conditional Implementation

```sigil
// Only implement Printable for Option<T> when T is Printable
impl<T> Printable for Option<T> where T: Printable {
    @to_string (self) -> str = match(self,
        Some(x) -> "Some(" + x.to_string() + ")",
        None -> "None",
    )
}
```

### Multiple Conditional Implementations

```sigil
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

```sigil
// T must be Hashable to be used in this struct
type Cache<T> where T: Hashable = {
    items: {T: Data},
    ...
}
```

---

## Negative Bounds

Sigil doesn't support negative bounds (`where T: !Trait`). Design around positive constraints instead.

---

## Bound Errors

### Missing Bound

```
error[E0277]: the trait bound `T: Printable` is not satisfied
  --> src/main.si:3:10
   |
 3 |     x.to_string()
   |       ^^^^^^^^^ T doesn't implement Printable
   |
   = help: add bound: where T: Printable
```

### Conflicting Bounds

```
error[E0277]: conflicting trait bounds
  --> src/main.si:5:1
   |
 5 | where T: A + B
   |          ^^^^^ A and B have conflicting associated types
```

---

## Best Practices

### Use Minimal Bounds

```sigil
// Good: only requires what's needed
@find<T> (items: [T], target: T) -> Option<int> where T: Eq = ...

// Bad: over-constrained
@find<T> (items: [T], target: T) -> Option<int>
    where T: Eq + Hashable + Clone + Printable = ...
```

### Document Bound Requirements

```sigil
// #Sorts items in ascending order
// Requires T to be Comparable for ordering
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

### Prefer Trait Bounds Over Concrete Types

```sigil
// Good: works with any Printable
@log<T> (x: T) -> void where T: Printable = print(x.to_string())

// Less flexible: only works with str
@log (x: str) -> void = print(x)
```

---

## See Also

- [Trait Definitions](01-trait-definitions.md)
- [Implementations](02-implementations.md)
- [Generics](../03-type-system/04-generics.md)

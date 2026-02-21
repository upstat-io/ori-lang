# Proposal: Ordering Type

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, comparison

---

## Summary

This proposal formalizes the `Ordering` type, including its variants, use with `Comparable`, and common operations.

---

## Problem Statement

The spec mentions `Ordering` in the prelude but leaves unclear:

1. **Variants**: What are the exact variant names?
2. **Operations**: What operations does Ordering support?
3. **Comparable**: How does Ordering relate to Comparable?
4. **Chaining**: How to chain comparisons?
5. **Reversal**: How to reverse an ordering?

---

## Definition

```ori
type Ordering = Less | Equal | Greater
```

`Ordering` represents the result of comparing two values.

---

## Variants

| Variant | Meaning | Numeric Equivalent |
|---------|---------|-------------------|
| `Less` | Left is less than right | -1 |
| `Equal` | Left equals right | 0 |
| `Greater` | Left is greater than right | 1 |

---

## Construction

### Via compare Function

```ori
compare(left: 1, right: 2)  // Less
compare(left: 2, right: 2)  // Equal
compare(left: 3, right: 2)  // Greater
```

### Via Comparable Trait

```ori
1.compare(other: 2)  // Less
```

### Direct Construction

```ori
let ord = Less
let ord = Equal
let ord = Greater
```

---

## Comparable Trait

```ori
trait Comparable {
    @compare (self, other: Self) -> Ordering
}
```

Types implementing `Comparable` can be ordered:

```ori
impl Comparable for int {
    @compare (self, other: int) -> Ordering =
        if self < other then Less
        else if self > other then Greater
        else Equal
}
```

---

## Operations

### is_* Methods

```ori
impl Ordering {
    @is_less (self) -> bool = match self { Less -> true, _ -> false}
    @is_equal (self) -> bool = match self { Equal -> true, _ -> false}
    @is_greater (self) -> bool = match self { Greater -> true, _ -> false}
    @is_less_or_equal (self) -> bool = match self { Greater -> false, _ -> true}
    @is_greater_or_equal (self) -> bool = match self { Less -> false, _ -> true}
}
```

```ori
compare(left: 1, right: 2).is_less()  // true
compare(left: 2, right: 2).is_equal() // true
```

### reverse

```ori
impl Ordering {
    @reverse (self) -> Ordering = match self {
        Less -> Greater
        Equal -> Equal
        Greater -> Less
    }
}
```

```ori
Less.reverse()    // Greater
Equal.reverse()   // Equal
Greater.reverse() // Less
```

### then

Chain comparisons (used for lexicographic ordering):

```ori
impl Ordering {
    @then (self, other: Ordering) -> Ordering = match self {
        Equal -> other
        result -> result
    }
}
```

```ori
// Compare (1, 2) with (1, 3)
compare(left: 1, right: 1)
    .then(other: compare(left: 2, right: 3))
// Equal.then(Less) = Less
```

### then_with

Lazy version of `then`:

```ori
impl Ordering {
    @then_with (self, f: () -> Ordering) -> Ordering = match self {
        Equal -> f()
        result -> result
    }
}
```

```ori
compare(left: a.first, right: b.first)
    .then_with(f: () -> compare(left: a.second, right: b.second))
// Only evaluates second comparison if first is Equal
```

---

## Traits Implemented

### Eq

```ori
Less == Less      // true
Less == Equal     // false
Equal == Greater  // false
```

### Comparable

Orderings themselves are comparable:

```ori
Less < Equal     // true
Equal < Greater  // true
Less < Greater   // true
```

Order: `Less < Equal < Greater`

### Clone

```ori
let a = Less
let b = a.clone()
```

### Debug and Printable

```ori
Less.debug()    // "Less"
Equal.to_str()  // "Equal"
Greater.debug() // "Greater"
```

### Hashable

Can be used as map key:

```ori
let counts: {Ordering: int} = {Less: 0, Equal: 0, Greater: 0}
```

### Default

```ori
Ordering.default()  // Equal
```

---

## Common Patterns

### Lexicographic Comparison

```ori
#derive(Eq)
type Point = { x: int, y: int }

impl Comparable for Point {
    @compare (self, other: Point) -> Ordering =
        compare(left: self.x, right: other.x)
            .then(other: compare(left: self.y, right: other.y))
}
```

### Multi-Field Comparison

```ori
impl Comparable for Person {
    @compare (self, other: Person) -> Ordering =
        compare(left: self.last_name, right: other.last_name)
            .then_with(f: () -> compare(left: self.first_name, right: other.first_name))
            .then_with(f: () -> compare(left: self.age, right: other.age))
}
```

### Sorting

```ori
@sort<T: Comparable> (items: [T]) -> [T] = ...

let sorted = sort(items: [3, 1, 4, 1, 5])  // [1, 1, 3, 4, 5]
```

### Custom Sort Order

```ori
@sort_by<T> (items: [T], compare: (T, T) -> Ordering) -> [T] = ...

// Sort descending
let desc = sort_by(
    items: numbers,
    compare: (a, b) -> compare(left: a, right: b).reverse(),
)
```

---

## Relationship to Operators

| Operator | Uses | Result |
|----------|------|--------|
| `<` | `compare().is_less()` | `bool` |
| `<=` | `compare().is_less_or_equal()` | `bool` |
| `>` | `compare().is_greater()` | `bool` |
| `>=` | `compare().is_greater_or_equal()` | `bool` |
| `==` | `Eq.equals()` | `bool` |
| `!=` | `!Eq.equals()` | `bool` |

Types implementing `Comparable` automatically get comparison operators.

---

## Derived Comparable

For derivation rules, see the [derived-traits-proposal](../approved/derived-traits-proposal.md). When `Comparable` is derived, fields are compared in declaration order using lexicographic comparison.

---

## Error Messages

### Not Comparable

```
error[E0930]: `MyType` does not implement `Comparable`
  --> src/main.ori:5:1
   |
 5 | compare(left: a, right: b)
   | ^^^^^^^^^^^^^^^^^^^^^^^^^^ trait not implemented
   |
   = help: implement `Comparable` for `MyType`
   = help: or derive it: `#derive(Comparable)`
```

---

## Spec Changes Required

### Update `06-types.md`

Add Ordering to Built-in Types section with:
1. Variant definitions
2. Method signatures
3. Trait implementations

### Update `07-properties-of-types.md`

Document Comparable trait and relationship to Ordering.

---

## Summary

| Aspect | Details |
|--------|---------|
| Type | `type Ordering = Less \| Equal \| Greater` |
| Purpose | Represents comparison result |
| Key methods | `is_less`, `is_equal`, `is_greater`, `reverse`, `then`, `then_with` |
| Traits | Eq, Comparable, Clone, Debug, Printable, Hashable, Default |
| Default | `Equal` |
| Comparable order | `Less < Equal < Greater` |

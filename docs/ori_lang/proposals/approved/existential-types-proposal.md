# Proposal: Existential Types (impl Trait)

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Compiler, type system, API design

---

## Summary

This proposal formalizes the `impl Trait` syntax for existential types in Ori. Existential types allow functions to return opaque types that satisfy trait bounds without exposing the concrete type to callers.

```ori
@make_iterator (items: [int]) -> impl Iterator where Item == int =
    items.iter()

// Caller sees: impl Iterator where Item == int
// Caller cannot access concrete type (ListIterator<int>)
let iter = make_iterator(items: [1, 2, 3])
for x in iter do print(msg: `{x}`)  // Works via Iterator trait
```

---

## Problem Statement

Ori's trait system requires explicit type parameters for polymorphism:

```ori
@process<I: Iterator> (iter: I) -> [I.Item] where I.Item == int =
    iter.collect()
```

This works for inputs but creates friction for outputs:

1. **Type leakage**: Returning concrete types exposes implementation details
2. **Verbose signatures**: Complex iterator chains yield unwieldy types like `MapIterator<FilterIterator<TakeIterator<...>>>`
3. **API fragility**: Changing the implementation changes the return type, breaking callers
4. **Unnecessary coupling**: Callers only need trait methods, not concrete type access

### Current State

The spec already uses `impl Trait` in the Iterator traits section:

```ori
trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}

trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```

However, this syntax has never been formally specified with:
- Semantic rules
- Valid positions
- Type inference behavior
- Error handling
- Comparison to trait objects

---

## Design

### Syntax

`impl Trait` appears in return position with optional trait bounds:

```ori
// Single trait
@numbers () -> impl Iterator where Item == int = [1, 2, 3].iter()

// Multiple traits with +
@clonable_numbers () -> impl Iterator + Clone where Item == int =
    [1, 2, 3].iter()

// Associated type constraints
@string_keys () -> impl Iterator where Item == (str, int) =
    {"a": 1, "b": 2}.iter()
```

### Grammar

The `impl Trait` type is a type expression with its own optional `where` clause for associated type constraints:

```ebnf
impl_trait_type = "impl" trait_bounds [ impl_where_clause ] .
trait_bounds    = type_path { "+" type_path } .
impl_where_clause = "where" assoc_constraint { "," assoc_constraint } .
assoc_constraint  = identifier "==" type .
```

The `where` clause on an `impl Trait` type constrains associated types of the trait(s), not generic type parameters. This is distinct from the function-level `where` clause.

### Constraint Syntax

Associated type constraints use a type-local `where` clause:

```ori
// Constrain Iterator's Item associated type
@int_iterator () -> impl Iterator where Item == int

// Multiple constraints (trait + associated type)
@bounded_ints () -> impl Iterator + Clone where Item == int
```

### Semantics

#### Opaque Type

The return type is _opaque_ to callers. The compiler knows the concrete type internally but callers only see the trait interface:

```ori
@make_iter () -> impl Iterator where Item == int = [1, 2, 3].iter()

let iter = make_iter()
iter.next()           // OK: Iterator method
iter.list             // Error: cannot access concrete type's fields
```

#### Single Concrete Type Requirement

All return paths must return the same concrete type:

```ori
// OK: all paths return ListIterator<int>
@numbers (flag: bool) -> impl Iterator where Item == int =
    if flag then [1, 2, 3].iter()
    else [4, 5, 6].iter()

// Error: different concrete types
@bad_numbers (flag: bool) -> impl Iterator where Item == int =
    if flag then
        [1, 2, 3].iter()       // ListIterator<int>
    else
        (1..10).iter()         // RangeIterator<int>
    // error: impl Trait requires all return paths to have the same concrete type
```

#### Trait Bound Satisfaction

The concrete type must implement all specified traits:

```ori
// OK: ListIterator implements Iterator and Clone
@clonable () -> impl Iterator + Clone where Item == int =
    [1, 2, 3].iter()

// Error: RangeIterator may not implement Clone
@bad_clone () -> impl Iterator + Clone where Item == int =
    (1..10).iter()  // error if RangeIterator doesn't implement Clone
```

### Type Inference

The concrete type is inferred from the function body:

```ori
@numbers () -> impl Iterator where Item == int =
    [1, 2, 3].iter()  // Inferred: ListIterator<int>

// Return type is impl Iterator where Item == int
// Internal type is ListIterator<int>
```

Type inference rules:

1. Infer concrete type from return expressions
2. Unify all return paths to same concrete type
3. Verify trait bounds satisfied by concrete type
4. Expose only trait interface to callers

---

## Valid Positions

### Return Position (Supported)

`impl Trait` is valid only in function return position:

```ori
// Function return
@make_iter () -> impl Iterator where Item == int = ...

// Method return
impl MyCollection {
    @iter (self) -> impl Iterator where Item == T = ...
}

// Trait method return (with restrictions, see below)
trait Iterable {
    @iter (self) -> impl Iterator where Item == Self.Item
}
```

### Argument Position (Not Supported)

`impl Trait` is not allowed in argument position. Use generic parameters instead:

```ori
// Error: impl Trait not allowed in argument position
@process (iter: impl Iterator where Item == int) -> int = ...

// Correct: use generic parameter
@process<I: Iterator> (iter: I) -> int where I.Item == int = ...
```

**Rationale**: Argument-position `impl Trait` creates ambiguity about whether each call site can pass different types (universal) or must pass the same type (existential). Generics make this explicit.

> **Note:** The Iterator Traits proposal originally showed `impl Iterator` in the `Collect` trait parameter position. This has been updated to use generics: `@from_iter<I: Iterator>(iter: I) -> Self where I.Item == T`.

### Struct Fields (Not Supported)

`impl Trait` is not allowed in struct fields. Use generic parameters:

```ori
// Error: impl Trait not allowed in struct field
type Container = {
    iter: impl Iterator where Item == int,
}

// Correct: use generic parameter
type Container<I: Iterator> = {
    iter: I,
} where I.Item == int
```

**Rationale**: Struct fields require known sizes at compile time. `impl Trait` hides the concrete type, making size computation impossible without boxing.

### Trait Definitions (Allowed with Constraints)

`impl Trait` in trait method returns is allowed but creates specific behavior:

```ori
trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}
```

Each implementor provides its own concrete type. The caller sees `impl Iterator` and can only use Iterator methods:

```ori
impl Iterable for [T] {
    type Item = T
    @iter (self) -> impl Iterator where Item == T = ListIterator { ... }
}

impl Iterable for Range<int> {
    type Item = int
    @iter (self) -> impl Iterator where Item == int = RangeIterator { ... }
}
```

---

## Comparison: impl Trait vs Trait Objects

| Aspect | `impl Trait` | Trait Object |
|--------|--------------|--------------|
| Dispatch | Static (monomorphized) | Dynamic (vtable) |
| Size | Concrete type size | Pointer size |
| Performance | Better (inlinable) | Vtable overhead |
| Type knowledge | Known at compile time | Erased at runtime |
| Flexibility | One concrete type per function | Any type at runtime |
| Object safety | All traits | Object-safe traits only |
| Recursion | Cannot (infinite type) | Can (via indirection) |

### When to Use Each

Use `impl Trait` when:
- Single concrete type returned
- Performance matters
- Hiding implementation details
- Simplifying complex type signatures

Use trait objects when:
- Multiple concrete types possible at runtime
- Dynamic dispatch required
- Runtime polymorphism needed
- Breaking recursive types

```ori
// impl Trait: single concrete type, best performance
@fast_iterator () -> impl Iterator where Item == int =
    [1, 2, 3].iter()

// Trait object: multiple types possible
@any_iterator (flag: bool) -> Iterator where Item == int =
    if flag then [1, 2, 3].iter()
    else (1..10).iter()
```

---

## Error Messages

### Different Concrete Types

```
error[E0XXX]: `impl Trait` requires all return paths to have the same concrete type
  --> src/main.ori:5:9
   |
 3 |     if flag then
 4 |         [1, 2, 3].iter()
   |         --------------- this returns `ListIterator<int>`
 5 |     else
 6 |         (1..10).iter()
   |         -------------- this returns `RangeIterator<int>`
   |
   = help: use a trait object if you need to return different types:
           `-> Iterator where Item == int`
```

### Invalid Position

```
error[E0XXX]: `impl Trait` is only allowed in return position
  --> src/main.ori:1:14
   |
 1 | @foo (x: impl Iterator) -> void
   |          ^^^^^^^^^^^^^ `impl Trait` not allowed here
   |
   = help: use a generic parameter instead:
           `@foo<I: Iterator> (x: I) -> void`
```

### Unsatisfied Trait Bound

```
error[E0XXX]: the trait bound `Clone` is not satisfied
  --> src/main.ori:2:5
   |
 1 | @make () -> impl Iterator + Clone where Item == int =
   |                             ----- required by this bound
 2 |     (1..10).iter()
   |     ^^^^^^^^^^^^^^ `RangeIterator<int>` does not implement `Clone`
```

---

## Examples

### Iterator Combinators

```ori
// Clean API hiding complex iterator types
@map<I: Iterator, U> (iter: I, f: (I.Item) -> U) -> impl Iterator where Item == U =
    MapIterator { inner: iter, transform: f }

@filter<I: Iterator> (iter: I, pred: (I.Item) -> bool) -> impl Iterator where Item == I.Item =
    FilterIterator { inner: iter, predicate: pred }

@take<I: Iterator> (iter: I, n: int) -> impl Iterator where Item == I.Item =
    TakeIterator { inner: iter, remaining: n }

// Composable usage - caller doesn't see MapIterator<FilterIterator<...>>
@first_10_even_squares () -> impl Iterator where Item == int =
    (1..100).iter()
        .filter(predicate: n -> n % 2 == 0)
        .map(transform: n -> n * n)
        .take(count: 10)
```

### Builder Pattern

```ori
type QueryBuilder = { /* internal state */ }

impl QueryBuilder {
    @new () -> QueryBuilder = ...

    @select (self, cols: [str]) -> QueryBuilder = ...

    @where_clause (self, cond: str) -> QueryBuilder = ...

    // Return opaque result type
    @execute (self) -> impl Iterator where Item == Row = run(
        let results = execute_query(builder: self),
        results.iter()
    )
}

// Usage - clean API, hidden implementation
let rows = QueryBuilder.new()
    .select(cols: ["name", "age"])
    .where_clause(cond: "age > 21")
    .execute()

for row in rows do
    print(msg: `{row.name}: {row.age}`)
```

### Capability-Constrained Resources

```ori
@make_reader (path: str) -> impl Iterator where Item == str uses FileSystem =
    File.open(path: path).lines()

// Caller gets iterator without knowing internal file handle type
let lines = make_reader(path: "data.txt")
for line in lines do process(line: line)
```

---

## Implementation Notes

### Parser Changes

Add `impl Trait` as a type variant:

```ebnf
type_expr = ... | impl_trait_type .
impl_trait_type = "impl" trait_bounds [ impl_where_clause ] .
trait_bounds = type_path { "+" type_path } .
impl_where_clause = "where" assoc_constraint { "," assoc_constraint } .
assoc_constraint = identifier "==" type .
```

Note: The `impl_where_clause` is distinct from the function-level `where_clause`. It constrains associated types of the traits in the bounds, not generic parameters.

### AST Representation

Add `ExistentialType` variant to `ParsedType`:

```rust
pub enum ParsedType {
    // ... existing variants ...
    ExistentialType {
        bounds: Vec<TraitBound>,
        where_clause: Option<WhereClause>,
    },
}
```

### Type Checker

1. Identify `impl Trait` return types
2. Infer concrete type from function body
3. Unify all return paths to same concrete type
4. Verify trait bounds satisfied
5. Record mapping from opaque to concrete type

### Code Generation

`impl Trait` is erased at runtime. The concrete type is used directly:

```ori
// Source
@numbers () -> impl Iterator where Item == int = [1, 2, 3].iter()

// Generated (conceptually)
@numbers () -> ListIterator<int> = [1, 2, 3].iter()
```

---

## Future Extensions

### Argument Position (Potential)

A future proposal may introduce argument-position `impl Trait` as syntactic sugar for generics:

```ori
// Potential future syntax
@process (iter: impl Iterator where Item == int) -> int

// Equivalent to
@process<I: Iterator> (iter: I) -> int where I.Item == int
```

This would require careful design to avoid universal/existential ambiguity. For now, use explicit generic parameters in argument position.

### Type Alias impl Trait (TAIT)

A future proposal may allow naming existential types:

```ori
// Potential future syntax
type IntIter = impl Iterator where Item == int

@numbers () -> IntIter = [1, 2, 3].iter()
@more_numbers () -> IntIter = [4, 5, 6].iter()  // Must be same concrete type
```

---

## Spec Changes Required

### `06-types.md`

Add new section "Existential Types" covering:
- `impl Trait` syntax
- Semantics and opaqueness
- Valid positions
- Type inference rules
- Comparison to trait objects

### `grammar.ebnf`

Add productions for:
- `impl_trait_type`
- `trait_bounds`
- Integration with return types

### `CLAUDE.md`

Update Types section to include:
- `impl Trait` syntax
- Valid positions summary
- Trait bound syntax

---

## Summary

| Aspect | Decision |
|--------|----------|
| Syntax | `impl Trait where Assoc == Type` |
| Position | Return only (initially) |
| Multiple traits | `impl A + B` with `+` |
| Associated types | `where Item == int` clause syntax |
| Inference | Per-function, from body |
| Concrete type | Single type across all return paths |
| Dispatch | Static (monomorphized) |
| Trait objects | Use for runtime polymorphism |
| Argument position | Not supported (use generics) |
| Struct fields | Not supported (use generics) |

This proposal formalizes existential types as a first-class feature in Ori, enabling clean APIs that hide implementation details while maintaining static dispatch performance.

# Generics

This document covers generic types and functions in Sigil.

---

## Generic Syntax

Sigil uses angle brackets for type parameters:

```sigil
// single type parameter
<T>
// multiple type parameters
<T, U>
// key/value convention
<K, V>
// value/error convention
<T, E>
```

---

## Generic Types

### Definition

```sigil
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
type Pair<T> = { first: T, second: T }
type KeyValue<K, V> = { key: K, value: V }
```

### Usage

```sigil
// Type inference from construction
// Option<int>
let opt = Some(42)
// Pair<int>
let pair = Pair { first: 1, second: 2 }

// Explicit type annotation
let opt: Option<str> = None
let result: Result<int, Error> = Ok(42)
```

### Nested Generics

```sigil
// Option containing a list
let opt_list: Option<[int]> = Some([1, 2, 3])

// List of results
let results: [Result<int, str>] = [Ok(1), Err("fail"), Ok(3)]

// Map with generic value type
let cache: {str: Option<Data>} = {}
```

---

## Generic Functions

### Definition

```sigil
@identity<T> (x: T) -> T = x

@swap<T, U> (pair: (T, U)) -> (U, T) = (pair.1, pair.0)

@first<T> (items: [T]) -> Option<T> =
    if items.is_empty() then None
    else Some(items[0])
```

### Type Parameter Position

Type parameters come after the function name, before parameters:

```sigil
@function_name<T, U> (param: T, ...) -> U = ...
```

### Calling Generic Functions

```sigil
// Type inference (common)
// inferred: identity<int>
let result = identity(42)
// inferred: swap<int, str>
let swapped = swap((1, "hello"))

// Explicit type arguments (when needed)
let result = identity<int>(42)
// need to specify T for empty list
let none = first<str>([])
```

---

## Type Constraints

Use `where` clauses to constrain type parameters. Constraints specify what traits a type parameter must implement.

### Why Constraints?

Without constraints, generic type parameters can only be used in limited ways:

```sigil
// ERROR: can't call to_string on arbitrary T
@describe<T> (x: T) -> str = x.to_string()

// OK: constraint specifies T must be Printable
@describe<T> (x: T) -> str where T: Printable = x.to_string()
```

### Single Constraint

```sigil
@sort<T> (items: [T]) -> [T] where T: Comparable =
    // can use comparison operations on T
    ...

@hash_all<T> (items: [T]) -> [int] where T: Hashable =
    map(
        .over: items,
        .transform: item -> item.hash(),
    )
```

### Multiple Constraints on One Type

Use `+` to require multiple traits:

```sigil
@print_sorted<T> (items: [T]) -> void where T: Comparable + Printable = run(
    let sorted = sort(items),
    map(
        .over: sorted,
        .transform: item -> print(item.to_string()),
    ),
)

// For map keys, typically need both Hashable and Eq
@to_map<K, V> (pairs: [(K, V)]) -> {K: V} where K: Hashable + Eq = ...
```

### Multiple Type Parameters

Separate constraints with commas:

```sigil
@convert_all<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default =
    map(
        .over: items,
        .transform: item -> f(item.clone()),
    )

@merge<K, V> (a: {K: V}, b: {K: V}) -> {K: V}
    where K: Hashable + Eq, V: Clone = ...
```

### Inline Constraint Syntax

For simple constraints, use inline syntax:

```sigil
// Inline (short form)
@describe<T: Printable> (x: T) -> str = x.to_string()

// Where clause (equivalent)
@describe<T> (x: T) -> str where T: Printable = x.to_string()
```

Use `where` for multiple or complex constraints.

### Common Built-in Constraints

| Trait | Provides | Common Use |
|-------|----------|------------|
| `Eq` | `equals()`, `not_equals()` | Equality comparison |
| `Comparable` | `compare()`, `<`, `>`, etc. | Ordering, sorting |
| `Hashable` | `hash()` | Map keys, set elements |
| `Clone` | `clone()` | Deep copying |
| `Printable` | `to_string()` | String conversion |
| `Default` | `default()` | Default values |

See [Bounds and Constraints](../04-traits/03-bounds-and-constraints.md) for complete documentation on trait bounds.

---

## Common Generic Patterns

### Higher-Order Functions

```sigil
@map<T, U> (items: [T], f: (T) -> U) -> [U] = ...

@filter<T> (items: [T], pred: (T) -> bool) -> [T] = ...

@fold<T, U> (items: [T], init: U, f: (U, T) -> U) -> U = ...
```

### Container Operations

```sigil
@unwrap_or<T> (opt: Option<T>, default: T) -> T = match(opt,
    Some(value) -> value,
    None -> default
)

@map_result<T, U, E> (result: Result<T, E>, transform: (T) -> U) -> Result<U, E> = match(result,
    Ok(value) -> Ok(transform(value)),
    Err(error) -> Err(error)
)
```

### Builder Pattern

```sigil
type Builder<T> = {
    value: T,
    transforms: [(T) -> T]
}

@build<T> (initial: T) -> Builder<T> =
    Builder { value: initial, transforms: [] }

@with_transform<T> (builder: Builder<T>, transform: (T) -> T) -> Builder<T> =
    Builder { value: builder.value, transforms: builder.transforms + [transform] }

@execute<T> (builder: Builder<T>) -> T =
    fold(builder.transforms, builder.value, (current, transform) -> transform(current))
```

---

## Associated Types

Traits can have associated types:

```sigil
trait Iterator {
    type Item

    @next (self) -> Option<Self.Item>
}

impl Iterator for Range {
    type Item = int

    @next (self) -> Option<int> = ...
}
```

Using associated types in bounds:

```sigil
@collect<I> (iter: I) -> [I.Item] where I: Iterator = ...
```

---

## Monomorphization

Generic functions are monomorphized—specialized versions are generated for each type:

```sigil
// Definition
@identity<T> (x: T) -> T = x

// Usage
// generates identity_int
identity(42)
// generates identity_str
identity("hello")
// generates identity_bool
identity(true)
```

This provides:
- No runtime overhead
- Type-specific optimizations
- Larger binary size (trade-off)

---

## Generic Best Practices

### Use Descriptive Type Parameters

```sigil
// Good: meaningful names
@map<Input, Output> (items: [Input], f: (Input) -> Output) -> [Output]

// OK: conventional single letters
@map<T, U> (items: [T], f: (T) -> U) -> [U]

// Avoid: meaningless names
@map<A, B> (items: [A], f: (A) -> B) -> [B]
```

### Constrain When Necessary

```sigil
// Too permissive: will fail if T isn't printable
@print_all<T> (items: [T]) -> void =
    map(items, x -> print(x.to_string()))

// Better: constraint makes requirements explicit
@print_all<T> (items: [T]) -> void where T: Printable =
    map(items, x -> print(x.to_string()))
```

### Prefer Concrete Types When Possible

```sigil
// Only use generics when truly needed
// Good
@add_ints (a: int, b: int) -> int = a + b

// Not everything needs to be generic
// Overkill for just ints
@add<T> (a: T, b: T) -> T where T: Addable
```

---

## Limitations

### No Inference of Type Parameters in Definitions

```sigil
// Must explicitly declare type parameters
// Required
@identity<T> (x: T) -> T = x

// Can't infer from usage
// ERROR: missing type annotations
@identity (x) = x
```

### No Higher-Kinded Types

```sigil
// NOT supported: abstracting over type constructors
// ERROR
@lift<F, T> (x: T) -> F<T>

// Work around with specific types
@lift_option<T> (x: T) -> Option<T> = Some(x)
@lift_result<T, E> (x: T) -> Result<T, E> = Ok(x)
```

---

## See Also

- [User-Defined Types](03-user-defined-types.md)
- [Traits](../04-traits/index.md)
- [Type Inference](05-type-inference.md)

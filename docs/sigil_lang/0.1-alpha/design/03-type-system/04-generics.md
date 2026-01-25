# Generics

This document covers generic types and functions in Sigil.

---

## Generic Syntax

Sigil uses angle brackets for type parameters:

```sigil
<T>        // single type parameter
<T, U>     // multiple type parameters
<K, V>     // key/value convention
<T, E>     // value/error convention
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
let opt = Some(42)           // Option<int>
let pair = Pair { first: 1, second: 2 }  // Pair<int>

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
let result = identity(42)        // inferred: identity<int>
let swapped = swap((1, "hello")) // inferred: swap<int, str>

// Explicit type arguments (when needed)
let result = identity<int>(42)
let none = first<str>([])        // need to specify T for empty list
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
        .transform: x -> x.hash(),
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
    Some(x) -> x,
    None -> default
)

@map_result<T, U, E> (r: Result<T, E>, f: (T) -> U) -> Result<U, E> = match(r,
    Ok(x) -> Ok(f(x)),
    Err(e) -> Err(e)
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

@with_transform<T> (b: Builder<T>, f: (T) -> T) -> Builder<T> =
    Builder { value: b.value, transforms: b.transforms + [f] }

@execute<T> (b: Builder<T>) -> T =
    fold(b.transforms, b.value, (val, f) -> f(val))
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
identity(42)       // generates identity_int
identity("hello")  // generates identity_str
identity(true)     // generates identity_bool
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
@add_ints (a: int, b: int) -> int = a + b  // Good

// Not everything needs to be generic
@add<T> (a: T, b: T) -> T where T: Addable  // Overkill for just ints
```

---

## Limitations

### No Inference of Type Parameters in Definitions

```sigil
// Must explicitly declare type parameters
@identity<T> (x: T) -> T = x  // Required

// Can't infer from usage
@identity (x) = x  // ERROR: missing type annotations
```

### No Higher-Kinded Types

```sigil
// NOT supported: abstracting over type constructors
@lift<F, T> (x: T) -> F<T>  // ERROR

// Work around with specific types
@lift_option<T> (x: T) -> Option<T> = Some(x)
@lift_result<T, E> (x: T) -> Result<T, E> = Ok(x)
```

---

## See Also

- [User-Defined Types](03-user-defined-types.md)
- [Traits](../04-traits/index.md)
- [Type Inference](05-type-inference.md)

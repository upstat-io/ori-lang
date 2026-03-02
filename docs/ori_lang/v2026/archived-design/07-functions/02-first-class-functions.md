# First-Class Functions

This document covers functions as first-class values in Ori, including function types, referencing functions, and function equality semantics.

---

## Overview

Functions in Ori are first-class values. This means functions can be:

- Assigned to variables
- Passed as arguments to other functions
- Returned from functions
- Stored in data structures

```ori
@double (number: int) -> int = number * 2

@main () -> void = run(
    // assign to variable
    let f = double,
    // call via variable: 10
    let result = f(5),
    let mapped = map(
        .over: [1, 2, 3],
        // pass to higher-order pattern
        .transform: double,
    ),
)
```

---

## Why First-Class Functions?

First-class functions are essential for Ori's design:

### Patterns Require Them

The pattern system depends on passing functions as arguments:

```ori
@double_all (items: [int]) -> [int] = map(
    .over: items,
    .transform: double,
)

@keep_positive (items: [int]) -> [int] = filter(
    .over: items,
    .predicate: item -> item > 0,
)

@sum (items: [int]) -> int = fold(
    .over: items,
    .initial: 0,
    .operation: add,
)
```

### Enables Composition

Functions can be combined and transformed:

```ori
@make_adder (amount: int) -> (int) -> int = value -> value + amount

@compose<A, B, C> (outer: (B) -> C, inner: (A) -> B) -> (A) -> C =
    value -> outer(inner(value))

@main () -> void = run(
    let add5 = make_adder(5),
    let add10 = make_adder(10),
    let double_then_add5 = compose(
        .outer: add5,
        .inner: double,
    ),
    // (3 * 2) + 5 = 11
    let result = double_then_add5(3),
)
```

### Simple Mental Model

The rule is simple: functions are values, like anything else. No special cases.

---

## Function Type Syntax

Function types use arrow syntax that mirrors function definitions.

### Basic Syntax

```ori
(parameter_types) -> return_type
```

### Examples

```ori
// One parameter
// takes int, returns int
(int) -> int
// takes str, returns bool
(str) -> bool

// Multiple parameters
// takes two ints, returns int
(int, int) -> int
// takes three params, returns str
(str, int, bool) -> str

// No parameters
// takes nothing, returns int
() -> int
// takes nothing, returns nothing
() -> void

// Void return
// takes int, returns nothing
(int) -> void
```

### Type Aliases

Use `type` to create named function types:

```ori
type Transform = (int) -> int
type Predicate = (int) -> bool
type BinaryOp = (int, int) -> int
type Comparator<T> = (T, T) -> Ordering
type EventHandler = (Event) -> void
```

### Using Type Aliases

```ori
type Transform = (int) -> int

@apply_twice (transform: Transform, value: int) -> int = transform(transform(value))

@main () -> void = run(
    // double(double(5)) = 20
    let doubled = apply_twice(
        .transform: double,
        .value: 5,
    ),
    print(doubled),
)
```

---

## Higher-Order Function Types

Functions that take or return functions have nested arrow types.

### Functions Returning Functions

```ori
// A function that takes int and returns a function (int) -> int
(int) -> (int) -> int
```

This is right-associative, so it parses as:

```ori
(int) -> ((int) -> int)
```

### Example: Curried Add

```ori
@curried_add (first: int) -> (int) -> int = second -> first + second

@main () -> void = run(
    let add5: (int) -> int = curried_add(5),
    // 15
    let result = add5(10),
)
```

### Functions Taking Functions

```ori
// A function that takes a predicate and returns filtered list
([int], (int) -> bool) -> [int]
```

### Example: Filter

```ori
@filter_with (items: [int], predicate: (int) -> bool) -> [int] = filter(
    .over: items,
    .predicate: predicate,
)

@main () -> void = run(
    // [2, 4]
    let evens = filter_with(
        .items: [1, 2, 3, 4],
        .predicate: item -> item % 2 == 0,
    ),
    print(evens),
)
```

### Complex Function Types

```ori
// Function that takes a binary operation and returns a reducer
((int, int) -> int) -> ([int]) -> int

// Example usage
@make_reducer (operation: (int, int) -> int) -> ([int]) -> int =
    items -> fold(
        .over: items,
        .initial: 0,
        .operation: operation,
    )

@main () -> void = run(
    let sum_reducer = make_reducer(
        .operation: (left, right) -> left + right,
    ),
    // 10
    let result = sum_reducer([1, 2, 3, 4]),
    print(result),
)
```

---

## Referencing Named Functions

### Bare Name Reference

To reference a named function without calling it, use just the name:

```ori
@double (number: int) -> int = number * 2

@main () -> void = run(
    // reference, not call
    let transform = double,
    // now call via variable
    let result = transform(5),
)
```

### Context Distinguishes Reference from Call

The context makes it clear:

```ori
// reference (bare name)
double
// call (with parentheses)
double(5)
```

### Passing to Higher-Order Patterns

When passing to patterns like `map`, use the bare name as the argument:

```ori
@double (number: int) -> int = number * 2
@is_positive (number: int) -> bool = number > 0
@add (left: int, right: int) -> int = left + right

@main () -> void = run(
    // [2, 4, 6]
    let doubled = map(
        .over: [1, 2, 3],
        .transform: double,
    ),
    // [1]
    let positive = filter(
        .over: [-1, 0, 1],
        .predicate: is_positive,
    ),
    // 6
    let sum = fold(
        .over: [1, 2, 3],
        .initial: 0,
        .operation: add,
    ),
)
```

### Why No Ori for References?

The `@` ori is only used in function definitions. References use bare names because:

1. **Simplicity** — `double` is simpler than `@double`
2. **Consistency** — Same syntax as calling: `double` vs `double(5)`
3. **Pattern compatibility** — `map(items, double)` reads naturally
4. **Context clarity** — No ambiguity about what `double` refers to

---

## Storing Functions

### In Variables

```ori
@double (number: int) -> int = number * 2

@main () -> void = run(
    // Inferred type
    let transform = double,

    // Explicit type annotation
    let another: (int) -> int = double,

    // Both work the same
    // 10
    print(transform(5)),
    // 10
    print(another(5)),
)
```

### In Structs

```ori
type Processor = {
    transform: (int) -> int,
    validate: (int) -> bool
}

@double (number: int) -> int = number * 2
@is_positive (number: int) -> bool = number > 0

@main () -> void = run(
    let proc = Processor {
        transform: double,
        validate: is_positive,
    },
    // 10
    let processed = proc.transform(5),
    // false
    let valid = proc.validate(-1),
)
```

### In Lists

```ori
type Transform = (int) -> int

@double (number: int) -> int = number * 2
@square (number: int) -> int = number * number
@negate (number: int) -> int = -number

@apply_all (transforms: [Transform], value: int) -> [int] =
    map(
        .over: transforms,
        .transform: transform -> transform(value),
    )

@main () -> void = run(
    let transforms = [double, square, negate],
    // [6, 9, -3]
    let results = apply_all(transforms, 3),
    print(results),
)
```

### In Maps

```ori
type Handler = (Request) -> Response

@handle_get (req: Request) -> Response = ...
@handle_post (req: Request) -> Response = ...
@handle_delete (req: Request) -> Response = ...

@main () -> void = run(
    let handlers: {str: Handler} = {
        "GET": handle_get,
        "POST": handle_post,
        "DELETE": handle_delete,
    },
    let handler = handlers["GET"] ?? handle_not_found,
    let response = handler(request),
)
```

---

## Function Equality

### Functions Are Not Comparable

Functions cannot be compared for equality:

```ori
@double (number: int) -> int = number * 2

@main () -> void = run(
    let first = double,
    let second = double,
    // ERROR: functions cannot be compared
    let result = first == second,
)
```

### Why No Function Equality?

Function equality is philosophically problematic:

1. **Reference equality is surprising**

   ```ori
   // Would these be equal?
   first = value -> value + 1
   second = value -> value + 1
   // Same behavior, but different closures — should first == second?
   ```

2. **Structural equality is undecidable**

   Determining if two functions produce the same output for all inputs is equivalent to the halting problem.

3. **Behavioral equality is impractical**

   Testing all possible inputs is impossible for most types.

### Working Without Function Equality

If you need to identify functions, use wrapper types:

```ori
type NamedTransform = {
    name: str,
    transform: (int) -> int
}

@main () -> void = run(
    let t1 = NamedTransform { name: "double", transform: value -> value * 2 },
    let t2 = NamedTransform { name: "double", transform: value -> value * 2 },
    // Compare by name
    let same = t1.name == t2.name,
)
```

Or use enums to represent a fixed set of operations:

```ori
type Operation = Double | Square | Negate

@apply (operation: Operation, number: int) -> int = match(operation,
    Double -> number * 2,
    Square -> number * number,
    Negate -> -number,
)

@main () -> void = run(
    let op1 = Double,
    let op2 = Double,
    // true - enums are comparable
    let same = op1 == op2,
    let result = apply(op1, 5),
)
```

---

## Generic Function Types

### Type Parameters in Function Types

```ori
// Generic identity function type
type Identity<T> = (T) -> T

// Generic predicate
type Predicate<T> = (T) -> bool

// Generic binary operation
type BinOp<T> = (T, T) -> T

// Generic transformation
type Mapper<T, U> = (T) -> U
```

### Using Generic Function Types

```ori
type Mapper<T, U> = (T) -> U

@apply_mapper<T, U> (items: [T], mapper: Mapper<T, U>) -> [U] =
    map(
        .over: items,
        .transform: mapper,
    )

@main () -> void = run(
    let lengths: [int] = apply_mapper(
        .items: ["a", "bb", "ccc"],
        .mapper: text -> text.len(),
    ),
    // [1, 2, 3]
    print(lengths),
)
```

### Higher-Kinded Function Types

```ori
// Function that transforms a list
type ListTransformer<T, U> = ([T]) -> [U]

// Function that transforms an Option
type OptionMapper<T, U> = (Option<T>, (T) -> U) -> Option<U>
```

---

## Operators as Functions

Some operators can be used as function values:

### Arithmetic Operators

```ori
@sum (items: [int]) -> int = fold(
    .over: items,
    .initial: 0,
    .operation: +,
)
@product (items: [int]) -> int = fold(
    .over: items,
    .initial: 1,
    .operation: *,
)
```

### Comparison Operators

```ori
// Using operators in fold
@all_equal (items: [int]) -> bool = fold(
    .over: items,
    .initial: true,
    .operation: ==,
)
```

### When Operators Can Be Functions

Operators work as functions when:
- They're binary operators (`+`, `-`, `*`, `/`, `==`, etc.)
- Used in a context expecting `(T, T) -> U`

```ori
// These work because fold expects (accumulator, item) -> accumulator
// (int, int) -> int
fold(
    .over: [1, 2, 3],
    .initial: 0,
    .operation: +,
)
// (int, int) -> int
fold(
    .over: [1, 2, 3],
    .initial: 1,
    .operation: *,
)
```

---

## Type Inference with Functions

### Inference from Context

Function types can often be inferred:

```ori
@main () -> void = run(
    // transform inferred as (int) -> int
    let transform = double,

    // Type inferred from usage
    // (int) -> int when used with ints
    let doubler = value -> value * 2,

    // Explicit when needed
    let explicit: (int) -> int = value -> value * 2,
)
```

### When Explicit Types Help

```ori
// Ambiguous: what's the type of value?
// ERROR: cannot infer type
transform = value -> value

// Explicit type resolves ambiguity
// OK
transform: (int) -> int = value -> value
```

### Inference in Higher-Order Functions

Types flow through higher-order functions:

```ori
@main () -> void = run(
    // map infers that item is int from the list type
    let doubled = map(
        .over: [1, 2, 3],
        .transform: item -> item * 2,
    ),

    // filter infers that text is str from the list type
    let long_names = filter(
        .over: ["a", "abc", "ab"],
        .predicate: text -> text.len() > 1,
    ),
)
```

---

## Common Patterns

### Function Composition

```ori
@compose<A, B, C> (outer: (B) -> C, inner: (A) -> B) -> (A) -> C =
    value -> outer(inner(value))

@double (number: int) -> int = number * 2
@add_one (number: int) -> int = number + 1

@main () -> void = run(
    let double_then_add = compose(
        .outer: add_one,
        .inner: double,
    ),
    // (5 * 2) + 1 = 11
    let result = double_then_add(5),
)
```

### Function Pipelines

```ori
@pipe<A, B, C> (value: A, first: (A) -> B, second: (B) -> C) -> C = second(first(value))

@main () -> void = run(
    // (5 * 2) + 1 = 11
    let result = pipe(
        .value: 5,
        .first: double,
        .second: add_one,
    ),
)
```

### Conditional Function Selection

```ori
@choose_transform (use_double: bool) -> (int) -> int =
    if use_double then double else square

@main () -> void = run(
    let transform = choose_transform(true),
    // 10
    let result = transform(5),
)
```

### Function Registry

```ori
type Command = { name: str, handler: ([str]) -> Result<void, Error> }

@execute (commands: [Command], name: str, args: [str]) -> Result<void, Error> =
    match(
        filter(
            .over: commands,
            .predicate: command -> command.name == name,
        ),
        [] -> Err(CommandNotFound(name)),
        [cmd, ..] -> cmd.handler(args),
    )
```

---

## Best Practices

### Use Descriptive Type Aliases

```ori
// Good: clear intent
type Validator = (Input) -> Result<Valid, [Error]>
type EventHandler = (Event) -> void
type Comparator<T> = (T, T) -> Ordering

// Avoid: raw function types everywhere
@process (f: (Input) -> Result<Valid, [Error]>) -> ...
```

### Keep Function Signatures Simple

```ori
// Good: clear and focused
@transform (items: [int], transform: (int) -> int) -> [int]

// Avoid: too many function parameters
@process (items: [int], mapper: (int) -> int, predicate: (int) -> bool, reducer: (int, int) -> int) -> [int]

// Better: use a struct
type Transforms = {
    mapper: (int) -> int,
    predicate: (int) -> bool,
    reducer: (int, int) -> int
}
@process (items: [int], transforms: Transforms) -> [int]
```

### Document Function Parameters

```ori
// #Applies a transformation to each item
// >transform([1,2,3], item -> item * 2) -> [2,4,6]
// !The transform function must be pure
@transform<T, U> (items: [T], transform: (T) -> U) -> [U] = map(
    .over: items,
    .transform: transform,
)
```

### Prefer Named Functions for Reuse

```ori
// Good: reusable
@is_positive (number: int) -> bool = number > 0
positive = filter(
    .over: items,
    .predicate: is_positive,
)

// OK for one-off: inline lambda
positive = filter(
    .over: items,
    .predicate: item -> item > 0,
)
```

---

## Summary

| Concept | Syntax |
|---------|--------|
| Function type | `(params) -> return` |
| Type alias | `type Name = (int) -> int` |
| Reference function | `transform = function_name` |
| Call via variable | `transform(args)` |
| Multi-param type | `(int, str) -> bool` |
| No-param type | `() -> int` |
| Void return type | `(int) -> void` |
| Curried type | `(int) -> (int) -> int` |
| Function equality | Not supported |

---

## See Also

- [Function Definitions](01-function-definitions.md)
- [Lambdas](03-lambdas.md)
- [Higher-Order Functions](04-higher-order.md)
- [Type Inference](../03-type-system/05-type-inference.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)

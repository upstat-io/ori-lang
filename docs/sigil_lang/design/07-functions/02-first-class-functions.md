# First-Class Functions

This document covers functions as first-class values in Sigil, including function types, referencing functions, and function equality semantics.

---

## Overview

Functions in Sigil are first-class values. This means functions can be:

- Assigned to variables
- Passed as arguments to other functions
- Returned from functions
- Stored in data structures

```sigil
@double (n: int) -> int = n * 2

@main () -> void = run(
    f = double,                    // assign to variable
    result = f(5),                 // call via variable: 10
    mapped = map([1, 2, 3], double) // pass to higher-order function
)
```

---

## Why First-Class Functions?

First-class functions are essential for Sigil's design:

### Patterns Require Them

The pattern system depends on passing functions as arguments:

```sigil
@double_all (items: [int]) -> [int] = map(items, double)

@keep_positive (items: [int]) -> [int] = filter(items, x -> x > 0)

@sum (items: [int]) -> int = fold(items, 0, add)
```

### Enables Composition

Functions can be combined and transformed:

```sigil
@make_adder (n: int) -> (int) -> int = x -> x + n

@compose<A, B, C> (f: (B) -> C, g: (A) -> B) -> (A) -> C =
    x -> f(g(x))

@main () -> void = run(
    add5 = make_adder(5),
    add10 = make_adder(10),
    double_then_add5 = compose(add5, double),
    result = double_then_add5(3)  // (3 * 2) + 5 = 11
)
```

### Simple Mental Model

The rule is simple: functions are values, like anything else. No special cases.

---

## Function Type Syntax

Function types use arrow syntax that mirrors function definitions.

### Basic Syntax

```sigil
(parameter_types) -> return_type
```

### Examples

```sigil
// One parameter
(int) -> int                     // takes int, returns int
(str) -> bool                    // takes str, returns bool

// Multiple parameters
(int, int) -> int                // takes two ints, returns int
(str, int, bool) -> str          // takes three params, returns str

// No parameters
() -> int                        // takes nothing, returns int
() -> void                       // takes nothing, returns nothing

// Void return
(int) -> void                    // takes int, returns nothing
```

### Type Aliases

Use `type` to create named function types:

```sigil
type Transform = (int) -> int
type Predicate = (int) -> bool
type BinaryOp = (int, int) -> int
type Comparator<T> = (T, T) -> Ordering
type EventHandler = (Event) -> void
```

### Using Type Aliases

```sigil
type Transform = (int) -> int

@apply_twice (f: Transform, x: int) -> int = f(f(x))

@main () -> void = run(
    doubled = apply_twice(double, 5),  // double(double(5)) = 20
    print(doubled)
)
```

---

## Higher-Order Function Types

Functions that take or return functions have nested arrow types.

### Functions Returning Functions

```sigil
// A function that takes int and returns a function (int) -> int
(int) -> (int) -> int
```

This is right-associative, so it parses as:

```sigil
(int) -> ((int) -> int)
```

### Example: Curried Add

```sigil
@curried_add (a: int) -> (int) -> int = b -> a + b

@main () -> void = run(
    add5: (int) -> int = curried_add(5),
    result = add5(10)  // 15
)
```

### Functions Taking Functions

```sigil
// A function that takes a predicate and returns filtered list
([int], (int) -> bool) -> [int]
```

### Example: Filter

```sigil
@filter_with (items: [int], pred: (int) -> bool) -> [int] =
    filter(items, pred)

@main () -> void = run(
    evens = filter_with([1, 2, 3, 4], x -> x % 2 == 0),  // [2, 4]
    print(evens)
)
```

### Complex Function Types

```sigil
// Function that takes a binary operation and returns a reducer
((int, int) -> int) -> ([int]) -> int

// Example usage
@make_reducer (op: (int, int) -> int) -> ([int]) -> int =
    items -> fold(items, 0, op)

@main () -> void = run(
    sum_reducer = make_reducer((a, b) -> a + b),
    result = sum_reducer([1, 2, 3, 4]),  // 10
    print(result)
)
```

---

## Referencing Named Functions

### Bare Name Reference

To reference a named function without calling it, use just the name:

```sigil
@double (n: int) -> int = n * 2

@main () -> void = run(
    f = double,      // reference, not call
    result = f(5)    // now call via variable
)
```

### Context Distinguishes Reference from Call

The context makes it clear:

```sigil
double        // reference (bare name)
double(5)     // call (with parentheses)
```

### Passing to Higher-Order Functions

When passing to patterns like `map`, use the bare name:

```sigil
@double (n: int) -> int = n * 2
@is_positive (n: int) -> bool = n > 0
@add (a: int, b: int) -> int = a + b

@main () -> void = run(
    doubled = map([1, 2, 3], double),           // [2, 4, 6]
    positive = filter([-1, 0, 1], is_positive), // [1]
    sum = fold([1, 2, 3], 0, add)               // 6
)
```

### Why No Sigil for References?

The `@` sigil is only used in function definitions. References use bare names because:

1. **Simplicity** — `double` is simpler than `@double`
2. **Consistency** — Same syntax as calling: `double` vs `double(5)`
3. **Pattern compatibility** — `map(items, double)` reads naturally
4. **Context clarity** — No ambiguity about what `double` refers to

---

## Storing Functions

### In Variables

```sigil
@double (n: int) -> int = n * 2

@main () -> void = run(
    // Inferred type
    f = double,

    // Explicit type annotation
    g: (int) -> int = double,

    // Both work the same
    print(f(5)),   // 10
    print(g(5))    // 10
)
```

### In Structs

```sigil
type Processor = {
    transform: (int) -> int,
    validate: (int) -> bool
}

@double (n: int) -> int = n * 2
@is_positive (n: int) -> bool = n > 0

@main () -> void = run(
    proc = Processor {
        transform: double,
        validate: is_positive
    },
    processed = proc.transform(5),  // 10
    valid = proc.validate(-1)       // false
)
```

### In Lists

```sigil
type Transform = (int) -> int

@double (n: int) -> int = n * 2
@square (n: int) -> int = n * n
@negate (n: int) -> int = -n

@apply_all (transforms: [Transform], value: int) -> [int] =
    map(transforms, t -> t(value))

@main () -> void = run(
    transforms = [double, square, negate],
    results = apply_all(transforms, 3),  // [6, 9, -3]
    print(results)
)
```

### In Maps

```sigil
type Handler = (Request) -> Response

@handle_get (req: Request) -> Response = ...
@handle_post (req: Request) -> Response = ...
@handle_delete (req: Request) -> Response = ...

@main () -> void = run(
    handlers: {str: Handler} = {
        "GET": handle_get,
        "POST": handle_post,
        "DELETE": handle_delete
    },
    handler = handlers["GET"] ?? handle_not_found,
    response = handler(request)
)
```

---

## Function Equality

### Functions Are Not Comparable

Functions cannot be compared for equality:

```sigil
@double (n: int) -> int = n * 2

@main () -> void = run(
    f = double,
    g = double,
    result = f == g   // ERROR: functions cannot be compared
)
```

### Why No Function Equality?

Function equality is philosophically problematic:

1. **Reference equality is surprising**

   ```sigil
   // Would these be equal?
   f = x -> x + 1
   g = x -> x + 1
   // Same behavior, but different closures — should f == g?
   ```

2. **Structural equality is undecidable**

   Determining if two functions produce the same output for all inputs is equivalent to the halting problem.

3. **Behavioral equality is impractical**

   Testing all possible inputs is impossible for most types.

### Working Without Function Equality

If you need to identify functions, use wrapper types:

```sigil
type NamedTransform = {
    name: str,
    transform: (int) -> int
}

@main () -> void = run(
    t1 = NamedTransform { name: "double", transform: x -> x * 2 },
    t2 = NamedTransform { name: "double", transform: x -> x * 2 },
    same = t1.name == t2.name   // Compare by name
)
```

Or use enums to represent a fixed set of operations:

```sigil
type Operation = Double | Square | Negate

@apply (op: Operation, n: int) -> int = match(op,
    Double -> n * 2,
    Square -> n * n,
    Negate -> -n
)

@main () -> void = run(
    op1 = Double,
    op2 = Double,
    same = op1 == op2,  // true - enums are comparable
    result = apply(op1, 5)
)
```

---

## Generic Function Types

### Type Parameters in Function Types

```sigil
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

```sigil
type Mapper<T, U> = (T) -> U

@apply_mapper<T, U> (items: [T], f: Mapper<T, U>) -> [U] =
    map(items, f)

@main () -> void = run(
    lengths: [int] = apply_mapper(["a", "bb", "ccc"], s -> s.len),
    print(lengths)  // [1, 2, 3]
)
```

### Higher-Kinded Function Types

```sigil
// Function that transforms a list
type ListTransformer<T, U> = ([T]) -> [U]

// Function that transforms an Option
type OptionMapper<T, U> = (Option<T>, (T) -> U) -> Option<U>
```

---

## Operators as Functions

Some operators can be used as function values:

### Arithmetic Operators

```sigil
@sum (items: [int]) -> int = fold(items, 0, +)
@product (items: [int]) -> int = fold(items, 1, *)
```

### Comparison Operators

```sigil
// Using operators in fold
@all_equal (items: [int]) -> bool = fold(items, true, ==)
```

### When Operators Can Be Functions

Operators work as functions when:
- They're binary operators (`+`, `-`, `*`, `/`, `==`, etc.)
- Used in a context expecting `(T, T) -> U`

```sigil
// These work because fold expects (acc, item) -> acc
fold([1, 2, 3], 0, +)   // (int, int) -> int
fold([1, 2, 3], 1, *)   // (int, int) -> int
```

---

## Type Inference with Functions

### Inference from Context

Function types can often be inferred:

```sigil
@main () -> void = run(
    // f inferred as (int) -> int
    f = double,

    // Type inferred from usage
    g = x -> x * 2,        // (int) -> int when used with ints

    // Explicit when needed
    h: (int) -> int = x -> x * 2
)
```

### When Explicit Types Help

```sigil
// Ambiguous: what's the type of x?
f = x -> x   // ERROR: cannot infer type

// Explicit type resolves ambiguity
f: (int) -> int = x -> x   // OK
```

### Inference in Higher-Order Functions

Types flow through higher-order functions:

```sigil
@main () -> void = run(
    // map infers that x is int from the list type
    doubled = map([1, 2, 3], x -> x * 2),

    // filter infers that x is str from the list type
    long_names = filter(["a", "abc", "ab"], x -> x.len > 1)
)
```

---

## Common Patterns

### Function Composition

```sigil
@compose<A, B, C> (f: (B) -> C, g: (A) -> B) -> (A) -> C =
    x -> f(g(x))

@double (n: int) -> int = n * 2
@add_one (n: int) -> int = n + 1

@main () -> void = run(
    double_then_add = compose(add_one, double),
    result = double_then_add(5)   // (5 * 2) + 1 = 11
)
```

### Function Pipelines

```sigil
@pipe<A, B, C> (x: A, f: (A) -> B, g: (B) -> C) -> C = g(f(x))

@main () -> void = run(
    result = pipe(5, double, add_one)  // (5 * 2) + 1 = 11
)
```

### Conditional Function Selection

```sigil
@choose_transform (use_double: bool) -> (int) -> int =
    if use_double then double else square

@main () -> void = run(
    transform = choose_transform(true),
    result = transform(5)   // 10
)
```

### Function Registry

```sigil
type Command = { name: str, handler: ([str]) -> Result<void, Error> }

@execute (commands: [Command], name: str, args: [str]) -> Result<void, Error> =
    match(filter(commands, c -> c.name == name),
        [] -> Err(CommandNotFound(name)),
        [cmd, ..] -> cmd.handler(args)
    )
```

---

## Best Practices

### Use Descriptive Type Aliases

```sigil
// Good: clear intent
type Validator = (Input) -> Result<Valid, [Error]>
type EventHandler = (Event) -> void
type Comparator<T> = (T, T) -> Ordering

// Avoid: raw function types everywhere
@process (f: (Input) -> Result<Valid, [Error]>) -> ...
```

### Keep Function Signatures Simple

```sigil
// Good: clear and focused
@transform (items: [int], f: (int) -> int) -> [int]

// Avoid: too many function parameters
@process (items: [int], f1: (int) -> int, f2: (int) -> bool, f3: (int, int) -> int) -> [int]

// Better: use a struct
type Transforms = {
    mapper: (int) -> int,
    predicate: (int) -> bool,
    reducer: (int, int) -> int
}
@process (items: [int], transforms: Transforms) -> [int]
```

### Document Function Parameters

```sigil
// #Applies a transformation to each item
// >transform([1,2,3], x -> x * 2) -> [2,4,6]
// !The transform function must be pure
@transform<T, U> (items: [T], f: (T) -> U) -> [U] = map(items, f)
```

### Prefer Named Functions for Reuse

```sigil
// Good: reusable
@is_positive (n: int) -> bool = n > 0
positive = filter(items, is_positive)

// OK for one-off: inline lambda
positive = filter(items, x -> x > 0)
```

---

## Summary

| Concept | Syntax |
|---------|--------|
| Function type | `(params) -> return` |
| Type alias | `type Name = (int) -> int` |
| Reference function | `f = function_name` |
| Call via variable | `f(args)` |
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

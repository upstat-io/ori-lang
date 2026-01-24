# Type Inference

This document covers Sigil's type inference rules and when explicit type annotations are required.

---

## Inference Philosophy

Sigil uses bidirectional type inference:

- **Explicit at boundaries** — Function signatures always require types
- **Inferred internally** — Types within functions are inferred
- **Push down, pull up** — Expected types flow down, inferred types flow up

---

## What's Inferred

### Local Variables

```sigil
@process (user: User) -> str = run(
    // inferred: str
    let name = user.name,
    // inferred: str
    let upper = name.upper(),
    // inferred: [str]
    let parts = upper.split(" "),
    // inferred: str
    parts.first() ?? "Anonymous",
)
```

### Lambda Parameters

```sigil
// Parameter types inferred from context
let doubled = map(
    .over: [1, 2, 3],
    // item: int inferred
    .transform: item -> item * 2,
)

let filtered = filter(
    .over: users,
    // user: User inferred
    .predicate: user -> user.age >= 18,
)
```

### Generic Type Arguments

```sigil
// Type arguments inferred from usage
// identity<int> inferred
let result = identity(42)
// Option<str> inferred
let opt = Some("hello")
```

### Collection Element Types

```sigil
// [int] inferred
let numbers = [1, 2, 3]
// [str] inferred
let names = ["a", "b", "c"]
// [(int, str)] inferred
let pairs = [(1, "a"), (2, "b")]
```

---

## What Requires Annotation

### Function Parameters

```sigil
// REQUIRED: parameter types
@add (left: int, right: int) -> int = left + right

// ERROR: missing parameter types
@add (left, right) -> int = left + right
```

### Function Return Types

```sigil
// REQUIRED: return type
@multiply (left: int, right: int) -> int = left * right

// ERROR: missing return type
@multiply (left: int, right: int) = left * right
```

### Empty Collections

```sigil
// Type cannot be inferred from empty literal
let empty: [int] = []
let no_values: {str: int} = {}

// OK if context provides type
@process (items: [int]) -> void = ...
// [int] inferred from parameter
process([])
```

### Ambiguous Generics

```sigil
// None could be Option<anything>
let none: Option<int> = None

// Empty result needs type
let empty: Result<int, str> = Err("empty")
```

---

## Inference Flow

### Top-Down (Expected Types)

Expected types flow from annotations down into expressions:

```sigil
// Return type str flows down
@describe (number: int) -> str =
    if number > 0 then "positive"
    else if number < 0 then "negative"
    else "zero"
    // All branches must be str
```

### Bottom-Up (Inferred Types)

Inferred types flow from expressions up to variables:

```sigil
@process () -> int = run(
    // x's type comes from compute()'s return type
    let x = compute(),
    // y: int (from x: int)
    let y = x * 2,
    y + 10,
)
```

### Bidirectional

Both directions work together:

```sigil
@transform (items: [int]) -> [str] =
    map(items, item -> str(item))
    // 1. items: [int] -> item: int (bottom-up from parameter)
    // 2. return [str] -> map returns [str] -> lambda returns str (top-down)
```

---

## Type Widening

Sigil does NOT widen types:

```sigil
// Each branch must have exact same type
// OK: both int
@get_value (cond: bool) -> int =
    if cond then 42 else 0

// ERROR: mismatched types
// int vs float
@bad (cond: bool) -> int =
    if cond then 42 else 3.14
```

---

## The `_` Type Hole

Use `_` to let the compiler infer and show you the type:

```sigil
@debug () -> void = run(
    let result: _ = complex_expression(),
    // Compiler tells you: "inferred type: SomeComplexType"
    print(result),
)
```

This is useful for:
- Understanding complex type expressions
- Documentation while developing
- Verifying expected types

---

## Generic Inference

### Function Calls

```sigil
// Type parameters inferred from arguments
// T = int
identity(42)
// T = str
identity("hello")

// T = int, U = str
swap((1, "a"))
```

### When Inference Fails

```sigil
// Can't infer T from empty list
// ERROR: cannot infer type parameter T
first([])

// Solution: explicit type
// OK: Option<str>
first<str>([])

// Or: provide context
// OK: T inferred from expected type
let result: Option<str> = first([])
```

### Method Chains

```sigil
// Types flow through chains
let result = items
    // still [int]
    .filter(item -> item > 0)
    // now [str]
    .map(item -> str(item))
    // now str
    .fold("", (accumulator, item) -> accumulator + ", " + item)
```

---

## Common Patterns

### Let the Return Type Guide

```sigil
// Return type guides internal inference
@parse_numbers (input: str) -> [int] = run(
    // [str]
    let parts = input.split(","),
    // must be [int] to match return
    map(parts, part -> int(part.trim())),
)
```

### Use Explicit Types for Clarity

```sigil
// Sometimes explicit is clearer even when inferable
@process () -> Result<Data, Error> = run(
    let response: HttpResponse = fetch(url),
    let data: Data = parse(response.body),
    Ok(data),
)
```

### Type Annotations on Complex Expressions

```sigil
// Help readers (and the compiler) understand complex code
@transform (input: Input) -> Output = run(
    let intermediate: IntermediateType = complex_computation(input),
    let final_result: Output = finalize(intermediate),
    final_result,
)
```

---

## Inference Errors

### Type Mismatch

```
error[E0308]: mismatched types
  --> src/main.si:5:10
   |
 5 |     x + "hello"
   |         ^^^^^^^ expected int, found str
```

### Cannot Infer

```
error[E0282]: cannot infer type
  --> src/main.si:3:5
   |
 3 |     empty = []
   |     ^^^^^ cannot infer element type
   |
   = help: add type annotation: empty: [int] = []
```

### Conflicting Requirements

```
error[E0308]: conflicting types
  --> src/main.si:4:5
   |
 4 |     if cond then 42 else "hello"
   |                  ^^       ^^^^^^^ str
   |                  |
   |                  int
   |
   = note: if and else branches must have same type
```

---

## See Also

- [Primitive Types](01-primitive-types.md)
- [Generics](04-generics.md)
- [Compositional Model](06-compositional-model.md)

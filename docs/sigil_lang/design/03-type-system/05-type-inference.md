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
    name = user.name,           // inferred: str
    upper = name.upper(),       // inferred: str
    parts = upper.split(" "),   // inferred: [str]
    parts.first() ?? "Anonymous" // inferred: str
)
```

### Lambda Parameters

```sigil
// Parameter types inferred from context
doubled = map([1, 2, 3], x -> x * 2)  // x: int inferred

filtered = filter(users, u -> u.age >= 18)  // u: User inferred
```

### Generic Type Arguments

```sigil
// Type arguments inferred from usage
result = identity(42)  // identity<int> inferred
opt = Some("hello")    // Option<str> inferred
```

### Collection Element Types

```sigil
numbers = [1, 2, 3]        // [int] inferred
names = ["a", "b", "c"]    // [str] inferred
pairs = [(1, "a"), (2, "b")]  // [(int, str)] inferred
```

---

## What Requires Annotation

### Function Parameters

```sigil
// REQUIRED: parameter types
@add (a: int, b: int) -> int = a + b

// ERROR: missing parameter types
@add (a, b) -> int = a + b
```

### Function Return Types

```sigil
// REQUIRED: return type
@multiply (a: int, b: int) -> int = a * b

// ERROR: missing return type
@multiply (a: int, b: int) = a * b
```

### Empty Collections

```sigil
// Type cannot be inferred from empty literal
empty: [int] = []
no_values: {str: int} = {}

// OK if context provides type
@process (items: [int]) -> void = ...
process([])  // [int] inferred from parameter
```

### Ambiguous Generics

```sigil
// None could be Option<anything>
none: Option<int> = None

// Empty result needs type
empty: Result<int, str> = Err("empty")
```

---

## Inference Flow

### Top-Down (Expected Types)

Expected types flow from annotations down into expressions:

```sigil
// Return type str flows down
@describe (n: int) -> str =
    if n > 0 then "positive"
    else if n < 0 then "negative"
    else "zero"
    // All branches must be str
```

### Bottom-Up (Inferred Types)

Inferred types flow from expressions up to variables:

```sigil
@process () -> int = run(
    x = compute(),      // x's type comes from compute()'s return type
    y = x * 2,          // y: int (from x: int)
    y + 10
)
```

### Bidirectional

Both directions work together:

```sigil
@transform (items: [int]) -> [str] =
    map(items, x -> str(x))
    // 1. items: [int] -> x: int (bottom-up from parameter)
    // 2. return [str] -> map returns [str] -> lambda returns str (top-down)
```

---

## Type Widening

Sigil does NOT widen types:

```sigil
// Each branch must have exact same type
@get_value (cond: bool) -> int =
    if cond then 42 else 0  // OK: both int

// ERROR: mismatched types
@bad (cond: bool) -> int =
    if cond then 42 else 3.14  // int vs float
```

---

## The `_` Type Hole

Use `_` to let the compiler infer and show you the type:

```sigil
@debug () -> void = run(
    result: _ = complex_expression(),
    // Compiler tells you: "inferred type: SomeComplexType"
    print(result)
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
identity(42)         // T = int
identity("hello")    // T = str

swap((1, "a"))       // T = int, U = str
```

### When Inference Fails

```sigil
// Can't infer T from empty list
first([])  // ERROR: cannot infer type parameter T

// Solution: explicit type
first<str>([])  // OK: Option<str>

// Or: provide context
result: Option<str> = first([])  // OK: T inferred from expected type
```

### Method Chains

```sigil
// Types flow through chains
result = items
    .filter(x -> x > 0)    // still [int]
    .map(x -> str(x))      // now [str]
    .fold("", (a, b) -> a + ", " + b)  // now str
```

---

## Common Patterns

### Let the Return Type Guide

```sigil
// Return type guides internal inference
@parse_numbers (s: str) -> [int] = run(
    parts = s.split(","),        // [str]
    map(parts, p -> int(p.trim()))  // must be [int] to match return
)
```

### Use Explicit Types for Clarity

```sigil
// Sometimes explicit is clearer even when inferable
@process () -> Result<Data, Error> = run(
    response: HttpResponse = fetch(url),
    data: Data = parse(response.body),
    Ok(data)
)
```

### Type Annotations on Complex Expressions

```sigil
// Help readers (and the compiler) understand complex code
@transform (input: Input) -> Output = run(
    intermediate: IntermediateType = complex_computation(input),
    final_result: Output = finalize(intermediate),
    final_result
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

# Value Semantics

This document covers Sigil's approach to values, bindings, and immutability: no reassignment, shadowing allowed, destructuring patterns, and binding syntax.

---

## Overview

Sigil enforces **value semantics** throughout the language:

- **Immutable by default** - All bindings are immutable
- **No reassignment** - Cannot change what a name refers to
- **Shadowing allowed** - Can create new binding with same name
- **Bindings in patterns** - Variables only introduced inside `run`, `try`, etc.
- **Destructuring** - Extract values from structs and lists

```sigil
@transform (data: Data) -> Data = run(
    // Shadowing: each 'data' is a new immutable binding
    data = step1(data),
    data = step2(data),
    data = step3(data),
    data
)
```

---

## Immutability

### All Values Are Immutable

In Sigil, once a value is created, it cannot be modified:

```sigil
@example () -> void = run(
    list = [1, 2, 3],
    // list[0] = 99     // ERROR: cannot mutate list
    // list.push(4)     // ERROR: no mutation methods

    // Instead: create new list
    new_list = list + [4],
    void
)
```

### Why Immutability?

**AI code quality:**
- Mutation is the #1 source of bugs AI generates
- "Forgot to update", "updated wrong variable", "order-dependent bugs"
- Immutable bindings are **linear** - AI can trace data flow top to bottom

**Human benefit:**
- Every step is visible and debuggable
- No hidden state changes
- Easier to reason about concurrent code

**Key insight:** Mutation helps humans avoid typing. AI doesn't care about typing. AI cares about correctness. Immutability wins.

### Immutable Data Flow

```sigil
@process_order (order: Order) -> Result<Invoice, Error> = run(
    // Each step creates new value, doesn't modify
    validated = validate(order),
    priced = calculate_prices(validated),
    taxed = apply_taxes(priced),
    invoice = generate_invoice(taxed),
    Ok(invoice)
)
```

Compare to mutable style (in other languages):

```python
# Mutable style - harder to trace
def process_order(order):
    validate(order)          # Modifies order?
    calculate_prices(order)  # Modifies order?
    apply_taxes(order)       # Modifies order?
    return generate_invoice(order)
```

---

## No Mutation (Shadowing Instead)

### What "No Reassignment" Means

In languages with mutation, `x = new_value` modifies the existing variable:

```python
# Python - mutation
x = 5
x = x + 1  # Same variable, new value (mutation)
```

In Sigil, every `=` creates a **new binding**. There is no way to modify an existing binding:

```sigil
@example () -> int = run(
    x = 5,         // Binding #1
    x = x + 1,     // Binding #2 (shadows #1) - this is valid!
    x              // Returns 6
)
```

This is **shadowing**, not reassignment. The old `x` still exists (and any references to it remain valid), but the name `x` now refers to the new binding.

### Why This Matters

**In mutable languages, mutation creates bugs:**
```python
# Bug: forgot that transform() doesn't modify in place
x = compute()
transform(x)  # Did this modify x? Who knows!
use(x)        # Using potentially stale value
```

**In Sigil, shadowing makes data flow explicit:**
```sigil
// Every transformation creates a new value
@explicit () -> int = run(
    input = 10,
    doubled = input * 2,    // New binding, input unchanged
    final = doubled + 1,    // New binding, doubled unchanged
    final
)
```

**Key insight:** There is no syntax for mutation in Sigil. Every `=` inside `run` creates a new binding. This eliminates an entire class of bugs where AI (or humans) forget whether a function mutates its argument.

---

## Shadowing

### Creating New Bindings with Same Name

Shadowing allows reusing a name for a new binding:

```sigil
@transform (data: Data) -> Data = run(
    data = step1(data),   // Shadows parameter 'data'
    data = step2(data),   // Shadows previous 'data'
    data = step3(data),   // Shadows again
    data                   // Returns final 'data'
)
```

**Key difference from reassignment:**
- Reassignment: Same binding, different value (forbidden)
- Shadowing: New binding with same name (allowed)

### How Shadowing Works

Each shadow creates a completely new binding:

```sigil
@shadow_demo () -> int = run(
    x = 5,          // Binding #1: x = 5
    y = x,          // y references binding #1
    x = x * 2,      // Binding #2: x = 10 (shadows #1)
    z = x + y,      // z = 10 + 5 = 15
    z               // Returns 15
)
```

Memory view:
```
After "x = 5":
  x (#1) -> 5

After "y = x":
  x (#1) -> 5
  y -> 5 (same value)

After "x = x * 2":
  x (#1) -> 5 (still exists, y references it)
  x (#2) -> 10 (shadows #1)
  y -> 5

After "z = x + y":
  x (#2) -> 10
  y -> 5
  z -> 15
```

### Why Allow Shadowing?

**Natural pipeline style:**
```sigil
// Without shadowing: naming is tedious
@without_shadowing (input: str) -> str = run(
    input1 = input.trim(),
    input2 = input1.lower(),
    input3 = input2.normalize(),
    input3
)

// With shadowing: clean pipeline
@with_shadowing (input: str) -> str = run(
    input = input.trim(),
    input = input.lower(),
    input = input.normalize(),
    input
)
```

**Semantic naming:**
```sigil
// Name stays meaningful throughout
@process_user (user: User) -> User = run(
    user = user.validate(),    // Still "user"
    user = user.enrich(),      // Still "user"
    user = user.normalize(),   // Still "user"
    user
)

// vs. inventing meaningless names
@process_user_verbose (user: User) -> User = run(
    validated_user = user.validate(),
    enriched_user = validated_user.enrich(),
    normalized_user = enriched_user.normalize(),
    normalized_user
)
```

**AI code generation:**
- AI doesn't have to generate `data1`, `data2`, `data3` (error-prone)
- The name stays semantically meaningful throughout
- Common in functional languages (Rust, OCaml, F#)

### Shadowing Scope Rules

**Same scope - allowed:**
```sigil
@same_scope () -> int = run(
    x = 1,
    x = 2,   // Shadows in same run block - OK
    x
)
```

**Nested scope - allowed:**
```sigil
@nested_scope (x: int) -> int = run(
    x = x * 2,        // Shadows parameter - OK
    result = run(
        x = x + 1,    // Shadows outer x - OK
        x * 10
    ),
    result + x        // Uses outer x (doubled)
)
```

**Lambda scope - requires care:**
```sigil
@lambda_scope (items: [int]) -> [int] = run(
    multiplier = 2,
    map(items, n -> n * multiplier)  // Captures multiplier
    // Cannot shadow 'multiplier' after lambda captures it
)
```

---

## Where Bindings Live

### Bindings Only in Patterns

Sigil restricts where variable bindings can occur:

```sigil
// Bindings allowed inside run, try, and other patterns
@allowed () -> int = run(
    x = compute(),   // Binding in run - OK
    y = transform(x),
    y
)

// Function body is single expression - no binding needed
@single_expr (a: int, b: int) -> int = a + b

// Cannot bind in bare expression
@not_allowed () -> int =
    x = 5,     // ERROR: binding outside pattern
    x + 1
```

### Why Restrict Binding Location?

**Explicit sequencing:**
- `run` already provides sequential evaluation
- Adding bindings is natural extension
- No need for separate `let ... in` syntax

**Clear scope:**
- Bindings exist only within their pattern
- No confusion about where variables are valid

**AI understanding:**
- Simple rule: need intermediate values? Use `run`
- Otherwise, single expression
- One syntax, one way to do it

### Binding in Different Patterns

**run - sequential execution:**
```sigil
@example () -> int = run(
    x = step1(),
    y = step2(x),
    z = step3(y),
    z
)
```

**try - error propagation:**
```sigil
@load (path: str) -> Result<Data, Error> = try(
    content = read_file(path),     // Binding, propagates Err
    parsed = parse(content),       // Binding, propagates Err
    Ok(transform(parsed))
)
```

**match - pattern matching:**
```sigil
@describe (opt: Option<int>) -> str = match(opt,
    Some(value) -> "got " + str(value),  // 'value' bound
    None -> "nothing"
)
```

---

## Binding Syntax

### Bare `=` Inside Patterns

Sigil uses simple `=` for bindings inside patterns:

```sigil
run(
    x = compute(),    // Binding with =
    result = use(x),
    result
)
```

### Why Not `let`?

**Inside `run`, it's already clear:**
```sigil
// 'let' would be redundant
run(
    let x = compute(),  // Unnecessary keyword
    x
)

// Simple = is clear enough
run(
    x = compute(),
    x
)
```

**Fewer tokens = less room for error:**
- AI generates fewer tokens
- Less visual noise
- Focus on the values, not keywords

### Type Annotations on Bindings

Add explicit types when needed:

```sigil
run(
    x: int = compute(),           // Explicit type
    y = compute(),                 // Inferred type
    result: float = float(x + y),  // Explicit for clarity
    result
)
```

---

## Destructuring

### Extracting Values from Structures

Destructuring binds multiple names from a single value:

**Struct destructuring:**
```sigil
type Point = { x: int, y: int }

@distance_from_origin (point: Point) -> float = run(
    { x, y } = point,   // Extract x and y
    sqrt(float(x * x + y * y))
)
```

**List destructuring:**
```sigil
@first_two (items: [int]) -> int = run(
    [a, b, ..rest] = items,   // First two and rest
    a + b
)
```

**Nested destructuring:**
```sigil
type Line = { start: Point, end: Point }

@line_length (line: Line) -> float = run(
    { start: { x: x1, y: y1 }, end: { x: x2, y: y2 } } = line,
    dx = x2 - x1,
    dy = y2 - y1,
    sqrt(float(dx * dx + dy * dy))
)
```

### Struct Destructuring Syntax

**Full extraction:**
```sigil
{ field1, field2, field3 } = struct_value
```

**Partial extraction:**
```sigil
{ field1, field2 } = struct_value   // Only extract what you need
```

**Renaming on extraction:**
```sigil
{ x: horizontal, y: vertical } = point   // Rename x to horizontal
```

**With type annotation:**
```sigil
{ x, y }: Point = compute_point()
```

### List Destructuring Syntax

**Fixed elements:**
```sigil
[first, second, third] = list   // Exactly 3 elements required
```

**With rest pattern:**
```sigil
[first, ..rest] = list          // First element, rest as list
[first, second, ..rest] = list  // First two, rest as list
[..init, last] = list           // All but last, then last
```

**Nested:**
```sigil
[[a, b], [c, d]] = matrix   // 2x2 matrix extraction
```

### Pattern Matching Destructuring

Destructuring in `match` arms:

```sigil
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(a: float, b: float, c: float)

@area (shape: Shape) -> float = match(shape,
    .Circle: { radius } -> 3.14159 * radius * radius,
    .Rectangle: { width, height } -> width * height,
    .Triangle: { a, b, c } -> run(
        s = (a + b + c) / 2.0,
        sqrt(s * (s - a) * (s - b) * (s - c))
    )
)
```

### Destructuring in Function Parameters

Function parameters can use destructuring:

```sigil
@distance ({ x: x1, y: y1 }: Point, { x: x2, y: y2 }: Point) -> float = run(
    dx = x2 - x1,
    dy = y2 - y1,
    sqrt(float(dx * dx + dy * dy))
)
```

---

## Comparison with Other Languages

### Rust

| Feature | Rust | Sigil |
|---------|------|-------|
| Immutable by default | `let` (mutable: `let mut`) | Always immutable |
| Shadowing | Allowed | Allowed |
| Reassignment | Allowed with `mut` | Forbidden |
| Destructuring | Yes | Yes |
| Binding location | Anywhere | Inside patterns only |

**Example comparison:**
```rust
// Rust
let x = 5;
let x = x + 1;  // Shadowing - OK
let mut y = 5;
y = y + 1;      // Reassignment - OK with mut
```

```sigil
// Sigil
run(
    x = 5,
    x = x + 1,    // Shadowing - OK
    // No way to mutate y in place
    x
)
```

### OCaml / F#

| Feature | OCaml/F# | Sigil |
|---------|----------|-------|
| Immutable by default | Yes | Yes |
| Shadowing | Allowed | Allowed |
| Let syntax | `let x = ... in ...` | `run(x = ..., ...)` |
| Destructuring | Yes | Yes |

**Example comparison:**
```ocaml
(* OCaml *)
let process data =
  let data = step1 data in
  let data = step2 data in
  let data = step3 data in
  data
```

```sigil
// Sigil
@process (data: Data) -> Data = run(
    data = step1(data),
    data = step2(data),
    data = step3(data),
    data
)
```

### JavaScript/TypeScript

| Feature | JS/TS | Sigil |
|---------|-------|-------|
| Immutable | `const` | Always |
| Shadowing | Allowed | Allowed |
| Reassignment | `let` allows | Forbidden |
| Destructuring | Yes | Yes |

**Example comparison:**
```javascript
// JavaScript
const process = (data) => {
    const data1 = step1(data);   // Can't shadow with const
    const data2 = step2(data1);  // Must use new names
    return step3(data2);
};

// Or with let (mutable)
const process = (data) => {
    let result = step1(data);    // Mutable
    result = step2(result);      // Reassignment
    return step3(result);
};
```

```sigil
// Sigil - shadowing makes this clean
@process (data: Data) -> Data = run(
    data = step1(data),
    data = step2(data),
    data = step3(data),
    data
)
```

---

## Common Patterns

### Pipeline Transformation

```sigil
@process_input (input: str) -> Result<Output, Error> = try(
    input = trim(input),
    input = validate(input),
    data = parse(input),
    data = transform(data),
    data = enrich(data),
    Ok(data)
)
```

### Conditional Binding

```sigil
@compute (value: int) -> int = run(
    result = if value > 0 then value * 2 else value / 2,
    adjusted = result + 10,
    adjusted
)
```

### Multiple Extractions

```sigil
type Response = {
    status: int,
    headers: Map<str, str>,
    body: str
}

@process_response (resp: Response) -> Result<Data, Error> = run(
    { status, body } = resp,
    data = if status == 200 then parse(body) else Err("bad status"),
    data
)
```

### Accumulation Without Mutation

```sigil
// Use fold instead of mutable accumulator
@sum_positives (items: [int]) -> int =
    fold(filter(items, n -> n > 0), 0, (acc, n) -> acc + n)

// Or with explicit bindings
@sum_positives_explicit (items: [int]) -> int = run(
    positives = filter(items, n -> n > 0),
    total = fold(positives, 0, +),
    total
)
```

---

## Error Messages

### Binding Outside Pattern

```sigil
@example () -> int =
    x = compute(),
    x + 1
```

```
error[E0202]: binding outside pattern
  --> example.si:2:5
   |
 2 |     x = compute(),
   |     ^^^^^^^^^^^^^ bindings only allowed inside patterns
   |
   = help: wrap in 'run' for sequential bindings:
           @example () -> int = run(
               x = compute(),
               x + 1
           )
```

### Destructuring Mismatch

```sigil
@example () -> int = run(
    [a, b, c] = [1, 2],   // Only 2 elements
    a + b + c
)
```

```
error[E0203]: destructuring pattern mismatch
  --> example.si:2:5
   |
 2 |     [a, b, c] = [1, 2],
   |     ^^^^^^^^^   ^^^^^^ list has 2 elements
   |     |
   |     pattern expects 3 elements
   |
   = help: use rest pattern if list may have fewer elements:
           [a, b, ..rest] = [1, 2]
```

---

## Best Practices

### 1. Use Shadowing for Pipelines

```sigil
// Good: clear pipeline with shadowing
@process (data: Data) -> Data = run(
    data = validate(data),
    data = transform(data),
    data = finalize(data),
    data
)

// Avoid: meaningless numbered names
@process_bad (data: Data) -> Data = run(
    data1 = validate(data),
    data2 = transform(data1),
    data3 = finalize(data2),
    data3
)
```

### 2. Use Meaningful Names When Values Differ

```sigil
// Good: names reflect meaning
@convert (input: str) -> Output = run(
    parsed = parse(input),        // Different type
    validated = validate(parsed), // Different state
    output = transform(validated),
    output
)
```

### 3. Destructure Close to Usage

```sigil
// Good: destructure where values are used
@compute (point: Point) -> float = run(
    { x, y } = point,   // Destructure at top
    squared = x * x + y * y,
    sqrt(float(squared))
)

// Also good: direct field access for simple cases
@compute_simple (point: Point) -> float =
    sqrt(float(point.x * point.x + point.y * point.y))
```

### 4. Prefer Single Expression When Possible

```sigil
// Good: single expression, no run needed
@double (n: int) -> int = n * 2

// Unnecessary: run for single binding
@double_verbose (n: int) -> int = run(
    result = n * 2,
    result
)
```

---

## See Also

- [ARC Overview](01-arc-overview.md) - Memory management
- [Strings and Lists](03-strings-and-lists.md) - Immutable collections
- [Pattern Matching](../06-pattern-matching/index.md) - Match expressions
- [Patterns Overview](../02-syntax/03-patterns-overview.md) - run, try, etc.

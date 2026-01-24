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
    let data = step1(data),
    let data = step2(data),
    let data = step3(data),
    data,
)
```

---

## Immutability

### All Values Are Immutable

In Sigil, once a value is created, it cannot be modified:

```sigil
@example () -> void = run(
    let list = [1, 2, 3],
    // ERROR: cannot mutate list
    // list[0] = 99
    // ERROR: no mutation methods
    // list.push(4)

    // Instead: create new list
    let new_list = list + [4],
    void,
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
    let validated = validate(order),
    let priced = calculate_prices(validated),
    let taxed = apply_taxes(priced),
    let invoice = generate_invoice(taxed),
    Ok(invoice),
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

In Sigil, every `let x =` creates a **new binding**. There is no way to modify an existing binding:

```sigil
@example () -> int = run(
    // Binding #1
    let x = 5,
    // Binding #2 (shadows #1) - this is valid!
    let x = x + 1,
    // Returns 6
    x,
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
    let input = 10,
    // New binding, input unchanged
    let doubled = input * 2,
    // New binding, doubled unchanged
    let final = doubled + 1,
    final,
)
```

**Key insight:** There is no syntax for mutation in Sigil. Every `let x =` inside `run` creates a new binding. This eliminates an entire class of bugs where AI (or humans) forget whether a function mutates its argument.

---

## Shadowing

### Creating New Bindings with Same Name

Shadowing allows reusing a name for a new binding:

```sigil
@transform (data: Data) -> Data = run(
    // Shadows parameter 'data'
    let data = step1(data),
    // Shadows previous 'data'
    let data = step2(data),
    // Shadows again
    let data = step3(data),
    // Returns final 'data'
    data,
)
```

**Key difference from reassignment:**
- Reassignment: Same binding, different value (forbidden)
- Shadowing: New binding with same name (allowed)

### How Shadowing Works

Each shadow creates a completely new binding:

```sigil
@shadow_demo () -> int = run(
    // Binding #1: x = 5
    let x = 5,
    // y references binding #1
    let y = x,
    // Binding #2: x = 10 (shadows #1)
    let x = x * 2,
    // z = 10 + 5 = 15
    let z = x + y,
    // Returns 15
    z,
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
    let input1 = input.trim(),
    let input2 = input1.lower(),
    let input3 = input2.normalize(),
    input3,
)

// With shadowing: clean pipeline
@with_shadowing (input: str) -> str = run(
    let input = input.trim(),
    let input = input.lower(),
    let input = input.normalize(),
    input,
)
```

**Semantic naming:**
```sigil
// Name stays meaningful throughout
@process_user (user: User) -> User = run(
    // Still "user"
    let user = user.validate(),
    // Still "user"
    let user = user.enrich(),
    // Still "user"
    let user = user.normalize(),
    user,
)

// vs. inventing meaningless names
@process_user_verbose (user: User) -> User = run(
    let validated_user = user.validate(),
    let enriched_user = validated_user.enrich(),
    let normalized_user = enriched_user.normalize(),
    normalized_user,
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
    let x = 1,
    // Shadows in same run block - OK
    let x = 2,
    x,
)
```

**Nested scope - allowed:**
```sigil
@nested_scope (x: int) -> int = run(
    // Shadows parameter - OK
    let x = x * 2,
    let result = run(
        // Shadows outer x - OK
        let x = x + 1,
        x * 10,
    ),
    // Uses outer x (doubled)
    result + x,
)
```

**Lambda scope - requires care:**
```sigil
@lambda_scope (items: [int]) -> [int] = run(
    let multiplier = 2,
    // Captures multiplier
    // Cannot shadow 'multiplier' after lambda captures it
    map(items, number -> number * multiplier),
)
```

---

## Where Bindings Live

### Bindings Only in Patterns

Sigil restricts where variable bindings can occur:

```sigil
// Bindings allowed inside run, try, and other patterns
@allowed () -> int = run(
    // Binding in run - OK
    let x = compute(),
    let y = transform(x),
    y,
)

// Function body is single expression - no binding needed
@single_expr (a: int, b: int) -> int = a + b

// Cannot bind in bare expression
// ERROR: binding outside pattern
@not_allowed () -> int =
    let x = 5,
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
    let x = step1(),
    let y = step2(x),
    let z = step3(y),
    z,
)
```

**try - error propagation:**
```sigil
@load (path: str) -> Result<Data, Error> = try(
    // Binding, propagates Err
    let content = read_file(path)?,
    // Binding, propagates Err
    let parsed = parse(content)?,
    Ok(transform(parsed)),
)
```

**match - pattern matching:**
```sigil
@describe (opt: Option<int>) -> str = match(opt,
    // 'value' bound
    Some(value) -> "got " + str(value),
    None -> "nothing"
)
```

---

## Binding Syntax

### Using `let` for Bindings

Sigil uses `let` for all bindings inside patterns:

```sigil
run(
    // Binding with let
    let x = compute(),
    let result = use(x),
    result,
)
```

### Why `let`?

**Explicit and consistent:**
```sigil
// Clear binding syntax
run(
    let x = compute(),
    let y = transform(x),
    y,
)
```

**Benefits of explicit `let`:**
- Clear visual indicator of new bindings
- Consistent with other modern languages (Rust, Swift, JavaScript)
- AI can reliably identify bindings vs other expressions
- Easier to grep and analyze code

### Type Annotations on Bindings

Add explicit types when needed:

```sigil
run(
    // Explicit type
    let x: int = compute(),
    // Inferred type
    let y = compute(),
    // Explicit for clarity
    let result: float = float(x + y),
    result,
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
    // Extract x and y
    let { x, y } = point,
    sqrt(float(x * x + y * y)),
)
```

**List destructuring:**
```sigil
@first_two (items: [int]) -> int = run(
    // First two and rest
    let [a, b, ..rest] = items,
    a + b,
)
```

**Nested destructuring:**
```sigil
type Line = { start: Point, end: Point }

@line_length (line: Line) -> float = run(
    let { start: { x: x1, y: y1 }, end: { x: x2, y: y2 } } = line,
    let dx = x2 - x1,
    let dy = y2 - y1,
    sqrt(float(dx * dx + dy * dy)),
)
```

### Struct Destructuring Syntax

**Full extraction:**
```sigil
let { field1, field2, field3 } = struct_value
```

**Partial extraction:**
```sigil
// Only extract what you need
let { field1, field2 } = struct_value
```

**Renaming on extraction:**
```sigil
// Rename x to horizontal
let { x: horizontal, y: vertical } = point
```

**With type annotation:**
```sigil
let { x, y }: Point = compute_point()
```

### List Destructuring Syntax

**Fixed elements:**
```sigil
// Exactly 3 elements required
let [first, second, third] = list
```

**With rest pattern:**
```sigil
// First element, rest as list
let [first, ..rest] = list
// First two, rest as list
let [first, second, ..rest] = list
// All but last, then last
let [..init, last] = list
```

**Nested:**
```sigil
// 2x2 matrix extraction
let [[a, b], [c, d]] = matrix
```

### Pattern Matching Destructuring

Destructuring in `match` arms:

```sigil
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(a: float, b: float, c: float)

@area (shape: Shape) -> float = match(shape,
    Circle(radius) -> 3.14159 * radius * radius,
    Rectangle(width, height) -> width * height,
    Triangle(a, b, c) -> run(
        let s = (a + b + c) / 2.0,
        sqrt(s * (s - a) * (s - b) * (s - c)),
    ),
)
```

### Destructuring in Function Parameters

Function parameters can use destructuring:

```sigil
@distance ({ x: x1, y: y1 }: Point, { x: x2, y: y2 }: Point) -> float = run(
    let dx = x2 - x1,
    let dy = y2 - y1,
    sqrt(float(dx * dx + dy * dy)),
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
    let x = 5,
    // Shadowing - OK
    let x = x + 1,
    // No way to mutate y in place
    x,
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
    let data = step1(data),
    let data = step2(data),
    let data = step3(data),
    data,
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
    let data = step1(data),
    let data = step2(data),
    let data = step3(data),
    data,
)
```

---

## Common Patterns

### Pipeline Transformation

```sigil
@process_input (input: str) -> Result<Output, Error> = try(
    let input = trim(input),
    let input = validate(input)?,
    let data = parse(input)?,
    let data = transform(data),
    let data = enrich(data),
    Ok(data),
)
```

### Conditional Binding

```sigil
@compute (value: int) -> int = run(
    let result = if value > 0 then value * 2 else value / 2,
    let adjusted = result + 10,
    adjusted,
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
    let { status, body } = resp,
    let data = if status == 200 then parse(body) else Err("bad status"),
    data,
)
```

### Accumulation Without Mutation

```sigil
// Use fold instead of mutable accumulator
@sum_positives (items: [int]) -> int =
    fold(filter(items, number -> number > 0), 0, (accumulator, number) -> accumulator + number)

// Or with explicit bindings
@sum_positives_explicit (items: [int]) -> int = run(
    let positives = filter(items, number -> number > 0),
    let total = fold(positives, 0, +),
    total,
)
```

---

## Error Messages

### Binding Outside Pattern

```sigil
@example () -> int =
    let x = compute(),
    x + 1
```

```
error[E0202]: binding outside pattern
  --> example.si:2:5
   |
 2 |     let x = compute(),
   |     ^^^^^^^^^^^^^^^^^ bindings only allowed inside patterns
   |
   = help: wrap in 'run' for sequential bindings:
           @example () -> int = run(
               let x = compute(),
               x + 1,
           )
```

### Destructuring Mismatch

```sigil
@example () -> int = run(
    // Only 2 elements
    let [a, b, c] = [1, 2],
    a + b + c,
)
```

```
error[E0203]: destructuring pattern mismatch
  --> example.si:2:5
   |
 2 |     let [a, b, c] = [1, 2],
   |         ^^^^^^^^^   ^^^^^^ list has 2 elements
   |         |
   |         pattern expects 3 elements
   |
   = help: use rest pattern if list may have fewer elements:
           let [a, b, ..rest] = [1, 2]
```

---

## Best Practices

### 1. Use Shadowing for Pipelines

```sigil
// Good: clear pipeline with shadowing
@process (data: Data) -> Data = run(
    let data = validate(data),
    let data = transform(data),
    let data = finalize(data),
    data,
)

// Avoid: meaningless numbered names
@process_bad (data: Data) -> Data = run(
    let data1 = validate(data),
    let data2 = transform(data1),
    let data3 = finalize(data2),
    data3,
)
```

### 2. Use Meaningful Names When Values Differ

```sigil
// Good: names reflect meaning
@convert (input: str) -> Output = run(
    // Different type
    let parsed = parse(input),
    // Different state
    let validated = validate(parsed),
    let output = transform(validated),
    output,
)
```

### 3. Destructure Close to Usage

```sigil
// Good: destructure where values are used
@compute (point: Point) -> float = run(
    // Destructure at top
    let { x, y } = point,
    let squared = x * x + y * y,
    sqrt(float(squared)),
)

// Also good: direct field access for simple cases
@compute_simple (point: Point) -> float =
    sqrt(float(point.x * point.x + point.y * point.y))
```

### 4. Prefer Single Expression When Possible

```sigil
// Good: single expression, no run needed
@double (number: int) -> int = number * 2

// Unnecessary: run for single binding
@double_verbose (number: int) -> int = run(
    let result = number * 2,
    result,
)
```

---

## See Also

- [ARC Overview](01-arc-overview.md) - Memory management
- [Strings and Lists](03-strings-and-lists.md) - Immutable collections
- [Pattern Matching](../06-pattern-matching/index.md) - Match expressions
- [Patterns Overview](../02-syntax/03-patterns-overview.md) - run, try, etc.

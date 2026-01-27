# Lambdas

This document covers anonymous functions (lambdas) in Ori, including syntax, closures, variable capture, and type inference.

---

## What Are Lambdas?

Lambdas are anonymous functions defined inline. They're particularly useful for:

- Short transformations passed to higher-order functions
- Creating closures that capture surrounding values
- Defining behavior without the ceremony of named functions

```ori
// Named function
@double (number: int) -> int = number * 2

// Equivalent lambda
value -> value * 2
```

---

## Basic Lambda Syntax

### Single Parameter

```ori
x -> expression
```

Examples:

```ori
// double
value -> value * 2
// is positive
item -> item > 0
// convert to string
item -> item.to_string()
// get string length
text -> text.len()
```

### Using Lambdas

Lambdas are typically passed to higher-order functions:

```ori
@main () -> void = run(
    // [2, 4, 6]
    let doubled = map(
        .over: [1, 2, 3],
        .transform: value -> value * 2,
    ),
    // [1]
    let positive = filter(
        .over: [-1, 0, 1],
        .predicate: item -> item > 0,
    ),
    // [1, 2]
    let lengths = map(
        .over: ["a", "bb"],
        .transform: text -> text.len(),
    ),
)
```

---

## Multiple Parameters

### Syntax

```ori
(param1, param2) -> expression
```

### Examples

```ori
// add two values
(left, right) -> left + right
// sum of squares
(first, second) -> first * first + second * second
// format person
(name, age) -> name + ": " + str(.value: age)
```

### In Higher-Order Functions

```ori
@main () -> void = run(
    // fold takes (accumulator, item) -> new_accumulator
    let sum = fold(
        .over: [1, 2, 3],
        .initial: 0,
        .operation: (accumulator, item) -> accumulator + item,
    ),

    // reduce pairs
    let pairs = [(1, 2), (3, 4), (5, 6)],
    let sums = map(
        .over: pairs,
        .transform: (left, right) -> left + right,
    ),
)
```

---

## No Parameters

Use empty parentheses for zero-parameter lambdas:

```ori
() -> expression
```

Examples:

```ori
// constant function
() -> 42
// deferred computation
() -> random()
// lazy evaluation
() -> get_current_time()
```

### Deferred Execution

```ori
@with_default<T> (opt: Option<T>, default_fn: () -> T) -> T = match(opt,
    Some(value) -> value,
    // only called if needed
    None -> default_fn()
)

@main () -> void = run(
    // expensive() only called if value is None
    let result = with_default(
        .opt: Some(5),
        .default_fn: () -> expensive(),
    ),
)
```

---

## Type Annotations in Lambdas

### Inferred Types (Common)

Types are usually inferred from context:

```ori
// The context tells us value is int
doubled = map(
    .over: [1, 2, 3],
    .transform: value -> value * 2,
)

// The context tells us s is str
lengths = map(
    .over: ["a", "bb"],
    .transform: text -> text.len(),
)
```

### Explicit Type Annotations

Add types when inference is insufficient:

```ori
// Annotate parameter
(value: int) -> value * 2

// Annotate multiple parameters
(left: int, right: int) -> left + right

// Full annotation with return type
(value: int) -> int = value * 2
```

### When Explicit Types Are Needed

```ori
// Ambiguous: what type is value?
// ERROR: cannot infer type
transform = value -> value

// Explicit type resolves ambiguity
// OK
transform: (int) -> int = value -> value

// Or annotate the lambda
// OK
transform = (value: int) -> value
```

---

## Closures

### What Are Closures?

A closure is a lambda that captures values from its surrounding scope:

```ori
@make_adder (amount: int) -> (int) -> int =
    // amount is captured from the outer scope
    value -> value + amount

@main () -> void = run(
    let add5 = make_adder(.amount: 5),
    let add10 = make_adder(.amount: 10),
    // 8
    print(add5(3)),
    // 13
    print(add10(3)),
)
```

The lambda `value -> value + amount` captures the value `amount` from `make_adder`.

### Multiple Captured Values

Closures can capture multiple values:

```ori
@make_linear_function (slope: int, intercept: int) -> (int) -> int =
    // captures both slope and intercept
    value -> slope * value + intercept

@main () -> void = run(
    let linear = make_linear_function(
        .slope: 2,
        .intercept: 3,
    ),
    // 2 * 5 + 3 = 13
    print(linear(5)),
)
```

### Capturing from Enclosing Scope

```ori
@filter_above (items: [int], threshold: int) -> [int] = filter(
    .over: items,
    // captures threshold
    .predicate: item -> item > threshold,
)

@main () -> void = run(
    let numbers = [1, 5, 3, 8, 2, 9],
    // [8, 9]
    let above_five = filter_above(
        .items: numbers,
        .threshold: 5,
    ),
    print(above_five),
)
```

---

## Capture by Value

### Semantics

Ori captures variables by value (copy):

```ori
@main () -> void = run(
    let amount = 5,
    // amount is COPIED into the closure
    let add = value -> value + amount,
    // Even if amount could change, add's captured amount remains 5
    // 15
    print(add(10)),
)
```

### Why Capture by Value?

1. **Safety** — No aliasing issues with captured references
2. **Simplicity** — Values are copied at closure creation time
3. **Immutability** — Aligns with Ori's immutable-by-default philosophy
4. **Predictability** — Closure behavior doesn't change if outer scope changes

### Capture Timing

Values are captured when the closure is created:

```ori
@make_closures () -> [(int) -> int] = run(
    let mut closures = [],
    // captures nothing special
    closures = closures + [value -> value + 1],
    closures = closures + [value -> value + 2],
    closures = closures + [value -> value + 3],
    closures,
)
```

Each closure captures the literal value from its definition.

---

## Closures in Patterns

### With `map`

```ori
@scale_all (items: [int], factor: int) -> [int] = map(
    .over: items,
    // captures factor
    .transform: item -> item * factor,
)

@main () -> void = run(
    // [10, 20, 30]
    let scaled = scale_all(
        .items: [1, 2, 3],
        .factor: 10,
    ),
    print(scaled),
)
```

### With `filter`

```ori
@keep_above (items: [int], threshold: int) -> [int] = filter(
    .over: items,
    // captures threshold
    .predicate: item -> item > threshold,
)

@main () -> void = run(
    // [5, 8]
    let big = keep_above(
        .items: [1, 5, 3, 8],
        .threshold: 4,
    ),
    print(big),
)
```

### With `fold`

```ori
@weighted_sum (items: [int], weight: int) -> int = fold(
    .over: items,
    .initial: 0,
    // captures weight
    .operation: (accumulator, item) -> accumulator + item * weight,
)

@main () -> void = run(
    // 0 + 2 + 4 + 6 = 12
    let total = weighted_sum(
        .items: [1, 2, 3],
        .weight: 2,
    ),
    print(total),
)
```

---

## Returning Closures

Functions can return closures:

```ori
@make_adder (amount: int) -> (int) -> int =
    value -> value + amount

@make_multiplier (factor: int) -> (int) -> int =
    value -> value * factor

@make_comparator (threshold: int) -> (int) -> bool =
    value -> value > threshold

@main () -> void = run(
    let add5 = make_adder(.amount: 5),
    let times3 = make_multiplier(.factor: 3),
    let is_big = make_comparator(.threshold: 100),

    // 15
    print(add5(10)),
    // 30
    print(times3(10)),
    // false
    print(is_big(50)),
    // true
    print(is_big(150)),
)
```

### Closure Factories

```ori
type Validator = (str) -> bool

@make_length_validator (min: int, max: int) -> Validator =
    text -> text.len() >= min && text.len() <= max

@make_pattern_validator (pattern: str) -> Validator =
    text -> text.contains(.str: pattern)

@main () -> void = run(
    let valid_username = make_length_validator(
        .min: 3,
        .max: 20,
    ),
    let valid_email = make_pattern_validator(.pattern: "@"),

    // false (too short)
    print(valid_username("ab")),
    // true
    print(valid_username("alice")),
    // true
    print(valid_email("test@x.com")),
)
```

---

## Type Inference in Lambdas

### How Inference Works

Lambda parameter types are inferred from context:

```ori
// map expects (T) -> U where T is the element type
doubled = map(
    // [int] tells us element type is int
    .over: [1, 2, 3],
    // x is inferred as int
    .transform: value -> value * 2,
)
```

### Inference Chain

Types flow through expressions:

```ori
@main () -> void = run(
    // [int]
    let numbers = [1, 2, 3],
    let doubled = map(
        .over: numbers,
        // item: int, result: [int]
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        // item: int, result: [int]
        .predicate: item -> item > 3,
    ),
    let sum = fold(
        .over: filtered,
        .initial: 0,
        // accumulator: int, item: int, result: int
        .operation: (accumulator, item) -> accumulator + item,
    ),
)
```

### Bidirectional Inference

Types can flow both ways:

```ori
// Type annotation on variable flows to lambda
// value is int
transform: (int) -> int = value -> value * 2

// Type from usage flows to lambda
items: [int] = map(
    .over: ["1", "2", "3"],
    .transform: text -> parse_int(.str: text),
)
// text is str (from input), result is int (from annotation)
```

### When Inference Fails

```ori
// Ambiguous: no context to determine value's type
// ERROR
transform = value -> value

// Solutions:
// annotate variable
transform: (int) -> int = value -> value
// annotate parameter
transform = (value: int) -> value
```

---

## Lambda Body Expressions

### Simple Expressions

```ori
value -> value * 2
value -> value.len
item -> item > 0
```

### Conditional Expressions

```ori
number -> if number > 0 then number else -number

(left, right) -> if left > right then left else right
```

### Pattern Matching

```ori
option -> match(option,
    Some(value) -> value,
    None -> 0,
)

result -> match(result,
    Ok(value) -> value.to_string(),
    Err(error) -> "Error: " + error.message,
)
```

### Using `run` for Complex Bodies

```ori
item -> run(
    let validated = validate(.item: item),
    let transformed = transform(.value: validated),
    finalize(.value: transformed),
)
```

---

## Multi-line Lambdas

For complex lambdas, spread across multiple lines:

```ori
@process_items (items: [Item]) -> [Result<EnrichedItem, Error>] = map(
    .over: items,
    .transform: item -> run(
        let validated = validate(.item: item),
        let enriched = enrich(.value: validated),
        Ok(enriched),
    ),
)

// Or with patterns
@categorize (items: [int]) -> [(int, str)] = map(
    .over: items,
    .transform: number ->
        if number < 0 then (number, "negative")
        else if number == 0 then (number, "zero")
        else (number, "positive"),
)
```

---

## Common Lambda Patterns

### Identity

```ori
value -> value
```

### Constant

```ori
// ignores input, always returns 42
_ -> 42
// deferred constant
() -> config
```

### Projection (Field Access)

```ori
user -> user.name
point -> point.x
item -> item.price
```

### Predicate

```ori
item -> item > 0
text -> len(.collection: text) < 10
user -> user.is_active
```

### Transformation

```ori
value -> value * 2
text -> text.upper()
item -> Item { id: item.id, name: item.name, processed: true }
```

### Combining Values

```ori
(left, right) -> left + right
(first, second) -> Point { x: first, y: second }
(name, value) -> name + "=" + str(.value: value)
```

---

## Lambdas vs Named Functions

### When to Use Lambdas

- Short, one-off transformations
- When the logic is simple and obvious
- When capturing values from the enclosing scope

```ori
// Good: clear and concise
doubled = map(
    .over: items,
    .transform: item -> item * 2,
)
above_five = filter(
    .over: items,
    .predicate: item -> item > 5,
)
```

### When to Use Named Functions

- Complex logic requiring documentation
- Reusable across multiple call sites
- When testing the function in isolation

```ori
// Good: reusable and testable
@is_valid_email (email: str) -> bool =
    email.contains(.str: "@") && email.contains(.str: ".")

// Can be tested separately and reused
valid_emails = filter(
    .over: emails,
    .predicate: is_valid_email,
)
```

### Hybrid Approach

```ori
// Define complex logic as named function
@calculate_score (user: User) -> int = run(
    let base = user.level * 100,
    let bonus = if user.is_premium then 50 else 0,
    base + bonus,
)

// Use in lambda for additional transformation
let ranked = map(
    .over: users,
    .transform: user -> (user, calculate_score(.user: user)),
)
```

---

## Nested Lambdas

Lambdas can contain other lambdas:

```ori
@make_mapper<T, U> (transform: (T) -> U) -> ([T]) -> [U] =
    items -> map(
        .over: items,
        .transform: transform,
    )

@main () -> void = run(
    let double_all = make_mapper(.transform: value -> value * 2),
    // [2, 4, 6]
    let result = double_all([1, 2, 3]),
)
```

### Caution with Nesting

Deep nesting can harm readability:

```ori
// Hard to read - too deeply nested
result = map(
    .over: items,
    .transform: item -> map(
        .over: item.children,
        .transform: child -> filter(
            .over: child.items,
            .predicate: entry -> entry.active,
        ),
    ),
)

// Better: use named functions or break into steps
@get_active_items (children: [Child]) -> [[Item]] = map(
    .over: children,
    .transform: child -> filter(
        .over: child.items,
        .predicate: entry -> entry.active,
    ),
)

result = map(
    .over: items,
    .transform: item -> get_active_items(.children: item.children),
)
```

---

## Best Practices

### Keep Lambdas Short

```ori
// Good: simple and readable
map(
    .over: items,
    .transform: item -> item * 2,
)

// Consider named function for complex logic
@process (item: Item) -> Result<EnrichedItem, Error> = run(
    let validated = validate(.item: item),
    let transformed = transform(.value: validated),
    let enriched = enrich(.value: transformed),
    Ok(enriched),
)
let result = map(
    .over: items,
    .transform: process,
)
```

### Use Descriptive Parameter Names

```ori
// Good: clear meaning
filter(
    .over: users,
    .predicate: user -> user.is_active,
)
map(
    .over: orders,
    .transform: order -> order.total,
)

// OK for simple cases
map(
    .over: [1, 2, 3],
    .transform: item -> item * 2,
)
```

### Avoid Capturing Mutable State

```ori
// Ori is immutable by default, so this is natural
@make_counter_bad () -> (() -> int) = run(
    let count = 0,
    // Can't mutate count anyway
    () -> count,
)

// Use explicit state if needed
type Counter = { value: int }
@increment (c: Counter) -> Counter = Counter { value: c.value + 1 }
```

### Leverage Type Inference

```ori
// Let the compiler infer types when obvious
doubled = map(
    .over: [1, 2, 3],
    .transform: item -> item * 2,
)

// Add types when it helps clarity or when required
transform: (User) -> str = user -> user.name + " <" + user.email + ">"
```

---

## Summary

| Syntax | Meaning |
|--------|---------|
| `value -> expr` | Single parameter lambda |
| `(left, right) -> expr` | Multi-parameter lambda |
| `() -> expr` | No-parameter lambda |
| `(value: int) -> expr` | Explicit parameter type |
| `value -> value + amount` | Closure (captures `amount`) |

---

## See Also

- [Function Definitions](01-function-definitions.md)
- [First-Class Functions](02-first-class-functions.md)
- [Higher-Order Functions](04-higher-order.md)
- [Type Inference](../03-type-system/05-type-inference.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)

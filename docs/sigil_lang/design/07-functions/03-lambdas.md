# Lambdas

This document covers anonymous functions (lambdas) in Sigil, including syntax, closures, variable capture, and type inference.

---

## What Are Lambdas?

Lambdas are anonymous functions defined inline. They're particularly useful for:

- Short transformations passed to higher-order functions
- Creating closures that capture surrounding values
- Defining behavior without the ceremony of named functions

```sigil
// Named function
@double (n: int) -> int = n * 2

// Equivalent lambda
x -> x * 2
```

---

## Basic Lambda Syntax

### Single Parameter

```sigil
x -> expression
```

Examples:

```sigil
x -> x * 2          // double
x -> x > 0          // is positive
x -> x.to_string()  // convert to string
s -> s.len          // get string length
```

### Using Lambdas

Lambdas are typically passed to higher-order functions:

```sigil
@main () -> void = run(
    doubled = map([1, 2, 3], x -> x * 2),       // [2, 4, 6]
    positive = filter([-1, 0, 1], x -> x > 0),  // [1]
    lengths = map(["a", "bb"], s -> s.len)      // [1, 2]
)
```

---

## Multiple Parameters

### Syntax

```sigil
(param1, param2) -> expression
```

### Examples

```sigil
(a, b) -> a + b                      // add two values
(x, y) -> x * x + y * y              // sum of squares
(name, age) -> name + ": " + str(age) // format person
```

### In Higher-Order Functions

```sigil
@main () -> void = run(
    // fold takes (accumulator, item) -> new_accumulator
    sum = fold([1, 2, 3], 0, (acc, n) -> acc + n),

    // reduce pairs
    pairs = [(1, 2), (3, 4), (5, 6)],
    sums = map(pairs, (a, b) -> a + b)
)
```

---

## No Parameters

Use empty parentheses for zero-parameter lambdas:

```sigil
() -> expression
```

Examples:

```sigil
() -> 42                    // constant function
() -> random()              // deferred computation
() -> get_current_time()    // lazy evaluation
```

### Deferred Execution

```sigil
@with_default<T> (opt: Option<T>, default_fn: () -> T) -> T = match(opt,
    Some(x) -> x,
    None -> default_fn()    // only called if needed
)

@main () -> void = run(
    // expensive() only called if value is None
    result = with_default(Some(5), () -> expensive())
)
```

---

## Type Annotations in Lambdas

### Inferred Types (Common)

Types are usually inferred from context:

```sigil
// The context tells us x is int
doubled = map([1, 2, 3], x -> x * 2)

// The context tells us s is str
lengths = map(["a", "bb"], s -> s.len)
```

### Explicit Type Annotations

Add types when inference is insufficient:

```sigil
// Annotate parameter
(x: int) -> x * 2

// Annotate multiple parameters
(a: int, b: int) -> a + b

// Full annotation with return type
(x: int) -> int : x * 2
```

### When Explicit Types Are Needed

```sigil
// Ambiguous: what type is x?
f = x -> x              // ERROR: cannot infer type

// Explicit type resolves ambiguity
f: (int) -> int = x -> x    // OK

// Or annotate the lambda
f = (x: int) -> x           // OK
```

---

## Closures

### What Are Closures?

A closure is a lambda that captures values from its surrounding scope:

```sigil
@make_adder (n: int) -> (int) -> int =
    x -> x + n    // n is captured from the outer scope

@main () -> void = run(
    add5 = make_adder(5),
    add10 = make_adder(10),
    print(add5(3)),    // 8
    print(add10(3))    // 13
)
```

The lambda `x -> x + n` captures the value `n` from `make_adder`.

### Multiple Captured Values

Closures can capture multiple values:

```sigil
@make_linear_function (slope: int, intercept: int) -> (int) -> int =
    x -> slope * x + intercept    // captures both slope and intercept

@main () -> void = run(
    f = make_linear_function(2, 3),
    print(f(5))    // 2 * 5 + 3 = 13
)
```

### Capturing from Enclosing Scope

```sigil
@filter_above (items: [int], threshold: int) -> [int] =
    filter(items, x -> x > threshold)    // captures threshold

@main () -> void = run(
    numbers = [1, 5, 3, 8, 2, 9],
    above_five = filter_above(numbers, 5),  // [8, 9]
    print(above_five)
)
```

---

## Capture by Value

### Semantics

Sigil captures variables by value (copy):

```sigil
@main () -> void = run(
    n = 5,
    f = x -> x + n,    // n is COPIED into the closure
    // Even if n could change, f's captured n remains 5
    print(f(10))       // 15
)
```

### Why Capture by Value?

1. **Safety** — No aliasing issues with captured references
2. **Simplicity** — Values are copied at closure creation time
3. **Immutability** — Aligns with Sigil's immutable-by-default philosophy
4. **Predictability** — Closure behavior doesn't change if outer scope changes

### Capture Timing

Values are captured when the closure is created:

```sigil
@make_closures () -> [(int) -> int] = run(
    closures = [],
    closures = closures + [x -> x + 1],  // captures nothing special
    closures = closures + [x -> x + 2],
    closures = closures + [x -> x + 3],
    closures
)
```

Each closure captures the literal value from its definition.

---

## Closures in Patterns

### With `map`

```sigil
@scale_all (items: [int], factor: int) -> [int] =
    map(items, x -> x * factor)    // captures factor

@main () -> void = run(
    scaled = scale_all([1, 2, 3], 10),  // [10, 20, 30]
    print(scaled)
)
```

### With `filter`

```sigil
@keep_above (items: [int], threshold: int) -> [int] =
    filter(items, x -> x > threshold)    // captures threshold

@main () -> void = run(
    big = keep_above([1, 5, 3, 8], 4),  // [5, 8]
    print(big)
)
```

### With `fold`

```sigil
@weighted_sum (items: [int], weight: int) -> int =
    fold(items, 0, (acc, x) -> acc + x * weight)    // captures weight

@main () -> void = run(
    total = weighted_sum([1, 2, 3], 2),  // 0 + 2 + 4 + 6 = 12
    print(total)
)
```

---

## Returning Closures

Functions can return closures:

```sigil
@make_adder (n: int) -> (int) -> int =
    x -> x + n

@make_multiplier (n: int) -> (int) -> int =
    x -> x * n

@make_comparator (threshold: int) -> (int) -> bool =
    x -> x > threshold

@main () -> void = run(
    add5 = make_adder(5),
    times3 = make_multiplier(3),
    is_big = make_comparator(100),

    print(add5(10)),      // 15
    print(times3(10)),    // 30
    print(is_big(50)),    // false
    print(is_big(150))    // true
)
```

### Closure Factories

```sigil
type Validator = (str) -> bool

@make_length_validator (min: int, max: int) -> Validator =
    s -> s.len >= min && s.len <= max

@make_pattern_validator (pattern: str) -> Validator =
    s -> s.contains(pattern)

@main () -> void = run(
    valid_username = make_length_validator(3, 20),
    valid_email = make_pattern_validator("@"),

    print(valid_username("ab")),     // false (too short)
    print(valid_username("alice")),  // true
    print(valid_email("test@x.com")) // true
)
```

---

## Type Inference in Lambdas

### How Inference Works

Lambda parameter types are inferred from context:

```sigil
// map expects (T) -> U where T is the element type
doubled = map([1, 2, 3], x -> x * 2)
//             ^^^^^^^^    ^ x is inferred as int
```

### Inference Chain

Types flow through expressions:

```sigil
@main () -> void = run(
    numbers = [1, 2, 3],                    // [int]
    doubled = map(numbers, x -> x * 2),     // x: int, result: [int]
    filtered = filter(doubled, x -> x > 3), // x: int, result: [int]
    sum = fold(filtered, 0, (a, b) -> a + b) // a: int, b: int, result: int
)
```

### Bidirectional Inference

Types can flow both ways:

```sigil
// Type annotation on variable flows to lambda
f: (int) -> int = x -> x * 2    // x is int

// Type from usage flows to lambda
items: [int] = map(["1", "2", "3"], s -> parse_int(s))
// s is str (from input), result is int (from annotation)
```

### When Inference Fails

```sigil
// Ambiguous: no context to determine x's type
f = x -> x    // ERROR

// Solutions:
f: (int) -> int = x -> x     // annotate variable
f = (x: int) -> x            // annotate parameter
```

---

## Lambda Body Expressions

### Simple Expressions

```sigil
x -> x * 2
x -> x.len
x -> x > 0
```

### Conditional Expressions

```sigil
x -> if x > 0 then x else -x

(a, b) -> if a > b then a else b
```

### Pattern Matching

```sigil
opt -> match(opt,
    Some(x) -> x,
    None -> 0
)

result -> match(result,
    Ok(value) -> value.to_string(),
    Err(e) -> "Error: " + e.message
)
```

### Using `run` for Complex Bodies

```sigil
item -> run(
    validated = validate(item),
    transformed = transform(validated),
    finalize(transformed)
)
```

---

## Multi-line Lambdas

For complex lambdas, spread across multiple lines:

```sigil
@process_items (items: [Item]) -> [Result<EnrichedItem, Error>] =
    map(items, item -> run(
        validated = validate(item),
        enriched = enrich(validated),
        Ok(enriched)
    ))

// Or with patterns
@categorize (items: [int]) -> [(int, str)] =
    map(items, x ->
        if x < 0 then (x, "negative")
        else if x == 0 then (x, "zero")
        else (x, "positive")
    )
```

---

## Common Lambda Patterns

### Identity

```sigil
x -> x
```

### Constant

```sigil
_ -> 42        // ignores input, always returns 42
() -> config   // deferred constant
```

### Projection (Field Access)

```sigil
user -> user.name
point -> point.x
item -> item.price
```

### Predicate

```sigil
x -> x > 0
s -> s.len < 10
user -> user.is_active
```

### Transformation

```sigil
x -> x * 2
s -> s.upper()
item -> Item { id: item.id, name: item.name, processed: true }
```

### Combining Values

```sigil
(a, b) -> a + b
(x, y) -> Point { x: x, y: y }
(name, value) -> name + "=" + str(value)
```

---

## Lambdas vs Named Functions

### When to Use Lambdas

- Short, one-off transformations
- When the logic is simple and obvious
- When capturing values from the enclosing scope

```sigil
// Good: clear and concise
doubled = map(items, x -> x * 2)
above_five = filter(items, x -> x > 5)
```

### When to Use Named Functions

- Complex logic requiring documentation
- Reusable across multiple call sites
- When testing the function in isolation

```sigil
// Good: reusable and testable
@is_valid_email (email: str) -> bool =
    email.contains("@") && email.contains(".")

// Can be tested separately and reused
valid_emails = filter(emails, is_valid_email)
```

### Hybrid Approach

```sigil
// Define complex logic as named function
@calculate_score (user: User) -> int = run(
    base = user.level * 100,
    bonus = if user.is_premium then 50 else 0,
    base + bonus
)

// Use in lambda for additional transformation
ranked = map(users, u -> (u, calculate_score(u)))
```

---

## Nested Lambdas

Lambdas can contain other lambdas:

```sigil
@make_mapper<T, U> (f: (T) -> U) -> ([T]) -> [U] =
    items -> map(items, f)

@main () -> void = run(
    double_all = make_mapper(x -> x * 2),
    result = double_all([1, 2, 3])  // [2, 4, 6]
)
```

### Caution with Nesting

Deep nesting can harm readability:

```sigil
// Hard to read
result = map(items, x -> map(x.children, c -> filter(c.items, i -> i.active)))

// Better: use named functions or break into steps
@get_active_items (children: [Child]) -> [[Item]] =
    map(children, c -> filter(c.items, i -> i.active))

result = map(items, x -> get_active_items(x.children))
```

---

## Best Practices

### Keep Lambdas Short

```sigil
// Good: simple and readable
map(items, x -> x * 2)

// Consider named function for complex logic
@process (item: Item) -> Result<EnrichedItem, Error> = run(
    validated = validate(item),
    transformed = transform(validated),
    enriched = enrich(transformed),
    Ok(enriched)
)
result = map(items, process)
```

### Use Descriptive Parameter Names

```sigil
// Good: clear meaning
filter(users, user -> user.is_active)
map(orders, order -> order.total)

// OK for simple cases
map([1, 2, 3], x -> x * 2)
```

### Avoid Capturing Mutable State

```sigil
// Sigil is immutable by default, so this is natural
@make_counter_bad () -> (() -> int) = run(
    count = 0,
    // Can't mutate count anyway
    () -> count
)

// Use explicit state if needed
type Counter = { value: int }
@increment (c: Counter) -> Counter = Counter { value: c.value + 1 }
```

### Leverage Type Inference

```sigil
// Let the compiler infer types when obvious
doubled = map([1, 2, 3], x -> x * 2)

// Add types when it helps clarity or when required
transform: (User) -> str = user -> user.name + " <" + user.email + ">"
```

---

## Summary

| Syntax | Meaning |
|--------|---------|
| `x -> expr` | Single parameter lambda |
| `(a, b) -> expr` | Multi-parameter lambda |
| `() -> expr` | No-parameter lambda |
| `(x: int) -> expr` | Explicit parameter type |
| `x -> x + n` | Closure (captures `n`) |

---

## See Also

- [Function Definitions](01-function-definitions.md)
- [First-Class Functions](02-first-class-functions.md)
- [Higher-Order Functions](04-higher-order.md)
- [Type Inference](../03-type-system/05-type-inference.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)

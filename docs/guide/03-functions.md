---
title: "Functions"
description: "Function definitions, generics, lambdas, and closures."
order: 3
---

# Functions

Functions are the building blocks of Ori programs. This guide covers everything from basic function definitions to advanced features like generics, pattern matching in parameters, and closures.

## Basic Functions

Functions use `@` for the name and specify parameters and return type:

```ori
@add (a: int, b: int) -> int = a + b;

@greet (name: str) -> str = `Hello, {name}!`;

@is_adult (age: int) -> bool = age >= 18;
```

The structure is:
- `@name` — function name (the `@` is part of the declaration, not the name)
- `(params)` — parameters with types
- `-> Type` — return type
- `= expression` — function body

### Calling Functions

**All arguments must be named:**

```ori
let sum = add(a: 5, b: 3);
let message = greet(name: "Alice");
let adult = is_adult(age: 25);
```

**Order doesn't matter:**

```ori
let sum = add(b: 3, a: 5);  // Same as add(a: 5, b: 3)
```

### Multi-Expression Functions

When a function needs multiple steps, use a block `{ }`:

```ori
@calculate_total (items: [Item]) -> float = {
    let subtotal = sum_prices(items: items);
    let tax = subtotal * 0.08;
    let discount = calculate_discount(subtotal: subtotal);

    subtotal + tax - discount
}
```

Statements are terminated with `;`. The last expression (without `;`) is the block's value.

## Named Arguments

Ori requires named arguments for all function calls. This design choice has several benefits:

### Self-Documentation

Compare:

```
// What do these mean?
create_user("Alice", 30, true, false)

// Clear and self-documenting
create_user(name: "Alice", age: 30, admin: true, verified: false)
```

### Argument Order Independence

```ori
// These are equivalent
send_email(to: alice, from: bob, subject: "Hello");
send_email(subject: "Hello", from: bob, to: alice);
```

### Evaluation Order

Arguments are evaluated left-to-right as written:

```ori
// Evaluation order: compute_to(), then compute_from()
send_email(to: compute_to(), from: compute_from());
```

### Exception: Lambda Literals

Single-parameter functions allow positional arguments when passing lambda literals directly:

```ori
items.map(x -> x * 2);           // OK: lambda literal
items.filter(x -> x > 0);        // OK: lambda literal

let double = x -> x * 2;
items.map(transform: double);    // Named required for function reference
```

## Default Parameter Values

Parameters can have default values:

```ori
@greet (name: str, greeting: str = "Hello") -> str =
    `{greeting}, {name}!`;

greet(name: "Alice");                        // "Hello, Alice!"
greet(name: "Alice", greeting: "Hi");        // "Hi, Alice!"
greet(greeting: "Hey", name: "Bob");         // "Hey, Bob!"
```

### Multiple Defaults

Defaults can appear at any position:

```ori
@configure (host: str = "localhost", port: int = 8080, secure: bool = false) -> Config =
    Config { host, port, secure };

configure();                           // All defaults
configure(port: 3000);                 // Override just port
configure(secure: true, host: "api");  // Override two
```

### Default Expression Evaluation

Default expressions are evaluated at call time, not definition time:

```ori
@log_with_time (msg: str, time: str = Clock.now() as str) -> void =
    print(msg: `[{time}] {msg}`);

// Each call gets a fresh timestamp
log_with_time(msg: "First");   // [10:00:01] First
log_with_time(msg: "Second");  // [10:00:02] Second
```

Default expressions cannot reference other parameters.

## Multi-Step Functions

When a function has multiple steps, use a block `{ }`:

```ori
@process_order (order: Order) -> Receipt = {
    let validated = validate(order: order);
    let priced = calculate_price(order: validated);
    let receipt = generate_receipt(order: priced);

    receipt
}
```

### Scope in Blocks

Each binding is visible to subsequent expressions:

```ori
{
    let a = 10;              // a is defined
    let b = a + 5;           // a is visible here
    let c = a + b;           // both a and b visible

    c                        // a, b, c all visible (block value)
}
```

### Return Value

The last expression becomes the return value:

```ori
@example () -> int = {
    let x = 10;
    let y = 20;

    x + y      // This is returned (30)
}
```

## Generic Functions

Functions can work with multiple types using generics:

```ori
@first<T> (items: [T]) -> Option<T> =
    if is_empty(collection: items) then None else Some(items[0]);

@swap<T> (pair: (T, T)) -> (T, T) = (pair.1, pair.0);

@identity<T> (value: T) -> T = value;
```

### Calling Generic Functions

Type parameters are usually inferred:

```ori
let n = first(items: [1, 2, 3]);           // T inferred as int
let s = first(items: ["a", "b", "c"]);     // T inferred as str
```

### Trait Bounds

Constrain type parameters with trait bounds:

```ori
@max<T: Comparable> (a: T, b: T) -> T =
    if a > b then a else b;

@sum<T: Add + Default> (items: [T]) -> T =
    items.fold(initial: T.default(), op: (acc, x) -> acc + x);
```

### Multiple Type Parameters

```ori
@pair<A, B> (first: A, second: B) -> (A, B) = (first, second);

@map_pair<A, B, C> (pair: (A, B), f: (A) -> C) -> (C, B) =
    (f(pair.0), pair.1);
```

### Multiple Bounds

Use `+` for multiple bounds:

```ori
@sort_and_print<T: Comparable + Printable> (items: [T]) -> void = {
    let sorted = items.sort();
    for item in sorted do print(msg: item.to_str());
}
```

### Where Clauses

For complex bounds, use `where`:

```ori
@process<T, U> (input: T, transform: (T) -> U) -> [U]
    where T: Clone,
          U: Printable + Default = {
    let items = [input.clone(), input.clone()];

    for item in items yield transform(item)
}
```

## Function Clauses

Define functions with multiple patterns:

```ori
@factorial (0: int) -> int = 1;
@factorial (n) -> int = n * factorial(n: n - 1);
```

### How Clauses Work

1. The first clause establishes the function signature
2. Subsequent clauses match in order
3. Types can be omitted in later clauses (inherited from first)
4. First matching clause wins

```ori
@describe (0: int) -> str = "zero";
@describe (1: int) -> str = "one";
@describe (n) -> str if n < 0 = "negative";
@describe (n) -> str = "many";
```

### Guards

Add conditions with `if`:

```ori
@classify (n: int) -> str if n < 0 = "negative";
@classify (n: int) -> str if n == 0 = "zero";
@classify (n: int) -> str = "positive";
```

### Exhaustiveness

The compiler warns about unreachable clauses and ensures all cases are covered:

```ori
// WARNING: unreachable clause
@process (0: int) -> int = 0;
@process (_: int) -> int = 1;
@process (5: int) -> int = 5;  // Never reached!
```

## Lambdas

Anonymous functions for short operations:

```ori
// Full form
let double = (x: int) -> int = x * 2;

// Short form (types inferred)
let double = x -> x * 2;

// Multiple parameters
let add = (a, b) -> a + b;

// No parameters
let get_time = () -> Clock.now();
```

### Lambda Syntax Variations

```ori
// Single parameter, type inferred
x -> x + 1

// Multiple parameters
(a, b) -> a + b

// With explicit types
(x: int, y: int) -> int = x + y

// Multi-line body
x -> {
    let doubled = x * 2;
    let formatted = `value: {doubled}`;

    formatted
}
```

### Using Lambdas

Lambdas are commonly used with higher-order functions:

```ori
let numbers = [1, 2, 3, 4, 5];

let doubled = numbers.map(x -> x * 2);           // [2, 4, 6, 8, 10]
let evens = numbers.filter(x -> x % 2 == 0);     // [2, 4]
let sum = numbers.fold(
    initial: 0,
    op: (acc, x) -> acc + x,
);                                               // 15
```

## Closures

Lambdas capture variables from their environment:

```ori
let multiplier = 3;
let multiply = x -> x * multiplier;

multiply(5);  // 15
```

### Capture by Value

**Important:** Closures capture **by value**, not by reference:

```ori
let x = 10;
let f = () -> x;    // f captures x = 10

let x = 20;         // This creates a new binding (shadowing)
f();                // Still returns 10
```

This design prevents reference cycles and makes closures safe to pass around.

### Capturing Multiple Values

```ori
let a = 10;
let b = 20;
let compute = () -> a + b;

compute();  // 30
```

### Closures and Mutability

Because closures capture by value, they get a snapshot:

```ori
let values: [int] = [];
let closures = for i in 0..3 yield () -> i;

// Each closure captured its own 'i' value
for closure in closures do
    print(msg: `{closure()}`);
// Prints: 0, 1, 2
```

## Visibility

Control what's accessible from other modules:

### Public Functions

Use `pub` to make functions accessible:

```ori
pub @public_function (x: int) -> int = x + 1;

@private_function (x: int) -> int = x - 1;
```

### Visibility Rules

| Modifier | Visibility |
|----------|------------|
| (none) | Private to current module |
| `pub` | Public, importable by other modules |

```ori
// In math.ori
pub @add (a: int, b: int) -> int = a + b;
@internal_helper (x: int) -> int = x * 2;  // Private

// In main.ori
use "./math" { add };          // OK
use "./math" { internal_helper };  // ERROR: private
```

## Tail Call Optimization

Ori guarantees tail call optimization for recursive functions:

```ori
@countdown (n: int) -> void =
    if n <= 0 then () else countdown(n: n - 1);

countdown(n: 1000000);  // No stack overflow
```

A call is in tail position if it's the last thing before the function returns:

```ori
// Tail recursive - optimized
@sum_tail (n: int, acc: int) -> int =
    if n == 0 then acc else sum_tail(n: n - 1, acc: acc + n);

// NOT tail recursive - builds up stack
@sum_regular (n: int) -> int =
    if n == 0 then 0 else n + sum_regular(n: n - 1);
```

## Tests Are Required

Every function needs at least one test:

```ori
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 2, b: 3), expected: 5);
    assert_eq(actual: add(a: -1, b: 1), expected: 0);
    assert_eq(actual: add(a: 0, b: 0), expected: 0);
}
```

### Testing Multiple Clauses

When using function clauses, test each pattern:

```ori
@factorial (0: int) -> int = 1;
@factorial (n) -> int = n * factorial(n: n - 1);

@test_factorial tests @factorial () -> void = {
    assert_eq(actual: factorial(n: 0), expected: 1);
    assert_eq(actual: factorial(n: 1), expected: 1);
    assert_eq(actual: factorial(n: 5), expected: 120);
}
```

## Complete Example

Here's a practical example combining multiple concepts:

```ori
// A generic function with bounds
@find_max<T: Comparable + Clone> (items: [T]) -> Option<T> =
    if is_empty(collection: items) then
        None
    else
        Some(items.fold(
            initial: items[0].clone(),
            op: (max, x) -> if x > max then x else max,
        ));

@test_find_max tests @find_max () -> void = {
    assert_eq(actual: find_max(items: [3, 1, 4, 1, 5]), expected: Some(5));
    assert_eq(actual: find_max(items: [1]), expected: Some(1));
    assert_eq(actual: find_max(items: [] as [int]), expected: None);
}

// Function with multiple clauses
@fibonacci (0: int) -> int = 0;
@fibonacci (1: int) -> int = 1;
@fibonacci (n) -> int = fibonacci(n: n - 1) + fibonacci(n: n - 2);

@test_fibonacci tests @fibonacci () -> void = {
    assert_eq(actual: fibonacci(n: 0), expected: 0);
    assert_eq(actual: fibonacci(n: 1), expected: 1);
    assert_eq(actual: fibonacci(n: 10), expected: 55);
}

// Higher-order function with closure
@create_multiplier (factor: int) -> (int) -> int =
    x -> x * factor;

@test_multiplier tests @create_multiplier () -> void = {
    let double = create_multiplier(factor: 2);
    let triple = create_multiplier(factor: 3);
    assert_eq(actual: double(5), expected: 10);
    assert_eq(actual: triple(5), expected: 15);
}

// Function with default parameters
@repeat (text: str, times: int = 1, separator: str = "") -> str =
    (for _ in 0..times yield text).join(sep: separator);

@test_repeat tests @repeat () -> void = {
    assert_eq(actual: repeat(text: "hi"), expected: "hi");
    assert_eq(actual: repeat(text: "hi", times: 3), expected: "hihihi");
    assert_eq(actual: repeat(text: "hi", times: 3, separator: "-"), expected: "hi-hi-hi");
}
```

## Quick Reference

### Function Definitions

```ori
@name (param: Type) -> Return = expr;
@name (param: Type = default) -> Return = expr;
@name<T> (param: T) -> T = expr;
@name<T: Bound> (param: T) -> T = expr;
@name<T: A + B> (param: T) -> T = expr;
@name<T> (param: T) -> T where T: Clone = expr;
pub @name (param: Type) -> Return = expr;
```

### Function Clauses

```ori
@fn (0: int) -> int = 1;
@fn (n) -> int = n * fn(n: n - 1);
@fn (n: int) -> int if n < 0 = -n;
```

### Lambdas

```ori
x -> x + 1
(a, b) -> a + b
() -> 42
(x: int) -> int = x * 2
```

### Testing

```ori
@test_name tests @target () -> void = {
    assert_eq(actual: target(x: 1), expected: 2);
}
```

## What's Next

Now that you understand functions:

- **[Collections](/guide/04-collections)** — Lists, maps, sets, and functional operations
- **[Custom Types](/guide/05-custom-types)** — Structs and sum types

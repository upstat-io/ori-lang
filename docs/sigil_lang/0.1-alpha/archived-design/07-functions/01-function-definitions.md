# Function Definitions

This document covers how to define functions in Sigil, including syntax, parameters, return types, visibility, and generics.

---

## Basic Syntax

Functions in Sigil are defined with the `@` prefix:

```sigil
@function_name (parameters) -> return_type = expression
```

### Components

- `@` — function sigil (required)
- `function_name` — identifier in snake_case
- `(parameters)` — parameter list (may be empty)
- `-> return_type` — return type annotation
- `= expression` — function body (single expression)

### Example

```sigil
@add (left: int, right: int) -> int = left + right

@greet (name: str) -> str = "Hello, " + name + "!"

@is_even (number: int) -> bool = number % 2 == 0
```

---

## The `@` Sigil

Sigil uses the `@` prefix to make function definitions visually distinct:

```sigil
@double (number: int) -> int = number * 2
@square (number: int) -> int = number * number
@negate (number: int) -> int = -number
```

### Why `@`?

1. **Visual distinction** — Functions are immediately recognizable
2. **Consistency** — Used in both definition and reference contexts in documentation
3. **AI-friendly** — Clear pattern for code generation
4. **Namespace clarity** — Distinguishes functions from variables and types

### Calling Functions

When calling a function, the `@` prefix is not used:

```sigil
// Returns 10
let result = double(
    .number: 5,
)
// Returns "Hello, World!"
let greeting = greet(
    .name: "World",
)
// Returns true
let check = is_even(
    .number: 4,
)
```

---

## Parameters

### Parameter Syntax

Each parameter has a name and type:

```sigil
@function (name: type) -> return_type = ...
```

### Multiple Parameters

Separate parameters with commas:

```sigil
@add (left: int, right: int) -> int = left + right

@substring (text: str, start: int, length: int) -> str = ...

@clamp (value: int, min: int, max: int) -> int =
    if value < min then min
    else if value > max then max
    else value
```

### No Parameters

Empty parentheses for functions with no parameters:

```sigil
@get_timestamp () -> int = ...

@random () -> float = ...

@get_default_config () -> Config = Config { timeout: 30, retries: 3 }
```

### Parameter Names

Use descriptive names:

```sigil
// Good: meaningful names
@calculate_area (width: int, height: int) -> int = width * height

// Avoid: single letters without context
@calculate_area (w: int, h: int) -> int = w * h

// OK: descriptive names for binary operations
@add (left: int, right: int) -> int = left + right
```

---

## Return Types

### Required Return Types

All functions must declare their return type:

```sigil
@double (number: int) -> int = number * 2
@is_positive (number: int) -> bool = number > 0
@greet (name: str) -> str = "Hello, " + name
```

### Void Return Type

Functions that return nothing use `void`:

```sigil
@print_message (msg: str) -> void = print(msg)

@log_error (error: Error) -> void = run(
    print("ERROR: " + error.message),
    write_log(error),
)
```

### Complex Return Types

Functions can return any type:

```sigil
// Lists
@double_all (items: [int]) -> [int] = map(
    .over: items,
    .transform: item -> item * 2,
)

// Structs
@create_point (x: int, y: int) -> Point = Point { x: x, y: y }

// Option
@find_first (items: [int], predicate: (int) -> bool) -> Option<int> = ...

// Result
@parse_int (input: str) -> Result<int, ParseError> = ...

// Functions
@make_adder (amount: int) -> (int) -> int = value -> value + amount
```

---

## Expression Bodies

### Single Expression

Function bodies are single expressions:

```sigil
@add (left: int, right: int) -> int = left + right
```

### Complex Expressions

Use patterns for complex logic:

```sigil
// Conditional
@abs (number: int) -> int = if number < 0 then -number else number

// Pattern matching
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete",
)

// Sequential operations with run
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)
```

### Multi-line Formatting

For readability, split long expressions across lines:

```sigil
@calculate_score (user: User, activity: Activity) -> int =
    user.base_score + activity.points * user.multiplier

@validate_input (input: Input) -> Result<Data, Error> = try(
    let name = validate_name(input.name),
    let age = validate_age(input.age),
    let email = validate_email(input.email),
    Ok(Data { name: name, age: age, email: email }),
)
```

### Line Continuation

Lines naturally continue after operators, opening brackets, and commas:

```sigil
@is_valid (user: User) -> bool =
    if user.age >= 18 &&
       user.email.contains("@") &&
       user.name.len() > 0 then true
    else false
```

---

## Visibility

### Private by Default

Functions are private by default:

```sigil
// private to this module
@helper () -> int = 42

// private
@internal_process (data: Data) -> Data = ...
```

### Public Functions

Use `pub` keyword for public visibility:

```sigil
pub @add (left: int, right: int) -> int = left + right

pub @calculate_total (items: [Item]) -> int = fold(
    .over: items,
    .initial: 0,
    .operation: (accumulator, item) -> accumulator + item.price,
)
```

### Visibility Rules

```sigil
// Public: accessible from other modules
pub @public_api () -> Data = ...

// Private: only accessible within this module
@private_helper () -> int = ...

// Public function can use private helpers
pub @process (input: Input) -> Output = run(
    let validated = private_helper(input),
    transform(validated),
)
```

### Module Example

```sigil
// math.si

// Public API
pub @add (left: int, right: int) -> int = left + right
pub @subtract (left: int, right: int) -> int = left - right
pub @multiply (left: int, right: int) -> int = left * right
pub @divide (left: int, right: int) -> Result<int, Error> = safe_divide(left, right)

// Private implementation
@safe_divide (dividend: int, divisor: int) -> Result<int, Error> =
    if divisor == 0 then Err(DivisionByZero)
    else Ok(dividend / divisor)
```

---

## Generic Functions

### Syntax

Type parameters come after the function name:

```sigil
@function_name<T> (param: T) -> T = ...
@function_name<T, U> (left: T, right: U) -> (T, U) = ...
```

### Basic Generic Functions

```sigil
@identity<T> (value: T) -> T = value

@swap<T, U> (pair: (T, U)) -> (U, T) = (pair.1, pair.0)

@first<T> (items: [T]) -> Option<T> =
    if items.is_empty() then None
    else Some(items[0])
```

### Multiple Type Parameters

```sigil
@pair<T, U> (left: T, right: U) -> (T, U) = (left, right)

@map_pair<T, U, V> (pair: (T, T), transform: (T) -> U) -> (U, U) =
    (transform(pair.0), transform(pair.1))
```

### Calling Generic Functions

```sigil
// Type inference (common)
// inferred: identity<int>
result = identity(42)
// inferred: swap<int, str>
swapped = swap((1, "hello"))

// Explicit type arguments (when needed)
result = identity<int>(42)
// need to specify T for empty list
empty_result = first<str>([])
```

### Generic Constraints

Use `where` clauses to constrain type parameters:

```sigil
@sort<T> (items: [T]) -> [T] where T: Comparable = ...

@print_all<T> (items: [T]) -> void where T: Printable =
    map(
        .over: items,
        .transform: item -> print(
            .msg: item.to_string(),
        ),
    )

@max<T> (left: T, right: T) -> T where T: Comparable =
    if left.compare(right) == Greater then left else right
```

### Multiple Constraints

```sigil
@sorted_unique<T> (items: [T]) -> [T] where T: Comparable + Eq = run(
    let sorted = sort(
        .items: items,
    ),
    unique(
        .items: sorted,
    ),
)

@debug_sorted<T> (items: [T]) -> [T] where T: Comparable + Printable = run(
    let sorted = sort(
        .items: items,
    ),
    map(
        .over: sorted,
        .transform: item -> print(
            .msg: item.to_string(),
        ),
    ),
    sorted,
)
```

---

## Capability Dependencies

### The `uses` Clause

Functions that require capabilities (side effects like HTTP, file I/O, etc.) declare them with `uses`:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get("/users/" + id),
    Ok(parse(json)),
)
```

### Syntax

```sigil
@function_name (params) -> ReturnType uses Capability = body
@function_name (params) -> ReturnType uses Cap1, Cap2 = body
```

### Multiple Capabilities

```sigil
@fetch_and_cache (key: str) -> Result<Data, Error> uses Http, Cache = try(
    let cached = Cache.get(key),
    match(cached,
        Some(data) -> Ok(parse(data)),
        None -> run(
            let response = Http.get("/data/" + key),
            Cache.set(key, response),
            Ok(parse(response)),
        ),
    ),
)
```

### Providing Capabilities

Capabilities are provided using `with`...`in`:

```sigil
@main () -> void =
    with Http = RealHttp { base_url: $api_url } in
    with Cache = RedisCache { host: $redis_host } in
    run_app()
```

### Why `uses`?

1. **Testability** — Tests can provide mock implementations
2. **Explicitness** — Clear what effects a function performs
3. **Compile safety** — Missing capabilities are compile errors

See [Capabilities](../14-capabilities/index.md) for complete documentation.

---

## Recursive Functions

### Basic Recursion

Functions can call themselves:

```sigil
@factorial (number: int) -> int =
    if number <= 1 then 1
    else number * factorial(number - 1)
```

### Using the `recurse` Pattern

For explicit recursion with features like memoization:

```sigil
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1),
)

@fibonacci (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: number,
    .step: self(number - 1) + self(number - 2),
    .memo: true,
)
```

### Mutual Recursion

Functions can call each other:

```sigil
@is_even (number: int) -> bool =
    if number == 0 then true
    else is_odd(number - 1)

@is_odd (number: int) -> bool =
    if number == 0 then false
    else is_even(number - 1)
```

---

## Documentation Comments

### Function Documentation

Use `//` comments with special prefixes:

```sigil
// #Adds two integers together
// >add(2, 3) -> 5
// >add(-1, 1) -> 0
@add (left: int, right: int) -> int = left + right

// #Finds the first element matching a predicate
// !Returns None if no element matches
// >find([1,2,3], item -> item > 2) -> Some(3)
// >find([1,2,3], item -> item > 5) -> None
@find<T> (items: [T], predicate: (T) -> bool) -> Option<T> = ...
```

### Documentation Prefixes

| Prefix | Meaning |
|--------|---------|
| `#` | Description |
| `>` | Example (input -> output) |
| `!` | Warning or important note |

### Comprehensive Example

```sigil
// #Safely divides two integers
// #Returns Err if divisor is zero
// !This is integer division; remainders are discarded
// >safe_divide(10, 2) -> Ok(5)
// >safe_divide(10, 3) -> Ok(3)
// >safe_divide(10, 0) -> Err(DivisionByZero)
pub @safe_divide (dividend: int, divisor: int) -> Result<int, MathError> =
    if divisor == 0 then Err(DivisionByZero)
    else Ok(dividend / divisor)
```

---

## Function Naming Conventions

### General Rules

- Use `snake_case` for function names
- Use descriptive, action-oriented names
- Prefer verbs or verb phrases

### Common Patterns

```sigil
// Getters: get_X, X (noun)
@get_user (id: int) -> User = ...
@name (user: User) -> str = user.name

// Predicates: is_X, has_X, can_X
@is_empty<T> (items: [T]) -> bool = ...
@has_permission (user: User, perm: Permission) -> bool = ...
@can_edit (user: User, doc: Document) -> bool = ...

// Transformations: to_X, from_X, as_X
@to_string (number: int) -> str = ...
@from_json (input: str) -> Result<Data, Error> = ...
@as_float (number: int) -> float = ...

// Actions: verb_noun
@create_user (name: str) -> User = ...
@delete_file (path: str) -> Result<void, Error> = ...
@calculate_total (items: [Item]) -> int = ...
```

---

## Best Practices

### Keep Functions Small

```sigil
// Good: focused functions
@validate_email (email: str) -> bool = email.contains("@")
@validate_age (age: int) -> bool = age >= 0 && age <= 150
@validate_name (name: str) -> bool = name.len() > 0

pub @validate_user (user: UserInput) -> bool =
    validate_email(user.email) &&
    validate_age(user.age) &&
    validate_name(user.name)

// Avoid: one large function doing everything
```

### Use Patterns for Complexity

```sigil
// Good: use patterns
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)

// Avoid: complex nested expressions (hard to read even with named args)
@process (items: [int]) -> int = fold(
    .over: filter(
        .over: map(
            .over: items,
            .transform: item -> item * 2,
        ),
        .predicate: item -> item > 10,
    ),
    .initial: 0,
    .operation: +,
)
```

### Explicit Over Implicit

```sigil
// Good: clear parameter names and types
@calculate_shipping (weight: float, distance: int, expedited: bool) -> float = ...

// Good: use structs for many parameters
type ShippingRequest = { weight: float, distance: int, expedited: bool }
@calculate_shipping (request: ShippingRequest) -> float = ...
```

### Test Every Function

Every function (except `@main`) must have tests:

```sigil
// math.si
pub @add (left: int, right: int) -> int = left + right

// _test/math.test.si
use math { add }

@test_add tests @add () -> void = run(
    assert_eq(
        .actual: add(
            .left: 2,
            .right: 3,
        ),
        .expected: 5,
    ),
    assert_eq(
        .actual: add(
            .left: -1,
            .right: 1,
        ),
        .expected: 0,
    ),
)
```

---

## Common Mistakes

### Missing Return Type

```sigil
// ERROR: missing return type
@add (left: int, right: int) = left + right

// Correct
@add (left: int, right: int) -> int = left + right
```

### Missing Parameter Types

```sigil
// ERROR: missing parameter types
@add (left, right) -> int = left + right

// Correct
@add (left: int, right: int) -> int = left + right
```

### Using @ When Calling

```sigil
// ERROR: don't use @ when calling
result = @add(2, 3)

// Correct
result = add(2, 3)
```

### Forgetting pub for API Functions

```sigil
// In library.si
// ERROR: private, can't be imported
@useful_function () -> int = 42

// Correct
pub @useful_function () -> int = 42
```

---

## Summary

| Aspect | Syntax |
|--------|--------|
| Definition | `@name (params) -> type = expr` |
| Public | `pub @name (params) -> type = expr` |
| Generic | `@name<T> (param: T) -> T = expr` |
| Constrained | `@name<T> (...) -> T where T: Trait = ...` |
| Capabilities | `@name (params) -> type uses Cap = expr` |
| No params | `@name () -> type = expr` |
| Void return | `@name (param: type) -> void = expr` |

---

## See Also

- [First-Class Functions](02-first-class-functions.md)
- [Lambdas](03-lambdas.md)
- [Higher-Order Functions](04-higher-order.md)
- [Capabilities](../14-capabilities/index.md) — The `uses` clause
- [Generics](../03-type-system/04-generics.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)

# Higher-Order Functions

This document covers functions that take other functions as arguments or return functions as results, including the core transformation patterns `map`, `filter`, and `fold`, as well as partial application techniques.

---

## What Are Higher-Order Functions?

A higher-order function is a function that either:

1. **Takes one or more functions as arguments**, or
2. **Returns a function as its result**

```ori
// Takes a function as argument
@apply_twice (transform: (int) -> int, value: int) -> int = transform(transform(value))

// Returns a function as result
@make_adder (amount: int) -> (int) -> int = value -> value + amount
```

Higher-order functions are fundamental to Ori's declarative pattern system.

---

## Functions as Arguments

### Basic Pattern

```ori
@apply (transform: (int) -> int, value: int) -> int = transform(value)

@main () -> void = run(
    // 10
    let result = apply(
        .transform: double,
        .value: 5,
    ),
    print(result),
)
```

### With Lambdas

```ori
@transform (function: (int) -> int, value: int) -> int = function(value)

@main () -> void = run(
    // 15
    let result = transform(
        .function: item -> item * 3,
        .value: 5,
    ),
    print(result),
)
```

### Generic Higher-Order Functions

```ori
@apply<T, U> (transform: (T) -> U, value: T) -> U = transform(value)

@main () -> void = run(
    // 10
    let int_result = apply(
        .transform: double,
        .value: 5,
    ),
    // 5
    let str_result = apply(
        .transform: text -> text.len(),
        .value: "hello",
    ),
    print(int_result),
    print(str_result),
)
```

---

## Functions as Return Values

### Basic Pattern

```ori
@make_adder (amount: int) -> (int) -> int =
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

### Function Factories

```ori
type Predicate<T> = (T) -> bool

@make_range_checker (min: int, max: int) -> Predicate<int> =
    value -> value >= min && value <= max

@make_length_checker (max_len: int) -> Predicate<str> =
    text -> text.len() <= max_len

@main () -> void = run(
    let valid_age = make_range_checker(
        .min: 0,
        .max: 150,
    ),
    let valid_name = make_length_checker(.max_len: 100),

    // true
    print(valid_age(25)),
    // false
    print(valid_age(200)),
    // true
    print(valid_name("Alice")),
)
```

### Currying via Closures

```ori
@add (first: int) -> (int) -> int =
    second -> first + second

@main () -> void = run(
    let add5 = add(.first: 5),
    // 8
    let result = add5(3),
    print(result),
)
```

---

## Core Transformation Patterns

### `map` — Transform Each Element

The `map` pattern applies a function to every element:

```ori
// Signature
@map<T, U> (items: [T], transform: (T) -> U) -> [U]

// Examples
// [2, 4, 6]
doubled = map(
    .over: [1, 2, 3],
    .transform: item -> item * 2,
)
// [1, 2, 3]
lengths = map(
    .over: ["a", "bb", "ccc"],
    .transform: text -> text.len(),
)
// [str]
names = map(
    .over: users,
    .transform: user -> user.name,
)
```

#### Using Named Functions with map

```ori
@double (number: int) -> int = number * 2
@square (number: int) -> int = number * number

@main () -> void = run(
    // [2, 4, 6]
    let doubled = map(
        .over: [1, 2, 3],
        .transform: double,
    ),
    // [1, 4, 9]
    let squared = map(
        .over: [1, 2, 3],
        .transform: square,
    ),
)
```

#### Chaining map Operations

```ori
@process (numbers: [int]) -> [str] = run(
    let doubled = map(
        .over: numbers,
        .transform: item -> item * 2,
    ),
    let positive = map(
        .over: doubled,
        .transform: number -> if number < 0 then -number else number,
    ),
    map(
        .over: positive,
        .transform: number -> str(.value: number),
    ),
)
```

---

### `filter` — Select Elements

The `filter` pattern keeps elements matching a predicate:

```ori
// Signature
@filter<T> (items: [T], predicate: (T) -> bool) -> [T]

// Examples
// [2, 4]
evens = filter(
    .over: [1, 2, 3, 4],
    .predicate: item -> item % 2 == 0,
)
// [1, 2]
positive = filter(
    .over: [-1, 0, 1, 2],
    .predicate: item -> item > 0,
)
// active users
active = filter(
    .over: users,
    .predicate: user -> user.is_active,
)
```

#### Using Named Functions with filter

```ori
@is_even (number: int) -> bool = number % 2 == 0
@is_positive (number: int) -> bool = number > 0

@main () -> void = run(
    // [2, 4]
    let evens = filter(
        .over: [1, 2, 3, 4],
        .predicate: is_even,
    ),
    // [1]
    let pos = filter(
        .over: [-1, 0, 1],
        .predicate: is_positive,
    ),
)
```

#### Combining filter with map

```ori
@get_active_user_names (users: [User]) -> [str] = run(
    let active = filter(
        .over: users,
        .predicate: user -> user.is_active,
    ),
    map(
        .over: active,
        .transform: user -> user.name,
    ),
)
```

---

### `fold` — Reduce to Single Value

The `fold` pattern accumulates elements into a single result:

```ori
// Signature
@fold<T, U> (items: [T], initial: U, operation: (U, T) -> U) -> U

// Examples
// 6
sum = fold(
    .over: [1, 2, 3],
    .initial: 0,
    .operation: (accumulator, item) -> accumulator + item,
)
// 24
product = fold(
    .over: [1, 2, 3, 4],
    .initial: 1,
    .operation: (accumulator, item) -> accumulator * item,
)
// "abc"
concat = fold(
    .over: ["a", "b", "c"],
    .initial: "",
    .operation: (accumulator, item) -> accumulator + item,
)
```

#### Using Operators with fold

```ori
// 6
sum = fold(
    .over: [1, 2, 3],
    .initial: 0,
    .operation: +,
)
// 24
product = fold(
    .over: [1, 2, 3, 4],
    .initial: 1,
    .operation: *,
)
```

#### Complex Accumulation

```ori
// Build a running total with index
// Extract the list from the tuple
@running_totals (items: [int]) -> [(int, int)] = fold(
    .over: items,
    .initial: (0, []),
    .operation: (state, item) -> run(
        let (total, results) = state,
        let new_total = total + item,
        (new_total, results ++ [(item, new_total)]),
    ),
).1

@main () -> void = run(
    let totals = running_totals(.items: [1, 2, 3, 4]),
    // [(1, 1), (2, 3), (3, 6), (4, 10)]
    print(totals),
)
```

---

## Combining Patterns

### Map-Filter-Reduce Pipeline

```ori
@process_orders (orders: [Order]) -> int = run(
    // Filter to completed orders
    let completed = filter(
        .over: orders,
        .predicate: order -> order.status == Complete,
    ),

    // Map to order totals
    let totals = map(
        .over: completed,
        .transform: order -> order.total,
    ),

    // Sum all totals
    fold(
        .over: totals,
        .initial: 0,
        .operation: +,
    ),
)
```

### Processing in Steps

```ori
@analyze_users (users: [User]) -> Summary = run(
    // Step 1: filter active users
    let active = filter(
        .over: users,
        .predicate: user -> user.is_active,
    ),

    // Step 2: map to relevant data
    let ages = map(
        .over: active,
        .transform: user -> user.age,
    ),

    // Step 3: compute statistics
    let total = fold(
        .over: ages,
        .initial: 0,
        .operation: +,
    ),
    let count = len(.of: ages),
    let avg = if count > 0 then total / count else 0,

    Summary { total_active: count, average_age: avg },
)
```

---

## Partial Application

### What Is Partial Application?

Partial application creates a new function by fixing some arguments of an existing function.

### Using Lambdas (Ori's Approach)

Ori uses lambdas for partial application:

```ori
@add (left: int, right: int) -> int = left + right

@main () -> void = run(
    // Partial application via lambda
    let add5 = value -> add(.left: 5, .right: value),
    let add10 = value -> add(.left: 10, .right: value),

    // 8
    print(add5(3)),
    // 13
    print(add10(3)),
)
```

### Why Lambdas Instead of Special Syntax?

1. **Explicit** — `value -> add(.left: 5, .right: value)` clearly shows what's happening
2. **Flexible** — Can fix any parameter in any position
3. **Familiar** — LLMs know lambda syntax well
4. **One way** — No confusion between multiple partial application syntaxes

### Partial Application Patterns

```ori
// Fix first argument
@multiply (left: int, right: int) -> int = left * right
let double = value -> multiply(.left: 2, .right: value)
let triple = value -> multiply(.left: 3, .right: value)

// Fix second argument
@divide (dividend: int, divisor: int) -> int = dividend / divisor
let half = value -> divide(.dividend: value, .divisor: 2)
let quarter = value -> divide(.dividend: value, .divisor: 4)

// Fix multiple arguments
@greet (greeting: str, name: str, punctuation: str) -> str =
    greeting + ", " + name + punctuation

let hello_to = name -> greet(
    .greeting: "Hello",
    .name: name,
    .punctuation: "!",
)
let goodbye_to = name -> greet(
    .greeting: "Goodbye",
    .name: name,
    .punctuation: ".",
)
```

### With Higher-Order Functions

```ori
@filter_above (threshold: int) -> ([int]) -> [int] =
    items -> filter(
        .over: items,
        .predicate: item -> item > threshold,
    )

@main () -> void = run(
    let above_five = filter_above(.threshold: 5),
    let above_ten = filter_above(.threshold: 10),

    let numbers = [3, 7, 12, 2, 9, 15],
    // [7, 12, 9, 15]
    let big = above_five(numbers),
    // [12, 15]
    let bigger = above_ten(numbers),
)
```

---

## Function Composition

### Manual Composition

```ori
@double (number: int) -> int = number * 2
@add_one (number: int) -> int = number + 1

@main () -> void = run(
    // Manual composition
    let double_then_add = value -> add_one(.number: double(.number: value)),
    // 11
    let result = double_then_add(5),
)
```

### Compose Function

```ori
@compose<A, B, C> (outer: (B) -> C, inner: (A) -> B) -> (A) -> C =
    value -> outer(inner(value))

@main () -> void = run(
    let double_then_add = compose(
        .outer: add_one,
        .inner: double,
    ),
    // 11
    let result = double_then_add(5),
)
```

### Pipeline Function

```ori
@pipe<A, B, C> (value: A, first: (A) -> B, second: (B) -> C) -> C = second(first(value))

@main () -> void = run(
    // 11
    let result = pipe(
        .value: 5,
        .first: double,
        .second: add_one,
    ),
)
```

### Composing Multiple Functions

```ori
@compose3<A, B, C, D> (
    first: (C) -> D,
    second: (B) -> C,
    third: (A) -> B
) -> (A) -> D =
    value -> first(second(third(value)))

@main () -> void = run(
    @negate (number: int) -> int = -number,
    let transform = compose3(
        .first: add_one,
        .second: double,
        .third: negate,
    ),
    // ((-3) * 2) + 1 = -5
    let result = transform(3),
)
```

---

## Common Higher-Order Functions

### `apply_n` — Apply Function N Times

```ori
@apply_n<T> (transform: (T) -> T, times: int, value: T) -> T =
    if times <= 0 then value
    else apply_n(
        .transform: transform,
        .times: times - 1,
        .value: transform(value),
    )

@main () -> void = run(
    // 2 -> 4 -> 8 -> 16
    let result = apply_n(
        .transform: double,
        .times: 3,
        .value: 2,
    ),
    print(result),
)
```

### `find` — Find First Matching Element

```ori
@find<T> (items: [T], predicate: (T) -> bool) -> Option<T> = match(
    filter(
        .over: items,
        .predicate: predicate,
    ),
    [] -> None,
    [first, ..] -> Some(first),
)

@main () -> void = run(
    // Some(4)
    let first_even = find(
        .items: [1, 3, 4, 5],
        .predicate: item -> item % 2 == 0,
    ),
    print(first_even),
)
```

### `any` / `all` — Check Predicates

```ori
@any<T> (items: [T], predicate: (T) -> bool) -> bool =
    len(.collection: filter(
        .over: items,
        .predicate: predicate,
    )) > 0

@all<T> (items: [T], predicate: (T) -> bool) -> bool =
    len(.collection: filter(
        .over: items,
        .predicate: item -> !predicate(item),
    )) == 0

@main () -> void = run(
    // true
    let has_even = any(
        .items: [1, 3, 4],
        .predicate: item -> item % 2 == 0,
    ),
    // true
    let all_positive = all(
        .items: [1, 2, 3],
        .predicate: item -> item > 0,
    ),
    print(has_even),
    print(all_positive),
)
```

### `partition` — Split by Predicate

```ori
@partition<T> (items: [T], predicate: (T) -> bool) -> ([T], [T]) = (
    filter(
        .over: items,
        .predicate: predicate,
    ),
    filter(
        .over: items,
        .predicate: item -> !predicate(item),
    ),
)

@main () -> void = run(
    let (evens, odds) = partition(
        .items: [1, 2, 3, 4, 5],
        .predicate: item -> item % 2 == 0,
    ),
    // [2, 4]
    print(evens),
    // [1, 3, 5]
    print(odds),
)
```

### `group_by` — Group by Key

```ori
// Partition items into two groups based on predicate
@partition<T> (items: [T], predicate: (T) -> bool) -> ([T], [T]) = fold(
    .over: items,
    .initial: ([], []),
    .operation: (accumulator, item) ->
        if predicate(item) then (accumulator.0 ++ [item], accumulator.1)
        else (accumulator.0, accumulator.1 ++ [item]),
)

@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5, 6],
    let (evens, odds) = partition(
        .items: numbers,
        .predicate: number -> number % 2 == 0,
    ),
    // evens: [2, 4, 6], odds: [1, 3, 5]
    print(evens),
)
```

---

## Higher-Order Functions with Multiple Function Arguments

### `zip_with` — Combine Two Lists with Function

```ori
@zip_with<T, U, V> (
    left_list: [T],
    right_list: [U],
    combiner: (T, U) -> V
) -> [V] = ...

@main () -> void = run(
    let sums = zip_with(
        .left_list: [1, 2, 3],
        .right_list: [4, 5, 6],
        .combiner: (left, right) -> left + right,
    ),
    // [5, 7, 9]
    print(sums),
)
```

### `until` — Apply Until Condition

```ori
@until<T> (predicate: (T) -> bool, transform: (T) -> T, value: T) -> T =
    if predicate(value) then value
    else until(
        .predicate: predicate,
        .transform: transform,
        .value: transform(value),
    )

@main () -> void = run(
    // Keep doubling until > 100
    // 128
    let result = until(
        .predicate: number -> number > 100,
        .transform: number -> number * 2,
        .value: 1,
    ),
    print(result),
)
```

---

## Callbacks and Event Handlers

### Callback Pattern

```ori
type Callback<T> = (T) -> void

@fetch_data (url: str, on_success: Callback<Data>, on_error: Callback<Error>) -> void = ...

@main () -> void = run(
    fetch_data(
        .url: "https://api.example.com/data",
        .on_success: data -> print("Got: " + data.to_string()),
        .on_error: error -> print("Error: " + error.message),
    ),
)
```

### Event Handler Pattern

```ori
type EventHandler<E> = (E) -> void

type Button = {
    label: str,
    on_click: EventHandler<ClickEvent>
}

@create_button (label: str, handler: EventHandler<ClickEvent>) -> Button =
    Button { label: label, on_click: handler }

@main () -> void = run(
    let button = create_button(
        .label: "Submit",
        .handler: event -> run(
            print("Button clicked at: " + str(.value: event.x) + ", " + str(.value: event.y)),
            submit_form(),
        ),
    ),
)
```

---

## Strategy Pattern

### Defining Strategies

```ori
type SortStrategy<T> = ([T]) -> [T]

@bubble_sort<T> (items: [T]) -> [T] where T: Comparable = ...
@quick_sort<T> (items: [T]) -> [T] where T: Comparable = ...
@merge_sort<T> (items: [T]) -> [T] where T: Comparable = ...

@sort_with<T> (items: [T], strategy: SortStrategy<T>) -> [T] =
    strategy(items)

@main () -> void = run(
    let numbers = [3, 1, 4, 1, 5, 9],
    let sorted = sort_with(
        .items: numbers,
        .strategy: quick_sort,
    ),
    print(sorted),
)
```

### Strategy Selection

```ori
@choose_sort_strategy<T> (items: [T]) -> SortStrategy<T> where T: Comparable =
    if len(.collection: items) < 10 then bubble_sort
    else if len(.collection: items) < 1000 then quick_sort
    else merge_sort

@smart_sort<T> (items: [T]) -> [T] where T: Comparable = run(
    let strategy = choose_sort_strategy(.items: items),
    strategy(items),
)
```

---

## Best Practices

### Keep Functions Pure

```ori
// Good: pure function, no side effects
@transform (items: [int], transform: (int) -> int) -> [int] = map(
    .over: items,
    .transform: transform,
)

// Avoid: hidden side effects in function argument
@process (items: [int], transform: (int) -> int) -> [int] = run(
    // If transform has side effects, this becomes unpredictable
    map(
        .over: items,
        .transform: transform,
    )
)
```

### Use Descriptive Names

```ori
// Good: clear intent
@apply_to_each<T, U> (items: [T], transform: (T) -> U) -> [U]
@keep_if<T> (items: [T], predicate: (T) -> bool) -> [T]
@combine_with<T, U> (items: [T], initial: U, reducer: (U, T) -> U) -> U

// Avoid: cryptic names
@f<T, U> (xs: [T], g: (T) -> U) -> [U]
```

### Prefer Small, Focused Functions

```ori
// Good: composable pieces
@is_active (user: User) -> bool = user.status == Active
@get_name (user: User) -> str = user.name
@get_active_names (users: [User]) -> [str] = run(
    let active = filter(
        .over: users,
        .predicate: is_active,
    ),
    map(
        .over: active,
        .transform: get_name,
    ),
)

// Avoid: monolithic function
@get_active_names_bad (users: [User]) -> [str] = map(
    .over: filter(
        .over: users,
        .predicate: user -> user.status == Active,
    ),
    .transform: user -> user.name,
)
```

### Document Function Parameters

```ori
// #Transforms each element in a list using the provided function
// >transform(.items: [1,2,3], .transform: item -> item * 2) -> [2,4,6]
// !The transform function should be pure (no side effects)
@transform<T, U> (items: [T], transform: (T) -> U) -> [U] = map(
    .over: items,
    .transform: transform,
)
```

---

## Summary

| Pattern | Signature | Purpose |
|---------|-----------|---------|
| `map` | `([T], (T) -> U) -> [U]` | Transform each element |
| `filter` | `([T], (T) -> bool) -> [T]` | Select matching elements |
| `fold` | `([T], U, (U, T) -> U) -> U` | Reduce to single value |
| Partial application | Lambda: `value -> function(.left: 5, .right: value)` | Fix some arguments |
| Composition | `compose(.outer: outer_fn, .inner: inner_fn)` | Chain functions |

---

## See Also

- [Function Definitions](01-function-definitions.md)
- [First-Class Functions](02-first-class-functions.md)
- [Lambdas](03-lambdas.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)

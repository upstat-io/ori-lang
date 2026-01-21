# Higher-Order Functions

This document covers functions that take other functions as arguments or return functions as results, including the core transformation patterns `map`, `filter`, and `fold`, as well as partial application techniques.

---

## What Are Higher-Order Functions?

A higher-order function is a function that either:

1. **Takes one or more functions as arguments**, or
2. **Returns a function as its result**

```sigil
// Takes a function as argument
@apply_twice (f: (int) -> int, x: int) -> int = f(f(x))

// Returns a function as result
@make_adder (n: int) -> (int) -> int = x -> x + n
```

Higher-order functions are fundamental to Sigil's declarative pattern system.

---

## Functions as Arguments

### Basic Pattern

```sigil
@apply (f: (int) -> int, x: int) -> int = f(x)

@main () -> void = run(
    let result = apply(
        .f: double,
        .x: 5,
    ),  // 10
    print(.msg: result),
)
```

### With Lambdas

```sigil
@transform (f: (int) -> int, x: int) -> int = f(x)

@main () -> void = run(
    let result = transform(
        .f: x -> x * 3,
        .x: 5,
    ),  // 15
    print(.msg: result),
)
```

### Generic Higher-Order Functions

```sigil
@apply<T, U> (f: (T) -> U, x: T) -> U = f(x)

@main () -> void = run(
    let int_result = apply(
        .f: double,
        .x: 5,
    ),                                           // 10
    let str_result = apply(
        .f: s -> s.len(),
        .x: "hello",
    ),                                           // 5
    print(.msg: int_result),
    print(.msg: str_result),
)
```

---

## Functions as Return Values

### Basic Pattern

```sigil
@make_adder (n: int) -> (int) -> int =
    x -> x + n

@main () -> void = run(
    let add5 = make_adder(.n: 5),
    let add10 = make_adder(.n: 10),
    print(.msg: add5(3)),    // 8
    print(.msg: add10(3)),   // 13
)
```

### Function Factories

```sigil
type Predicate<T> = (T) -> bool

@make_range_checker (min: int, max: int) -> Predicate<int> =
    x -> x >= min && x <= max

@make_length_checker (max_len: int) -> Predicate<str> =
    s -> s.len() <= max_len

@main () -> void = run(
    let valid_age = make_range_checker(
        .min: 0,
        .max: 150,
    ),
    let valid_name = make_length_checker(.max_len: 100),

    print(.msg: valid_age(25)),          // true
    print(.msg: valid_age(200)),         // false
    print(.msg: valid_name("Alice")),    // true
)
```

### Currying via Closures

```sigil
@add (a: int) -> (int) -> int =
    b -> a + b

@main () -> void = run(
    let add5 = add(.a: 5),
    let result = add5(3),  // 8
    print(.msg: result),
)
```

---

## Core Transformation Patterns

### `map` — Transform Each Element

The `map` pattern applies a function to every element:

```sigil
// Signature
@map<T, U> (items: [T], f: (T) -> U) -> [U]

// Examples
doubled = map(
    .over: [1, 2, 3],
    .transform: x -> x * 2,
)                                             // [2, 4, 6]
lengths = map(
    .over: ["a", "bb", "ccc"],
    .transform: s -> s.len(),
)                                             // [1, 2, 3]
names = map(
    .over: users,
    .transform: user -> user.name,
)                                             // [str]
```

#### Using Named Functions with map

```sigil
@double (n: int) -> int = n * 2
@square (n: int) -> int = n * n

@main () -> void = run(
    let doubled = map(
        .over: [1, 2, 3],
        .transform: double,
    ),  // [2, 4, 6]
    let squared = map(
        .over: [1, 2, 3],
        .transform: square,
    ),  // [1, 4, 9]
)
```

#### Chaining map Operations

```sigil
@process (numbers: [int]) -> [str] = run(
    let doubled = map(
        .over: numbers,
        .transform: x -> x * 2,
    ),
    let positive = map(
        .over: doubled,
        .transform: x -> if x < 0 then -x else x,
    ),
    map(
        .over: positive,
        .transform: x -> str(.value: x),
    ),
)
```

---

### `filter` — Select Elements

The `filter` pattern keeps elements matching a predicate:

```sigil
// Signature
@filter<T> (items: [T], pred: (T) -> bool) -> [T]

// Examples
evens = filter(
    .over: [1, 2, 3, 4],
    .predicate: x -> x % 2 == 0,
)                                                  // [2, 4]
positive = filter(
    .over: [-1, 0, 1, 2],
    .predicate: x -> x > 0,
)                                                  // [1, 2]
active = filter(
    .over: users,
    .predicate: user -> user.is_active,
)                                                  // active users
```

#### Using Named Functions with filter

```sigil
@is_even (n: int) -> bool = n % 2 == 0
@is_positive (n: int) -> bool = n > 0

@main () -> void = run(
    let evens = filter(
        .over: [1, 2, 3, 4],
        .predicate: is_even,
    ),    // [2, 4]
    let pos = filter(
        .over: [-1, 0, 1],
        .predicate: is_positive,
    ),    // [1]
)
```

#### Combining filter with map

```sigil
@get_active_user_names (users: [User]) -> [str] = run(
    let active = filter(
        .over: users,
        .predicate: u -> u.is_active,
    ),
    map(
        .over: active,
        .transform: u -> u.name,
    ),
)
```

---

### `fold` — Reduce to Single Value

The `fold` pattern accumulates elements into a single result:

```sigil
// Signature
@fold<T, U> (items: [T], init: U, f: (U, T) -> U) -> U

// Examples
sum = fold(
    .over: [1, 2, 3],
    .init: 0,
    .op: (acc, x) -> acc + x,
)                                                  // 6
product = fold(
    .over: [1, 2, 3, 4],
    .init: 1,
    .op: (acc, x) -> acc * x,
)                                                  // 24
concat = fold(
    .over: ["a", "b", "c"],
    .init: "",
    .op: (s, x) -> s + x,
)                                                  // "abc"
```

#### Using Operators with fold

```sigil
sum = fold(
    .over: [1, 2, 3],
    .init: 0,
    .op: +,
)        // 6
product = fold(
    .over: [1, 2, 3, 4],
    .init: 1,
    .op: *,
)        // 24
```

#### Complex Accumulation

```sigil
// Build a running total with index
@running_totals (items: [int]) -> [(int, int)] = fold(
    .over: items,
    .init: (0, []),
    .op: (state, item) -> run(
        let (total, results) = state,
        let new_total = total + item,
        (new_total, results ++ [(item, new_total)]),
    ),
).1  // Extract the list from the tuple

@main () -> void = run(
    let totals = running_totals(.items: [1, 2, 3, 4]),
    // [(1, 1), (2, 3), (3, 6), (4, 10)]
    print(.msg: totals),
)
```

---

## Combining Patterns

### Map-Filter-Reduce Pipeline

```sigil
@process_orders (orders: [Order]) -> int = run(
    // Filter to completed orders
    let completed = filter(
        .over: orders,
        .predicate: o -> o.status == Complete,
    ),

    // Map to order totals
    let totals = map(
        .over: completed,
        .transform: o -> o.total,
    ),

    // Sum all totals
    fold(
        .over: totals,
        .init: 0,
        .op: +,
    ),
)
```

### Processing in Steps

```sigil
@analyze_users (users: [User]) -> Summary = run(
    // Step 1: filter active users
    let active = filter(
        .over: users,
        .predicate: u -> u.is_active,
    ),

    // Step 2: map to relevant data
    let ages = map(
        .over: active,
        .transform: u -> u.age,
    ),

    // Step 3: compute statistics
    let total = fold(
        .over: ages,
        .init: 0,
        .op: +,
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

### Using Lambdas (Sigil's Approach)

Sigil uses lambdas for partial application:

```sigil
@add (a: int, b: int) -> int = a + b

@main () -> void = run(
    // Partial application via lambda
    let add5 = x -> add(.a: 5, .b: x),
    let add10 = x -> add(.a: 10, .b: x),

    print(.msg: add5(3)),   // 8
    print(.msg: add10(3)),  // 13
)
```

### Why Lambdas Instead of Special Syntax?

1. **Explicit** — `x -> add(.a: 5, .b: x)` clearly shows what's happening
2. **Flexible** — Can fix any parameter in any position
3. **Familiar** — LLMs know lambda syntax well
4. **One way** — No confusion between multiple partial application syntaxes

### Partial Application Patterns

```sigil
// Fix first argument
@multiply (a: int, b: int) -> int = a * b
let double = x -> multiply(.a: 2, .b: x)
let triple = x -> multiply(.a: 3, .b: x)

// Fix second argument
@divide (a: int, b: int) -> int = a / b
let half = x -> divide(.a: x, .b: 2)
let quarter = x -> divide(.a: x, .b: 4)

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

```sigil
@filter_above (threshold: int) -> ([int]) -> [int] =
    items -> filter(
        .over: items,
        .predicate: x -> x > threshold,
    )

@main () -> void = run(
    let above_five = filter_above(.threshold: 5),
    let above_ten = filter_above(.threshold: 10),

    let numbers = [3, 7, 12, 2, 9, 15],
    let big = above_five(numbers),    // [7, 12, 9, 15]
    let bigger = above_ten(numbers),  // [12, 15]
)
```

---

## Function Composition

### Manual Composition

```sigil
@double (n: int) -> int = n * 2
@add_one (n: int) -> int = n + 1

@main () -> void = run(
    // Manual composition
    let double_then_add = x -> add_one(.n: double(.n: x)),
    let result = double_then_add(5),  // 11
)
```

### Compose Function

```sigil
@compose<A, B, C> (f: (B) -> C, g: (A) -> B) -> (A) -> C =
    x -> f(g(x))

@main () -> void = run(
    let double_then_add = compose(
        .f: add_one,
        .g: double,
    ),
    let result = double_then_add(5),  // 11
)
```

### Pipeline Function

```sigil
@pipe<A, B, C> (x: A, f: (A) -> B, g: (B) -> C) -> C = g(f(x))

@main () -> void = run(
    let result = pipe(
        .x: 5,
        .f: double,
        .g: add_one,
    ),  // 11
)
```

### Composing Multiple Functions

```sigil
@compose3<A, B, C, D> (
    f: (C) -> D,
    g: (B) -> C,
    h: (A) -> B
) -> (A) -> D =
    x -> f(g(h(x)))

@main () -> void = run(
    @negate (n: int) -> int = -n,
    let transform = compose3(
        .f: add_one,
        .g: double,
        .h: negate,
    ),
    let result = transform(3),  // ((-3) * 2) + 1 = -5
)
```

---

## Common Higher-Order Functions

### `apply_n` — Apply Function N Times

```sigil
@apply_n<T> (f: (T) -> T, n: int, x: T) -> T =
    if n <= 0 then x
    else apply_n(
        .f: f,
        .n: n - 1,
        .x: f(x),
    )

@main () -> void = run(
    let result = apply_n(
        .f: double,
        .n: 3,
        .x: 2,
    ),  // 2 -> 4 -> 8 -> 16
    print(.msg: result),
)
```

### `find` — Find First Matching Element

```sigil
@find<T> (items: [T], pred: (T) -> bool) -> Option<T> = match(
    filter(
        .over: items,
        .predicate: pred,
    ),
    [] -> None,
    [first, ..] -> Some(first),
)

@main () -> void = run(
    let first_even = find(
        .items: [1, 3, 4, 5],
        .pred: x -> x % 2 == 0,
    ),  // Some(4)
    print(.msg: first_even),
)
```

### `any` / `all` — Check Predicates

```sigil
@any<T> (items: [T], pred: (T) -> bool) -> bool =
    len(.of: filter(
        .over: items,
        .predicate: pred,
    )) > 0

@all<T> (items: [T], pred: (T) -> bool) -> bool =
    len(.of: filter(
        .over: items,
        .predicate: x -> !pred(x),
    )) == 0

@main () -> void = run(
    let has_even = any(
        .items: [1, 3, 4],
        .pred: x -> x % 2 == 0,
    ),   // true
    let all_positive = all(
        .items: [1, 2, 3],
        .pred: x -> x > 0,
    ),    // true
    print(.msg: has_even),
    print(.msg: all_positive),
)
```

### `partition` — Split by Predicate

```sigil
@partition<T> (items: [T], pred: (T) -> bool) -> ([T], [T]) = (
    filter(
        .over: items,
        .predicate: pred,
    ),
    filter(
        .over: items,
        .predicate: x -> !pred(x),
    ),
)

@main () -> void = run(
    let (evens, odds) = partition(
        .items: [1, 2, 3, 4, 5],
        .pred: x -> x % 2 == 0,
    ),
    print(.msg: evens),  // [2, 4]
    print(.msg: odds),   // [1, 3, 5]
)
```

### `group_by` — Group by Key

```sigil
// Partition items into two groups based on predicate
@partition<T> (items: [T], pred: (T) -> bool) -> ([T], [T]) = fold(
    .over: items,
    .init: ([], []),
    .op: (acc, item) ->
        if pred(item) then (acc.0 ++ [item], acc.1)
        else (acc.0, acc.1 ++ [item]),
)

@main () -> void = run(
    let numbers = [1, 2, 3, 4, 5, 6],
    let (evens, odds) = partition(
        .items: numbers,
        .pred: n -> n % 2 == 0,
    ),
    // evens: [2, 4, 6], odds: [1, 3, 5]
    print(.msg: evens),
)
```

---

## Higher-Order Functions with Multiple Function Arguments

### `zip_with` — Combine Two Lists with Function

```sigil
@zip_with<T, U, V> (
    as: [T],
    bs: [U],
    f: (T, U) -> V
) -> [V] = ...

@main () -> void = run(
    let sums = zip_with(
        .as: [1, 2, 3],
        .bs: [4, 5, 6],
        .f: (a, b) -> a + b,
    ),
    // [5, 7, 9]
    print(.msg: sums),
)
```

### `until` — Apply Until Condition

```sigil
@until<T> (pred: (T) -> bool, f: (T) -> T, x: T) -> T =
    if pred(x) then x
    else until(
        .pred: pred,
        .f: f,
        .x: f(x),
    )

@main () -> void = run(
    // Keep doubling until > 100
    let result = until(
        .pred: x -> x > 100,
        .f: x -> x * 2,
        .x: 1,
    ),  // 128
    print(.msg: result),
)
```

---

## Callbacks and Event Handlers

### Callback Pattern

```sigil
type Callback<T> = (T) -> void

@fetch_data (url: str, on_success: Callback<Data>, on_error: Callback<Error>) -> void = ...

@main () -> void = run(
    fetch_data(
        .url: "https://api.example.com/data",
        .on_success: data -> print(.msg: "Got: " + data.to_string()),
        .on_error: err -> print(.msg: "Error: " + err.message),
    ),
)
```

### Event Handler Pattern

```sigil
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
            print(.msg: "Button clicked at: " + str(.value: event.x) + ", " + str(.value: event.y)),
            submit_form(),
        ),
    ),
)
```

---

## Strategy Pattern

### Defining Strategies

```sigil
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
    print(.msg: sorted),
)
```

### Strategy Selection

```sigil
@choose_sort_strategy<T> (items: [T]) -> SortStrategy<T> where T: Comparable =
    if len(.of: items) < 10 then bubble_sort
    else if len(.of: items) < 1000 then quick_sort
    else merge_sort

@smart_sort<T> (items: [T]) -> [T] where T: Comparable = run(
    let strategy = choose_sort_strategy(.items: items),
    strategy(items),
)
```

---

## Best Practices

### Keep Functions Pure

```sigil
// Good: pure function, no side effects
@transform (items: [int], f: (int) -> int) -> [int] = map(
    .over: items,
    .transform: f,
)

// Avoid: hidden side effects in function argument
@process (items: [int], f: (int) -> int) -> [int] = run(
    // If f has side effects, this becomes unpredictable
    map(
        .over: items,
        .transform: f,
    )
)
```

### Use Descriptive Names

```sigil
// Good: clear intent
@apply_to_each<T, U> (items: [T], transform: (T) -> U) -> [U]
@keep_if<T> (items: [T], predicate: (T) -> bool) -> [T]
@combine_with<T, U> (items: [T], initial: U, reducer: (U, T) -> U) -> U

// Avoid: cryptic names
@f<T, U> (xs: [T], g: (T) -> U) -> [U]
```

### Prefer Small, Focused Functions

```sigil
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
        .predicate: u -> u.status == Active,
    ),
    .transform: u -> u.name,
)
```

### Document Function Parameters

```sigil
// #Transforms each element in a list using the provided function
// >transform(.items: [1,2,3], .f: x -> x * 2) -> [2,4,6]
// !The transform function should be pure (no side effects)
@transform<T, U> (items: [T], f: (T) -> U) -> [U] = map(
    .over: items,
    .transform: f,
)
```

---

## Summary

| Pattern | Signature | Purpose |
|---------|-----------|---------|
| `map` | `([T], (T) -> U) -> [U]` | Transform each element |
| `filter` | `([T], (T) -> bool) -> [T]` | Select matching elements |
| `fold` | `([T], U, (U, T) -> U) -> U` | Reduce to single value |
| Partial application | Lambda: `x -> f(5, x)` | Fix some arguments |
| Composition | `compose(f, g)` | Chain functions |

---

## See Also

- [Function Definitions](01-function-definitions.md)
- [First-Class Functions](02-first-class-functions.md)
- [Lambdas](03-lambdas.md)
- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)

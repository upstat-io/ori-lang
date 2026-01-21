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
    result = apply(double, 5),  // 10
    print(result)
)
```

### With Lambdas

```sigil
@transform (f: (int) -> int, x: int) -> int = f(x)

@main () -> void = run(
    result = transform(x -> x * 3, 5),  // 15
    print(result)
)
```

### Generic Higher-Order Functions

```sigil
@apply<T, U> (f: (T) -> U, x: T) -> U = f(x)

@main () -> void = run(
    int_result = apply(double, 5),           // 10
    str_result = apply(s -> s.len, "hello"), // 5
    print(int_result),
    print(str_result)
)
```

---

## Functions as Return Values

### Basic Pattern

```sigil
@make_adder (n: int) -> (int) -> int =
    x -> x + n

@main () -> void = run(
    add5 = make_adder(5),
    add10 = make_adder(10),
    print(add5(3)),    // 8
    print(add10(3))    // 13
)
```

### Function Factories

```sigil
type Predicate<T> = (T) -> bool

@make_range_checker (min: int, max: int) -> Predicate<int> =
    x -> x >= min && x <= max

@make_length_checker (max_len: int) -> Predicate<str> =
    s -> s.len <= max_len

@main () -> void = run(
    valid_age = make_range_checker(0, 150),
    valid_name = make_length_checker(100),

    print(valid_age(25)),          // true
    print(valid_age(200)),         // false
    print(valid_name("Alice"))     // true
)
```

### Currying via Closures

```sigil
@add (a: int) -> (int) -> int =
    b -> a + b

@main () -> void = run(
    add5 = add(5),
    result = add5(3),  // 8
    print(result)
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
doubled = map([1, 2, 3], x -> x * 2)          // [2, 4, 6]
lengths = map(["a", "bb", "ccc"], s -> s.len) // [1, 2, 3]
names = map(users, user -> user.name)         // [str]
```

#### Using Named Functions with map

```sigil
@double (n: int) -> int = n * 2
@square (n: int) -> int = n * n

@main () -> void = run(
    doubled = map([1, 2, 3], double),  // [2, 4, 6]
    squared = map([1, 2, 3], square)   // [1, 4, 9]
)
```

#### Chaining map Operations

```sigil
@process (numbers: [int]) -> [str] = run(
    doubled = map(numbers, x -> x * 2),
    positive = map(doubled, x -> if x < 0 then -x else x),
    map(positive, x -> str(x))
)
```

---

### `filter` — Select Elements

The `filter` pattern keeps elements matching a predicate:

```sigil
// Signature
@filter<T> (items: [T], pred: (T) -> bool) -> [T]

// Examples
evens = filter([1, 2, 3, 4], x -> x % 2 == 0)      // [2, 4]
positive = filter([-1, 0, 1, 2], x -> x > 0)       // [1, 2]
active = filter(users, user -> user.is_active)     // active users
```

#### Using Named Functions with filter

```sigil
@is_even (n: int) -> bool = n % 2 == 0
@is_positive (n: int) -> bool = n > 0

@main () -> void = run(
    evens = filter([1, 2, 3, 4], is_even),    // [2, 4]
    pos = filter([-1, 0, 1], is_positive)     // [1]
)
```

#### Combining filter with map

```sigil
@get_active_user_names (users: [User]) -> [str] = run(
    active = filter(users, u -> u.is_active),
    map(active, u -> u.name)
)
```

---

### `fold` — Reduce to Single Value

The `fold` pattern accumulates elements into a single result:

```sigil
// Signature
@fold<T, U> (items: [T], init: U, f: (U, T) -> U) -> U

// Examples
sum = fold([1, 2, 3], 0, (acc, x) -> acc + x)      // 6
product = fold([1, 2, 3, 4], 1, (acc, x) -> acc * x) // 24
concat = fold(["a", "b", "c"], "", (s, x) -> s + x) // "abc"
```

#### Using Operators with fold

```sigil
sum = fold([1, 2, 3], 0, +)        // 6
product = fold([1, 2, 3, 4], 1, *)  // 24
```

#### Complex Accumulation

```sigil
// Build a running total with index
@running_totals (items: [int]) -> [(int, int)] =
    fold(items, (0, []), (state, item) -> run(
        (total, results) = state,
        new_total = total + item,
        (new_total, results ++ [(item, new_total)])
    )).1  // Extract the list from the tuple

@main () -> void = run(
    totals = running_totals([1, 2, 3, 4]),
    // [(1, 1), (2, 3), (3, 6), (4, 10)]
    print(totals)
)
```

---

## Combining Patterns

### Map-Filter-Reduce Pipeline

```sigil
@process_orders (orders: [Order]) -> int = run(
    // Filter to completed orders
    completed = filter(orders, o -> o.status == Complete),

    // Map to order totals
    totals = map(completed, o -> o.total),

    // Sum all totals
    fold(totals, 0, +)
)
```

### Processing in Steps

```sigil
@analyze_users (users: [User]) -> Summary = run(
    // Step 1: filter active users
    active = filter(users, u -> u.is_active),

    // Step 2: map to relevant data
    ages = map(active, u -> u.age),

    // Step 3: compute statistics
    total = fold(ages, 0, +),
    count = len(ages),
    avg = if count > 0 then total / count else 0,

    Summary { total_active: count, average_age: avg }
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
    add5 = x -> add(5, x),
    add10 = x -> add(10, x),

    print(add5(3)),   // 8
    print(add10(3))   // 13
)
```

### Why Lambdas Instead of Special Syntax?

1. **Explicit** — `x -> add(5, x)` clearly shows what's happening
2. **Flexible** — Can fix any parameter in any position
3. **Familiar** — LLMs know lambda syntax well
4. **One way** — No confusion between multiple partial application syntaxes

### Partial Application Patterns

```sigil
// Fix first argument
@multiply (a: int, b: int) -> int = a * b
double = x -> multiply(2, x)
triple = x -> multiply(3, x)

// Fix second argument
@divide (a: int, b: int) -> int = a / b
half = x -> divide(x, 2)
quarter = x -> divide(x, 4)

// Fix multiple arguments
@greet (greeting: str, name: str, punctuation: str) -> str =
    greeting + ", " + name + punctuation

hello_to = name -> greet("Hello", name, "!")
goodbye_to = name -> greet("Goodbye", name, ".")
```

### With Higher-Order Functions

```sigil
@filter_above (threshold: int) -> ([int]) -> [int] =
    items -> filter(items, x -> x > threshold)

@main () -> void = run(
    above_five = filter_above(5),
    above_ten = filter_above(10),

    numbers = [3, 7, 12, 2, 9, 15],
    big = above_five(numbers),    // [7, 12, 9, 15]
    bigger = above_ten(numbers)   // [12, 15]
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
    double_then_add = x -> add_one(double(x)),
    result = double_then_add(5)  // 11
)
```

### Compose Function

```sigil
@compose<A, B, C> (f: (B) -> C, g: (A) -> B) -> (A) -> C =
    x -> f(g(x))

@main () -> void = run(
    double_then_add = compose(add_one, double),
    result = double_then_add(5)  // 11
)
```

### Pipeline Function

```sigil
@pipe<A, B, C> (x: A, f: (A) -> B, g: (B) -> C) -> C = g(f(x))

@main () -> void = run(
    result = pipe(5, double, add_one)  // 11
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
    transform = compose3(add_one, double, negate),
    result = transform(3)  // ((-3) * 2) + 1 = -5
)
```

---

## Common Higher-Order Functions

### `apply_n` — Apply Function N Times

```sigil
@apply_n<T> (f: (T) -> T, n: int, x: T) -> T =
    if n <= 0 then x
    else apply_n(f, n - 1, f(x))

@main () -> void = run(
    result = apply_n(double, 3, 2),  // 2 -> 4 -> 8 -> 16
    print(result)
)
```

### `find` — Find First Matching Element

```sigil
@find<T> (items: [T], pred: (T) -> bool) -> Option<T> =
    match(filter(items, pred),
        [] -> None,
        [first, ..] -> Some(first)
    )

@main () -> void = run(
    first_even = find([1, 3, 4, 5], x -> x % 2 == 0),  // Some(4)
    print(first_even)
)
```

### `any` / `all` — Check Predicates

```sigil
@any<T> (items: [T], pred: (T) -> bool) -> bool =
    len(filter(items, pred)) > 0

@all<T> (items: [T], pred: (T) -> bool) -> bool =
    len(filter(items, x -> !pred(x))) == 0

@main () -> void = run(
    has_even = any([1, 3, 4], x -> x % 2 == 0),   // true
    all_positive = all([1, 2, 3], x -> x > 0),    // true
    print(has_even),
    print(all_positive)
)
```

### `partition` — Split by Predicate

```sigil
@partition<T> (items: [T], pred: (T) -> bool) -> ([T], [T]) =
    (filter(items, pred), filter(items, x -> !pred(x)))

@main () -> void = run(
    (evens, odds) = partition([1, 2, 3, 4, 5], x -> x % 2 == 0),
    print(evens),  // [2, 4]
    print(odds)    // [1, 3, 5]
)
```

### `group_by` — Group by Key

```sigil
// Partition items into two groups based on predicate
@partition<T> (items: [T], pred: (T) -> bool) -> ([T], [T]) =
    fold(items, ([], []), (acc, item) ->
        if pred(item) then (acc.0 ++ [item], acc.1)
        else (acc.0, acc.1 ++ [item])
    )

@main () -> void = run(
    numbers = [1, 2, 3, 4, 5, 6],
    (evens, odds) = partition(numbers, n -> n % 2 == 0),
    // evens: [2, 4, 6], odds: [1, 3, 5]
    print(evens)
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
    sums = zip_with([1, 2, 3], [4, 5, 6], (a, b) -> a + b),
    // [5, 7, 9]
    print(sums)
)
```

### `until` — Apply Until Condition

```sigil
@until<T> (pred: (T) -> bool, f: (T) -> T, x: T) -> T =
    if pred(x) then x
    else until(pred, f, f(x))

@main () -> void = run(
    // Keep doubling until > 100
    result = until(x -> x > 100, x -> x * 2, 1),  // 128
    print(result)
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
        "https://api.example.com/data",
        data -> print("Got: " + data.to_string()),
        err -> print("Error: " + err.message)
    )
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
    button = create_button("Submit", event -> run(
        print("Button clicked at: " + str(event.x) + ", " + str(event.y)),
        submit_form()
    ))
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
    numbers = [3, 1, 4, 1, 5, 9],
    sorted = sort_with(numbers, quick_sort),
    print(sorted)
)
```

### Strategy Selection

```sigil
@choose_sort_strategy<T> (items: [T]) -> SortStrategy<T> where T: Comparable =
    if len(items) < 10 then bubble_sort
    else if len(items) < 1000 then quick_sort
    else merge_sort

@smart_sort<T> (items: [T]) -> [T] where T: Comparable = run(
    strategy = choose_sort_strategy(items),
    strategy(items)
)
```

---

## Best Practices

### Keep Functions Pure

```sigil
// Good: pure function, no side effects
@transform (items: [int], f: (int) -> int) -> [int] = map(items, f)

// Avoid: hidden side effects in function argument
@process (items: [int], f: (int) -> int) -> [int] = run(
    // If f has side effects, this becomes unpredictable
    map(items, f)
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
    active = filter(users, is_active),
    map(active, get_name)
)

// Avoid: monolithic function
@get_active_names_bad (users: [User]) -> [str] =
    map(filter(users, u -> u.status == Active), u -> u.name)
```

### Document Function Parameters

```sigil
// #Transforms each element in a list using the provided function
// >transform([1,2,3], x -> x * 2) -> [2,4,6]
// !The transform function should be pure (no side effects)
@transform<T, U> (items: [T], f: (T) -> U) -> [U] = map(items, f)
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

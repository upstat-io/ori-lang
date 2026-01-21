# Expressions

This document covers Sigil's expressions: operators, conditionals, and line continuation.

---

## Expression-Based Design

Sigil is expression-based—everything returns a value:

```sigil
// if/else is an expression
result = if x > 0 then "positive" else "non-positive"

// match is an expression
description = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete"
)

// Blocks return their last expression
@process () -> int = run(
    x = compute(),
    y = transform(x),
    x + y  // returned
)
```

---

## Operators

### Arithmetic

| Operator | Meaning | Example |
|----------|---------|---------|
| `+` | Addition | `a + b` |
| `-` | Subtraction | `a - b` |
| `*` | Multiplication | `a * b` |
| `/` | Division (truncates toward zero) | `a / b` |
| `%` | Modulo (sign follows dividend) | `a % b` |
| `div` | Floor division (toward -∞) | `a div b` |

### Comparison

| Operator | Meaning | Example |
|----------|---------|---------|
| `==` | Equal | `a == b` |
| `!=` | Not equal | `a != b` |
| `<` | Less than | `a < b` |
| `>` | Greater than | `a > b` |
| `<=` | Less or equal | `a <= b` |
| `>=` | Greater or equal | `a >= b` |

### Logical

| Operator | Meaning | Example |
|----------|---------|---------|
| `&&` | Logical and | `a && b` |
| `\|\|` | Logical or | `a \|\| b` |
| `!` | Logical not | `!a` |

### Other

| Operator | Meaning | Example |
|----------|---------|---------|
| `..` | Range (exclusive end) | `0..10` |
| `..=` | Range (inclusive end) | `0..=10` |
| `??` | Coalesce (None/Err default) | `value ?? default` |

---

## Operator Precedence

From highest to lowest:

1. Unary: `!`, `-` (negation)
2. Multiplicative: `*`, `/`, `%`, `div`
3. Additive: `+`, `-`
4. Range: `..`, `..=`
5. Comparison: `<`, `>`, `<=`, `>=`
6. Equality: `==`, `!=`
7. Logical and: `&&`
8. Logical or: `||`
9. Coalesce: `??`

Use parentheses to override:

```sigil
result = (a + b) * c
```

---

## Conditionals

### If Expression

```sigil
result = if condition then value_if_true else value_if_false
```

### Chained Conditions

```sigil
@fizzbuzz (n: int) -> str =
    if n % 15 == 0 then "FizzBuzz"
    else if n % 3 == 0 then "Fizz"
    else if n % 5 == 0 then "Buzz"
    else str(n)
```

### As Expression

Since `if` returns a value, no ternary operator is needed:

```sigil
// Other languages: x > 0 ? "positive" : "non-positive"
// Sigil:
result = if x > 0 then "positive" else "non-positive"
```

---

## For Expression

The `for` expression iterates over collections.

### Iteration (Side Effects)

Use `do` for side-effect iteration (returns `void`):

```sigil
for item in items do
    print(item)
```

### Building Collections (Yield)

Use `yield` to build a new collection:

```sigil
// Returns [int]
squares = for n in numbers yield n * n

// Equivalent to: map(numbers, n -> n * n)
```

### Filter + Transform

Combine `if` guard with `yield`:

```sigil
// Returns [int] — only even numbers, squared
even_squares = for n in numbers if n % 2 == 0 yield n * n

// Equivalent to: map(filter(numbers, n -> n % 2 == 0), n -> n * n)
```

### Multiple Bindings

Iterate over multiple collections (cartesian product):

```sigil
// Nested form
for x in xs do
    for y in ys yield (x, y)

// Flat form
pairs = for x in xs, y in ys yield (x, y)
```

### With Ranges

```sigil
// Build list from range
squares = for i in 0..10 yield i * i

// With filter
odd_squares = for i in 0..10 if i % 2 == 1 yield i * i
```

---

## For Pattern (Early Exit)

For early exit with Result semantics, use the pattern form of `for`:

### Basic Pattern Form

```sigil
// Iterates until match, returns Ok or Err
@find_positive (numbers: [int]) -> Result<int, void> = for(
    .over: numbers,
    .match: Ok(n).match(n > 0),
    .default: Err(void),
)
```

### With Transformation

```sigil
// Map each item, then match
@find_valid_parsed (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(item),
    .match: Ok(n).match(n > 0 && n < 100),
    .default: Err(void),
)
```

### For vs Find

Use `find` for simple searches, `for` pattern for complex matching:

```sigil
// Simple — use find pattern
@first_positive (items: [int]) -> Option<int> = find(
    .over: items,
    .where: n -> n > 0,
)

// Complex — use for pattern with transformation
@first_valid_parsed (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(item),
    .match: Ok(n).match(n > 0 && n < 100),
    .default: Err(void),
)
```

---

## Loop Expression

The `loop` expression creates an infinite loop, exited with `break`.

### Basic Loop

```sigil
loop(
    // body executes repeatedly until break
)
```

### Loop with Break

```sigil
@consume_channel (ch: Channel<int>) -> int = run(
    sum = 0,
    loop(
        match(ch.receive().await,
            Some(value) -> sum = sum + value,
            None -> break  // exit loop when channel closes
        )
    ),
    sum
)
```

### Break and Continue

| Keyword | Effect |
|---------|--------|
| `break` | Exit the innermost loop |
| `continue` | Skip to next iteration |

```sigil
@process_with_skip (ch: Channel<Item>) -> void = loop(
    match(ch.receive().await,
        Some(item) ->
            if item.should_skip then continue  // skip this item
            else process(item),
        None -> break  // exit on channel close
    )
)
```

### Common Patterns

**Channel consumer:**
```sigil
@worker (work: Channel<Job>, results: Channel<Result<Output, Error>>) -> async void = loop(
    match(work.receive().await,
        Some(job) -> results.send(process(job)).await,
        None -> break
    )
)
```

**Polling with cancellation:**
```sigil
@poll_until_cancelled (ctx: Context) -> async void = loop(
    if ctx.is_cancelled() then break,
    perform_check().await,
    sleep(100ms).await
)
```

### Loop vs For

| Use `loop` when | Use `for` when |
|-----------------|----------------|
| Infinite iteration | Bounded iteration |
| Channel consumption | Collection processing |
| Event loops | Transformation/mapping |
| Unknown end condition | Known end condition |

---

## Line Continuation

Use `_` at end of line to continue on next line:

```sigil
@check (a: int, b: int, c: int) -> bool =
    if a > 0 && _
       b > 0 && _
       c > 0 then true
    else false
```

### When to Use

Line continuation is useful for:
- Long boolean expressions
- Complex conditions
- Readable formatting

```sigil
@validate (user: User) -> bool =
    if user.age >= 18 && _
       user.verified && _
       !user.banned && _
       user.email.contains("@") then true
    else false
```

---

## Array Indexing

### Basic Indexing

```sigil
first = arr[0]
second = arr[1]
```

### Length-Relative Indexing

Use `#` inside brackets to refer to array length:

```sigil
arr[0]        // first element
arr[# - 1]    // last element
arr[# - 2]    // second to last
arr[# / 2]    // middle element
```

### Examples

```sigil
@last<T> (items: [T]) -> T = items[# - 1]

@middle<T> (items: [T]) -> T = items[# / 2]
```

---

## Assignment

### Basic Binding

Inside `run` or `try`, use `=` for bindings:

```sigil
@process (items: [int]) -> int = run(
    doubled = map(items, x -> x * 2),
    filtered = filter(doubled, x -> x > 10),
    fold(filtered, 0, +)
)
```

### With Type Annotation

```sigil
@process () -> int = run(
    x: int = compute(),
    y: float = 3.14,
    int(x) + int(y)
)
```

### Destructuring

```sigil
@process (point: Point) -> int = run(
    { x, y } = point,
    x + y
)

@first_two (items: [int]) -> int = run(
    [a, b, ..rest] = items,
    a + b
)
```

---

## Lambdas

### Basic Syntax

```sigil
x -> x * 2           // single parameter
(x, y) -> x + y      // multiple parameters
() -> 42             // no parameters
```

### With Type Annotations

```sigil
(x: int) -> x * 2
(x: int, y: int) -> x + y
```

### Usage

```sigil
doubled = map([1, 2, 3], x -> x * 2)
sum = fold([1, 2, 3], 0, (acc, x) -> acc + x)
filtered = filter([1, 2, 3, 4], x -> x % 2 == 0)
```

---

## Method Calls

Methods use dot notation:

```sigil
name = "hello"
upper = name.upper()          // "HELLO"
length = name.len()           // 5
contains = name.contains("ll") // true

items = [3, 1, 4, 1, 5]
sorted = items.sort()
reversed = items.reverse()
```

### Chaining

```sigil
result = text
    .trim()
    .lower()
    .split(" ")
    .filter(word -> word.len() > 0)
```

---

## Function Calls

### Basic Calls

```sigil
result = add(1, 2)
data = fetch_data(url)
```

### Pattern Calls

Patterns use the same call syntax:

```sigil
sum = fold(items, 0, +)
doubled = map(items, x -> x * 2)
```

### Named Arguments

Patterns support named property syntax:

```sigil
@fib (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)
```

---

## See Also

- [Basic Syntax](01-basic-syntax.md)
- [Patterns Overview](03-patterns-overview.md)
- [Pattern Matching](../06-pattern-matching/index.md)

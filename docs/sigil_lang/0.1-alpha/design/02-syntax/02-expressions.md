# Expressions

This document covers Sigil's expressions: operators, conditionals, and line continuation.

---

## Expression-Based Design

Sigil is expression-based—everything returns a value:

```sigil
// if/else is an expression
let result = if x > 0 then "positive" else "non-positive"

// match is an expression
let description = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete",
)

// Blocks return their last expression
@process () -> int = run(
    let x = compute(),
    let y = transform(.value: x),
    x + y,  // returned
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

### Bitwise

| Operator | Meaning | Example |
|----------|---------|---------|
| `&` | Bitwise and | `a & b` |
| `\|` | Bitwise or | `a \| b` |
| `^` | Bitwise xor | `a ^ b` |
| `~` | Bitwise not | `~a` |
| `<<` | Left shift | `a << 2` |
| `>>` | Right shift (arithmetic) | `a >> 2` |

Bitwise operators work on `int` and `byte` types:

```sigil
let flags = 0b1010 & 0b1100  // 0b1000 (8)
let mask = 1 << 4            // 16
let high = value >> 8        // shift right by 8 bits
let inverted = ~flags        // bitwise complement
```

**Shift behavior:**
- Shift amount is taken modulo the bit width of the type
- Right shift is arithmetic (preserves sign for negative numbers)
- Negative shift amounts are undefined behavior

### Other

| Operator | Meaning | Example |
|----------|---------|---------|
| `..` | Range (exclusive end) | `0..10` |
| `..=` | Range (inclusive end) | `0..=10` |
| `??` | Coalesce (None/Err default) | `value ?? default` |

---

## Operator Precedence

From highest to lowest:

1. Postfix: `.`, `[]`, `()`, `?` (access, index, call, propagate)
2. Unary: `!`, `-`, `~` (not, negate, bitwise not)
3. Multiplicative: `*`, `/`, `%`, `div`
4. Additive: `+`, `-`
5. Shift: `<<`, `>>` (left shift, right shift)
6. Range: `..`, `..=`
7. Comparison: `<`, `>`, `<=`, `>=`
8. Equality: `==`, `!=`
9. Bitwise and: `&`
10. Bitwise xor: `^`
11. Bitwise or: `|`
12. Logical and: `&&`
13. Logical or: `||`
14. Coalesce: `??`

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
    else str(.value: n)
```

### As Expression

Since `if` returns a value, no ternary operator is needed:

```sigil
// Other languages: x > 0 ? "positive" : "non-positive"
// Sigil:
let result = if x > 0 then "positive" else "non-positive"
```

---

## For Expression

The `for` expression iterates over collections.

### Iteration (Side Effects)

Use `do` for side-effect iteration (returns `void`):

```sigil
for item in items do
    print(.msg: item)
```

### Building Collections (Yield)

Use `yield` to build a new collection:

```sigil
// Returns [int]
squares = for n in numbers yield n * n

// Equivalent to: map(.over: numbers, .transform: n -> n * n)
```

### Filter + Transform

Combine `if` guard with `yield`:

```sigil
// Returns [int] — only even numbers, squared
even_squares = for n in numbers if n % 2 == 0 yield n * n

// Equivalent to:
// map(
//     .over: filter(
//         .over: numbers,
//         .predicate: n -> n % 2 == 0,
//     ),
//     .transform: n -> n * n,
// )
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
@consume_channel (ch: Channel<int>) -> int uses Async = run(
    let mut sum = 0,
    loop(
        match(ch.receive(),
            Some(value) -> sum = sum + value,
            None -> break,  // exit loop when channel closes
        ),
    ),
    sum,
)
```

### Break and Continue

| Keyword | Effect |
|---------|--------|
| `break` | Exit the innermost loop |
| `continue` | Skip to next iteration |

```sigil
@process_with_skip (ch: Channel<Item>) -> void uses Async = loop(
    match(ch.receive(),
        Some(item) ->
            if item.should_skip then continue  // skip this item
            else process(.item: item),
        None -> break,  // exit on channel close
    ),
)
```

### Common Patterns

**Channel consumer:**
```sigil
@worker (work: Channel<Job>, results: Channel<Result<Output, Error>>) -> void uses Async = loop(
    match(work.receive(),
        Some(job) -> results.send(.value: process(.job: job)),
        None -> break,
    ),
)
```

**Polling with cancellation:**
```sigil
@poll_until_cancelled (ctx: Context) -> void uses Async, Clock = loop(
    if ctx.is_cancelled() then break,
    perform_check(),
    Clock.sleep(100ms),
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

Lines naturally continue after operators, opening brackets, and commas. No explicit continuation character is needed.

```sigil
@check (a: int, b: int, c: int) -> bool =
    if a > 0 &&
       b > 0 &&
       c > 0 then true
    else false
```

### Natural Continuation Points

Lines continue automatically after:
- Binary operators: `&&`, `||`, `+`, `-`, `*`, `/`, etc.
- Opening brackets: `(`, `[`, `{`
- Commas: `,`
- Assignment: `=`

```sigil
@validate (user: User) -> bool =
    if user.age >= 18 &&
       user.verified &&
       !user.banned &&
       user.email.contains("@") then true
    else false

// Long function calls
let result = some_function(
    first_argument,
    second_argument,
    third_argument,
)

// Chained operations using method syntax
let processed = data
    .filter(.predicate: x -> x > 0)
    .map(.transform: x -> x * 2)
    .fold(.init: 0, .op: +)
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

Inside `run` or `try`, use `let` for bindings:

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: x -> x * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: x -> x > 10,
    ),
    fold(
        .over: filtered,
        .init: 0,
        .op: +,
    ),
)
```

### With Type Annotation

```sigil
@process () -> int = run(
    let x: int = compute(),
    let y: float = 3.14,
    int(.value: x) + int(.value: y),
)
```

### Mutable Bindings

Use `let mut` for variables that will be reassigned:

```sigil
@accumulate (items: [int]) -> int = run(
    let mut total = 0,
    for item in items do
        total = total + item,
    total,
)
```

### Destructuring

```sigil
@process (point: Point) -> int = run(
    let { x, y } = point,
    x + y,
)

@first_two (items: [int]) -> int = run(
    let [a, b, ..rest] = items,
    a + b,
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
doubled = map(
    .over: [1, 2, 3],
    .transform: x -> x * 2,
)
sum = fold(
    .over: [1, 2, 3],
    .init: 0,
    .op: (acc, x) -> acc + x,
)
filtered = filter(
    .over: [1, 2, 3, 4],
    .predicate: x -> x % 2 == 0,
)
```

---

## Method Calls

Methods use dot notation:

```sigil
name = "hello"
upper = name.upper()                 // "HELLO"
length = name.len()                  // 5
contains = name.contains(.str: "ll") // true

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
result = add(.a: 1, .b: 2)
data = fetch_data(.url: url)
```

### Pattern Calls

Patterns use named property syntax:

```sigil
sum = fold(
    .over: items,
    .init: 0,
    .op: +,
)
doubled = map(
    .over: items,
    .transform: x -> x * 2,
)
```

### Named Arguments

All function calls use named property syntax:

```sigil
@fib (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true,
)
```

---

## See Also

- [Basic Syntax](01-basic-syntax.md)
- [Patterns Overview](03-patterns-overview.md)
- [Pattern Matching](../06-pattern-matching/index.md)

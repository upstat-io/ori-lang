# Expressions

This document covers Ori's expressions: operators, conditionals, and line continuation.

---

## Expression-Based Design

Ori is expression-based—everything returns a value:

```ori
// if/else is an expression
let result = if value > 0 then "positive" else "non-positive"

// match is an expression
let description = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete",
)

// Blocks return their last expression
@process () -> int = run(
    let first = compute(),
    let second = transform(
        .input: first,
    ),
    // returned
    first + second,
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

```ori
// Bitwise AND: 0b1010 & 0b1100 = 0b1000 (8)
let flags = 0b1010 & 0b1100
// Left shift: 1 << 4 = 16
let mask = 1 << 4
// Right shift by 8 bits
let high = value >> 8
// Bitwise complement
let inverted = ~flags
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

```ori
let result = (a + b) * c
```

---

## Conditionals

### If Expression

```ori
let result = if condition then value_if_true else value_if_false
```

### Chained Conditions

```ori
@fizzbuzz (number: int) -> str =
    if number % 15 == 0 then "FizzBuzz"
    else if number % 3 == 0 then "Fizz"
    else if number % 5 == 0 then "Buzz"
    else str(number)
```

### As Expression

Since `if` returns a value, no ternary operator is needed:

```ori
// Other languages: value > 0 ? "positive" : "non-positive"
// Ori:
let result = if value > 0 then "positive" else "non-positive"
```

---

## For Expression

The `for` expression iterates over collections.

### Iteration (Side Effects)

Use `do` for side-effect iteration (returns `void`):

```ori
for item in items do
    print(
        .message: item,
    )
```

### Building Collections (Yield)

Use `yield` to build a new collection:

```ori
// Returns [int]
let squares = for number in numbers yield number * number

// Equivalent to:
// map(
//     .over: numbers,
//     .transform: number -> number * number,
// )
```

### Filter + Transform

Combine `if` guard with `yield`:

```ori
// Returns [int] — only even numbers, squared
let even_squares = for number in numbers if number % 2 == 0 yield number * number

// Equivalent to:
// map(
//     .over: filter(
//         .over: numbers,
//         .predicate: number -> number % 2 == 0,
//     ),
//     .transform: number -> number * number,
// )
```

### Multiple Bindings

Iterate over multiple collections (cartesian product):

```ori
// Nested form
for first in first_list do
    for second in second_list yield (first, second)

// Flat form
let pairs = for first in first_list, second in second_list yield (first, second)
```

### With Ranges

```ori
// Build list from range
let squares = for index in 0..10 yield index * index

// With filter
let odd_squares = for index in 0..10 if index % 2 == 1 yield index * index
```

---

## For Pattern (Early Exit)

For early exit with Result semantics, use the pattern form of `for`:

### Basic Pattern Form

```ori
// Iterates until match, returns Ok or Err
@find_positive (numbers: [int]) -> Result<int, void> = for(
    .over: numbers,
    .match: Ok(number).match(number > 0),
    .default: Err(void),
)
```

### With Transformation

```ori
// Map each item, then match
@find_valid_parsed (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(
        .text: item,
    ),
    .match: Ok(value).match(value > 0 && value < 100),
    .default: Err(void),
)
```

### For vs Find

Use `find` for simple searches, `for` pattern for complex matching:

```ori
// Simple — use find pattern
@first_positive (items: [int]) -> Option<int> = find(
    .over: items,
    .where: number -> number > 0,
)

// Complex — use for pattern with transformation
@first_valid_parsed (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(
        .text: item,
    ),
    .match: Ok(value).match(value > 0 && value < 100),
    .default: Err(void),
)
```

---

## Loop Expression

The `loop` expression creates an infinite loop, exited with `break`.

### Basic Loop

```ori
loop(
    // body executes repeatedly until break
)
```

### Loop with Break

```ori
@consume_channel (channel: Channel<int>) -> int uses Async = run(
    let mut sum = 0,
    loop(
        match(channel.receive(),
            Some(value) -> sum = sum + value,
            // exit loop when channel closes
            None -> break,
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

```ori
@process_with_skip (channel: Channel<Item>) -> void uses Async = loop(
    match(channel.receive(),
        Some(item) ->
            // skip this item
            if item.should_skip then continue
            else process(
                .item: item,
            ),
        // exit on channel close
        None -> break,
    ),
)
```

### Common Patterns

**Channel consumer:**
```ori
@worker (work_channel: Channel<Job>, results_channel: Channel<Result<Output, Error>>) -> void uses Async = loop(
    match(work_channel.receive(),
        Some(job) -> results_channel.send(
            .value: process(
                .job: job,
            ),
        ),
        None -> break,
    ),
)
```

**Polling with cancellation:**
```ori
@poll_until_cancelled (context: Context) -> void uses Async, Clock = loop(
    if context.is_cancelled() then break,
    perform_check(),
    Clock.sleep(
        .duration: 100ms,
    ),
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

```ori
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

```ori
@validate (user: User) -> bool =
    if user.age >= 18 &&
       user.verified &&
       !user.banned &&
       user.email.contains(
           .substring: "@",
       ) then true
    else false

// Long function calls
let result = some_function(
    .first: first_argument,
    .second: second_argument,
    .third: third_argument,
)

// Chained operations using method syntax
let processed = data
    .filter(
        .predicate: item -> item > 0,
    )
    .map(
        .transform: item -> item * 2,
    )
    .fold(
        .initial: 0,
        .operation: +,
    )
```

---

## Array Indexing

### Basic Indexing

```ori
let first = arr[0]
let second = arr[1]
```

### Length-Relative Indexing

Use `#` inside brackets to refer to array length:

```ori
// First element
arr[0]
// Last element
arr[# - 1]
// Second to last
arr[# - 2]
// Middle element
arr[# / 2]
```

### Examples

```ori
@last<T> (items: [T]) -> T = items[# - 1]

@middle<T> (items: [T]) -> T = items[# / 2]
```

---

## Assignment

### Basic Binding

Inside `run` or `try`, use `let` for bindings:

```ori
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

### With Type Annotation

```ori
@process () -> int = run(
    let first: int = compute(),
    let second: float = 3.14,
    first + int(second),
)
```

### Mutable Bindings

Use `let mut` for variables that will be reassigned:

```ori
@accumulate (items: [int]) -> int = run(
    let mut total = 0,
    for item in items do
        total = total + item,
    total,
)
```

### Destructuring

```ori
@process (point: Point) -> int = run(
    let { x, y } = point,
    x + y,
)

@first_two (items: [int]) -> int = run(
    let [first, second, ..rest] = items,
    first + second,
)
```

---

## Lambdas

### Basic Syntax

```ori
// single parameter
number -> number * 2
// multiple parameters
(left, right) -> left + right
// no parameters
() -> 42
```

### With Type Annotations

```ori
(number: int) -> number * 2
(left: int, right: int) -> left + right
```

### Usage

```ori
let doubled = map(
    .over: [1, 2, 3],
    .transform: number -> number * 2,
)
let sum = fold(
    .over: [1, 2, 3],
    .initial: 0,
    .operation: (accumulator, number) -> accumulator + number,
)
let filtered = filter(
    .over: [1, 2, 3, 4],
    .predicate: number -> number % 2 == 0,
)
```

---

## Method Calls

Methods use dot notation:

```ori
let name = "hello"
// Returns "HELLO"
let upper = name.upper()
// Returns 5
let length = name.len()
// Returns true
let contains = name.contains(
    .substring: "ll",
)

let items = [3, 1, 4, 1, 5]
let sorted = items.sort()
let reversed = items.reverse()
```

### Chaining

```ori
let result = text
    .trim()
    .lower()
    .split(
        .separator: " ",
    )
    .filter(
        .predicate: word -> word.len() > 0,
    )
```

---

## Function Calls

### Basic Calls

```ori
let result = add(
    .left: 1,
    .right: 2,
)
let data = fetch_data(
    .url: url,
)
```

### Pattern Calls

Patterns use named property syntax:

```ori
let sum = fold(
    .over: items,
    .initial: 0,
    .operation: +,
)
let doubled = map(
    .over: items,
    .transform: item -> item * 2,
)
```

### Named Arguments

All function calls use named property syntax:

```ori
@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)
```

### Built-in Function Resolution

Built-in function names are recognized **in call position only** (`name(`). This allows the same names to be used as variables without shadowing:

```ori
// Variable named 'min'
let min = 5
// Built-in min function, using variable
let result = min(
    .left: min,
    .right: 10,
)
```

**Why this design?**

1. **Natural variable names**: `min`, `max`, `str` are intuitive names for local values
2. **No shadowing confusion**: `min(` always calls the built-in—predictable
3. **AI-friendly**: Built-in calls are syntactically identifiable (`name(`)
4. **Function reservation**: Defining `@min` is an error, so there's no user-defined function ambiguity

**What the parser does:**

| Syntax | Interpretation |
|--------|----------------|
| `min(...)` | Built-in function call |
| `min` | Variable reference |
| `@min (...)` | Compile error (reserved name) |

This is a context-sensitive approach: the `(` determines whether we're in call position.

---

## See Also

- [Basic Syntax](01-basic-syntax.md)
- [Patterns Overview](03-patterns-overview.md)
- [Pattern Matching](../06-pattern-matching/index.md)

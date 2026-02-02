---
title: "Edge Cases"
description: "Ori Formatter Design — Comprehensive Edge Case Examples"
order: 1
section: "Appendices"
---

# Edge Cases

This appendix documents edge cases and their expected formatting.

## Empty Constructs

```ori
// Empty collections - no inner space
let empty_list = []
let empty_map = {}
let empty_tuple = ()
let empty_struct = Empty {}

// Empty function body
@noop () -> void = ()

// Empty trait
trait Marker {}

// Empty impl
impl Marker for Point {}
```

## Single-Element Collections

```ori
// Single-element list
let single = [42]
let single_str = ["hello"]

// Single-entry map
let single_map = {"key": "value"}

// Single-element tuple (note: this is just parenthesized expression)
let not_tuple = (42)

// Single-field struct
type Wrapper = { value: int }
let w = Wrapper { value: 42 }
```

## Very Long Single Items

```ori
// Long string - never break inside, break the binding if needed
let message =
    "This is a very long string that exceeds 100 characters but we never break inside string literals"

// Long template string
let template =
    `Dear {user.name}, your order #{order.id} has been shipped to {order.address}.`

// Long identifier (rare but possible)
let very_long_variable_name_that_somehow_exists_in_the_codebase = compute()
```

## Deeply Nested Structures

```ori
// Each level breaks independently
let config = Config {
    database: DatabaseConfig {
        connection: ConnectionConfig {
            host: "production-db.example.com",
            port: 5432,
            pool: PoolConfig {
                min_size: 5,
                max_size: 20,
                timeout: 30s,
            },
        },
        credentials: Credentials { username: user, password: pass },
    },
    cache: CacheConfig { host: "localhost", port: 6379 },
}

// Deeply nested calls
let result = process(
    data: transform(
        input: validate(
            data: parse(
                text: fetch(url: endpoint)?,
            )?,
        )?,
        options: TransformOptions {
            mode: Mode.Strict,
            fallback: None,
        },
    ),
)
```

## Chains with Mixed Call Lengths

```ori
// All break once any breaks
let result = items
    .a()
    .b()
    .very_long_method_name_that_takes_many_arguments(
        first: value1,
        second: value2,
        third: value3,
    )
    .c()
    .d()
```

## Binary Expressions with Mixed Precedence

```ori
// Parentheses preserved, breaks before operators
let result = (first_value + second_value)
    * (third_value - fourth_value)
    / (fifth_value + sixth_value)

// Complex boolean
let valid = (is_admin || has_permission(user, resource))
    && is_authenticated(user)
    && !is_expired(token)
```

## Parentheses Preservation

Parentheses are required in certain positions to maintain correct parsing. See [ParenthesesRule](../03-layers/04-rules.md#parenthesesrule) for details.

```ori
// Method receiver - parens required for complex expressions
(for x in items yield x).fold(0, acc, x -> acc + x)
(items.filter(x -> x > 0)).count()

// Call target - parens required for lambdas
(x -> x * 2)(5)
((a, b) -> a + b)(1, 2)

// Iterator source - parens required for nested for/if/lambda
for x in (for y in outer yield transform(y)) yield process(x)
for item in (if has_data then items else []) yield item

// Continue with value - parens may be needed for complex expressions
for x in items yield
    if skip_condition then continue (x * 2)
    else x
```

## Function Signatures at Boundary

```ori
// Just under 100 - stays inline
@process (input: int, config: Config) -> Result<Output, Error> = do_it()

// Just over 100 - breaks
@process (
    input: int,
    config: Config,
) -> Result<Output, ProcessingError> = do_work()
```

## Where Clause Combinations

```ori
// Single short constraint - inline
@sort<T> (items: [T]) -> [T] where T: Comparable = do_sort()

// Single long constraint - breaks
@process<T> (items: [T]) -> [T]
    where T: Clone + Debug + Default + Printable = do_it()

// Multiple constraints
@transform<T, U, V> (a: T, b: U) -> V
    where T: Clone + Into<V>,
          U: Debug + Default,
          V: Printable = do_transform()
```

## Capabilities with Long Names

```ori
// Multiple capabilities fit
@fetch (url: str) -> Result<str, Error> uses Http, Logger = do_fetch()

// Capabilities break to new line
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache, Database = do_it()
```

## Lambdas in Various Contexts

```ori
// Simple lambda in chain
items.map(x -> x * 2)

// Lambda with type annotation
items.map((x: int) -> int = x * 2)

// Lambda as standalone value
let transform: (int) -> int = x -> x * 2

// Lambda with complex body in chain
items.map(
    x ->
        run(
            let doubled = x * 2,
            let validated = validate(doubled),
            validated,
        ),
)

// Multiple lambdas in call
combine(
    first: x -> x + 1,
    second: x -> x * 2,
    third: x -> x - 1,
)
```

## Match with Various Arm Complexities

```ori
// Simple arms
let result = match(value,
    Some(x) -> x,
    None -> 0,
)

// Arms with guards
let result = match(n,
    x if x < 0 -> "negative",
    0 -> "zero",
    x if x < 10 -> "small",
    _ -> "large",
)

// Arms with complex patterns
let result = match(event,
    Event.Click { x, y, button: Button.Left } -> handle_left_click(x, y),
    Event.Click { x, y, button: Button.Right } -> handle_right_click(x, y),
    Event.KeyPress { key, modifiers: Modifiers { ctrl: true, .. } } ->
        handle_ctrl_key(key),
    _ -> ignore(),
)

// Arms with complex bodies
let result = match(data,
    Valid(content) ->
        run(
            let processed = process(content),
            let validated = validate(processed),
            Ok(validated),
        ),
    Invalid(error) -> Err(error),
)
```

## Conditionals at Boundaries

```ori
// Just fits - inline
let x = if condition then "yes" else "no"

// Slightly too long - breaks
let category =
    if value > threshold then "above"
    else "below"

// Complex condition that breaks
let result =
    if is_valid(data)
        && has_permission(user)
        && is_within_quota(user)
    then process(data)
    else reject(data)

// Nested conditionals
let grade =
    if score >= 90 then "A"
    else if score >= 80 then "B"
    else if score >= 70 then "C"
    else if score >= 60 then "D"
    else "F"
```

## Import Edge Cases

```ori
// Single import
use std.math { sqrt }

// Many imports from one module - wraps
use std.collections {
    BTreeMap,
    BTreeSet,
    HashMap,
    HashSet,
    LinkedList,
    VecDeque,
}

// Alias
use std.net.http as http
use "./internal" { VeryLongTypeName as Short }

// Private import
use "./internal" { ::private_helper }

// Re-export
pub use "./internal" { Widget, Button, Label }
```

## Struct Definition Edge Cases

```ori
// Empty struct
type Empty = {}

// Single field
type Wrapper = { value: int }

// Two short fields - inline
type Point = { x: int, y: int }

// Two long fields - breaks
type Range = {
    start: Timestamp,
    end: Timestamp,
}

// Generic struct
type Container<T> = { value: T, metadata: Metadata }

// Struct with derive
#derive(Eq, Clone, Debug, Default)
type Config = {
    timeout: Duration,
    retries: int,
    base_url: str,
}
```

## Sum Type Edge Cases

```ori
// All unit variants - may fit inline
type Direction = North | South | East | West

// Mixed variants
type Option<T> = Some(value: T) | None

// Complex variants - always breaks
type Expr =
    | Literal(value: Value)
    | Binary(left: Box<Expr>, op: BinaryOp, right: Box<Expr>)
    | Unary(op: UnaryOp, operand: Box<Expr>)
    | Call(func: Box<Expr>, args: Vec<Argument>)
    | If(condition: Box<Expr>, then_branch: Box<Expr>, else_branch: Box<Expr>)
```

## Tests with Multiple Targets

```ori
// Single target
@test_add tests @add () -> void = run(
    assert_eq(actual: add(a: 1, b: 2), expected: 3),
)

// Multiple targets - stays on one line if fits
@test_math tests @add tests @subtract () -> void = run(
    assert_eq(actual: add(a: 1, b: 2), expected: 3),
    assert_eq(actual: subtract(a: 5, b: 3), expected: 2),
)

// Many targets - may need to break (rare)
@test_all_operations tests @add tests @subtract tests @multiply tests @divide () -> void = run(
    // assertions
)
```

## Contract Edge Cases

```ori
// Simple pre_check
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a / b,
)

// Pre_check with message
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0 | "divisor cannot be zero",
    a / b,
)

// Complex pre_check condition
@process (data: Data) -> Result<Output, Error> = run(
    pre_check: data.is_valid()
        && data.size() > 0
        && data.size() < MAX_SIZE
        | "data must be valid and within size limits",
    do_process(data),
)

// Both pre and post
@abs (n: int) -> int = run(
    pre_check: true,  // No precondition
    let result = if n >= 0 then n else -n,
    post_check: r -> r >= 0,
    result,
)
```

## Trait and Impl Edge Cases

```ori
// Trait with associated type
trait Iterator {
    type Item

    @next (self) -> (Option<Self.Item>, Self)
}

// Trait with default implementation
trait Default {
    @default () -> Self
}

// Generic impl
impl<T: Clone> Clone for Option<T> {
    @clone (self) -> Self = match(self,
        Some(value) -> Some(value.clone()),
        None -> None,
    )
}

// Impl with where clause
impl<T, U> From<T> for Container<U>
    where T: Into<U> {
    @from (value: T) -> Self = Container { value: value.into() }
}
```

## Nursery and Parallel Edge Cases

```ori
// Simple nursery
let results = nursery(
    body: n -> run(
        n.spawn(task: fetch(url: "/a")),
        n.spawn(task: fetch(url: "/b")),
    ),
    on_error: CancelRemaining,
)

// Nursery with dynamic spawning
let results = nursery(
    body: n ->
        for url in urls do
            n.spawn(task: fetch(url: url)),
    on_error: CollectAll,
    timeout: 30s,
)

// Parallel with all options
let results = parallel(
    tasks: [
        fetch_user(id: 1),
        fetch_user(id: 2),
        fetch_user(id: 3),
    ],
    max_concurrent: 2,
    timeout: 10s,
)
```

## Comments in Various Positions

```ori
// Comment before function
// This function adds two numbers.
@add (a: int, b: int) -> int = a + b

// Comments in run block
@process (data: Data) -> Result<Output, Error> = run(
    // First, validate the input
    let validated = validate(data),

    // Then transform it
    let transformed = transform(validated),

    // Finally, produce the result
    Ok(transformed),
)

// Doc comments in correct order
// #Computes the factorial.
// @param n Must be non-negative.
// !Panics if n is negative.
// >factorial(n: 5) -> 120
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)
```

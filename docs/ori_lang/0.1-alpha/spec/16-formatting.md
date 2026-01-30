---
title: "Formatting"
description: "Ori Language Specification — Formatting"
order: 16
section: "Tooling"
---

# Formatting

Canonical source formatting. Zero-config, deterministic.

## Normalization Rules

The formatter applies normalization rules that produce a single canonical format. The core principle is **width-based breaking**: constructs remain inline if they fit within 100 characters, otherwise they break according to construct-specific rules.

### General

- 4 spaces indentation, no tabs
- 100 character line limit (hard)
- Trailing commas required in multi-line, forbidden in single-line
- No consecutive, leading, or trailing blank lines

### Spacing

| Context | Rule | Example |
|---------|------|---------|
| Binary operators | Space around | `a + b`, `x == y` |
| Arrows | Space around | `x -> x + 1`, `-> Type` |
| Colons (type annotations) | Space after | `x: int`, `key: value` |
| Commas | Space after | `f(a, b, c)` |
| Parentheses | No space inside | `f(x)`, `(a, b)` |
| Brackets | No space inside | `[1, 2]`, `items[0]` |
| Struct braces | Space inside | `Point { x, y }` |
| Empty delimiters | No space | `[]`, `{}`, `()` |
| Field/member access | No space around `.` | `point.x`, `std.math` |
| Range operators | No space around `..`/`..=` | `0..10`, `0..=100` |
| Range step | Space around `by` | `0..100 by 5` |
| Spread operator | No space after `...` | `[...a, ...b]` |
| Unary operators | No space after | `-x`, `!valid`, `~mask` |
| Error propagation | No space before `?` | `fetch()?` |
| Labels | No space around `:` | `loop:outer`, `break:label` |
| Type conversion | Space around `as`/`as?` | `42 as float` |
| Visibility | Space after `pub` | `pub @add` |
| Generic bounds | Space after `:`, around `+` | `<T: Clone + Debug>` |
| Sum type variants | Space around `\|` | `Red \| Green \| Blue` |
| Comments | Space after `//` | `// comment` |

### Blank Lines

- One after imports block
- One after constants block
- One between top-level declarations (functions, types, traits, impls)
- One between trait/impl methods (except single-method blocks)
- No consecutive blank lines
- User's blank lines within constant blocks are preserved

### Width-Based Breaking

Most constructs follow this rule: **inline if ≤100 characters, break otherwise**.

| Construct | Inline | Broken |
|-----------|--------|--------|
| Function parameters | All on one line | One per line |
| Function arguments | All on one line | One per line |
| Generic parameters | All on one line | One per line |
| Where constraints | After signature | New line, aligned |
| Capabilities | After signature | New line, comma-separated |
| Struct fields (def) | All on one line | One per line |
| Struct fields (literal) | All on one line | One per line |
| Sum type variants | All on one line | One per line with leading `\|` |
| Map entries | All on one line | One per line |
| Tuple elements | All on one line | One per line |
| Import items | All on one line | One per line, sorted |
| Lists (simple items) | All on one line | Wrap multiple per line |
| Lists (complex items) | All on one line | One per line |

### Always-Stacked Constructs

These constructs are always stacked regardless of width:

| Construct | Reason |
|-----------|--------|
| `run` / `try` | Sequential blocks; stacking shows execution order |
| `match` arms | Pattern matching; one arm per line aids readability |
| `recurse` | Named parameters pattern |
| `parallel` / `spawn` | Concurrency patterns |
| `nursery` | Structured concurrency pattern |

### Independent Breaking

Nested constructs break independently based on their own width. Outer breaking does not force inner breaking.

---

## Function Signatures

Inline if ≤100 characters:

```ori
@add (a: int, b: int) -> int = a + b

@transform (user_id: int, transform: (User) -> User) -> Result<User, Error> = do_work()
```

Break parameters if >100 characters:

```ori
@send_notification (
    user_id: int,
    notification: Notification,
    preferences: NotificationPreferences,
) -> Result<void, Error> = do_notify()
```

Break return type if `) -> Type =` still exceeds 100. Body on same line if it fits, otherwise indented:

```ori
@long_function_name (
    first: int,
    second: str,
) -> Result<HashMap<UserId, Preferences>, ServiceError> = do_work()

@long_function_name (
    first: int,
    second: str,
) -> Result<HashMap<UserId, Preferences>, ServiceError> =
    compute_something_complex(input: data)
```

## Function Calls

Inline if ≤100 characters:

```ori
let result = add(a: 1, b: 2)
let result = send_email(to: recipient, subject: title, body: content)
```

Break arguments if >100 characters (even single-argument calls):

```ori
let result = send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
)

let result = process(
    data: some_very_long_variable_name_that_pushes_past_limit,
)
```

## Generics

Inline if ≤100 characters:

```ori
@identity<T> (x: T) -> T = x
type Pair<T, U> = { first: T, second: U }
```

Break if >100 characters:

```ori
type Complex<
    InputType,
    OutputType,
    ErrorType,
    ConfigType,
> = { ... }
```

## Where Clauses

Inline if ≤100 characters:

```ori
@sort<T> (items: [T]) -> [T] where T: Comparable = do_sort()
```

Break if >100 characters. Constraints aligned:

```ori
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone + Debug,
          U: Default + Printable = do_it()
```

## Capabilities

Inline if ≤100 characters:

```ori
@fetch (url: str) -> Result<str, Error> uses Http = http_get(url)
```

Break if >100 characters. Capabilities stay comma-separated:

```ori
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache = do_it()
```

## Lists

Inline if ≤100 characters:

```ori
let nums = [1, 2, 3, 4, 5]
```

Simple items wrap multiple per line:

```ori
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
]
```

Complex items (structs, calls, nested collections) one per line:

```ori
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
    User { id: 3, name: "Charlie" },
]
```

Empty list: `[]`

## Maps

Inline if ≤100 characters:

```ori
let m = {"a": 1, "b": 2}
```

One entry per line if >100 characters:

```ori
let m = {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
}
```

Empty map: `{}`

## Tuples

Inline if ≤100 characters:

```ori
let pair = (1, "hello")
```

One element per line if >100 characters:

```ori
let data = (
    first_very_long_value,
    second_very_long_value,
)
```

Unit: `()`

## Struct Literals

Inline if ≤100 characters:

```ori
let p = Point { x: 0, y: 0 }
let u = User { id: 1, name: "Alice", active: true }
```

One field per line if >100 characters:

```ori
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
}
```

Field shorthand: `Point { x, y }`

Spread: `Point { ...original, x: 10 }`

## Type Definitions

### Structs

Inline if ≤100 characters:

```ori
type Point = { x: int, y: int }
```

One field per line if >100 characters:

```ori
type User = {
    id: int,
    name: str,
    email: str,
    created_at: Timestamp,
}
```

### Sum Types

Inline if ≤100 characters:

```ori
type Color = Red | Green | Blue
type Result<T, E> = Ok(value: T) | Err(error: E)
```

One variant per line with leading `|` if >100 characters:

```ori
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: Modifiers)
    | Scroll(delta_x: float, delta_y: float)
```

### Attributes

Attributes on own line. Multiple derives combined:

```ori
#derive(Eq, Clone, Debug)
type Point = { x: int, y: int }

#derive(Eq, Clone)
#deprecated("use NewType instead")
type OldType = { value: int }
```

## Trait/Impl Blocks

Opening brace on same line. One blank line between methods (except single-method blocks):

```ori
trait Printable {
    @to_str (self) -> str
}

impl Printable for Point {
    @to_str (self) -> str = `({self.x}, {self.y})`
}

impl Point {
    @new (x: int, y: int) -> Point = Point { x, y }

    @distance (self, other: Point) -> float = run(
        let dx = self.x - other.x,
        let dy = self.y - other.y,
        sqrt(float(dx * dx + dy * dy)),
    )
}
```

## Lambdas

No parens for single untyped param:

```ori
x -> x + 1
items.map(x -> x * 2)
```

Parens for zero, multiple, or typed params:

```ori
() -> 42
(a, b) -> a + b
(x: int) -> int = x * 2
```

Break after `->` only for always-stacked patterns (`run`, `try`, `match`):

```ori
let process = x ->
    run(
        let doubled = x * 2,
        let validated = validate(doubled),
        validated,
    )
```

## Conditionals

Inline if ≤100 characters:

```ori
let sign = if x > 0 then "positive" else "negative"
```

Break with `if` on new line, keeping `if cond then expr` together:

```ori
let category =
    if value > 100 then "large"
    else "small"
```

Chained else-if:

```ori
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

Branch bodies break independently:

```ori
let result =
    if condition then compute_simple(x: value)
    else compute_with_many_args(
        input: data,
        fallback: default,
    )
```

## Binary Expressions

Inline if ≤100 characters:

```ori
let result = a + b * c - d
```

First operand on `let` line, break before operator:

```ori
let result = first_value + second_value
    - third_value * fourth_value
    + fifth_value / sixth_value
```

## Method Chains

Inline if ≤100 characters:

```ori
let result = items.filter(x -> x > 0).map(x -> x * 2)
```

Initial value on `let` line, break at every `.` once any break needed:

```ori
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b)
```

## run/try

Always stacked (never inline):

```ori
let result = run(
    let x = compute(),
    let y = transform(x),
    x + y,
)

let result = try(
    let data = fetch(url: endpoint)?,
    let parsed = parse(input: data)?,
    Ok(parsed),
)
```

With contracts:

```ori
let result = run(
    pre_check: b != 0 | "divisor cannot be zero",
    a / b,
)

let result = run(
    let value = compute(),
    post_check: r -> r >= 0,
    value,
)
```

## match

Scrutinee on first line, arms always stacked:

```ori
let label = match(status,
    Pending -> "waiting",
    Running -> "in progress",
    Complete -> "done",
)
```

Arms with long calls break the call arguments (not after `->`):

```ori
let result = match(event,
    Click(x, y) -> handle_click_with_long_name(
        x: x,
        y: y,
        options: defaults,
    ),
    KeyPress(key) -> handle_key(key),
)
```

Arms with always-stacked bodies break after `->`:

```ori
let result = match(data,
    Valid(content) ->
        run(
            let processed = process(content),
            Ok(processed),
        ),
    Invalid(error) -> Err(error),
)
```

## recurse

Always stacked:

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)
```

## parallel/spawn

Always stacked. Task list follows list rules:

```ori
let results = parallel(
    tasks: [
        fetch_user(id: 1),
        fetch_user(id: 2),
        fetch_user(id: 3),
    ],
    max_concurrent: 3,
)
```

## timeout/cache/catch

Width-based. Inline if ≤100:

```ori
let result = timeout(op: fetch(url: endpoint), after: 5s)
let user = cache(key: "k", op: get(), ttl: 1m)
let safe = catch(expr: might_panic())
```

Stacked if >100:

```ori
let result = timeout(
    op: fetch(url: slow_endpoint),
    after: 5s,
)
```

## with Expressions

Inline if ≤100:

```ori
let result = with Http = mock_http in fetch(url: "/api")
```

Broken with capabilities aligned:

```ori
let result =
    with Http = MockHttp { responses: default_responses }
    in fetch_user_data(user_id: current_user)

let result =
    with Http = mock_http,
         Logger = mock_logger
    in perform_operation(input: data)
```

## for Loops

Inline if short:

```ori
for x in items do print(msg: x)
let doubled = for x in items yield x * 2
```

Body on next line when broken:

```ori
for user in users do
    process_user(user: user, options: default_options)

let results = for item in items yield
    transform(input: item, config: default_config)
```

## nursery

Always stacked:

```ori
let results = nursery(
    body: n ->
        run(
            n.spawn(task: fetch(url: "/a")),
            n.spawn(task: fetch(url: "/b")),
        ),
    on_error: CancelRemaining,
    timeout: 30s,
)
```

## Imports

Stdlib first, relative second, blank line between. Sorted alphabetically:

```ori
use std.collections { HashMap, Set }
use std.math { abs, sqrt }
use std.time { Duration }

use "../utils" { format }
use "./helpers" { compute, validate }
```

Items sorted alphabetically. Break to multiple lines if >100 characters:

```ori
use std.collections {
    BTreeMap,
    BTreeSet,
    HashMap,
    HashSet,
    LinkedList,
}
```

Extension imports follow the same rules:

```ori
extension std.iter.extensions { Iterator.count, Iterator.last }
```

## Constants

Group related constants, blank line between groups (preserved by formatter):

```ori
let $api_base = "https://api.example.com"
let $api_version = "v1"

let $timeout = 30s
let $max_retries = 3
```

## Comments

Comments must appear on their own line. Inline comments prohibited:

```ori
// Valid
let x = 42

let y = 42  // error: inline comment
```

Formatter normalizes spacing:

| Input | Output |
|-------|--------|
| `//comment` | `// comment` |
| `//  comment` | `// comment` |

### Doc Comments

Space after `//`, no space after marker. Required order (formatter reorders if wrong):

| Order | Marker | Purpose |
|-------|--------|---------|
| 1 | `#` | Description |
| 2 | `@param`, `@field` | Parameters/fields |
| 3 | `!` | Warning |
| 4 | `>` | Example |

```ori
// #Computes the sum of two integers.
// @param a The first operand.
// @param b The second operand.
// !Panics if overflow occurs.
// >add(a: 2, b: 3) -> 5
@add (a: int, b: int) -> int = a + b
```

Formatter normalizations:

| Input | Output |
|-------|--------|
| `//# Desc` | `// #Desc` |
| `// # Desc` | `// #Desc` |
| `//#Desc` | `// #Desc` |

`@param` order matches signature order. `@field` order matches struct field order.

## Ranges

No space around `..`/`..=`, space around `by`:

```ori
0..10
0..=100
0..100 by 5
10..0 by -1
```

## Destructuring

No space before `..` in rest patterns:

```ori
let { x, y } = point
let (first, second) = pair
let [$head, ..tail] = items
let [first, second, ..rest] = items
```

## Strings

Never break inside strings. Break the binding if needed:

```ori
let message =
    "This is a very long string that exceeds 100 characters but we never break inside"

let template =
    `Dear {user.name}, your order #{order.id} has been shipped.`
```

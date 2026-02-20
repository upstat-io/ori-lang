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
- 100 character line limit (default, configurable)
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
| Default type params | Space around `=` | `<Rhs = Self>` |
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
| `run` (top-level) | Function body; stacking shows sequential execution |
| `try` | Sequential blocks with error propagation; stacking shows execution order |
| `match` arms | Pattern matching; one arm per line aids readability |
| `recurse` | Named parameters pattern |
| `parallel` / `spawn` | Concurrency patterns |
| `nursery` | Structured concurrency pattern |

> **Note:** Nested blocks (inside for body, if body, etc.) follow width-based breaking (see [blocks](#blocks)).

### Independent Breaking

Nested constructs break independently based on their own width. Outer breaking does not force inner breaking.

---

## Function Signatures

Inline if ≤100 characters:

```ori
@add (a: int, b: int) -> int = a + b;

@transform (user_id: int, transform: (User) -> User) -> Result<User, Error> = do_work();
```

Break parameters if >100 characters:

```ori
@send_notification (
    user_id: int,
    notification: Notification,
    preferences: NotificationPreferences,
) -> Result<void, Error> = do_notify();
```

Break return type if `) -> Type =` still exceeds 100. Body on same line if it fits, otherwise indented:

```ori
@long_function_name (
    first: int,
    second: str,
) -> Result<HashMap<UserId, Preferences>, ServiceError> = do_work();

@long_function_name (
    first: int,
    second: str,
) -> Result<HashMap<UserId, Preferences>, ServiceError> =
    compute_something_complex(input: data);
```

## Function Calls

Inline if ≤100 characters:

```ori
let result = add(a: 1, b: 2);
let result = send_email(to: recipient, subject: title, body: content);
```

Break arguments if >100 characters (even single-argument calls):

```ori
let result = send_notification(
    user_id: current_user,
    message: notification_text,
    priority: Priority.High,
);

let result = process(
    data: some_very_long_variable_name_that_pushes_past_limit,
);
```

## Generics

Inline if ≤100 characters:

```ori
@identity<T> (x: T) -> T = x;
type Pair<T, U> = { first: T, second: U }
trait Add<Rhs = Self> { @add (self, rhs: Rhs) -> Self; }
```

Default type parameters have space around `=`:

```ori
trait Transform<Input = Self, Output = Input> {
    @transform (self, input: Input) -> Output;
}
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
@sort<T> (items: [T]) -> [T] where T: Comparable = do_sort();
```

Break if >100 characters. Constraints aligned:

```ori
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone + Debug,
          U: Default + Printable = do_it();
```

## Capabilities

Inline if ≤100 characters:

```ori
@fetch (url: str) -> Result<str, Error> uses Http = http_get(url);
```

Break if >100 characters. Capabilities stay comma-separated:

```ori
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache = do_it();
```

## Lists

Inline if ≤100 characters:

```ori
let nums = [1, 2, 3, 4, 5];
```

Simple items wrap multiple per line:

```ori
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
];
```

Complex items (structs, calls, nested collections) one per line:

```ori
let users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
    User { id: 3, name: "Charlie" },
];
```

Empty list: `[]`

## Maps

Inline if ≤100 characters:

```ori
let m = {"a": 1, "b": 2};
```

One entry per line if >100 characters:

```ori
let m = {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
};
```

Empty map: `{}`

## Tuples

Inline if ≤100 characters:

```ori
let pair = (1, "hello");
```

One element per line if >100 characters:

```ori
let data = (
    first_very_long_value,
    second_very_long_value,
);
```

Unit: `()`

## Struct Literals

Inline if ≤100 characters:

```ori
let p = Point { x: 0, y: 0 };
let u = User { id: 1, name: "Alice", active: true };
```

One field per line if >100 characters:

```ori
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
};
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
type Color = Red | Green | Blue;
type Result<T, E> = Ok(value: T) | Err(error: E);
```

One variant per line with leading `|` if >100 characters:

```ori
type Event =
    | Click(x: int, y: int)
    | KeyPress(key: char, modifiers: Modifiers)
    | Scroll(delta_x: float, delta_y: float);
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
    @to_str (self) -> str;
}

impl Printable for Point {
    @to_str (self) -> str = `({self.x}, {self.y})`;
}

impl Point {
    @new (x: int, y: int) -> Point = Point { x, y };

    @distance (self, other: Point) -> float = {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        sqrt(float(dx * dx + dy * dy))
    }
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
    {
        let doubled = x * 2;
        let validated = validate(doubled);
        validated
    };
```

## Conditionals

Inline if ≤100 characters:

```ori
let sign = if x > 0 then "positive" else "negative";
```

Break with `if` on new line, keeping `if cond then expr` together:

```ori
let category =
    if value > 100 then "large"
    else "small";
```

### Chained else-if

When an if-then-else has multiple else/else-if clauses, the first `if` stays on the assignment line (Kotlin style), and each `else` clause is indented:

```ori
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large";

let category = if score >= 90 then "A"
    else if score >= 80 then "B"
    else if score >= 70 then "C"
    else if score >= 60 then "D"
    else "F";
```

Branch bodies break independently:

```ori
let result = if condition then compute_simple(x: value)
    else compute_with_many_args(
        input: data,
        fallback: default,
    );
```

## Binary Expressions

Inline if ≤100 characters:

```ori
let result = a + b * c - d;
```

First operand on `let` line, break before operator:

```ori
let result = first_value + second_value
    - third_value * fourth_value
    + fifth_value / sixth_value;
```

### Long Boolean Expressions

When a boolean expression contains multiple `||` clauses, each clause receives its own line with `||` at the start of continuation lines:

```ori
x > 5 && x < 9
    || x == 1
    || x == 10
```

This rule applies when the expression exceeds line width or has three or more `||` clauses. The first clause remains on the initial line; subsequent clauses break with leading `||`.

## Method Chains

Inline if ≤100 characters:

```ori
let result = items.filter(x -> x > 0).map(x -> x * 2);
```

Receiver stays on assignment/yield line, break at every `.` once any break needed:

```ori
let result = items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .fold(0, (a, b) -> a + b);
```

In `for...yield` bodies, the same rule applies—receiver stays with `yield`, all methods break:

```ori
@process (items: [str]) -> [str] =
    for x in items yield x
        .to_upper()
        .trim()
        .replace(old: "O", new: "0");
```

## run/try

### Blocks

#### Top-level blocks

When a block `{ }` appears as a function body (top-level position), it is always stacked:

```ori
@process () -> int = {
    let x = get_value();
    let y = transform(x);

    x + y
}
```

#### Nested blocks

When a block appears nested inside another construct (for body, if body, lambda body, etc.), it follows width-based breaking. Inline if ≤100 characters:

```ori
let result = { let x = 1, let y = 2, x + y };
let doubled = { let v = compute(), v * 2 };

@with_cap () -> [int] uses Print =
    for x in [1, 2, 3] do {
        print(msg: x.to_str());
        x
    };
```

Stacked when contents exceed line width:

```ori
let result = {
    let x = compute();
    let y = transform(x);

    x + y
};
```

#### Contracts

Contracts go on the function declaration, not inside the block:

```ori
@divide (a: int, b: int) -> int
    pre(b != 0 | "divisor cannot be zero")
= a / b;

@compute (x: int) -> int
    post(r -> r >= 0)
= {
    let value = transform(x:);

    value
}
```

### try

`try { }` is always stacked (never inline):

```ori
let result = try {
    let data = fetch(url: endpoint)?;
    let parsed = parse(input: data)?;
    Ok(parsed)
};
```

## loop

### Simple Body

When `loop { }` contains a simple expression body, it stays inline if it fits:

```ori
loop {body()}
loop {process_next()}
```

### Complex Body

When `loop { }` contains a complex body (`try`, `match`, or `for`), the body is indented inside the braces:

```ori
loop {
        let input = read_input();
        if input == "quit" then break
        else process(input: input)
    }

loop {
    match get_command() {
        Quit -> break
        Process(data) -> handle(data: data)
        _ -> continue
    }
}

loop {
    try {
        let data = fetch_next()?;
        if data.is_empty() then break Ok(results)
        else results.push(data)
    }
}
```

## match

The `match` construct is always stacked regardless of length. This matches Rust and Kotlin behavior, where pattern matching arms are always formatted one per line for readability.

Scrutinee on first line, arms always stacked:

```ori
let label = match status {
    Pending -> "waiting"
    Running -> "in progress"
    Complete -> "done"
};
```

Arms with long calls break the call arguments (not after `->`):

```ori
let result = match event {
    Click(x, y) -> handle_click_with_long_name(
        x: x
        y: y
        options: defaults
    )
    KeyPress(key) -> handle_key(key)
};
```

Arms with always-stacked bodies break after `->`:

```ori
let result = match data {
    Valid(content) ->
        {
            let processed = process(content);
            Ok(processed)
        }
    Invalid(error) -> Err(error)
};
```

## recurse

Always stacked:

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
);
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
);
```

## timeout/cache/catch

Width-based. Inline if ≤100:

```ori
let result = timeout(op: fetch(url: endpoint), after: 5s);
let user = cache(key: "k", op: get(), ttl: 1m);
let safe = catch(expr: might_panic());
```

Stacked if >100:

```ori
let result = timeout(
    op: fetch(url: slow_endpoint),
    after: 5s,
);
```

## with Expressions

The `with...in` construct stays on one line when the body is short. It only breaks at `in` when the body is complex or the line exceeds 100 characters.

Inline if ≤100:

```ori
let result = with Http = mock_http in fetch(url: "/api");
let cached = with Cache = memory_cache in lookup(key: "user");
```

Broken with capabilities aligned. The break occurs at `in` when the body is complex:

```ori
let result =
    with Http = MockHttp { responses: default_responses }
    in fetch_user_data(user_id: current_user);

let result =
    with Http = mock_http,
         Logger = mock_logger
    in perform_operation(input: data);
```

Multiple capability bindings are comma-separated and aligned when broken.

## for Loops

### Inline vs. Broken

When the entire for loop (including nested loops) fits within 100 characters, it remains inline:

```ori
@short () -> [[int]] = for x in [1, 2] yield for y in [3, 4] yield x * y;
```

### Simple Expression Body

When the for body is a simple expression without control flow, it stays inline if it fits within 100 characters:

```ori
for x in items do print(msg: x);
let doubled = for x in items yield x * 2;
let squares = for n in 1..10 yield n * n;
```

#### Short Body Rule

A simple body (identifier, literal, or expression under approximately 20 characters) must remain with `yield`/`do` even when the overall line is long. A lone identifier or literal never appears on its own line:

```ori
// Correct: simple body stays with yield
@func () -> [int] =
    for x in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] yield x;

// Incorrect: lone identifier should never be isolated
@func () -> [int] =
    for x in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] yield
        x;
```

The breaking point moves to before `for` rather than after `yield` when the iterable is long but the body is simple.

#### Long Body Breaking

Body on next line when the body expression itself exceeds line width:

```ori
for user in users do
    process_user(user: user, options: default_options);

let results = for item in items yield
    transform(input: item, config: default_config);
```

### Control Flow in Body

When a `for...do` or `for...yield` body contains control flow constructs (`if`, `match`, or another `for`), break after `do`/`yield` with body on next line indented:

```ori
// Body contains if → break after do
for item in items do
    if item.active then process(item: item);

// Body contains match → break after yield
let labels = for status in statuses yield
    match status {
        Pending -> "waiting"
        Complete -> "done"
    };

// Body contains nested for → break after yield
let pairs = for x in xs yield
    for y in ys yield (x, y);
```

### Nested for

Since inner `for` is control flow, outer `for` body always breaks when it contains another `for`:

```ori
// Outer breaks because inner for is control flow
let matrix = for row in 0..height yield
    for col in 0..width yield
        compute(row: row, col: col);

// Triple nesting
let cube = for x in xs yield
    for y in ys yield
        for z in zs yield
            Point { x, y, z };
```

### Nested For - Rust-style Indentation

When nested for loops break (due to width), each nesting level gets its own line with incremented indentation. This follows Rust-style formatting for nested iterators:

```ori
// When breaking is needed, each level is indented
@deeper () -> [[[int]]] =
    for x in [1, 2, 3] yield
        for y in [4, 5, 6] yield
            for z in [7, 8, 9] yield x * y * z;
```

Each nested `for` introduces a new indentation level. The innermost body follows standard body breaking rules.

## nursery

Always stacked:

```ori
let results = nursery(
    body: n ->
        {
            n.spawn(task: fetch(url: "/a"));
            n.spawn(task: fetch(url: "/b"));
        },
    on_error: CancelRemaining,
    timeout: 30s,
);
```

## Imports

Stdlib first, relative second, blank line between. Sorted alphabetically:

```ori
use std.collections { HashMap, Set };
use std.math { abs, sqrt };
use std.time { Duration };

use "../utils" { format };
use "./helpers" { compute, validate };
```

Items sorted alphabetically. Break to multiple lines if >100 characters:

```ori
use std.collections {
    BTreeMap,
    BTreeSet,
    HashMap,
    HashSet,
    LinkedList,
};
```

Extension imports follow the same rules:

```ori
extension std.iter.extensions { Iterator.count, Iterator.last };

```

## Constants

Group related constants, blank line between groups (preserved by formatter):

```ori
let $api_base = "https://api.example.com";
let $api_version = "v1";

let $timeout = 30s;
let $max_retries = 3;
```

## Comments

Comments must appear on their own line. Inline comments prohibited:

```ori
// Valid
let x = 42;

let y = 42;  // error: inline comment
```

Formatter normalizes spacing:

| Input | Output |
|-------|--------|
| `//comment` | `// comment` |
| `//  comment` | `// comment` |

### Doc Comments

Space after `//`, space after marker. Required order (formatter reorders if wrong):

| Order | Marker | Purpose |
|-------|--------|---------|
| 1 | *(none)* | Description |
| 2 | `*` | Parameters/fields |
| 3 | `!` | Warning |
| 4 | `>` | Example |

```ori
// Computes the sum of two integers.
// * a: The first operand.
// * b: The second operand.
// ! Panics if overflow occurs.
// > add(a: 2, b: 3) -> 5
@add (a: int, b: int) -> int = a + b;
```

Formatter normalizations:

| Input | Output |
|-------|--------|
| `//*name:` | `// * name:` |
| `// *name:` | `// * name:` |
| `//! Warning` | `// ! Warning` |
| `//>example` | `// > example` |

`*` entries are reordered to match declaration order (parameters match signature order, fields match struct field order).

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
let { x, y } = point;
let (first, second) = pair;
let [$head, ..tail] = items;
let [first, second, ..rest] = items;
```

## Strings

Never break inside strings. Break the binding if needed:

```ori
let message =
    "This is a very long string that exceeds 100 characters but we never break inside"

let template =
    `Dear {user.name}, your order #{order.id} has been shipped.`
```

## Parentheses Preservation

Parentheses that are semantically required are preserved. This applies when an expression that would not normally be callable or indexable is used in such a position.

### Method Receiver

Iterator expressions as method receivers:

```ori
(for x in items yield x * 2).fold(0, (a, b) -> a + b)
(for x in items yield x).count()
```

Loop expressions as method receivers:

```ori
(loop {break 42}).unwrap()
(loop {break Some(value)}).map(f)
```

### Call Target

Lambda expressions as call targets:

```ori
(x -> x * 2)(5)
((a, b) -> a + b)(1, 2)
```

### Iterator Source

Iterator expressions in `for...in`:

```ori
for x in (for y in items yield y * 2) yield x + 1
for pair in (for x in xs yield (x, x * 2)) do process(pair)
```

### Parentheses Preservation

User parentheses are always preserved. The formatter never removes parentheses, even when not strictly required for precedence:

```ori
// Preserved: user's parens are kept for clarity
let x = (1 + 2);      // → let x = (1 + 2)  (unchanged)
let y = ((a));        // → let y = ((a))    (unchanged)

// Also preserved: precedence parens
let z = (1 + 2) * 3;  // → let z = (1 + 2) * 3
```

This ensures the formatter cannot accidentally change program semantics and respects programmer intent when parentheses are added for readability.

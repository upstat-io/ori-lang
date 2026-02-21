---
title: "Formatting"
description: "Ori Language Specification — Formatting"
order: 16
section: "Tooling"
---

# Formatting

Canonical source formatting. Zero-config, deterministic.

The `ori fmt` command produces a single canonical format for any valid Ori source file. The core principle is **width-based breaking**: constructs remain inline if they fit within the line width limit, otherwise they break according to construct-specific rules.

---

## General Rules

### Indentation

Four spaces per indentation level. Tabs are not permitted.

### Line Width

100 characters. This is the threshold for width-based breaking decisions.

### Trailing Commas

Required in multi-line constructs. Forbidden in single-line constructs.

```ori
// Single-line: no trailing comma
let $p = Point { x: 1, y: 2 };

// Multi-line: trailing comma required
let $config = Config {
    host: "localhost",
    port: 8080,
    debug: true,
};
```

### Semicolons

A semicolon terminates statements within blocks. The last expression in a block (the result expression) has no semicolon. A block where all expressions are terminated by semicolons produces `void`.

At module level, declarations ending with `}` (block bodies, type definitions with struct bodies, trait/impl blocks) have no trailing semicolon. All other declarations end with `;`.

```ori
// Block body: no ; after }
@process () -> int = {
    let $x = get_value();
    let $y = transform(x:);

    x + y
}

// Expression body: ; terminates
@add (a: int, b: int) -> int = a + b;

// Module-level: ; after non-block declarations
let $MAX = 100;
type UserId = int;
```

### Blank Lines

- One blank line between top-level declarations (functions, types, traits, impls)
- One blank line after the imports block
- One blank line after the constants block
- One blank line between trait/impl methods, except in single-method blocks
- No consecutive blank lines (collapsed to one)
- User blank lines within constant blocks are preserved for semantic grouping

### Blank Line Before Result Expression

In a block with two or more statements preceding the result expression, a blank line must separate the last statement from the result expression.

```ori
@compute (x: int) -> int = {
    let $a = step_one(x:);
    let $b = step_two(a:);

    a + b
}
```

### Whitespace Cleanup

Consecutive blank lines are collapsed to a single blank line. Trailing whitespace on all lines is stripped. Files end with a single newline character.

---

## Spacing Rules

| Context | Rule | Example |
|---------|------|---------|
| Binary operators | Space around | `a + b`, `x == y`, `a && b` |
| Arrows | Space around | `x -> x + 1`, `-> Type` |
| Colons (type annotations) | Space after | `x: int`, `key: value` |
| Commas | Space after | `f(a, b, c)` |
| Parentheses | No space inside | `f(x)`, `(a, b)` |
| Brackets | No space inside | `[1, 2]`, `items[0]` |
| Braces (all `{ }`) | Space inside | `Point { x, y }`, `{ a; b }` |
| Empty delimiters | No space | `[]`, `{}`, `()` |
| Field/member access `.` | No space | `point.x`, `std.math` |
| Range operators `..`/`..=` | No space | `0..10`, `0..=100` |
| Range step `by` | Space around | `0..100 by 5` |
| Spread `...` | No space after | `[...a, ...b]`, `f(...args)` |
| Unary operators | No space after | `-x`, `!valid`, `~mask` |
| Error propagation `?` | No space before | `fetch()?` |
| Nullish coalescing `??` | Space around | `a ?? b` |
| Labels `:` | No space around | `loop:outer`, `break:label` |
| Type conversion `as`/`as?` | Space around | `42 as float`, `"42" as? int` |
| Visibility `pub` | Space after | `pub @add`, `pub type` |
| Generic bounds `with` | Space around | `T with Clone` |
| Multi-trait `+` | Space around | `Printable + Debug` |
| Default type params `=` | Space around | `<Rhs = Self>` |
| Default param values `=` | Space around | `port: int = 8080` |
| Sum type variants `\|` | Space around | `Red \| Green \| Blue` |
| Compound assignment | Space around | `x += 1`, `flags \|= FLAG` |
| Comments `//` | Space after | `// comment` |
| Punning `:` | No space after | `f(name:, age:)` |

---

## Breaking Rules

### Width-Based Breaking

Most constructs follow a single rule: **inline if the construct fits within 100 characters, break otherwise**.

| Construct | Inline | Broken |
|-----------|--------|--------|
| Function parameters | All on one line | One per line, `)` on own line |
| Function arguments | All on one line | One per line, `)` on own line |
| Generic parameters | All on one line | One per line, `>` on own line |
| Where constraints | After signature | New indented line, aligned |
| Capabilities (`uses`) | After signature | New indented line |
| Contracts (`pre`/`post`) | After signature | Each on own indented line |
| Struct fields (definition) | All on one line | One per line |
| Struct fields (literal) | All on one line | One per line |
| Sum type variants | All on one line | One per line with leading `\|` |
| Map entries | All on one line | One per line |
| Tuple elements | All on one line | One per line |
| Import items | All on one line | One per line, sorted |
| Lists (simple items) | All on one line | Wrap (bin-pack) |
| Lists (complex items) | All on one line | One per line |
| Nested blocks | Inline | Stacked |
| `if-then-else` | Inline | `else` on new indented line |
| `for...do`/`yield` | Inline | Break after `do`/`yield` |
| `loop { }` | Inline | Stacked |
| `unsafe { }` | Inline | Stacked |
| `with...in` | Inline | Break at `in` |
| `timeout`/`cache`/`catch` | Inline | Stacked |
| Lambdas | Inline | Block body `{ }` |
| Method chains | Inline | Break at every `.` |
| Binary expressions | Inline | Break before operator |
| Destructuring patterns | Inline | One per line |
| Capset declarations | Inline, sorted | One per line, sorted |
| Conditional compilation attrs | Inline | One condition per line |
| Complex type annotations | Inline | Break at outermost `<>` or `->` |
| `impl Trait` with `where` | Inline | `where` on new indented line |

### Always-Stacked Constructs

These constructs are always formatted in stacked (multi-line) form regardless of width:

| Construct | Reason |
|-----------|--------|
| Function block body `= { }` | Top-level function bodies always stacked |
| `try { }` | Error-propagating blocks emphasize sequential steps |
| `match { }` | One arm per line aids pattern scanning |
| `recurse()` | Named parameter pattern with lambda arguments |
| `parallel()` / `spawn()` | Concurrency patterns with task lists |
| `nursery()` | Structured concurrency pattern |

### Independent Breaking

Nested constructs break independently based on their own width. An outer construct breaking does not force inner constructs to break. Each nested construct applies its own formatting rules.

The formatter does not enforce a maximum nesting depth. Deep nesting is a code quality concern, not a formatting concern.

---

## Declarations

### Functions

Expression body inline when the entire declaration fits within 100 characters. Block body always stacked.

```ori
// Expression body — inline
@add (a: int, b: int) -> int = a + b;

// Block body — always stacked
@process (input: str) -> Result<str, Error> = {
    let $data = parse(input:);
    let $result = transform(data:);

    Ok(result)
}
```

Opening brace `{` appears on the same line as `=` (K&R/1TBS style). Closing `}` aligns with the declaration keyword (`@`, `trait`, `impl`, etc.).

### Parameters

All parameters inline if the declaration fits within 100 characters. Otherwise, one parameter per line with `)` on its own line. Trailing comma in multi-line form.

```ori
// Inline
@connect (host: str, port: int = 8080) -> Connection = { ... }

// Broken — one per line
@configure (
    host: str = "localhost",
    port: int = 443,
    timeout: Duration = 30s,
    retries: int = 3,
) -> Config = { ... }
```

Default parameter values use `= expr` with spaces around `=`. The default stays with its parameter when breaking.

Variadic parameters use `...` attached to the type with no space: `nums: ...int`.

### Return Type

The return type stays on the `)` line. The body breaks to the next line if the full declaration exceeds 100 characters.

```ori
// Return type on ) line
@long_name (
    first: int,
    second: str,
) -> Result<Data, Error> = compute(first:, second:);

// Body breaks to next line when too long
@long_name (
    first: int,
    second: str,
) -> Result<Data, Error> =
    compute_something_complex(input: data);
```

### Generic Parameters

Inline if the declaration fits. One per line otherwise. Uses `T with Trait` syntax for bounds.

```ori
// Inline
@identity<T> (x: T) -> T = x;
@sort<T with Comparable> (items: [T]) -> [T] = { ... }

// Broken — one per line
@complex<
    T with Comparable,
    U with Hashable,
    $N: int,
    $M: int,
> (items: [T], keys: [U]) -> [[T]]
    where N > 0,
          M > 0
= { ... }
```

Const generic parameters use `$N: type` in generic brackets. Const bounds use `where N > 0` or compound expressions like `where N > 0 && N <= 100`.

### Where Clauses

Inline if the declaration fits. Otherwise, `where` on a new indented line with constraints aligned.

```ori
// Inline
@sort<T> (items: [T]) -> [T] where T with Comparable = { ... }

// Broken — where on new line
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T with Clone + Debug,
          U with Default + Printable
= { ... }
```

### Capabilities

Inline if the declaration fits. Otherwise, on a new indented line, comma-separated.

```ori
// Inline
@fetch (url: str) -> Result<str, Error> uses Http = { ... }

// Broken
@complex_operation (input: Data) -> Result<Output, Error>
    uses Http, FileSystem, Logger, Cache
= { ... }
```

### Contracts

Inline if the declaration fits. Otherwise, each contract on its own indented line. Canonical order: `where` → `uses` → `pre` → `post`.

```ori
// Inline
@clamp (n: int, lo: int, hi: int) -> int pre(lo <= hi) = { ... }

// Broken — each on own line
@process<T with Comparable> (items: [T]) -> [T]
    where T with Clone
    uses FileSystem
    pre(!items.is_empty() | "items must not be empty")
    post(r -> r.len() <= items.len())
= { ... }

// Multiple pre conditions
@range_check (low: int, high: int, value: int) -> bool
    pre(low <= high | "low must not exceed high")
    pre(value >= 0 | "value must be non-negative")
= value >= low && value <= high;
```

### Visibility

`pub` is a prefix with a space before the declaration keyword.

```ori
pub @process (input: str) -> str = { ... }
pub type Config = { ... };
pub let $VERSION = "1.0.0";
pub trait Serializable { ... }
pub def impl Printable { ... }
pub extend str { ... }
```

### Pattern Parameters and Guards

Each clause of a pattern-matched function is a separate declaration. Guards use `if` after parameters. Long guards break to an indented line. Clauses of the same function are separated by blank lines.

```ori
@factorial (0: int) -> int = 1;

@factorial (n: int) -> int = n * factorial(n - 1);

@classify (n: int) -> str if n > 0 = "positive";

@classify (n: int) -> str if n < 0 = "negative";

@classify (_: int) -> str = "zero";
```

### Const Functions

Const functions use the `$` prefix instead of `@`. All other formatting rules are identical to regular functions.

```ori
$factorial (n: int) -> int = if n <= 1 then 1 else n * factorial(n - 1);

$fibonacci (n: int) -> int = {
    if n <= 1 then n
    else fibonacci(n - 1) + fibonacci(n - 2)
}
```

### Test Declarations

Test attributes appear on their own line above. `tests @target` stays on the same line as the function name. Block body is always stacked. Test declaration order is preserved (no reordering).

```ori
@test_add tests @add () -> void = {
    assert_eq(actual: add(a: 1, b: 2), expected: 3);
}

#skip("not yet implemented")
@test_advanced tests @parse () -> void = {
    assert_eq(actual: parse(input: "42"), expected: 42);
}

#compile_fail("E0042")
@test_bad_type tests @process () -> void = {
    process(input: 42);
}

// Multi-target test
@test_both tests @parse tests @format () -> void = {
    let $parsed = parse(input: "42");
    let $formatted = format(value: parsed);
    assert_eq(actual: formatted, expected: "42");
}
```

### `@main` Entry Points

`@main` follows all regular function formatting rules. No special treatment.

```ori
@main (args: [str]) -> void
    uses Http, FileSystem
= {
    let $data = fetch(url: args[0]);
    write_file(path: args[1], content: data);
}
```

---

## Type Definitions

### Structs

Width-based. Fields one per line when broken. Trailing comma in multi-line form.

```ori
// Inline
type Point = { x: int, y: int };

// Broken
type User = {
    id: int,
    name: str,
    email: str,
    created_at: Duration,
};
```

### Structs with Bounds

`where` clause appears before `=`. When `where` is present and breaks, `=` goes on its own line.

```ori
type Wrapper<T with Clone> = { value: T };

type SortedMap<K, V>
    where K with Comparable + Hashable
= {
    keys: [K],
    values: [V],
    size: int,
}
```

### Sum Types

Inline if fits. When broken, variants use leading `|` on each line. Variant payloads break independently.

```ori
// Inline
type Color = Red | Green | Blue;

// Broken — leading |
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(a: float, b: float, c: float);

// Complex payloads break independently
type Event =
    | Click(x: int, y: int, button: MouseButton)
    | KeyPress(key: Key, modifiers: Set<Modifier>)
    | Resize(
        width: int,
        height: int,
        old_width: int,
        old_height: int,
    )
    | Close;
```

### Newtypes

Newtype declarations are always inline. Derives appear on their own line above.

```ori
type UserId = int;

#derive(Eq, Clone, Debug, Hashable)
type UserId = int;
```

### Trait Object Types

Trait object types are formatted as regular types. Multi-trait uses `+` with spaces around. Breaks at `+` when long.

```ori
@display (item: Printable) -> str = item.to_str();
@debug_print (item: Printable + Debug) -> void = { ... }
```

### Existential Types (`impl Trait`)

Inline if fits. `where` on associated types breaks to a new indented line when long.

```ori
// Inline
@iter (self) -> impl Iterator where Item == int = { ... }

// Broken — where on new line
@pairs (self) -> impl Iterator
    where Item == (str, int)
= { ... }
```

### Complex Type Annotations

Inline if fits. Break at outermost `<>` or `->` first. Inner types break independently.

```ori
// Inline
let $handler: (int) -> Result<str, Error> = process;

// Function type — break before ->
let $processor: (Config, [UserData], {str: int})
    -> Result<[ProcessedData], Error> = pipeline;

// Deeply nested — break at each level
let $complex: Result<
    {str: [Option<UserData>]},
    Error,
> = fetch_all();
```

---

## Trait and Impl Blocks

### Trait Bodies

Opening `{` on same line. Canonical order within trait: associated types first, then required methods (no body), then default methods (with body). Blank line between each group. Single-method traits skip blank lines.

```ori
trait Printable {
    @to_str (self) -> str
}

trait Collection {
    type Item;
    type Index = int;

    @get (self, index: Self.Index) -> Option<Self.Item>
    @len (self) -> int

    @is_empty (self) -> bool = {
        self.len() == 0
    }
}
```

### Impl Bodies

Associated type assignments first, then methods in trait declaration order. Blank line between methods.

```ori
impl Printable for Point {
    @to_str (self) -> str = `({self.x}, {self.y})`;
}

impl Iterator for Range {
    type Item = int;

    @next (self) -> (Option<int>, Self) = {
        if self.current >= self.end then (None, self)
        else (Some(self.current), { ...self, current: self.current + 1 })
    }
}

// Generic impl
impl<T with Printable> Printable for [T] {
    @to_str (self) -> str = {
        let $items = for item in self yield item.to_str();

        "[" + items.join(separator: ", ") + "]"
    }
}
```

### Default Implementations

`def` is a prefix like `pub`. Body formatting is identical to regular `impl` blocks.

```ori
def impl Printable {
    @to_str (self) -> str = self.debug();
}

pub def impl Comparable {
    @compare (self, other: Self) -> Ordering = {
        Ordering.Equal
    }
}
```

### Extensions

`extend` blocks follow the same formatting as `impl` blocks.

```ori
pub extend str {
    @words (self) -> [str] = self.split(separator: " ");

    @lines (self) -> [str] = self.split(separator: "\n");
}

extend<T with Printable> [T] {
    @join_str (self, separator: str) -> str = {
        for item in self yield item.to_str()
    }
}
```

---

## Expressions

### Blocks

Function-body blocks are always stacked. Nested blocks (inside `let`, `for`, `if`, etc.) follow width-based breaking.

```ori
// Function body — always stacked
@compute () -> int = {
    let $x = 1;
    let $y = 2;

    x + y
}

// Nested block — inline if fits
let $v = { let $x = 1; x + 2 };

// Nested block — stacked when long
let $result = {
    let $x = compute();
    let $y = transform(x:);

    x + y
};
```

### Try Blocks

`try { }` is always stacked. Never inline.

```ori
let $result = try {
    let $file = open(path:)?;
    let $data = read(file:)?;
    let $parsed = parse(input: data)?;

    validate(data: parsed)?
};
```

### Unsafe Blocks

Width-based. Inline if fits, stacked otherwise.

```ori
// Inline
let $value = unsafe { ptr_read(ptr:) };

// Stacked
let $data = unsafe {
    let $ptr = get_raw_pointer();
    let $value = ptr_read(ptr:);
    validate(value:)
};
```

### If-Then-Else

Inline if fits. `if cond then expr` stays together. Chained `else if` each on own line. `else` on new indented line when breaking. Mixed inline and block bodies are permitted.

```ori
// Inline
let $sign = if x > 0 then "positive" else "negative";

// Chained else-if
let $grade = if score >= 90 then "A"
    else if score >= 80 then "B"
    else if score >= 70 then "C"
    else "F";

// Block bodies
let $result = if condition1 then {
    let $x = compute_a();
    process(x:)
}
else if condition2 then {
    let $y = compute_b();
    process(y:)
}
else {
    default_value()
};

// Long condition — breaks before && / ||
let $status = if user.is_active
    && user.has_permission(perm: "write")
    && !user.is_suspended
    then "active"
    else "inactive";

// Void if (no else)
if should_log then log(msg: "event occurred");

if should_process then {
    validate(input:);
    process(input:);
};
```

### For Loops with `yield`

Inline if fits. Filter `if` stays on the same line. Complex yield body uses block `{ }`. Nested `for` each on own line.

```ori
// Inline
let $doubled = for x in items yield x * 2;
let $evens = for x in items if x % 2 == 0 yield x;

// Breaking — yield on own line
let $names = for user in users
    yield user.profile.display_name;

// With filter — breaking
let $active = for user in users
    if user.is_active && user.age >= 18
    yield user.name;

// Block yield body
let $records = for item in items yield {
    let $processed = validate(item:);
    let $formatted = format(data: processed);

    Record { data: formatted, timestamp: now() }
};

// Nested for
let $pairs = for x in xs
    for y in ys
    yield (x, y);
```

### For Loops with `do`

Inline if fits. Block `do { }` for multi-statement bodies.

```ori
// Inline
for item in items do print(msg: item);
for x in items if x > 0 do process(value: x);

// Block do body
for user in users do {
    let $profile = fetch_profile(id: user.id);
    update_cache(key: user.id, value: profile);
};

// With label
for:outer item in items do {
    for:inner sub in item.children do {
        if sub.is_invalid then break:outer;
        process(sub:);
    };
};
```

### Loop

Width-based. Inline if fits, stacked otherwise.

```ori
// Inline
loop { process_next() }

// Stacked
loop {
    let $input = read_input();
    if input == "quit" then break;
    process(input:);
}
```

Labels attach to the keyword with no space: `loop:name { }`.

### Match

Always stacked. One arm per line. Trailing comma after every arm.

```ori
let $msg = match status {
    Ok(value) -> `Success: {value}`,
    Err(e) -> `Error: {e}`,
};
```

Guards stay inline with the pattern. Block arm bodies use `-> { }` with closing `},` aligned.

```ori
let $label = match score {
    n if n >= 90 -> "A",
    n if n >= 80 -> "B",
    _ -> "F",
};

let $result = match event {
    Click(x, y, button) -> {
        let $target = find_target(x:, y:);
        handle_click(target:, button:)
    },
    Close -> shutdown(),
};
```

### Or-Patterns in Match

Inline if fits. When breaking, each alternative gets its own line with leading `|`. Body `->` goes on the last pattern's line.

```ori
// Inline
let $is_vowel = match c {
    'a' | 'e' | 'i' | 'o' | 'u' -> true,
    _ -> false,
};

// Broken — leading |
let $msg = match error {
    NotFound(path:)
    | PermissionDenied(path:)
    | AccessError(path:) -> {
        log(msg: `File error: {path}`);
        default_value()
    },
    Timeout -> retry(),
};
```

### Method Chains

Inline if fits. When any break is needed, receiver stays on the first line and all subsequent dots break (all-or-nothing). Method arguments break independently inside.

```ori
// Inline
let $result = items.filter(predicate: x -> x > 0).map(transform: x -> x * 2);

// Broken — all dots break
let $result = items
    .filter(predicate: x -> x > 0)
    .map(transform: x -> x * 2)
    .fold(initial: 0, op: (a, b) -> a + b);
```

Associated function calls (`Type.method()`) follow the same rules.

```ori
let $p = Point.new(x: 10, y: 20);
let $result = Point.new(x: 1, y: 2)
    .distance_to(other: origin);
```

### Binary Expressions

Inline if fits. Break before the operator. First operand on assignment line. Continuation lines start with the operator, indented.

```ori
let $result = first_value + second_value
    - third_value * fourth_value
    + fifth_value;

// || chains: each clause on own line
let $valid = is_admin
    || is_moderator
    || has_permission(perm: "write");
```

### Lambdas

Inline if fits. No parens for single untyped param. Block body for multi-statement.

```ori
// Inline
x -> x + 1
(a, b) -> a + b
() -> 42

// Block body
(x) -> {
    let $processed = validate(value: x);
    transform(processed:)
}
```

### Labels

No space around `:` in labels. Label attaches directly to the keyword.

```ori
loop:outer {
    loop:inner {
        if done then break:outer;
        if skip then continue:inner;
    };
};
```

### Break with Value

Space between `break` and value. Value on the same line when short. Long value breaks to the next indented line.

```ori
let $found = loop {
    let $item = next();
    if item.matches(query:) then break item;
};
```

### Error Propagation

`?` is attached to the expression with no space (postfix). `??` is a binary operator with space around that breaks before the operator like other binary operators.

```ori
// ? attached
let $data = read_file(path:)?;
let $name = get_user(id:)?
    .profile()?
    .display_name();

// ?? breaks before operator
let $connection = try_primary_db()
    ?? try_secondary_db()
    ?? try_fallback_db()
    ?? panic(msg: "no database available");
```

### Assignments

Compound assignment operators (`+=`, `-=`, `|=`, etc.) have space around the operator. Index and field assignments format like `let` bindings.

```ori
count += 1;
list[0] = new_value;
state.name = new_name;
state.items[i] = new_item;
```

---

## Literals and Collections

### Lists

Simple items (literals, identifiers) wrap (bin-pack) when broken. Complex items (structs, calls, nested collections) one per line.

```ori
// Inline
let $nums = [1, 2, 3, 4, 5];

// Simple items — wrap
let $nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
];

// Complex items — one per line
let $users = [
    User { id: 1, name: "Alice" },
    User { id: 2, name: "Bob" },
];
```

### Maps

Inline if fits. One entry per line when broken.

```ori
let $m = { "a": 1, "b": 2 };

let $config = {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
};
```

### Tuples and Struct Literals

Width-based. One element/field per line when broken. Field shorthand supported.

```ori
let $pair = (1, "hello");
let $p = Point { x: 0, y: 0 };

// Broken
let $config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
};
```

### Spread

`...` attached to the expression with no space. Spread on its own line when the collection is broken.

```ori
let $combined = [...first, ...second];

let $updated = {
    ...original,
    name: new_name,
    email: new_email,
};
```

### Ranges

No space around `..`/`..=`. Space around `by`.

```ori
0..10
0..=100
0..100 by 5
10..0 by -1
```

### Strings and Templates

Never break inside string or template string content. Break the containing construct instead. No space inside `{}` interpolation braces.

```ori
// Break the binding, not the string
let $report =
    `User {user.name} logged in from {ip} at {timestamp}`;

// Extract to variables for very long templates
let $user_info = user.display_name;
let $report = `User {user_info} logged in from {location} at {time}`;
```

### Named Arguments

Punning form `name:` (no space after colon) when variable matches parameter. Full form `name: value` (space after colon) otherwise.

```ori
// Punning
let $p = Point.new(x:, y:);

// Full form
let $p = Point.new(x: 10, y: 20);

// Mixed
let $conn = Database.connect(
    host:,
    port:,
    username: "admin",
    password: get_password(),
);

// Single-param with inline lambda — no name needed
list.map(x -> x * 2);
```

---

## Control Flow Expressions

### `with...in` Capability Binding

Inline if fits. When breaking, capabilities are comma-separated and aligned under the first. `in` keyword on its own line. Stateful handlers are always stacked.

```ori
// Inline
let $result = with Http = mock in fetch(url:);

// Multiple — breaking
let $result =
    with Http = mock,
         Cache = mock,
         Clock = test_clock
    in {
        let $data = fetch(url:);
        process(data:)
    };

// Stateful handler — always stacked
let $result =
    with Logger = handler(state: []) {
        log: (s, msg) -> {
            ([...s, msg], void)
        },
    }
    in {
        log(msg: "starting");
        do_work()
    };
```

### Always-Stacked Function Expressions

`recurse`, `parallel`, `spawn`, `nursery` are always stacked. Named arguments one per line. Lambda bodies break independently. Trailing comma.

```ori
let $factorial = recurse(
    condition: n -> n <= 1,
    base: _ -> 1,
    step: (n, self) -> n * self(n - 1),
);

let $results = parallel(
    tasks: [
        () -> fetch(url: url1),
        () -> fetch(url: url2),
    ],
    max_concurrent: 2,
    timeout: 30s,
);

nursery(
    body: (n) -> {
        n.spawn(task: () -> worker(id: 1));
        n.spawn(task: () -> worker(id: 2));
    },
    on_error: NurseryErrorMode.CancelRemaining,
    timeout: 60s,
);
```

### Width-Based Function Expressions

`timeout`, `cache`, `catch`, `with()` follow width-based breaking. Inline when short, stacked when long.

```ori
// Inline
let $result = catch(expr: parse(input:));
let $data = timeout(op: () -> fetch(url:), after: 5s);

// Stacked
let $result = with(
    acquire: () -> open_file(path:),
    action: (file) -> {
        let $data = read(file:);
        process(data:)
    },
    release: (file) -> close(file:),
);
```

---

## Module Organization

### File Layout

The formatter enforces this ordering at the top of the file:

1. File-level attributes (`#!target(...)`)
2. Imports (sorted and grouped)
3. Constants (grouped, user blank lines preserved)
4. Everything else in user-defined order

```ori
#!target(os: "linux")

use std.collections { HashMap };
use std.io { read_file, write_file };

use "../config" { Config };
use "./models" { User };

extension std.iter.extensions { Iterator.count };

let $VERSION = "1.0.0";
let $MAX_RETRIES = 3;

type AppConfig = { ... };

@process () -> void = { ... }

@main () -> void = { ... }
```

### Imports

Stdlib imports first, relative imports second. Each group sorted alphabetically by module path. Blank line between groups. Items within `{ }` sorted alphabetically.

```ori
// Group 1: stdlib (sorted)
use std.collections { BTreeMap, HashMap, HashSet };
use std.io { read_file, write_file };
use std.testing { assert_eq };

// Group 2: relative (sorted)
use "../config" { Config, defaults };
use "./models" { Post, User };
use "./utils" { format_date, validate };
```

Extension imports appear after regular imports in the same group. Methods sorted alphabetically.

```ori
extension std.iter.extensions { Iterator.count };
extension std.collections.extensions {
    List.chunk,
    List.flatten,
    List.unique,
    Map.merge,
};
```

### Extern Blocks

Opening `{` on the same line. Declarations indented. `as "alias"` stays on the declaration line.

```ori
extern "c" from "libm" {
    @_sin (x: float) -> float as "sin";
    @_cos (x: float) -> float as "cos";
    @_sqrt (x: float) -> float as "sqrt";
}

extern "js" from "./utils.js" {
    @_parse (input: str) -> JsValue as "parse";
}
```

### Capset Declarations

Inline if fits. Capabilities sorted alphabetically. One per line when broken with trailing comma.

```ori
capset Net = Dns, Http, Tls;

capset Full =
    Cache,
    Clock,
    Crypto,
    Dns,
    FileSystem,
    Http,
    Logger,
    Print,
    Random,
    Tls;
```

---

## Attributes

Each attribute on its own line above the declaration. Multiple attributes use canonical order: `#target`/`#cfg` → `#repr` → `#derive` → `#skip`/`#compile_fail`/`#fail`. No blank lines between stacked attributes.

```ori
#target(os: "linux")
#repr("c")
#derive(Eq, Clone, Debug)
type NativeBuffer = {
    ptr: CPtr,
    len: c_size,
    cap: c_size,
};
```

Conditional compilation attributes follow width-based breaking: inline if fits, one condition per line otherwise.

```ori
#target(os: "linux", arch: "x86_64")
@simd_add (a: int, b: int) -> int = { ... }

#target(
    os: "linux",
    arch: "x86_64",
    family: "unix",
)
@platform_specific () -> void = { ... }
```

`#repr` attributes are always inline.

```ori
#repr("c")
#repr("aligned", 16)
#repr("transparent")
```

---

## Destructuring

Width-based. One element per line when broken. Rest patterns `..rest` and `..$rest` have no space before `..`. Nested patterns break independently.

```ori
// Inline
let (x, y) = get_point();
let { name, age } = get_user();
let [$head, ..tail] = items;

// Broken — one per line
let {
    name,
    email,
    age,
    address,
    phone,
} = get_full_profile();

// Nested — breaks independently
let {
    name,
    address: {
        street,
        city,
        state,
        zip,
    },
} = get_user();
```

---

## Parentheses

All user parentheses are preserved. The formatter never removes parentheses, even when not strictly required for precedence. This ensures the formatter cannot change program semantics.

```ori
// Preserved for clarity
let $x = (a + b) * c;

// Preserved: user intent
let $y = (1 + 2);

// Required: expression as method receiver
(for x in items yield x * 2).fold(initial: 0, op: (a, b) -> a + b)

// Required: lambda as call target
(x -> x * 2)(5)
```

---

## Comments

Comments must appear on their own line. Inline (end-of-line) comments are prohibited. The formatter normalizes spacing: `//comment` becomes `// comment`.

### Doc Comments

Space after `//`, space after doc marker. Formatter enforces marker order and reorders `*` entries to match declaration parameter order.

| Order | Marker | Purpose |
|-------|--------|---------|
| 1 | *(none)* | Description |
| 2 | `*` | Parameters/fields |
| 3 | `!` | Errors/warnings |
| 4 | `>` | Examples |

```ori
// Computes the sum of two integers.
// * a: The first operand.
// * b: The second operand.
// ! Panics if overflow occurs.
// > add(a: 2, b: 3) -> 5
@add (a: int, b: int) -> int = a + b;
```

Normalizations:

| Input | Output |
|-------|--------|
| `//comment` | `// comment` |
| `//  comment` | `// comment` |
| `//*name:` | `// * name:` |
| `//!Warning` | `// ! Warning` |
| `//>example` | `// > example` |

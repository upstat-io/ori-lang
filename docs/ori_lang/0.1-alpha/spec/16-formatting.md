---
title: "Formatting"
description: "Ori Language Specification — Formatting"
order: 16
---

# Formatting

Canonical source formatting. Zero-config, deterministic.

## Rules Summary

**General**
- 4 spaces indentation, no tabs
- 100 character line limit
- Trailing commas required in multi-line, forbidden in single-line
- No consecutive, leading, or trailing blank lines

**Spacing**
- Space around binary operators and arrows
- Space after colons and commas
- No space inside parentheses or brackets
- No space inside empty delimiters: `[]`, `{}`, `()`
- Space after `//`

**Blank Lines**
- One after imports
- One after config block
- One between top-level declarations (functions, types, traits, impls)
- One blank line between trait/impl methods

**Named Arguments**
- Inline if: fits in 100 chars AND no value >30 chars AND no complex values
- Stacked otherwise (one per line, trailing commas)

**Collections**
- Lists: inline if fits, else bump brackets and wrap at column width
- Maps: inline if ≤2 entries and fits, else one entry per line
- Tuples: inline if fits, else one element per line
- Sets: same as lists

**Struct Literals**
- Inline if ≤3 fields AND fits in 60 chars
- Stacked otherwise, one field per line

**Lambdas**
- No parens for single untyped param: `x -> x + 1`
- Parens for zero, multiple, or typed params: `() -> 42`, `(a, b) -> a + b`
- Break before `->` if body is complex or multi-line

**Functions**
- Short signatures inline
- Long signatures: break params (one per line) or break after `->`
- Space between `@name` and `(`

**Generics**
- Inline if fits: `<T, U>`
- Break if >3 params or exceeds 40 chars: one per line

**Where Clauses**
- Inline if single short constraint: `where T: Clone`
- Otherwise start on new line, one constraint per line

**Type Definitions**
- Struct fields: one per line if >2 fields or any field >30 chars
- Sum variants: one per line
- Attributes on own line above type

**Trait/Impl Blocks**
- Opening brace on same line
- Methods indented, one blank line between methods
- Closing brace on own line

**Expressions**
- Binary: break before operator
- Chains: each call on own line if >2 calls or exceeds line width
- `run`/`try`: always stacked
- `match`: scrutinee on first line, arms on separate lines
- `if`/`else`: inline if short, else `then`/`else` on own lines

**Imports**
- Sorted alphabetically within groups
- Stdlib first, relative second, blank line between
- Import items sorted alphabetically
- One import per line if >4 items

**Config**
- Group related configs together
- One blank line between groups

**Comments**
- Own line only (no inline comments)
- Space after `//`, no space after doc marker
- Doc comment order: `#`, `@param`/`@field`, `!`, `>`
- `@param` order matches signature; `@field` order matches struct

---

## General

- **Indent**: 4 spaces, no tabs
- **Line length**: 100 characters max
- **Trailing commas**: Required in multi-line, forbidden in single-line

## Spacing

```
a + b                    // binary operators
x -> x + 1               // arrows
let x: int = 42          // colons
f(a, b, c)               // commas
f(x)  [1, 2]  (a, b)     // no space inside parens/brackets
// comment               // space after //
```

## Blank Lines

- One after imports
- One after config
- One between functions
- No consecutive, leading, or trailing blank lines

## Named Arguments

**Inline** when:
- Fits in 100 chars, AND
- No value >30 chars, AND
- No complex values (lists, maps, nested calls)

```ori
assert_eq(actual: result, expected: 10)
```

**Stacked** otherwise:

```ori
assert_eq(
    actual: open_doors(),
    expected: [1, 4, 9, 16, 25, 36, 49, 64, 81, 100],
)
```

## Lists

Inline if fits:

```ori
let nums = [1, 2, 3]
```

Long lists bump brackets and wrap values at column width:

```ori
let nums = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    21, 22, 23, 24, 25,
]
```

Empty list has no space: `[]`

## Maps

Inline if ≤2 entries and fits:

```ori
let m = {"a": 1, "b": 2}
```

Otherwise one entry per line:

```ori
let m = {
    "name": "Alice",
    "age": 30,
    "email": "alice@example.com",
}
```

Empty map has no space: `{}`

## Tuples

Inline if fits:

```ori
let pair = (1, "hello")
```

Multi-line if long:

```ori
let data = (
    first_very_long_value,
    second_very_long_value,
)
```

Unit has no space: `()`

## Struct Literals

Inline if ≤3 fields AND total ≤60 chars:

```ori
let p = Point { x: 0, y: 0 }
let u = User { id: 1, name: "Alice", active: true }
```

Stacked otherwise:

```ori
let config = Config {
    timeout: 30s,
    max_retries: 3,
    base_url: "https://api.example.com",
    debug_mode: false,
}
```

Field shorthand kept inline if eligible:

```ori
let p = Point { x, y }
```

## Lambdas

No parens for single untyped param:

```ori
x -> x + 1
items.map(transform: x -> x * 2)
```

Parens required for zero, multiple, or typed params:

```ori
() -> 42
(a, b) -> a + b
(x: int) -> int = x * 2
```

Break before `->` if body complex:

```ori
let process = (x: int) -> int =
    run(
        let y = compute(x),
        y * 2,
    )
```

## Generics

Inline if fits:

```ori
type Pair<T, U> = { first: T, second: U }
@identity<T> (x: T) -> T = x
```

Break if >3 params or >40 chars:

```ori
type Complex<
    Input,
    Output,
    Error,
    Context,
> = ...
```

## Where Clauses

Inline if single short constraint:

```ori
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

New line with multiple or long constraints:

```ori
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone + Default,
          U: Printable = ...
```

## Type Definitions

Struct with ≤2 short fields inline:

```ori
type Point = { x: int, y: int }
```

Otherwise one field per line:

```ori
type User = {
    id: int,
    name: str,
    email: str,
    created_at: Timestamp,
}
```

Sum type variants always one per line:

```ori
type Status =
    | Pending
    | Running(progress: int)
    | Done
    | Failed(error: Error)
```

Attributes on own line:

```ori
#[derive(Eq, Clone)]
type Point = { x: int, y: int }
```

## Trait/Impl Blocks

Opening brace on same line, one blank line between methods:

```ori
trait Printable {
    @to_string (self) -> str
}

impl Printable for Point {
    @to_string (self) -> str = "(" + str(self.x) + ", " + str(self.y) + ")"
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

## Conditionals

Short conditionals inline:

```ori
if x > 0 then "positive" else "non-positive"
```

Multi-line when branches are complex:

```ori
if condition
    then compute_positive_result()
    else compute_negative_result()
```

Chained else-if:

```ori
if n % 15 == 0 then "FizzBuzz"
else if n % 3 == 0 then "Fizz"
else if n % 5 == 0 then "Buzz"
else str(n)
```

## Signatures

```ori
@add (a: int, b: int) -> int = a + b

@process (
    input: [int],
    transform: (int) -> int,
) -> [int] = run(...)
```

## Binary Expressions

Break before operator:

```ori
let result = long_expr
    + another_expr
```

## Chains

```ori
items
    .filter(predicate: x -> x > 0)
    .map(transform: x -> x * 2)
```

## run/try

Always stacked:

```ori
run(
    let x = compute(),
    x + 1,
)
```

## match

Arms on separate lines:

```ori
match(value,
    Some(x) -> x,
    None -> 0,
)
```

## Imports

Sorted alphabetically within groups. Stdlib first, relative second:

```ori
use std.collections { HashMap, Set }
use std.math { abs, sqrt }
use std.time { Duration }

use '../utils' { format }
use './helpers' { compute, validate }
use './local' { helper }
```

Import items sorted alphabetically:

```ori
use std.math { abs, cos, sin, sqrt, tan }
```

Break to multiple lines if >4 items:

```ori
use std.math {
    abs,
    cos,
    pow,
    sin,
    sqrt,
    tan,
}
```

## Config

Group related configs, blank line between groups:

```ori
$api_base = "https://api.example.com"
$api_version = "v1"

$timeout = 30s
$max_retries = 3

$debug_mode = false
```

## Comments

Comments must appear on their own line. Inline comments prohibited.

```ori
// Valid
let x = 42

let y = 42  // error: inline comment
```

Space after `//`:

```ori
// Correct
//Wrong
```

### Doc Comments

Space after `//`, no space after marker.

**Required order** (formatter reorders if wrong):

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

Formatter fixes:
```
//# Wrong      ->  // #Wrong
// # Wrong     ->  // #Wrong
//#Wrong       ->  // #Wrong

// >example    ->  // #Desc
// #Desc            // >example

// @param b Second.   ->  // @param a First.
// @param a First.        // @param b Second.
@add (a: int, b: int)     @add (a: int, b: int)
```

`@param` order matches signature order. `@field` order matches struct field order.

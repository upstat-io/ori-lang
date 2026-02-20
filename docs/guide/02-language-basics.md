---
title: "Language Basics"
description: "Learn the fundamentals: types, variables, operators, and control flow."
order: 2
---

# Language Basics

This guide teaches you the core building blocks of Ori through progressive examples. By the end, you'll understand variables, types, expressions, and control flow well enough to write real programs.

## Everything Is an Expression

Before diving into syntax, understand Ori's fundamental principle: **everything is an expression that produces a value**.

In many languages, you write "statements" that do things but don't return values:

```javascript
// JavaScript - statements don't return values
let x = 5;        // This is a statement
if (x > 0) {      // This is a statement
  console.log("positive");
}
```

In Ori, everything returns a value:

```ori
// Ori - everything is an expression
let x = 5                                    // Returns () (void)
let sign = if x > 0 then "positive" else "negative"  // Returns "positive"
```

This means you can use any expression anywhere a value is expected:

```ori
let result = {
    let a = 10
    let b = 20
    a + b,        // This expression's value (30) becomes the result
}

let message = `The answer is {if ready then compute() else "pending"}`
```

Keep this in mind — it explains why Ori code looks the way it does.

## Primitive Types

Ori provides these fundamental types:

### Numbers

**Integers** (`int`) are signed, with range -2⁶³ to 2⁶³ - 1:

```ori
let a = 42
let b = -17
let c = 1_000_000      // Underscores for readability
let d = 0xFF           // Hexadecimal (255)
let e = 0b1010         // Binary (10)
let f = 0o755          // Octal (493)
```

**Floats** (`float`) follow IEEE 754 double-precision semantics:

```ori
let pi = 3.14159
let tiny = 2.5e-8
let big = 1.5e10
```

**Integer division** truncates toward zero:

```ori
7 / 3      // 2, not 2.333...
-7 / 3     // -2, not -3
```

**Integer overflow** causes a panic (runtime error):

```ori
let max = 9223372036854775807  // Maximum int
let overflow = max + 1         // PANIC: integer overflow
```

For wrapping arithmetic, use `std.math`:

```ori
use std.math { wrapping_add }

let result = wrapping_add(left: max, right: 1)  // Wraps to minimum int
```

### Booleans

```ori
let active = true
let pending = false

let result = active && pending   // false (and)
let either = active || pending   // true (or)
let negated = !active            // false (not)
```

**Short-circuit evaluation:** `&&` and `||` only evaluate the right side if needed:

```ori
let safe = is_valid(x) && expensive_check(x)  // expensive_check only runs if is_valid is true
```

### Strings

**Regular strings** use double quotes:

```ori
let greeting = "Hello, World!"
let multiline = "Line 1\nLine 2\nLine 3"
let escaped = "She said \"Hello\""
let path = "C:\\Users\\Alice"
```

Escape sequences:
- `\\` — backslash
- `\"` — double quote
- `\n` — newline
- `\t` — tab
- `\r` — carriage return
- `\0` — null character

**Template strings** use backticks and support interpolation:

```ori
let name = "Alice"
let age = 30

let simple = `Hello, {name}!`              // "Hello, Alice!"
let computed = `In 10 years: {age + 10}`   // "In 10 years: 40"
let nested = `{if age >= 18 then "adult" else "minor"}`
```

**Format specifiers** control how values are displayed:

```ori
let n = 42
let pi = 3.14159

`Hex: {n:x}`           // "Hex: 2a"
`HEX: {n:X}`           // "HEX: 2A"
`Binary: {n:b}`        // "Binary: 101010"
`Padded: {n:05}`       // "Padded: 00042"
`Float: {pi:.2}`       // "Float: 3.14"
`Right: {n:>10}`       // "Right:         42"
`Left: {n:<10}`        // "Left: 42        "
`Center: {n:^10}`      // "Center:     42    "
```

**String indexing** returns a single character as a string:

```ori
let s = "Hello"
s[0]     // "H"
s[4]     // "o"
s[# - 1] // "o" (# is the length inside brackets)
```

### Characters

Single characters use single quotes:

```ori
let letter = 'A'
let emoji = '🎉'
let newline = '\n'
```

Characters are Unicode code points, not bytes.

### Void and Unit

`void` represents "no meaningful value":

```ori
@print_greeting (name: str) -> void = print(msg: `Hello, {name}!`)
```

The unit value `()` is the single value of type `void`:

```ori
let nothing = ()
let also_nothing: void = ()
```

### Never

The `Never` type represents computations that never complete normally:

```ori
@fail (msg: str) -> Never = panic(msg: msg)
```

Functions returning `Never` either panic or loop forever.

### Special Literals

**Duration** for time values:

```ori
let timeout = 30s       // 30 seconds
let interval = 100ms    // 100 milliseconds
let delay = 5m          // 5 minutes
let long = 2h           // 2 hours
```

**Size** for byte quantities:

```ori
let buffer = 4kb        // 4 kilobytes
let limit = 10mb        // 10 megabytes
let quota = 2gb         // 2 gigabytes
```

## Variables and Bindings

### Creating Variables

Use `let` to bind a name to a value:

```ori
let name = "Alice"
let age = 30
let pi = 3.14159
let active = true
```

The variable exists from its declaration to the end of its scope:

```ori
@main () -> void = {
    let x = 10,          // x exists from here...
    let y = x + 5,       // ...can use it here...
    print(msg: `{y}`),   // ...and here
}                        // x and y go out of scope
```

### Type Inference

Ori infers types from values:

```ori
let count = 42          // Ori infers: int
let price = 19.99       // Ori infers: float
let name = "Alice"      // Ori infers: str
let active = true       // Ori infers: bool
```

You can add type annotations for clarity or when inference needs help:

```ori
let count: int = 42
let price: float = 19.99
let items: [int] = []        // Empty list needs annotation
let lookup: {str: int} = {}  // Empty map needs annotation
```

### Mutable vs Immutable

By default, bindings can be reassigned:

```ori
let counter = 0
counter = 1      // OK - reassignment
counter = 2      // OK - reassignment again
```

Add `$` to make a binding immutable:

```ori
let $max_size = 100
max_size = 200   // ERROR: cannot reassign immutable binding
```

**Guidance:** Use `$` by default. Only remove it when you actually need to reassign:

```ori
// Good: clearly communicates intent
let $config = load_config()      // Won't change after loading
let $user_id = get_user_id()     // Identity, shouldn't change

// Only mutable when needed
let total = 0
for item in items do
    total = total + item.price   // Accumulating, needs reassignment
```

### Shadowing

You can declare a new variable with the same name, which shadows the previous one:

```ori
let x = 10
let x = x + 5        // New binding, shadows previous x
let x = `value: {x}` // New binding, different type is OK
```

This is different from reassignment — you're creating a new binding:

```ori
let $x = 10      // Immutable
let $x = x + 5   // OK: new immutable binding, shadows previous
```

Shadowing is useful for transforming values through a pipeline:

```ori
let data = fetch_raw()
let data = parse(raw: data)
let data = validate(parsed: data)
let data = transform(validated: data)
```

### Scope

Variables are scoped to their containing block:

```ori
@example () -> int = {
    let outer = 10

    let inner_result = {
        let inner = 20,        // Only visible in this run block
        outer + inner,         // outer is still visible
    }

    // inner is not visible here
    outer + inner_result
}
```

## Operators

### Arithmetic

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Add | `5 + 3` -> `8` |
| `-` | Subtract | `5 - 3` -> `2` |
| `*` | Multiply | `5 * 3` -> `15` |
| `/` | Divide | `5 / 3` -> `1` |
| `%` | Remainder | `5 % 3` -> `2` |
| `div` | Floor divide | `5 div 3` -> `1` |

For floats, `/` does true division: `5.0 / 3.0` -> `1.666...`

### Comparison

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `5 == 5` -> `true` |
| `!=` | Not equal | `5 != 3` -> `true` |
| `<` | Less than | `3 < 5` -> `true` |
| `>` | Greater than | `5 > 3` -> `true` |
| `<=` | Less or equal | `5 <= 5` -> `true` |
| `>=` | Greater or equal | `5 >= 3` -> `true` |

### Logical

| Operator | Description | Example |
|----------|-------------|---------|
| `&&` | And (short-circuit) | `true && false` -> `false` |
| `\|\|` | Or (short-circuit) | `true \|\| false` -> `true` |
| `!` | Not | `!true` -> `false` |

### Bitwise

| Operator | Description | Example |
|----------|-------------|---------|
| `&` | Bitwise and | `0b1100 & 0b1010` -> `0b1000` |
| `\|` | Bitwise or | `0b1100 \| 0b1010` -> `0b1110` |
| `^` | Bitwise xor | `0b1100 ^ 0b1010` -> `0b0110` |
| `~` | Bitwise not | `~0b1100` -> `...0011` |
| `<<` | Left shift | `1 << 4` -> `16` |
| `>>` | Right shift | `16 >> 2` -> `4` |

### String Concatenation

Use `+` to concatenate strings:

```ori
let full = "Hello" + ", " + "World!"  // "Hello, World!"
```

Or use template strings:

```ori
let first = "Hello"
let second = "World"
let full = `{first}, {second}!`       // "Hello, World!"
```

### Operator Precedence

From highest to lowest:

1. `.` `[]` `()` `?` — access, index, call, propagate
2. `!` `-` `~` — unary operators
3. `*` `/` `%` `div` — multiplicative
4. `+` `-` — additive
5. `<<` `>>` — shift
6. `..` `..=` `by` — range
7. `<` `>` `<=` `>=` — comparison
8. `==` `!=` — equality
9. `&` — bitwise and
10. `^` — bitwise xor
11. `|` — bitwise or
12. `&&` — logical and
13. `||` — logical or
14. `??` — coalesce

When in doubt, use parentheses:

```ori
let result = (a + b) * c
let flag = (x > 0) && (y < 10)
```

### The Coalesce Operator `??`

The `??` operator provides a default value for `None` or `Err`:

```ori
let name = maybe_name ?? "Anonymous"
let count = parse_int(s: input) ?? 0
let config = load_config() ?? default_config
```

## Control Flow

### Conditionals

The `if` expression evaluates a condition and returns one of two values:

```ori
let status = if age >= 18 then "adult" else "minor"
```

Both branches must have the same type:

```ori
// ERROR: branches have different types
let result = if condition then 42 else "hello"
```

Chain conditions with `else if`:

```ori
let grade = if score >= 90 then "A"
    else if score >= 80 then "B"
    else if score >= 70 then "C"
    else if score >= 60 then "D"
    else "F"
```

**Without `else`:** When you don't need a value, omit `else`:

```ori
if should_log then print(msg: "Logging...")
// Returns () if condition is false
```

### Loops with `for`

**Basic iteration:**

```ori
for item in items do
    print(msg: item)
```

**With `run` for multiple statements:**

```ori
for item in items do {
    let processed = transform(input: item)
    print(msg: processed)
}
```

**Collecting with `yield`:**

```ori
let doubled = for x in numbers yield x * 2
// [1, 2, 3] becomes [2, 4, 6]
```

**Filtering with `if`:**

```ori
let positive = for x in numbers if x > 0 yield x
// [-1, 2, -3, 4] becomes [2, 4]
```

**Combining filter and transform:**

```ori
let result = for x in numbers if x > 0 yield x * 2
// [-1, 2, -3, 4] becomes [4, 8]
```

### Ranges

Create sequences of numbers:

```ori
0..5        // 0, 1, 2, 3, 4 (exclusive end)
0..=5       // 0, 1, 2, 3, 4, 5 (inclusive end)
0..10 by 2  // 0, 2, 4, 6, 8 (with step)
10..0 by -1 // 10, 9, 8, 7, 6, 5, 4, 3, 2, 1 (descending)
```

Use in loops:

```ori
for i in 0..10 do
    print(msg: `Index: {i}`)

for i in 0..100 by 10 do
    print(msg: `Tens: {i}`)
```

### Loop Control

**`break`** exits a loop early:

```ori
let found = loop {
    let item = next_item()
    if item == target then break item
    if is_empty(collection: remaining) then break None
}
```

**`continue`** skips to the next iteration:

```ori
for item in items do {
    if should_skip(item: item) then continue
    process(item: item)
}
```

**Labeled loops** for nested control:

```ori
for:outer i in 0..10 do
    for j in 0..10 do {
        if condition(x: i, y: j) then break:outer
        process(x: i, y: j)
    }
```

### The `loop` Pattern

For loops that don't iterate over a collection:

```ori
let result = loop {
    let input = read_line()
    if input == "quit" then break "goodbye"
    print(msg: `You said: {input}`)
}
```

`loop` runs forever until `break` is called. The value passed to `break` becomes the loop's result.

## Type Conversions

Ori doesn't do implicit conversions. Use explicit syntax:

### Infallible Conversions with `as`

When conversion always succeeds:

```ori
let x = 42
let y = x as float      // 42.0

let n = 65
let c = n as char       // 'A'
```

### Fallible Conversions with `as?`

When conversion might fail:

```ori
let s = "42"
let n = s as? int       // Some(42)

let bad = "hello"
let m = bad as? int     // None
```

### Common Conversions

```ori
// Numbers
42 as float             // 42.0
3.7 as int              // 3 (truncates)
65 as char              // 'A'
'A' as int              // 65

// Strings
42 as str               // "42"
3.14 as str             // "3.14"
true as str             // "true"

// Parsing (fallible)
"42" as? int            // Some(42)
"3.14" as? float        // Some(3.14)
"true" as? bool         // Some(true)
"nope" as? int          // None
```

## Comments and Documentation

### Line Comments

Comments must be on their own line:

```ori
// This is valid
let x = 42

let y = 42  // This is a syntax error - no inline comments
```

### Doc Comments

Use special markers for documentation:

```ori
// #Description
// Calculates the area of a circle
//
// @param radius The radius of the circle (must be positive)
// !Panics if radius is negative
// >area(radius: 5.0) -> 78.54

@area (radius: float) -> float = {
    pre_check: radius >= 0.0 | "radius must be non-negative"
    3.14159 * radius * radius
}
```

## Putting It Together

Let's write a small program using everything we've learned:

```ori
// Configuration
let $TAX_RATE = 0.08
let $DISCOUNT_THRESHOLD = 100.0

// Types
type Item = { name: str, price: float, quantity: int }

// Calculate item total
@item_total (item: Item) -> float =
    float(item.quantity) * item.price

@test_item_total tests @item_total () -> void = {
    let item = Item { name: "Widget", price: 10.0, quantity: 3 }
    assert_eq(actual: item_total(item: item), expected: 30.0)
}

// Calculate subtotal
@subtotal (items: [Item]) -> float =
    items.map(item -> item_total(item: item))
        .fold(initial: 0.0, op: (acc, x) -> acc + x)

@test_subtotal tests @subtotal () -> void = {
    let items = [
        Item { name: "A", price: 10.0, quantity: 2 }
        Item { name: "B", price: 5.0, quantity: 3 }
    ]
    assert_eq(actual: subtotal(items: items), expected: 35.0)
}

// Calculate discount
@discount (amount: float) -> float =
    if amount >= $DISCOUNT_THRESHOLD then amount * 0.10 else 0.0

@test_discount tests @discount () -> void = {
    assert_eq(actual: discount(amount: 150.0), expected: 15.0)
    assert_eq(actual: discount(amount: 50.0), expected: 0.0)
}

// Calculate final total
@calculate_total (items: [Item]) -> float = {
    let sub = subtotal(items: items)
    let disc = discount(amount: sub)
    let tax = (sub - disc) * $TAX_RATE
    sub - disc + tax
}

@test_calculate_total tests @calculate_total () -> void = {
    let items = [
        Item { name: "Widget", price: 50.0, quantity: 3 }
    ]
    // subtotal: 150, discount: 15, taxable: 135, tax: 10.8
    assert_eq(actual: calculate_total(items: items), expected: 145.8)
}

// Main program
@main () -> void = {
    let $cart = [
        Item { name: "Keyboard", price: 75.0, quantity: 1 }
        Item { name: "Mouse", price: 25.0, quantity: 2 }
        Item { name: "Cable", price: 10.0, quantity: 3 }
    ]
    print(msg: `Subtotal: ${subtotal(items: cart):.2}`)
    print(msg: `Total:    ${calculate_total(items: cart):.2}`)
}
```

## Quick Reference

### Variables

```ori
let x = 42              // Mutable
let $x = 42             // Immutable
let x: int = 42         // With type
```

### Types

```ori
int, float, bool, str, char, byte, void, Never
Duration, Size
[T], {K: V}, Set<T>     // Collections
(T, U), (T, U, V)       // Tuples
Option<T>, Result<T, E> // Sum types
```

### Operators

```ori
+ - * / % div           // Arithmetic
== != < > <= >=         // Comparison
&& || !                 // Logical
& | ^ ~ << >>           // Bitwise
.. ..= by               // Range
??                      // Coalesce
as as?                  // Conversion
```

### Control Flow

```ori
if cond then a else b
for x in items do expr
for x in items yield expr
for x in items if cond yield expr
loop {expr}
break value
continue
```

## What's Next

Now that you understand the language basics:

- **[Functions](/guide/03-functions)** — Deep dive into function definitions, generics, and lambdas
- **[Collections](/guide/04-collections)** — Lists, maps, sets, and tuples

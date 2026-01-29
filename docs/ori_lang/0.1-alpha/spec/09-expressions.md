---
title: "Expressions"
description: "Ori Language Specification — Expressions"
order: 9
---

# Expressions

Expressions compute values.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § EXPRESSIONS

## Postfix Expressions

### Field and Method Access

```ori
point.x
list.len()
```

### Index Access

```ori
list[0]
list[# - 1]    // # is length within brackets
map["key"]     // returns Option<V>
```

Lists/strings panic on out-of-bounds; maps return `Option`.

### Function Call

```ori
add(a: 1, b: 2)
fetch_user(id: 1)
print(msg: "hello")
assert_eq(actual: result, expected: 10)
```

Named arguments are required for direct function and method calls. Argument names must match parameter names. Argument order is irrelevant.

Positional arguments are permitted in three cases:

1. Type conversion functions (`int`, `float`, `str`, `byte`):

```ori
int(3.14)      // OK: type conversion
float(42)      // OK: type conversion
str(value)     // OK: type conversion
```

2. Calls through function variables (parameter names are unknowable):

```ori
let f = (x: int) -> x + 1
f(5)           // OK: calling through variable

let apply = (fn: (int) -> int, val: int) -> fn(val)
apply(fn: inc, val: 10)  // outer call: named required
                         // inner fn(val): positional OK
```

3. Single-parameter functions called with inline lambda expressions:

```ori
items.map(x -> x * 2)           // OK: lambda literal
items.filter(x -> x > 0)        // OK: lambda literal
items.map(transform: x -> x * 2) // OK: named always works

let double = x -> x * 2
items.map(double)               // error: named arg required
items.map(transform: double)    // OK: function reference needs name
```

A lambda expression is `x -> expr`, `(a, b) -> expr`, `() -> expr`, or `(x: Type) -> Type = expr`. Function references and variables holding functions are not lambda expressions and require named arguments.

For methods, `self` is not counted when determining "single parameter." A method like `map(transform: fn)` has one explicit parameter, so lambda arguments may be positional.

It is a compile-time error to use positional arguments in direct function or method calls outside these three cases.

### Error Propagation

```ori
value?         // returns Err early if Err
```

## Unary Expressions

`!` logical not, `-` negation, `~` bitwise not.

## Binary Expressions

| Operator | Operation |
|----------|-----------|
| `+` `-` `*` `/` | Arithmetic |
| `%` | Modulo |
| `div` | Floor division |
| `==` `!=` `<` `>` `<=` `>=` | Comparison |
| `&&` `\|\|` | Logical (short-circuit) |
| `&` `\|` `^` `~` | Bitwise |
| `<<` `>>` | Shift |
| `..` `..=` | Range |
| `by` | Range step |
| `??` | Coalesce (None/Err → default) |

### Operator Type Constraints

Binary operators require operands of matching types. No implicit conversions.

**Arithmetic** (`+` `-` `*` `/`):

| Left | Right | Result |
|------|-------|--------|
| `int` | `int` | `int` |
| `float` | `float` | `float` |

**String concatenation** (`+`):

| Left | Right | Result |
|------|-------|--------|
| `str` | `str` | `str` |

**Integer-only** (`%` `div` `<<` `>>` `&` `|` `^`):

| Left | Right | Result |
|------|-------|--------|
| `int` | `int` | `int` |

**Comparison** (`<` `>` `<=` `>=`):

Operands must be the same type implementing `Comparable`. Returns `bool`.

**Equality** (`==` `!=`):

Operands must be the same type implementing `Eq`. Returns `bool`.

Mixed-type operations are compile errors:

```ori
1 + 2.0          // error: mismatched types int and float
float(1) + 2.0   // OK: 3.0
1 + int(2.0)     // OK: 3
```

### Numeric Behavior

**Integer overflow**: Panics. Addition, subtraction, multiplication, and negation all panic on overflow.

```ori
let max: int = 9223372036854775807
max + 1      // panic: integer overflow
int.min - 1  // panic: integer overflow
-int.min     // panic: integer overflow (negation)
```

Programs requiring wrapping or saturating arithmetic should use functions from `std.math`.

**Integer division and modulo overflow**: The expression `int.min / -1` and `int.min % -1` panic because the mathematical result cannot be represented.

```ori
int.min div -1  // panic: integer overflow
int.min % -1    // panic: integer overflow
```

**Integer division by zero**: Panics.

```ori
5 / 0    // panic: division by zero
5 % 0    // panic: modulo by zero
```

**Float division by zero**: Returns infinity or NaN per IEEE 754.

```ori
1.0 / 0.0    // Inf
-1.0 / 0.0   // -Inf
0.0 / 0.0    // NaN
```

**Float NaN propagation**: Any operation involving NaN produces NaN.

```ori
NaN + 1.0    // NaN
NaN == NaN   // false (IEEE 754)
NaN != NaN   // true
```

**Float comparison**: Exact bit comparison. No epsilon tolerance.

```ori
0.1 + 0.2 == 0.3  // false (floating-point representation)

## Range Expressions

Range expressions produce `Range<T>` values.

```ori
0..10       // 0, 1, 2, ..., 9 (exclusive)
0..=10      // 0, 1, 2, ..., 10 (inclusive)
```

### Range with Step

The `by` keyword specifies a step value for non-unit increments:

```ori
0..10 by 2      // 0, 2, 4, 6, 8
0..=10 by 2     // 0, 2, 4, 6, 8, 10
10..0 by -1     // 10, 9, 8, 7, 6, 5, 4, 3, 2, 1
10..=0 by -2    // 10, 8, 6, 4, 2, 0
```

`by` is a context-sensitive keyword recognized only following a range expression.

**Type constraints:**

- Range with step is supported only for `int` ranges
- Start, end, and step must all be `int`
- It is a compile-time error to use `by` with non-integer ranges

**Runtime behavior:**

- Step of zero causes a panic
- Mismatched direction produces an empty range (no panic)

```ori
0..10 by 0      // panic: step cannot be zero
0..10 by -1     // empty range (can't go from 0 to 10 with negative step)
10..0 by 1      // empty range (can't go from 10 to 0 with positive step)
```

## With Expression

```ori
with Http = MockHttp { ... } in fetch("/data")
```

## Let Binding

```ori
let x = 5           // mutable
let $x = 5          // immutable
let { x, y } = point
let { $x, y } = point  // x immutable, y mutable
```

## Conditional

```ori
if x > 0 then "positive" else "non-positive"
```

Condition must be `bool`. When `else` is present, branches must have compatible types.

When `else` is omitted, the expression has type `void`. The `then` branch must also have type `void` (or type `Never`, which is compatible with any type).

```ori
// Valid: then-branch is void
if debug then print(msg: "debug mode")

// Valid: then-branch is Never (panic returns Never)
if !valid then panic(msg: "invalid state")

// Invalid: then-branch has non-void type without else
if x > 0 then "positive"  // error: non-void then-branch requires else
```

## For Expression

```ori
for item in items do print(item)
for n in numbers if n > 0 yield n * n
```

`do` returns `void`; `yield` collects results.

## Loop Expression

```ori
loop(
    match(ch.receive(),
        Some(v) -> process(v),
        None -> break,
    ),
)
```

`break` exits; `continue` skips to next iteration.

## Lambda

```ori
x -> x * 2
(x, y) -> x + y
(x: int) -> int = x * 2
```

## Evaluation

Expressions are evaluated left-to-right. This order is guaranteed and observable.

### Operand Evaluation

Binary operators evaluate the left operand before the right:

```ori
left() + right()  // left() called first, then right()
```

### Argument Evaluation

Function arguments are evaluated left-to-right as written, before the call:

```ori
foo(a: first(), b: second(), c: third())
// Order: first(), second(), third(), then foo()
```

Named arguments evaluate in written order, not parameter order:

```ori
foo(c: third(), a: first(), b: second())
// Order: third(), first(), second(), then foo()
```

### Compound Expressions

Postfix operations evaluate left-to-right:

```ori
list[index()].method(arg())
// Order: list, index(), method lookup, arg(), method call
```

### List and Map Literals

Elements evaluate left-to-right:

```ori
[first(), second(), third()]
{"a": first(), "b": second()}
```

## Spread Operator

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § EXPRESSIONS (list_element, map_element, struct_element)

The spread operator `...` expands collections and structs in literal contexts.

### List Spread

Expands list elements into a list literal:

```ori
let a = [1, 2, 3]
let b = [4, 5, 6]

[...a, ...b]           // [1, 2, 3, 4, 5, 6]
[0, ...a, 10]          // [0, 1, 2, 3, 10]
[first, ...middle, last]
```

The spread expression must be of type `[T]` where `T` matches the list element type.

### Map Spread

Expands map entries into a map literal:

```ori
let defaults = {"timeout": 30, "retries": 3}
let custom = {"retries": 5, "verbose": true}

{...defaults, ...custom}
// {"timeout": 30, "retries": 5, "verbose": true}
```

Later entries override earlier ones on key conflicts. The spread expression must be of type `{K: V}` matching the map type.

### Struct Spread

Copies fields from an existing struct:

```ori
type Point = { x: int, y: int, z: int }
let original = Point { x: 1, y: 2, z: 3 }

Point { ...original, x: 10 }  // Point { x: 10, y: 2, z: 3 }
Point { x: 10, ...original }  // Point { x: 1, y: 2, z: 3 }
```

Order determines precedence: later fields override earlier ones. The spread expression must be of the same struct type.

### Constraints

- Spread is only valid in literal contexts (lists, maps, struct constructors)
- It is a compile-time error to use spread in function call arguments
- All spread expressions must have compatible types with the target container
- Struct spread requires the exact same type (not subtypes or supertypes)

### Evaluation Order

Spread expressions evaluate left-to-right:

```ori
[first(), ...middle(), last()]
// Order: first(), middle(), last()

{...defaults(), "key": computed(), ...overrides()}
// Order: defaults(), computed(), overrides()
```

### Assignment

The right side evaluates before assignment:

```ori
x = compute()  // compute() evaluated, then assigned to x
```

### Short-Circuit Evaluation

Logical and coalesce operators may skip the right operand:

| Operator | Skips right when |
|----------|------------------|
| `&&` | Left is `false` |
| `\|\|` | Left is `true` |
| `??` | Left is `Some`/`Ok` |

```ori
false && expensive()  // expensive() not called
true \|\| expensive()  // expensive() not called
Some(x) ?? expensive()  // expensive() not called
```

### Conditional Branches

Only the taken branch is evaluated:

```ori
if condition then
    only_if_true()
else
    only_if_false()
```

See [Control Flow](19-control-flow.md) for details on conditionals and loops.

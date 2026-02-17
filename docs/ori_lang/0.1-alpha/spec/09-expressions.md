---
title: "Expressions"
description: "Ori Language Specification — Expressions"
order: 9
section: "Expressions"
---

# Expressions

Expressions compute values.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § EXPRESSIONS

## Postfix Expressions

### Field and Method Access

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § `postfix_op`, `member_name`

```ori
point.x
list.len()
```

The member name after `.` may be an identifier or a reserved keyword. Keywords are valid in member position because the `.` prefix provides unambiguous context:

```ori
ordering.then(other: Less)    // method call — `then` keyword allowed after `.`
point.type                    // field access — `type` keyword allowed after `.`
```

Integer literals are valid in member position for tuple field access. The index
is zero-based and must be within the tuple's arity:

```ori
let pair = (10, "hello")
pair.0          // 10
pair.1          // "hello"
```

An out-of-bounds index is a compile-time error. Tuple field access is equivalent
to destructuring but provides direct positional access without binding all elements.

Chained tuple field access on nested tuples must use parentheses because the
lexer tokenizes `0.1` as a float literal:

```ori
let nested = ((1, 2), (3, 4))
(nested.0).1    // 2 — parentheses required
nested.0.1      // error: lexer sees 0.1 as float
```

### Index Access

```ori
list[0]
list[# - 1]    // # is length within brackets
map["key"]     // returns Option<V>
```

Lists/strings panic on out-of-bounds; maps return `Option`.

#### Index Trait

User-defined types can implement the `Index` trait for custom subscripting:

```ori
trait Index<Key, Value> {
    @index (self, key: Key) -> Value
}
```

The compiler desugars subscript expressions to trait method calls:

```ori
x[key]
// Desugars to:
x.index(key: key)
```

A type may implement `Index` for multiple key types:

```ori
impl Index<str, Option<JsonValue>> for JsonValue { ... }
impl Index<int, Option<JsonValue>> for JsonValue { ... }
```

If the key type is ambiguous, the call is a compile-time error.

Return types encode error handling strategy:
- `T` — panics on invalid key (fixed-size containers)
- `Option<T>` — returns `None` for missing keys (sparse data)
- `Result<T, E>` — returns detailed errors (external data)

Built-in implementations:
- `[T]` implements `Index<int, T>` (panics on out-of-bounds)
- `[T, max N]` implements `Index<int, T>` (same as `[T]`)
- `{K: V}` implements `Index<K, Option<V>>`
- `str` implements `Index<int, str>` (single codepoint, panics on out-of-bounds)

The `#` length shorthand is supported only for built-in types. Custom types use `len()` explicitly.

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

### Conversion Expressions

The `as` and `as?` operators convert values between types.

```ori
42 as float           // 42.0 (infallible)
"42" as? int          // Some(42) (fallible)
```

**Syntax:**

| Form | Semantics | Return Type |
|------|-----------|-------------|
| `expr as Type` | Infallible conversion | `Type` |
| `expr as? Type` | Fallible conversion | `Option<Type>` |

**Backing Traits:**

- `expr as Type` desugars to `As<Type>.as(self: expr)`
- `expr as? Type` desugars to `TryAs<Type>.try_as(self: expr)`

See [Types § Conversion Traits](06-types.md#conversion-traits) for trait definitions.

**Compile-Time Enforcement:**

The compiler enforces that `as` is only used for conversions that cannot fail:

```ori
42 as float         // OK: int -> float always succeeds
"42" as int         // ERROR: str -> int can fail, use `as?`
3.14 as int         // ERROR: lossy conversion, use explicit method
```

Lossy conversions (like `float -> int`) require explicit methods:

```ori
3.99.truncate()     // 3 (toward zero)
3.99.round()        // 4 (nearest)
3.99.floor()        // 3 (toward negative infinity)
3.99.ceil()         // 4 (toward positive infinity)
```

**Chaining:**

`as` and `as?` are postfix operators that chain naturally:

```ori
input.trim() as? int      // (input.trim()) as? int
items[0] as str           // (items[0]) as str
get_value()? as float     // (get_value()?) as float
```

## Unary Expressions

### Logical Not (`!`)

Inverts a boolean value.

```ori
!true   // false
!false  // true
!!x     // x (double negation)
```

Type constraint: `! : bool -> bool`. It is a compile-time error to apply `!` to non-boolean types. For bitwise complement of integers, use `~`.

### Arithmetic Negation (`-`)

Negates a numeric value.

```ori
-42      // -42
-3.14    // -3.14
-(-5)    // 5
```

Type constraints:
- `- : int -> int`
- `- : float -> float`
- `- : Duration -> Duration`

Integer negation panics on overflow: `-int.min` panics because the positive result does not fit in `int`.

Float negation never overflows (flips sign bit). Duration negation follows the same overflow rules as integer negation.

It is a compile-time error to apply unary `-` to `Size` (byte counts are non-negative).

### Bitwise Not (`~`)

Inverts all bits of an integer.

```ori
~0       // -1 (all bits set)
~(-1)    // 0
~5       // -6
```

Type constraints:
- `~ : int -> int`
- `~ : byte -> byte`

For `int`, `~x` is equivalent to `-(x + 1)`. For `byte`, the result is the bitwise complement within 8 bits.

It is a compile-time error to apply `~` to `bool`. Use `!` for boolean negation.

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

**Integer-only** (`%` `div`):

| Left | Right | Result |
|------|-------|--------|
| `int` | `int` | `int` |

**Bitwise** (`&` `|` `^`):

| Left | Right | Result |
|------|-------|--------|
| `int` | `int` | `int` |
| `byte` | `byte` | `byte` |

**Shift** (`<<` `>>`):

| Left | Right | Result |
|------|-------|--------|
| `int` | `int` | `int` |
| `byte` | `int` | `byte` |

The shift count is always `int`. It is a compile-time error to mix `int` and `byte` for bitwise AND/OR/XOR.

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

**Shift overflow**: Shift operations panic when the shift count is negative, exceeds the bit width, or the result overflows.

```ori
1 << 63     // panic: shift overflow (result doesn't fit in signed int)
1 << 64     // panic: shift count exceeds bit width
1 << -1     // panic: negative shift count
16 >> 64    // panic: shift count exceeds bit width
```

For `int` (64-bit signed), valid shift counts are 0 to 62 for left shift when the result must remain representable. For right shift, counts 0 to 63 are valid.

For `byte` (8-bit unsigned), valid shift counts are 0 to 7.

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
```

## Operator Precedence

Operators are listed from highest to lowest precedence:

| Level | Operators | Associativity | Description |
|-------|-----------|---------------|-------------|
| 1 | `.` `[]` `()` `?` `as` `as?` | Left | Postfix |
| 2 | `!` `-` `~` | Right | Unary |
| 3 | `*` `/` `%` `div` | Left | Multiplicative |
| 4 | `+` `-` | Left | Additive |
| 5 | `<<` `>>` | Left | Shift |
| 6 | `..` `..=` `by` | Left | Range |
| 7 | `<` `>` `<=` `>=` | Left | Relational |
| 8 | `==` `!=` | Left | Equality |
| 9 | `&` | Left | Bitwise AND |
| 10 | `^` | Left | Bitwise XOR |
| 11 | `\|` | Left | Bitwise OR |
| 12 | `&&` | Left | Logical AND |
| 13 | `\|\|` | Left | Logical OR |
| 14 | `??` | Right | Coalesce |

Parentheses override precedence:

```ori
(a & b) == 0    // Compare result of AND with 0
a & b == 0      // Parsed as a & (b == 0) — likely not intended
```

## Operator Traits

Operators are desugared to trait method calls. User-defined types can implement operator traits to support operator syntax.

### Arithmetic Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a + b` | `Add` | `a.add(rhs: b)` |
| `a - b` | `Sub` | `a.subtract(rhs: b)` |
| `a * b` | `Mul` | `a.multiply(rhs: b)` |
| `a / b` | `Div` | `a.divide(rhs: b)` |
| `a div b` | `FloorDiv` | `a.floor_divide(rhs: b)` |
| `a % b` | `Rem` | `a.remainder(rhs: b)` |

### Unary Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `-a` | `Neg` | `a.negate()` |
| `!a` | `Not` | `a.not()` |
| `~a` | `BitNot` | `a.bit_not()` |

### Bitwise Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a & b` | `BitAnd` | `a.bit_and(rhs: b)` |
| `a \| b` | `BitOr` | `a.bit_or(rhs: b)` |
| `a ^ b` | `BitXor` | `a.bit_xor(rhs: b)` |
| `a << b` | `Shl` | `a.shift_left(rhs: b)` |
| `a >> b` | `Shr` | `a.shift_right(rhs: b)` |

### Comparison Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a == b` | `Eq` | `a.equals(other: b)` |
| `a != b` | `Eq` | `!a.equals(other: b)` |
| `a < b` | `Comparable` | `a.compare(other: b).is_less()` |
| `a <= b` | `Comparable` | `a.compare(other: b).is_less_or_equal()` |
| `a > b` | `Comparable` | `a.compare(other: b).is_greater()` |
| `a >= b` | `Comparable` | `a.compare(other: b).is_greater_or_equal()` |

### Trait Definitions

Operator traits use default type parameters and default associated types:

```ori
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}
```

The `Rhs` parameter defaults to `Self`, and `Output` defaults to `Self`. Implementations may override either.

> **Note:** The `Div` trait method is named `divide` rather than `div` because `div` is a reserved keyword for the floor division operator.

### User-Defined Example

```ori
type Vector2 = { x: float, y: float }

impl Add for Vector2 {
    @add (self, rhs: Vector2) -> Self = Vector2 {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

let a = Vector2 { x: 1.0, y: 2.0 }
let b = Vector2 { x: 3.0, y: 4.0 }
let sum = a + b  // Vector2 { x: 4.0, y: 6.0 }
```

### Mixed-Type Operations

Traits support different operand types. Commutative operations require both orderings:

```ori
// Duration * int
impl Mul<int> for Duration {
    type Output = Duration
    @multiply (self, n: int) -> Duration = ...
}

// int * Duration
impl Mul<Duration> for int {
    type Output = Duration
    @multiply (self, d: Duration) -> Duration = d * self
}
```

The compiler does not automatically commute operands.

### Built-in Implementations

Primitive types have built-in implementations for their applicable operators. These implementations use compiler intrinsics.

See [Declarations § Traits](08-declarations.md#traits) for trait definition syntax.

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

### Infinite Ranges

Omitting the end creates an unbounded ascending range:

```ori
0..           // 0, 1, 2, 3, ... (infinite ascending)
100..         // 100, 101, 102, ... (infinite ascending from 100)
0.. by 2      // 0, 2, 4, 6, ... (infinite ascending by 2)
0.. by -1     // 0, -1, -2, ... (infinite descending)
```

**Type constraints:**

- Infinite ranges are supported only for `int`
- The step must be non-zero (zero step panics)

**Semantics:**

- `start..` creates an unbounded range with step +1
- `start.. by step` creates an unbounded range with explicit step
- Infinite ranges implement `Iterable` but NOT `DoubleEndedIterator` (no end to iterate from)

Infinite ranges must be bounded before terminal operations like `collect()`:

```ori
(0..).iter().take(count: 10).collect()    // OK: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
(0..).iter().collect()                     // infinite loop, eventually OOM
```

Implementations SHOULD warn on obvious unbounded consumption patterns.

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

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § if_expr

```ori
if x > 0 then "positive" else "non-positive"
```

The _condition_ must have type `bool`. It is a compile-time error if the condition has any other type.

### Branch Evaluation

Only one branch is evaluated at runtime. The unevaluated branch does not execute. This is guaranteed and observable (side effects in the unevaluated branch do not occur).

### Type Unification

When `else` is present, both branches must produce types that unify to a common type:

```ori
if cond then 1 else 2              // type: int
if cond then Some(1) else None     // type: Option<int>
if cond then 1 else "two"          // error: cannot unify int and str
```

### Without Else

When `else` is omitted, the expression has type `void`. The `then` branch must have type `void` or `Never`:

```ori
// Valid: then-branch is void
if debug then print(msg: "debug mode")

// Valid: then-branch is Never (coerces to void)
if !valid then panic(msg: "invalid state")

// Invalid: then-branch has non-void type without else
if x > 0 then "positive"  // error: non-void then-branch requires else
```

When the `then` branch has type `Never`, it coerces to `void`.

### Never Type Coercion

The `Never` type coerces to any type in conditional branches:

```ori
let x: int = if condition then 42 else panic(msg: "unreachable")
// else branch is Never, coerces to int
```

If both branches have type `Never`, the expression has type `Never`:

```ori
let x = if a then panic(msg: "a") else panic(msg: "b")
// type: Never
```

### Else-If Chains

```ori
if condition1 then expression1
else if condition2 then expression2
else expression3
```

The grammar treats `else if` as a single production for parsing convenience, but semantically the `else` branch contains another `if` expression.

### Struct Literal Restriction

Struct literals are not permitted directly in the condition position. This prevents parsing ambiguity with block expressions:

```ori
if Point { x: 0, y: 0 } then ...  // error: struct literal in condition
if (Point { x: 0, y: 0 }) then ...  // OK: parentheses re-enable struct literals
```

The parser disables struct literal parsing in the condition context. Parenthesized expressions re-enable it.

## For Expression

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § For Expression

### For-Do

The `for...do` expression iterates for side effects and returns `void`:

```ori
for item in items do print(msg: item)
for (key, value) in map do process(key: key, value: value)
```

The source must implement `Iterable`. The binding supports destructuring patterns.

### Guard Condition

An optional `if` clause filters elements:

```ori
for x in items if x > 0 do process(x: x)
```

The guard is evaluated per item before the body.

### Break and Continue

In `for...do`, `break` exits the loop and `continue` skips to the next iteration:

```ori
for x in items do
    if done(x) then break,
    if skip(x) then continue,
    process(x: x),
```

`break value` and `continue value` are errors in `for...do` context — there is no collection to contribute to.

### For-Yield

The `for...yield` expression builds collections:

```ori
for n in numbers if n > 0 yield n * n
```

See [Patterns § For-Yield Comprehensions](10-patterns.md#for-yield-comprehensions) for complete semantics including type inference, nested comprehensions, and break/continue with values.

### Labeled For

Labels enable break/continue to target outer loops:

```ori
for:outer x in xs do
    for y in ys do
        if done(x, y) then break:outer,
```

See [Control Flow § Labeled Loops](19-control-flow.md#labeled-loops) for label semantics.

## Loop Expression

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § loop_expr

The `loop(...)` expression repeatedly evaluates its body until a `break` is encountered.

### Syntax

```ori
loop(body)
loop:name(body)  // labeled
```

### Body

The body is a single expression. For multiple expressions, use `run(...)`:

```ori
// Single expression
loop(process_next())

// Multiple expressions
loop(run(
    let x = compute(),
    if done(x) then break x,
    update(x),
))
```

### Loop Type

The type of a `loop` expression is determined by its break values:

- **Break with value**: Loop type is the break value type
- **Break without value**: Loop type is `void`
- **No break**: Loop type is `Never` (infinite loop)

```ori
let result: int = loop(run(
    let x = compute(),
    if x > 100 then break x,
))  // type: int

loop(run(
    let msg = receive(),
    if is_shutdown(msg) then break,
    process(msg),
))  // type: void

@server () -> Never = loop(handle_request())  // type: Never
```

### Multiple Break Paths

All break paths must produce compatible types:

```ori
loop(run(
    if a then break 1,      // int
    if b then break "two",  // error E0860: expected int, found str
))
```

### Continue

`continue` skips the rest of the current iteration:

```ori
loop(run(
    let item = next(),
    if is_none(item) then break,
    if skip(item.unwrap()) then continue,
    process(item.unwrap()),
))
```

`continue value` in a loop is an error (E0861). Loops do not accumulate values.

### Labeled Loops

Labels allow `break` and `continue` to target a specific loop. See [Control Flow § Labeled Loops](19-control-flow.md#labeled-loops) for label semantics.

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

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § EXPRESSIONS (list_element, map_element, struct_element)

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

---
title: "Expressions"
description: "Clause 14: Ori Language Specification — Expressions"
order: 14
section: "Language"
---

# 14 Expressions

Expressions compute values.

> **Grammar:** See [grammar.ebnf](grammar.md) § EXPRESSIONS

## 14.1 Postfix Expressions

### 14.1.1 Field and Method Access

> **Grammar:** See [grammar.ebnf](grammar.md) § `postfix_op`, `member_name`

```ori
point.x;
list.len();
```

The member name after `.` may be an identifier or a reserved keyword. Keywords are valid in member position because the `.` prefix provides unambiguous context:

```ori
ordering.then(other: Less);    // method call — `then` keyword allowed after `.`
point.type;                    // field access — `type` keyword allowed after `.`
```

Integer literals are valid in member position for tuple field access. The index
is zero-based and shall be within the tuple's arity:

```ori
let pair = (10, "hello");
pair.0;          // 10
pair.1;          // "hello"
```

An out-of-bounds index is a compile-time error. Tuple field access is equivalent
to destructuring but provides direct positional access without binding all elements.

Chained tuple field access on nested tuples shall use parentheses because the
lexer tokenizes `0.1` as a float literal:

```ori
let nested = ((1, 2), (3, 4));
(nested.0).1;    // 2 — parentheses required
nested.0.1;      // error: lexer sees 0.1 as float
```

### 14.1.2 Index Access

```ori
list[0];
list[# - 1];    // # is length within brackets
map["key"];     // returns Option<V>
```

Lists/strings panic on out-of-bounds; maps return `Option`.

#### Index Trait

User-defined types can implement the `Index` trait for custom subscripting:

```ori
trait Index<Key, Value> {
    @index (self, key: Key) -> Value;
}
```

The compiler desugars subscript expressions to trait method calls:

```ori
x[key];
// Desugars to:
x.index(key: key);

```

A type may implement `Index` for multiple key types:

```ori
impl JsonValue: Index<str, Option<JsonValue>> { ... }
impl JsonValue: Index<int, Option<JsonValue>> { ... }
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

### 14.1.3 Function Call

```ori
add(a: 1, b: 2);
fetch_user(id: 1);
print(msg: "hello");
assert_eq(actual: result, expected: 10);

```

Named arguments are required for direct function and method calls. Argument names shall match parameter names. Argument order is irrelevant.

Positional arguments are permitted in three cases:

1. Type conversion functions (`int`, `float`, `str`, `byte`):

```ori
int(3.14);      // OK: type conversion
float(42);      // OK: type conversion
str(value);     // OK: type conversion
```

2. Calls through function variables (parameter names are unknowable):

```ori
let f = (x: int) -> x + 1;
f(5);           // OK: calling through variable

let apply = (fn: (int) -> int, val: int) -> fn(val);
apply(fn: inc, val: 10);  // outer call: named required
                           // inner fn(val): positional OK
```

3. Single-parameter functions called with inline lambda expressions:

```ori
items.map(x -> x * 2);           // OK: lambda literal
items.filter(x -> x > 0);        // OK: lambda literal
items.map(transform: x -> x * 2); // OK: named always works

let double = x -> x * 2;
items.map(double);               // error: named arg required
items.map(transform: double);    // OK: function reference needs name
```

A lambda expression is `x -> expr`, `(a, b) -> expr`, `() -> expr`, or `(x: Type) -> Type = expr`. Function references and variables holding functions are not lambda expressions and require named arguments.

For methods, `self` is not counted when determining "single parameter." A method like `map(transform: fn)` has one explicit parameter, so lambda arguments may be positional.

It is a compile-time error to use positional arguments in direct function or method calls outside these three cases.

### 14.1.4 Error Propagation

```ori
value?;         // returns Err early if Err
```

### 14.1.5 Conversion Expressions

The `as` and `as?` operators convert values between types.

```ori
42 as float;           // 42.0 (infallible)
"42" as? int;          // Some(42) (fallible)
```

**Syntax:**

| Form | Semantics | Return Type |
|------|-----------|-------------|
| `expr as Type` | Infallible conversion | `Type` |
| `expr as? Type` | Fallible conversion | `Option<Type>` |

**Backing Traits:**

- `expr as Type` desugars to `As<Type>.as(self: expr)`
- `expr as? Type` desugars to `TryAs<Type>.try_as(self: expr)`

See [Types § Conversion Traits](08-types.md#811-conversion-traits) for trait definitions.

**Compile-Time Enforcement:**

The compiler enforces that `as` is only used for conversions that cannot fail:

```ori
42 as float;         // OK: int -> float always succeeds
"42" as int;         // ERROR: str -> int can fail, use `as?`
3.14 as int;         // ERROR: lossy conversion, use explicit method
```

Lossy conversions (like `float -> int`) require explicit methods:

```ori
3.99.truncate();     // 3 (toward zero)
3.99.round();        // 4 (nearest)
3.99.floor();        // 3 (toward negative infinity)
3.99.ceil();         // 4 (toward positive infinity)
```

**Built-in `as` conversions:**

The following table lists all built-in infallible conversions. Conversions not in this table are compile-time errors for `as`. User types implement the `As<T>` trait.

| Source | Target | Behavior |
|--------|--------|----------|
| `int` | `float` | Exact representation (within i64 range) |
| `int` | `byte` | Panic if value outside 0–255 |
| `int` | `char` | Panic if not a valid Unicode scalar value |
| `byte` | `int` | Zero-extend (always succeeds) |
| `byte` | `char` | Latin-1 interpretation U+0000–U+00FF (always succeeds) |
| `char` | `int` | Unicode code point value (always succeeds) |
| `char` | `str` | Single-character string (always succeeds) |

NOTE  `float` to `int` is not an `as` conversion because it is lossy. Use `.truncate()`, `.round()`, `.floor()`, or `.ceil()` for explicit rounding.

**Built-in `as?` conversions:**

The following table lists all built-in fallible conversions. Conversions not in this table are compile-time errors for `as?`. User types implement the `TryAs<T>` trait.

| Source | Target | Returns |
|--------|--------|---------|
| `str` | `int` | `Some(n)` if valid integer, `None` otherwise |
| `str` | `float` | `Some(f)` if valid float, `None` otherwise |
| `str` | `bool` | `Some(b)` for `"true"`/`"false"` (case-sensitive), `None` otherwise |
| `int` | `byte` | `Some(b)` if 0–255, `None` otherwise |
| `int` | `char` | `Some(c)` if valid Unicode scalar value, `None` otherwise |
| `float` | `int` | `Some(n)` if whole number, no precision loss, and within i64 range; `None` otherwise |

Parsing rules for `str` → `int`: leading and trailing whitespace is stripped; leading `+` or `-` is accepted; digit separators (`_`), hex (`0x`), and binary (`0b`) prefixes are rejected; overflow produces `None`.

Parsing rules for `str` → `float`: leading and trailing whitespace is stripped; the strings `"inf"`, `"-inf"`, and `"nan"` (case-insensitive) are accepted; scientific notation (`1.5e10`) is accepted.

**Chaining:**

`as` and `as?` are postfix operators that chain naturally:

```ori
input.trim() as? int;      // (input.trim()) as? int
items[0] as str;           // (items[0]) as str
get_value()? as float;     // (get_value()?) as float
```

### 14.1.6 Type narrowing

Ori does not perform implicit type narrowing after conditional checks. The type of a variable does not change based on control flow.

```ori
let x: Option<int> = Some(42);
if is_some(x) then
    // x is still Option<int> here, NOT int
    match x { Some(v) -> v, None -> 0 }
else
    0
```

To extract a value, use `match` for destructuring. This is the idiomatic Ori pattern for working with `Option` and `Result`.

NOTE  Type narrowing _does_ occur within `match` arms: a pattern `Some(v)` binds `v` with type `int`, not `Option<int>`.

## 14.2 Unary Expressions

### 14.2.1 Logical Not (`!`)

Inverts a boolean value.

```ori
!true;   // false
!false;  // true
!!x;     // x (double negation)
```

Type constraint: `! : bool -> bool`. It is a compile-time error to apply `!` to non-boolean types. For bitwise complement of integers, use `~`.

### 14.2.2 Arithmetic Negation (`-`)

Negates a numeric value.

```ori
-42;      // -42
-3.14;    // -3.14
-(-5);    // 5
```

Type constraints:
- `- : int -> int`
- `- : float -> float`
- `- : Duration -> Duration`

Integer negation panics on overflow: `-int.min` panics because the positive result does not fit in `int`.

Float negation never overflows (flips sign bit). Duration negation follows the same overflow rules as integer negation.

It is a compile-time error to apply unary `-` to `Size` (byte counts are non-negative).

### 14.2.3 Bitwise Not (`~`)

Inverts all bits of an integer.

```ori
~0;       // -1 (all bits set)
~(-1);    // 0
~5;       // -6
```

Type constraints:
- `~ : int -> int`
- `~ : byte -> byte`

For `int`, `~x` is equivalent to `-(x + 1)`. For `byte`, the result is the bitwise complement within 8 bits.

It is a compile-time error to apply `~` to `bool`. Use `!` for boolean negation.

## 14.3 Binary Expressions

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

### 14.3.1 Operator Type Constraints

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

Operands shall be the same type implementing `Comparable`. Returns `bool`.

**Equality** (`==` `!=`):

Operands shall be the same type implementing `Eq`. Returns `bool`.

Mixed-type operations are compile errors:

```ori
1 + 2.0;          // error: mismatched types int and float
float(1) + 2.0;   // OK: 3.0
1 + int(2.0);     // OK: 3
```

### 14.3.2 Numeric Behavior

**Integer overflow**: Panics. Addition, subtraction, multiplication, and negation all panic on overflow.

```ori
let max: int = 9223372036854775807;
max + 1;      // panic: integer overflow
int.min - 1;  // panic: integer overflow
-int.min;     // panic: integer overflow (negation)
```

Programs requiring wrapping or saturating arithmetic should use functions from `std.math`.

**Shift overflow**: Shift operations panic when the shift count is negative, exceeds the bit width, or the result overflows.

```ori
1 << 63;     // panic: shift overflow (result doesn't fit in signed int)
1 << 64;     // panic: shift count exceeds bit width
1 << -1;     // panic: negative shift count
16 >> 64;    // panic: shift count exceeds bit width
```

For `int` (signed, range -2⁶³ to 2⁶³ - 1), valid shift counts are 0 to 62 for left shift when the result shall remain representable. For right shift, counts 0 to 63 are valid.

For `byte` (unsigned, range 0 to 255), valid shift counts are 0 to 7.

**Integer division and modulo overflow**: The expression `int.min / -1` and `int.min % -1` panic because the mathematical result cannot be represented.

```ori
int.min div -1;  // panic: integer overflow
int.min % -1;    // panic: integer overflow
```

**Integer division by zero**: Panics.

```ori
5 / 0;    // panic: division by zero
5 % 0;    // panic: modulo by zero
```

**Float division by zero**: Returns infinity or NaN per IEEE 754.

```ori
1.0 / 0.0;    // Inf
-1.0 / 0.0;   // -Inf
0.0 / 0.0;    // NaN
```

**Float NaN propagation**: Any operation involving NaN produces NaN.

```ori
NaN + 1.0;    // NaN
NaN == NaN;   // false (IEEE 754)
NaN != NaN;   // true
```

**Float comparison**: Exact bit comparison. No epsilon tolerance.

```ori
0.1 + 0.2 == 0.3;  // false (floating-point representation)
```

## 14.4 Operator Precedence

Operators are listed from highest to lowest precedence:

| Level | Operators | Associativity | Description |
|-------|-----------|---------------|-------------|
| 1 | `.` `[]` `()` `?` `as` `as?` | Left | Postfix |
| 2 | `**` | Right | Power |
| 3 | `!` `-` `~` | Right | Unary |
| 4 | `*` `/` `%` `div` `@` | Left | Multiplicative |
| 5 | `+` `-` | Left | Additive |
| 6 | `<<` `>>` | Left | Shift |
| 7 | `..` `..=` `by` | Left | Range |
| 8 | `<` `>` `<=` `>=` | Left | Relational |
| 9 | `==` `!=` | Left | Equality |
| 10 | `&` | Left | Bitwise AND |
| 11 | `^` | Left | Bitwise XOR |
| 12 | `\|` | Left | Bitwise OR |
| 13 | `&&` | Left | Logical AND |
| 14 | `\|\|` | Left | Logical OR |
| 15 | `??` | Right | Coalesce |
| 16 | `\|>` | Left | Pipe |

Parentheses override precedence:

```ori
(a & b) == 0;    // Compare result of AND with 0
a & b == 0;      // Parsed as a & (b == 0) — likely not intended
```

## 14.5 Operator Traits

Operators are desugared to trait method calls. User-defined types can implement operator traits to support operator syntax.

### 14.5.1 Arithmetic Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a + b` | `Add` | `a.add(rhs: b)` |
| `a - b` | `Sub` | `a.subtract(rhs: b)` |
| `a * b` | `Mul` | `a.multiply(rhs: b)` |
| `a / b` | `Div` | `a.divide(rhs: b)` |
| `a div b` | `FloorDiv` | `a.floor_divide(rhs: b)` |
| `a % b` | `Rem` | `a.remainder(rhs: b)` |
| `a ** b` | `Pow` | `a.power(rhs: b)` |
| `a @ b` | `MatMul` | `a.matrix_multiply(rhs: b)` |

### 14.5.2 Unary Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `-a` | `Neg` | `a.negate()` |
| `!a` | `Not` | `a.not()` |
| `~a` | `BitNot` | `a.bit_not()` |

### 14.5.3 Bitwise Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a & b` | `BitAnd` | `a.bit_and(rhs: b)` |
| `a \| b` | `BitOr` | `a.bit_or(rhs: b)` |
| `a ^ b` | `BitXor` | `a.bit_xor(rhs: b)` |
| `a << b` | `Shl` | `a.shift_left(rhs: b)` |
| `a >> b` | `Shr` | `a.shift_right(rhs: b)` |

### 14.5.4 Comparison Operators

| Operator | Trait | Method |
|----------|-------|--------|
| `a == b` | `Eq` | `a.equals(other: b)` |
| `a != b` | `Eq` | `!a.equals(other: b)` |
| `a < b` | `Comparable` | `a.compare(other: b).is_less()` |
| `a <= b` | `Comparable` | `a.compare(other: b).is_less_or_equal()` |
| `a > b` | `Comparable` | `a.compare(other: b).is_greater()` |
| `a >= b` | `Comparable` | `a.compare(other: b).is_greater_or_equal()` |

### 14.5.5 Trait Definitions

Operator traits use default type parameters and default associated types:

```ori
trait Add<Rhs = Self> {
    type Output = Self;
    @add (self, rhs: Rhs) -> Self.Output;
}
```

The `Rhs` parameter defaults to `Self`, and `Output` defaults to `Self`. Implementations may override either.

NOTE  The `Div` trait method is named `divide` rather than `div` because `div` is a reserved keyword for the floor division operator.

### 14.5.6 User-Defined Example

```ori
type Vector2 = { x: float, y: float }

impl Vector2: Add {
    @add (self, rhs: Vector2) -> Self = Vector2 {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

let a = Vector2 { x: 1.0, y: 2.0 };
let b = Vector2 { x: 3.0, y: 4.0 };
let sum = a + b;  // Vector2 { x: 4.0, y: 6.0 }
```

### 14.5.7 Mixed-Type Operations

Traits support different operand types. Commutative operations require both orderings:

```ori
// Duration * int
impl Duration: Mul<int> {
    type Output = Duration;
    @multiply (self, n: int) -> Duration = ...;
}

// int * Duration
impl int: Mul<Duration> {
    type Output = Duration;
    @multiply (self, d: Duration) -> Duration = d * self;
}
```

The compiler does not automatically commute operands.

### 14.5.8 Built-in Implementations

Primitive types have built-in implementations for their applicable operators. These implementations use compiler intrinsics.

See [Declarations § Traits](10-declarations.md#103-traits) for trait definition syntax.

## 14.6 Range Expressions

Range expressions produce `Range<T>` values.

```ori
0..10;       // 0, 1, 2, ..., 9 (exclusive)
0..=10;      // 0, 1, 2, ..., 10 (inclusive)
```

### 14.6.1 Range with Step

The `by` keyword specifies a step value for non-unit increments:

```ori
0..10 by 2;      // 0, 2, 4, 6, 8
0..=10 by 2;     // 0, 2, 4, 6, 8, 10
10..0 by -1;     // 10, 9, 8, 7, 6, 5, 4, 3, 2, 1
10..=0 by -2;    // 10, 8, 6, 4, 2, 0
```

`by` is a context-sensitive keyword recognized only following a range expression.

**Type constraints:**

- Range with step is supported only for `int` ranges
- Start, end, and step shall all be `int`
- It is a compile-time error to use `by` with non-integer ranges

**Runtime behavior:**

- Step of zero causes a panic
- Mismatched direction produces an empty range (no panic)

```ori
0..10 by 0;      // panic: step cannot be zero
0..10 by -1;     // empty range (can't go from 0 to 10 with negative step)
10..0 by 1;      // empty range (can't go from 10 to 0 with positive step)
```

### 14.6.2 Infinite Ranges

Omitting the end creates an unbounded ascending range:

```ori
0..;           // 0, 1, 2, 3, ... (infinite ascending)
100..;         // 100, 101, 102, ... (infinite ascending from 100)
0.. by 2;      // 0, 2, 4, 6, ... (infinite ascending by 2)
0.. by -1;     // 0, -1, -2, ... (infinite descending)
```

**Type constraints:**

- Infinite ranges are supported only for `int`
- The step shall be non-zero (zero step panics)

**Semantics:**

- `start..` creates an unbounded range with step +1
- `start.. by step` creates an unbounded range with explicit step
- Infinite ranges implement `Iterable` but NOT `DoubleEndedIterator` (no end to iterate from)

Infinite ranges shall be bounded before terminal operations like `collect()`:

```ori
(0..).iter().take(count: 10).collect();    // OK: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
(0..).iter().collect();                     // infinite loop, eventually OOM
```

Implementations SHOULD warn on obvious unbounded consumption patterns.

## 14.7 With Expression

```ori
with Http = MockHttp { ... } in fetch("/data");

```

## 14.8 Let Binding

```ori
let x = 5;           // mutable
let $x = 5;          // immutable
let { x, y } = point;
let { $x, y } = point;  // x immutable, y mutable
```

## 14.9 Conditional

> **Grammar:** See [grammar.ebnf](grammar.md) § if_expr

```ori
if x > 0 then "positive" else "non-positive";

```

The _condition_ shall have type `bool`. It is a compile-time error if the condition has any other type.

### 14.9.1 Branch Evaluation

Only one branch is evaluated at runtime. The unevaluated branch does not execute. This is guaranteed and observable (side effects in the unevaluated branch do not occur).

### 14.9.2 Type Unification

When `else` is present, both branches shall produce types that unify to a common type:

```ori
if cond then 1 else 2;              // type: int
if cond then Some(1) else None;     // type: Option<int>
if cond then 1 else "two";          // error: cannot unify int and str
```

### 14.9.3 Without Else

When `else` is omitted, the expression has type `void`. The `then` branch shall have type `void` or `Never`:

```ori
// Valid: then-branch is void
if debug then print(msg: "debug mode");

// Valid: then-branch is Never (coerces to void)
if !valid then panic(msg: "invalid state");

// Invalid: then-branch has non-void type without else
if x > 0 then "positive";  // error: non-void then-branch requires else
```

When the `then` branch has type `Never`, it coerces to `void`.

### 14.9.4 Never Type Coercion

The `Never` type coerces to any type in conditional branches:

```ori
let x: int = if condition then 42 else panic(msg: "unreachable");
// else branch is Never, coerces to int
```

If both branches have type `Never`, the expression has type `Never`:

```ori
let x = if a then panic(msg: "a") else panic(msg: "b");
// type: Never
```

### 14.9.5 Else-If Chains

```ori
if condition1 then expression1
else if condition2 then expression2
else expression3
```

The grammar treats `else if` as a single production for parsing convenience, but semantically the `else` branch contains another `if` expression.

### 14.9.6 Struct Literal Restriction

Struct literals are not permitted directly in the condition position. This prevents parsing ambiguity with block expressions:

```ori
if Point { x: 0, y: 0 } then ...;  // error: struct literal in condition
if (Point { x: 0, y: 0 }) then ...;  // OK: parentheses re-enable struct literals
```

The parser disables struct literal parsing in the condition context. Parenthesized expressions re-enable it.

## 14.10 For Expression

> **Grammar:** See [grammar.ebnf](grammar.md) § For Expression

### 14.10.1 For-Do

The `for...do` expression iterates for side effects and returns `void`:

```ori
for item in items do print(msg: item);
for (key, value) in map do process(key: key, value: value);

```

The source shall implement `Iterable`. The binding supports destructuring patterns.

### 14.10.2 Guard Condition

An optional `if` clause filters elements:

```ori
for x in items if x > 0 do process(x: x);

```

The guard is evaluated per item before the body.

### 14.10.3 Break and Continue

In `for...do`, `break` exits the loop and `continue` skips to the next iteration:

```ori
for x in items do {
    if done(x) then break;
    if skip(x) then continue;
    process(x: x);
}
```

`break value` and `continue value` are errors in `for...do` context — there is no collection to contribute to.

### 14.10.4 For-Yield

The `for...yield` expression builds collections:

```ori
for n in numbers if n > 0 yield n * n;

```

See [Patterns § For-Yield Comprehensions](15-patterns.md#159-for-yield-comprehensions) for complete semantics including type inference, nested comprehensions, and break/continue with values.

### 14.10.5 Labeled For

Labels enable break/continue to target outer loops:

```ori
for:outer x in xs do
    for y in ys do
        if done(x, y) then break:outer;

```

See [Control Flow § Labeled Loops](16-control-flow.md#163-labeled-loops) for label semantics.

## 14.11 Loop Expression

> **Grammar:** See [grammar.ebnf](grammar.md) § loop_expr

The `loop { }` expression repeatedly evaluates its body until a `break` is encountered.

### 14.11.1 Syntax

```ori
loop {body}
loop:name { body }  // labeled
```

### 14.11.2 Body

The body is a block expression. Use `{ }` for multiple expressions:

```ori
// Single expression
loop {process_next()}

// Multiple expressions
loop {
    let x = compute();
    if done(x) then break x;
    update(x);
}
```

### 14.11.3 Loop Type

The type of a `loop` expression is determined by its break values:

- **Break with value**: Loop type is the break value type
- **Break without value**: Loop type is `void`
- **No break**: Loop type is `Never` (infinite loop)

```ori
let result: int = loop {
    let x = compute();
    if x > 100 then break x
};  // type: int

loop {
    let msg = receive();
    if is_shutdown(msg) then break;
    process(msg);
}  // type: void

@server () -> Never = loop {handle_request()};  // type: Never
```

### 14.11.4 Multiple Break Paths

All break paths shall produce compatible types:

```ori
loop {
    if a then break 1;      // int
    if b then break "two";  // error E0860: expected int, found str
}
```

### 14.11.5 Continue

`continue` skips the rest of the current iteration:

```ori
loop {
    let item = next();
    if is_none(item) then break;
    if skip(item.unwrap()) then continue;
    process(item.unwrap());
}
```

`continue value` in a loop is an error (E0861). Loops do not accumulate values.

### 14.11.6 Labeled Loops

Labels allow `break` and `continue` to target a specific loop. See [Control Flow § Labeled Loops](16-control-flow.md#163-labeled-loops) for label semantics.

## 14.12 Lambda

```ori
x -> x * 2;
(x, y) -> x + y;
(x: int) -> int = x * 2;
```

## 14.13 Evaluation

Expressions are evaluated left-to-right. This order is guaranteed and observable.

### 14.13.1 Operand Evaluation

Binary operators evaluate the left operand before the right:

```ori
left() + right();  // left() called first, then right()
```

### 14.13.2 Argument Evaluation

Function arguments are evaluated left-to-right as written, before the call:

```ori
foo(a: first(), b: second(), c: third());
// Order: first(), second(), third(), then foo()
```

Named arguments evaluate in written order, not parameter order:

```ori
foo(c: third(), a: first(), b: second());
// Order: third(), first(), second(), then foo()
```

### 14.13.3 Compound Expressions

Postfix operations evaluate left-to-right:

```ori
list[index()].method(arg());
// Order: list, index(), method lookup, arg(), method call
```

### 14.13.4 List and Map Literals

Elements evaluate left-to-right:

```ori
[first(), second(), third()];
{"a": first(), "b": second()};

```

## 14.14 Pipe Operator

> **Grammar:** See [grammar.ebnf](grammar.md) § `pipe_expr`, `pipe_step`

The _pipe operator_ `|>` enables left-to-right function composition. The left operand is evaluated and passed as an argument to the right operand.

```ori
data
    |> filter(predicate: x -> x > 0)
    |> map(transform: x -> x * 2)
    |> sum
```

### 14.14.1 Implicit Fill

When the right side of `|>` is a function call, the piped value fills the single _unspecified parameter_. A parameter is unspecified when it is both (a) not provided in the call arguments and (b) has no default value.

```ori
// max_pool2d has params: (input: Tensor, kernel_size: int) -> Tensor
x |> max_pool2d(kernel_size: 2)
// Fills: max_pool2d(input: x, kernel_size: 2)
```

It is a compile-time error if:
- Zero parameters are unspecified (all provided or defaulted): "nothing for pipe to fill"
- Two or more parameters are unspecified: "ambiguous pipe target; specify all parameters except one"

### 14.14.2 Method Calls on the Piped Value

A leading `.` calls a method on the piped value itself, rather than passing it as a function argument:

```ori
x |> .flatten(start_dim: 1)    // x.flatten(start_dim: 1)
x |> .sort()                   // x.sort()
```

Without the dot, the pipe step is a free function call with implicit fill:

```ori
x |> sort          // free function: sort(<piped>: x)
x |> .sort()       // method: x.sort()
```

### 14.14.3 Lambda Pipe Steps

A lambda receives the piped value as its parameter:

```ori
x |> (a -> a @ weight + bias)
x |> (a -> a ** 2)
```

### 14.14.4 Error Propagation

The `?` operator on a pipe step applies to the result of the desugared call:

```ori
data |> parse_csv?
// Desugars to: parse_csv(input: data)?
```

### 14.14.5 Desugaring

Each pipe step desugars to a let-binding and an ordinary call:

```ori
expr |> func(arg: val)
// Desugars to:
{
    let $__pipe = expr;
    func(<unspecified>: __pipe, arg: val)
}

expr |> .method(arg: val)
// Desugars to:
{
    let $__pipe = expr;
    __pipe.method(arg: val)
}
```

The type checker resolves implicit fill by inspecting the function signature. The evaluator and codegen see only the desugared form.

### 14.14.6 Precedence and Associativity

`|>` has the lowest precedence of all binary operators (level 16, below `??` at 15). It is left-associative.

```ori
a + b |> process       // (a + b) |> process
a |> f |> g |> h       // ((a |> f) |> g) |> h — equivalent to h(g(f(a)))
```

---

## 14.15 Spread Operator

> **Grammar:** See [grammar.ebnf](grammar.md) § EXPRESSIONS (list_element, map_element, struct_element)

The spread operator `...` expands collections and structs in literal contexts.

### 14.15.1 List Spread

Expands list elements into a list literal:

```ori
let a = [1, 2, 3];
let b = [4, 5, 6];

[...a, ...b];           // [1, 2, 3, 4, 5, 6]
[0, ...a, 10];          // [0, 1, 2, 3, 10]
[first, ...middle, last];

```

The spread expression shall be of type `[T]` where `T` matches the list element type.

### 14.15.2 Map Spread

Expands map entries into a map literal:

```ori
let defaults = {"timeout": 30, "retries": 3};
let custom = {"retries": 5, "verbose": true};

{...defaults, ...custom};
// {"timeout": 30, "retries": 5, "verbose": true}
```

Later entries override earlier ones on key conflicts. The spread expression shall be of type `{K: V}` matching the map type.

### 14.15.3 Struct Spread

Copies fields from an existing struct:

```ori
type Point = { x: int, y: int, z: int }
let original = Point { x: 1, y: 2, z: 3 };

Point { ...original, x: 10 };  // Point { x: 10, y: 2, z: 3 }
Point { x: 10, ...original };  // Point { x: 1, y: 2, z: 3 }
```

Order determines precedence: later fields override earlier ones. The spread expression shall be of the same struct type.

### 14.15.4 Constraints

- Spread is only valid in literal contexts (lists, maps, struct constructors)
- It is a compile-time error to use spread in function call arguments
- All spread expressions shall have compatible types with the target container
- Struct spread requires the exact same type (not subtypes or supertypes)

### 14.15.5 Evaluation Order

Spread expressions evaluate left-to-right:

```ori
[first(), ...middle(), last()]
// Order: first(), middle(), last()

{...defaults(), "key": computed(), ...overrides()}
// Order: defaults(), computed(), overrides()
```

### 14.15.6 Assignment

The right side evaluates before assignment:

```ori
x = compute();  // compute() evaluated, then assigned to x
```

### 14.15.7 Compound Assignment

> **Grammar:** See [grammar.ebnf](grammar.md) § `compound_op`
> **Rules:** See [operator-rules.md](operator-rules.md) § Compound Assignment

A _compound assignment_ `x op= y` desugars to `x = x op y` at parse time. The left-hand side shall be a mutable binding. Compound assignment is a statement, not an expression.

```ori
x += 1;              // desugars to: x = x + 1
point.x *= scale;    // desugars to: point.x = point.x * scale
flags |= MASK;       // desugars to: flags = flags | MASK
passed &&= check();  // desugars to: passed = passed && check()
```

Supported operators: `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `@=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `&&=`, `||=`.

The `&&=` and `||=` forms preserve short-circuit evaluation: `x &&= expr` does not evaluate `expr` when `x` is `false`.

### 14.15.8 Short-Circuit Evaluation

Logical and coalesce operators may skip the right operand:

| Operator | Skips right when |
|----------|------------------|
| `&&` | Left is `false` |
| `\|\|` | Left is `true` |
| `??` | Left is `Some`/`Ok` |

```ori
false && expensive();  // expensive() not called
true \|\| expensive();  // expensive() not called
Some(x) ?? expensive();  // expensive() not called
```

### 14.15.9 Conditional Branches

Only the taken branch is evaluated:

```ori
if condition then
    only_if_true()
else
    only_if_false()
```

See [Control Flow](16-control-flow.md) for details on conditionals and loops.

## 14.16 Composite Literal Typing

### 14.16.1 List literals

The type of a list literal `[e1, e2, ..., en]` is `[T]` where `T` is the unified type of all elements. All elements shall have the same type; there are no implicit numeric conversions.

```ori
[1, 2, 3]           // type: [int]
["a", "b"]          // type: [str]
[1, "hello"]        // error: cannot unify int and str
[1, 2.0]            // error: no implicit int → float conversion
[[1, 2], [3, 4]]    // type: [[int]]
```

An empty list literal `[]` requires type context for inference. Without context, it is a compile-time error.

```ori
let x: [int] = [];  // OK: type context provides [int]
let y = [];          // error: cannot infer element type
```

### 14.16.2 Map literals

The type of a map literal `{k1: v1, k2: v2, ...}` is `{K: V}` where `K` is the unified key type and `V` is the unified value type. Keys shall implement `Eq` and `Hashable`.

```ori
{"name": "Alice", "city": "NYC"}    // type: {str: str}
{1: "one", 2: "two"}                // type: {int: str}
```

The syntax `{ }` (braces with whitespace) is parsed as an empty map literal.

NOTE  When key expressions are bare identifiers (e.g., `{key: value}`), the parser distinguishes map literals from struct literals by the presence or absence of a type name prefix.

### 14.16.3 Struct literals

The type of a struct literal `TypeName { f1: v1, f2: v2 }` is `TypeName`. All fields shall be provided unless a spread expression (`...`) supplies the remaining fields. Field types shall match the declared field types.

An unknown field name is a compile-time error. A missing required field is a compile-time error.

### 14.16.4 Tuple literals

The type of a tuple literal `(e1, e2, ..., en)` is `(T1, T2, ..., Tn)` where each `Ti` is the type of the corresponding element. The empty tuple `()` has type `void`.

NOTE  Single-element tuples are not supported. `(a,)` is parsed as a parenthesized expression.

## 14.17 Template String Semantics

A template string `` `text {expr} text` `` desugars to string concatenation of its segments. Each interpolated expression is converted to `str` by calling its `Printable.to_str()` method.

When a format specifier is present, the expression shall implement `Formattable`, and the interpolation desugars to a call to `Formattable.format(self:, spec:)`.

Segments are evaluated left-to-right. Nested template strings are valid.

EXAMPLE  `` `Hello, {name}!` `` desugars to `"Hello, " + name.to_str() + "!"`.

## 14.18 General Evaluation Order

Expressions are evaluated left-to-right. In a binary expression `a + b`, `a` is evaluated before `b`. In a function call `f(x: a, y: b)`, `a` is evaluated before `b`. Arguments are evaluated in textual order regardless of parameter names.

Exceptions to left-to-right evaluation:
- Short-circuit operators (`&&`, `||`, `??`) may skip the right operand (see 14.15.8).
- Conditional branches evaluate only the taken branch (see 14.15.9).

## 14.19 Method Values

Methods cannot be referenced without calling them. The expression `value.method` without trailing parentheses is a compile-time error.

```ori
let f = list.contains;   // error: method reference not supported
let g = list.contains(value: 42);  // OK: method call
```

NOTE  Associated functions may be referenced as values by their qualified name. See [Clause 10](10-declarations.md#1041-associated-functions).

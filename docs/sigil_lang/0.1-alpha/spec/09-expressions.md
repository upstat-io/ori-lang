# Expressions

This section defines the expression syntax and semantics.

## Expression Categories

```
expression    = with_expr
              | let_expr
              | pattern_expr
              | if_expr
              | for_expr
              | loop_expr
              | lambda
              | binary_expr .

with_expr     = "with" identifier "=" expression "in" expression .
let_expr      = "let" [ "mut" ] identifier [ ":" type ] "=" expression .
```

## Primary Expressions

### Syntax

```
primary       = literal
              | identifier
              | "self"
              | "Self"
              | "(" expression ")"
              | list_literal
              | map_literal
              | struct_literal .

list_literal  = "[" [ expression { "," expression } [ "," ] ] "]" .
map_literal   = "{" [ map_entry { "," map_entry } [ "," ] ] "}" .
map_entry     = expression ":" expression .
struct_literal = type_path "{" [ field_init { "," field_init } [ "," ] ] "}" .
field_init    = identifier [ ":" expression ] .
```

### Identifiers

An identifier expression evaluates to the value bound to that identifier:

```sigil
x         // variable reference
add       // function reference
Point     // type reference (in type position)
```

### self and Self

- `self` — the receiver instance in methods
- `Self` — the implementing type in trait/impl contexts

### Literals

See [Lexical Elements § Literals](03-lexical-elements.md#literals).

### List Literals

```sigil
[]              // empty list
[1, 2, 3]       // list of integers
[a, b, c]       // list from variables
```

### Map Literals

```sigil
{}                          // empty map
{"a": 1, "b": 2}           // map literal
{key1: value1, key2: value2}
```

### Struct Literals

```sigil
Point { x: 0, y: 0 }
User { id: 1, name: "Alice", email: "a@b.com" }
Point { x, y }              // field shorthand when variable matches field name
```

## Postfix Expressions

### Syntax

```ebnf
postfix_expr  = primary { postfix_op } .
postfix_op    = "." identifier [ call_args ]
              | "[" expression "]"
              | call_args
              | "?" .

call_args     = "(" [ call_arg { "," call_arg } [ "," ] ] ")" .
call_arg      = expression | named_arg .
named_arg     = "." identifier ":" expression .
```

> **Note:** Sigil does not have a `.await` postfix operator. Async behavior is declared at the function level via `uses Async`. See [Capabilities](14-capabilities.md).

### Field Access

```sigil
point.x
user.name
config.timeout
```

### Method Call

```sigil
list.len()
string.upper()
value.to_string()
```

### Index Access

```sigil
list[0]
list[# - 1]     // # refers to length within brackets
map["key"]
```

The `#` symbol within index brackets refers to the length of the collection.

Indexing rules:

- Lists require an `int` index and panic on out-of-bounds access.
- Strings require an `int` index, return a single-code-point `str`, and panic on out-of-bounds access.
- Map indexing returns `Option<V>` and yields `None` if the key is missing.

### Function Call

```sigil
add(1, 2)
process(data)
fetch_user(id)
```

Named arguments may be used in place of positional arguments. When named arguments are used, every argument must be named, each parameter may appear at most once, argument order is irrelevant, and names must match the function's parameter names.

```sigil
add(.a: 1, .b: 2)
fetch_user(.id: id)
```

### Error Propagation

The `?` suffix propagates errors from `Result` types:

```sigil
value?          // returns Err if value is Err
parse(input)?   // propagates parse error
```

Within a `try` block, `?` unwraps `Ok` values and returns early on `Err`.

## Unary Expressions

### Syntax

```
unary_expr    = [ "!" | "-" | "~" ] postfix_expr .
```

### Logical Not

```sigil
!true       // false
!false      // true
!condition
```

### Negation

```sigil
-42
-x
-3.14
```

## Binary Expressions

### Syntax

```
binary_expr   = or_expr .
or_expr       = and_expr { "||" and_expr } .
and_expr      = bit_or_expr { "&&" bit_or_expr } .
bit_or_expr   = bit_xor_expr { "|" bit_xor_expr } .
bit_xor_expr  = bit_and_expr { "^" bit_and_expr } .
bit_and_expr  = eq_expr { "&" eq_expr } .
eq_expr       = cmp_expr { ( "==" | "!=" ) cmp_expr } .
cmp_expr      = range_expr { ( "<" | ">" | "<=" | ">=" ) range_expr } .
range_expr    = add_expr [ ( ".." | "..=" ) add_expr ] .
add_expr      = mul_expr { ( "+" | "-" ) mul_expr } .
mul_expr      = unary_expr { ( "*" | "/" | "%" | "div" ) unary_expr } .
```

### Arithmetic Operators

| Operator | Operation | Operand Types | Result Type |
|----------|-----------|---------------|-------------|
| `+` | Addition | `int`, `int` | `int` |
| `+` | Addition | `float`, `float` | `float` |
| `+` | Concatenation | `str`, `str` | `str` |
| `+` | Concatenation | `[T]`, `[T]` | `[T]` |
| `-` | Subtraction | numeric | same |
| `*` | Multiplication | numeric | same |
| `/` | Division | numeric | same |
| `%` | Modulo | `int`, `int` | `int` |
| `div` | Floor division | `int`, `int` | `int` |

Division `/` truncates toward zero for integers. Floor division `div` truncates toward negative infinity.

### Comparison Operators

| Operator | Meaning |
|----------|---------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `>` | Greater than |
| `<=` | Less or equal |
| `>=` | Greater or equal |

Comparison operators return `bool`.

### Logical Operators

| Operator | Meaning | Short-circuit |
|----------|---------|---------------|
| `&&` | Logical AND | Yes |
| `\|\|` | Logical OR | Yes |

Logical operators use short-circuit evaluation: the right operand is evaluated only if necessary.

### Range Operators

| Operator | Meaning |
|----------|---------|
| `..` | Exclusive range |
| `..=` | Inclusive range |

```sigil
0..10       // 0, 1, 2, ..., 9
0..=10      // 0, 1, 2, ..., 10
```

### Coalesce Operator

```sigil
expression ?? default
```

If the left expression is `None` or `Err`, evaluates to `default`. Otherwise evaluates to the unwrapped value.

### Bitwise Operators

| Operator | Operation | Operand Types | Result Type |
|----------|-----------|---------------|-------------|
| `&` | Bitwise AND | `byte`, `byte` | `byte` |
| `\|` | Bitwise OR | `byte`, `byte` | `byte` |
| `^` | Bitwise XOR | `byte`, `byte` | `byte` |
| `~` | Bitwise NOT | `byte` | `byte` |

Bitwise operations wrap on overflow.

## With Expression

A `with` expression provides a capability implementation for the `in` expression.

```sigil
with Http = RealHttp { base_url: "https://api.example.com" } in
    fetch_user("123")
```

See [Capabilities](14-capabilities.md) for capability scoping rules.

## Let Binding

### Syntax

```
let_expr      = "let" [ "mut" ] identifier [ ":" type ] "=" expression .
```

### Semantics

A `let` expression introduces a new binding in the current scope. Bindings are immutable by default.

```sigil
let x = 5
let name = "Alice"
let point = Point { x: 0, y: 0 }
```

The optional type annotation constrains the binding:

```sigil
let x: int = 5
let items: [str] = []
```

### Mutable Bindings

The `mut` modifier creates a mutable binding that can be reassigned:

```sigil
let mut counter = 0
counter = counter + 1
```

Reassignment to an immutable binding is a compile-time error.

### Shadowing

A binding may shadow an outer binding with the same name:

```sigil
let x = 5
let x = x + 1    // shadows outer x
```

Each `let` creates a new binding. Shadowing is distinct from mutation.

### Destructuring

Bindings support pattern destructuring:

```sigil
let { x, y } = point           // struct destructuring
let (first, second) = pair     // tuple destructuring
let [head, ..tail] = items     // list destructuring
```

See [Patterns § Match Patterns](10-patterns.md#match-patterns) for pattern syntax.

## Conditional Expression

### Syntax

```
if_expr       = "if" expression "then" expression
                { "else" "if" expression "then" expression }
                "else" expression .
```

### Semantics

The condition must have type `bool`. Both branches must have compatible types.

```sigil
if x > 0 then "positive" else "non-positive"

if n % 15 == 0 then "FizzBuzz"
else if n % 3 == 0 then "Fizz"
else if n % 5 == 0 then "Buzz"
else str(n)
```

## For Expression

### Imperative Form

```
for_imperative = "for" for_binding { "," for_binding } [ for_guard ] ( do_clause | yield_clause ) .
for_binding    = identifier "in" expression .
for_guard      = "if" expression .
do_clause      = "do" expression .
yield_clause   = "yield" expression .
```

The `do` form executes for side effects (returns `void`):

```sigil
for item in items do print(item)
```

The `yield` form builds a new collection:

```sigil
for n in numbers yield n * 2
for n in numbers if n > 0 yield n * n
```

### Pattern Form

```
for_pattern    = "for" "(" named_args ")" .
```

See [Patterns § for](10-patterns.md#for).

## Loop Expression

### Syntax

```
loop_expr     = "loop" "(" expression ")" .
break_expr    = "break" .
continue_expr = "continue" .
```

### Semantics

A `loop` expression repeats indefinitely until `break` is encountered:

```sigil
@process_channel (ch: Channel<int>) -> void uses Async = loop(
    match(ch.receive(),
        Some(value) -> process(value),
        None -> break,
    ),
)
```

`continue` skips to the next iteration.

## Lambda Expression

### Syntax

```
lambda        = simple_lambda | typed_lambda .
simple_lambda = lambda_params "->" expression .
typed_lambda  = "(" [ typed_param { "," typed_param } ] ")" "->" type "=" expression .
lambda_params = identifier
              | "(" [ identifier { "," identifier } ] ")" .
typed_param   = identifier ":" type .
```

### Semantics

A lambda creates an anonymous function:

```sigil
x -> x * 2
(x, y) -> x + y
() -> 42
```

Lambda parameter types are inferred from context.

### Typed Lambdas

When explicit type annotations are needed, use the typed lambda form with `=`:

```sigil
(x: int) -> int = x * 2
(a: int, b: int) -> int = a + b
() -> str = "hello"
```

The `=` separates the signature from the body, consistent with function definitions.

## Match Expression

See [Patterns § match](10-patterns.md#match).

## Pattern Expressions

Pattern expressions are covered in [Patterns](10-patterns.md).

## Expression Evaluation

### Order of Evaluation

Expressions are evaluated left-to-right. Function arguments are evaluated left-to-right before the call.

### Side Effects

Side effects occur in evaluation order. Short-circuit operators may prevent evaluation of the right operand.

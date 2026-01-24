# Expressions

Expressions compute values.

## Syntax

```
expression    = with_expr | let_expr | if_expr | for_expr | loop_expr | lambda | binary_expr .
primary       = literal | identifier | "self" | "Self"
              | "(" expression ")" | list_literal | map_literal | struct_literal .
list_literal  = "[" [ expression { "," expression } ] "]" .
map_literal   = "{" [ map_entry { "," map_entry } ] "}" .
map_entry     = expression ":" expression .
struct_literal = type_path "{" [ field_init { "," field_init } ] "}" .
field_init    = identifier [ ":" expression ] .
```

## Postfix Expressions

```
postfix_expr  = primary { postfix_op } .
postfix_op    = "." identifier [ call_args ]
              | "[" expression "]"
              | call_args
              | "?" .
call_args     = "(" [ call_arg { "," call_arg } ] ")" .
call_arg      = expression | named_arg .
named_arg     = identifier ":" expression .
```

### Field and Method Access

```sigil
point.x
list.len()
```

### Index Access

```sigil
list[0]
list[# - 1]    // # is length within brackets
map["key"]     // returns Option<V>
```

Lists/strings panic on out-of-bounds; maps return `Option`.

### Function Call

```sigil
add(a: 1, b: 2)
fetch_user(id: 1)
```

Named arguments: all-or-nothing, order irrelevant, names must match parameters.

### Error Propagation

```sigil
value?         // returns Err early if Err
```

## Unary Expressions

```
unary_expr = [ "!" | "-" | "~" ] postfix_expr .
```

`!` logical not, `-` negation, `~` bitwise not.

## Binary Expressions

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
| `??` | Coalesce (None/Err → default) |

## With Expression

```
with_expr = "with" identifier "=" expression "in" expression .
```

```sigil
with Http = MockHttp { ... } in fetch("/data")
```

## Let Binding

```
let_expr = "let" [ "mut" ] pattern [ ":" type ] "=" expression .
```

```sigil
let x = 5
let mut counter = 0
let { x, y } = point
```

## Conditional

```
if_expr = "if" expression "then" expression
          { "else" "if" expression "then" expression }
          "else" expression .
```

```sigil
if x > 0 then "positive" else "non-positive"
```

Condition must be `bool`. Branches must have compatible types.

## For Expression

```
for_expr   = "for" identifier "in" expression [ "if" expression ] ( "do" | "yield" ) expression .
```

```sigil
for item in items do print(item)
for n in numbers if n > 0 yield n * n
```

`do` returns `void`; `yield` collects results.

## Loop Expression

```
loop_expr = "loop" "(" expression ")" .
```

```sigil
loop(
    match(ch.receive(),
        Some(v) -> process(v),
        None -> break,
    ),
)
```

`break` exits; `continue` skips to next iteration.

## Lambda

```
lambda        = simple_lambda | typed_lambda .
simple_lambda = lambda_params "->" expression .
typed_lambda  = "(" [ typed_param { "," typed_param } ] ")" "->" type "=" expression .
lambda_params = identifier | "(" [ identifier { "," identifier } ] ")" .
```

```sigil
x -> x * 2
(x, y) -> x + y
(x: int) -> int = x * 2
```

## Evaluation

Expressions are evaluated left-to-right. This order is guaranteed and observable.

### Operand Evaluation

Binary operators evaluate the left operand before the right:

```sigil
left() + right()  // left() called first, then right()
```

### Argument Evaluation

Function arguments are evaluated left-to-right as written, before the call:

```sigil
foo(a: first(), b: second(), c: third())
// Order: first(), second(), third(), then foo()
```

Named arguments evaluate in written order, not parameter order:

```sigil
foo(c: third(), a: first(), b: second())
// Order: third(), first(), second(), then foo()
```

### Compound Expressions

Postfix operations evaluate left-to-right:

```sigil
list[index()].method(arg())
// Order: list, index(), method lookup, arg(), method call
```

### List and Map Literals

Elements evaluate left-to-right:

```sigil
[first(), second(), third()]
{"a": first(), "b": second()}
```

### Assignment

The right side evaluates before assignment:

```sigil
x = compute()  // compute() evaluated, then assigned to x
```

### Short-Circuit Evaluation

Logical and coalesce operators may skip the right operand:

| Operator | Skips right when |
|----------|------------------|
| `&&` | Left is `false` |
| `\|\|` | Left is `true` |
| `??` | Left is `Some`/`Ok` |

```sigil
false && expensive()  // expensive() not called
true \|\| expensive()  // expensive() not called
Some(x) ?? expensive()  // expensive() not called
```

### Conditional Branches

Only the taken branch is evaluated:

```sigil
if condition then
    only_if_true()
else
    only_if_false()
```

See [Control Flow](19-control-flow.md) for details on conditionals and loops.

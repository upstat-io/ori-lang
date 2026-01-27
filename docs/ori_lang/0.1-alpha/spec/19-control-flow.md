---
title: "Control Flow"
description: "Ori Language Specification — Control Flow"
order: 19
---

# Control Flow

Control flow determines the order of expression evaluation and how execution transfers between expressions.

## Sequential Flow

Expressions in `run(...)` evaluate left to right. Each expression completes before the next begins.

```ori
run(
    let x = 1,
    let y = 2,
    x + y,
)
```

If any expression terminates early (via `break`, `continue`, `?`, or panic), subsequent expressions are not evaluated.

## Loop Control

### Break

`break` exits the innermost enclosing loop.

```ori
loop(
    if done then break,
    process(),
)
```

`break` may include a value. The loop expression evaluates to this value:

```ori
let result = loop(
    let x = compute(),
    if x > 100 then break x,
)
// result = first x greater than 100
```

A `break` without a value in a context requiring a value is an error.

### Continue

`continue` skips to the next iteration of the innermost enclosing loop.

```ori
for x in items do
    if x < 0 then continue,
    process(x),
```

In `for...yield`, `continue` without a value skips the element:

```ori
for x in items yield
    if x < 0 then continue,  // element not added
    x * 2,
```

`continue` with a value uses that value for the current iteration:

```ori
for x in items yield
    if x < 0 then continue 0,  // use 0 instead
    x * 2,
```

## Labeled Loops

Labels allow `break` and `continue` to target an outer loop.

### Label Declaration

Labels use the syntax `loop:name` or `for:name`, with no space around the colon:

```ori
loop:outer(
    for x in items do
        if x == target then break:outer,
)

for:outer x in items do
    for y in other do
        if done(x, y) then break:outer,
```

### Label Reference

Reference labels with `break:name` or `continue:name`:

```ori
loop:search(
    for x in items do
        if found(x) then break:search x,
)
```

With value:

```ori
let result = loop:outer(
    for x in items do
        if match(x) then break:outer x,
    None,
)
```

## Error Propagation

The `?` operator propagates errors and absent values.

### On Result

If the value is `Err(e)`, the enclosing function returns `Err(e)`:

```ori
@load (path: str) -> Result<Data, Error> = run(
    let content = read_file(path)?,  // Err propagates
    let data = parse(content)?,
    Ok(data),
)
```

### On Option

If the value is `None`, the enclosing function returns `None`:

```ori
@find (id: int) -> Option<User> = run(
    let record = db.lookup(id)?,  // None propagates
    Some(User { ...record }),
)
```

The function's return type must be compatible with the propagated type.

## Terminating Expressions

Some expressions never produce a value normally:

| Expression | Behavior |
|------------|----------|
| `panic(msg)` | Terminates program |
| `break` | Exits loop |
| `continue` | Skips to next iteration |
| `expr?` on Err/None | Returns from function |

These expressions have type `Never`, which is compatible with any type:

```ori
let x: int = if condition then 42 else panic("unreachable")
```

## Evaluation in Conditionals

In `if...then...else`, only one branch evaluates:

```ori
if condition then
    expr_a  // evaluated if true
else
    expr_b  // evaluated if false
```

In `match`, only the matching arm evaluates:

```ori
match(value,
    Some(x) -> process(x),  // only if Some
    None -> default(),      // only if None
)
```

## Short-Circuit Operators

Logical operators may skip evaluation of the right operand:

| Operator | Skips right when |
|----------|------------------|
| `&&` | Left is `false` |
| `\|\|` | Left is `true` |
| `??` | Left is not `None`/`Err` |

```ori
valid && expensive()   // expensive() skipped if valid is false
cached ?? compute()    // compute() skipped if cached is Some/Ok
```

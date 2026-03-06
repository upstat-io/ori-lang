---
title: "Control Flow"
description: "Clause 16: Ori Language Specification — Control Flow"
order: 16
section: "Language"
---

# 16 Control flow

Control flow determines the order of expression evaluation and how execution transfers between expressions.

Ori is expression-based: all control flow constructs are expressions that produce values. The distinction between "statement" and "expression" is positional — any expression terminated by `;` is used as a statement (its value is discarded).

## 16.0 Statements and expressions

### 16.0.1 Expression statements

An _expression statement_ is an expression evaluated for its side effects. The result value is discarded. An expression statement is terminated by `;` within a block.

```ori
print(msg: "hello");     // evaluated for side effect, result discarded
```

### 16.0.2 Result expressions

The last expression in a block, if not terminated by `;`, is the _result expression_. Its value becomes the value of the block. See [7.8.1](07-lexical-elements.md#781-block-semicolons).

### 16.0.3 Statement-only constructs

The following constructs always have type `void` and are used only as statements:

- `let` bindings
- `use` imports
- Assignments (`x = v`)
- Compound assignments (`x += v`)

### 16.0.4 Value-producing expressions

All other constructs are expressions that produce values:

| Expression | Type |
|------------|------|
| `if c then a else b` | Unified type of `a` and `b` |
| `if c then a` | `void` (or `Never` if `a` is `Never`) |
| `match expr { arms }` | Unified type of all arm bodies |
| `for x in s yield e` | `[T]` where `T` is type of `e` |
| `for x in s do e` | `void` |
| `while c do e` | `void` |
| `loop { ... break v }` | Type of `v` |
| `loop { ... break }` | `void` |
| `loop { ... }` (no break) | `Never` |
| `block:name { ... break:name v }` | Type of `v` |
| `try { ... }` | `Result<T, E>` |

## 16.1 Sequential flow

Expressions in a block `{ }` evaluate top to bottom. Each expression completes before the next begins.

```ori
{
    let x = 1;
    let y = 2;
    x + y
}
```

If any expression terminates early (via `break`, `continue`, `?`, or panic), subsequent expressions are not evaluated.

## 16.2 Loop control

### 16.2.1 Break

`break` exits the innermost enclosing loop.

```ori
loop {
    if done then break;
    process()
}
```

`break` may include a value. The loop expression evaluates to this value:

```ori
let result = loop {
    let x = compute();
    if x > 100 then break x
}
// result = first x greater than 100
```

A `break` without a value in a context requiring a value is an error.

### 16.2.2 Continue

`continue` skips to the next iteration of the innermost enclosing loop.

```ori
for x in items do {
    if x < 0 then continue;
    process(x);
}
```

In `for...yield`, `continue` without a value skips the element:

```ori
for x in items yield {
    if x < 0 then continue;  // element not added

    x * 2
}
```

`continue` with a value uses that value for the current iteration:

```ori
for x in items yield {
    if x < 0 then continue 0;  // use 0 instead

    x * 2
}
```

### 16.2.3 Continue in loop

In `loop { }`, `continue` skips to the next iteration. `continue value` is an error (E0861) — loops do not accumulate values:

```ori
loop {
    if skip then continue;  // OK: start next iteration
    if bad then continue 42;  // error E0861: loop doesn't collect
    process()
}
```

## 16.2.4 While loop

A `while` expression evaluates a condition before each iteration. If the condition is `false`, the loop exits.

> **Grammar:** See [Annex A](grammar.md) § `while_expr`

`while condition do body` desugars to:

```ori
loop {
    if !condition then break;
    body
}
```

The condition expression shall have type `bool`. The `while` expression has type `void`.

```ori
while self.pos < self.buf.len() do {
    self.pos += 1
}
```

`break` exits the loop. `break value` is a compile-time error (E0860) — `while...do` does not produce a value.

`continue` skips to the next iteration (re-evaluating the condition). `continue value` is a compile-time error (E0861) — `while` does not accumulate values.

Labels work on `while` loops as on `loop` and `for`:

```ori
while:outer scanning do {
    while self.pos < self.buf.len() do {
        if done then break:outer
    }
}
```

NOTE  There is no `while...yield` form. Use `for...yield` with an iterator for collection building.

## 16.3 Labeled loops

Labels allow `break` and `continue` to target an outer loop.

### 16.3.1 Label declaration

Labels use the syntax `loop:name`, `while:name`, or `for:name`, with no space around the colon:

```ori
loop:outer {
    for x in items do
        if x == target then break:outer
}

for:outer x in items do
    for y in other do
        if done(x, y) then break:outer
```

### 16.3.2 Label reference

Reference labels with `break:name` or `continue:name`:

```ori
loop:search {
    for x in items do
        if found(x) then break:search x
}
```

With value:

```ori
let result = loop:outer {
    for x in items do
        if match(x) then break:outer x
    None
}
```

### 16.3.3 Label scope

A label is visible within the loop body it labels. Labels scope correctly through arbitrary nesting:

```ori
loop:a {
    loop:b {
        loop:c {
            break:a;   // OK: exits outermost
            break:b;   // OK: exits middle
            break:c    // OK: exits innermost
        }
    }
}
```

There is no language-imposed limit on label nesting depth.

### 16.3.4 No label shadowing

Labels cannot be shadowed within their scope:

```ori
loop:outer {
    loop:outer {  // ERROR E0871: label 'outer' already in scope
        ...
    }
}
```

### 16.3.5 Type consistency

All `break` paths for a labeled loop shall produce values of the same type:

```ori
let x: int = loop:outer {
    for item in items do {
        if a(item) then break:outer 1;       // int
        if b(item) then break:outer "two";   // ERROR E0872: expected int, found str
    }
    0
}
```

### 16.3.6 Continue with value

In `for...yield` context, `continue:name value` contributes `value` to the outer loop's collection:

```ori
let results = for:outer x in xs yield
    for:inner y in ys yield {
        if special(x, y) then continue:outer x * y;  // Contribute to outer

        transform(x, y)
    }
```

The value in `continue:label value` shall have the same type as the target loop's yield element type.

When `continue:label value` exits an inner `for...yield` to contribute to an outer `for...yield`, the inner loop's partially-built collection is discarded. Only `value` is contributed to the outer loop for this iteration.

In `for...do` context, `continue:name value` is an error — there is no collection to contribute to:

```ori
for:outer x in xs do
    for y in ys do {
        if skip(x, y) then continue:outer 42;  // ERROR E0873: for-do doesn't collect
        process(x, y);
    }
```

### 16.3.7 Valid label names

Labels follow identifier rules. They cannot be keywords:

```ori
loop:search { }      // OK
loop:_private { }    // OK
loop:loop123 { }     // OK
loop:for { }         // ERROR: 'for' is a keyword
```

## 16.4 Labeled blocks

A _labeled block_ is a block expression with a label, allowing early exit via `break:label value`.

> **Grammar:** See [Annex A](grammar.md) § `labeled_block`

### 16.4.1 Syntax

The syntax is `block:name { body }`, where `block` is a context-sensitive keyword recognized only before `:`:

```ori
let x = block:done {
    if condition1 then break:done value1;
    if condition2 then break:done value2;
    default_value
}
```

`block` is a valid identifier outside this position:

```ori
let block = 5;              // OK: identifier
let x = block:done { 42 }   // OK: labeled block
```

### 16.4.2 Semantics

`break:label value` exits the named block and produces `value` as the block's result. All `break:label` paths and the final expression shall have compatible types. The type of the labeled block is the unified type of all exit paths.

```ori
@validate (input: Request) -> Result<ValidRequest, Error> = block:done {
    if input.name.is_empty() then
        break:done Err(Error { message: "name required" });

    Ok(ValidRequest { name: input.name })
}
```

### 16.4.3 Unlabeled break is loop-only

Bare `break` (without a label) inside a labeled block targets the innermost enclosing **loop**, not the block:

```ori
loop {
    let x = block:result {
        if done then break;          // exits the LOOP
        if found then break:result v; // exits the BLOCK
        default
    };
    process(x)
}
```

### 16.4.4 Continue targeting a block

`continue:label` targeting a labeled block is a compile-time error. Blocks do not iterate:

```ori
block:result {
    continue:result;  // ERROR: cannot continue a labeled block
}
```

### 16.4.5 Transparency to loop control flow

Labeled blocks are transparent to `break` and `continue` targeting outer loops, in the same way as `try` blocks (see 16.7.3):

```ori
for:search items in collection do {
    let result = block:check {
        if invalid(items) then continue:search;  // OK: continues outer for loop
        if found(items) then break:search items;  // OK: breaks outer for loop
        transform(items)
    };
    process(result)
}
```

### 16.4.6 Nesting and label namespace

Labeled blocks are nestable. Block labels share the label namespace with loop labels. The no-shadowing rule (16.3.4) applies across all labeled constructs:

```ori
block:outer {
    block:inner {
        break:outer 1;  // OK: exits outer block
        break:inner 2;  // OK: exits inner block
    }
}

loop:name {
    block:name { }  // ERROR E0871: label 'name' already in scope
}
```

## 16.5 Error propagation

The `?` operator propagates errors and absent values.

### 16.5.1 On Result

If the value is `Err(e)`, the enclosing function returns `Err(e)`:

```ori
@load (path: str) -> Result<Data, Error> = {
    let content = read_file(path)?;  // Err propagates
    let data = parse(content)?;
    Ok(data)
}
```

### 16.5.2 On Option

If the value is `None`, the enclosing function returns `None`:

```ori
@find (id: int) -> Option<User> = {
    let record = db.lookup(id)?;  // None propagates
    Some(User { ...record })
}
```

The function's return type shall be compatible with the propagated type.

## 16.6 Terminating expressions

A _terminating expression_ is an expression whose evaluation is guaranteed to not complete normally. Terminating expressions have type `Never`, which is compatible with any type (see [8.1.1](08-types.md)).

The following are terminating expressions:

1. `panic(msg:)`, `todo()`, `unreachable()` — always terminate the program
2. `break` and `break value` — exit the enclosing loop or labeled block
3. `continue` and `continue value` — skip to the next iteration
4. `expr?` when the Err/None branch is taken — returns from the enclosing function
5. A block `{ ... e }` where the last expression `e` is terminating
6. `if c then t else e` where both `t` and `e` are terminating
7. `match expr { arms }` where every arm body is terminating
8. `loop { body }` with no `break` — an infinite loop with type `Never`

```ori
let x: int = if condition then 42 else panic(msg: "unreachable");
// panic(...) has type Never, compatible with int
```

Code following a terminating expression within the same block is unreachable. The compiler should warn about unreachable code.

```ori
{
    panic(msg: "fail");
    let x = 42;         // warning: unreachable code
}
```

## 16.7 Conditional evaluation

### 16.7.1 If-then-else

The condition expression shall have type `bool`. Only the taken branch is evaluated.

With `else`: both branches shall have compatible types. The type of the `if` expression is the unified type of the two branches.

Without `else`: the then-branch shall have type `void` or `Never`. An `if` without `else` has type `void`.

```ori
// With else — expression producing a value
let x = if a > b then a else b;

// Without else — statement (void)
if debug then print(msg: "debug info");

// Chained else if
if x > 0 then "positive"
else if x < 0 then "negative"
else "zero"
```

NOTE  There is no `if let` syntax. Use `match` for destructuring conditionals.

### 16.7.2 Match

The scrutinee expression is evaluated exactly once. Arms are tested top-to-bottom. The body of the first matching arm is evaluated; no further arms are tested.

All arm bodies shall have compatible types. The type of the `match` expression is the unified type of all arm bodies. An arm with type `Never` is compatible with any other arm type.

The compiler shall verify that the match is _exhaustive_: every possible value of the scrutinee type is covered by at least one arm. A non-exhaustive match is a compile-time error.

Guards (`if`) are evaluated after the pattern matches. Guarded arms do not contribute to exhaustiveness; a catch-all pattern (`_` or binding) is required after guarded arms.

Unreachable arms (patterns that are subsets of earlier patterns) produce a compiler warning.

```ori
match value {
    Some(x) if x > 0 -> x,    // guard: only positive
    Some(x) -> -x,             // remaining Some values
    None -> 0,                 // exhaustive with this arm
}
```

### 16.7.3 Try blocks

A `try` block wraps an expression in error-handling context. The `?` operator inside a `try` block propagates to the `try` boundary rather than the enclosing function.

```ori
let result: Result<int, Error> = try {
    let $a = parse(input)?;
    let $b = validate($a)?;
    $a + $b
};
```

The type of a `try` block is `Result<T, E>` where `T` is the block's value type and `E` is the error type from `?` operations.

`break` and `continue` inside a `try` block target the enclosing loop (passing through the `try` boundary).

## 16.8 Short-circuit operators

Logical operators may skip evaluation of the right operand:

| Operator | Skips right when |
|----------|------------------|
| `&&` | Left is `false` |
| `\|\|` | Left is `true` |
| `??` | Left is not `None`/`Err` |

```ori
valid && expensive();   // expensive() skipped if valid is false
cached ?? compute();    // compute() skipped if cached is Some/Ok
```

## 16.9 Iteration protocol

A `for` expression desugars to calls on the `Iterable` and `Iterator` traits.

`for x in source do body` desugars to:

1. Call `source.iter()` to obtain an iterator (via `Iterable` trait)
2. Call `iterator.next()` to obtain `(Option<Item>, Iterator)`
3. If `Some(value)`: bind `x = value`, evaluate body, go to step 2
4. If `None`: stop

For `for x in source if guard do body`, the guard is evaluated after `x` is bound. If the guard evaluates to `false`, the iteration skips the body (implicit `continue`).

For `for x in source yield expr`:

- Each iteration appends the value of `expr` to an accumulating list
- The result type is `[T]` where `T` is the type of `expr`
- An empty source produces an empty list `[]`
- `break` stops iteration and returns the accumulated values so far
- `break value` appends `value` and returns
- `continue` skips this element (nothing appended)
- `continue value` appends `value` instead of the normal yield expression

Nested `for...yield` composes as flat-map:

```ori
for x in xs
for y in ys
yield (x, y)
// Equivalent to: xs.flat_map(x -> ys.map(y -> (x, y)))
```

### 16.9.1 For producing maps

When a `for...yield` expression yields tuples of `(K, V)` and the target type is `{K: V}`, the result is a map:

```ori
let m: {str: int} = for item in items yield (item.name, item.count);
```

## 16.10 Break and continue summary

| Form | Valid in | Effect |
|------|---------|--------|
| `break` | `loop`, `while...do`, `for...do`, `for...yield` | Exit loop |
| `break value` | `loop`, `for...yield` | Exit with value |
| `break:label` | Labeled `loop`, `while`, `for`, `block` | Exit labeled construct |
| `break:label value` | Labeled `loop`, `for...yield`, `block` | Exit labeled with value |
| `continue` | `loop`, `while...do`, `for...do`, `for...yield` | Next iteration |
| `continue value` | `for...yield` | Substitute value |
| `continue:label` | Labeled `loop`, `while`, `for` | Continue labeled loop |
| `continue:label value` | Labeled `for...yield` | Substitute in labeled yield |

The following uses are compile-time errors:

- `break` or `continue` outside any loop: error
- `break value` in `for...do` or `while...do`: error (E0860) — these forms have type `void`
- `continue value` in `loop` or `while`: error (E0861) — these loops do not accumulate values
- `continue:label value` targeting a `for...do`: error (E0873)
- `continue:label` targeting a labeled `block`: error — blocks do not iterate
- Reference to undefined label: error
- Label shadowing: error (E0871)

# Proposal: If Expression

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Approved:** 2026-01-31
**Affects:** Compiler, expressions, type inference

---

## Summary

This proposal formalizes the `if...then...else` conditional expression syntax, including type compatibility rules, branch evaluation, and interaction with the `Never` type.

---

## Problem Statement

The spec documents `if...then...else` syntax but leaves unclear:

1. **Else-if chains**: Is `else if` a special construct or just composition?
2. **Type compatibility**: How are branch types unified?
3. **Never coercion**: How does `Never` interact with branch types?
4. **Struct literal ambiguity**: Why are struct literals restricted in conditions?
5. **Parentheses**: When are parentheses required?

---

## Syntax

### Basic Form

```ori
if condition then expression else expression
```

### Without Else

```ori
if condition then expression
```

When `else` is omitted, the expression has type `void`.

### Else-If Chains

```ori
if condition1 then expression1
else if condition2 then expression2
else if condition3 then expression3
else expression4
```

The grammar treats `else if` as a single production for parsing convenience, but semantically the `else` branch contains another `if` expression. This distinction affects only parsing and error recovery; the evaluation model is recursive composition.

---

## Semantics

### Condition Type

The condition must have type `bool`. It is a compile-time error if the condition has any other type.

```ori
if x > 0 then "positive" else "non-positive"  // OK: x > 0 is bool
if x then "truthy" else "falsy"               // ERROR: x must be bool
```

### Branch Evaluation

Only one branch is evaluated at runtime. The unevaluated branch does not execute:

```ori
if true then compute_a() else compute_b()
// Only compute_a() is called
```

This is guaranteed and observable (side effects in the unevaluated branch do not occur).

### Type Unification

When `else` is present, both branches must produce types that unify to a common type.

**Same types:**

```ori
if cond then 1 else 2  // type: int
if cond then "a" else "b"  // type: str
```

**Compatible types:**

```ori
if cond then Some(1) else None  // type: Option<int>
if cond then Ok(1) else Err("fail")  // type: Result<int, str>
```

**Incompatible types (error):**

```ori
if cond then 1 else "two"  // ERROR: cannot unify int and str
```

### Without Else Branch

When `else` is omitted, the `then` branch must have type `void` (or `Never`):

```ori
// Valid: then-branch is void
if debug then print(msg: "debug mode")

// Valid: then-branch is Never (coerces to void)
if !valid then panic(msg: "invalid state")

// Invalid: then-branch has non-void type
if x > 0 then "positive"  // ERROR: non-void then-branch requires else
```

The overall expression has type `void`. When the `then` branch has type `Never`, it coerces to `void`.

### Never Type Coercion

The `Never` type (from `panic`, `break`, `continue`, `?` propagation) coerces to any type:

```ori
let x: int = if condition then 42 else panic(msg: "unreachable")
// else branch is Never, coerces to int
```

If both branches are `Never`, the expression has type `Never`:

```ori
let x = if a then panic(msg: "a") else panic(msg: "b")
// type: Never
```

---

## Struct Literal Restriction

Struct literals are not permitted directly in the condition position. This prevents parsing ambiguity:

```ori
// Ambiguous without restriction:
if Point { x: 0, y: 0 } then ...
//   ^^^^^^^^^^^^^^^^^^ Is this a struct literal or block?

// Use parentheses:
if (Point { x: 0, y: 0 }) then ...  // OK
if (create_point()) then ...         // OK (if returns bool)
```

The parser disables struct literal parsing in the condition context. Parenthesized expressions re-enable it.

---

## Nesting

Conditionals can be nested in any branch:

```ori
if a then
    if b then "both"
    else "only a"
else
    if b then "only b"
    else "neither"
```

Parentheses can clarify intent but are not required:

```ori
if a then (if b then x else y) else z
```

---

## Interaction with Patterns

### In Run Pattern

```ori
run(
    let value = compute(),
    if value > threshold then Ok(value) else Err("too low"),
)
```

### In Match Arms

```ori
match(option,
    Some(x) -> if x > 0 then x else 0,
    None -> 0,
)
```

---

## Expression Context

`if...then...else` is an expression, not a statement. It produces a value:

```ori
let sign = if x > 0 then 1 else if x < 0 then -1 else 0

@max (a: int, b: int) -> int = if a > b then a else b

// Ori uses for-yield, not list comprehension syntax:
for x in numbers yield if x > 0 then x else 0
```

---

## Error Messages

### Non-Boolean Condition

```
error[E0201]: condition must be `bool`
  --> src/main.ori:5:4
   |
 5 | if x then y else z
   |    ^ expected `bool`, found `int`
   |
   = help: use a comparison: `x > 0`, `x != 0`
```

### Missing Else for Non-Void

```
error[E0202]: `if` without `else` requires `void` then-branch
  --> src/main.ori:5:1
   |
 5 | if condition then "value"
   | ^^^^^^^^^^^^^^^^^^^^^^^^^ then-branch has type `str`
   |
   = note: without `else`, the expression must produce `void`
   = help: add an `else` branch or change the then-branch to return `void`
```

### Type Mismatch

```
error[E0203]: mismatched types in `if` branches
  --> src/main.ori:5:1
   |
 5 | if cond then 1 else "two"
   |              -      ^^^^^ expected `int`, found `str`
   |              |
   |              then-branch has type `int`
   |
   = note: both branches must have compatible types
```

### Struct Literal in Condition

```
error[E0204]: struct literal not allowed in `if` condition
  --> src/main.ori:5:4
   |
 5 | if Point { x: 0 } then ...
   |    ^^^^^^^^^^^^^^ struct literal here
   |
   = help: wrap in parentheses: `if (Point { x: 0 }) then ...`
```

---

## Examples

### Basic Conditional

```ori
@sign (x: int) -> int = if x > 0 then 1 else if x < 0 then -1 else 0
```

### Guard with Side Effects

```ori
@process (item: Item) -> void =
    if item.needs_validation then validate(item)
```

### Never Coercion

```ori
@unwrap_or_panic<T> (opt: Option<T>, msg: str) -> T =
    if is_some(opt) then opt.unwrap() else panic(msg: msg)
```

### Nested Conditionals

```ori
@classify (x: int, y: int) -> str =
    if x > 0 then
        if y > 0 then "quadrant 1"
        else if y < 0 then "quadrant 4"
        else "positive x-axis"
    else if x < 0 then
        if y > 0 then "quadrant 2"
        else if y < 0 then "quadrant 3"
        else "negative x-axis"
    else
        if y > 0 then "positive y-axis"
        else if y < 0 then "negative y-axis"
        else "origin"
```

---

## Grammar

The grammar is defined in `grammar.ebnf` under the `if_expr` production:

```
if_expr = "if" expression "then" expression
          { "else" "if" expression "then" expression }
          [ "else" expression ] .
```

The grammar treats `else if` as a single production for parsing convenience. The condition expression excludes struct literals (handled by parse context).

---

## Spec Changes Applied

The following spec files were updated upon approval:

- `09-expressions.md`: Expanded Conditional section with struct literal restriction, Never coercion, else-if clarification
- `CLAUDE.md`: Verified consistent (already documents if...then...else semantics)

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Syntax | `if cond then expr [else expr]` |
| Condition type | `bool` required |
| Branch types | Must unify (or use `Never`) |
| Without else | `then` must be `void` or `Never`; result is `void` |
| Never coercion | `Never` coerces to any type |
| Evaluation | Only taken branch evaluates |
| Struct literals | Not allowed in condition (use parentheses) |
| Else-if | Grammar convenience; semantically nested |

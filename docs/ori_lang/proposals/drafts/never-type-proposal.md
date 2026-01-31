# Proposal: Never Type Semantics

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, control flow

---

## Summary

This proposal formalizes the `Never` type semantics, including its role as a bottom type, coercion rules, and use cases.

---

## Problem Statement

The spec mentions `Never` as "uninhabited" but leaves unclear:

1. **Coercion**: How does `Never` coerce to other types?
2. **Contexts**: Where can `Never` appear?
3. **Inference**: How does type inference handle `Never`?
4. **Functions**: What does returning `Never` mean?
5. **Expressions**: Which expressions have type `Never`?

---

## Definition

`Never` is the _bottom type_ — a type with no values. It represents computations that never complete normally.

---

## Uninhabited

No value has type `Never`:

```ori
let x: Never = ???  // No valid expression
```

This makes `Never` useful for:
- Functions that never return
- Match arms that never execute
- Unreachable code paths

---

## Coercion Rules

### Coerces to Any Type

`Never` coerces to any type `T`:

```ori
let x: int = panic(msg: "unreachable")  // panic returns Never, coerces to int
let y: str = unreachable()              // unreachable returns Never, coerces to str
```

### Rationale

Since `Never` has no values, the coercion never actually executes — the expression diverges before producing a value.

### Coercion Contexts

`Never` coerces in:
- Assignment: `let x: T = never_expr`
- Return: `-> T { ... never_expr }`
- Conditionals: `if c then expr else never_expr`
- Match arms: `Some(x) -> x, None -> panic(...)`

---

## Expressions with Type Never

### Panic

```ori
panic(msg: "error")  // -> Never
```

### Todo

```ori
todo()                    // -> Never
todo(reason: "not implemented")  // -> Never
```

### Unreachable

```ori
unreachable()                     // -> Never
unreachable(reason: "impossible") // -> Never
```

### Break and Continue

Inside loops, `break` and `continue` have type `Never`:

```ori
loop(
    if done then break,  // break: Never
    process(),
)
```

### Error Propagation

When `?` causes early return:

```ori
let x = fallible()?  // If Err, ? has type Never (returns early)
```

### Infinite Loops

A loop with no `break` has type `Never`:

```ori
let forever: Never = loop(
    process_events(),  // No break, never terminates
)
```

---

## Function Return Type

### Diverging Functions

Functions returning `Never` never return normally:

```ori
@fail (msg: str) -> Never = panic(msg: msg)

@infinite_loop () -> Never = loop(
    process(),
)
```

### Calling Diverging Functions

```ori
@process (x: Option<int>) -> int = match(x,
    Some(v) -> v,
    None -> fail(msg: "expected value"),  // Never coerces to int
)
```

---

## Type Inference

### Bottom-Up Propagation

In conditionals, `Never` doesn't constrain the result type:

```ori
let x = if condition then 42 else panic(msg: "fail")
// Type: int (Never coerces to int)
```

### Match Exhaustiveness

`Never` arms don't affect the result type:

```ori
let result = match(opt,
    Some(v) -> v,        // int
    None -> panic(...),  // Never coerces to int
)
// result: int
```

### Multiple Never Paths

If all paths return `Never`, the expression has type `Never`:

```ori
let x = if condition then panic(msg: "a") else panic(msg: "b")
// x: Never
```

---

## Generic Contexts

### Never as Type Argument

`Never` can be a type argument:

```ori
let empty: Result<Never, str> = Err("always error")
```

This represents a `Result` that can never be `Ok`.

### Inference with Never

```ori
let opt: Option<Never> = None  // Can never be Some
```

---

## Pattern Matching

### Exhaustiveness

`Never` variants need not be matched:

```ori
type MaybeNever = Value(int) | Impossible(Never)

let x = match(maybe,
    Value(v) -> v,
    // Impossible case can be omitted — it can never occur
)
```

### With Explicit Match

Matching `Never` is allowed but the arm is unreachable:

```ori
match(maybe,
    Value(v) -> v,
    Impossible(n) -> match(n, ),  // Empty match on Never
)
```

---

## Common Patterns

### Assertion Helper

```ori
@assert_some<T> (opt: Option<T>) -> T = match(opt,
    Some(v) -> v,
    None -> panic(msg: "expected Some"),  // Never coerces to T
)
```

### Placeholder Implementation

```ori
@complex_algorithm (data: Data) -> Result<Output, Error> =
    todo(reason: "implement algorithm")  // Never coerces to Result
```

### Unreachable Branches

```ori
@process (status: Status) -> str = match(status,
    Active -> "running",
    Paused -> "paused",
    // Completed and Failed handled elsewhere
    _ -> unreachable(reason: "invalid status"),
)
```

---

## Interaction with Other Types

### Never and Void

| Type | Values | Use |
|------|--------|-----|
| `void` | One value: `()` | No meaningful return |
| `Never` | No values | Never returns |

```ori
@log (msg: str) -> void = print(msg: msg)  // Returns ()
@fail (msg: str) -> Never = panic(msg: msg) // Never returns
```

### Never and Option

```ori
Option<Never>  // Can only be None
Result<Never, E>  // Can only be Err
Result<T, Never>  // Can only be Ok
```

---

## Error Messages

### Unreachable Code Warning

```
warning[W0200]: unreachable code
  --> src/main.ori:6:5
   |
 5 |     panic(msg: "fail")
   |     ------------------ diverges here
 6 |     process()
   |     ^^^^^^^^^ this code is unreachable
```

### Never in Invalid Context

```
error[E0920]: cannot use `Never` as struct field type
  --> src/types.ori:2:10
   |
 2 |     field: Never,
   |            ^^^^^ uninhabited type
   |
   = note: structs with `Never` fields cannot be constructed
```

---

## Spec Changes Required

### Update `06-types.md`

Expand Never section with:
1. Definition as bottom type
2. Coercion rules
3. Expressions that produce Never
4. Generic type argument usage

### Update `09-expressions.md`

Document Never-producing expressions.

### Update `19-control-flow.md`

Document break/continue as Never type.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Definition | Uninhabited bottom type |
| Values | None |
| Coerces to | Any type T |
| Produced by | panic, todo, unreachable, break, continue, infinite loop |
| Function return | Indicates function never returns normally |
| Generic usage | `Option<Never>` = always None, `Result<Never, E>` = always Err |
| Match exhaustiveness | Never variants can be omitted |

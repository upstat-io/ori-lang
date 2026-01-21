# Type Narrowing

This document covers Sigil's flow-sensitive typing: how the compiler tracks more specific types after type checks, narrowing functions, scope rules, and how assertions affect types.

---

## Overview

Type narrowing is flow-sensitive typing where the compiler tracks more specific types after conditions are checked. After you verify that an `Option` contains a value, the compiler knows it's `Some`, not just `Option`.

```sigil
@process (opt: Option<int>) -> int = run(
    if is_some(opt) then
        // opt narrowed to Some(int) here
        unwrap(opt)  // safe, compiler knows it's Some
    else
        0
)
```

---

## Basic Type Narrowing

### Option Narrowing

```sigil
@get_value (opt: Option<int>) -> int = run(
    if is_some(opt) then
        // Type of opt is narrowed from Option<int> to Some(int)
        unwrap(opt)  // guaranteed safe
    else
        // Type of opt is None here
        0
)
```

### Result Narrowing

```sigil
@handle (r: Result<Data, Error>) -> str = run(
    if is_ok(r) then
        // r is Ok(Data)
        unwrap(r).to_string()
    else
        // r is Err(Error)
        unwrap_err(r).message
)
```

### Sum Type Narrowing

```sigil
type Status = Pending | Running(int) | Done | Failed(str)

@describe (s: Status) -> str = run(
    if is_variant(s, Running) then run(
        // s is Running(int)
        let progress = as_variant(s, Running),
        "at " + str(progress) + "%",
    )
    else if is_variant(s, Failed) then run(
        // s is Failed(str)
        let msg = as_variant(s, Failed),
        "error: " + msg,
    )
    else
        // s is Pending or Done
        "waiting or complete",
)
```

---

## Narrowing Functions

Specific functions trigger type narrowing. These are recognized by the compiler.

### Option Functions

| Function | Narrows to |
|----------|------------|
| `is_some(opt)` | `Some(T)` in true branch |
| `is_none(opt)` | `None` in true branch |

```sigil
@process (opt: Option<str>) -> str = run(
    if is_none(opt) then
        "missing"
    else
        // opt is Some(str)
        unwrap(opt)
)
```

### Result Functions

| Function | Narrows to |
|----------|------------|
| `is_ok(result)` | `Ok(T)` in true branch |
| `is_err(result)` | `Err(E)` in true branch |

```sigil
@handle (r: Result<int, str>) -> int = run(
    if is_err(r) then
        print("Error: " + unwrap_err(r)),
        -1
    else
        // r is Ok(int)
        unwrap(r) * 2
)
```

### Variant Check Functions

| Function | Narrows to |
|----------|------------|
| `is_variant(val, Variant)` | That specific variant |

```sigil
type Shape = Circle(float) | Rectangle(float, float) | Triangle(float, float, float)

@area (s: Shape) -> float = run(
    if is_variant(s, Circle) then run(
        let r = as_variant(s, Circle),
        3.14159 * r * r,
    )
    else if is_variant(s, Rectangle) then run(
        let (w, h) = as_variant(s, Rectangle),
        w * h,
    )
    else run(
        // s must be Triangle
        let (a, b, c) = as_variant(s, Triangle),
        // Heron's formula
        let sp = (a + b + c) / 2.0,
        sqrt(sp * (sp - a) * (sp - b) * (sp - c)),
    ),
)
```

---

## Match is Preferred

While `is_variant` and `as_variant` work, `match` is cleaner and provides automatic narrowing:

```sigil
// Verbose: if-chain with narrowing functions
@describe_if (s: Status) -> str = run(
    if is_variant(s, Pending) then "waiting"
    else if is_variant(s, Running) then run(
        let p = as_variant(s, Running),
        "at " + str(p) + "%",
    )
    else if is_variant(s, Done) then "complete"
    else run(
        let msg = as_variant(s, Failed),
        "error: " + msg,
    ),
)

// Preferred: match with automatic narrowing
@describe_match (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running(p) -> "at " + str(p) + "%",
    Done -> "complete",
    Failed(msg) -> "error: " + msg
)
```

**Use narrowing functions when:**
- Complex control flow doesn't fit match
- Need to check multiple values
- Building conditional pipelines

**Use match when:**
- Simple dispatch on variants
- All cases can be handled with patterns
- Destructuring needed

---

## Narrowing Scope

Type narrowing applies within specific scopes based on control flow.

### Branch-Local Narrowing

Narrowing applies only within the conditional branch:

```sigil
@process (opt: Option<int>) -> int = run(
    if is_some(opt) then
        unwrap(opt)  // safe: opt is Some here
    else
        0,

    // After if-else: opt is back to Option<int>
    // unwrap(opt)  // would be ERROR
    0
)
```

### Match-Based Narrowing

Pattern matching provides type narrowing within each arm:

```sigil
@process (opt: Option<int>) -> int = match(opt,
    Some(n) -> n * 2,    // n is int, narrowed from Option
    None -> 0
)

@validate (r: Result<Data, Error>) -> Data = match(r,
    Ok(data) -> data,    // data is Data, narrowed from Result
    Err(e) -> panic("validation failed: " + e.message)
)
```

The narrowing is implicit in destructuring—when you match `Some(n)`, you have direct access to the unwrapped value.

### Nested Scope

Inner scopes inherit narrowing:

```sigil
@process (opt: Option<int>) -> int = run(
    if is_some(opt) then
        run(
            // Still narrowed in nested run
            let value = unwrap(opt),
            value * 2,
        )
    else
        0,
)
```

---

## Narrowing with Boolean Bindings

Narrowing works with bound boolean results:

```sigil
@process (opt: Option<int>) -> int = run(
    let has_value = is_some(opt),

    if has_value then
        unwrap(opt)  // narrowed based on has_value
    else
        0,
)
```

### Limitation: Computed Booleans

Complex boolean expressions don't narrow:

```sigil
@process (opt: Option<int>) -> int = run(
    // Computed boolean: narrowing lost
    let check = is_some(opt) && some_other_condition(),

    if check then
        unwrap(opt)  // ERROR: opt not narrowed
    else
        0,
)

// Fix: separate the checks
@process_fixed (opt: Option<int>) -> int = run(
    if is_some(opt) && some_other_condition() then
        unwrap(opt)  // narrowed: direct use in condition
    else
        0,
)
```

---

## No Implicit Truthiness

Sigil requires explicit type checks. Values don't implicitly convert to boolean.

```sigil
// ERROR: Option is not a boolean
@bad (opt: Option<int>) -> int = run(
    if opt then      // ERROR: type mismatch
        unwrap(opt)
    else
        0
)

// Correct: explicit check
@good (opt: Option<int>) -> int = run(
    if is_some(opt) then
        unwrap(opt)
    else
        0
)
```

This applies to all types:

```sigil
// ERROR: int is not boolean
if count then ...        // ERROR

// Correct
if count > 0 then ...    // OK
if count != 0 then ...   // OK
```

---

## Narrowing in Logical Operators

The `&&` operator propagates narrowing through short-circuit evaluation.

### And (&&) Propagation

```sigil
@process (a: Option<int>, b: Option<int>) -> int = run(
    if is_some(a) && is_some(b) then
        // Both narrowed to Some
        unwrap(a) + unwrap(b)
    else
        0
)
```

The left condition is evaluated first. If true, the right condition is evaluated with the left's narrowing in effect:

```sigil
@chain (opt: Option<int>) -> int = run(
    // unwrap(opt) in second part is safe because is_some(opt) passed
    if is_some(opt) && unwrap(opt) > 0 then
        unwrap(opt) * 2
    else
        0
)
```

### Or (||) Does Not Propagate

With `||`, either branch might execute, so no narrowing carries:

```sigil
@process (opt: Option<int>) -> int = run(
    // No narrowing: might take either branch
    if is_none(opt) || unwrap(opt) < 0 then  // ERROR: opt not narrowed
        0
    else
        unwrap(opt)
)

// Fix: restructure
@process_fixed (opt: Option<int>) -> int = run(
    if is_none(opt) then
        0
    else if unwrap(opt) < 0 then  // safe: opt is Some
        0
    else
        unwrap(opt)
)
```

---

## Assert Narrows

`assert` narrows types for the rest of the scope:

```sigil
@process (opt: Option<int>) -> int = run(
    assert(is_some(opt), "value required"),

    // opt narrowed to Some for rest of function
    unwrap(opt) * 2
)
```

### Multiple Asserts

```sigil
@calculate (a: Option<int>, b: Option<int>) -> int = run(
    assert(is_some(a), "a required"),
    assert(is_some(b), "b required"),

    // Both narrowed
    unwrap(a) + unwrap(b)
)
```

### Assert with Results

```sigil
@validate (r: Result<Data, Error>) -> Data = run(
    assert(is_ok(r), "must succeed"),

    // r is Ok(Data)
    unwrap(r)
)
```

### Assert with Variants

```sigil
@get_progress (s: Status) -> int = run(
    assert(is_variant(s, Running), "must be running"),

    // s is Running(int)
    as_variant(s, Running)
)
```

---

## Type Narrowing with Custom Types

User-defined sum types narrow via `is_variant` and `as_variant`:

```sigil
type Tree<T> = Leaf(T) | Node(left: Tree<T>, right: Tree<T>)

@sum_tree (t: Tree<int>) -> int = run(
    if is_variant(t, Leaf) then
        as_variant(t, Leaf)
    else run(
        let (left, right) = as_variant(t, Node),
        sum_tree(left) + sum_tree(right),
    ),
)
```

### Nested Type Narrowing

```sigil
type Container = { value: Option<Result<int, str>> }

@extract (c: Container) -> int = run(
    if is_some(c.value) then run(
        let inner = unwrap(c.value),
        if is_ok(inner) then
            unwrap(inner)
        else
            0,
    )
    else
        -1,
)
```

---

## Narrowing in Loops

Narrowing in loops requires care because the type might change between iterations.

### Loop-Local Narrowing

```sigil
@process_all (items: [Option<int>]) -> [int] = run(
    let mut results = [],
    for item in items do
        if is_some(item) then
            // item narrowed in this iteration
            results = results + [unwrap(item)],
    results,
)
```

### Early Exit with Narrowing

```sigil
@find_first (items: [Option<int>]) -> Option<int> = for(
    .over: items,
    .map: item -> item,  // identity - we want the Option itself
    .match: Some(n) -> true,  // match any Some
    .default: None
)
```

---

## Narrowing Limitations

### Function Calls Don't Narrow

```sigil
@check_value (opt: Option<int>) -> bool = is_some(opt)

@process (opt: Option<int>) -> int = run(
    // Function call result doesn't narrow
    if check_value(opt) then
        unwrap(opt)  // ERROR: opt not narrowed
    else
        0
)
```

**Why:** The compiler can't know what `check_value` actually checks.

**Fix:** Use direct narrowing functions:

```sigil
@process (opt: Option<int>) -> int = run(
    if opt.is_some() then
        opt.unwrap()  // OK: direct narrowing method
    else
        0
)
```

### Shadowing Invalidates Narrowing

```sigil
@process (opt: Option<int>) -> int = run(
    if opt.is_some() then run(
        let opt = get_new_option(),  // shadows with new value
        opt.unwrap(),  // ERROR: narrowing lost, opt might be None
    )
    else
        0,
)
```

The shadow creates a new binding with `let`. The type checker cannot assume the new `opt` has the same narrowed type as the original.

### Field Access Doesn't Track

```sigil
type Wrapper = { value: Option<int> }

@process (w: Wrapper) -> int = run(
    if w.value.is_some() then
        w.value.unwrap()  // OK for now
    else
        0,

    // But if w could be modified...
    modify(w),
    // w.value.unwrap()  // would be dangerous
)
```

---

## Combining Narrowing Techniques

### Narrowing with Destructuring

```sigil
@process (pair: (Option<int>, Option<int>)) -> int = run(
    let (a, b) = pair,
    if a.is_some() && b.is_some() then
        a.unwrap() + b.unwrap()
    else
        0,
)
```

### Narrowing with Guards in Match

```sigil
@process (opt: Option<int>) -> str = match(opt,
    Some(n).match(n > 0) -> "positive: " + str(n),
    Some(n).match(n < 0) -> "negative: " + str(n),
    Some(_) -> "zero",
    None -> "nothing"
)
```

---

## Best Practices

### Prefer Match Over Narrowing

```sigil
// Verbose: explicit narrowing
@describe (opt: Option<User>) -> str = run(
    if is_some(opt) then run(
        let user = unwrap(opt),
        user.name,
    )
    else
        "anonymous",
)

// Better: match handles it
@describe (opt: Option<User>) -> str = match(opt,
    Some(user) -> user.name,
    None -> "anonymous"
)
```

### Use Early Return for Validation

```sigil
@process (input: Input) -> Result<Output, Error> = run(
    if is_none(input.required_field) then
        return Err(MissingField { name: "required_field" }),

    if is_err(validate(input)) then
        return Err(ValidationFailed {}),

    // All checks passed, types are narrowed
    Ok(transform(input))
)
```

### Group Related Checks

```sigil
@validate_user (u: User) -> Result<User, [str]> = run(
    let mut errors = [],

    if is_none(u.email) then
        errors = errors + ["email required"],

    if is_none(u.name) then
        errors = errors + ["name required"],

    if len(errors) > 0 then
        Err(errors)
    else
        Ok(u),
)
```

---

## Error Messages

### Narrowing Not Applied

```
error[E0500]: cannot unwrap Option that may be None
  |
5 | unwrap(opt)
  | ^^^^^^^^^^^ opt has type Option<int>, not Some<int>
  |
help: check with is_some() first:
  | if is_some(opt) then unwrap(opt) else default
```

### Narrowing Lost After Reassignment

```
error[E0501]: narrowing invalidated by reassignment
  |
5 | if is_some(opt) then
6 |     opt = compute(),
7 |     unwrap(opt)
  |     ^^^^^^^^^^^ opt was reassigned, narrowing no longer valid
```

### Computed Boolean No Narrowing

```
warning[W0502]: computed boolean does not enable narrowing
  |
5 | valid = is_some(a) && is_some(b)
6 | if valid then
7 |     unwrap(a)  // ERROR
  |
help: use the condition directly:
  | if is_some(a) && is_some(b) then
```

---

## Summary

| Feature | Behavior |
|---------|----------|
| `is_some(opt)` | Narrows to `Some(T)` in true branch |
| `is_none(opt)` | Narrows to `None` in true branch |
| `is_ok(r)` | Narrows to `Ok(T)` in true branch |
| `is_err(r)` | Narrows to `Err(E)` in true branch |
| `is_variant(v, X)` | Narrows to variant `X` in true branch |
| Early return | Narrows for rest of scope |
| `assert(check)` | Narrows for rest of scope |
| `&&` | Propagates narrowing left-to-right |
| `\|\|` | No narrowing propagation |
| Match | Auto-narrows in each arm |

---

## See Also

- [Match Pattern](01-match-pattern.md) — Pattern matching with automatic narrowing
- [Guards and Bindings](03-guards-and-bindings.md) — Conditional patterns
- [Error Handling](../05-error-handling/index.md) — Result and Option types
- [Type System](../03-type-system/index.md) — Sum types and generics

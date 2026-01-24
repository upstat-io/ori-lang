# Type Narrowing

This document covers Sigil's flow-sensitive typing: how the compiler tracks more specific types after type checks, narrowing functions, scope rules, and how assertions affect types.

---

## Overview

Type narrowing is flow-sensitive typing where the compiler tracks more specific types after conditions are checked. After you verify that an `Option` contains a value, the compiler knows it's `Some`, not just `Option`.

```sigil
@process (opt: Option<int>) -> int = run(
    if is_some(.opt: opt) then
        // opt narrowed to Some(int) here
        // safe, compiler knows it's Some
        unwrap(.opt: opt)
    else
        0,
)
```

---

## Basic Type Narrowing

### Option Narrowing

```sigil
@get_value (opt: Option<int>) -> int = run(
    if is_some(.opt: opt) then
        // Type of opt is narrowed from Option<int> to Some(int)
        // guaranteed safe
        unwrap(.opt: opt)
    else
        // Type of opt is None here
        0,
)
```

### Result Narrowing

```sigil
@handle (result: Result<Data, Error>) -> str = run(
    if is_ok(.result: result) then
        // result is Ok(Data)
        unwrap(.result: result).to_string()
    else
        // result is Err(Error)
        unwrap_err(.result: result).message,
)
```

### Sum Type Narrowing

```sigil
type Status = Pending | Running(int) | Done | Failed(str)

@describe (status: Status) -> str = run(
    if is_variant(
        .value: status,
        .variant: Running,
    ) then run(
        // status is Running(int)
        let progress = as_variant(
            .value: status,
            .variant: Running,
        ),
        "at " + str(.value: progress) + "%",
    )
    else if is_variant(
        .value: status,
        .variant: Failed,
    ) then run(
        // status is Failed(str)
        let msg = as_variant(
            .value: status,
            .variant: Failed,
        ),
        "error: " + msg,
    )
    else
        // status is Pending or Done
        "waiting or complete",
)
```

---

## Narrowing Functions

Specific functions trigger type narrowing. These are recognized by the compiler.

### Option Functions

| Function | Narrows to |
|----------|------------|
| `is_some(.opt: opt)` | `Some(T)` in true branch |
| `is_none(.opt: opt)` | `None` in true branch |

```sigil
@process (opt: Option<str>) -> str = run(
    if is_none(.opt: opt) then
        "missing"
    else
        // opt is Some(str)
        unwrap(.opt: opt),
)
```

### Result Functions

| Function | Narrows to |
|----------|------------|
| `is_ok(.result: result)` | `Ok(T)` in true branch |
| `is_err(.result: result)` | `Err(E)` in true branch |

```sigil
@handle (result: Result<int, str>) -> int = run(
    if is_err(.result: result) then
        print(.message: "Error: " + unwrap_err(.result: result)),
        -1
    else
        // result is Ok(int)
        unwrap(.result: result) * 2,
)
```

### Variant Check Functions

| Function | Narrows to |
|----------|------------|
| `is_variant(.value: val, .variant: Variant)` | That specific variant |

```sigil
type Shape = Circle(float) | Rectangle(float, float) | Triangle(float, float, float)

@area (shape: Shape) -> float = run(
    if is_variant(
        .value: shape,
        .variant: Circle,
    ) then run(
        let radius = as_variant(
            .value: shape,
            .variant: Circle,
        ),
        3.14159 * radius * radius,
    )
    else if is_variant(
        .value: shape,
        .variant: Rectangle,
    ) then run(
        let (width, height) = as_variant(
            .value: shape,
            .variant: Rectangle,
        ),
        width * height,
    )
    else run(
        // shape must be Triangle
        let (side_a, side_b, side_c) = as_variant(
            .value: shape,
            .variant: Triangle,
        ),
        // Heron's formula
        let semi_perimeter = (side_a + side_b + side_c) / 2.0,
        sqrt(
            .value: semi_perimeter * (semi_perimeter - side_a) * (semi_perimeter - side_b) * (semi_perimeter - side_c),
        ),
    ),
)
```

---

## Match is Preferred

While `is_variant` and `as_variant` work, `match` is cleaner and provides automatic narrowing:

```sigil
// Verbose: if-chain with narrowing functions
@describe_if (status: Status) -> str = run(
    if is_variant(
        .value: status,
        .variant: Pending,
    ) then "waiting"
    else if is_variant(
        .value: status,
        .variant: Running,
    ) then run(
        let progress = as_variant(
            .value: status,
            .variant: Running,
        ),
        "at " + str(.value: progress) + "%",
    )
    else if is_variant(
        .value: status,
        .variant: Done,
    ) then "complete"
    else run(
        let msg = as_variant(
            .value: status,
            .variant: Failed,
        ),
        "error: " + msg,
    ),
)

// Preferred: match with automatic narrowing
@describe_match (status: Status) -> str = match(
    status,
    Pending -> "waiting",
    Running(progress) -> "at " + str(.value: progress) + "%",
    Done -> "complete",
    Failed(msg) -> "error: " + msg,
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
    if is_some(.opt: opt) then
        // safe: opt is Some here
        unwrap(.opt: opt)
    else
        0,

    // After if-else: opt is back to Option<int>
    // unwrap(.opt: opt)  // would be ERROR
    0,
)
```

### Match-Based Narrowing

Pattern matching provides type narrowing within each arm:

```sigil
@process (opt: Option<int>) -> int = match(
    opt,
    // value is int, narrowed from Option
    Some(value) -> value * 2,
    None -> 0,
)

@validate (result: Result<Data, Error>) -> Data = match(
    result,
    // data is Data, narrowed from Result
    Ok(data) -> data,
    Err(err) -> panic(.message: "validation failed: " + err.message),
)
```

The narrowing is implicit in destructuring—when you match `Some(n)`, you have direct access to the unwrapped value.

### Nested Scope

Inner scopes inherit narrowing:

```sigil
@process (opt: Option<int>) -> int = run(
    if is_some(.opt: opt) then
        run(
            // Still narrowed in nested run
            let value = unwrap(.opt: opt),
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
    let has_value = is_some(.opt: opt),

    if has_value then
        // narrowed based on has_value
        unwrap(.opt: opt)
    else
        0,
)
```

### Limitation: Computed Booleans

Complex boolean expressions don't narrow:

```sigil
@process (opt: Option<int>) -> int = run(
    // Computed boolean: narrowing lost
    let check = is_some(.opt: opt) && some_other_condition(),

    if check then
        // ERROR: opt not narrowed
        unwrap(.opt: opt)
    else
        0,
)

// Fix: separate the checks
@process_fixed (opt: Option<int>) -> int = run(
    if is_some(.opt: opt) && some_other_condition() then
        // narrowed: direct use in condition
        unwrap(.opt: opt)
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
    // ERROR: type mismatch
    if opt then
        unwrap(.opt: opt)
    else
        0,
)

// Correct: explicit check
@good (opt: Option<int>) -> int = run(
    if is_some(.opt: opt) then
        unwrap(.opt: opt)
    else
        0,
)
```

This applies to all types:

```sigil
// ERROR: int is not boolean
// ERROR
if count then ...

// Correct
// OK
if count > 0 then ...
// OK
if count != 0 then ...
```

---

## Narrowing in Logical Operators

The `&&` operator propagates narrowing through short-circuit evaluation.

### And (&&) Propagation

```sigil
@process (opt_a: Option<int>, opt_b: Option<int>) -> int = run(
    if is_some(.opt: opt_a) && is_some(.opt: opt_b) then
        // Both narrowed to Some
        unwrap(.opt: opt_a) + unwrap(.opt: opt_b)
    else
        0,
)
```

The left condition is evaluated first. If true, the right condition is evaluated with the left's narrowing in effect:

```sigil
@chain (opt: Option<int>) -> int = run(
    // unwrap(.opt: opt) in second part is safe because is_some(.opt: opt) passed
    if is_some(.opt: opt) && unwrap(.opt: opt) > 0 then
        unwrap(.opt: opt) * 2
    else
        0,
)
```

### Or (||) Does Not Propagate

With `||`, either branch might execute, so no narrowing carries:

```sigil
@process (opt: Option<int>) -> int = run(
    // No narrowing: might take either branch
    // ERROR: opt not narrowed
    if is_none(.opt: opt) || unwrap(.opt: opt) < 0 then
        0
    else
        unwrap(.opt: opt),
)

// Fix: restructure
@process_fixed (opt: Option<int>) -> int = run(
    if is_none(.opt: opt) then
        0
    // safe: opt is Some
    else if unwrap(.opt: opt) < 0 then
        0
    else
        unwrap(.opt: opt),
)
```

---

## Assert Narrows

`assert` narrows types for the rest of the scope:

```sigil
@process (opt: Option<int>) -> int = run(
    assert(
        .condition: is_some(.opt: opt),
        .message: "value required",
    ),

    // opt narrowed to Some for rest of function
    unwrap(.opt: opt) * 2,
)
```

### Multiple Asserts

```sigil
@calculate (opt_a: Option<int>, opt_b: Option<int>) -> int = run(
    assert(
        .condition: is_some(.opt: opt_a),
        .message: "opt_a required",
    ),
    assert(
        .condition: is_some(.opt: opt_b),
        .message: "opt_b required",
    ),

    // Both narrowed
    unwrap(.opt: opt_a) + unwrap(.opt: opt_b),
)
```

### Assert with Results

```sigil
@validate (result: Result<Data, Error>) -> Data = run(
    assert(
        .condition: is_ok(.result: result),
        .message: "must succeed",
    ),

    // result is Ok(Data)
    unwrap(.result: result),
)
```

### Assert with Variants

```sigil
@get_progress (status: Status) -> int = run(
    assert(
        .condition: is_variant(
            .value: status,
            .variant: Running,
        ),
        .message: "must be running",
    ),

    // status is Running(int)
    as_variant(
        .value: status,
        .variant: Running,
    ),
)
```

---

## Type Narrowing with Custom Types

User-defined sum types narrow via `is_variant` and `as_variant`:

```sigil
type Tree<T> = Leaf(T) | Node(left: Tree<T>, right: Tree<T>)

@sum_tree (tree: Tree<int>) -> int = run(
    if is_variant(
        .value: tree,
        .variant: Leaf,
    ) then
        as_variant(
            .value: tree,
            .variant: Leaf,
        )
    else run(
        let (left, right) = as_variant(
            .value: tree,
            .variant: Node,
        ),
        sum_tree(.tree: left) + sum_tree(.tree: right),
    ),
)
```

### Nested Type Narrowing

```sigil
type Container = { value: Option<Result<int, str>> }

@extract (container: Container) -> int = run(
    if is_some(.opt: container.value) then run(
        let inner = unwrap(.opt: container.value),
        if is_ok(.result: inner) then
            unwrap(.result: inner)
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
        if is_some(.opt: item) then
            // item narrowed in this iteration
            results = results + [unwrap(.opt: item)],
    results,
)
```

### Early Exit with Narrowing

```sigil
@find_first (items: [Option<int>]) -> Option<int> = for(
    .over: items,
    // identity - we want the Option itself
    .map: item -> item,
    // match any Some
    .match: Some(value) -> true,
    .default: None,
)
```

---

## Narrowing Limitations

### Function Calls Don't Narrow

```sigil
@check_value (opt: Option<int>) -> bool = is_some(.opt: opt)

@process (opt: Option<int>) -> int = run(
    // Function call result doesn't narrow
    if check_value(.opt: opt) then
        // ERROR: opt not narrowed
        unwrap(.opt: opt)
    else
        0,
)
```

**Why:** The compiler can't know what `check_value` actually checks.

**Fix:** Use direct narrowing functions:

```sigil
@process (opt: Option<int>) -> int = run(
    if opt.is_some() then
        // OK: direct narrowing method
        opt.unwrap()
    else
        0,
)
```

### Shadowing Invalidates Narrowing

```sigil
@process (opt: Option<int>) -> int = run(
    if opt.is_some() then run(
        // shadows with new value
        let opt = get_new_option(),
        // ERROR: narrowing lost, opt might be None
        opt.unwrap(),
    )
    else
        0,
)
```

The shadow creates a new binding with `let`. The type checker cannot assume the new `opt` has the same narrowed type as the original.

### Field Access Doesn't Track

```sigil
type Wrapper = { value: Option<int> }

@process (wrapper: Wrapper) -> int = run(
    if wrapper.value.is_some() then
        // OK for now
        wrapper.value.unwrap()
    else
        0,

    // But if wrapper could be modified...
    modify(.wrapper: wrapper),
    // wrapper.value.unwrap()  // would be dangerous
    0,
)
```

---

## Combining Narrowing Techniques

### Narrowing with Destructuring

```sigil
@process (pair: (Option<int>, Option<int>)) -> int = run(
    let (opt_a, opt_b) = pair,
    if opt_a.is_some() && opt_b.is_some() then
        opt_a.unwrap() + opt_b.unwrap()
    else
        0,
)
```

### Narrowing with Guards in Match

```sigil
@process (opt: Option<int>) -> str = match(
    opt,
    Some(value).match(value > 0) -> "positive: " + str(.value: value),
    Some(value).match(value < 0) -> "negative: " + str(.value: value),
    Some(_) -> "zero",
    None -> "nothing",
)
```

---

## Best Practices

### Prefer Match Over Narrowing

```sigil
// Verbose: explicit narrowing
@describe (opt: Option<User>) -> str = run(
    if is_some(.opt: opt) then run(
        let user = unwrap(.opt: opt),
        user.name,
    )
    else
        "anonymous",
)

// Better: match handles it
@describe (opt: Option<User>) -> str = match(
    opt,
    Some(user) -> user.name,
    None -> "anonymous",
)
```

### Use Early Return for Validation

```sigil
@process (input: Input) -> Result<Output, Error> = run(
    if is_none(.opt: input.required_field) then
        return Err(MissingField { name: "required_field" }),

    if is_err(.result: validate(.input: input)) then
        return Err(ValidationFailed {}),

    // All checks passed, types are narrowed
    Ok(transform(.input: input)),
)
```

### Group Related Checks

```sigil
@validate_user (user: User) -> Result<User, [str]> = run(
    let mut errors = [],

    if is_none(.opt: user.email) then
        errors = errors + ["email required"],

    if is_none(.opt: user.name) then
        errors = errors + ["name required"],

    if len(.collection: errors) > 0 then
        Err(errors)
    else
        Ok(user),
)
```

---

## Error Messages

### Narrowing Not Applied

```
error[E0500]: cannot unwrap Option that may be None
  |
5 | unwrap(.opt: opt)
  | ^^^^^^^^^^^^^^^^^ opt has type Option<int>, not Some<int>
  |
help: check with is_some() first:
  | if is_some(.opt: opt) then unwrap(.opt: opt) else default
```

### Narrowing Lost After Reassignment

```
error[E0501]: narrowing invalidated by reassignment
  |
5 | if is_some(.opt: opt) then
6 |     opt = compute(),
7 |     unwrap(.opt: opt)
  |     ^^^^^^^^^^^^^^^^^ opt was reassigned, narrowing no longer valid
```

### Computed Boolean No Narrowing

```
warning[W0502]: computed boolean does not enable narrowing
  |
5 | valid = is_some(.opt: opt_a) && is_some(.opt: opt_b)
6 | if valid then
7 |     unwrap(.opt: opt_a)
  |
help: use the condition directly:
  | if is_some(.opt: opt_a) && is_some(.opt: opt_b) then
```

---

## Summary

| Feature | Behavior |
|---------|----------|
| `is_some(.opt: opt)` | Narrows to `Some(T)` in true branch |
| `is_none(.opt: opt)` | Narrows to `None` in true branch |
| `is_ok(.result: result)` | Narrows to `Ok(T)` in true branch |
| `is_err(.result: result)` | Narrows to `Err(E)` in true branch |
| `is_variant(.value: val, .variant: X)` | Narrows to variant `X` in true branch |
| Early return | Narrows for rest of scope |
| `assert(.condition: check)` | Narrows for rest of scope |
| `&&` | Propagates narrowing left-to-right |
| `\|\|` | No narrowing propagation |
| Match | Auto-narrows in each arm |

---

## See Also

- [Match Pattern](01-match-pattern.md) — Pattern matching with automatic narrowing
- [Guards and Bindings](03-guards-and-bindings.md) — Conditional patterns
- [Error Handling](../05-error-handling/index.md) — Result and Option types
- [Type System](../03-type-system/index.md) — Sum types and generics

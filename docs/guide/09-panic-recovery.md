---
title: "Panic and Recovery"
description: "Unrecoverable errors, contracts, and catching panics."
order: 9
part: "Error Handling & Safety"
---

# Panic and Recovery

While `Option` and `Result` handle expected failures, some situations can't be recovered from. Panics are for these unrecoverable errors.

## When to Panic

Panic for situations that indicate bugs or violated assumptions:

### Programming Errors

```ori
@get_required (key: str, map: {str: int}) -> int = {
    let value = map[key]
    match value {
        Some(v) -> v
        None -> panic(msg: `Required key '{key}' missing`)
    }
}
```

### Invariant Violations

```ori
@process_positive (n: int) -> int = {
    if n <= 0 then panic(msg: `Expected positive number, got {n}`)
    n * 2
}
```

### Impossible States

```ori
type Status = Active | Inactive

@activate (s: Status) -> Status = match s {
    Inactive -> Active
    Active -> panic(msg: "Cannot activate already active")
}
```

## The `panic` Function

```ori
@panic (msg: str) -> Never
```

- Takes a message describing what went wrong
- Returns `Never` — the function never completes normally
- Immediately terminates normal execution

### Panic Messages

Write clear, actionable messages:

```ori
// Good: explains what went wrong
panic(msg: `User ID {id} not found in database`)
panic(msg: `Index {i} out of bounds for list of length {len}`)
panic(msg: "Division by zero")

// Less helpful
panic(msg: "error")
panic(msg: "something went wrong")
```

## The `Never` Type

Functions that panic return `Never`:

```ori
@fail_with_code (code: int) -> Never = panic(msg: `Error code: {code}`)
```

`Never` is useful in type system:

```ori
// Both branches must have same type
// panic returns Never, which is compatible with any type
let value = if condition then
    compute_value()
else
    panic(msg: "should not happen")
```

## Panic vs Result

| Situation | Use |
|-----------|-----|
| File not found | `Result` — caller can handle |
| Invalid user input | `Result` — show error message |
| Network timeout | `Result` — can retry |
| Index out of bounds (your bug) | `panic` — programming error |
| Invariant violated | `panic` — should never happen |
| Missing required config at startup | `panic` — can't continue |

**Rule of thumb:**
- `Result` for expected failures the caller should handle
- `panic` for bugs and "impossible" situations

## Contracts

Contracts express assumptions about function inputs and outputs.

### Pre-conditions with `pre_check`

Verify assumptions before the function body:

```ori
@sqrt (x: float) -> float = {
    pre_check: x >= 0.0
    compute_sqrt(x: x)
}
```

If the condition fails, the function panics with a default message.

### Custom Error Messages

Add a message with `|`:

```ori
@sqrt (x: float) -> float = {
    pre_check: x >= 0.0 | "x must be non-negative"
    compute_sqrt(x: x)
}

@divide (a: int, b: int) -> int = {
    pre_check: b != 0 | "division by zero"
    a / b
}
```

### Post-conditions with `post_check`

Verify the result after computation:

```ori
@abs (n: int) -> int = {
    pre_check: true,                        // No pre-condition
    if n < 0 then -n else n
    post_check: result -> result >= 0,      // Result must be non-negative
}
```

The post-check receives the result value:

```ori
@clamp (value: int, min: int, max: int) -> int = {
    pre_check: min <= max | "min must not exceed max"
    if value < min then min else if value > max then max else value
    post_check: result -> result >= min && result <= max
}
```

### Combining Pre and Post Checks

```ori
@factorial (n: int) -> int = {
    pre_check: n >= 0 | "factorial undefined for negative numbers"
    if n <= 1 then 1 else n * factorial(n: n - 1)
    post_check: result -> result > 0 | "factorial must be positive"
}
```

### When to Use Contracts

**Use pre_check for:**
- Validating function arguments
- Documenting assumptions
- Catching caller mistakes early

**Use post_check for:**
- Verifying function correctness
- Documenting guarantees
- Catching implementation bugs

## Catching Panics

Use `catch` to capture panics (at boundaries):

```ori
let result = catch(expr: might_panic())
// Result<T, str> where str is the panic message

match result {
    Ok(v) -> print(msg: `Success: {v}`)
    Err(msg) -> print(msg: `Panic caught: {msg}`)
}
```

### When to Catch Panics

Don't use `catch` for normal error handling — it's for exceptional situations:

**Test frameworks:**
```ori
@test_panics tests @divide () -> void = {
    let result = catch(expr: divide(a: 1, b: 0))
    assert_err(result: result)
}
```

**Plugin systems:**
```ori
@run_plugin (plugin: Plugin) -> Result<void, str> =
    catch(expr: plugin.execute())
```

**REPL environments:**
```ori
@eval_safely (code: str) -> Result<Value, str> =
    catch(expr: evaluate(code: code))
```

### Catch vs Result

| Approach | Use Case |
|----------|----------|
| `Result` | Expected, recoverable errors |
| `catch` | Isolating untrusted code, test frameworks |

## Testing Panics

Assert that code panics:

```ori
@test_divide_by_zero tests @divide () -> void = {
    assert_panics(f: () -> divide(a: 1, b: 0))
}
```

Assert panic with specific message:

```ori
@test_divide_message tests @divide () -> void = {
    assert_panics_with(
        f: () -> divide(a: 1, b: 0)
        msg: "division by zero"
    )
}
```

## PanicInfo Type

When a panic is caught, you can get details:

```ori
type PanicInfo = {
    message: str,
    location: str,
}
```

## Complete Example

```ori
// A stack data structure with contracts
type Stack<T> = { items: [T], max_size: int }

impl<T> Stack<T> {
    @new (max_size: int) -> Stack<T> = {
        pre_check: max_size > 0 | "max_size must be positive"
        Stack { items: [], max_size }
    }

    @push (self, item: T) -> Stack<T> = {
        pre_check: self.len() < self.max_size | "stack overflow"
        Stack { ...self, items: [...self.items, item] }
        post_check: result -> result.len() == self.len() + 1
    }

    @pop (self) -> (T, Stack<T>) = {
        pre_check: self.len() > 0 | "stack underflow"
        let last_index = self.len() - 1
        let item = self.items[last_index]
        let new_items = self.items.take(count: last_index).collect()
        (item, Stack { ...self, items: new_items })
        post_check: (_, result) -> result.len() == self.len() - 1
    }

    @peek (self) -> T = {
        pre_check: self.len() > 0 | "cannot peek empty stack"
        self.items[self.len() - 1]
    }

    @len (self) -> int = len(collection: self.items)

    @is_empty (self) -> bool = self.len() == 0

    @is_full (self) -> bool = self.len() == self.max_size
}

@test_stack_new tests @Stack.new () -> void = {
    let s = Stack<int>.new(max_size: 5)
    assert(condition: s.is_empty())
    assert(condition: !s.is_full())
}

@test_stack_push tests @Stack.push () -> void = {
    let s = Stack<int>.new(max_size: 2)
    let s = s.push(item: 1)
    let s = s.push(item: 2)
    assert(condition: s.is_full())
}

@test_stack_overflow tests @Stack.push () -> void = {
    let s = Stack<int>.new(max_size: 1)
    let s = s.push(item: 1)
    assert_panics_with(
        f: () -> s.push(item: 2)
        msg: "stack overflow"
    )
}

@test_stack_pop tests @Stack.pop () -> void = {
    let s = Stack<int>.new(max_size: 5)
    let s = s.push(item: 10)
    let (item, s) = s.pop()
    assert_eq(actual: item, expected: 10)
    assert(condition: s.is_empty())
}

@test_stack_underflow tests @Stack.pop () -> void = {
    let s = Stack<int>.new(max_size: 5)
    assert_panics_with(
        f: () -> s.pop()
        msg: "stack underflow"
    )
}

// Calculator with validation
@safe_divide (a: float, b: float) -> float = {
    pre_check: b != 0.0 | "division by zero"
    a / b
}

@safe_sqrt (x: float) -> float = {
    pre_check: x >= 0.0 | `sqrt undefined for negative: {x}`
    compute_sqrt(x: x)
    post_check: result -> result >= 0.0
}

// Placeholder for actual sqrt implementation
@compute_sqrt (x: float) -> float = x  // Simplified

@test_safe_divide tests @safe_divide () -> void = {
    assert_eq(actual: safe_divide(a: 10.0, b: 2.0), expected: 5.0)
    assert_panics(f: () -> safe_divide(a: 10.0, b: 0.0))
}

@test_safe_sqrt tests @safe_sqrt () -> void = {
    assert_eq(actual: safe_sqrt(x: 0.0), expected: 0.0)
    assert_panics(f: () -> safe_sqrt(x: -1.0))
}
```

## Quick Reference

### Panic

```ori
panic(msg: "error message") -> Never
```

### Contracts

```ori
{
    pre_check: condition | "error message"
    body_expression
    post_check: result -> condition | "error message"
}
```

### Catching Panics

```ori
catch(expr: might_panic()) -> Result<T, str>
```

### Testing Panics

```ori
assert_panics(f: () -> might_panic())
assert_panics_with(f: () -> might_panic(), msg: "expected message")
```

## What's Next

Now that you understand panic and recovery:

- **[Modules and Imports](/guide/10-modules-imports)** — Organize code into modules
- **[Testing](/guide/12-testing)** — Write comprehensive tests

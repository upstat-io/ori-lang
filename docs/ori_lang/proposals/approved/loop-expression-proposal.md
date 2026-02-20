# Proposal: Loop Expression

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Compiler, expressions, control flow

---

## Summary

This proposal formalizes the `loop(...)` infinite loop expression syntax, including break/continue semantics, type inference from break values, and interaction with labeled loops.

---

## Problem Statement

The spec documents `loop(...)` syntax but leaves unclear:

1. **Type inference**: How is the loop type determined from break values?
2. **Infinite loops**: What is the type of a loop with no break?
3. **Multiple breaks**: How are multiple break paths unified?
4. **Continue semantics**: What does continue mean in a loop?
5. **Body structure**: What expressions are valid in the loop body?

---

## Syntax

### Basic Form

```ori
loop {body}
```

### With Label

```ori
loop:name(body)
```

---

## Semantics

### Infinite Iteration

The `loop(...)` expression repeatedly evaluates its body until a `break` is encountered:

```ori
loop {
    let item = queue.pop()
    if is_none(item) then break
    process(item.unwrap())
}
```

### Body

The body is a single expression. For multiple expressions, use `run(...)`:

```ori
// Single expression
loop {process_next()}

// Multiple expressions — use run
loop {
    let x = compute()
    if done(x) then break x
    update(x)
}
```

### Loop Type

The type of a `loop` expression is determined by its break values:

```ori
let result: int = loop {
    let x = compute()
    if x > 100 then break x
}
// result has type int
```

### Infinite Loop Type

A loop with no break (or only `break` without value) has type `Never`:

```ori
@server_loop () -> Never = loop {
    let request = accept()
    handle(request)
}
```

This is useful for server main loops, event loops, and other intentionally infinite processes.

### Break Without Value

`break` without a value exits the loop. The loop expression has type `void`:

```ori
loop {
    let msg = receive()
    if is_shutdown(msg) then break
    process(msg)
}
// Expression has type void
```

### Break With Value

`break value` exits the loop and makes the loop evaluate to `value`:

```ori
let found = loop {
    let candidate = next()
    if is_none(candidate) then break None
    let item = candidate.unwrap()
    if matches(item) then break Some(item)
}
// found has type Option<T>
```

### Multiple Break Paths

All break paths must produce compatible types:

```ori
let result = loop {
    if condition_a then break 1,      // int
    if condition_b then break 2,      // int
    if condition_c then break "three",  // ERROR: expected int
}
```

If breaks have different types, it is a compile-time error.

### Continue

`continue` skips the rest of the current iteration and starts the next:

```ori
loop {
    let item = next()
    if is_none(item) then break
    if skip(item.unwrap()) then continue,  // Start next iteration
    process(item.unwrap())
}
```

### Continue With Value

`continue value` in a loop is an error. Unlike `for...yield`, loops do not accumulate values:

```ori
loop {
    if condition then continue 42,  // ERROR: loop doesn't collect
}
```

---

## Labeled Loops

### Label Syntax

```ori
loop:name(body)
```

No space around the colon.

### Break to Label

```ori
loop:outer({
    loop:inner({
        if done then break:outer,  // Exit outer loop
        if next then break:inner,  // Exit inner loop
        process()
    })
})
```

### Break With Value to Label

```ori
let result = loop:search({
    for item in items do
        if matches(item) then break:search item
    break:search None
})
```

### Continue to Label

```ori
loop:outer({
    for x in xs do
        if skip_all(x) then continue:outer,  // Restart outer loop
        process(x)
})
```

---

## Interaction with For-Do

A common pattern combines `loop` with inner `for...do`:

```ori
@find<T> (items: [T], predicate: (T) -> bool) -> Option<T> =
    loop {
        for item in items do
            if predicate(item) then break Some(item)
        break None
    }
```

The `for...do` executes, and if no break occurs during iteration, the explicit `break None` exits the loop.

---

## Type Examples

### Void Loop

```ori
loop {
    process()
    if done() then break
}
// Type: void
```

### Value-Producing Loop

```ori
loop {
    let x = compute()
    if x > threshold then break x
}
// Type: int (assuming compute returns int)
```

### Never (Infinite)

```ori
loop {handle_event(wait_for_event())}
// Type: Never (no break)
```

### Optional Result

```ori
loop {
    let maybe = try_next()
    if is_none(maybe) then break None
    let item = maybe.unwrap()
    if matches(item) then break Some(item)
}
// Type: Option<T>
```

---

## Error Propagation

The `?` operator can be used within loops:

```ori
@process_until_error (items: [Item]) -> Result<void, Error> =
    loop {
        for item in items do
            validate(item)?,  // Propagates Err, exits function
        break Ok(())
    }
```

When `?` propagates an error, it exits the enclosing function, not just the loop.

---

## Nested Loops

Loops can be arbitrarily nested:

```ori
loop:outer({
    loop:middle({
        loop:inner({
            if done_all then break:outer
            if done_middle then break:middle
            if done_inner then break:inner
            process()
        })
    })
})
```

Labels distinguish which loop to exit.

---

## Error Messages

### Break Type Mismatch

```
error[E0860]: mismatched types in loop break
  --> src/main.ori:5:25
   |
 3 |     if a then break 1,
   |                     - expected `int` due to this
 5 |     if b then break "two",
   |                     ^^^^^ expected `int`, found `str`
   |
   = note: all break paths must have compatible types
```

### Continue With Value

```
error[E0861]: `continue` with value in `loop`
  --> src/main.ori:5:20
   |
 5 |     if skip then continue 42,
   |                  ^^^^^^^^^^^ `loop` does not collect values
   |
   = help: use `break` to exit with a value
   = help: or remove the value: `continue`
```

### Missing Break in Value Context

```
error[E0862]: infinite loop used in value context
  --> src/main.ori:5:13
   |
 5 | let x: int = loop(
   |              ^^^^ loop never breaks with a value
   |
   = note: expected type `int`
   = note: loop has type `Never` (no break with value)
   = help: add `break value` to produce a result
```

### Break Value Without Context

```
error[E0863]: `break` with value in void context
  --> src/main.ori:5:20
   |
 5 |     if done then break 42,
   |                  ^^^^^^^^ loop discards this value
   |
   = note: enclosing loop is in void context
   = help: remove the value: `break`
   = help: or assign the loop to a variable
```

---

## Examples

### Event Loop

```ori
@event_loop (handler: (Event) -> void) -> Never =
    loop {
        let event = wait_for_event()
        handler(event)
    }
```

### Search with Limit

```ori
@find_with_limit<T> (source: impl Iterator<Item = T>, pred: (T) -> bool, limit: int) -> Option<T> =
    {
        let count = 0
        loop {
            if count >= limit then break None
            let item = source.next()
            if is_none(item) then break None
            if pred(item.unwrap()) then break Some(item.unwrap())
            count = count + 1
        }
    }
```

### State Machine

```ori
@run_state_machine (initial: State) -> FinalState =
    {
        let state = initial
        loop {
            let result = state.step()
            match result {
                Continue(next) -> state = next
                Done(final) -> break final
            }
        }
    }
```

### Retry Loop

```ori
@retry<T> (max_attempts: int, operation: () -> Result<T, Error>) -> Result<T, Error> =
    {
        let attempts = 0
        loop {
            attempts = attempts + 1
            match operation() {
                Ok(value) -> break Ok(value)
                Err(e) -> if attempts >= max_attempts then break Err(e)
            }
        }
    }
```

### Nested Search

```ori
@find_in_matrix (matrix: [[int]], target: int) -> Option<(int, int)> =
    loop:search({
        for (i, row) in matrix.iter().enumerate() do
            for (j, value) in row.iter().enumerate() do
                if value == target then break:search Some((i, j))
        break:search None
    })
```

---

## Grammar

The grammar is defined in `grammar.ebnf` under the `loop_expr` production:

```
loop_expr = "loop" [ label ] "(" expr ")" .
label = ":" identifier .
```

---

## Spec Changes Required

### Update `09-expressions.md`

Expand Loop Expression section with:

1. Type inference from break values
2. Never type for infinite loops
3. Continue restrictions
4. Body structure (single expression, use `run(...)` for sequences)

### Update `19-control-flow.md`

Add detailed break/continue semantics specific to `loop`.

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Syntax | `loop(body)` or `loop:name(body)` |
| Body | Single expression; use `run(...)` for sequences |
| Iteration | Infinite until `break` |
| Type | Inferred from break values |
| No break | Type is `Never` (infinite) |
| Break without value | Type is `void` |
| Break with value | Loop evaluates to value |
| Multiple breaks | Must have compatible types |
| Continue | Skips to next iteration |
| Continue with value | Error (loop doesn't collect) |
| Labels | Target specific loop with `break:name`/`continue:name` |

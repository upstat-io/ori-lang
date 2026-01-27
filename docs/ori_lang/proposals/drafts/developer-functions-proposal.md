# Proposal: Developer Convenience Functions

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-01-27

---

## Summary

Add three built-in functions to the prelude for common development tasks:

```ori
@todo () -> Never
@todo (reason: str) -> Never

@unreachable () -> Never
@unreachable (reason: str) -> Never

@dbg<T: Debug> (value: T) -> T
@dbg<T: Debug> (value: T, label: str) -> T
```

These functions improve developer experience with minimal language complexity.

---

## Motivation

### The Problem

Developers frequently need to:

1. **Mark unfinished code** — Placeholder that compiles but panics at runtime
2. **Mark impossible branches** — Assert that certain code paths should never execute
3. **Quick debugging** — Print values without disrupting code flow

Currently, all of these require manual `panic()` calls or `print()` statements:

```ori
// Marking unfinished code
@new_feature () -> Result<Data, Error> =
    panic(msg: "TODO: implement new_feature")

// Marking impossible branches
match(
    validated_input,
    Valid(v) -> process(v: v),
    Invalid(_) -> panic(msg: "unreachable: input was validated"),
)

// Debugging (disrupts code flow)
let x = calculate()
print(msg: "x = " + (x as str))  // Can't easily show complex types
let result = use_value(x: x)
```

### Why Dedicated Functions?

| Function | Benefit |
|----------|---------|
| `todo` | Clearly communicates "not done yet" in code and panic message |
| `unreachable` | Documents impossible states; different semantic meaning than `todo` |
| `dbg` | Returns value, includes location, uses `Debug` trait |

---

## Design

### `todo`

Marks code that hasn't been implemented yet. Panics when executed.

```ori
@todo () -> Never
@todo (reason: str) -> Never
```

**Behavior:**
- Panics with message including file, line, and optional reason
- Return type `Never` allows use in any expression position

**Examples:**

```ori
@parse_json (input: str) -> Result<Json, Error> =
    todo()  // Panics: "not yet implemented at src/parser.ori:15"

@handle_event (event: Event) -> void = match(
    event,
    Click(pos) -> handle_click(pos: pos),
    Scroll(delta) -> todo(reason: "scroll handling"),  // "not yet implemented: scroll handling at src/ui.ori:42"
    KeyPress(key) -> handle_key(key: key),
)

// Works in expression position due to Never type
let value: int = if condition then 42 else todo()
```

**Panic message format:**

```
PANIC at src/file.ori:15:5
  not yet implemented
```

```
PANIC at src/file.ori:42:20
  not yet implemented: scroll handling
```

### `unreachable`

Marks code that should never execute. Panics if reached (indicates logic error).

```ori
@unreachable () -> Never
@unreachable (reason: str) -> Never
```

**Behavior:**
- Panics with message indicating unreachable code was reached
- Semantically different from `todo` — this isn't "not done", it's "should be impossible"

**Examples:**

```ori
@safe_divide (a: int, b: int) -> int = run(
    if b == 0 then
        panic(msg: "division by zero")
    else run(
        // At this point, b is guaranteed non-zero
        let result = a / b,
        if result * b != a then
            unreachable(reason: "integer division invariant violated")
        else
            result,
    ),
)

@process_validated (input: Input) -> Output = run(
    // Input was validated before this function was called
    match(
        input.status,
        Status.Valid -> compute(input: input),
        Status.Invalid -> unreachable(reason: "invalid input passed to process_validated"),
    ),
)

// Exhaustive match where we know some cases can't happen
@day_type (day: int) -> str = match(
    day,
    1 | 2 | 3 | 4 | 5 -> "weekday",
    6 | 7 -> "weekend",
    _ -> unreachable(reason: "day must be 1-7"),
)
```

**Panic message format:**

```
PANIC at src/file.ori:30:12
  unreachable code reached
```

```
PANIC at src/file.ori:45:8
  unreachable code reached: invalid input passed to process_validated
```

### `dbg`

Prints a value with location information and returns it, enabling inline debugging.

```ori
@dbg<T: Debug> (value: T) -> T
@dbg<T: Debug> (value: T, label: str) -> T
```

**Behavior:**
- Prints `[file:line] value = <debug representation>` to stderr
- Returns the value unchanged
- Requires `Debug` trait (see Debug Trait proposal)

**Examples:**

```ori
// Simple debugging
let result = dbg(value: calculate())
// Prints: [src/math.ori:10] value = 42
// result = 42

// With label for clarity
let x = dbg(value: get_x(), label: "x coordinate")
let y = dbg(value: get_y(), label: "y coordinate")
// Prints: [src/point.ori:5] x coordinate = 100
// Prints: [src/point.ori:6] y coordinate = 200

// Inline in expressions (doesn't disrupt flow)
let doubled = dbg(value: items, label: "input")
    .map(transform: x -> x * 2)
    .filter(predicate: x -> dbg(value: x, label: "checking") > 10)
// Prints: [src/process.ori:12] input = [1, 2, 3, 4, 5]
// Prints: [src/process.ori:14] checking = 2
// Prints: [src/process.ori:14] checking = 4
// Prints: [src/process.ori:14] checking = 6
// ... etc

// Debugging complex types
#[derive(Debug)]
type Request = { method: str, path: str, headers: {str: str} }

let req = dbg(value: build_request())
// Prints: [src/http.ori:20] value = Request { method: "GET", path: "/api", headers: {"Host": "example.com"} }
```

**Output format:**

```
[src/file.ori:10] value = 42
[src/file.ori:15] my label = Point { x: 1, y: 2 }
```

**Output destination:** stderr (not stdout), so it doesn't interfere with program output.

---

## Implementation Notes

### `Never` Type

All three functions (when panicking) return `Never`, the bottom type:
- `Never` is a subtype of all types
- Allows these functions in any expression position
- The function never actually returns (it panics)

```ori
// These all type-check because Never is a subtype of int
let x: int = todo()
let y: int = unreachable()
let z: int = if always_true then 42 else unreachable()
```

### Location Information

The functions need access to call-site location (file, line, column). This is typically:
- Captured at compile time
- Passed implicitly by the compiler
- Not visible in the function signature

```ori
// Conceptually, the compiler transforms:
todo(reason: "foo")
// Into something like:
__todo_impl(reason: "foo", location: Location { file: "src/x.ori", line: 42, column: 5 })
```

### `dbg` and Capabilities

`dbg` writes to stderr, which is I/O. Options:

A) **Implicit Print capability** — `dbg` uses the `Print` capability (which has a default)
B) **Exempt from capabilities** — Debug output is special, always allowed
C) **Require capability** — `dbg` requires `uses Print`

**Recommendation:** A — Use the existing `Print` capability with its default. This keeps the capability system consistent while still making `dbg` convenient (Print has a default, so no explicit `with` needed).

---

## Design Rationale

### Why Not Just `panic`?

`panic(msg: "TODO")` works, but:
- Doesn't convey intent (is this temporary? a bug? impossible?)
- Requires manually typing location info for useful messages
- `todo` and `unreachable` have distinct meanings

### Why Named Parameters?

Ori uses named parameters for all function calls. These functions follow that convention:

```ori
todo(reason: "waiting on API")
unreachable(reason: "validated above")
dbg(value: x, label: "result")
```

This is consistent with the rest of the language.

### Why `dbg` Returns the Value?

Returning the value is the key feature:

```ori
// Without return value (current situation)
let x = calculate()
print(msg: x.debug())
let result = process(x: x)

// With return value
let result = process(x: dbg(value: calculate()))
```

The second form:
- Doesn't require a separate binding
- Can be inserted/removed without restructuring code
- Works inline in method chains

### Why `label` Instead of Expression Text?

Rust's `dbg!` macro captures the expression text:
```rust
dbg!(x + y)  // Prints: [file:line] x + y = 30
```

Ori doesn't have macros, so we can't capture `x + y` as text. The `label` parameter is the pragmatic alternative:
```ori
dbg(value: x + y, label: "sum")  // Prints: [file:line] sum = 30
```

---

## Examples

### Incremental Implementation

```ori
@process_command (cmd: Command) -> Result<Response, Error> = match(
    cmd,
    Command.Help -> Ok(help_text()),
    Command.Version -> Ok(version_info()),
    Command.Run(args) -> todo(reason: "run command"),
    Command.Test(args) -> todo(reason: "test command"),
)
```

### Defensive Programming

```ori
@get_user (id: UserId) -> User = run(
    let user = database.find(id: id),
    match(
        user,
        Some(u) -> u,
        None -> unreachable(reason: "user id was validated"),
    ),
)
```

### Debugging a Pipeline

```ori
@analyze (data: [Record]) -> Summary =
    data
        .iter()
        .filter(predicate: r -> dbg(value: r.is_valid(), label: "valid?"))
        .map(transform: r -> dbg(value: extract(r: r), label: "extracted"))
        .fold(initial: Summary.empty(), op: (acc, item) ->
            dbg(value: acc.add(item: item), label: "accumulated"))
```

---

## Spec Changes Required

### `12-modules.md`

Add to prelude built-in functions:

```markdown
### Developer Functions

```ori
@todo () -> Never
@todo (reason: str) -> Never
```

Marks unfinished code. Panics with "not yet implemented" and location.

```ori
@unreachable () -> Never
@unreachable (reason: str) -> Never
```

Marks code that should never execute. Panics with "unreachable code reached" and location.

```ori
@dbg<T: Debug> (value: T) -> T
@dbg<T: Debug> (value: T, label: str) -> T
```

Prints value with location to stderr, returns value unchanged. Requires `Debug` trait.
```

### `/CLAUDE.md`

Add to built-in functions:
- `todo()`, `todo(reason: str)` -> `Never`
- `unreachable()`, `unreachable(reason: str)` -> `Never`
- `dbg(value: T)`, `dbg(value: T, label: str)` -> `T`

---

## Summary

| Function | Purpose | Returns | Panic Message |
|----------|---------|---------|---------------|
| `todo()` | Mark unfinished code | `Never` | "not yet implemented at file:line" |
| `todo(reason:)` | Mark unfinished with context | `Never` | "not yet implemented: {reason} at file:line" |
| `unreachable()` | Mark impossible code | `Never` | "unreachable code reached at file:line" |
| `unreachable(reason:)` | Mark impossible with context | `Never` | "unreachable code reached: {reason} at file:line" |
| `dbg(value:)` | Debug print, return value | `T` | N/A (prints to stderr) |
| `dbg(value:, label:)` | Debug print with label | `T` | N/A (prints to stderr) |

These three functions address common development needs with minimal language complexity, following Ori's conventions for named parameters and explicit behavior.

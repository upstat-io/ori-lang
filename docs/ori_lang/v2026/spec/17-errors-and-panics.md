---
title: "Errors and Panics"
description: "Clause 17: Ori Language Specification — Errors and Panics"
order: 17
section: "Language"
---

# 17 Errors and panics

Ori distinguishes between recoverable errors and unrecoverable panics.

> **Grammar:** See [grammar.ebnf](grammar.md) § PATTERNS (catch_expr)

## 17.1 Recoverable errors

Recoverable errors use `Result<T, E>` and `Option<T>` types. See [Types](08-types.md) for type definitions and methods.

### 17.1.1 Error propagation

The `?` operator propagates errors. See [Control Flow](16-control-flow.md) for details.

```ori
@load (path: str) -> Result<Data, Error> = {
    let content = read_file(path)?;
    let data = parse(content)?;
    Ok(data)
}
```

## 17.2 Panics

A _panic_ is an unrecoverable error that terminates normal execution.

### 17.2.1 Implicit panics

The following operations cause implicit panics:

| Operation | Condition | Panic message |
|-----------|-----------|---------------|
| List index | Index out of bounds | "index out of bounds: index N, length M" |
| String index | Index out of bounds | "index out of bounds: index N, length M" |
| `.unwrap()` | Called on `None` | "called unwrap on None" |
| `.unwrap()` | Called on `Err(e)` | "called unwrap on Err: {e}" |
| Division | Divisor is zero | "division by zero" |
| Modulo | Divisor is zero | "modulo by zero" |
| Integer arithmetic | Result overflows `int` range | "integer overflow in {operation}" |
| Division/modulo | `int.min / -1` or `int.min % -1` | "integer overflow in {operation}" |

### 17.2.2 Explicit panic

The `panic` function triggers a panic explicitly:

```ori
panic(message)
```

`panic` has return type `Never` and never returns normally:

```ori
let x: int = if valid then value else panic("invalid state");
```

### 17.2.3 Panic behavior

When a panic occurs:

1. Error message is recorded with source location
2. Stack trace is captured
3. If inside `catch(...)`, control transfers to the catch
4. Otherwise, message and trace print to stderr, program exits with code 1

### 17.2.4 Panic message format

Panic messages include the source location where the panic occurred:

```
<message> at <file>:<line>:<column>
```

For example:
```ori
panic(msg: "invalid state")
// Produces: "invalid state at src/main.ori:42:5"
```

This format applies to both explicit `panic()` calls and implicit panics (index out of bounds, division by zero, etc.).

### 17.2.5 PanicInfo type

The `PanicInfo` type contains structured information about a panic:

```ori
type PanicInfo = {
    message: str,
    location: TraceEntry,
    stack_trace: [TraceEntry],
    thread_id: Option<int>,
}
```

| Field | Description |
|-------|-------------|
| `message` | The panic message string |
| `location` | Source location where the panic occurred |
| `stack_trace` | Full call stack at panic time |
| `thread_id` | Task identifier if in concurrent context |

`PanicInfo` is available in the prelude. It implements `Printable` and `Debug`:

```ori
impl PanicInfo: Printable {
    @to_str (self) -> str =
        `panic at {self.location.file}:{self.location.line}:{self.location.column}: {self.message}`;
}
```

NOTE  The `catch` pattern returns `Result<T, str>`, not `Result<T, PanicInfo>`. `PanicInfo` is available for custom panic handlers via the optional `@panic` entry point.

## 17.3 Integer overflow

Integer arithmetic panics on overflow:

```ori
let max: int = 9223372036854775807;  // max signed 64-bit
let result = catch(expr: max + 1);   // Err("integer overflow")
```

Addition, subtraction, multiplication, and negation all panic on overflow. Programs requiring wrapping or saturating arithmetic should use methods from `std.math`.

## 17.4 Catching panics

The `catch` pattern captures panics and converts them to `Result<T, str>`:

```ori
let result = catch(expr: dangerous_operation());
// result: Result<T, str>

match result {
    Ok(value) -> use(value),
    Err(msg) -> handle_error(msg),
}
```

If the expression evaluates successfully, `catch` returns `Ok(value)`. If the expression panics, `catch` returns `Err(message)` where `message` is the panic message string.

### 17.4.1 Nested catch

`catch` expressions may be nested. A panic propagates to the innermost enclosing `catch`:

```ori
catch(expr: {
    let x = catch(expr: may_panic())?,  // inner catch
    process(x),                          // may also panic
})
// outer catch handles panics from process()
```

### 17.4.2 Limitations

`catch` cannot recover from:
- Process-level signals (SIGKILL, SIGSEGV)
- Out of memory conditions
- Stack overflow

These conditions terminate the program immediately.

## 17.5 Panic assertions

The prelude provides two functions for testing panic behavior:

### 17.5.1 `assert_panics`

```ori
assert_panics(f: () -> void) -> void
```

`assert_panics` evaluates the thunk `f`. If `f` panics, the assertion succeeds. If `f` returns normally, `assert_panics` itself panics with the message `"assertion failed: expected panic but succeeded"`.

### 17.5.2 `assert_panics_with`

```ori
assert_panics_with(f: () -> void, msg: str) -> void
```

`assert_panics_with` evaluates the thunk `f`. If `f` panics with a message equal to `msg`, the assertion succeeds. If `f` panics with a different message or returns normally, `assert_panics_with` panics.

## 17.6 Error conventions

The `Error` trait provides a standard interface for error types:

```ori
trait Error {
    @message (self) -> str;
}
```

Custom error types should implement `Error`:

```ori
type ParseError = { line: int, message: str }

impl ParseError: Error {
    @message (self) -> str =
        "line " + str(self.line) + ": " + self.message;
}
```

Functions returning `Result` conventionally use `E: Error`, but any type may be used as the error type.

## 17.7 Error return traces

When the `?` operator propagates an error, the source location is automatically recorded. This builds an _error return trace_ showing the propagation path.

### 17.7.1 Automatic collection

```ori
@load (path: str) -> Result<Data, Error> = try {
    let content = read_file(path)?,  // location recorded if Err
    let parsed = parse(content)?,     // location recorded if Err
    Ok(parsed)
}
```

Traces are collected unconditionally in all builds. No syntax changes required.

### 17.7.2 TraceEntry type

```ori
type TraceEntry = {
    function: str,
    file: str,
    line: int,
    column: int,
}
```

The `function` field includes the `@` prefix for function names (e.g., `"@load_config"`).

### 17.7.3 Trace ordering

Entries are ordered most recent first (like a stack trace). The first entry is the most recent `?` propagation point.

### 17.7.4 Accessing traces

The `Error` type provides trace access methods:

| Method | Return Type | Description |
|--------|-------------|-------------|
| `.trace()` | `str` | Formatted trace string |
| `.trace_entries()` | `[TraceEntry]` | Programmatic access |
| `.has_trace()` | `bool` | Check if trace available |

### 17.7.5 Trace format

The `.trace()` method returns:

```
<function_name> at <file_path>:<line>:<column>
```

One entry per line, most recent propagation point first. Function names are left-padded to align the "at" column.

### 17.7.6 Context method

`Result` provides a `.context()` method to add context while preserving traces:

```ori
@load_config () -> Result<Config, Error> = try {
    let content = read_file("config.json")
        .context("failed to load config")?;
    Ok(parse(content))
}
```

### 17.7.7 Traceable trait

Custom error types may implement `Traceable` to carry their own traces:

```ori
trait Traceable {
    @with_trace (self, trace: [TraceEntry]) -> Self;
    @trace (self) -> [TraceEntry];
}
```

`Traceable` is optional. For non-implementing error types, traces attach to the `Result` wrapper during propagation.

### 17.7.8 Result trace methods

`Result<T, E>` provides trace access regardless of whether `E` implements `Traceable`:

| Method | Return Type | Description |
|--------|-------------|-------------|
| `.trace()` | `str` | Formatted trace string |
| `.trace_entries()` | `[TraceEntry]` | Programmatic access |
| `.has_trace()` | `bool` | Check if trace available |

When `E: Traceable`, these methods delegate to the error's trace methods. When `E` does not implement `Traceable`, the `Result` carries the trace internally.

### 17.7.9 Context storage

When `.context(msg:)` is called on a `Result`, the context string is stored separately from the trace. Contexts are ordered most recent first, matching trace ordering.

For error types implementing `Traceable`, contexts are stored in the error value. For non-Traceable errors, contexts are stored in the `Result` wrapper alongside the trace.

### 17.7.10 Relationship to panic traces

| Aspect | Error Return Trace | Panic Stack Trace |
|--------|-------------------|-------------------|
| Trigger | `?` propagation | `panic()` or implicit panic |
| Contents | Only `?` propagation points | Full call stack |
| Recovery | Via `Result` handling | Via `catch(...)` |

The two trace types may intersect. If an error is converted to a panic (e.g., via `.unwrap()`), the panic trace includes the unwrap location, while the error's return trace shows how the error arrived there.

## 17.8 Async error traces

Error traces are preserved across task boundaries in concurrent code.

### 17.8.1 Task boundary marker

When an error crosses from a spawned task to the parent task, a marker entry is inserted into the trace:

```ori
TraceEntry { function: "<task boundary>", file: "", line: 0, column: 0 }
```

This pseudo-entry indicates where the error crossed task boundaries, helping distinguish between propagation within a task and propagation across tasks.

### 17.8.2 Trace from parallel tasks

Each task in `parallel(...)` or `nursery(...)` maintains its own trace. When errors are collected:

```ori
@process_all (items: [int]) -> [Result<int, Error>] uses Suspend =
    parallel(tasks: items.map(i -> () -> process(i)));

// Each result's trace shows:
// - Propagation points within the spawned task
// - Task boundary marker
// - Propagation points in the parent task (if any)
```

### 17.8.3 Catch and panic traces

The `catch` pattern converts panics to `Result<T, str>`. Panics do not generate structured `?`-style traces because they bypass normal return flow. The panic message string contains the location information but not a `[TraceEntry]` list.

If code within `catch` returns `Err` (not a panic), the error's trace is preserved normally:

```ori
let result = catch(expr: {
    let x = fallible()?,  // Trace entry added
    Ok(x)
});
// result: Result<Result<T, Error>, str>
// Inner Err has trace; outer Ok means no panic
```

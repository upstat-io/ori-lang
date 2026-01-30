# Proposal: Error Trace Semantics with Async and Catch

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-29
**Affects:** Compiler, runtime, error handling

---

## Summary

This proposal specifies how error traces interact with async code, the `catch` pattern, and `try` blocks, addressing gaps around trace preservation across task boundaries and when traces can be lost.

---

## Problem Statement

The spec defines error return traces (collected at `?` propagation) but leaves unclear:

1. **Async traces**: How do traces work across task boundaries in `parallel`/`nursery`?
2. **Catch interaction**: When a panic is caught, is its trace preserved or lost?
3. **Multiple contexts**: How do `.context()` calls chain across error conversions?
4. **Memory overhead**: What are the costs of unconditional trace collection?
5. **Trace entry ordering**: What order are entries in, especially with async?

---

## Trace Collection Model

### When Traces Are Collected

Trace entries are added at each `?` propagation point:

```ori
@outer () -> Result<int, Error> = run(
    let x = inner()?,  // Entry added here
    Ok(x),
)

@inner () -> Result<int, Error> = run(
    let y = deep()?,   // Entry added here
    Ok(y),
)

@deep () -> Result<int, Error> =
    Err(Error { message: "failed" })  // Original error, no trace yet
```

If `deep` returns `Err`, the trace contains:
1. Entry from `deep()?` in `inner` (line X)
2. Entry from `inner()?` in `outer` (line Y)

### Trace Entry Structure

```ori
type TraceEntry = {
    function: str,   // Function name where ? occurred
    file: str,       // Source file path
    line: int,       // Line number
    column: int,     // Column number
}
```

### Trace Ordering

Entries are ordered **most recent first** (like a stack trace):

```ori
error.trace_entries()
// [
//   TraceEntry { function: "outer", ... },  // Most recent propagation
//   TraceEntry { function: "inner", ... },  // Earlier propagation
// ]
```

---

## Async Trace Behavior

### Within a Single Task

Traces work normally within a task — `?` adds entries as expected:

```ori
@async_fn () -> Result<Data, Error> uses Async = run(
    let x = step1()?,  // Entry added
    let y = step2()?,  // Entry added
    Ok(y),
)
```

### Across Task Boundaries

When errors cross task boundaries (via channel or nursery results), traces are **preserved**:

```ori
@outer () -> Result<[int], Error> uses Async = run(
    let results = parallel(
        tasks: items.map(i -> () -> process(i)),  // May return errors
    ),
    // results[n].err() contains full trace from spawned task
    ...
)
```

The trace includes entries from the spawned task's call stack.

### Trace Origin Marker

Async-originated traces include a marker indicating task boundary:

```ori
// Trace from parallel task:
// [
//   TraceEntry { function: "outer", ... },      // After parallel
//   TraceEntry { function: "<task boundary>", file: "", line: 0, column: 0 },
//   TraceEntry { function: "process", ... },    // Inside spawned task
//   TraceEntry { function: "inner_call", ... }, // Deeper in spawned task
// ]
```

The `<task boundary>` pseudo-entry marks where the error crossed from one task to another.

---

## Catch Pattern Interaction

### Catching Panics

The `catch` pattern converts panics to `Result`:

```ori
let result = catch(expr: may_panic())
// result: Result<T, str> where str is panic message
```

### Panic Trace Preservation

**Panics do NOT generate `?`-style traces** because panics bypass normal return flow. However, the panic message may contain location information:

```ori
@may_panic () -> int = panic(msg: "something wrong")

let result = catch(expr: may_panic())
// result = Err("something wrong at src/foo.ori:5:10")
```

The panic message includes the location but not a structured trace.

### Catching Errors vs Panics

| Mechanism | Trace Preserved? | Type |
|-----------|-----------------|------|
| `Result` with `?` | Yes, structured | `Error` with `.trace_entries()` |
| `panic` with `catch` | Location in message only | `str` |

### Result Errors Inside Catch

If caught code returns `Err` (not panic), traces work normally:

```ori
@returns_error () -> Result<int, Error> = Err(Error { ... })

let result = catch(expr: run(
    let x = returns_error()?,  // Trace entry added
    Ok(x),
))
// result: Result<Result<int, Error>, str>
// Inner Err has trace; outer Ok means no panic
```

---

## Context Chaining

### Adding Context

`.context()` adds human-readable context while preserving the trace:

```ori
let x = fallible()
    .context(msg: "while loading config")?;
```

### Multiple Contexts

Contexts chain, with most recent first:

```ori
@load_config () -> Result<Config, Error> = run(
    let raw = read_file(path: "config.json")
        .context(msg: "reading config file")?,
    let parsed = parse_json(raw)
        .context(msg: "parsing config JSON")?,
    Ok(parsed),
)

// If parse_json fails, error message shows:
// "parsing config JSON"
// With trace showing both context points
```

### Context vs Trace

| Aspect | Context | Trace |
|--------|---------|-------|
| Purpose | Human-readable explanation | Debug location info |
| Added by | `.context(msg:)` call | `?` propagation |
| Contains | Message string | File, line, function |
| Ordering | Most recent first | Most recent first |

---

## Memory Overhead

### Trace Storage

Traces are stored in the `Error` type (or attached to `Result` for non-Traceable errors):

```ori
type Error = {
    message: str,
    trace: [TraceEntry],  // Grows with each ?
    contexts: [str],      // Grows with each .context()
}
```

### Overhead Characteristics

| Scenario | Trace Size |
|----------|-----------|
| Shallow call stack (3-5 levels) | ~5 entries, negligible |
| Deep recursion (100+ levels) | ~100 entries, noticeable |
| Hot error path in loop | Entries accumulate per iteration |

### No Runtime Disable

Traces are **always collected** — there is no runtime flag to disable. This ensures:
- Consistent debugging experience
- No "works in debug, fails in prod" issues
- Predictable error behavior

### Optimization Notes

- Entries are small (4 values, references to interned strings)
- In success path, no allocation (no error = no trace)
- Errors are rare; overhead only matters when errors occur

---

## Trace-Preserving Error Conversion

### The Problem

Converting between error types can lose traces:

```ori
// BAD: trace lost
let result = fallible().map_err(e -> MyError { message: e.message });
```

### The Solution: Traceable Trait

Error types implementing `Traceable` preserve traces:

```ori
trait Traceable {
    @with_trace (self, trace: [TraceEntry]) -> Self
    @trace (self) -> [TraceEntry]
}
```

Conversion methods preserve traces automatically:

```ori
// GOOD: trace preserved
let result = fallible().map_err(e -> MyError::from(e));

// Or using context (always preserves)
let result = fallible().context(msg: "while doing X")?;
```

### Non-Traceable Errors

For error types that don't implement `Traceable`, the trace attaches to the `Result` wrapper:

```ori
type SimpleError = { code: int }  // No Traceable impl

@fallible () -> Result<int, SimpleError> = Err(SimpleError { code: 404 })

let result = fallible()?;
// Trace attached to Result, accessible via result.trace_entries()
// Even though SimpleError doesn't have .trace() method
```

---

## Examples

### Complete Trace Example

```ori
@main () -> void = run(
    match(load_user(id: 123),
        Ok(user) -> print(msg: user.name),
        Err(e) -> run(
            print(msg: `Error: {e.message}`),
            print(msg: `Trace:\n{e.trace()}`),
        ),
    ),
)

@load_user (id: int) -> Result<User, Error> uses Http = run(
    let response = fetch(url: `/users/{id}`)
        .context(msg: "fetching user data")?,
    let user = parse_user(response)
        .context(msg: "parsing user response")?,
    Ok(user),
)

// Output on error:
// Error: invalid JSON at position 42
// Trace:
//   at load_user (src/users.ori:8:5) - parsing user response
//   at load_user (src/users.ori:6:5) - fetching user data
//   at main (src/main.ori:2:5)
```

### Async Trace Example

```ori
@fetch_all (urls: [str]) -> [Result<str, Error>] uses Async =
    parallel(tasks: urls.map(url -> () -> fetch(url)))

// Each result has its own trace from its task:
// results[0].err().trace_entries() shows trace inside task 0
// results[1].err().trace_entries() shows trace inside task 1
```

---

## Spec Changes Required

### Update `20-errors-and-panics.md`

Add:
1. Trace ordering specification (most recent first)
2. Async trace behavior
3. Task boundary markers
4. `catch` interaction with traces

### Update Traceable Trait

Clarify:
1. Automatic trace attachment for non-Traceable errors
2. Context preservation rules
3. Trace-preserving error conversion patterns

---

## Summary

| Aspect | Behavior |
|--------|----------|
| Collection | At each `?` propagation |
| Ordering | Most recent first |
| Async | Preserved across task boundaries |
| Task marker | `<task boundary>` pseudo-entry |
| Panic in catch | Location in message only, no structured trace |
| Context | Chains with trace, most recent first |
| Memory | Proportional to propagation depth |
| Disable | Not possible (always on) |
| Non-Traceable | Trace attaches to Result wrapper |

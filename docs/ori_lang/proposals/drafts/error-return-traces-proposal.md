# Proposal: Error Return Traces

**Status:** Approved
**Author:** Eric
**Created:** 2026-01-22
**Approved:** 2026-01-25
**Inspired by:** Zig's error return traces

---

## Summary

Add automatic stack trace collection to `Result` error paths, enabling developers to see where errors originated, not just where they were caught.

```ori
@fetch_user (id: int) -> Result<User, Error> uses Http = try(
    let response = Http.get("/users/" + str(id))?,
    let user = parse_user(response)?,
    Ok(user),
)

// When this fails, the error includes a trace:
//   Error: invalid JSON at position 42
//   Trace:
//     parse_user     at src/users.ori:25:12
//     fetch_user     at src/users.ori:18:16
//     load_dashboard at src/dashboard.ori:45:20
//     main           at src/main.ori:10:5
```

---

## Motivation

### The Problem

When errors propagate through multiple function calls, the original error location is lost:

```ori
@main () -> Result<void, Error> = try(
    let data = process()?,  // Error caught here, but where did it start?
    Ok(()),
)

@process () -> Result<Data, Error> = try(
    let raw = fetch()?,
    let parsed = parse(raw)?,
    let validated = validate(parsed)?,  // Maybe the error originated here?
    Ok(validated),
)
```

With only the error message, debugging requires:
1. Reading the error message
2. Manually tracing through code to find possible sources
3. Adding temporary logging/prints
4. Re-running to gather more information

This is time-consuming for humans and difficult for AI to assist with.

### Current Ori Error Model

Ori's `Error` type supports manual chaining:

```ori
type Error = {
    message: str,
    source: Option<Error>,  // Manual chaining
}
```

This requires explicit wrapping at each level:

```ori
@process () -> Result<Data, Error> = try(
    let raw = fetch().map_err(e -> Error {
        message: "failed to fetch",
        source: Some(e),
    })?,
    // ... more wrapping ...
)
```

**Problems:**
- Verbose boilerplate at every propagation point
- Easy to forget, losing context
- Doesn't capture stack location automatically

### Prior Art

| Language | Approach |
|----------|----------|
| Zig | Error return traces - automatic, zero-cost in release |
| Rust | `anyhow`, `eyre` crates add backtraces to errors |
| Go | `pkg/errors` wraps with stack traces |
| Java | Exceptions carry stack traces automatically |
| Python | Exceptions carry tracebacks automatically |

Zig's approach is notable because:
- Traces are collected without heap allocation
- Minimal runtime overhead
- Works even in release builds
- No changes to function signatures

---

## Design

### Automatic Trace Collection

When `?` propagates an error, the current source location is automatically recorded:

```ori
@load (path: str) -> Result<Data, Error> = try(
    let content = read_file(path)?,  // Location recorded if Err
    let parsed = parse(content)?,     // Location recorded if Err
    Ok(parsed),
)
```

No syntax changes required. The `?` operator handles trace collection internally.

### Error Type Enhancement

The `Error` type gains an internal trace field (not directly accessible):

```ori
// Conceptual representation (actual implementation is internal)
type Error = {
    message: str,
    source: Option<Error>,
    trace: [TraceEntry],  // Internal, collected automatically
}

type TraceEntry = {
    function: str,
    file: str,
    line: int,
    column: int,
}
```

### Accessing Traces

New methods on `Error`:

```ori
impl Error {
    // Get formatted trace as string
    @trace (self) -> str

    // Get trace entries for programmatic access
    @trace_entries (self) -> [TraceEntry]

    // Check if trace is available
    @has_trace (self) -> bool
}
```

### Printing Errors with Traces

The `Printable` implementation for `Error` includes the trace:

```ori
let result = load("data.json")
match(result,
    Ok(data) -> use(data),
    Err(e) -> print(str(e)),  // Includes trace automatically
)
```

Output:
```
Error: file not found: data.json
Trace:
  read_file at std/fs.ori:142:8
  load      at src/loader.ori:15:20
  main      at src/main.ori:8:16
```

### Custom Error Types

User-defined error types can opt into trace collection by implementing a trait:

```ori
trait Traceable {
    @with_trace (self, trace: [TraceEntry]) -> Self
    @get_trace (self) -> [TraceEntry]
}

// Automatic for types containing Error
type MyError = NotFound | InvalidFormat(Error) | NetworkError(Error)
```

For simple sum types without an `Error` field, traces attach to the propagation chain rather than the error value itself.

### Result Methods

`Result` gains trace-aware methods:

```ori
impl Result<T, E> {
    // Existing: map_err transforms the error
    @map_err<F> (self, f: (E) -> F) -> Result<T, F>

    // New: add context while preserving trace
    @context (self, msg: str) -> Result<T, Error> where E: Into<Error>
}
```

Usage:
```ori
@load_config () -> Result<Config, Error> = try(
    let content = read_file("config.json")
        .context("failed to load config")?,
    let config = parse(content)
        .context("invalid config format")?,
    Ok(config),
)
```

Output on error:
```
Error: invalid config format
Caused by: unexpected token at line 15
Trace:
  parse       at std/json.ori:89:12
  load_config at src/config.ori:18:16
  main        at src/main.ori:5:20
```

---

## Examples

### Basic Error Propagation

```ori
@fetch_and_process (url: str) -> Result<Data, Error> uses Http = try(
    let response = Http.get(url)?,
    let data = parse_response(response)?,
    let validated = validate(data)?,
    Ok(validated),
)

// If validate() fails:
// Error: validation failed: missing required field 'id'
// Trace:
//   validate           at src/validation.ori:42:8
//   fetch_and_process  at src/api.ori:19:20
//   handle_request     at src/server.ori:88:12
```

### Nested Function Calls

```ori
@a () -> Result<int, Error> = b()
@b () -> Result<int, Error> = c()
@c () -> Result<int, Error> = d()
@d () -> Result<int, Error> = Err(Error { message: "deep error", source: None })

// Calling a() produces:
// Error: deep error
// Trace:
//   d at src/example.ori:4:35
//   c at src/example.ori:3:35
//   b at src/example.ori:2:35
//   a at src/example.ori:1:35
```

### Programmatic Trace Access

```ori
@report_error (e: Error) -> void = run(
    print("Error: " + e.message),

    if e.has_trace() then run(
        print("Stack trace:"),
        for entry in e.trace_entries() do
            print("  " + entry.function + " at " + entry.file + ":" + str(entry.line)),
    ) else print("(no trace available)"),
)
```

### Integration with Logging

```ori
@handle_request (req: Request) -> Result<Response, Error> uses Http, Logger = try(
    let result = process(req),

    match(result,
        Ok(resp) -> Ok(resp),
        Err(e) -> run(
            Logger.error("Request failed", {
                "error": e.message,
                "trace": e.trace(),
                "request_id": req.id,
            }),
            Err(e),
        ),
    ),
)
```

---

## Design Rationale

### Why Automatic Collection?

Manual error wrapping is:
- Verbose and repetitive
- Easy to forget
- Inconsistent across codebases

Automatic collection ensures every error path has context without developer effort.

### Why Attach to `?` Operator?

The `?` operator is the idiomatic propagation point. It's where errors "travel" up the stack, making it the natural place to record trace information.

Alternatives considered:
- Collect at `Err()` construction: Misses propagation path
- Collect everywhere: Too much overhead
- Manual `trace!()` macro: Back to boilerplate

### Why Not Full Stack Traces?

Full stack traces (like exceptions) have problems:
- Expensive to capture (stack walking)
- Include runtime internals
- Can be very deep

Error return traces only capture the `?` propagation path - the actual error handling chain. This is:
- More relevant (only error-handling code)
- Cheaper (only at `?` points)
- Cleaner output

### Why Internal Trace Storage?

Making traces internal (not a public field) allows:
- Implementation flexibility
- Future optimizations
- Consistent API across error types

The `trace()` and `trace_entries()` methods provide access without exposing internals.

---

## Implementation Notes

### Trace Storage

Options for storing trace entries:

1. **Inline array**: Fixed-size buffer in error value
   - Pro: No allocation
   - Con: Limited depth, larger error size

2. **Heap allocated**: Dynamic array
   - Pro: Unlimited depth
   - Con: Allocation on error path

3. **Thread-local ring buffer**: Zig's approach
   - Pro: Zero allocation, efficient
   - Con: More complex, traces can be overwritten

**Recommendation**: Start with heap allocation for simplicity. Optimize later if profiling shows issues.

### Compiler Changes

1. `?` operator emits trace collection code
2. Source location info available at compile time
3. Function names stored in binary (already needed for panic messages)

### Performance Considerations

- Trace collection only happens on error paths
- Error paths should be rare in well-designed programs
- Small overhead is acceptable for debugging benefits

### Debug vs Release

Unlike Zig, Ori should collect traces in all builds:
- Consistent behavior (Ori principle)
- Production errors need debugging too
- Overhead is only on error paths

If performance is critical, a future `#[no_trace]` attribute could opt out specific functions.

---

## Alternatives Considered

### 1. Keep Manual Chaining Only

**Status:** Rejected

Current `source: Option<Error>` requires explicit wrapping everywhere. Too verbose, often forgotten.

### 2. Macro-Based Approach

```ori
let data = trace!(fetch_data())?
```

**Status:** Rejected

Still requires developer action at each site. Easy to forget.

### 3. Full Exception-Style Stack Traces

Capture entire call stack at error construction.

**Status:** Rejected

- Expensive (stack walking)
- Includes irrelevant frames
- Against Ori's "explicit error handling" philosophy

### 4. Separate Trace Type

```ori
type TracedError<E> = { error: E, trace: [TraceEntry] }
```

**Status:** Rejected

- Changes function signatures
- Doesn't compose well with existing `Result<T, E>`
- Viral type changes throughout codebase

---

## Migration

This is an additive feature:
- Existing code continues to work
- Errors automatically gain traces
- No breaking changes

New code can use `.context()` and `.trace()` methods.

---

## Future Extensions

### 1. Trace Filtering

```ori
// Only show application frames, not stdlib
e.trace_entries().filter(entry -> entry.file.starts_with("src/"))
```

### 2. Structured Logging Integration

```ori
Logger.error("operation failed", { "trace": e.trace_json() })
```

### 3. Trace Compression

For long-running services, compress repeated trace patterns.

### 4. Async-Aware Traces

When `uses Async` is involved, trace across suspension points.

---

## Summary

Error return traces in Ori:

1. **Automatic** - Collected at `?` propagation points, no boilerplate
2. **Lightweight** - Only error paths, not full stack traces
3. **Accessible** - `trace()` and `trace_entries()` methods
4. **Composable** - `.context()` adds messages while preserving traces
5. **Consistent** - Same behavior in all builds

This aligns with Ori's philosophy of being explicit about errors while reducing debugging friction for both humans and AI.

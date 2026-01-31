# Proposal: App-Wide Panic Handler

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Approved:** 2026-01-31
**Affects:** Language design, runtime, compiler

---

## Summary

Add an optional app-wide `@panic` handler function that executes before program termination when a panic occurs. This provides a hook for logging, error reporting, and cleanup without enabling local recovery.

```ori
@main () -> void = run(
    start_application(),
)

@panic (info: PanicInfo) -> void = run(
    print(msg: `Fatal error: {info.message}`),
    print(msg: `Location: {info.location.file}:{info.location.line}`),
    send_to_error_tracking(info),
    cleanup_resources(),
)
```

---

## Motivation

### Current State

Ori has:
- `Result<T, E>` for expected, recoverable errors
- `panic(message)` for unrecoverable errors (bugs, invariant violations)

When `panic` is called, the program terminates. There's no way to:
- Log the panic before exit
- Send crash reports to monitoring services
- Perform graceful cleanup
- Provide user-friendly error messages

### The Problem

Production applications need crash handling:

```ori
// Current: panic just exits
@process_request (req: Request) -> Response = run(
    let data = parse(req.body),  // might panic on malformed data
    // ... if this panics, no logging, no cleanup, nothing
)
```

Operators have no visibility into crashes. Users see abrupt termination.

### Why Not Go-Style `recover()`?

Go allows catching panics anywhere with `recover()`:

```go
func risky() {
    defer func() {
        if r := recover(); r != nil {
            // recovered, continue execution
        }
    }()
    panic("oops")
}
```

Problems with local recovery:
1. **Encourages using panic for control flow** - should use `Result`
2. **Scattered recovery logic** - hard to reason about
3. **Inconsistent handling** - some panics caught, others not
4. **Violates "panic = bug" philosophy** - if you can recover, use `Result`

### The Solution: App-Wide Handler

A single, top-level handler that:
- Runs before termination (program still crashes)
- Provides crash context (message, location, stack)
- Enables logging, reporting, cleanup
- Doesn't allow "recovery" - panic is still fatal

Similar to:
- Rust's `std::panic::set_hook`
- Python's `sys.excepthook`
- Node's `process.on('uncaughtException')`
- Java's `Thread.setDefaultUncaughtExceptionHandler`

But as a **first-class language construct**, not a runtime API.

---

## Design

### The `@panic` Function

An optional top-level function with a specific signature:

```ori
@panic (info: PanicInfo) -> void = run(
    // handle the panic
)
```

**Rules:**
- At most one `@panic` function per program
- Must have signature `(PanicInfo) -> void`
- Executes synchronously before program exit
- If `@panic` itself panics, immediate termination (no recursion)

### PanicInfo Type

This proposal extends `PanicInfo` (from the additional-builtins proposal) with richer information:

```ori
type PanicInfo = {
    message: str,
    location: TraceEntry,
    stack_trace: [TraceEntry],
    thread_id: Option<int>,
}
```

The `location` field uses the existing `TraceEntry` type which has `function`, `file`, `line`, and `column` fields.

The `stack_trace` is a list of `TraceEntry` values representing the call stack at the point of panic, ordered from most recent to oldest.

The `thread_id` is `Some(id)` when the panic occurs in a concurrent context (inside `parallel`, `nursery`, or `spawn`), and `None` for single-threaded execution.

### Implicit Stderr in @panic

Inside the `@panic` handler, `print()` automatically writes to stderr instead of stdout:

```ori
@panic (info: PanicInfo) -> void = run(
    print(msg: `Crash: {info.message}`),  // Writes to stderr
)
```

This ensures panic output goes to the error stream without needing a separate `print_stderr` function.

### Default Behavior

If no `@panic` handler is defined, default behavior:

```ori
// Implicit default
@panic (info: PanicInfo) -> void = run(
    print(msg: `panic: {info.message}`),
    print(msg: `  at {info.location.file}:{info.location.line}`),
    for frame in info.stack_trace do
        print(msg: `    {frame.function}`),
)
```

### Capabilities

The `@panic` handler may declare any capability:

```ori
// OK: basic I/O for logging (Print is implicit)
@panic (info: PanicInfo) -> void = run(
    print(msg: `Crash: {info.message}`),
)

// OK: file writing
@panic (info: PanicInfo) -> void uses FileSystem = run(
    write_file(path: "/var/log/crashes.log", content: info.to_str()),
)

// OK but risky: network calls might timeout/fail
@panic (info: PanicInfo) -> void uses Http = run(
    // This could hang or fail - use with caution
    Http.post(url: "https://errors.example.com", body: info),
)
```

**Warning:** Capabilities that perform I/O (Http, FileSystem, Network) may hang, timeout, or fail. This risks the handler never completing.

**Recommendations:**
- Keep handlers simple (stderr logging is safest)
- Use short timeouts for network calls
- Fire-and-forget patterns are safer than waiting for responses

### Re-Panic Protection

If the panic handler itself panics:

```ori
@panic (info: PanicInfo) -> void = run(
    panic(msg: "oops"),  // panic inside panic handler
    // Immediate termination, no recursion
)
```

The runtime detects re-panic and terminates immediately with both panic messages.

---

## Concurrency

### First Panic Wins

When multiple tasks panic simultaneously (e.g., in a `parallel` or `nursery` context):

1. The first panic to reach the handler wins
2. Subsequent panics are recorded but do not re-run the handler
3. After the handler completes (or terminates), the program exits with the first panic's exit code
4. All pending panics are logged to stderr before exit

### Task Panics

When a task in `parallel` panics:

```ori
parallel(
    tasks: [might_panic(), other_work()],
)
```

Behavior:
1. Panicking task is cancelled
2. Sibling tasks are cancelled
3. Parent scope receives panic (propagates up)
4. `@panic` runs once at the top level (first panic wins)

### Process Isolation

With process isolation:

```ori
spawn_process(task: risky_work, input: data)
```

Each process has its own `@panic` handler. Parent process isn't affected by child panics.

---

## Examples

### Basic Logging

```ori
@panic (info: PanicInfo) -> void = run(
    print(msg: ""),
    print(msg: "=== FATAL ERROR ==="),
    print(msg: `Message: {info.message}`),
    print(msg: `Location: {info.location.file}:{info.location.line}`),
    print(msg: `Function: {info.location.function}`),
    print(msg: ""),
    print(msg: "Stack trace:"),
    for frame in info.stack_trace do
        print(msg: `  - {frame.function}`),
)
```

### Error Reporting Service

```ori
@panic (info: PanicInfo) -> void uses Http, Clock = run(
    let report = CrashReport {
        app_version: $version,
        message: info.message,
        stack: info.stack_trace,
        timestamp: Clock.now(),
    },

    // Best-effort send - might fail, that's OK
    let _ = Http.post(
        url: "https://sentry.example.com/api/crashes",
        body: report.to_json(),
        timeout: 5s,
    ),
)
```

### Graceful Cleanup

```ori
// Global resources (set during init)
let $db_connection: Option<DbConnection> = None
let $temp_files: [str] = []

@panic (info: PanicInfo) -> void uses FileSystem = run(
    print(msg: `Fatal: {info.message}`),

    // Clean up temp files
    for file in $temp_files do
        let _ = FileSystem.delete(path: file),

    // Note: can't safely close DB here if it might have caused the panic
    print(msg: "Cleanup attempted"),
)
```

### User-Friendly Message

```ori
@panic (info: PanicInfo) -> void = run(
    print(msg: ""),
    print(msg: "Oops! Something went wrong."),
    print(msg: ""),
    print(msg: "The application encountered an unexpected error and needs to close."),
    print(msg: ""),
    print(msg: "Technical details:"),
    print(msg: `  {info.message}`),
    print(msg: `  at {info.location.file}:{info.location.line}`),
    print(msg: ""),
    print(msg: "Please report this issue at: https://github.com/example/app/issues"),
)
```

### Conditional Debug Info

```ori
let $debug_mode = false

@panic (info: PanicInfo) -> void = run(
    print(msg: `Error: {info.message}`),

    if $debug_mode then run(
        print(msg: ""),
        print(msg: "Debug information:"),
        print(msg: `Location: {info.location.file}:{info.location.line}`),
        for frame in info.stack_trace do
            print(msg: `  {frame.function}`),
    ),
)
```

---

## Design Rationale

### Why a Function, Not a Block?

Alternative considered:

```ori
@main () -> void = run(
    on_panic(handler: info -> log(msg: info)),  // runtime registration
    start_app(),
)
```

Problems:
- Dynamic registration can be forgotten
- Multiple registrations unclear
- Requires runtime bookkeeping

A top-level function is:
- Declarative
- Single point of definition
- Checked at compile time

### Why `@panic` Not `@on_panic`?

Considered names:
- `@on_panic` - event handler style
- `@panic_handler` - explicit but verbose
- `@handle_panic` - verb form
- **`@panic`** - mirrors `@main`, consistent

`@panic` is clean and parallels `@main` as a special entry point.

### Why Not Allow Recovery?

Recovery would undermine Ori's error philosophy:
- `Result<T, E>` = expected error, handle it
- `panic` = bug, fix the code

If you can recover from it, it's not a panic - use `Result`.

The handler is for **observability**, not **recovery**.

---

## Implementation Notes

### Compiler Changes

1. Recognize `@panic` as special function (like `@main`)
2. Validate signature: `(PanicInfo) -> void`
3. Error if multiple `@panic` definitions
4. Generate runtime hook registration
5. Redirect `print()` to stderr within `@panic` scope

### Runtime Changes

1. Install panic hook at program start
2. On panic: construct `PanicInfo`, call handler
3. Detect re-panic, terminate immediately
4. Track first panic in concurrent context (first panic wins)
5. After handler returns, exit with non-zero code

### Stack Trace Collection

Stack traces require:
- Debug symbols (optional, for function names)
- Stack unwinding support
- Platform-specific implementation

If debug info unavailable, `stack_trace` may be empty or contain only partial information.

---

## Alternatives Considered

### 1. Result-Everywhere (No Panic Handler)

Force all errors through `Result<T, E>`.

Rejected: Some errors are truly unrecoverable (out of memory, stack overflow, assertion failures). These need panic semantics.

### 2. Try-Catch Style

```ori
try {
    risky_code()
} catch (e: Error) {
    handle(e)
}
```

Rejected: Encourages using exceptions for control flow. Ori uses `Result` for expected errors.

### 3. Multiple Handlers

```ori
@panic_io (info: PanicInfo) -> void = ...
@panic_parse (info: PanicInfo) -> void = ...
```

Rejected: Overcomplicates. One handler can dispatch internally if needed.

### 4. Runtime API Only

```ori
set_panic_hook(handler: info -> log(msg: info))
```

Rejected: Less discoverable, can be forgotten, allows multiple registrations.

---

## Summary

The `@panic` handler provides:

1. **Single location** for crash handling
2. **No recovery** - panic is still fatal
3. **Observability** - logging, reporting, cleanup
4. **Simple model** - like `@main`, a special entry point
5. **First panic wins** - deterministic behavior in concurrent contexts

```ori
@main () -> void = start_app()

@panic (info: PanicInfo) -> void = run(
    log_crash(info),
    send_report(info),
)
```

Program crashes are now visible and reportable, without compromising Ori's error handling philosophy.

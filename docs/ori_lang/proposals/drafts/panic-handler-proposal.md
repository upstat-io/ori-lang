# Proposal: App-Wide Panic Handler

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Language design, runtime, compiler

---

## Summary

Add an optional app-wide `@panic` handler function that executes before program termination when a panic occurs. This provides a hook for logging, error reporting, and cleanup without enabling local recovery.

```ori
@main () -> void = run(
    start_application(),
)

@panic (info: PanicInfo) -> void = run(
    log_error("Fatal error: {info.message}"),
    log_error("Location: {info.location}"),
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
- Cannot use capabilities that might panic (avoid infinite loops)
- Executes synchronously before program exit
- If `@panic` itself panics, immediate termination (no recursion)

### PanicInfo Type

```ori
type PanicInfo = {
    message: str,
    location: SourceLocation,
    stack_trace: [StackFrame],
    thread_id: Option<int>,  // if in concurrent context
}

type SourceLocation = {
    file: str,
    line: int,
    column: int,
    function: str,
}

type StackFrame = {
    function: str,
    location: Option<SourceLocation>,
}
```

### Default Behavior

If no `@panic` handler is defined, default behavior:

```ori
// Implicit default
@panic (info: PanicInfo) -> void = run(
    print_stderr("panic: {info.message}"),
    print_stderr("  at {info.location.file}:{info.location.line}"),
    for frame in info.stack_trace do
        print_stderr("    {frame.function}"),
)
```

### Capability Restrictions

The `@panic` handler has limited capabilities to prevent cascading failures:

```ori
// OK: basic I/O for logging
@panic (info: PanicInfo) -> void = run(
    print_stderr("Crash: {info.message}"),
)

// OK: file writing (if FileSystem allowed)
@panic (info: PanicInfo) -> void uses FileSystem = run(
    write_file("/var/log/crashes.log", str(info)),
)

// RISKY: network calls might timeout/fail
@panic (info: PanicInfo) -> void uses Http = run(
    // This could hang or fail - use with caution
    Http.post("https://errors.example.com", info),
)
```

**Recommendation:** Keep panic handlers simple. Fire-and-forget logging is safest.

### Re-Panic Protection

If the panic handler itself panics:

```ori
@panic (info: PanicInfo) -> void = run(
    panic("oops"),  // panic inside panic handler
    // Immediate termination, no recursion
)
```

The runtime detects re-panic and terminates immediately with both panic messages.

---

## Examples

### Basic Logging

```ori
@panic (info: PanicInfo) -> void = run(
    print_stderr(""),
    print_stderr("=== FATAL ERROR ==="),
    print_stderr("Message: {info.message}"),
    print_stderr("Location: {info.location.file}:{info.location.line}"),
    print_stderr("Function: {info.location.function}"),
    print_stderr(""),
    print_stderr("Stack trace:"),
    for frame in info.stack_trace do
        print_stderr("  - {frame.function}"),
)
```

### Error Reporting Service

```ori
@panic (info: PanicInfo) -> void uses Http = run(
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
$db_connection: Option<DbConnection> = None
$temp_files: [str] = []

@panic (info: PanicInfo) -> void uses FileSystem = run(
    print_stderr("Fatal: {info.message}"),

    // Clean up temp files
    for file in $temp_files do
        let _ = FileSystem.delete(file),

    // Note: can't safely close DB here if it might have caused the panic
    print_stderr("Cleanup attempted"),
)
```

### User-Friendly Message

```ori
@panic (info: PanicInfo) -> void = run(
    print_stderr(""),
    print_stderr("Oops! Something went wrong."),
    print_stderr(""),
    print_stderr("The application encountered an unexpected error and needs to close."),
    print_stderr(""),
    print_stderr("Technical details:"),
    print_stderr("  {info.message}"),
    print_stderr("  at {info.location.file}:{info.location.line}"),
    print_stderr(""),
    print_stderr("Please report this issue at: https://github.com/example/app/issues"),
)
```

### Conditional Debug Info

```ori
$debug_mode = false

@panic (info: PanicInfo) -> void = run(
    print_stderr("Error: {info.message}"),

    if $debug_mode then run(
        print_stderr(""),
        print_stderr("Debug information:"),
        print_stderr("Location: {info.location.file}:{info.location.line}"),
        for frame in info.stack_trace do
            print_stderr("  {frame.function}"),
    ),
)
```

---

## Interaction with Concurrency

### Task Panics

When a task in `parallel` panics:

```ori
parallel(
    .task1: might_panic(),
    .task2: other_work(),
)
```

Behavior:
1. Panicking task triggers its `@panic` handler
2. Sibling tasks are cancelled
3. Parent scope receives panic (propagates up)
4. `@panic` runs once at the top level

### Process Isolation

With process isolation (from concurrency proposal):

```ori
spawn_process(task: risky_work, input: data)
```

Each process has its own `@panic` handler. Parent process isn't affected by child panics.

---

## Design Rationale

### Why a Function, Not a Block?

Alternative considered:

```ori
@main () -> void = run(
    on_panic(info -> log(info)),  // runtime registration
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

### Why Limit Capabilities?

A panic means something is wrong. Using complex capabilities in the handler risks:
- Cascading failures
- Infinite loops
- Hangs

Simple handlers (stderr, file write) are safest. Network calls are possible but risky.

---

## Implementation Notes

### Compiler Changes

1. Recognize `@panic` as special function (like `@main`)
2. Validate signature: `(PanicInfo) -> void`
3. Error if multiple `@panic` definitions
4. Generate runtime hook registration

### Runtime Changes

1. Install panic hook at program start
2. On panic: construct `PanicInfo`, call handler
3. Detect re-panic, terminate immediately
4. After handler returns, exit with non-zero code

### Stack Trace Collection

Stack traces require:
- Debug symbols (optional, for function names)
- Stack unwinding support
- Platform-specific implementation

If debug info unavailable, `stack_trace` may be empty or contain only addresses.

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
set_panic_hook(info -> log(info))
```

Rejected: Less discoverable, can be forgotten, allows multiple registrations.

---

## Summary

The `@panic` handler provides:

1. **Single location** for crash handling
2. **No recovery** - panic is still fatal
3. **Observability** - logging, reporting, cleanup
4. **Simple model** - like `@main`, a special entry point

```ori
@main () -> void = start_app()

@panic (info: PanicInfo) -> void = run(
    log_crash(info),
    send_report(info),
)
```

Program crashes are now visible and reportable, without compromising Ori's error handling philosophy.

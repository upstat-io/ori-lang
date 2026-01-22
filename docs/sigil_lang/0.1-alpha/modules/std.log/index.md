# std.log

Structured logging.

```sigil
use std.log { info, warn, error, debug, Logger }
```

**Capability required:** `Logger`

---

## Overview

The `std.log` module provides:

- Leveled logging (debug, info, warn, error)
- Structured log fields
- Configurable output

---

## The Logger Capability

```sigil
trait Logger {
    @debug (message: str) -> void
    @info (message: str) -> void
    @warn (message: str) -> void
    @error (message: str) -> void
}
```

The `Logger` capability represents the ability to write log messages. Functions that perform logging must declare `uses Logger` in their signature.

```sigil
@process_order (order: Order) -> Result<void, Error> uses Logger =
    run(
        Logger.info("Processing order: " + order.id),
        // ... processing logic
        Logger.debug("Order validated"),
    )
```

**Implementations:**

| Type | Description |
|------|-------------|
| `StdoutLogger` | Logs to stdout (default) |
| `FileLogger` | Logs to a file |
| `NullLogger` | Discards all logs (for testing) |
| `CapturingLogger` | Captures logs in memory (for testing) |

### CapturingLogger

For testing code that logs:

```sigil
type CapturingLogger = {
    messages: [LogEntry],
}

type LogEntry = { level: Level, message: str }

impl Logger for CapturingLogger {
    @debug (message: str) -> void =
        self.messages = self.messages + [LogEntry { level: Debug, message: message }]

    @info (message: str) -> void =
        self.messages = self.messages + [LogEntry { level: Info, message: message }]

    @warn (message: str) -> void =
        self.messages = self.messages + [LogEntry { level: Warn, message: message }]

    @error (message: str) -> void =
        self.messages = self.messages + [LogEntry { level: Error, message: message }]
}
```

```sigil
@test_order_logging tests @process_order () -> void =
    with Logger = CapturingLogger { messages: [] } in
    run(
        process_order(test_order)?,
        assert(Logger.messages.any(e -> e.message.contains("Processing order"))),
    )
```

---

## Log Levels

| Level | Function | Use Case |
|-------|----------|----------|
| `debug` | `debug()` | Development diagnostics |
| `info` | `info()` | Normal operation events |
| `warn` | `warn()` | Potential problems |
| `error` | `error()` | Errors that need attention |

---

## Functions

### @debug

```sigil
@debug (message: str) -> void
@debug (message: str, fields: {str: str}) -> void
```

Logs at debug level.

```sigil
use std.log { debug }

debug("Processing item")
debug("Cache lookup", {"key": cache_key, "hit": str(found)})
```

---

### @info

```sigil
@info (message: str) -> void
@info (message: str, fields: {str: str}) -> void
```

Logs at info level.

```sigil
use std.log { info }

info("Server started")
info("Request handled", {"method": "GET", "path": path, "status": "200"})
```

---

### @warn

```sigil
@warn (message: str) -> void
@warn (message: str, fields: {str: str}) -> void
```

Logs at warning level.

```sigil
use std.log { warn }

warn("Deprecated API used")
warn("High memory usage", {"used_mb": str(used), "limit_mb": str(limit)})
```

---

### @error

```sigil
@error (message: str) -> void
@error (message: str, fields: {str: str}) -> void
```

Logs at error level.

```sigil
use std.log { error }

error("Database connection failed")
error("Request failed", {"error": err.message, "request_id": req_id})
```

---

## Logger Type

### Logger

```sigil
type Logger = {
    level: Level,
    output: impl Writer,
    format: Format,
}

type Level = Debug | Info | Warn | Error
type Format = Text | Json
```

Configurable logger instance.

```sigil
use std.log { Logger, Level, Format }
use std.fs { create }

let file = create("app.log")?
let logger = Logger.new()
    .level(Level.Info)
    .output(file)
    .format(Format.Json)

logger.info("Application started")
```

**Methods:**
- `new() -> Logger` — Create with defaults
- `level(l: Level) -> Logger` — Set minimum level
- `output(w: impl Writer) -> Logger` — Set output
- `format(f: Format) -> Logger` — Set format
- `with_field(key: str, value: str) -> Logger` — Add default field
- `debug(msg: str)` / `info(msg: str)` / `warn(msg: str)` / `error(msg: str)`

---

## Configuration

### @set_level

```sigil
@set_level (level: Level) -> void
```

Sets global log level.

```sigil
use std.log { set_level, Level }

set_level(Level.Debug)  // Show all logs
set_level(Level.Warn)   // Only warnings and errors
```

---

### @set_format

```sigil
@set_format (format: Format) -> void
```

Sets global log format.

```sigil
use std.log { set_format, Format }

set_format(Format.Json)  // {"level":"info","message":"...","time":"..."}
set_format(Format.Text)  // 2024-01-15 10:30:45 INFO: ...
```

---

## Output Format

### Text Format (default)

```
2024-01-15 10:30:45 INFO  Server started
2024-01-15 10:30:46 DEBUG Processing request method=GET path=/api/users
2024-01-15 10:30:47 ERROR Database error error="connection refused"
```

### JSON Format

```json
{"time":"2024-01-15T10:30:45Z","level":"info","message":"Server started"}
{"time":"2024-01-15T10:30:46Z","level":"debug","message":"Processing request","method":"GET","path":"/api/users"}
{"time":"2024-01-15T10:30:47Z","level":"error","message":"Database error","error":"connection refused"}
```

---

## Examples

### Basic logging

```sigil
use std.log { info, error, set_level, Level }

@main () uses IO -> void = run(
    set_level(Level.Info),

    info("Application starting"),

    match(initialize(),
        Ok(_) -> info("Initialized successfully"),
        Err(e) -> error("Initialization failed", {"error": e.message}),
    ),
)
```

### Request logging middleware

```sigil
use std.log { info }
use std.time { now }

@log_request (handler: Request -> Response) -> Request -> Response =
    req -> run(
        let start = now(),
        let resp = handler(req),
        let duration = now().diff(start),
        info("Request", {
            "method": str(req.method),
            "path": req.url,
            "status": str(resp.status),
            "duration_ms": str(duration.as_ms()),
        }),
        resp,
    )
```

### Custom logger

```sigil
use std.log { Logger, Level, Format }
use std.fs { open_append }

@setup_logger () uses FileSystem + IO -> Result<Logger, Error> = run(
    let file = open_append("app.log")?,
    Ok(Logger.new()
        .level(Level.Info)
        .output(file)
        .format(Format.Json)
        .with_field("app", "myservice")
        .with_field("version", "1.0.0")),
)
```

---

## See Also

- [std.io](../std.io/) — I/O operations
- [std.time](../std.time/) — Timestamps

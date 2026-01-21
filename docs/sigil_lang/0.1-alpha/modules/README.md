# Sigil Standard Library

This is the reference documentation for Sigil's standard library — the modules that ship with every Sigil installation.

---

## Overview

The standard library provides production-ready implementations for common programming tasks:

| Category | Modules | Description |
|----------|---------|-------------|
| **Core** | [prelude](prelude.md), [std](std/) | Auto-imported types and core utilities |
| **I/O** | [std.io](std.io/), [std.fs](std.fs/) | Input/output and filesystem |
| **Network** | [std.net](std.net/) | TCP/UDP, HTTP client/server |
| **Data** | [std.json](std.json/), [std.encoding](std.encoding/) | Serialization and encoding |
| **Text** | [std.fmt](std.fmt/), [std.text](std.text/) | Formatting and text processing |
| **Time** | [std.time](std.time/) | Dates, times, and durations |
| **Math** | [std.math](std.math/) | Mathematical functions |
| **Security** | [std.crypto](std.crypto/) | Cryptographic primitives |
| **System** | [std.env](std.env/), [std.process](std.process/) | Environment and processes |
| **Development** | [std.log](std.log/), [std.testing](std.testing/) | Logging and testing utilities |
| **Async** | [std.async](std.async/) | Async utilities beyond channels |
| **Collections** | [std.collections](std.collections/) | Additional collection types |
| **Compression** | [std.compress](std.compress/) | Compression algorithms |

---

## Module Naming

Sigil uses dot-separated module names:

```sigil
use std.time { Date, Time }
use std.net.http { Client, Server }
use std.encoding.base64 { encode, decode }
```

### Conventions

- **Flat hierarchy** — `std.json` not `std.encoding.json`
- **Submodules where natural** — `std.net.http`, `std.text.regex`
- **Lowercase names** — `std.time` not `std.Time`

---

## Prelude

The [prelude](prelude.md) contains items automatically imported into every Sigil program:

```sigil
// These are always available without import:
Option<T>       // Some(T) | None
Result<T, E>    // Ok(T) | Err(E)
Ordering        // Less | Equal | Greater
Error           // Standard error type

// Built-in collections
[T]             // List
{K: V}          // Map
Set<T>          // Set
Range<T>        // Range

// Core functions
print(value)    // Print to stdout
len(collection) // Get length
str(value)      // Convert to string
```

Everything else requires explicit import.

---

## Capabilities

Some modules require capabilities to use:

| Module | Capability | Reason |
|--------|------------|--------|
| `std.io` | `IO` | Reads/writes to streams |
| `std.fs` | `FileSystem` | Accesses filesystem |
| `std.net` | `Network` | Network communication |
| `std.process` | `Process` | Spawns processes |
| `std.env` | `Env` | Reads environment |

Pure modules (no capabilities required):
- `std.time` (Duration math, date calculations)
- `std.fmt` (String formatting)
- `std.text` (Text processing)
- `std.json` (Encoding/decoding)
- `std.math` (Mathematical functions)
- `std.collections` (Data structures)

See [Capabilities](../spec/14-capabilities.md) for details.

---

## Documentation Format

Each module's documentation includes:

1. **Overview** — What the module provides
2. **Types** — Type definitions with fields
3. **Functions** — Function signatures and descriptions
4. **Examples** — Working code examples
5. **Errors** — Error types that functions may return

### Example Entry

```
## @read_file

Reads a file's contents as a string.

### Signature

@read_file (path: str) -> Result<str, FileError>

### Parameters

- `path` — Path to the file to read

### Returns

- `Ok(content)` — File contents as UTF-8 string
- `Err(NotFound)` — File does not exist
- `Err(PermissionDenied)` — Cannot read file
- `Err(IsDirectory)` — Path is a directory

### Example

use std.fs { read_file }

let content = read_file("config.json")?
```

---

## Version

This documentation is for Sigil **0.1-alpha**.

See [Specification](../spec/) for language definition.
See [Design](../design/) for rationale and philosophy.

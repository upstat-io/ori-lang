# Proposal: Conditional Compilation

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Compiler, build system, language syntax

---

## Summary

Add conditional compilation support to Ori, allowing code to be included or excluded based on target platform, architecture, or custom build flags.

```ori
#[target(os: "linux")]
@get_home_dir () -> str = Env.get("HOME").unwrap_or("/root")

#[target(os: "windows")]
@get_home_dir () -> str = Env.get("USERPROFILE").unwrap_or("C:\\Users\\Default")
```

---

## Motivation

### The Problem

Cross-platform applications need platform-specific code:
- File paths differ (`/` vs `\`)
- System APIs differ (POSIX vs Win32)
- Available features differ (epoll vs kqueue vs IOCP)
- FFI bindings are platform-specific

Without conditional compilation, options are:
1. **Runtime checks** - wasteful, bloats binary with unused code
2. **Separate codebases** - maintenance nightmare
3. **Abstraction layers only** - sometimes not possible

### Use Cases

1. **OS-specific implementations**
   ```ori
   // Linux uses epoll, macOS uses kqueue, Windows uses IOCP
   @create_event_loop () -> EventLoop
   ```

2. **Architecture-specific optimizations**
   ```ori
   // SIMD on x86_64, scalar fallback elsewhere
   @vector_add (a: [float], b: [float]) -> [float]
   ```

3. **Feature flags**
   ```ori
   // Include debug logging only in debug builds
   #[cfg(debug)]
   @debug_log (msg: str) -> void = print("[DEBUG] {msg}")
   ```

4. **Optional dependencies**
   ```ori
   // Only include if SSL feature enabled
   #[cfg(feature: "ssl")]
   @connect_tls (host: str) -> TlsConnection
   ```

### Prior Art

| Language | Syntax | Scope |
|----------|--------|-------|
| Go | `// +build linux` | File-level |
| Rust | `#[cfg(target_os = "linux")]` | Item-level |
| C/C++ | `#ifdef _WIN32` | Line-level (preprocessor) |
| Zig | `if (builtin.os == .linux)` | Expression-level |
| Swift | `#if os(Linux)` | Block-level |

---

## Design

### Attribute Syntax

Use Ori's attribute syntax with `#[target(...)]` and `#[cfg(...)]`:

```ori
#[target(os: "linux")]
@linux_only () -> void = ...

#[target(os: "windows", arch: "x86_64")]
@windows_64bit_only () -> void = ...

#[cfg(debug)]
@debug_only () -> void = ...

#[cfg(feature: "async")]
type AsyncRuntime = ...
```

### Target Conditions

#### Operating System

```ori
#[target(os: "linux")]
#[target(os: "macos")]
#[target(os: "windows")]
#[target(os: "freebsd")]
#[target(os: "android")]
#[target(os: "ios")]
```

#### Architecture

```ori
#[target(arch: "x86_64")]
#[target(arch: "aarch64")]
#[target(arch: "arm")]
#[target(arch: "wasm32")]
#[target(arch: "riscv64")]
```

#### Combined Conditions

```ori
// AND: both must match
#[target(os: "linux", arch: "x86_64")]

// Multiple attributes = AND
#[target(os: "linux")]
#[target(arch: "x86_64")]
@linux_x64 () -> void = ...
```

#### Negation

```ori
#[target(not_os: "windows")]
@unix_like () -> void = ...

#[target(not_arch: "wasm32")]
@native_only () -> void = ...
```

### Configuration Flags

Beyond platform, support custom flags:

```ori
// Build mode
#[cfg(debug)]
#[cfg(release)]

// Custom features
#[cfg(feature: "ssl")]
#[cfg(feature: "async")]
#[cfg(feature: "experimental")]

// Negation
#[cfg(not: "debug")]
#[cfg(not_feature: "ssl")]
```

### Applicable Items

Conditional compilation applies to:

```ori
// Functions
#[target(os: "linux")]
@platform_func () -> void = ...

// Types
#[target(os: "windows")]
type Handle = int

// Trait implementations
#[target(os: "linux")]
impl FileDescriptor for Socket { ... }

// Config constants
#[cfg(debug)]
$log_level = "debug"

#[cfg(release)]
$log_level = "error"

// Imports
#[target(os: "linux")]
use './linux/io' { epoll_create, epoll_wait }

#[target(os: "macos")]
use './macos/io' { kqueue, kevent }
```

### File-Level Conditions

For entire files, use a module-level attribute:

```ori
// file: linux_impl.ori
#![target(os: "linux")]

// Everything in this file is Linux-only
@epoll_create () -> int = ...
@epoll_wait (fd: int) -> [Event] = ...
```

The `#!` prefix indicates file-level (inner) attribute.

### Compile-Time Constants

Access target info in code via compile-time constants:

```ori
$target_os: str      // "linux", "macos", "windows", etc.
$target_arch: str    // "x86_64", "aarch64", etc.
$debug: bool         // true in debug builds
$release: bool       // true in release builds
```

Usage:

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/"
```

**Note:** These are compile-time evaluated. Dead branches are eliminated.

---

## Examples

### Platform-Specific File Paths

```ori
#[target(os: "windows")]
@get_config_dir () -> str =
    "{Env.get("APPDATA").unwrap_or("C:\\Users\\Default\\AppData\\Roaming")}\\MyApp"

#[target(os: "macos")]
@get_config_dir () -> str =
    "{Env.get("HOME").unwrap_or("/Users/Shared")}/Library/Application Support/MyApp"

#[target(os: "linux")]
@get_config_dir () -> str =
    "{Env.get("XDG_CONFIG_HOME").unwrap_or("{Env.get("HOME").unwrap_or("/tmp")}/.config")}/myapp"
```

### Architecture-Specific Optimization

```ori
#[target(arch: "x86_64")]
@fast_checksum (data: [byte]) -> int uses Intrinsics =
    // Use SSE4.2 CRC32 instruction
    Intrinsics.crc32(data)

#[target(not_arch: "x86_64")]
@fast_checksum (data: [byte]) -> int =
    // Scalar fallback
    fold(over: data, init: 0, op: (acc, b) -> acc ^ int(b))
```

### Debug-Only Logging

```ori
#[cfg(debug)]
@debug (msg: str) -> void = print("[DEBUG] {msg}")

#[cfg(not: "debug")]
@debug (msg: str) -> void = ()  // no-op in release

// Usage - always compiles, no-op in release
@process (data: Data) -> Result<Output, Error> = run(
    debug("Processing data: {data}"),
    // ... actual processing
)
```

### Feature Flags

```ori
// In ori.toml or build config:
// [features]
// ssl = true
// async = true

#[cfg(feature: "ssl")]
use std.crypto.tls { TlsStream }

#[cfg(feature: "ssl")]
@connect_secure (host: str, port: int) -> Result<TlsStream, Error> = ...

#[cfg(not_feature: "ssl")]
@connect_secure (host: str, port: int) -> Result<Never, Error> =
    Err(Error.new("SSL support not compiled in"))
```

### Cross-Platform Module

```ori
// file: io.ori

// Re-export platform-specific implementation
#[target(os: "linux")]
pub use './io/linux' { EventLoop, Event }

#[target(os: "macos")]
pub use './io/macos' { EventLoop, Event }

#[target(os: "windows")]
pub use './io/windows' { EventLoop, Event }

// Common interface (always available)
pub trait EventSource {
    @register (self, loop: EventLoop) -> Result<void, Error>
    @unregister (self) -> void
}
```

### Test-Only Code

```ori
#[cfg(test)]
@make_test_data () -> TestData = ...

#[cfg(test)]
type MockDatabase = { ... }

#[cfg(test)]
impl Database for MockDatabase { ... }
```

---

## Build Configuration

### Command Line

```bash
# Target specification
ori build --target linux-x86_64
ori build --target macos-aarch64
ori build --target windows-x86_64

# Features
ori build --feature ssl --feature async
ori build --no-default-features --feature minimal

# Build mode (implicit cfg flags)
ori build --debug    # sets cfg(debug)
ori build --release  # sets cfg(release)

# Custom cfg flags
ori build --cfg experimental
ori build --cfg "log_level=verbose"
```

### Project Configuration

```toml
# ori.toml

[package]
name = "myapp"
version = "1.0.0"

[features]
default = ["ssl"]
ssl = []
async = ["dep:async-runtime"]
experimental = []

[target.linux]
dependencies = ["libc"]

[target.windows]
dependencies = ["winapi"]
```

---

## Compile-Time Evaluation

### Dead Code Elimination

Code in false conditions is completely eliminated:

```ori
#[target(os: "linux")]
@linux_func () -> void = ...  // Not in Windows binary

#[cfg(debug)]
$verbose_logging = true  // Not in release binary
```

### Compile Errors in Dead Code

Code in false conditions is still **parsed** but not **type-checked**:

```ori
#[target(os: "nonexistent")]
@broken () -> void =
    this_is_not_valid_ori!@#$  // Parse error, even if not compiled
```

But type errors in unused code don't trigger:

```ori
#[target(os: "windows")]
@windows_only () -> void =
    WindowsApi.call()  // Only type-checked when targeting Windows
```

---

## Design Rationale

### Why Attributes, Not Preprocessor?

C-style preprocessor (`#ifdef`):
- Text-based, not syntax-aware
- Hard to debug
- Can break syntax in subtle ways

Attributes:
- Part of the syntax tree
- IDE-friendly (can gray out inactive code)
- Type-safe

### Why Both `target` and `cfg`?

- `target` - platform/architecture conditions (well-known set)
- `cfg` - arbitrary build flags (features, modes, custom)

Separation makes intent clear:
```ori
#[target(os: "linux")]  // Platform-specific
#[cfg(feature: "ssl")]  // Feature flag
```

### Why File-Level `#!` Syntax?

Matches Rust's inner attribute convention. Keeps file-level conditions visible at the top:

```ori
#![target(os: "linux")]
// Entire file is Linux-only
```

### Why Compile-Time Constants?

Sometimes you need conditions in expressions:

```ori
let path = if $target_os == "windows" then "\\" else "/"
```

This is cleaner than:
```ori
#[target(os: "windows")]
$path_sep = "\\"
#[target(not_os: "windows")]
$path_sep = "/"
let path = $path_sep
```

Both are valid; use what's clearest.

---

## Implementation Notes

### Compiler Pipeline

1. **Parse** - All code is parsed, regardless of conditions
2. **Condition Evaluation** - Evaluate `#[target]` and `#[cfg]` against build config
3. **Filter** - Remove items with false conditions from AST
4. **Type Check** - Only check remaining items
5. **Codegen** - Generate code for remaining items

### Condition Evaluation

Conditions evaluated at compile time:
- `os` - from target triple
- `arch` - from target triple
- `debug`/`release` - from build mode
- `feature` - from build config/CLI
- Custom `cfg` - from CLI

### IDE Support

IDEs should:
- Gray out inactive code
- Show which conditions apply
- Allow switching "virtual target" for editing
- Report errors only for active conditions

---

## Alternatives Considered

### 1. Runtime Only

```ori
@get_config_dir () -> str =
    if Os.current() == "windows" then "..." else "..."
```

Rejected: Includes all code in binary, runtime overhead, can't handle type differences.

### 2. Expression-Level Conditions (Zig-style)

```ori
let x = comptime if ($os == "linux") linux_impl() else windows_impl()
```

Rejected: More complex, harder to read for item-level conditions.

### 3. Separate Files Only (Go-style)

```
io_linux.ori
io_windows.ori
io_macos.ori
```

Rejected: Sometimes too coarse. Small platform differences shouldn't require separate files.

### 4. Build Scripts

Generate platform-specific code via build scripts.

Rejected: Adds complexity, loses IDE support, harder to maintain.

---

## Future Extensions

### Complex Conditions

```ori
#[cfg(any(os: "linux", os: "macos"))]
@unix_like () -> void = ...

#[cfg(all(debug, feature: "verbose"))]
@verbose_debug () -> void = ...
```

### Version Conditions

```ori
#[cfg(ori_version: ">=1.2.0")]
@new_feature () -> void = ...
```

### Target Family

```ori
#[target(family: "unix")]   // linux, macos, freebsd, etc.
#[target(family: "windows")]
```

---

## Summary

Conditional compilation in Ori:

- **`#[target(os: "...", arch: "...")]`** - Platform conditions
- **`#[cfg(feature: "...", debug, ...)]`** - Build flags
- **`#![...]`** - File-level conditions
- **`$target_os`, `$target_arch`** - Compile-time constants

```ori
#[target(os: "linux")]
@linux_impl () -> void = ...

#[target(os: "windows")]
@windows_impl () -> void = ...

#[cfg(debug)]
@debug_log (msg: str) -> void = print("[DEBUG] {msg}")
```

Zero-cost abstraction - unused code is eliminated at compile time.

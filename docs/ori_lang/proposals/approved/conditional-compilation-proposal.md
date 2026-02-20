# Proposal: Conditional Compilation

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Approved:** 2026-01-30
**Affects:** Compiler, build system, language syntax

---

## Summary

Add conditional compilation support to Ori, allowing code to be included or excluded based on target platform, architecture, or custom build flags.

```ori
#target(os: "linux")
@get_home_dir () -> str = Env.get("HOME").unwrap_or("/root")

#target(os: "windows")
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
   #cfg(debug)
   @debug_log (msg: str) -> void = print("[DEBUG] {msg}")
   ```

4. **Optional dependencies**
   ```ori
   // Only include if SSL feature enabled
   #cfg(feature: "ssl")
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

Use Ori's attribute syntax with `#target(...)` and `#cfg(...)`:

```ori
#target(os: "linux")
@linux_only () -> void = ...

#target(os: "windows", arch: "x86_64")
@windows_64bit_only () -> void = ...

#cfg(debug)
@debug_only () -> void = ...

#cfg(feature: "async")
type AsyncRuntime = ...
```

### Target Conditions

#### Operating System

```ori
#target(os: "linux")
#target(os: "macos")
#target(os: "windows")
#target(os: "freebsd")
#target(os: "android")
#target(os: "ios")
```

#### Architecture

```ori
#target(arch: "x86_64")
#target(arch: "aarch64")
#target(arch: "arm")
#target(arch: "wasm32")
#target(arch: "riscv64")
```

#### Target Families

Target families group related operating systems:

```ori
#target(family: "unix")     // linux, macos, freebsd, openbsd, netbsd, android, ios
#target(family: "windows")  // windows
#target(family: "wasm")     // wasm32, wasm64
```

Families provide a convenient way to target platform groups without listing each OS:

```ori
#target(family: "unix")
@get_home_dir () -> str = Env.get("HOME").unwrap_or("/home")

#target(family: "windows")
@get_home_dir () -> str = Env.get("USERPROFILE").unwrap_or("C:\\Users\\Default")
```

#### Combined Conditions (AND)

Multiple conditions in one attribute require all to match:

```ori
// AND: both must match
#target(os: "linux", arch: "x86_64")

// Multiple attributes = AND
#target(os: "linux")
#target(arch: "x86_64")
@linux_x64 () -> void = ...
```

#### OR Conditions

Use `any_*` variants to match any of a list of values:

```ori
// Match any listed OS
#target(any_os: ["linux", "macos", "freebsd"])
@unix_like () -> void = ...

// Match any listed architecture
#target(any_arch: ["x86_64", "aarch64"])
@desktop_arch () -> void = ...

// Match any listed feature
#cfg(any_feature: ["ssl", "tls"])
@secure_connection () -> void = ...
```

#### Negation

Use `not_*` prefix for negation:

```ori
#target(not_os: "windows")
@non_windows () -> void = ...

#target(not_arch: "wasm32")
@native_only () -> void = ...

#target(not_family: "wasm")
@native_platform () -> void = ...

#cfg(not_debug)
@release_only () -> void = ...

#cfg(not_feature: "ssl")
@insecure_fallback () -> void = ...
```

### Configuration Flags

Beyond platform, support custom flags:

```ori
// Build mode
#cfg(debug)
#cfg(release)

// Custom features
#cfg(feature: "ssl")
#cfg(feature: "async")
#cfg(feature: "experimental")

// Negation
#cfg(not_debug)
#cfg(not_release)
#cfg(not_feature: "ssl")
```

### Feature Names

Feature names must be valid Ori identifiers:
- Start with a letter or underscore
- Contain only letters, digits, and underscores
- Case-sensitive

```ori
#cfg(feature: "ssl")           // valid
#cfg(feature: "async_io")      // valid
#cfg(feature: "_internal")     // valid
#cfg(feature: "my-feature")    // error: invalid feature name (hyphen)
#cfg(feature: "123")           // error: invalid feature name (starts with digit)
```

### Applicable Items

Conditional compilation applies to:

```ori
// Functions
#target(os: "linux")
@platform_func () -> void = ...

// Types
#target(os: "windows")
type Handle = int

// Trait implementations
#target(os: "linux")
impl FileDescriptor for Socket { ... }

// Config constants
#cfg(debug)
let $log_level = "debug"

#cfg(release)
let $log_level = "error"

// Imports
#target(os: "linux")
use "./linux/io" { epoll_create, epoll_wait }

#target(os: "macos")
use "./macos/io" { kqueue, kevent }
```

### File-Level Conditions

For entire files, use a file directive at the top:

```ori
// file: linux_impl.ori
#!target(os: "linux")

// Everything in this file is Linux-only
@epoll_create () -> int = ...
@epoll_wait (fd: int) -> [Event] = ...
```

The `#!` prefix indicates a file-level condition. It must appear before any other declarations (after comments and doc comments).

### Compile-Time Constants

Access target info in code via compile-time constants:

```ori
$target_os: str       // "linux", "macos", "windows", etc.
$target_arch: str     // "x86_64", "aarch64", etc.
$target_family: str   // "unix", "windows", "wasm"
$debug: bool          // true in debug builds
$release: bool        // true in release builds
```

Usage:

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/"
```

---

## Examples

### Platform-Specific File Paths

```ori
#target(os: "windows")
@get_config_dir () -> str =
    `{Env.get("APPDATA").unwrap_or("C:\\Users\\Default\\AppData\\Roaming")}\\MyApp`

#target(os: "macos")
@get_config_dir () -> str =
    `{Env.get("HOME").unwrap_or("/Users/Shared")}/Library/Application Support/MyApp`

#target(os: "linux")
@get_config_dir () -> str =
    `{Env.get("XDG_CONFIG_HOME").unwrap_or(`{Env.get("HOME").unwrap_or("/tmp")}/.config`)}/myapp`
```

### Architecture-Specific Optimization

```ori
#target(arch: "x86_64")
@fast_checksum (data: [byte]) -> int uses Intrinsics =
    // Use SSE4.2 CRC32 instruction
    Intrinsics.crc32(data: data)

#target(not_arch: "x86_64")
@fast_checksum (data: [byte]) -> int =
    // Scalar fallback
    data.fold(initial: 0, op: (acc, b) -> acc ^ (b as int))
```

### Debug-Only Logging

```ori
#cfg(debug)
@debug (msg: str) -> void = print(msg: `[DEBUG] {msg}`)

#cfg(not_debug)
@debug (msg: str) -> void = ()  // no-op in release

// Usage - always compiles, no-op in release
@process (data: Data) -> Result<Output, Error> = {
    debug(msg: `Processing data: {data}`)
    // ... actual processing
}
```

### Feature Flags

```ori
// In ori.toml or build config:
// [features]
// ssl = true
// async = true

#cfg(feature: "ssl")
use std.crypto.tls { TlsStream }

#cfg(feature: "ssl")
@connect_secure (host: str, port: int) -> Result<TlsStream, Error> = ...

#cfg(not_feature: "ssl")
@connect_secure (host: str, port: int) -> Result<Never, Error> =
    Err(Error.new(msg: "SSL support not compiled in"))
```

### Cross-Platform Module

```ori
// file: io.ori

// Re-export platform-specific implementation
#target(os: "linux")
pub use "./io/linux" { EventLoop, Event }

#target(os: "macos")
pub use "./io/macos" { EventLoop, Event }

#target(os: "windows")
pub use "./io/windows" { EventLoop, Event }

// Common interface (always available)
pub trait EventSource {
    @register (self, loop: EventLoop) -> Result<void, Error>
    @unregister (self) -> void
}
```

### Unix-Like Platforms

```ori
#target(family: "unix")
@get_uid () -> int = Unix.getuid()

#target(any_os: ["linux", "freebsd"])
@get_epoll_fd () -> int = ...

#target(not_family: "windows")
@use_forward_slashes () -> bool = true
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
#target(os: "linux")
@linux_func () -> void = ...  // Not in Windows binary

#cfg(debug)
let $verbose_logging = true  // Not in release binary
```

### Dead Code Elimination for Compile-Time Constants

Branches conditioned on compile-time constants are eliminated:

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/"
```

When targeting Linux, this compiles to:

```ori
@get_path_separator () -> str = "/"
```

The false branch is not type-checked. This enables platform-specific code that references types or functions only available on certain platforms:

```ori
@get_window_handle () -> WindowHandle =
    if $target_os == "windows" then
        WinApi.get_hwnd()  // Only type-checked on Windows
    else
        panic(msg: "Not supported on this platform")
```

### Compile Errors in Dead Code

Code in false conditions is still **parsed** but not **type-checked**:

```ori
#target(os: "nonexistent")
@broken () -> void =
    this_is_not_valid_ori!@#$  // Parse error, even if not compiled
```

But type errors in unused code don't trigger:

```ori
#target(os: "windows")
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
#target(os: "linux")    // Platform-specific
#cfg(feature: "ssl")    // Feature flag
```

### Why File-Level `#!` Syntax?

Keeps file-level conditions visible at the top:

```ori
#!target(os: "linux")
// Entire file is Linux-only
```

The `#!` is distinct from item-level `#` and immediately signals "this applies to the whole file."

### Why Compile-Time Constants?

Sometimes you need conditions in expressions:

```ori
let path = if $target_os == "windows" then "\\" else "/"
```

This is cleaner than:
```ori
#target(os: "windows")
let $path_sep = "\\"
#target(not_os: "windows")
let $path_sep = "/"
let path = $path_sep
```

Both are valid; use what's clearest.

### Why Unified Negation Syntax?

Using `not_*` prefix consistently:
- `not_os`, `not_arch`, `not_family` for target
- `not_debug`, `not_release`, `not_feature` for cfg

This is easier to remember than mixing prefix and wrapper styles.

### Why Include OR Conditions?

The `any_*` variants are essential for real-world cross-platform code:
- "Unix-like" (linux OR macos OR freebsd) is extremely common
- Without OR, users must duplicate code or use awkward workarounds
- Target families help but don't cover all cases

---

## Implementation Notes

### Compiler Pipeline

1. **Parse** - All code is parsed, regardless of conditions
2. **Condition Evaluation** - Evaluate `#target` and `#cfg` against build config
3. **Filter** - Remove items with false conditions from AST
4. **Type Check** - Only check remaining items
5. **Codegen** - Generate code for remaining items

### Condition Evaluation

Conditions evaluated at compile time:
- `os` - from target triple
- `arch` - from target triple
- `family` - derived from os
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

### Complex Boolean Conditions

```ori
#cfg(all(debug, feature: "verbose"))
@verbose_debug () -> void = ...

#cfg(any(debug, feature: "trace"))
@trace_enabled () -> void = ...
```

### Version Conditions

```ori
#cfg(ori_version: ">=1.2.0")
@new_feature () -> void = ...
```

### Pointer Width

```ori
#target(pointer_width: 64)
@wide_pointer () -> void = ...
```

### Endianness

```ori
#target(endian: "little")
@little_endian_only () -> void = ...
```

---

## Summary

Conditional compilation in Ori:

- **`#target(os: "...", arch: "...", family: "...")`** - Platform conditions
- **`#target(any_os: [...], any_arch: [...])`** - OR conditions
- **`#target(not_os: "...", not_family: "...")`** - Negation
- **`#cfg(feature: "...", debug, release)`** - Build flags
- **`#cfg(any_feature: [...], not_feature: "...")`** - Feature OR/negation
- **`#!target(...)`** - File-level conditions
- **`$target_os`, `$target_arch`, `$target_family`** - Compile-time constants
- **`$debug`, `$release`** - Build mode constants

```ori
#target(os: "linux")
@linux_impl () -> void = ...

#target(os: "windows")
@windows_impl () -> void = ...

#target(family: "unix")
@unix_impl () -> void = ...

#cfg(debug)
@debug_log (msg: str) -> void = print(msg: `[DEBUG] {msg}`)
```

Zero-cost abstraction - unused code is eliminated at compile time.

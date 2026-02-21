---
title: "Conditional Compilation"
description: "Ori Language Specification — Conditional Compilation"
order: 25
section: "Conditional Compilation"
---

# Conditional Compilation

Conditional compilation enables code to be included or excluded based on target platform, architecture, or build configuration.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § ATTRIBUTES

## Overview

Two attribute forms control conditional compilation:

| Attribute | Purpose |
|-----------|---------|
| `#target(...)` | Platform and architecture conditions |
| `#cfg(...)` | Build configuration flags |

Code in false conditions is parsed but not type-checked. Code in true conditions is type-checked and compiled.

## Target Conditions

### Operating System

```ori
#target(os: "linux")
#target(os: "macos")
#target(os: "windows")
#target(os: "freebsd")
#target(os: "android")
#target(os: "ios")
```

### Architecture

```ori
#target(arch: "x86_64")
#target(arch: "aarch64")
#target(arch: "arm")
#target(arch: "wasm32")
#target(arch: "riscv64")
```

### Target Families

Target families group related operating systems:

| Family | Operating Systems |
|--------|-------------------|
| `unix` | linux, macos, freebsd, openbsd, netbsd, android, ios |
| `windows` | windows |
| `wasm` | wasm32, wasm64 |

```ori
#target(family: "unix")
@get_home_dir () -> str = Env.get("HOME").unwrap_or("/home");
```

### Combined Conditions (AND)

Multiple conditions in one attribute require all to match:

```ori
#target(os: "linux", arch: "x86_64")
@linux_x64_only () -> void = ...;
```

Multiple attributes also combine with AND:

```ori
#target(os: "linux")
#target(arch: "x86_64")
@linux_x64_only () -> void = ...;
```

### OR Conditions

The `any_*` variants match any value in a list:

```ori
#target(any_os: ["linux", "macos", "freebsd"])
@unix_like () -> void = ...;

#target(any_arch: ["x86_64", "aarch64"])
@desktop_arch () -> void = ...;
```

### Negation

The `not_*` prefix negates a condition:

```ori
#target(not_os: "windows")
@non_windows () -> void = ...;

#target(not_arch: "wasm32")
@native_only () -> void = ...;

#target(not_family: "wasm")
@native_platform () -> void = ...;
```

## Configuration Flags

### Build Mode

```ori
#cfg(debug)
@debug_only () -> void = ...;

#cfg(release)
@release_only () -> void = ...;

#cfg(not_debug)
@optimized () -> void = ...;
```

### Feature Flags

```ori
#cfg(feature: "ssl")
@secure_connect () -> void = ...;

#cfg(feature: "async")
type AsyncRuntime = ...;

#cfg(not_feature: "ssl")
@insecure_fallback () -> void = ...;
```

Feature names must be valid Ori identifiers:
- Start with a letter or underscore
- Contain only letters, digits, and underscores
- Case-sensitive

### OR for Features

```ori
#cfg(any_feature: ["ssl", "tls"])
@secure_connection () -> void = ...;
```

## Applicable Items

Conditional compilation applies to:

| Item | Example |
|------|---------|
| Functions | `#target(os: "linux") @platform_func () -> void` |
| Types | `#target(os: "windows") type Handle = int` |
| Trait implementations | `#target(os: "linux") impl FileDescriptor for Socket` |
| Constants | `#cfg(debug) let $log_level = "debug"` |
| Imports | `#target(os: "linux") use "./linux/io" { epoll_create }` |

## File-Level Conditions

The `#!` prefix applies a condition to the entire file:

```ori
#!target(os: "linux")

// Everything in this file is Linux-only
@epoll_create () -> int = ...;
@epoll_wait (fd: int) -> [Event] = ...;
```

File-level conditions must appear before any declarations (after comments and doc comments).

## Compile-Time Constants

Target information is available as compile-time constants:

| Constant | Type | Description |
|----------|------|-------------|
| `$target_os` | `str` | Operating system ("linux", "macos", etc.) |
| `$target_arch` | `str` | Architecture ("x86_64", "aarch64", etc.) |
| `$target_family` | `str` | Target family ("unix", "windows", "wasm") |
| `$debug` | `bool` | True in debug builds |
| `$release` | `bool` | True in release builds |

### Usage in Expressions

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/";
```

Branches conditioned on compile-time constants are eliminated. The false branch is not type-checked:

```ori
@get_window_handle () -> WindowHandle =
    if $target_os == "windows" then
        WinApi.get_hwnd()  // Only type-checked on Windows
    else
        panic(msg: "Not supported");
```

## Compilation Semantics

### Dead Code Elimination

Code in false conditions is completely eliminated from the binary:

```ori
#target(os: "linux")
@linux_func () -> void = ...;  // Not in Windows binary

#cfg(debug)
let $verbose = true;  // Not in release binary
```

### Parse vs Type-Check

Code in false conditions is:
- **Parsed**: Syntax errors are reported regardless of condition
- **Not type-checked**: Type errors in unused code do not trigger

```ori
#target(os: "nonexistent")
@broken () -> void =
    this_is_not_valid!@#$;  // Parse error, even if not compiled

#target(os: "windows")
@windows_only () -> void =
    WindowsApi.call();  // Only type-checked when targeting Windows
```

## Command Line

```bash
# Target specification
ori build --target linux-x86_64
ori build --target macos-aarch64

# Features
ori build --feature ssl --feature async
ori build --no-default-features --feature minimal

# Build mode
ori build --debug    # sets cfg(debug)
ori build --release  # sets cfg(release)

# Custom cfg flags
ori build --cfg experimental
```

## Project Configuration

```toml
# ori.toml

[package]
name = "myapp"
version = "1.0.0"

[features]
default = ["ssl"]
ssl = []
async = ["dep:async-runtime"]

[target.linux]
dependencies = ["libc"]

[target.windows]
dependencies = ["winapi"]
```

## Error Codes

| Code | Description |
|------|-------------|
| E0930 | Invalid target OS |
| E0931 | Invalid target architecture |
| E0932 | Invalid feature name |
| E0933 | File-level condition must be first |

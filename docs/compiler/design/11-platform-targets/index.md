---
title: "Platform Targets"
description: "Building Ori for different platforms — Native and WebAssembly"
order: 11
---

# Platform Targets

The Ori compiler supports multiple target platforms through Rust's conditional compilation system. The primary targets are:

| Target | Use Case | Stack Management |
|--------|----------|------------------|
| **Native** (x86_64, aarch64) | CLI, desktop applications | Dynamic via `stacker` crate |
| **WebAssembly** (wasm32) | Browser playground, Node.js | Fixed with configurable limit |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    ori_eval crate                        │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────────┐   ┌─────────────────────────┐  │
│  │   Native Build      │   │     WASM Build          │  │
│  │   #[cfg(not(wasm))] │   │   #[cfg(wasm32)]        │  │
│  ├─────────────────────┤   ├─────────────────────────┤  │
│  │ • stacker crate     │   │ • Fixed stack (~1MB)    │  │
│  │ • Dynamic growth    │   │ • Configurable limit    │  │
│  │ • No recursion cap  │   │ • Default: 200 calls    │  │
│  │ • Deep recursion OK │   │ • Runtime configurable  │  │
│  └─────────────────────┘   └─────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Key Differences

### Stack Management

**Native builds** use the `stacker` crate (`ori_stack`) to dynamically grow the stack when needed. This allows arbitrarily deep recursion limited only by available memory.

**WASM builds** have a fixed stack (~1MB in browsers) that cannot grow. Without protection, deep recursion causes cryptic errors like "Maximum call stack size exceeded" or "memory access out of bounds".

### Recursion Limiting

To provide a good developer experience on WASM, the interpreter tracks call depth and fails gracefully before exhausting the stack:

```
[runtime] maximum recursion depth exceeded (WASM limit: 200)
```

This limit is:
- **Configurable** via the JavaScript API
- **Runtime-adjustable** per execution
- **Disabled on native** (no artificial limit)

## Documentation Sections

- [Conditional Compilation](conditional-compilation.md) - Platform-specific code patterns
- [WASM Target](wasm-target.md) - WebAssembly-specific considerations
- [Recursion Limits](recursion-limits.md) - Stack safety implementation

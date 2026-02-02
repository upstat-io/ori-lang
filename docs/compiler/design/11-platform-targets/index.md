---
title: "Platform Targets"
description: "Building Ori for different platforms — Native and WebAssembly"
order: 11
---

# Platform Targets

Ori supports multiple target platforms in two ways:

1. **Interpreter portability**: The `ori_eval` interpreter compiles to native and WASM via Rust's conditional compilation
2. **Cross-compilation**: The LLVM backend compiles Ori programs to 10 officially supported targets

## Supported Compilation Targets (LLVM Backend)

| Target Triple | Platform |
|---------------|----------|
| `x86_64-unknown-linux-gnu` | 64-bit Linux (glibc) |
| `x86_64-unknown-linux-musl` | 64-bit Linux (musl, static) |
| `aarch64-unknown-linux-gnu` | ARM64 Linux (glibc) |
| `aarch64-unknown-linux-musl` | ARM64 Linux (musl, static) |
| `x86_64-apple-darwin` | Intel macOS |
| `aarch64-apple-darwin` | Apple Silicon macOS |
| `x86_64-pc-windows-msvc` | 64-bit Windows (MSVC) |
| `x86_64-pc-windows-gnu` | 64-bit Windows (MinGW) |
| `wasm32-unknown-unknown` | Standalone WebAssembly |
| `wasm32-wasi` | WebAssembly with WASI |

See `ori_llvm/src/aot/target.rs` for implementation.

## Interpreter Platforms

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

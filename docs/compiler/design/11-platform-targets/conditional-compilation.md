---
title: "Conditional Compilation"
description: "Platform-specific code using Rust's cfg attributes"
order: 1
---

# Conditional Compilation

The Ori compiler uses Rust's `#[cfg]` attributes to compile different code paths for different target platforms.

## Target Detection

The primary target attribute used is `target_arch`:

```rust
#[cfg(target_arch = "wasm32")]        // WebAssembly (browser, Node.js, etc.)
#[cfg(not(target_arch = "wasm32"))]   // Native (x86_64, aarch64, etc.)
```

## Patterns Used

### Constants

Platform-specific constants are defined only for the relevant target:

```rust
// Only exists in WASM builds
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_MAX_CALL_DEPTH: usize = 200;
```

### Imports

Conditional imports prevent unused code warnings:

```rust
#[cfg(target_arch = "wasm32")]
use ori_patterns::recursion_limit_exceeded;
```

### Struct Fields

Fields can be added only for specific targets:

```rust
pub struct Interpreter<'a> {
    // Common fields...
    pub(crate) call_depth: usize,

    // WASM-only field
    #[cfg(target_arch = "wasm32")]
    pub(crate) max_call_depth: usize,
}
```

### Method Implementations

When methods differ significantly, use separate implementations:

```rust
impl Interpreter<'_> {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        if self.call_depth >= self.max_call_depth {
            Err(recursion_limit_exceeded(self.max_call_depth))
        } else {
            Ok(())
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[expect(clippy::unused_self, clippy::unnecessary_wraps,
        reason = "API parity with WASM version")]
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        Ok(())  // No-op on native
    }
}
```

### Builder Methods

Methods that only make sense for certain targets:

```rust
impl InterpreterBuilder<'_> {
    /// Only available on WASM builds
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn max_call_depth(mut self, limit: usize) -> Self {
        self.max_call_depth = limit;
        self
    }
}
```

## The `ori_stack` Crate

The `ori_stack` crate provides a unified API for stack management:

```rust
// In ori_stack/src/lib.rs

#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

#[cfg(target_arch = "wasm32")]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    f()  // No-op on WASM
}
```

This allows the interpreter to call `ensure_sufficient_stack(|| ...)` without caring about the target platform.

## Best Practices

### 1. Maintain API Parity

When a method exists on one platform, provide a compatible (possibly no-op) version on the other:

```rust
// WASM: actual implementation
#[cfg(target_arch = "wasm32")]
fn check_recursion_limit(&self) -> Result<(), EvalError> { ... }

// Native: no-op with same signature
#[cfg(not(target_arch = "wasm32"))]
fn check_recursion_limit(&self) -> Result<(), EvalError> { Ok(()) }
```

### 2. Document Platform Differences

Use doc comments to explain platform-specific behavior:

```rust
/// Maximum call depth for WASM builds.
///
/// On native builds, `stacker` handles deep recursion by growing the stack,
/// so this limit is not enforced.
#[cfg(target_arch = "wasm32")]
pub const DEFAULT_MAX_CALL_DEPTH: usize = 200;
```

### 3. Use `#[expect]` for Intentional Lint Violations

When API parity causes clippy warnings, explain why:

```rust
#[expect(
    clippy::unused_self,
    reason = "API parity with WASM version which uses self.max_call_depth"
)]
```

### 4. Test Both Targets

The CI should build and test for both native and WASM:

```bash
# Native
cargo test --workspace

# WASM
cargo check --target wasm32-unknown-unknown -p ori_eval
wasm-pack build --target web
```

## Crates with Platform-Specific Code

| Crate | Platform Code | Purpose |
|-------|--------------|---------|
| `ori_stack` | Stack management | `stacker` vs no-op |
| `ori_eval` | Recursion limits | Call depth tracking |
| `ori_llvm` | Target/linker selection | Cross-compilation support |
| `playground-wasm` | WASM bindings | JavaScript interop |

### LLVM Backend Platform Code

The `ori_llvm` crate uses target detection for linker selection and sysroot discovery:

```rust
// In ori_llvm/src/aot/syslib.rs
#[cfg(target_arch = "x86_64")]
fn default_lib_dirs() -> Vec<PathBuf> { /* x86_64 paths */ }

#[cfg(target_arch = "aarch64")]
fn default_lib_dirs() -> Vec<PathBuf> { /* aarch64 paths */ }

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn default_lib_dirs() -> Vec<PathBuf> { vec![] }
```

Platform-specific linker drivers are selected based on target triple, not host architecture:

```rust
pub fn driver_for_target(triple: &str) -> Box<dyn LinkerDriver> {
    match triple {
        t if t.contains("windows-msvc") => Box::new(MsvcLinker::new()),
        t if t.contains("wasm32") => Box::new(WasmLinker::new()),
        _ => Box::new(GccLinker::new()),
    }
}
```

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

The interpreter uses a unified `CallStack` struct that replaces the old `call_depth`/`max_call_depth` pattern. The `CallStack` stores `CallFrame` entries (function name + call site span) and an `Option<usize>` depth limit. Platform differences are handled at the limit level: `EvalMode::max_recursion_depth()` returns `None` on native (stacker grows the stack) and `Some(200)` on WASM.

```rust
pub struct Interpreter<'a> {
    // Common fields...
    pub(crate) call_stack: CallStack,
}
```

### Method Implementations

The recursion limit check is unified across platforms. Rather than separate `#[cfg]` implementations, the interpreter delegates to `EvalMode::max_recursion_depth()`, which uses `#[cfg]` internally to return the appropriate limit:

```rust
impl Interpreter<'_> {
    pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
        if let Some(max_depth) = self.mode.max_recursion_depth() {
            if self.call_stack.depth() >= max_depth {
                return Err(recursion_limit_exceeded(max_depth));
            }
        }
        Ok(())
    }
}
```

### Builder Methods

The `InterpreterBuilder` accepts a `CallStack` via its `.call_stack()` method, which is platform-independent. The caller constructs the `CallStack` with the appropriate depth limit:

```rust
impl InterpreterBuilder<'_> {
    #[must_use]
    pub fn call_stack(mut self, call_stack: CallStack) -> Self {
        self.call_stack = call_stack;
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

### 1. Prefer Unified Code with cfg-Gated Data

When possible, write a single implementation that branches on data rather than duplicating implementations with `#[cfg]`. The `check_recursion_limit()` pattern demonstrates this: one method body, with the platform difference pushed into the limit value (`Option<usize>` from `EvalMode::max_recursion_depth()`).

For cases where the implementation truly diverges, provide API parity across platforms:

```rust
// Platform-divergent: ensure_sufficient_stack (fundamentally different behavior)
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

#[cfg(target_arch = "wasm32")]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    f()  // No-op on WASM — same signature, different behavior
}
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
// In ori_llvm/src/aot/syslib/mod.rs
fn detect_library_paths(
    target: &TargetTripleComponents,
    sysroot: Option<&PathBuf>,
) -> Vec<PathBuf> { /* target-aware path detection */ }
```

Linker selection uses `LinkerFlavor::for_target()`, an enum-based dispatch that selects the appropriate `LinkerImpl` variant (no trait objects):

```rust
pub enum LinkerFlavor { Gcc, Lld, Msvc, WasmLd }

impl LinkerFlavor {
    pub fn for_target(target: &TargetTripleComponents) -> Self {
        if target.is_wasm() { Self::WasmLd }
        else if target.is_windows() && target.env.as_deref() == Some("msvc") { Self::Msvc }
        else { Self::Gcc }
    }
}

pub enum LinkerImpl {
    Gcc(GccLinker),
    Msvc(MsvcLinker),
    Wasm(WasmLinker),
}
```

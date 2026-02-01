---
title: "WASM Target"
description: "WebAssembly-specific considerations for the Ori interpreter"
order: 2
---

# WASM Target

The Ori interpreter can be compiled to WebAssembly for embedding in browsers, Node.js, Deno, Wasmtime, Wasmer, and other WASM runtimes.

## Building for WASM

```bash
cargo build --target wasm32-unknown-unknown -p ori_eval
```

Or with wasm-pack for JavaScript bindings:

```bash
wasm-pack build --target web --release
```

## Portable Crate Design

The following crates are WASM-compatible (no Salsa dependency):

```
ori_eval ──┬── ori_patterns
           ├── ori_ir
           └── ori_stack (no-op on WASM)

ori_typeck ─┬── ori_types
            └── ori_parse ── ori_lexer

ori_fmt
```

**Not WASM-compatible:** `oric` (uses Salsa, which requires `Arc<Mutex<T>>`)

## WASM Limitations

### Fixed Stack Size

WASM runtimes have fixed stack sizes that cannot grow dynamically:

| Runtime | Typical Stack | Recommended Limit |
|---------|---------------|-------------------|
| Browsers | ~1MB | 200 calls |
| Node.js | Larger | 500+ calls |
| Wasmtime/Wasmer | Configurable | 1000+ calls |
| Embedded | Limited | 50-100 calls |

Each Ori function call consumes multiple WASM stack frames:

```
eval() → eval_inner() → eval_call() → create_interpreter() → eval()
```

This multiplication (~5-10x) means 200 Ori calls ≈ 1000-2000 WASM frames.

### No Salsa Queries

Salsa requires thread-safe interiors (`Arc<Mutex<T>>`) which don't work reliably in single-threaded WASM. WASM builds must use direct interpreter calls rather than cached queries.

### No Environment Variables

WASM cannot read environment variables at runtime. All configuration must be:
- Compile-time constants (`const`)
- Runtime parameters passed from the host

### No Native Capabilities

WASM sandboxing means these capabilities require host integration:
- `FileSystem` - Must be provided by host
- `Http` - Must use fetch API or host binding
- `Clock` - Must use host time APIs

## Stack Frame Cost

Why the conservative default limit? Each Ori call involves:

1. `eval()` - Expression evaluation entry
2. `eval_inner()` - Main dispatch
3. `eval_call()` - Function call handling
4. `create_function_interpreter()` - Child interpreter setup
5. `InterpreterBuilder::build()` - Builder pattern
6. Back to `eval()` for the function body

That's 5-6 Rust stack frames per Ori function call, plus frames for:
- Pattern matching in the body
- Binary/unary operators
- Method calls

A safe estimate is **5-10 WASM frames per Ori call**.

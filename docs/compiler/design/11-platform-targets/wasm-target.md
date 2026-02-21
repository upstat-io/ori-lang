---
title: "WASM Target"
description: "WebAssembly support in Ori - interpreter and compilation target"
order: 2
---

# WASM Target

Ori supports WebAssembly in two distinct ways:

1. **Interpreter in WASM**: The Ori interpreter (`ori_eval`) compiles to WASM for embedding in browsers (Playground)
2. **Compiling to WASM**: The LLVM backend can compile Ori programs to `.wasm` files

## Compiling Ori to WebAssembly (LLVM Backend)

The LLVM backend supports two WebAssembly targets:

| Target | Description |
|--------|-------------|
| `wasm32-unknown-unknown` | Standalone WebAssembly (no host APIs) |
| `wasm32-wasi` | WebAssembly with WASI (file system, stdio, etc.) |

### Building WASM Binaries

```bash
# Compile to standalone WASM
ori build --target wasm32-unknown-unknown program.ori

# Compile with WASI support
ori build --target wasm32-wasi program.ori
```

### WASM-Specific Configuration

The `WasmConfig` struct controls WASM-specific options:

```rust
pub struct WasmConfig {
    /// Memory configuration (import vs export, initial/max pages).
    pub memory: WasmMemoryConfig,
    /// Stack configuration (size in bytes, separate from memory).
    pub stack: WasmStackConfig,
    /// Output generation options (JS bindings, TypeScript decls, wasm-opt).
    pub output: WasmOutputOptions,
    /// Enable WASI support.
    pub wasi: bool,
    /// WASI-specific configuration (when wasi is true).
    pub wasi_config: Option<WasiConfig>,
    /// WebAssembly feature flags.
    pub features: WasmFeatures,
}
```

`WasmStackConfig` is separate from `WasmMemoryConfig` because WASM stack size is a linker argument (`--stack-size`), not a memory import/export setting. The default is 1MB.

`WasmFeatures` controls target feature flags (SIMD, bulk memory, reference types, multi-value, exception handling). These map to `wasm-ld --enable-*` flags. `WasmFeatures::default_enabled()` turns on `bulk_memory` and `multi_value`.

`WasiConfig` configures WASI capability access (filesystem, clock, random, environment variables, command-line arguments) and preopened directory mappings. Factory methods `WasiConfig::cli()` and `WasiConfig::minimal()` provide common configurations.

`WasmOutputOptions` controls post-compilation output: `generate_js_bindings` and `generate_dts` for JavaScript/TypeScript binding generation, and `run_wasm_opt` / `wasm_opt_level` for Binaryen wasm-opt post-processing (optimization levels O0-O4, Os, Oz).

See `ori_llvm/src/aot/wasm.rs` for implementation details.

---

## Interpreter in WASM (Playground)

The Ori interpreter can be compiled to WebAssembly for embedding in browsers, Node.js, Deno, Wasmtime, Wasmer, and other WASM runtimes.

### Building the Interpreter for WASM

```bash
cargo build --target wasm32-unknown-unknown -p ori_eval
```

Or with wasm-pack for JavaScript bindings:

```bash
wasm-pack build --target web --release
```

### Portable Crate Design

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
eval_can() → eval_can_inner() → eval_call() → create_interpreter() → eval_can()
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

1. `eval_can()` - Canonical IR evaluation entry (`can_eval.rs`)
2. `eval_can_inner()` - Main dispatch
3. `eval_call()` - Function call handling (`function_call.rs:28`)
4. `create_function_interpreter()` - Child interpreter setup
5. `InterpreterBuilder::build()` - Builder pattern
6. Back to `eval_can()` for the function body

That's 5-6 Rust stack frames per Ori function call, plus frames for:
- Pattern matching in the body
- Binary/unary operators
- Method calls

A safe estimate is **5-10 WASM frames per Ori call**.

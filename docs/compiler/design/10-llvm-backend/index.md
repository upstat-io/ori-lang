---
title: "LLVM Backend Overview"
description: "LLVM backend architecture for JIT compilation and native code generation"
order: 1
section: "LLVM Backend"
---

# LLVM Backend Overview

The LLVM backend (`ori_llvm` crate) provides JIT compilation and native code generation for Ori programs. It translates the typed AST directly to LLVM IR, bypassing the tree-walking interpreter for improved performance.

## Architecture

The backend follows patterns from `rustc_codegen_llvm`:

```
                                        ┌──────────────────────┐
                                        │   SimpleCx (scx)     │
                                        │   - LLVM Context     │
                                        │   - Type cache       │
                                        │   - Target machine   │
┌─────────────────┐    ┌────────────────┴──────────────────────┤
│  Typed AST      │───►│        CodegenCx (cx)                 │
│  (TypedModule)  │    │   - SimpleCx reference                │
└─────────────────┘    │   - LLVM Module                       │
                       │   - String interner                   │
                       │   - Type mappings                     │
                       └────────────────┬──────────────────────┘
                                        │
                       ┌────────────────▼──────────────────────┐
                       │         Builder                       │
                       │   - LLVM IRBuilder                    │
                       │   - CodegenCx reference               │
                       │   - Current basic block               │
                       │   - Expression compilation            │
                       └───────────────────────────────────────┘
```

### Context Hierarchy

| Type | Lifetime | Purpose |
|------|----------|---------|
| `SimpleCx` | Process-wide | LLVM context, target machine, cached types |
| `CodegenCx` | Per-module | LLVM module, string interner, function declarations |
| `Builder` | Per-function | IR building, local variable tracking |

## Type Mappings

Ori types map to LLVM types as follows:

| Ori Type | LLVM Type | Notes |
|----------|-----------|-------|
| `int` | `i64` | 64-bit signed integer |
| `float` | `f64` | 64-bit IEEE 754 |
| `bool` | `i1` | 1-bit boolean |
| `byte` | `i8` | 8-bit unsigned |
| `str` | `{ i64, ptr }` | Length + data pointer |
| `[T]` | `{ i64, i64, ptr }` | Length, capacity, data pointer |
| `Option<T>` | `{ i8, T }` | Tag (0=None, 1=Some) + payload |
| `Result<T, E>` | `{ i8, payload }` | Tag (0=Ok, 1=Err) + payload |
| `(A, B, ...)` | `{ A, B, ... }` | Anonymous struct |
| User structs | Named `{ fields... }` | Registered via `StructLayout` (see [User Types](user-types.md)) |
| Closures | `i64` | Tagged pointer (see [Closures](closures.md)) |

## Compilation Phases

The backend uses a two-phase approach:

### Phase 1: Declaration

All functions are declared before any are defined. This enables mutual recursion without forward declaration syntax.

```rust
// Declare all functions first
for func in module.functions() {
    declare_function(func);
}

// Then define function bodies
for func in module.functions() {
    define_function(func);
}
```

### Phase 2: Definition

Each function body is compiled:

1. Create entry basic block
2. Bind parameters to LLVM values
3. Compile function body expression
4. Build return instruction

## Runtime Functions

The backend links against runtime functions for operations that require heap allocation or complex logic. These are Rust functions with `extern "C"` ABI:

| Function | Purpose |
|----------|---------|
| `ori_print`, `ori_print_int`, etc. | Output |
| `ori_str_concat` | String concatenation |
| `ori_str_eq`, `ori_str_ne` | String comparison |
| `ori_list_new`, `ori_list_free` | List allocation |
| `ori_closure_box` | Closure heap allocation |
| `ori_panic`, `ori_panic_cstr` | Panic handling |
| `ori_assert*` | Assertion variants |

See [runtime.rs](../../../compiler/ori_llvm/src/runtime.rs) for the complete list.

## Documentation Sections

- [Closures](closures.md) - Closure representation and calling conventions
- [User-Defined Types](user-types.md) - Struct types, impl blocks, and method dispatch

## Source Files

| File | Purpose |
|------|---------|
| `context.rs` | `SimpleCx`, `CodegenCx`, `StructLayout`, `TypeCache` |
| `builder.rs` | `Builder` type and IR generation helpers |
| `declare.rs` | Function declaration phase |
| `module.rs` | Module-level compilation, struct registration |
| `evaluator.rs` | JIT evaluation, module loading orchestration |
| `functions/` | Function body compilation |
| `functions/calls.rs` | Function and method call compilation |
| `collections/` | Collection type handling (lists, maps, tuples) |
| `collections/structs.rs` | Struct literals and field access |
| `control_flow.rs` | If/else, loops, match |
| `operators.rs` | Binary and unary operators |
| `runtime.rs` | Runtime function definitions |
| `types.rs` | Ori-to-LLVM type mapping |

## Development

The LLVM crate requires Docker for building and testing due to LLVM library dependencies:

```bash
./llvm-build    # Build the crate
./llvm-test     # Run unit tests
./llvm-clippy   # Run clippy
```

Formatting works without Docker:

```bash
cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml
```

## Status

- JIT execution: Working (734/753 tests passing)
- AOT compilation: Pending

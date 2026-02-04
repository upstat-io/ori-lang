---
title: "LLVM Backend Overview"
description: "LLVM backend architecture for JIT compilation and native code generation"
order: 1
section: "LLVM Backend"
---

# LLVM Backend Overview

The LLVM backend (`ori_llvm` crate) provides both JIT compilation and AOT (Ahead-of-Time) native code generation for Ori programs. It translates the typed AST directly to LLVM IR.

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

## Compilation Modes

### JIT Compilation

JIT execution compiles and runs code immediately in the same process:

```rust
let evaluator = LlvmEvaluator::new(db)?;
let result = evaluator.evaluate_file(source)?;
```

### AOT Compilation

AOT compilation generates native executables or libraries. See [AOT Compilation](aot.md) for details.

```rust
let target = TargetConfig::native()?;
let emitter = ObjectEmitter::new(&target)?;
emitter.emit_object(&module, Path::new("output.o"))?;

let driver = LinkerDriver::new(&target);
driver.link(LinkInput { ... })?;
```

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

## Control Flow Compilation

### Short-Circuit Operators

Logical `&&` and `||` operators use short-circuit evaluation with proper basic block structure:

```
// Compiling: left && right
                    ┌──────────┐
                    │  entry   │
                    │ eval left│
                    └────┬─────┘
                         │
              ┌──false───┴───true──┐
              ▼                    ▼
        ┌──────────┐         ┌──────────┐
        │  merge   │         │ and_rhs  │
        │(phi=false)◄────────│eval right│
        └──────────┘         └──────────┘
```

The implementation handles edge cases where the right operand may terminate (e.g., `panic()`).

### Conditionals

If/else expressions create three basic blocks (then, else, merge) with PHI nodes for value-producing branches. Terminating branches (panic, return, break) skip the merge jump.

### Loops

Loop compilation creates structured basic blocks with proper control flow:

**Infinite loops (`loop(...)`)**:
```
entry → header → body → header (or exit via break)
```

**For loops** use a four-block structure with a dedicated latch block:
```
                    ┌──────────┐
                    │  entry   │
                    │(init idx)│
                    └────┬─────┘
                         │
                    ┌────▼─────┐◄─────────┐
                    │  header  │          │
                    │(idx<len?)│          │
                    └────┬─────┘          │
              ┌─false────┴───true─┐       │
              ▼                   ▼       │
        ┌──────────┐        ┌──────────┐  │
        │   exit   │        │   body   │  │
        └──────────┘        │(loop code)│  │
                            └────┬─────┘  │
                                 │        │
                            ┌────▼─────┐  │
                            │  latch   │──┘
                            │ (idx++)  │
                            └──────────┘
```

**Critical:** `continue` jumps to the latch block (which increments the index), not the header. Jumping directly to the header would create an infinite loop on the same element.

Loop context tracks continue and exit targets for nested control flow:
```rust
let for_loop_ctx = LoopContext {
    header: latch_bb,  // continue → latch (increment then check)
    exit: exit_bb,     // break → exit
    break_phi: None,
};
```

## Runtime Functions

The backend links against runtime functions for operations that require heap allocation or complex logic. These are provided by `libori_rt`:

| Category | Functions |
|----------|-----------|
| Output | `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` |
| Strings | `ori_str_concat`, `ori_str_eq`, `ori_str_ne`, `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float` |
| Collections | `ori_list_new`, `ori_list_free`, `ori_list_len` |
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| Reference Counting | `ori_rc_new`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_count`, `ori_rc_data` |
| Closures | `ori_closure_box` |
| Panic | `ori_panic`, `ori_panic_cstr` |
| Assertions | `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |

## Documentation Sections

- [AOT Compilation](aot.md) - Native executable and WebAssembly generation
- [Closures](closures.md) - Closure representation and calling conventions
- [User-Defined Types](user-types.md) - Struct types, impl blocks, and method dispatch

## Source Files

### Core

| File | Purpose |
|------|---------|
| `context.rs` | `SimpleCx`, `CodegenCx`, `StructLayout`, `TypeCache` |
| `builder.rs` | `Builder` type and IR generation helpers |
| `declare.rs` | Function declaration phase |
| `module.rs` | Module-level compilation, struct registration |
| `evaluator.rs` | JIT evaluation, module loading orchestration |
| `types.rs` | Ori-to-LLVM type mapping |

### Code Generation

| File | Purpose |
|------|---------|
| `functions/` | Function body compilation |
| `functions/body.rs` | Function body entry and setup |
| `functions/expressions.rs` | Expression compilation dispatch |
| `functions/calls.rs` | Function and method call compilation |
| `functions/lambdas.rs` | Lambda expression compilation |
| `functions/sequences.rs` | Sequence expression handling (`run`, `try`, `match`) |
| `functions/builtins.rs` | Built-in function compilation |
| `functions/helpers.rs` | Common compilation helpers |
| `functions/phi.rs` | PHI node construction for control flow |
| `collections/` | Collection type handling (lists, maps, tuples) |
| `collections/structs.rs` | Struct literals and field access |
| `collections/strings.rs` | String operations and concatenation |
| `collections/lists.rs` | List construction and operations |
| `collections/maps.rs` | Map construction |
| `collections/tuples.rs` | Tuple construction and access |
| `collections/ranges.rs` | Range expression handling |
| `collections/indexing.rs` | Index access operations |
| `collections/wrappers.rs` | Option and Result wrapper types |
| `control_flow.rs` | If/else, loops, match, short-circuit operators |
| `operators.rs` | Binary and unary operators |
| `matching.rs` | Pattern matching compilation |
| `traits.rs` | Trait method resolution |
| `builtin_methods/` | Built-in type method implementations |
| `builtin_methods/numeric.rs` | Numeric type methods |
| `builtin_methods/ordering.rs` | Ordering type methods |
| `builtin_methods/units.rs` | Duration and Size type methods |

### AOT

| File | Purpose |
|------|---------|
| `aot/target.rs` | Target configuration and machine creation |
| `aot/object.rs` | Object file emission |
| `aot/mangle.rs` | Symbol mangling/demangling |
| `aot/debug.rs` | Debug information (DWARF/CodeView) |
| `aot/passes.rs` | Optimization pipeline |
| `aot/linker/` | Platform-agnostic linker driver |
| `aot/runtime.rs` | Runtime library discovery |
| `aot/multi_file.rs` | Multi-file compilation |
| `aot/wasm.rs` | WebAssembly configuration |
| `aot/incremental/` | Caching and parallel compilation |

## Development

The LLVM crate is built locally with LLVM 17+:

```bash
./llvm-build    # Build the crate
./llvm-test     # Run unit tests
./llvm-clippy   # Run clippy
```

Formatting works without special setup:

```bash
cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml
```

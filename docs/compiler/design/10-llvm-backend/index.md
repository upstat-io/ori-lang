---
title: "LLVM Backend Overview"
description: "LLVM backend architecture for JIT compilation and native code generation"
order: 1
section: "LLVM Backend"
---

# LLVM Backend Overview

The LLVM backend (`ori_llvm` crate) provides both JIT compilation and AOT (Ahead-of-Time) native code generation for Ori programs. It consumes canonical IR (`CanonResult`) from `ori_canon`, optionally running the ARC pipeline (`ori_arc`) for memory management, and produces LLVM IR.

## Architecture

The backend follows patterns from `rustc_codegen_llvm`, using a two-layer architecture:

```
┌─────────────────┐    ┌──────────────────────────────────────┐
│  CanonResult    │    │   SimpleCx (scx)                     │
│  + Pool         │    │   - LLVM Context + Module            │
│  + TypeCheck    │    │   - Common type constructors         │
└───────┬─────────┘    └───────────────┬──────────────────────┘
        │                              │
        │              ┌───────────────▼──────────────────────┐
        │              │   TypeInfoStore + TypeLayoutResolver  │
        │              │   - Idx → TypeInfo (enum) cache       │
        │              │   - Idx → LLVM type resolution        │
        │              └───────────────┬──────────────────────┘
        │                              │
        └──────────────┬───────────────┘
                       │
        ┌──────────────▼──────────────────────────────────────┐
        │   IrBuilder                                         │
        │   - ID-based LLVM instruction builder               │
        │   - ValueId / BlockId / FunctionId handles          │
        │   - Wraps inkwell builder with opaque IDs           │
        ├─────────────────────────────────────────────────────┤
        │   FunctionCompiler                                  │
        │   - Orchestrates function body compilation          │
        │   - ABI handling (sret, parameter passing)          │
        ├─────────────────────────────────────────────────────┤
        │   ExprLowerer                                       │
        │   - Expression dispatch (lower_* methods)           │
        │   - Scope management (persistent-map scoping)       │
        └─────────────────────────────────────────────────────┘
```

### Key Types

| Type | Lifetime | Purpose |
|------|----------|---------|
| `SimpleCx` | Per-compilation | LLVM context, module, common type constructors |
| `TypeInfoStore` | Per-compilation | Lazily-populated `Idx` to `TypeInfo` cache backed by Pool |
| `TypeLayoutResolver` | Per-compilation | Resolves `Idx` to LLVM `BasicTypeEnum` via `TypeInfoStore` + `SimpleCx` |
| `IrBuilder` | Per-module | ID-based instruction builder wrapping inkwell |
| `FunctionCompiler` | Per-module | Function declaration, ABI, runtime declarations |
| `ExprLowerer` | Per-function | Expression lowering coordinator with scoped locals |

## Type Mappings

Ori types map to LLVM types as follows. These are **canonical** mappings — the compiler may use narrower types when it can prove semantic equivalence (see [Representation Optimization Proposal](../../../ori_lang/proposals/approved/representation-optimization-proposal.md)):

| Ori Type | LLVM Type | Notes |
|----------|-----------|-------|
| `int` | `i64` | Canonical: signed integer, range [-2⁶³, 2⁶³ - 1] |
| `float` | `f64` | Canonical: IEEE 754 double-precision |
| `bool` | `i1` | 1-bit boolean (already narrowed from canonical) |
| `byte` | `i8` | Unsigned, range [0, 255] (already narrowed from canonical) |
| `str` | `{ i64, ptr }` | Length + data pointer |
| `[T]` | `{ i64, i64, ptr }` | Length, capacity, data pointer |
| `Option<T>` | `{ i8, T }` | Tag (0=None, 1=Some) + payload |
| `Result<T, E>` | `{ i8, payload }` | Tag (0=Ok, 1=Err) + payload |
| `(A, B, ...)` | `{ A, B, ... }` | Anonymous struct |
| User structs | Named `{ fields... }` | Registered via `TypeInfo::Struct` and `TypeEntry` (see [User Types](user-types.md)) |
| Closures | `{ ptr, ptr }` | Fat pointer `{ fn_ptr, env_ptr }` (see [Closures](closures.md)) |

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

Each function body is compiled. For **Tier 1** (expression lowering without ARC):

1. Create entry basic block
2. Bind parameters to LLVM values
3. Compile function body expression via `ExprLowerer`
4. Build return instruction

For **Tier 2** (ARC codegen), the function goes through the full ARC pipeline:

1. Lower canonical IR to ARC IR (`ori_arc::lower`)
2. Run the unified ARC pipeline via `ori_arc::run_arc_pipeline()`
   - Borrow inference, liveness, RC insertion, reset/reuse, expansion, elimination
3. Emit ARC IR instructions as LLVM IR via `ArcEmitter`

The `run_arc_pipeline()` entry point enforces correct pass ordering — consumers never sequence passes manually.

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
    exit_block: exit_bb,                // break → exit
    continue_block: latch_bb,           // continue → latch (increment then check)
    break_values: Vec::new(),           // deferred break-with-value PHI inputs
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
| Reference Counting | `ori_rc_alloc`, `ori_rc_free`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_count` |
| Panic | `ori_panic`, `ori_panic_cstr`, `ori_register_panic_handler` |
| Assertions | `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`, `ori_assert_eq_float` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |
| Entry | `ori_run_main`, `ori_args_from_argv` |

## Documentation Sections

- [AOT Compilation](aot.md) - Native executable and WebAssembly generation
- [Closures](closures.md) - Closure representation and calling conventions
- [User-Defined Types](user-types.md) - Struct types, impl blocks, and method dispatch

## Source Files

### Core

| File | Purpose |
|------|---------|
| `context.rs` | `SimpleCx` -- minimal LLVM context (module, common types) |
| `evaluator.rs` | JIT evaluation, module loading, pipeline orchestration |
| `runtime.rs` | Runtime library (`libori_rt`) implementation |

### Code Generation (`codegen/`)

| File / Directory | Purpose |
|------------------|---------|
| `type_info/mod.rs` | `TypeInfo` enum, `TypeInfoStore`, `TypeLayoutResolver` |
| `ir_builder/mod.rs` | `IrBuilder` -- ID-based LLVM instruction builder (9 submodules: aggregates, arithmetic, calls, comparisons, constants, control_flow, conversions, memory, phi_types_blocks) |
| `scope/mod.rs` | Persistent-map variable scoping with O(1) clone |
| `value_id/mod.rs` | `ValueId`, `BlockId`, `FunctionId`, `LLVMTypeId` opaque handles |
| `function_compiler/mod.rs` | Function declaration, ABI, body compilation orchestration |
| `abi/mod.rs` | ABI computation (parameter passing, sret returns) |
| `runtime_decl/mod.rs` | Runtime function declarations in LLVM module |
| `type_registration/mod.rs` | `register_user_types()` -- eager type resolution from `TypeEntry` |
| `expr_lowerer.rs` | `ExprLowerer` -- expression dispatch coordinator |
| `lower_literals.rs` | Literals, identifiers, constants |
| `lower_operators.rs` | Binary/unary ops, cast, short-circuit |
| `lower_control_flow.rs` | If, loop, block, break, continue, match |
| `lower_for_loop.rs` | Dedicated for-loop lowering (range, list, str, option, set, map) |
| `lower_error_handling.rs` | Ok, Err, Some, None, Try (`?` operator) |
| `lower_collections.rs` | List, map, tuple, struct, range, field, index |
| `lower_calls.rs` | Call, MethodCall, Lambda (fat-pointer closures) |
| `lower_lambdas.rs` | Lambda/closure lowering |
| `lower_constructs.rs` | Block expressions (FunctionSeq IR), FunctionExp, SelfRef, Await |
| `lower_conversion_builtins.rs` | `str()`, `int()`, `float()`, `byte()`, `assert_eq()` conversions |
| `lower_iterator_trampolines.rs` | Iterator trampolines |
| `lower_builtin_methods/` | Built-in method dispatch (9 files: collections, helpers, inner_dispatch, iterator, option, primitives, result, tuple, mod) |
| `lower_collection_methods/` | Loop-based collection method implementation (list, map, set) |
| `derive_codegen/` | Derived trait code generation (bodies, field_ops, string_helpers, mod) |
| `arc_emitter.rs` | ARC IR emission (retain/release/drop) |

### AOT

| File | Purpose |
|------|---------|
| `aot/target.rs` | Target configuration and machine creation |
| `aot/object.rs` | Object file emission |
| `aot/mangle.rs` | Symbol mangling/demangling |
| `aot/debug/` | Debug information (DWARF/CodeView) |
| `aot/passes.rs` | Optimization pipeline |
| `aot/linker/` | Platform-agnostic linker driver |
| `aot/runtime.rs` | Runtime library discovery |
| `aot/multi_file.rs` | Multi-file compilation |
| `aot/wasm.rs` | WebAssembly configuration |
| `aot/incremental/` | Caching and parallel compilation |

## Development

The LLVM crate is built locally with LLVM 17+:

```bash
./llvm-build.sh    # Build the crate
./llvm-test.sh     # Run unit tests
./llvm-clippy.sh   # Run clippy
```

Formatting works without special setup:

```bash
cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml
```

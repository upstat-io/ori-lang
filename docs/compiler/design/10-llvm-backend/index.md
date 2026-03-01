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
        │   ArcIrEmitter                                      │
        │   - ARC IR instruction dispatch (emit_* methods)    │
        │   - Dead-block elimination, scope management        │
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
| `ArcIrEmitter` | Per-function | ARC IR instruction dispatch with dead-block elimination |

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

Each function body is compiled through the ARC pipeline (the sole codegen path):

1. Lower canonical IR to ARC IR (`ori_arc::lower`)
2. Run the unified ARC pipeline via `ori_arc::run_arc_pipeline()`
   - Borrow inference, liveness, RC insertion, reset/reuse, expansion, elimination
3. Emit ARC IR instructions as LLVM IR via `ArcIrEmitter`

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

The backend links against runtime functions for operations that require heap allocation or complex logic. These are provided by `libori_rt`. All declarations live in `codegen/runtime_decl/runtime_functions.rs` as a single source-of-truth `RT_FUNCTIONS` table.

| Category | Functions |
|----------|-----------|
| Output | `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool` |
| Strings | `ori_str_concat`, `ori_str_eq`, `ori_str_ne`, `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float` |
| String SSO | `ori_str_concat_sso` (SSO-aware concat), `ori_str_from_char` |
| Collections | `ori_list_new`, `ori_list_free`, `ori_list_len` |
| List COW | `ori_list_push_cow`, `ori_list_pop_cow`, `ori_list_set_cow`, `ori_list_insert_cow`, `ori_list_remove_cow` |
| Map COW | `ori_map_insert_cow`, `ori_map_remove_cow` |
| Set COW | `ori_set_insert_cow`, `ori_set_remove_cow`, `ori_set_union_cow`, `ori_set_intersection_cow`, `ori_set_difference_cow` |
| Memory | `ori_alloc`, `ori_free`, `ori_realloc` |
| Reference Counting | `ori_rc_alloc`, `ori_rc_free`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_is_unique` |
| Panic | `ori_panic`, `ori_panic_cstr`, `ori_register_panic_handler` |
| Assertions | `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`, `ori_assert_eq_float` |
| Comparison | `ori_compare_int`, `ori_min_int`, `ori_max_int` |
| Iterators | `ori_iter_from_list`, `ori_iter_from_range`, `ori_iter_from_str`, `ori_iter_next`, `ori_iter_drop` |
| Entry | `ori_run_main`, `ori_args_from_argv` |

### Codegen Verification

The `verify/` module provides an in-pipeline LLVM IR audit pass, gated behind `ORI_AUDIT_CODEGEN=1`:

| Check | Module | Purpose |
|-------|--------|---------|
| RC balance | `rc_balance` | Tracks alloc→inc→dec→free lifecycle per allocation |
| COW rules | `cow_rules` | Validates COW input sequencing (no reuse before uniqueness check) |
| ABI check | `abi_check` | Verifies arg counts, detects large aggregate loads, nounwind+invoke conflicts |
| Safety checks | `safety_checks` | Panic/assert call density analysis |

Options: `ORI_AUDIT_STRICT=1` (pessimistic mode), `ORI_AUDIT_FUNCTION=<name>` (filter to one function).

## Documentation Sections

- [AOT Compilation](aot.md) - Native executable and WebAssembly generation
- [Closures](closures.md) - Closure representation and calling conventions
- [User-Defined Types](user-types.md) - Struct types, impl blocks, and method dispatch

## Source Files

### Core

| File | Purpose |
|------|---------|
| `context/mod.rs` | `SimpleCx` -- minimal LLVM context (module, common types) |
| `evaluator/mod.rs` | JIT evaluation, module loading, pipeline orchestration |
| `evaluator/compile.rs` | Compilation pipeline (ARC lowering → LLVM emission) |
| `evaluator/runtime_mappings.rs` | JIT symbol resolution for runtime functions |
| `runtime.rs` | Runtime library (`libori_rt`) implementation |
| `monomorphize/mod.rs` | Generic function monomorphization |

### Code Generation (`codegen/`)

| File / Directory | Purpose |
|------------------|---------|
| `type_info/` | `TypeInfo` enum (`info.rs`), `TypeInfoStore` + `TypeLayoutResolver` (`store.rs`) |
| `ir_builder/` | `IrBuilder` -- ID-based LLVM instruction builder (9 submodules: aggregates, arithmetic, calls, comparisons, constants, control_flow, conversions, memory, phi_types_blocks) |
| `value_id/mod.rs` | `ValueId`, `BlockId`, `FunctionId`, `LLVMTypeId` opaque handles |
| `function_compiler/` | Function declaration (`mod.rs`), body compilation (`define_phase.rs`), entry point generation (`entry_point.rs`), nounwind analysis (`nounwind.rs`), impl/trait methods (`impls.rs`) |
| `abi/mod.rs` | ABI computation (parameter passing, sret returns) |
| `runtime_decl/` | Runtime function declarations: `mod.rs` (lazy API), `runtime_functions.rs` (RT_FUNCTIONS table) |
| `type_registration/mod.rs` | `register_user_types()` -- eager type resolution from `TypeEntry` |
| `derive_codegen/` | Derived trait code generation (`bodies.rs`, `field_ops.rs`, `string_helpers.rs`) |
| `arc_emitter/` | ARC IR → LLVM IR emission (see below) |

### ARC Emitter (`codegen/arc_emitter/`)

| File | Purpose |
|------|---------|
| `mod.rs` | `ArcIrEmitter`: main emission loop, instruction dispatch, dead-block elimination |
| `apply.rs` | Function call emission (Apply, ApplyIndirect, PartialApply) |
| `closures.rs` | Closure creation and environment capture |
| `construction.rs` | Construct instruction emission (structs, enums, tuples) |
| `context.rs` | `CodegenContext`: shared state across emission |
| `drop_gen.rs` | `DropFunctionGenerator`: per-type LLVM drop functions (cached by mangled name) |
| `element_fn_gen.rs` | Element RC callback function generation (for COW collection operations) |
| `operators.rs` | Binary/unary operator emission |
| `rc_helpers.rs` | RC operation helpers (inc, dec, is_unique) |
| `rc_ops.rs` | RcInc, RcDec, IsShared instruction emission |
| `rc_value_traversal.rs` | Recursive value traversal for RC operations |
| `terminators.rs` | Block terminator emission (Return, Branch, Switch, Invoke) |
| `value_emission.rs` | Value loading, storing, and literal emission |
| `builtins/` | Built-in method codegen: `primitives.rs`, `compound_traits.rs`, `list_traits.rs`, `iterator.rs`, `option_result.rs`, `prelude.rs`, `traits.rs`, `trampolines.rs` |
| `builtins/collections/` | Collection codegen: `list_builtins.rs`, `list_cow.rs`, `map_set_builtins.rs`, `string_builtins.rs` |

### Verification (`verify/`)

| File | Purpose |
|------|---------|
| `mod.rs` | Audit entry point, options, environment variable constants |
| `rc_balance.rs` | RC lifecycle tracking (alloc→inc→dec→free) |
| `cow_rules.rs` | COW sequencing validation |
| `abi_check.rs` | Argument count, aggregate load, nounwind+invoke checks |
| `safety_checks.rs` | Panic/assert call density analysis |
| `report.rs` | `AuditReport` aggregation and formatting |

### AOT (`aot/`)

| File / Directory | Purpose |
|------------------|---------|
| `target.rs` | Target configuration and machine creation |
| `object.rs` | Object file emission |
| `mangle.rs` | Symbol mangling/demangling |
| `debug/` | Debug information: `builder.rs` (DWARF/CodeView), `builder_scope.rs`, `config.rs`, `context.rs`, `line_map.rs` |
| `passes/` | Optimization pipeline: `mod.rs` (pass manager), `config.rs` (pass configuration) |
| `linker/` | Platform-agnostic linker driver: `mod.rs`, `gcc.rs`, `msvc.rs` |
| `runtime.rs` | Runtime library discovery |
| `multi_file/mod.rs` | Multi-file compilation |
| `wasm/` | WebAssembly: `mod.rs`, `config.rs`, `optimize.rs`, `wasi.rs` |
| `incremental/` | Caching and parallel compilation: `cache/` (function caching), `arc_cache/` (ARC IR caching), `deps/` (dependency tracking), `function_deps/` (function dependencies), `function_hash/` (function hashing), `hash/` (general hashing), `parallel/` (parallel execution with `executor.rs`) |
| `syslib/mod.rs` | System library detection |

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

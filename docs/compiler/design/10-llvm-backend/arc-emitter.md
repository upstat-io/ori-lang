---
title: "ARC Emitter"
description: "Ori Compiler Design — ARC IR to LLVM IR Translation"
order: 1004
section: "LLVM Backend"
---

# ARC Emitter

## Overview

The `ArcIrEmitter` is the sole codegen path for all Ori functions. Every function body -- whether user-defined, derived trait method, or compiler-generated closure -- flows through the same pipeline: canonical IR is lowered to ARC IR by `ori_arc`, then `ArcIrEmitter` translates that ARC IR into LLVM IR.

This single-path design eliminates the class of bugs where "direct" and "ARC-managed" codegen diverge. The emitter handles RC lifecycle operations, closure environment capture, drop function generation, and all control flow (including structured exception handling via invoke/resume).

## Architecture

`ArcIrEmitter<'a, 'scx, 'ctx, 'tcx>` wraps an `IrBuilder` and manages several caches and lookup tables:

| Field | Type | Purpose |
|-------|------|---------|
| `var_map` | `FxHashMap<ArcVarId, ValueId>` | Maps ARC IR variables to LLVM values |
| `block_map` | `FxHashMap<ArcBlockId, BlockId>` | Maps ARC IR blocks to LLVM basic blocks |
| `drop_fn_cache` | `FxHashMap<String, FunctionId>` | Memoized drop functions by mangled name |
| `element_inc_cache` | `FxHashMap<Idx, FunctionId>` | Per-element-type RC increment callbacks |
| `element_dec_cache` | `FxHashMap<Idx, FunctionId>` | Per-element-type RC decrement callbacks |
| `comparison_thunks` | `FxHashMap<Idx, FunctionId>` | Comparison function thunks for sort/search |
| `type_info_store` | `&TypeInfoStore` | Lazy type info resolution (Idx to TypeInfo) |
| `type_layout` | `&TypeLayoutResolver` | Idx to LLVM `BasicTypeEnum` resolution |

The emitter borrows the `IrBuilder` mutably for the duration of a single function's emission. After emission completes, control returns to `FunctionCompiler`, which may proceed with the next function.

## RPO Block Emission

Blocks are emitted in **Reverse Post-Order** (RPO), not in the array order they appear in the ARC IR. This ordering guarantee is necessary because the `expand_reuse` pass in `ori_arc` appends fast-path/slow-path/merge blocks whose `Invoke` terminators may target blocks that appear earlier in the array. RPO ensures that every block's dominators are visited before the block itself, which is required for correct SSA construction.

The RPO traversal is computed once from the ARC IR's control flow graph before emission begins. The algorithm:

1. Build a CFG adjacency list from terminators
2. Compute post-order via iterative DFS
3. Reverse to get RPO
4. Skip dead blocks (see below)

**Dead unwind blocks** -- blocks that are only reachable via the unwind edge of an invoke that was downgraded to a `call` (because the callee is provably `nounwind`) -- are detected and skipped entirely. They produce no LLVM IR. This avoids emitting unreachable cleanup code that would confuse the LLVM optimizer and inflate binary size.

## EmittedValue

`EmittedValue` is a tagged wrapper around LLVM values that carries memory representation information through the emission pipeline. Rather than tracking representation separately, the emitter produces and consumes `EmittedValue` at every step.

| Variant | Payload | When Used |
|---------|---------|-----------|
| `Immediate(ValueId)` | Register-width scalar | `int`, `float`, `bool`, `byte`, `char`, pointers |
| `RcPointer(ValueId)` | Pointer to RC-managed heap allocation | Struct/enum instances that are heap-allocated |
| `Aggregate(ValueId)` | Stack-allocated LLVM struct value | Tuples, small structs passed by value, Option/Result |
| `Pair { first, second }` | Two separate `ValueId`s | `str` (len + ptr), closures (fn_ptr + env_ptr) |
| `ZeroSized` | No payload | `void`, `Never`, unit structs with no fields |

The distinction between `Immediate` and `RcPointer` matters for RC operations: incrementing an `Immediate(i64)` is a no-op, while incrementing an `RcPointer` calls `ori_rc_inc`. The distinction between `Aggregate` and `Pair` matters for ABI: aggregates are passed as single LLVM values, while pairs must be split/joined at call boundaries.

## RC Operation Emission

RC operations in the ARC IR (`RcInc`, `RcDec`, `IsShared`) are emitted according to the `RcStrategy` attached to each operation. The strategy is computed by `ori_arc` during borrow inference and RC insertion.

| Strategy | Inc Pattern | Dec Pattern |
|----------|-------------|-------------|
| `HeapPointer` | `call ori_rc_inc(ptr)` | `call ori_rc_dec(ptr)` |
| `HeapPointer` (list) | `call ori_list_rc_inc(data, cap)` | `call ori_list_rc_dec(data, cap)` |
| `FatPointer` | Extract field 1, `call ori_rc_inc(field1)` | Extract field 1, `call ori_rc_dec(field1)` |
| `Closure` | Extract `env_ptr` from field 1; null-check (non-capturing closures have null env); if non-null, `call ori_rc_inc(env_ptr)` | Extract `env_ptr`; null-check; if non-null, load `drop_fn` from env header, `call ori_rc_dec_with_drop(env_ptr, drop_fn)` |
| `AggregateFields` | Recursively inc each RC-containing field | Recursively dec each RC-containing field |
| `InlineEnum` | No-op (variants handle their own RC) | Tag-switch: emit per-variant cleanup block, each block decs that variant's RC fields |

The `AggregateFields` strategy uses `rc_value_traversal.rs` to walk the type structure at codegen time, emitting GEP instructions to reach each nested RC field. This handles arbitrarily nested structs without runtime type information.

For `InlineEnum`, the dec path generates a switch on the discriminant tag, with one arm per variant. Each arm extracts and decrements only the RC fields present in that variant. The inc path is a no-op because enum values are always freshly constructed with correct reference counts.

## Drop Function Generation

`DropFunctionGenerator` (in `drop_gen.rs`) creates specialized `extern "C" fn(*mut u8)` functions for each type that requires custom cleanup at RC-zero. These functions are called by the runtime's `ori_rc_dec` when the reference count reaches zero.

### Naming and Caching

Drop functions are named `_ori_drop$<idx_raw>` where `idx_raw` is the numeric type index. They are cached in `drop_fn_cache` by mangled name. Critically, the cache entry is inserted **before** the function body is generated. This handles recursive types: if type `A` contains a field of type `A` (via `Option<A>` or similar), the drop function for `A` can call itself without infinite recursion during generation.

### Drop Variants

| Variant | Generated Body |
|---------|---------------|
| `Trivial` | Just `call ori_free(ptr)` -- no fields need cleanup |
| `Fields` | GEP to each RC field, load, dec, then `call ori_free(ptr)` |
| `Enum` | Load discriminant tag, switch to per-variant cleanup (each variant decs its own fields), then `call ori_free(ptr)` |
| `Collection` | Loop over elements, dec each, then free backing storage |
| `Map` | Loop over entries, dec each key and value, then free backing storage |
| `ClosureEnv` | GEP to each captured variable, dec if RC-managed, then `call ori_free(ptr)` |

### Element RC Callbacks

Collection types (`[T]`, `{K: V}`, `Set<T>`) need element-level RC operations for COW mutations. The emitter generates `extern "C" fn(*mut u8)` callbacks (one for inc, one for dec per element type) and passes them to runtime COW functions. These are cached in `element_inc_cache` / `element_dec_cache`.

## Terminator Emission

Each ARC IR block ends with exactly one terminator. The emitter translates terminators as follows:

### Return

Respects the function's ABI classification:

- **Direct**: `ret <value>` for register-sized returns
- **Sret**: store value through the sret pointer (first parameter), then `ret void`
- **Void**: `ret void` (for functions returning `void` or `Never`)

### Jump

Unconditional branch to a target block. Records PHI incoming values for the target block's parameters, then emits `br label %target`.

### Branch

Conditional branch on a boolean value. Emits `br i1 %cond, label %true_target, label %false_target`. Records PHI incoming values for both targets.

### Switch

Multi-way branch on a discriminant integer. Emits LLVM `switch` with one case per variant and a default (which may be `unreachable` if the match is exhaustive). Records PHI incoming values for all targets.

### Invoke

Function call with exception handling. The emission depends on `InvokeMode`:

- **`Invoke { normal, unwind }`**: Emits LLVM `invoke` with normal and unwind continuations. The unwind block typically contains cleanup code (RC decrements for in-scope values) followed by `resume`.
- **`Call { normal }`**: Emits LLVM `call` followed by `br label %normal`. Used when the callee is provably `nounwind` (determined by `nounwind.rs` analysis). This eliminates unnecessary landingpad overhead.

### Resume and Unreachable

- **Resume**: re-raises the current exception. Emits LLVM `resume` with the landingpad value.
- **Unreachable**: emits LLVM `unreachable`. Used after calls to `@noreturn` functions like `panic`.

## InvokeMode

`InvokeMode` is an enum that makes call-site control flow explicit, eliminating the boolean flags (`is_nounwind`, `has_unwind_target`) that previously caused subtle bugs:

```
enum InvokeMode {
    Invoke { normal: BlockId, unwind: BlockId },
    Call { normal: BlockId },
}
```

The mode is determined once per call site based on:

1. Whether the callee is marked `nounwind` (via analysis or annotation)
2. Whether the ARC IR provides an unwind continuation

If both conditions align (nounwind callee + no unwind target), `Call` mode is used. Otherwise, `Invoke` mode is used even for nounwind callees if the ARC IR explicitly provides an unwind path (this can happen with conservative RC cleanup).

## Submodule Organization

The `arc_emitter/` directory is organized by responsibility:

| File | Responsibility |
|------|---------------|
| `mod.rs` | Main emission loop, instruction dispatch, RPO traversal, dead-block elimination |
| `apply.rs` | Function call emission: `Apply` (direct), `ApplyIndirect` (closure/fn-value), `PartialApply` (partial application / closure creation) |
| `closures.rs` | Closure environment allocation, capture emission, env pointer packing |
| `construction.rs` | `Construct` instruction: struct literals, enum variant construction, tuple creation |
| `context.rs` | `CodegenContext`: shared mutable state (current block, loop contexts, landing pads) |
| `drop_gen.rs` | `DropFunctionGenerator`: per-type drop functions with recursive-type support |
| `element_fn_gen.rs` | Element-level RC callback generation for COW collection operations |
| `operators.rs` | Binary operators (arithmetic, comparison, bitwise, logical) and unary operators (negation, bitwise not) |
| `rc_helpers.rs` | Low-level RC operation helpers: raw inc/dec calls, null-check wrappers, is_unique checks |
| `rc_ops.rs` | `RcInc`, `RcDec`, `IsShared` instruction emission (dispatches on `RcStrategy`) |
| `rc_value_traversal.rs` | Recursive type-structure walk for `AggregateFields` RC strategy |
| `terminators.rs` | All terminator emission (Return, Jump, Branch, Switch, Invoke, Resume, Unreachable) |
| `value_emission.rs` | Literal emission, value loading/storing, `EmittedValue` construction and decomposition |
| `builtins/` | Built-in method codegen (see [Builtins Codegen](builtins-codegen.md)) |

## Interaction with ori_arc

The emitter consumes `ArcFunction` values produced by `ori_arc::run_arc_pipeline()`. It does not call any ARC analysis passes itself. The contract is:

- **ori_arc** is responsible for: borrow inference, liveness analysis, RC insertion, reset/reuse optimization, dead code elimination, and expansion of high-level ARC operations into explicit control flow.
- **ArcIrEmitter** is responsible for: translating the resulting flat ARC IR blocks into LLVM IR, generating drop functions, emitting RC runtime calls, and managing the LLVM basic block structure.

This separation means the emitter never needs to reason about variable liveness or RC placement -- those decisions are already baked into the ARC IR's instruction sequence and `RcStrategy` annotations.

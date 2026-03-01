---
title: "Closures"
description: "Closure representation and calling conventions in the LLVM backend"
order: 2
section: "LLVM Backend"
---

# Closures

Closures in Ori capture variables from their enclosing scope by value. The LLVM backend uses a fat pointer representation for uniform handling of closures with and without captures.

## Representation

All closures (with or without captures) use a fat pointer `{ fn_ptr: ptr, env_ptr: ptr }`:

```
Fat Pointer (LLVM struct { ptr, ptr }):
┌──────────────────────────────────┬──────────────────────────────┐
│         fn_ptr (ptr)             │         env_ptr (ptr)        │
│   Pointer to lambda function     │   Pointer to env struct      │
│                                  │   (null if no captures)      │
└──────────────────────────────────┴──────────────────────────────┘
```

### Closures Without Captures

When a closure captures no variables, the `env_ptr` field is null. The lambda function still takes a hidden `ptr %env` first parameter (which it ignores), so the calling convention is uniform.

### Closures With Captures

When a closure captures variables, a heap-allocated environment struct is created containing the captured values at their native types:

```
env_ptr ───────────────▶ Environment Struct:
                         ┌────────────────┬────────────────┬───────┐
                         │ capture0 (T0)  │ capture1 (T1)  │ ...   │
                         └────────────────┴────────────────┴───────┘
```

Each capture is stored at its native LLVM type (not coerced to i64). The lambda function unpacks captures from the environment struct using `struct_gep` with the correct field types.

## Compilation

### Lambda Compilation (`emit_partial_apply`)

The `emit_partial_apply` method in `codegen/arc_emitter/closures.rs` compiles a `PartialApply` ARC IR instruction into a fat-pointer closure. The steps are:

1. **Capture analysis**: Walk the lambda body to find free variables (variables used but not bound as parameters). Each capture includes the variable's `Name`, current `ValueId`, and type `Idx`.
2. **Get type info**: Read the lambda's `TypeInfo::Function` to determine actual parameter and return types.
3. **Generate lambda function**: Create an LLVM function with signature `(ptr %env, T1 %p1, T2 %p2, ...) -> R` where the hidden `ptr %env` is always the first parameter.
4. **Unpack captures in body**: If captures exist, use `struct_gep` on the env pointer to load each captured value at its native type.
5. **Compile body**: Lower the lambda body expression with captures and parameters in scope.
6. **Build fat pointer**: Construct `{ fn_ptr, env_ptr }` where `env_ptr` is null if no captures, or a heap-allocated environment struct otherwise.

```rust
// Pseudocode for lambda compilation (via ARC pipeline)
fn emit_partial_apply(params, body) -> { ptr, ptr } {
    let captures = collect_captures(body, params);

    // Lambda signature: (ptr env, actual params...) -> actual_ret_type
    let lambda_fn = declare_function("__lambda_N", [ptr, P1, P2, ...], R);

    // In lambda body: unpack captures from env struct via struct_gep
    if !captures.is_empty() {
        let env_ptr = get_param(lambda_fn, 0);
        for (i, capture) in captures {
            let field_ptr = struct_gep(env_struct_ty, env_ptr, i);
            let val = load(field_ty, field_ptr);
            scope.bind(capture.name, val);
        }
    }

    // Compile body, emit return at native type
    compile_body(body);

    // Build environment
    let env_ptr = if captures.is_empty() {
        null_ptr
    } else {
        let env = alloca(env_struct_type);
        for (i, capture) in captures {
            let ptr = struct_gep(env_struct_ty, env, i);
            store(capture.value, ptr);
        }
        env
    };

    // Return fat pointer
    return { fn_ptr: lambda_fn, env_ptr };
}
```

### Closure Calling (`emit_apply_indirect`)

When calling a closure stored in a variable via an `ApplyIndirect` ARC IR instruction, the calling convention is uniform regardless of whether captures exist:

1. **Extract** `fn_ptr` and `env_ptr` from the fat pointer via `extract_value`
2. **Prepend** `env_ptr` as the first argument
3. **Call indirectly** through `fn_ptr` with actual types from `TypeInfo::Function`

```rust
// Pseudocode for closure call (via ARC pipeline)
fn emit_apply_indirect(closure_val: { ptr, ptr }, args) -> R {
    let fn_ptr = extract_value(closure_val, 0);  // fn_ptr
    let env_ptr = extract_value(closure_val, 1);  // env_ptr

    // Build args: env_ptr first, then actual arguments
    let all_args = [env_ptr] ++ lower_each(args);

    // Indirect call through fn_ptr with actual types
    return call_indirect(ret_type, param_types, fn_ptr, all_args);
}
```

No tag-bit checking is needed because the calling convention is uniform: all lambda functions accept `ptr %env` as their first parameter, whether or not they use it.

## Capture Analysis

Capture analysis is performed during ARC lowering (in `ori_arc`), not during LLVM emission. The `collect_captures` method on `ArcLowerer` walks the lambda body recursively to identify free variables:

```rust
fn collect_captures(
    &self, body: CanId, params: &[Name],
    captures: &mut Vec<(Name, ArcVarId)>, seen: &mut HashSet<Name>,
) {
    // Walk body, collect identifiers that:
    // 1. Are NOT lambda parameters
    // 2. ARE in the current scope (captured from enclosing scope)
    // 3. Haven't been seen yet (avoid duplicates)
    // Returns (name, arc_var_id) pairs
}
```

Captures become the first parameters of the lambda's `ArcFunction`. The ARC IR `PartialApply` instruction records which outer variables are captured. The LLVM `emit_partial_apply` then generates the environment struct with native-typed fields.

Supported expression types for capture analysis:
- Identifiers (primary capture source)
- Binary/unary operations
- Function calls (named and positional)
- Conditionals
- Blocks with let bindings
- Nested lambdas
- Field access and indexing

## IrBuilder API

The `IrBuilder` provides methods for working with closure fat pointers:

- `closure_type()` -- returns the `{ ptr, ptr }` struct type used for all closures
- `extract_value(agg, index, name)` -- extracts a field from a struct value (used for `fn_ptr` at index 0 and `env_ptr` at index 1)
- `build_struct(ty, fields, name)` -- constructs a fat pointer from `fn_ptr` and `env_ptr` values
- `call_indirect(ret_ty, param_types, fn_ptr, args, name)` -- indirect call through a function pointer

## Limitations

- Captured values are stored at native types (no coercion), but the environment struct layout is ephemeral and not accessible across compilation units
- No closure deallocation currently (relies on program termination or ARC for cleanup)
- Closures always use `fastcc` calling convention for the lambda function

## Source Files

| File | Purpose |
|------|---------|
| `codegen/arc_emitter/closures.rs` | `emit_partial_apply`, closure creation, environment allocation, wrapper generation |
| `codegen/arc_emitter/apply.rs` | `emit_apply`, `emit_apply_indirect`, call dispatch (direct + indirect through closures) |
| `codegen/ir_builder/` | `closure_type()`, `extract_value`, `build_struct`, `call_indirect` (split across `aggregates.rs`, `calls.rs`, `memory.rs`) |
| `codegen/arc_emitter/rc_ops.rs` | RC operations for closure environments (retain/release env_ptr) |
| `codegen/arc_emitter/drop_gen.rs` | Drop function generation for closure environment structs |
| `codegen/arc_emitter/value_emission.rs` | Literal and function-as-value emission |

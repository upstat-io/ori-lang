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

### Lambda Compilation (`lower_lambda`)

The `lower_lambda` function in `codegen/lower_calls.rs` compiles a lambda expression into a fat-pointer closure. The steps are:

1. **Capture analysis**: Walk the lambda body to find free variables (variables used but not bound as parameters). Each capture includes the variable's `Name`, current `ValueId`, and type `Idx`.
2. **Get type info**: Read the lambda's `TypeInfo::Function` to determine actual parameter and return types.
3. **Generate lambda function**: Create an LLVM function with signature `(ptr %env, T1 %p1, T2 %p2, ...) -> R` where the hidden `ptr %env` is always the first parameter.
4. **Unpack captures in body**: If captures exist, use `struct_gep` on the env pointer to load each captured value at its native type.
5. **Compile body**: Lower the lambda body expression with captures and parameters in scope.
6. **Build fat pointer**: Construct `{ fn_ptr, env_ptr }` where `env_ptr` is null if no captures, or a heap-allocated environment struct otherwise.

```rust
// Pseudocode for lambda compilation
fn lower_lambda(params, body) -> { ptr, ptr } {
    let captures = find_captures(body, params);

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

### Closure Calling (`lower_closure_call`)

When calling a closure stored in a variable, the calling convention is uniform regardless of whether captures exist:

1. **Extract** `fn_ptr` and `env_ptr` from the fat pointer via `extract_value`
2. **Prepend** `env_ptr` as the first argument
3. **Call indirectly** through `fn_ptr` with actual types from `TypeInfo::Function`

```rust
// Pseudocode for closure call
fn lower_closure_call(closure_val: { ptr, ptr }, args) -> R {
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

The `find_captures` method on `ExprLowerer` walks the lambda body recursively to identify free variables. Each capture includes three pieces of information:

```rust
fn find_captures(&mut self, body: CanId, params: CanParamRange) -> Vec<(Name, ValueId, Idx)> {
    // Walk body, collect identifiers that:
    // 1. Are NOT lambda parameters
    // 2. ARE in the current scope (captured from enclosing scope)
    // 3. Haven't been seen yet (avoid duplicates)
    // Returns (name, current_value, type_index) triples
}
```

The type index (`Idx`) is needed to build the environment struct with native-typed fields, so each capture is stored at its correct LLVM type rather than being coerced to `i64`.

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
| `codegen/lower_calls.rs` | `lower_lambda`, `lower_closure_call`, capture analysis |
| `codegen/ir_builder.rs` | `closure_type()`, `extract_value`, `build_struct`, `call_indirect` |
| `codegen/arc_emitter.rs` | ARC emission for closure values (retain/release env) |
| `codegen/lower_literals.rs` | Function-as-value wrapping (non-lambda function references as fat pointers) |

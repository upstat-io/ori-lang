---
title: "User-Defined Types and Impl Blocks"
description: "Compilation of user-defined struct types, impl blocks, and method dispatch in the LLVM backend"
order: 3
section: "LLVM Backend"
---

# User-Defined Types and Impl Blocks

The LLVM backend supports user-defined struct types and impl blocks with associated functions and methods. This document describes the struct layout tracking system, type registration, and method call compilation.

## Type Information System

User-defined struct types are represented through the `TypeInfo` enum in `codegen/type_info.rs`. Each Ori type category gets a `TypeInfo` variant that encapsulates its LLVM representation, memory layout, and calling convention.

### `TypeInfo::Struct`

Struct types are represented by the `TypeInfo::Struct` variant:

```rust
pub enum TypeInfo {
    // ...
    /// User-defined struct -> {field1, field2, ...}
    Struct { fields: Vec<(Name, Idx)> },
    // ...
}
```

Each field carries its `Name` (interned) and `Idx` (type pool index), which allows the `TypeLayoutResolver` to compute the LLVM struct type with native-typed fields.

### `TypeInfoStore`

The `TypeInfoStore` is a lazily-populated `Idx` to `TypeInfo` cache backed by the type checker's `Pool`:

```rust
pub struct TypeInfoStore<'pool> {
    pool: &'pool Pool,
    cache: RefCell<FxHashMap<Idx, TypeInfo>>,
}
```

When a type is first accessed, the store reads the `Pool`'s `Tag` for that `Idx` and constructs the appropriate `TypeInfo` variant. Subsequent accesses return the cached result.

### `TypeLayoutResolver`

The `TypeLayoutResolver` resolves `Idx` to LLVM `BasicTypeEnum` by combining `TypeInfoStore` with `SimpleCx`:

```rust
pub struct TypeLayoutResolver<'store, 'pool, 'll> {
    store: &'store TypeInfoStore<'pool>,
    scx: &'store SimpleCx<'ll>,
    // ...
}
```

It handles recursive types via named struct forward references and caches resolved LLVM types for performance.

## Struct Type Registration

Before compiling function bodies, user-defined struct types must be registered with the backend. This is driven by the type checker's `TypeEntry` output and handled by `register_user_types()` in `codegen/type_registration.rs`.

### Registration Flow

1. The type checker produces a list of `TypeEntry` values (from `ori_types`), each describing a user-defined type with its name, kind, fields, and type pool index.
2. `LLVMEvaluator` creates a `TypeInfoStore` and `TypeLayoutResolver`.
3. `register_user_types()` iterates over `TypeEntry` values and eagerly resolves each through the `TypeLayoutResolver`, which creates LLVM named struct types in the module.

```rust
// In evaluator.rs
let store = TypeInfoStore::new(self.pool);
let resolver = TypeLayoutResolver::new(&store, scx_ref);
type_registration::register_user_types(&resolver, user_types);
```

### `register_user_types()`

The registration function in `codegen/type_registration.rs`:

```rust
pub fn register_user_types(resolver: &TypeLayoutResolver<'_, '_, '_>, types: &[TypeEntry]) {
    for entry in types {
        // Skip generic types — they're resolved during monomorphization
        if !entry.type_params.is_empty() {
            continue;
        }

        // Eagerly resolve the type to create the LLVM named struct.
        // The resolver caches the result, so subsequent calls return
        // the cached type.
        resolver.resolve(entry.idx);
    }
}
```

Generic types (those with non-empty `type_params`) are skipped during registration and resolved later when concrete instances are encountered during monomorphization.

## Impl Block Compilation

Impl blocks define methods for types. The LLVM backend compiles impl methods as standalone functions that can be called via method syntax.

### Compilation Flow

1. **Type Registration**: Struct types are registered before function compilation
2. **Function Compilation**: Regular module functions are compiled
3. **Method Compilation**: Impl block methods are converted to `Function` IR and compiled

```rust
// Compile impl block methods
for impl_def in &module.impls {
    for method in &impl_def.methods {
        // Convert ImplMethod to Function for compilation
        let func = ori_ir::Function {
            name: method.name,
            generics: impl_def.generics,
            params: method.params,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            body: method.body,
            span: method.span,
            visibility: Visibility::Private,
        };
        compiler.compile_function(&func, arena, expr_types);
    }
}
```

### Method Naming

Methods are compiled as functions with their original name. This means method resolution relies on the function being found in the LLVM module by name lookup.

## Method Call Compilation

Method calls (`receiver.method(args)`) are compiled by `lower_method_call` in `codegen/lower_calls.rs`.

### Dispatch Order

The `lower_method_call` function uses a four-tier dispatch strategy:

1. **Built-in methods**: Type-specific inline codegen (e.g., `list.len()`, `str.contains()`)
2. **Type-qualified method lookup**: Resolve receiver type index to a type name, then look up `(type_name, method_name)` in the `method_functions` map
3. **Bare-name fallback**: Check the `functions` map by method name alone
4. **LLVM module lookup**: Search for runtime functions by name in the LLVM module

```rust
pub(crate) fn lower_method_call(
    &mut self,
    receiver: CanId,
    method: Name,
    args: CanRange,
) -> Option<ValueId> {
    let recv_type = self.expr_type(receiver);
    let recv_val = self.lower(receiver)?;

    // 1. Try built-in method dispatch
    if let Some(result) = self.lower_builtin_method(recv_val, recv_type, &method_str, args) {
        return Some(result);
    }

    // 2. Type-qualified method lookup
    if let Some(&type_name) = self.type_idx_to_name.get(&recv_type) {
        if let Some((func_id, abi)) = self.method_functions.get(&(type_name, method)) {
            return self.emit_method_call(func_id, &abi, recv_val, args, "method_call");
        }
    }

    // 3. Bare-name fallback
    // 4. LLVM module lookup
    // ...
}
```

### Method Call ABI

When a method function has ABI information (from `FunctionCompiler`), method calls use `emit_method_call` which handles:
- Receiver passed as the first argument
- ABI-aware parameter passing (direct, indirect, reference)
- Sret returns for large types

## Field Access

Field access on structs is handled by ARC lowering (`ori_arc`) before it reaches LLVM codegen. The ARC lowering pass translates high-level field access into explicit `StructGet` instructions that carry the field index directly, removing the need for field name lookups during code generation.

In the LLVM backend, struct field access appears as `extract_value` operations on LLVM struct values, using the field indices computed during ARC lowering.

## Compilation Order Summary

1. **Type Info Setup**: Create `TypeInfoStore` and `TypeLayoutResolver` from `Pool`
2. **Type Registration**: `register_user_types()` eagerly resolves `TypeEntry` values via `TypeLayoutResolver`
3. **Runtime Declaration**: Declare runtime functions in the LLVM module
4. **Function Compilation**: `FunctionCompiler` declares and defines function bodies
5. **Expression Lowering**: `ExprLowerer` handles method calls, field access, etc.

## Source Files

| File | Purpose |
|------|---------|
| `codegen/type_info.rs` | `TypeInfo` enum (including `TypeInfo::Struct`), `TypeInfoStore`, `TypeLayoutResolver` |
| `codegen/type_registration.rs` | `register_user_types()` -- eager resolution from `TypeEntry` |
| `codegen/lower_calls.rs` | `lower_method_call` -- method dispatch and calling |
| `codegen/lower_collections.rs` | Struct literal construction, field access lowering |
| `evaluator.rs` | Pipeline orchestration, creates `TypeInfoStore` and `TypeLayoutResolver` |

## Limitations

- Generic struct types are skipped during registration (resolved at monomorphization)
- Method resolution relies on function name lookup (no overloading)
- Enum types use a tag + max-payload representation (not optimized for single-variant enums)

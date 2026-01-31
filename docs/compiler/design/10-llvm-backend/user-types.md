---
title: "User-Defined Types and Impl Blocks"
description: "Compilation of user-defined struct types, impl blocks, and method dispatch in the LLVM backend"
order: 3
section: "LLVM Backend"
---

# User-Defined Types and Impl Blocks

The LLVM backend supports user-defined struct types and impl blocks with associated functions and methods. This document describes the struct layout tracking system, type registration, and method call compilation.

## Struct Layout Tracking

User-defined struct types require metadata for field access code generation. The backend tracks this through the `StructLayout` type in `context.rs`:

```rust
pub struct StructLayout {
    /// Field names in declaration order (index = LLVM struct field index).
    pub fields: Vec<Name>,
    /// Map from field name to index for O(1) lookup.
    pub field_indices: HashMap<Name, u32>,
}
```

The `TypeCache` maintains a registry of struct layouts:

```rust
pub struct TypeCache<'ll> {
    // ... other fields ...

    /// Named struct types for forward references.
    pub named_structs: HashMap<Name, StructType<'ll>>,
    /// Struct field layouts for user-defined types.
    pub struct_layouts: HashMap<Name, StructLayout>,
}
```

### Layout Creation

Layouts are created when struct types are registered:

```rust
impl StructLayout {
    pub fn new(fields: Vec<Name>) -> Self {
        let field_indices = fields
            .iter()
            .enumerate()
            .map(|(i, &name)| (name, i as u32))
            .collect();
        Self { fields, field_indices }
    }
}
```

## Struct Type Registration

Before compiling function bodies, user-defined struct types must be registered with the backend. This happens in `LLVMEvaluator::eval_test`:

```rust
// Register user-defined struct types
for type_decl in &module.types {
    if let TypeDeclKind::Struct(fields) = &type_decl.kind {
        let field_names: Vec<Name> = fields.iter().map(|f| f.name).collect();
        compiler.register_struct(type_decl.name, field_names);
    }
}
```

The `ModuleCompiler::register_struct` method:

1. Creates LLVM field types (currently all `i64` for simplicity)
2. Creates or retrieves a named LLVM struct type
3. Sets the struct body with field types
4. Records the field layout for later access

```rust
pub fn register_struct(&self, name: Name, field_names: Vec<Name>) {
    let field_types: Vec<_> = field_names
        .iter()
        .map(|_| self.cx.scx.type_i64().into())
        .collect();

    self.cx.register_struct(name, field_names, &field_types);
}
```

### CodegenCx Registration

The `CodegenCx::register_struct` method handles the actual LLVM type creation:

```rust
pub fn register_struct(
    &self,
    name: Name,
    field_names: Vec<Name>,
    field_types: &[BasicTypeEnum<'ll>],
) {
    // Create or get the named struct type
    let struct_ty = self.get_or_create_named_struct(name);

    // Set the struct body with field types
    self.scx.set_struct_body(struct_ty, field_types, false);

    // Record the field layout for field access
    let layout = StructLayout::new(field_names);
    self.type_cache.borrow_mut().struct_layouts.insert(name, layout);
}
```

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

Method calls (`receiver.method(args)`) are compiled differently based on whether the receiver is a value or a type name.

### Instance Methods vs Associated Functions

The `compile_method_call` and `compile_method_call_named` functions in `functions/calls.rs` handle both cases:

```rust
pub(crate) fn compile_method_call(
    &self,
    receiver: ExprId,
    method: Name,
    args: ExprRange,
    arena: &ExprArena,
    expr_types: &[TypeId],
    locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
    function: FunctionValue<'ll>,
    loop_ctx: Option<&LoopContext<'ll>>,
) -> Option<BasicValueEnum<'ll>> {
    // Try to compile receiver
    let recv_val = self.compile_expr(receiver, arena, expr_types, locals, function, loop_ctx);

    // Compile arguments
    let arg_ids = arena.get_expr_list(args);

    // If receiver compiled to a value, it's an instance method
    // If receiver is None (type name for associated function), don't include it
    let mut compiled_args: Vec<BasicValueEnum<'ll>> = match recv_val {
        Some(val) => vec![val],  // Instance method: receiver is first arg
        None => vec![],          // Associated function: no receiver arg
    };

    for &arg_id in arg_ids {
        if let Some(arg_val) = self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx) {
            compiled_args.push(arg_val);
        }
    }

    // Look up method function
    let method_name = self.cx().interner.lookup(method);
    if let Some(callee) = self.cx().llmod().get_function(method_name) {
        self.call(callee, &compiled_args, "method_call")
    } else {
        None
    }
}
```

### Receiver Classification

| Receiver Type | `compile_expr` Result | Argument Handling |
|--------------|----------------------|-------------------|
| Value (e.g., `point`) | `Some(value)` | Value passed as first argument |
| Type name (e.g., `Point`) | `None` | No first argument (associated function) |

This design allows the same code path to handle both:
- `point.distance()` - instance method, receiver passed as first arg
- `Point.new(x: 1, y: 2)` - associated function, no receiver arg

## Field Access

Field access on structs uses the registered layout to determine field indices:

```rust
pub(crate) fn compile_field_access(
    &self,
    receiver: ExprId,
    field: Name,
    arena: &ExprArena,
    expr_types: &[TypeId],
    locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
    function: FunctionValue<'ll>,
    loop_ctx: Option<&LoopContext<'ll>>,
) -> Option<BasicValueEnum<'ll>>
```

The current implementation uses heuristics for field index lookup. When proper type tracking is available, it uses `CodegenCx::get_field_index`:

```rust
pub fn get_field_index(&self, struct_name: Name, field_name: Name) -> Option<u32> {
    self.type_cache
        .borrow()
        .struct_layouts
        .get(&struct_name)
        .and_then(|layout| layout.field_index(field_name))
}
```

### Defensive Handling

The `compile_field_access` function includes a defensive check for non-struct values:

```rust
let struct_val = match receiver_val {
    BasicValueEnum::StructValue(s) => s,
    _ => {
        // Not a struct - return placeholder for now
        return Some(self.cx().scx.type_i64().const_int(0, false).into());
    }
};
```

This handles cases where method compilation falls back to INT types due to missing type information.

## Compilation Order Summary

1. **Runtime Declaration**: `compiler.declare_runtime()` - declare runtime functions
2. **Type Registration**: Loop over `module.types`, register struct types
3. **Function Compilation**: Loop over `module.functions`, compile each
4. **Impl Method Compilation**: Loop over `module.impls`, compile methods
5. **Test Compilation**: Create and compile test wrapper functions

## Lookup Methods

`CodegenCx` provides several lookup methods for struct information:

| Method | Purpose |
|--------|---------|
| `get_struct_type(name)` | Get LLVM struct type by name |
| `get_field_index(struct_name, field_name)` | Get field index for access |
| `get_struct_layout(name)` | Get full layout information |
| `get_or_create_named_struct(name)` | Get or create opaque struct for forward references |

## Source Files

| File | Purpose |
|------|---------|
| `context.rs` | `StructLayout`, `TypeCache`, struct registration |
| `module.rs` | `ModuleCompiler::register_struct` |
| `evaluator.rs` | Module loading, type registration orchestration |
| `functions/calls.rs` | Method call compilation, receiver handling |
| `collections/structs.rs` | Struct literal compilation, field access |

## Limitations

- All struct fields currently use `i64` type (matching INT fallback)
- Field access uses heuristics for common field names when type info unavailable
- Method resolution relies on function name lookup (no overloading)
- No support for generic struct types yet

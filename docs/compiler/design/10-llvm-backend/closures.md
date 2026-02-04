---
title: "Closures"
description: "Closure representation and calling conventions in the LLVM backend"
order: 2
section: "LLVM Backend"
---

# Closures

Closures in Ori capture variables from their enclosing scope by value. The LLVM backend uses a tagged pointer representation that optimizes the common case of closures without captures.

## Representation

### Closures Without Captures

Closures that don't capture any variables are represented as plain function pointers stored as `i64`:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Function Pointer (i64)                     │
│                         (lowest bit = 0)                        │
└─────────────────────────────────────────────────────────────────┘
```

The lowest bit is always 0 for aligned function pointers, which serves as a tag indicating "plain function pointer".

### Closures With Captures

Closures that capture variables are heap-allocated ("boxed") and represented as a tagged pointer. The lowest bit is set to 1 to distinguish from plain function pointers:

```
Tagged Pointer (i64):
┌─────────────────────────────────────────────────────────────────┐
│                   Heap Pointer | 1 (tag bit)                    │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
Heap-Allocated Closure Struct:
┌────────────┬─────────────────┬────────────┬────────────┬───────┐
│ i8 count   │ i64 fn_ptr      │ i64 cap0   │ i64 cap1   │ ...   │
│ (captures) │                 │            │            │       │
└────────────┴─────────────────┴────────────┴────────────┴───────┘
  offset 0     offset 8          offset 16    offset 24
```

The struct layout:
- **Byte 0**: Capture count (i8) - number of captured values
- **Bytes 8-15**: Function pointer as i64
- **Bytes 16+**: Captured values, each as i64 (8 bytes per capture)

## Compilation

### Lambda Compilation

When compiling a lambda expression:

1. **Analyze captures**: Walk the lambda body to find free variables (variables used but not bound as parameters)
2. **Generate lambda function**: Create an LLVM function that takes regular parameters plus captured values as additional parameters
3. **Build return value**:
   - No captures: Return function pointer as i64 (tag bit naturally 0)
   - Has captures: Allocate closure struct via `ori_closure_box`, store function pointer and captures, return tagged pointer (with bit 0 set to 1)

```rust
// Pseudocode for lambda compilation
fn compile_lambda(params, body, captures) -> i64 {
    // Create lambda function with signature:
    // (param0, param1, ..., capture0, capture1, ...) -> i64
    let fn_ptr = create_lambda_function(params, captures, body);

    if captures.is_empty() {
        // Return plain function pointer
        return fn_ptr as i64;
    }

    // Allocate closure struct
    let size = 1 + 8 + 8 * captures.len();
    let ptr = ori_closure_box(size);

    // Store closure data
    *ptr.offset(0) = captures.len() as i8;
    *ptr.offset(8) = fn_ptr as i64;
    for (i, capture) in captures.enumerate() {
        *ptr.offset(16 + i * 8) = capture as i64;
    }

    // Return tagged pointer
    return (ptr as i64) | 1;
}
```

### Closure Calling

When calling a closure stored in a variable:

1. **Check tag bit**: Examine bit 0 of the i64 value
2. **Plain pointer (bit 0 = 0)**: Cast directly to function pointer and call
3. **Boxed closure (bit 0 = 1)**: Clear tag bit, dereference to get closure struct, extract function pointer and captures, call with captures appended to arguments

```rust
// Pseudocode for closure call
fn call_closure(closure_val: i64, args: &[i64]) -> i64 {
    if closure_val & 1 == 0 {
        // Plain function pointer
        let fn_ptr = closure_val as fn(*args) -> i64;
        return fn_ptr(args);
    }

    // Boxed closure - clear tag bit
    let ptr = (closure_val & !1) as *const ClosureStruct;
    let capture_count = (*ptr).count;
    let fn_ptr = (*ptr).fn_ptr;

    // Build args: original args + captured values
    let mut all_args = args.to_vec();
    for i in 0..capture_count {
        all_args.push((*ptr).captures[i]);
    }

    // Call with extended args
    return fn_ptr(all_args);
}
```

## Runtime Support

### `ori_closure_box`

Allocates heap memory for a closure struct:

```rust
#[no_mangle]
pub extern "C" fn ori_closure_box(size: i64) -> *mut u8 {
    let size = size.max(8) as usize;
    let layout = Layout::from_size_align(size, 8).unwrap();
    unsafe { alloc(layout) }
}
```

This function is declared in LLVM during module initialization and called during lambda compilation when captures exist.

## Capture Analysis

The `find_captures` function walks the lambda body recursively to identify free variables:

```rust
fn find_captures(body, arena, param_names, outer_locals) -> Vec<(Name, Value)> {
    // Walk body, collect identifiers that:
    // 1. Are NOT lambda parameters
    // 2. ARE in outer_locals (captured from enclosing scope)
    // 3. Haven't been seen yet (avoid duplicates)
}
```

Supported expression types for capture analysis:
- Identifiers (primary capture source)
- Binary/unary operations
- Function calls (named and positional)
- Conditionals
- Blocks with let bindings
- Nested lambdas
- Field access and indexing

## Struct Shorthand

The LLVM backend supports struct shorthand syntax in closures:

```ori
let x = 10
let y = 20
let point = Point { x, y }  // Shorthand for Point { x: x, y: y }
```

When compiling struct expressions, if a field has no explicit value (`init.value` is `None`), the backend looks up a variable with the same name as the field from the `locals` map:

```rust
let val = if let Some(value_id) = init.value {
    // Explicit: Point { x: 10 }
    compile_expr(value_id, ...)
} else {
    // Shorthand: Point { x } - look up variable named 'x'
    locals.get(&init.name).copied()?
};
```

## Builder API

The `Builder::extract_value` method returns `Option<BasicValueEnum>` to handle out-of-range field access gracefully:

```rust
pub fn extract_value(
    &self,
    agg: StructValue<'ll>,
    index: u32,
    name: &str,
) -> Option<BasicValueEnum<'ll>> {
    self.llbuilder.build_extract_value(agg, index, name).ok()
}
```

This is used when extracting captures from boxed closures and fields from struct values.

## Limitations

- All captured values are coerced to i64 (type information reconstructed at call site)
- Capture count stored in i8 field (theoretical 255 limit, though memory would limit earlier)
- No closure deallocation currently (relies on program termination for cleanup)

## Source Files

| File | Purpose |
|------|---------|
| `functions/lambdas.rs` | Lambda compilation, capture analysis |
| `functions/calls.rs` | Closure calling conventions |
| `runtime.rs` | `ori_closure_box` implementation |
| `builder.rs` | `extract_value` and struct manipulation |
| `collections/structs.rs` | Struct shorthand handling |

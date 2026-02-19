---
section: "04"
title: LLVM Codegen Consolidation
status: not-started
goal: Split lower_builtin_methods.rs (1497 lines) by type group and extract derive scaffolding into a factory
sections:
  - id: "04.1"
    title: Split lower_builtin_methods.rs
    status: not-started
  - id: "04.2"
    title: Derive Scaffolding Factory
    status: not-started
  - id: "04.3"
    title: Unify Field Operations
    status: not-started
  - id: "04.4"
    title: Completion Checklist
    status: not-started
---

# Section 04: LLVM Codegen Consolidation

**Status:** Not Started
**Goal:** Split `lower_builtin_methods.rs` (1497 lines, 3× the 500-line limit) into type-grouped submodules. Extract the repeated derive scaffolding into a factory function. Unify the three copies of field-operation TypeInfo dispatch (`emit_field_eq`, `emit_field_compare`, `coerce_to_i64`) into a shared abstraction.

**Current state:**
- `lower_builtin_methods.rs` (1497 lines) handles builtin methods for 18+ types via per-type handler functions, each with its own method name match
- `derive_codegen/mod.rs` (575 lines) repeats a 15-line function setup scaffolding for each of 6 `compile_derive_*()` functions
- `derive_codegen/field_ops.rs` (266 lines) contains three separate TypeInfo match blocks for equality, comparison, and hashing

---

## 04.1 Split `lower_builtin_methods.rs`

### Target Structure

```
compiler/ori_llvm/src/codegen/
├── lower_builtin_methods/
│   ├── mod.rs           — Dispatch entry point (~100 lines)
│   ├── numeric.rs       — int, float, byte methods (~250 lines)
│   ├── text.rs          — str, char methods (~200 lines)
│   ├── containers.rs    — option, result, tuple, ordering methods (~350 lines)
│   └── primitives.rs    — bool, unit, duration, size methods (~150 lines)
```

### Module Responsibilities

**`mod.rs`** — Contains `lower_builtin_method()` entry point which matches on TypeInfo and dispatches to submodule functions. Only dispatching logic, no method implementations.

**`numeric.rs`** — `lower_int_method()`, `lower_float_method()`, `lower_byte_method()`. These share patterns: arithmetic operations, comparison, hashing, numeric conversions.

**`text.rs`** — `lower_str_method()`, `lower_char_method()`. String methods involve runtime calls (`ori_str_*`); char methods are simpler inline operations.

**`containers.rs`** — `lower_option_method()`, `lower_result_method()`, `lower_tuple_method()`, `lower_ordering_method()`. These handle tag-based dispatch (option discriminant, result discriminant, tuple field access).

**`primitives.rs`** — `lower_bool_method()`, `lower_unit_method()`, `lower_duration_method()`, `lower_size_method()`. Simple methods on simple types.

### Split Strategy

Each `lower_*_method()` function moves to its submodule unchanged. The only refactoring is file location — no logic changes.

```rust
// lower_builtin_methods/mod.rs — dispatch only
pub(crate) fn lower_builtin_method(
    fc: &mut FunctionCompiler,
    receiver: ValueId,
    method: &str,
    args: &[ValueId],
    type_info: &TypeInfo,
) -> Option<ValueId> {
    match type_info {
        TypeInfo::Int => numeric::lower_int_method(fc, receiver, method, args),
        TypeInfo::Float => numeric::lower_float_method(fc, receiver, method, args),
        TypeInfo::Byte => numeric::lower_byte_method(fc, receiver, method, args),
        TypeInfo::Str => text::lower_str_method(fc, receiver, method, args),
        TypeInfo::Char => text::lower_char_method(fc, receiver, method, args),
        TypeInfo::Bool => primitives::lower_bool_method(fc, receiver, method, args),
        TypeInfo::Unit => primitives::lower_unit_method(fc, receiver, method, args),
        TypeInfo::Duration => primitives::lower_duration_method(fc, receiver, method, args),
        TypeInfo::Size => primitives::lower_size_method(fc, receiver, method, args),
        TypeInfo::Ordering => containers::lower_ordering_method(fc, receiver, method, args),
        TypeInfo::Option { .. } => containers::lower_option_method(fc, receiver, method, args, type_info),
        TypeInfo::Result { .. } => containers::lower_result_method(fc, receiver, method, args, type_info),
        TypeInfo::Tuple { .. } => containers::lower_tuple_method(fc, receiver, method, args, type_info),
        _ => None,
    }
}
```

- [ ] Create `lower_builtin_methods/` directory
- [ ] Move dispatch entry point to `lower_builtin_methods/mod.rs`
- [ ] Move int/float/byte handlers to `numeric.rs`
- [ ] Move str/char handlers to `text.rs`
- [ ] Move option/result/tuple/ordering handlers to `containers.rs`
- [ ] Move bool/unit/duration/size handlers to `primitives.rs`
- [ ] `cargo t -p ori_llvm` passes
- [ ] Each file under 400 lines
- [ ] `./test-all.sh` passes

---

## 04.2 Derive Scaffolding Factory

### Current Problem

Six `compile_derive_*()` functions in `derive_codegen/mod.rs` all repeat this scaffolding:

```rust
fn compile_derive_eq(fc, module, type_name, type_idx, type_name_str, fields) {
    let method_name = fc.intern("eq");                              // varies
    let sig = make_sig(fc, type_idx, /*params*/ 2, Idx::BOOL);     // varies
    let abi = compute_function_abi(fc, &sig);                       // same
    let symbol = fc.mangle_method(type_name_str, "eq");             // varies
    let func_id = fc.declare_and_bind_derive(symbol, &sig, &abi);   // same
    // ... trait-specific body ...
    emit_derive_return(fc, func_id, result, &abi);                  // same
}
```

### Factory Function

```rust
/// Scaffolding for all derived trait codegen functions.
///
/// Handles: signature construction, ABI computation, symbol mangling,
/// function declaration, parameter binding. The caller provides only
/// the body logic via `emit_body`.
fn with_derive_function<F>(
    fc: &mut FunctionCompiler<'_, '_, '_, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
    emit_body: F,
) where
    F: FnOnce(
        &mut FunctionCompiler<'_, '_, '_, '_>,
        FunctionId,
        &Abi,
        ValueId,        // self_val
        Option<ValueId>, // other_val (None for Clone/Hash/Printable/Debug/Default)
        &[FieldDef],
    ) -> Option<ValueId>,
{
    let method_name_str = trait_kind.method_name();
    let shape = trait_kind.shape();

    let param_count = shape.param_count();
    let return_type = match shape {
        DerivedMethodShape::BinaryPredicate => Idx::BOOL,
        DerivedMethodShape::UnaryIdentity => type_idx,
        DerivedMethodShape::UnaryToInt => Idx::INT,
        DerivedMethodShape::UnaryToStr => Idx::STR,
        DerivedMethodShape::Nullary => type_idx,
        DerivedMethodShape::BinaryToOrdering => Idx::ORDERING,
    };

    let sig = make_sig(fc, type_idx, param_count, return_type);
    let abi = compute_function_abi(fc, &sig);
    let symbol = fc.mangle_method(type_name_str, method_name_str);
    let func_id = fc.declare_and_bind_derive(symbol, &sig, &abi);

    let self_val = if shape.has_self() {
        Some(fc.get_param(func_id, 0))
    } else {
        None
    };

    let other_val = if shape.has_other() {
        Some(fc.get_param(func_id, 1))
    } else {
        None
    };

    let result = emit_body(
        fc, func_id, &abi,
        self_val.unwrap_or(ValueId::INVALID),
        other_val,
        fields,
    );

    if let Some(val) = result {
        emit_derive_return(fc, func_id, val, &abi);
    }
}
```

### Usage

```rust
// BEFORE: 40 lines of scaffolding + body
fn compile_derive_eq(fc, ..., fields) {
    // 15 lines of scaffolding
    // 25 lines of body
}

// AFTER: body only
fn compile_derive_eq(fc, ..., fields) {
    with_derive_function(fc, DerivedTrait::Eq, type_name, type_idx, type_name_str, fields,
        |fc, func_id, abi, self_val, other_val, fields| {
            let other = other_val.expect("Eq has other param");
            // 25 lines of body — just the field comparison logic
            Some(result)
        },
    );
}
```

- [ ] Implement `with_derive_function()` factory
- [ ] Refactor `compile_derive_eq()` to use factory
- [ ] Refactor `compile_derive_clone()` to use factory
- [ ] Refactor `compile_derive_hash()` to use factory
- [ ] Refactor `compile_derive_printable()` to use factory
- [ ] Refactor `compile_derive_debug()` to use factory (or implement if missing)
- [ ] Refactor `compile_derive_default()` to use factory
- [ ] Refactor `compile_derive_comparable()` to use factory
- [ ] `cargo t -p ori_llvm` passes
- [ ] `./llvm-test.sh` passes
- [ ] `./test-all.sh` passes
- [ ] Net line reduction: ~90 lines of repeated scaffolding eliminated

---

## 04.3 Unify Field Operations

### Current Problem

`field_ops.rs` contains three functions that all match on `TypeInfo` to dispatch field-level operations:

1. `emit_field_eq(fc, lhs, rhs, field_type)` — emits equality check per field type
2. `emit_field_compare(fc, lhs, rhs, field_type)` — emits comparison per field type
3. `coerce_to_i64(fc, val, field_type)` — emits hash coercion per field type

Each contains a 15+ arm match on TypeInfo (Int, Float, Bool, Str, Char, Byte, Ordering, Struct, etc.) with the same structure:

```rust
match type_info {
    TypeInfo::Int => /* int-specific operation */,
    TypeInfo::Float => /* float-specific operation */,
    TypeInfo::Str => /* call runtime function */,
    TypeInfo::Struct { .. } => /* recurse into struct method */,
    _ => /* fallback */,
}
```

### Solution: Per-Type Operation Table

Instead of three separate match blocks, define a unified field operation dispatcher:

```rust
/// Emit a field-level operation (eq, compare, or hash) for the given type.
///
/// Dispatches once on TypeInfo, then applies the requested operation.
/// Eliminates the three separate match blocks in emit_field_eq,
/// emit_field_compare, and coerce_to_i64.
pub(crate) fn emit_field_operation(
    fc: &mut FunctionCompiler,
    op: FieldOp,
    lhs: ValueId,
    rhs: Option<ValueId>,  // None for hash (unary)
    field_type: Idx,
) -> ValueId {
    let type_info = fc.type_info_for(field_type);

    match type_info {
        TypeInfo::Int | TypeInfo::Byte | TypeInfo::Char | TypeInfo::Bool => {
            match op {
                FieldOp::Eq => fc.emit_icmp_eq(lhs, rhs.unwrap()),
                FieldOp::Compare => fc.emit_icmp_ordering(lhs, rhs.unwrap()),
                FieldOp::Hash => fc.emit_sext_i64(lhs),
            }
        }
        TypeInfo::Float => {
            match op {
                FieldOp::Eq => fc.emit_fcmp_eq(lhs, rhs.unwrap()),
                FieldOp::Compare => fc.emit_fcmp_ordering(lhs, rhs.unwrap()),
                FieldOp::Hash => fc.emit_float_hash(lhs),
            }
        }
        TypeInfo::Str => {
            match op {
                FieldOp::Eq => fc.emit_call("ori_str_eq", &[lhs, rhs.unwrap()]),
                FieldOp::Compare => fc.emit_call("ori_str_compare", &[lhs, rhs.unwrap()]),
                FieldOp::Hash => fc.emit_call("ori_str_hash", &[lhs]),
            }
        }
        TypeInfo::Struct { name, .. } => {
            let method = match op {
                FieldOp::Eq => "eq",
                FieldOp::Compare => "compare",
                FieldOp::Hash => "hash",
            };
            fc.emit_method_call(name, method, lhs, rhs.as_slice())
        }
        _ => {
            match op {
                FieldOp::Eq => fc.emit_const_bool(true),    // fallback: equal
                FieldOp::Compare => fc.emit_const_ordering_equal(),
                FieldOp::Hash => fc.emit_const_i64(0),
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FieldOp {
    Eq,
    Compare,
    Hash,
}
```

### Callers

```rust
// BEFORE:
let eq = emit_field_eq(fc, lhs_field, rhs_field, field_type);

// AFTER:
let eq = emit_field_operation(fc, FieldOp::Eq, lhs_field, Some(rhs_field), field_type);
```

- [ ] Define `FieldOp` enum
- [ ] Implement `emit_field_operation()` unified dispatcher
- [ ] Refactor `compile_derive_eq()` to use `emit_field_operation(FieldOp::Eq, ...)`
- [ ] Refactor `compile_derive_comparable()` to use `emit_field_operation(FieldOp::Compare, ...)`
- [ ] Refactor `compile_derive_hash()` to use `emit_field_operation(FieldOp::Hash, ...)`
- [ ] Delete `emit_field_eq()`, `emit_field_compare()`, `coerce_to_i64()`
- [ ] `./llvm-test.sh` passes
- [ ] `./test-all.sh` passes
- [ ] Net line reduction: 3 match blocks (~180 lines) → 1 match block (~60 lines)

---

## 04.4 Completion Checklist

- [ ] `lower_builtin_methods.rs` split into 4 submodules, each under 400 lines
- [ ] `lower_builtin_methods/mod.rs` is dispatch-only (~100 lines)
- [ ] `with_derive_function()` factory eliminates scaffolding repetition
- [ ] All 7 `compile_derive_*()` functions use the factory
- [ ] `emit_field_operation()` unifies the three field-level dispatch functions
- [ ] `field_ops.rs` is under 200 lines (down from 266)
- [ ] `derive_codegen/mod.rs` is under 400 lines (down from 575)
- [ ] All LLVM AOT tests pass: `./llvm-test.sh`
- [ ] All spec tests pass: `cargo st`
- [ ] `./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes

**Exit Criteria:** No file in the LLVM codegen layer exceeds 500 lines (excluding test files). Adding a new builtin method for a type means editing one type-specific file, not a 1497-line monolith. Adding a new derived trait means writing the body logic only — scaffolding is handled by the factory. Field operations share one TypeInfo dispatch, not three.

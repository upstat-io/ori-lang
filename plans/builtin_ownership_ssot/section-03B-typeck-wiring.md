---
section: "03B"
title: "Wire ori_types to Read TypeFlow from Registry"
status: not-started
goal: "Type checker reads TypeFlow from MethodDef instead of hard-coding unification logic"
files:
  - compiler/ori_types/src/infer/expr/calls.rs
  - compiler/ori_ir/src/builtin_methods/mod.rs
---

# Section 03B: Wire ori_types to Read TypeFlow from Registry

**Status:** Not Started
**Goal:** Replace the hard-coded `match method_str { "map" | "flat_map" | "fold" | ... }` in `unify_higher_order_constraints()` with a registry lookup on `MethodDef.type_flow`.

---

## 03B.0 Current Problem

**File:** `compiler/ori_types/src/infer/expr/calls.rs:700-760`

Higher-order iterator adapters (`map`, `flat_map`, `fold`) create fresh type variables in their return types. After the closure arguments are inferred, `unify_higher_order_constraints()` unifies those variables with the closure's return type so they resolve to concrete types before codegen.

Currently this function hard-codes which methods need special unification:

```rust
fn unify_higher_order_constraints(engine, method, ret_ty, arg_types) {
    match method_str {
        "map" => { /* closure[0].return → ret.element */ }
        "flat_map" => { /* closure[0].return.element → ret.element */ }
        "fold" | "rfold" => { /* param[0] → ret, closure[1].return → ret */ }
        _ => {} // silently ignored — new methods get no unification
    }
}
```

**Problem:** Adding a new higher-order method (e.g., `List.map`, `Option.map`) requires editing this match statement. Forgetting to do so means the type variable remains unresolved → codegen crash. No compile-time enforcement.

---

## 03B.1 Prerequisite: BuiltinType::Iterator

This section requires `BuiltinType::Iterator` to exist (added in Section 02) so that `find_method(BuiltinType::Iterator, "map")` can find the Iterator.map entry with its `TypeFlow`.

---

## 03B.2 Add Tag-to-BuiltinType Mapping

**File:** `compiler/ori_types/src/infer/expr/calls.rs`

Add a helper to map the type checker's `Tag` to the IR's `BuiltinType`:

```rust
/// Map a type pool Tag to a BuiltinType for registry lookup.
///
/// Returns `None` for tags that don't correspond to builtin types
/// (e.g., Struct, Enum, Function, Var).
fn tag_to_builtin_type(tag: Tag) -> Option<BuiltinType> {
    match tag {
        Tag::Int => Some(BuiltinType::Int),
        Tag::Float => Some(BuiltinType::Float),
        Tag::Bool => Some(BuiltinType::Bool),
        Tag::Str => Some(BuiltinType::Str),
        Tag::Char => Some(BuiltinType::Char),
        Tag::Byte => Some(BuiltinType::Byte),
        Tag::Unit => Some(BuiltinType::Unit),
        Tag::Never => Some(BuiltinType::Never),
        Tag::Duration => Some(BuiltinType::Duration),
        Tag::Size => Some(BuiltinType::Size),
        Tag::Ordering => Some(BuiltinType::Ordering),
        Tag::List => Some(BuiltinType::List),
        Tag::Map => Some(BuiltinType::Map),
        Tag::Option => Some(BuiltinType::Option),
        Tag::Result => Some(BuiltinType::Result),
        Tag::Range => Some(BuiltinType::Range),
        Tag::Set => Some(BuiltinType::Set),
        Tag::Channel => Some(BuiltinType::Channel),
        Tag::Iterator | Tag::DoubleEndedIterator => Some(BuiltinType::Iterator),
        _ => None,
    }
}
```

---

## 03B.3 Pass Tag Through ReceiverDispatch

**File:** `compiler/ori_types/src/infer/expr/calls.rs`

Change `ReceiverDispatch::Return` to carry the receiver's `Tag`:

```rust
enum ReceiverDispatch {
    /// Builtin method resolved. Caller must infer all args, apply TypeFlow, and return.
    Return { ty: Idx, tag: Tag },
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
}
```

Update all `ReceiverDispatch::Return(...)` sites in `resolve_receiver_and_builtin()`:
- Error propagation (line 783): `Return { ty: Idx::ERROR, tag: Tag::Error }`
- Type variable deferral (line 797): `Return { ty: fresh_var, tag }`
- Builtin method found (line 808): `Return { ty: ret, tag }`
- DEI rejection (line 825): `Return { ty: Idx::ERROR, tag }`
- Range float rejection (line 831): `Return { ty: err, tag }`

Update both call sites in `infer_method_call()` and `infer_method_call_named()`:

```rust
ReceiverDispatch::Return { ty, tag } => {
    let arg_types: Vec<Idx> = ...;
    unify_higher_order_constraints(engine, tag, method, ty, &arg_types);
    return ty;
}
```

---

## 03B.4 Replace Hard-Coded Match with TypeFlow Dispatch

**File:** `compiler/ori_types/src/infer/expr/calls.rs`

```rust
/// Unify fresh type variables in builtin method return types with constraints
/// from closure arguments.
///
/// Reads `TypeFlow` from the `MethodDef` registry instead of hard-coding
/// unification logic per method name. This is the consumer side of the
/// TypeFlow single source of truth.
fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    tag: Tag,
    method: Name,
    ret_ty: Idx,
    arg_types: &[Idx],
) {
    use ori_ir::builtin_methods::{find_method, TypeFlow};

    let Some(method_str) = engine.lookup_name(method) else { return };

    // Look up TypeFlow from the registry
    let type_flow = tag_to_builtin_type(tag)
        .and_then(|bt| find_method(bt, method_str))
        .map(|m| m.type_flow)
        .unwrap_or(TypeFlow::Standard);

    match type_flow {
        TypeFlow::Standard => {}

        TypeFlow::ClosureOutputBecomesElement { closure_param } => {
            // ret_ty is Iterator<var>. arg_types[closure_param] is (T) -> U.
            // Unify var with U.
            let Some(&closure_ty) = arg_types.get(closure_param as usize) else { return };
            let resolved_ret = engine.resolve(ret_ty);
            if !engine.pool().tag(resolved_ret).is_iterator() { return; }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let _ = engine.unify().unify(elem_var, closure_ret);
            }
        }

        TypeFlow::ClosureOutputFlatElement { closure_param } => {
            // ret_ty is Iterator<var>. arg_types[closure_param] is (T) -> Iterator<U>.
            // Unify var with U.
            let Some(&closure_ty) = arg_types.get(closure_param as usize) else { return };
            let resolved_ret = engine.resolve(ret_ty);
            if !engine.pool().tag(resolved_ret).is_iterator() { return; }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let resolved_inner = engine.resolve(closure_ret);
                if engine.pool().tag(resolved_inner).is_iterator() {
                    let inner_elem = engine.pool().iterator_elem(resolved_inner);
                    let _ = engine.unify().unify(elem_var, inner_elem);
                }
            }
        }

        TypeFlow::Accumulator { init_param, closure_param } => {
            // ret_ty is a fresh var. Unify with init and closure return.
            if let Some(&init_ty) = arg_types.get(init_param as usize) {
                let _ = engine.unify().unify(ret_ty, init_ty);
            }
            if let Some(&closure_ty) = arg_types.get(closure_param as usize) {
                let resolved_closure = engine.resolve(closure_ty);
                if engine.pool().tag(resolved_closure) == Tag::Function {
                    let closure_ret = engine.pool().function_return(resolved_closure);
                    let _ = engine.unify().unify(ret_ty, closure_ret);
                }
            }
        }
    }
}
```

**Key:** The `unwrap_or(TypeFlow::Standard)` handles methods not yet in the registry — they get no-op unification, which is correct. As methods are added to the registry (Section 02), they automatically get correct TypeFlow handling without any changes to `calls.rs`.

---

## 03B.5 Delete Hard-Coded Method Names

After the wiring is complete, verify no hard-coded method name strings remain:

```bash
grep -n '"map"\|"flat_map"\|"fold"\|"rfold"' compiler/ori_types/src/infer/expr/calls.rs
# Expected: 0 results
```

---

## 03B.6 Verification

- [ ] `cargo c -p ori_types` — compiles with new import and TypeFlow dispatch
- [ ] `cargo t -p ori_types` — type inference tests pass (same behavior, different source)
- [ ] `grep` for hard-coded method names in `calls.rs` returns 0
- [ ] `./test-all.sh` — full suite green

**Behavior invariant:** The function produces exactly the same unification results as the old hard-coded version. The only change is where the dispatch decision comes from (registry vs. string match).

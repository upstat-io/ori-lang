---
section: "01"
title: "Type Checker Infrastructure"
status: complete
goal: "Discover monomorphization instances during type checking and propagate them to TypedModule"
sections:
  - id: "01.1"
    title: "FunctionSig scheme_var_ids"
    status: complete
  - id: "01.2"
    title: "MonoInstance and Recording Infrastructure"
    status: complete
  - id: "01.3"
    title: "Pool Type Substitution"
    status: complete
  - id: "01.4"
    title: "Record Mono Instances at Call Sites"
    status: complete
  - id: "01.5"
    title: "Propagate to TypedModule"
    status: complete
---

# Section 01: Type Checker Infrastructure

**Goal:** During type checking, when a generic function is called with concrete type arguments, record a `MonoInstance` capturing the function name, concrete generic args, substituted parameter/return types, and a `body_type_map` for the ARC lowerer. Propagate these through `ModuleChecker` into `TypedModule`.

---

## 01.1 FunctionSig `scheme_var_ids` — DONE

**File:** `compiler/ori_types/src/output/mod.rs`, `compiler/ori_types/src/check/signatures/mod.rs`

The type checker already computes `var_ids` when building function signatures for generic functions — they're the Pool variable IDs for the scheme's quantified type variables, parallel to `type_params`. They just weren't stored. Wire them into `FunctionSig` so the monomorphizer can build the `var_id -> concrete_type` substitution map.

- [x] Add `scheme_var_ids: Vec<u32>` field to `FunctionSig`
- [x] Store `var_ids` in signature construction (`check/signatures/mod.rs`)
- [x] Update all `FunctionSig` constructors (`simple()`, `synthetic()`, `builtin()`, etc.) with `scheme_var_ids: Vec::new()`
- [x] Compilation clean, all tests pass

---

## 01.2 MonoInstance and Recording Infrastructure — DONE

**Files:** `compiler/ori_types/src/output/mod.rs`, `compiler/ori_types/src/infer/mod.rs`, `compiler/ori_types/src/check/mod.rs`, `compiler/ori_types/src/lib.rs`

Define the core data structures for recording monomorphization instances, and add recording/extraction methods to `InferEngine` and `ModuleChecker`.

**Design decision (GenericArg enum):** After studying Rust (`GenericArgKind`), Swift (`SubstitutionMap`), Zig (`InternPool.Index`), and Lean 4 (selective monomorphization), adopted a unified `GenericArg` enum instead of `type_args: Vec<Idx>`. This accommodates future const generics without structural changes. See `docs/ori_lang/0.1-alpha/design/monomorphization-architecture.md` for full rationale.

```rust
pub enum GenericArg {
    Type(Idx),
    Const(ConstValue),
}

pub enum ConstValue {
    Int(i64),
    Bool(bool),
    // Future phases add variants as const generic eligibility expands
}

pub struct MonoInstance {
    pub fn_name: Name,
    pub generic_args: Vec<GenericArg>,
    pub concrete_param_types: Vec<Idx>,
    pub concrete_return_type: Idx,
    pub body_type_map: FxHashMap<Idx, Idx>,
}
```

- [x] Define `ConstValue` enum in `output/mod.rs`
- [x] Define `GenericArg` enum in `output/mod.rs`
- [x] Define `MonoInstance` struct with manual `Hash` impl (hashes only `fn_name` + `generic_args`)
- [x] Add `mono_instances: Vec<MonoInstance>` to `TypedModule`
- [x] Add `mono_instances` field + `record_mono_instance()` + `take_mono_instances()` to `InferEngine`
- [x] Add `mono_instances` field + `accumulate_mono_instances()` to `ModuleChecker`
- [x] Dedup in `finish_with_pool()` by `(fn_name, generic_args)`
- [x] Re-export `GenericArg`, `ConstValue`, `MonoInstance` from `lib.rs`
- [x] Compilation clean, all tests pass

---

## 01.3 Pool Type Substitution — DONE

**File:** `compiler/ori_types/src/pool/substitute.rs` (NEW)

Recursive type substitution using mutable Pool. Called during type checking to build the `body_type_map`. Follows the same structural recursion pattern as `UnifyEngine::substitute()` in `unify/mod.rs` but operates as a standalone function.

```rust
pub fn substitute_in_pool(
    pool: &mut Pool,
    ty: Idx,
    var_subst: &FxHashMap<u32, Idx>,
) -> Idx
```

Handles all Tag variants that can contain type variables:
- **Var**: Direct var_id lookup, follow links, check generalized state
- **Single-child containers**: List, Option, Set, Channel, Range, Iterator, DoubleEndedIterator
- **Two-child containers**: Map, Result
- **Borrowed**: Inner type + lifetime preserved
- **Variable-length**: Function (params + return), Tuple (elements), Applied (name + args)
- **Fast path**: Skip recursion when `!pool.flags(ty).contains(TypeFlags::HAS_VAR)`

New types are interned in the pool (deduplication is automatic).

- [x] Create `pool/substitute/mod.rs` with `substitute_in_pool()` and per-tag helpers
- [x] Wire into pool module (`mod substitute; pub use substitute::substitute_in_pool;`)
- [x] Re-export from `lib.rs`
- [x] Fix `compute_flags()` bug: Applied types now propagate `HAS_VAR` from type args (was `IS_NAMED` only)
- [x] 17 unit tests: primitive passthrough, var substituted/not-in-map, single-child containers (list, option), two-child (map, result), function type, tuple, applied type, nested types, interning dedup, no-op, linked var, generalized var
- [x] Compilation clean, all 10,021 tests pass

---

## 01.4 Record Mono Instances at Call Sites

**File:** `compiler/ori_types/src/infer/expr/calls.rs`

After the argument-checking loop in `infer_call()` and `infer_call_named()`, detect generic function calls and record `MonoInstance` values.

```rust
if let Some(fn_name) = func_name_id {
    if let Some(sig) = engine.get_signature(fn_name) {
        if sig.is_generic() && !sig.scheme_var_ids.is_empty() {
            record_mono_call(engine, fn_name, sig, &params, ret);
        }
    }
}
```

The `record_mono_call()` helper:
1. For each type param, use `generic_param_mapping[i] = Some(param_idx)` to get concrete type via `engine.resolve(params[param_idx])`
2. Build `var_subst: FxHashMap<u32, Idx>` from `scheme_var_ids` + concrete types
3. Build `body_type_map` by calling `substitute_in_pool()` on relevant pool entries with `HAS_VAR`
4. Create `MonoInstance` and push via `engine.record_mono_instance()`

- [x] Add `maybe_record_mono_instance()` helper in `calls.rs`
- [x] Call from `infer_call()` after argument checking
- [x] Call from `infer_call_named()` after argument checking
- [x] Handle case where type args can't be fully resolved (skip recording, not an error)
- [x] Fix `scheme_var_ids` ordering: iterate `type_params` in order (was HashMap values → random order)
- [x] Integration tests: generic identity, two-param, non-generic, dedup, different type args

---

## 01.5 Propagate to TypedModule

**Files:** `compiler/ori_types/src/check/bodies/mod.rs`, `compiler/ori_types/src/check/mod.rs`

After `engine.take_expr_types()` in body checking, also call `engine.take_mono_instances()` and accumulate them in the module checker. The dedup logic already exists in `ModuleChecker::finish_with_pool()`.

- [x] Call `engine.take_mono_instances()` after body checking in `check/bodies/mod.rs` (all 4 extraction points)
- [x] Pass extracted instances to `ModuleChecker::accumulate_mono_instances()`
- [x] Verify dedup works: `same_generic_call_twice_deduplicates` integration test
- [x] Integration test: `check_module()` on source with generic calls → `TypedModule.mono_instances` is non-empty
- [x] Compilation clean, all 10,026 tests pass

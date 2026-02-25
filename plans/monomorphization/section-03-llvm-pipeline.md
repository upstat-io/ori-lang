---
section: "03"
title: "LLVM Pipeline Integration"
status: complete
goal: "Collect mono instances into concrete functions, lower them through ARC, declare/define in LLVM, and resolve call sites"
sections:
  - id: "03.1"
    title: "Monomorphization Collection Pass"
    status: complete
  - id: "03.2"
    title: "Evaluator Pipeline Integration"
    status: complete
  - id: "03.3"
    title: "FunctionCompiler Declare/Define"
    status: complete
  - id: "03.4"
    title: "Call Site Resolution"
    status: complete
---

# Section 03: LLVM Pipeline Integration

**Goal:** Wire monomorphized functions into the existing LLVM compilation pipeline. The key property: monomorphized functions have non-generic `FunctionSig` values, so existing `declare_function()` / `define_function_body()` work unchanged. The new code is confined to (1) collecting and mangling, (2) ARC lowering with substitution, (3) registration in the function table, and (4) call site resolution.

---

## 03.1 Monomorphization Collection Pass

**File:** `compiler/ori_llvm/src/monomorphize/mod.rs` (NEW)

For each unique `MonoInstance` from `TypedModule`, produce a `MonoFunction` with a mangled name and concrete (non-generic) signature.

```rust
pub struct MonoFunction {
    pub mangled_name: Name,
    pub original_name: Name,
    pub sig: FunctionSig,         // Concrete (is_generic() = false)
    pub body_type_map: FxHashMap<Idx, Idx>,
}

pub fn collect_mono_functions(
    mono_instances: &[MonoInstance],
    function_sigs: &FxHashMap<Name, FunctionSig>,
    interner: &StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction>
```

For each `MonoInstance`:
1. Find the generic `FunctionSig` by `fn_name`
2. Create a monomorphic `FunctionSig` (empty `type_params`, concrete `param_types`/`return_type`)
3. Compute mangled name using the scheme from `00-overview.md` Section "Name Mangling Scheme"
4. Return `MonoFunction` carrying the `body_type_map` for ARC lowering

- [x] Create `monomorphize/mod.rs` with `MonoFunction` struct
- [x] Implement `collect_mono_functions()`
- [x] Implement `mangle_mono_name()` helper using type encoding table
- [x] Implement `encode_type()` recursive type-to-string encoder
- [x] Wire into `ori_llvm/src/lib.rs` (`mod monomorphize; pub use ...`)
- [x] Unit tests: mangling produces expected names, collection deduplicates, non-generic sigs produced

---

## 03.2 Evaluator Pipeline Integration

**File:** `compiler/ori_llvm/src/evaluator.rs`

After the existing ARC lowering loop for module functions, add a loop for monomorphized functions. Each mono function reuses the same canonical IR body (shared, not cloned) but passes its `body_type_map` as the `type_subst`.

- [x] Call `collect_mono_functions()` after existing signature collection
- [x] Add ARC lowering loop for mono functions
- [x] Pass `mono_functions` to `FunctionCompiler` (for declare/define)
- [x] Pass mono function names to call site resolution data

---

## 03.3 FunctionCompiler Declare/Define

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

- [x] Add `declare_mono_functions()` method — loops over mono functions, calls `declare_function()` with mangled name + concrete sig
- [x] Add `define_mono_functions()` method — loops over mono functions, calls `define_function_body_arc_with_subst()` with `type_subst`
- [x] Refactor `define_function_body_arc` → `define_function_body_arc_with_subst` to accept optional `type_subst`
- [x] Verify: existing non-generic functions unchanged (10,035 tests pass)

---

## 03.4 Call Site Resolution

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

In `emit_apply()` and `emit_invoke()`, when the callee is a generic function (not found by name in the function table), resolve it via the `mono_dispatch` index.

**Architecture:** `mono_dispatch: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>` maps original generic name → list of `(concrete_param_types, mangled_name)`. Built in `declare_mono_functions()`, passed to `ArcIrEmitter`.

`lookup_mono_dispatch()`:
1. Look up `callee` in `mono_dispatch`
2. Get concrete arg types from `func.var_type(args[i])`
3. Find entry where `param_types` matches
4. Resolve `mangled_name` through `self.functions`

- [x] Add `mono_dispatch` field to `ArcIrEmitter` struct
- [x] Add `mono_dispatch` parameter to `ArcIrEmitter::new()`
- [x] Implement `lookup_mono_dispatch()` method
- [x] Add fallback in both `emit_apply()` and `emit_invoke()` lookup chains (step 4 of 5)
- [x] Add `mono_dispatch` field to `FunctionCompiler`, build in `declare_mono_functions()`
- [x] Pass `&self.mono_dispatch` to all `ArcIrEmitter::new()` calls (3 production + 19 test)
- [x] Verify: all 10,035 tests pass, non-generic calls unchanged

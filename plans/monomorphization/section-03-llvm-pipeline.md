---
section: "03"
title: "LLVM Pipeline Integration"
status: not-started
goal: "Collect mono instances into concrete functions, lower them through ARC, declare/define in LLVM, and resolve call sites"
sections:
  - id: "03.1"
    title: "Monomorphization Collection Pass"
    status: not-started
  - id: "03.2"
    title: "Evaluator Pipeline Integration"
    status: not-started
  - id: "03.3"
    title: "FunctionCompiler Declare/Define"
    status: not-started
  - id: "03.4"
    title: "Call Site Resolution"
    status: not-started
---

# Section 03: LLVM Pipeline Integration

**Goal:** Wire monomorphized functions into the existing LLVM compilation pipeline. The key property: monomorphized functions have non-generic `FunctionSig` values, so existing `declare_function()` / `define_function_body()` work unchanged. The new code is confined to (1) collecting and mangling, (2) ARC lowering with substitution, (3) registration in the function table, and (4) call site resolution.

---

## 03.1 Monomorphization Collection Pass

**File:** `compiler/ori_llvm/src/monomorphize.rs` (NEW)

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
    interner: &mut StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction>
```

For each `MonoInstance`:
1. Find the generic `FunctionSig` by `fn_name`
2. Create a monomorphic `FunctionSig` (empty `type_params`, concrete `param_types`/`return_type`)
3. Compute mangled name using the scheme from `00-overview.md` Section "Name Mangling Scheme"
4. Return `MonoFunction` carrying the `body_type_map` for ARC lowering

- [ ] Create `monomorphize.rs` with `MonoFunction` struct
- [ ] Implement `collect_mono_functions()`
- [ ] Implement `mangle_mono_name()` helper using type encoding table
- [ ] Implement `encode_type()` recursive type-to-string encoder
- [ ] Wire into `ori_llvm/src/lib.rs` (`mod monomorphize; pub use ...`)
- [ ] Unit tests: mangling produces expected names, collection deduplicates, non-generic sigs produced

---

## 03.2 Evaluator Pipeline Integration

**File:** `compiler/ori_llvm/src/evaluator.rs`

After the existing ARC lowering loop for module functions, add a loop for monomorphized functions. Each mono function reuses the same canonical IR body (shared, not cloned) but passes its `body_type_map` as the `type_subst`.

```rust
for mono_fn in &mono_functions {
    let params: Vec<(Name, Idx)> = mono_fn.sig.param_names.iter()
        .zip(mono_fn.sig.param_types.iter())
        .map(|(&n, &t)| (n, t)).collect();
    let body_id = canon.root_for(mono_fn.original_name).unwrap_or(canon.root);
    let (arc_fn, lambdas) = ori_arc::lower_function_can(
        mono_fn.mangled_name,
        &params,
        mono_fn.sig.return_type,
        body_id,
        canon,
        interner,
        self.pool,
        &mut arc_problems,
        false,
        Some(&mono_fn.body_type_map),
    );
    arc_functions.push(arc_fn);
    arc_functions.extend(lambdas);
}
```

- [ ] Call `collect_mono_functions()` after existing signature collection
- [ ] Add ARC lowering loop for mono functions
- [ ] Pass `mono_functions` to `FunctionCompiler` (for declare/define)
- [ ] Pass mono function names to call site resolution data

---

## 03.3 FunctionCompiler Declare/Define

**File:** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`

In `declare_all()` and `define_all()`, add loops for `MonoFunction` entries. These have non-generic `FunctionSig` values, so existing `declare_function()` / `define_function_body()` work unchanged — no special handling needed.

- [ ] Add `mono_functions: &[MonoFunction]` parameter to `declare_all()` and `define_all()`
- [ ] Loop over mono functions in `declare_all()`, calling `declare_function()` with mangled name + concrete sig
- [ ] Loop over mono functions in `define_all()`, calling `define_function_body()` with mangled name
- [ ] Verify: existing non-generic functions unchanged

---

## 03.4 Call Site Resolution

**File:** `compiler/ori_llvm/src/codegen/arc_emitter/mod.rs`

In `emit_apply()`, when the callee is a generic function (not found in the normal function table), resolve it to the mangled monomorphized name.

```rust
.or_else(|| self.resolve_mono_call(callee, args, func))
```

`resolve_mono_call()`:
1. Check if `callee` is a known generic function (stored in a lookup set from `FunctionCompiler`)
2. Get concrete arg types from `func.var_type(args[i])`
3. Compute type args from the ARC IR's concrete types
4. Compute mangled name (same `mangle_mono_name()` from Section 03.1)
5. Look up mangled name in `self.functions`

**Alternative simpler approach:** Register mono functions under BOTH names — the mangled name AND an `(original_name, concrete_arg_types)` lookup key. The emitter computes arg types and looks up directly.

- [ ] Add generic function name set to emitter state
- [ ] Implement `resolve_mono_call()` method
- [ ] Add fallback in `emit_apply()` function lookup chain
- [ ] Test: generic call resolves to mangled function, non-generic calls unchanged

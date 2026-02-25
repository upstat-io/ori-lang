# Monomorphization Plan Index

> **Status: COMPLETE (2026-02-25)** — All 4 sections implemented and verified. 10,040 tests pass.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Type Checker Infrastructure
**File:** `section-01-type-checker.md` | **Status:** Complete

```
MonoInstance, GenericArg, ConstValue, scheme_var_ids, FunctionSig
substitute_in_pool, type substitution, var_subst, HAS_VAR fast path
record_mono_instance, take_mono_instances, InferEngine
infer_call, infer_call_named, generic_param_mapping
body_type_map, concrete_param_types, concrete_return_type
accumulate_mono_instances, dedup, ModuleChecker, TypedModule
Pool, Tag, Idx, VarState, TypeFlags
```

---

### Section 02: ARC Lowering Integration
**File:** `section-02-arc-lowering.md` | **Status:** Complete

```
lower_function_can, type_subst, ArcLowerer, resolve_body_type
CanNode.ty, expression type lookup, body_type_map
ori_arc, ArcFunction, ARC IR, retain, release, drop
type-specific RC, concrete types, substitution map
```

---

### Section 03: LLVM Pipeline Integration
**File:** `section-03-llvm-pipeline.md` | **Status:** Complete

```
MonoFunction, collect_mono_functions, monomorphize/mod.rs
mangled_name, name mangling, $m$, type encoding
evaluator.rs, compile_common.rs, declare_all, define_all, FunctionCompiler
emit_apply, lookup_mono_dispatch, mono_dispatch, call site resolution
generic function lookup, arg type resolution
arc_emitter, function_compiler, declare_mono_functions, define_mono_functions
```

---

### Section 04: Verification
**File:** `section-04-verification.md` | **Status:** Complete

```
test_aot_generic_identity, test_aot_generic_pair
test_aot_generic_three_type_params, test_aot_generic_calling_non_generic
test_aot_generic_two_specializations
test-all.sh, cargo blr, clippy-all.sh, FastISel bug guard
spec.rs, AOT tests, monomorphization tests
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Type Checker Infrastructure | `section-01-type-checker.md` | Complete |
| 02 | ARC Lowering Integration | `section-02-arc-lowering.md` | Complete |
| 03 | LLVM Pipeline Integration | `section-03-llvm-pipeline.md` | Complete |
| 04 | Verification | `section-04-verification.md` | Complete |

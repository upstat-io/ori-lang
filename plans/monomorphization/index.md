# Monomorphization Plan Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Type Checker Infrastructure
**File:** `section-01-type-checker.md` | **Status:** In Progress

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
**File:** `section-02-arc-lowering.md` | **Status:** Not Started

```
lower_function_can, type_subst, ArcLowerer, resolve_body_type
CanNode.ty, expression type lookup, body_type_map
ori_arc, ArcFunction, ARC IR, retain, release, drop
type-specific RC, concrete types, substitution map
```

---

### Section 03: LLVM Pipeline Integration
**File:** `section-03-llvm-pipeline.md` | **Status:** Not Started

```
MonoFunction, collect_mono_functions, monomorphize.rs
mangled_name, name mangling, $m$, type encoding
evaluator.rs, declare_all, define_all, FunctionCompiler
emit_apply, resolve_mono_call, call site resolution
generic function lookup, arg type resolution
arc_emitter, function_compiler, declare_function
```

---

### Section 04: Verification
**File:** `section-04-verification.md` | **Status:** Not Started

```
test_aot_generic_identity, test_aot_generic_pair
dual-execution, dual-exec-verify.sh, llvm-test.sh
test-all.sh, cargo blr, FastISel bug guard
spec.rs, AOT tests, monomorphization tests
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Type Checker Infrastructure | `section-01-type-checker.md` | In Progress |
| 02 | ARC Lowering Integration | `section-02-arc-lowering.md` | Not Started |
| 03 | LLVM Pipeline Integration | `section-03-llvm-pipeline.md` | Not Started |
| 04 | Verification | `section-04-verification.md` | Not Started |

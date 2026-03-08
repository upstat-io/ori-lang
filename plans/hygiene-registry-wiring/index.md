---
reroute: false
name: "Hygiene Fixes"
full_name: "Hygiene Fixes for Registry Wiring"
status: active
order: 999
parallel: true
---

# Hygiene Fixes for Registry Wiring — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Critical Fixes (ori_llvm)
**File:** `section-01-critical-fixes.md` | **Status:** Not Started

```
Result, err_ty, emit_result_equals, emit_result_compare, emit_result_hash
compound_type_impls.rs, arc_emitter, builtins, correctness
WASM, JS wrapper, string param, memory leak, void return
generate_js_wrapper, wasm/mod.rs, cleanup, encodeString
```

---

### Section 02: Drift Fixes (ori_types)
**File:** `section-02-drift-fixes.md` | **Status:** Not Started

```
Range, float, iteration, method list, duplication
iter, to_list, collect, RANGE_FLOAT_ITERATION_METHODS
methods/mod.rs, method_call.rs, deduplication
```

---

### Section 03: Registry Cleanup (ori_registry)
**File:** `section-03-registry-cleanup.md` | **Status:** Not Started

```
tags/mod.rs, 500-line limit, ReturnTag, TypeProjection, extract
SELF_PARAM, ParamDef, duplication, 15 def files
must_use, query, method_names_for, borrowing_methods
str.from_utf8, associated, MethodDef, constructor bypass
bool.rs, test file, missing tests
```

---

### Section 04: Type Checker Polish (ori_types)
**File:** `section-04-typeck-polish.md` | **Status:** Not Started

```
return_tag_to_idx, must_use, registry_bridge
import style, super::super, crate::infer, InferEngine
import grouping, methods/mod.rs, decorative banners
lib.rs, section markers, banners
```

---

### Section 05: LLVM Bloat Reduction (ori_llvm)
**File:** `section-05-llvm-bloat.md` | **Status:** Not Started

```
compile_module_with_tests, 185 lines, extract helpers
compile.rs, prepare_compilation_context, compile_functions
emit_field_operation, 108 lines, field_ops.rs, struct, enum
clippy::too_many_lines, crate-level allow, per-function expect
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Not Started

```
test-all.sh, clippy-all.sh, cleanup, delete plan
verification, no behavior change, no regressions
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Critical Fixes (ori_llvm) | `section-01-critical-fixes.md` |
| 02 | Drift Fixes (ori_types) | `section-02-drift-fixes.md` |
| 03 | Registry Cleanup (ori_registry) | `section-03-registry-cleanup.md` |
| 04 | Type Checker Polish (ori_types) | `section-04-typeck-polish.md` |
| 05 | LLVM Bloat Reduction (ori_llvm) | `section-05-llvm-bloat.md` |
| 06 | Verification | `section-06-verification.md` |

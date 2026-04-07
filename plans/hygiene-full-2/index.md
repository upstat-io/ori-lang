---
reroute: true
name: "Hygiene Full-2"
full_name: "Implementation Hygiene Full Sweep #2"
status: active
order: 3
---

# Hygiene Full-2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Disposable:** Delete this plan directory when all sections are complete.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Runtime COW Protocol Centralization
**File:** `section-01-rt-cow-protocol.md` | **Status:** Not Started

```
cow_mode, ori_rc_is_unique, is_slice_cap, COW uniqueness
propagate_elem_header, propagate_header, write_list_output
write_map_struct, write_set_struct, cow.rs, cow_structural.rs
cow_sort, map/cow.rs, set/cow, ori_rt, SAFETY comments
unsafe blocks, raw pointer, slice_original_data
```

---

### Section 02: Evaluator Algorithmic DRY
**File:** `section-02-eval-dry.md` | **Status:** Not Started

```
eval_iter_fold, eval_iter_count, eval_iter_find, eval_iter_any
eval_iter_all, eval_iter_for_each, eval_iter_collect
iterator consumers, drive_iterator, higher-order function
eval_option_map, eval_option_and_then, eval_result_map
Option/Result method handlers, collection_ops.rs
dispatch_check, is_collection_dispatched, resolve_iterator_method
all_iterator_variants, BuiltinMethodNames, OpNames, interned_names
```

---

### Section 03: Cross-Backend Dispatch Unification
**File:** `section-03-cross-backend-dispatch.md` | **Status:** Not Started

```
ori_eval, ori_llvm, parallel dispatch, method dispatch
builtin methods, str methods, map methods, set methods
iterator methods, trait methods, operator dispatch
declare_builtins!, TypeInfo, Value, ori_registry
registry-driven, enforcement test, backend_required
method coverage gap, eval-only methods
```

---

### Section 04: Type Resolution DRY
**File:** `section-04-type-resolution-dry.md` | **Status:** Not Started

```
ParsedType, resolve_parsed_type_simple, resolve_type_with_params
resolve_type_with_self_inner, resolve_and_check_type_with_vars
resolve_parsed_type, TypeResolver, type_resolution.rs
WellKnownNames, resolve_well_known_generic, is_concrete_named_type
ori_types, check/registration, infer/expr, check/signatures
Unit, Never, Function, TypeDef, ori_registry, trait satisfaction
```

---

### Section 05: LLVM Codegen Internal DRY
**File:** `section-05-llvm-dry.md` | **Status:** Not Started

```
iterator dispatch stanzas, COW list mutation dispatch
emit_cow_list_op, emit_result_debug, emit_nested_result_debug
declare_builtins!, traits.rs, primitives.rs, compound_traits.rs
runtime_mappings.rs, lookup_jit_address, RT_FUNCTIONS
arg_vals guard, thunks.rs, get_or_create_thunk
```

---

### Section 06: Lexer/Parser DRY
**File:** `section-06-lexer-parser-dry.md` | **Status:** Complete

```
cook_template_head, cook_template_middle, cook_template_tail
cook_template_complete, cook_string, escape_cooking.rs
cook_duration, cook_size, cook_int, cook_hex_int, cook_bin_int
compound-assignment operators, plus, star, percent, caret
expect_ident, expect_member_name, expect_ident_or_keyword
outcome/mod.rs macros, cursor/mod.rs identifiers
```

---

### Section 07: Stale Annotations and Decorative Banners
**File:** `section-07-stale-annotations.md` | **Status:** Not Started

```
TPR-01, TPR-03, TPR-04, stale plan annotations
decorative banners, // ===, // ---
bare TODO, FIXME, HACK
module_parse.rs, compile.rs, lib.rs
ori_types, ori_parse, ori_llvm, ori_repr, ori_rt
```

---

### Section 08: File and Function Size Violations
**File:** `section-08-file-size.md` | **Status:** Not Started

```
500-line limit, 100-line function limit, BLOAT, file splitting
submodule extraction, function decomposition, helper extraction
operators.rs 799, runtime_functions.rs 1606, terminators.rs 745
errors/mod.rs 1018, terminal/mod.rs 841
ori_types, ori_llvm, ori_arc, ori_eval, ori_patterns
ori_ir, oric, ori_diagnostic, ori_fmt, ori_rt, ori_parse
69 files, 31+ functions, SIZE EXEMPTION
```

---

### Section 09: SAFETY Comments for ori_rt
**File:** `section-09-safety-comments.md` | **Status:** Not Started

```
SAFETY comment, unsafe block, raw pointer arithmetic
cow_structural.rs, cow_sort/mod.rs, map/cow.rs
set/cow/basic.rs, set/cow/algebra.rs
iterator/consumers.rs, iterator/adapters.rs
string/methods, string/ops, rc/list_rc, format
io/mod.rs, io/jit_recovery.rs, rc/allocate.rs
ori_rt, extern "C", FFI, ~289 undocumented blocks
```

---

### Section 10: Cleanup
**File:** `section-10-cleanup.md` | **Status:** Not Started

```
test-all.sh, clippy-all.sh, cleanup
delete plan, hygiene-full-2
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Runtime COW Protocol Centralization | `section-01-rt-cow-protocol.md` |
| 02 | Evaluator Algorithmic DRY | `section-02-eval-dry.md` |
| 03 | Cross-Backend Dispatch Unification | `section-03-cross-backend-dispatch.md` |
| 04 | Type Resolution DRY | `section-04-type-resolution-dry.md` |
| 05 | LLVM Codegen Internal DRY | `section-05-llvm-dry.md` |
| 06 | Lexer/Parser DRY | `section-06-lexer-parser-dry.md` |
| 07 | Stale Annotations and Decorative Banners | `section-07-stale-annotations.md` |
| 08 | File and Function Size Violations (69 files, 31+ functions) | `section-08-file-size.md` |
| 09 | SAFETY Comments for ori_rt | `section-09-safety-comments.md` |
| 10 | Cleanup | `section-10-cleanup.md` |

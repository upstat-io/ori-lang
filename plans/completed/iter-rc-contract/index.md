---
reroute: true
name: "Iter RC"
full_name: "Iterator-Collection RC Ownership Contract"
status: resolved
order: 1
---

> **Historical Note:** The `__for_coll` phantom binding mechanism described in this plan was removed by the `rc-header-elem-dec` plan (2026-03-22) and replaced with header-based element cleanup via `elem_dec_fn` in the V5 RC header. References to `__for_coll` below are historical.

# Iterator-Collection RC Ownership Contract Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

### Section 01: Root Cause Analysis & Design
**File:** `section-01-root-cause.md` | **Status:** Complete

```
iterator ownership, elem_dec_fn, NULL function pointer, double-free
ori_buffer_rc_dec, ori_buffer_drop_unique, ori_iter_drop, ori_iter_from_list
__for_coll phantom, for-do vs for-yield parity, ArcVarId lifetime scoping
AIMS emit_defined_dead, collect_iter_element_defs, RcDec suppression
list_builtins.rs, for_yield.rs, for_iterator.rs, loops.rs
emit_rc/edge_cleanup.rs, realize/walk.rs, emit_rc/helpers.rs, element_fn_gen.rs
drop_elements_and_free, _ori_elem_dec, dec_value_rc
map_builtins.rs, ori_iter_from_map, ori_map_buffer_rc_dec, key_dec_fn, val_dec_fn
```

---

### Section 02: Fix Iterator elem_dec_fn (List, Map, Set)
**File:** `section-02-elem-dec-fn.md` | **Status:** Complete

```
elem_dec_fn, get_or_generate_elem_dec_fn, element_fn_gen.rs
ori_iter_from_list, list_builtins.rs, emit_list_iter
ori_iter_from_map, map_builtins.rs, emit_map_iter, key_dec_fn, val_dec_fn
emit_auto_iter, TypeInfo::Set, builtins/mod.rs
NULL function pointer, element cleanup, _ori_elem_dec, dec_value_rc
Option<str>, tag-switch dispatch, sum type payload
[str], [[int]], closure list, struct with Drop fields
{str: int} map, {str: str} map, ori_map_buffer_rc_dec
drop_elements_and_free, ori_buffer_rc_dec
```

---

### Section 03: Fix For-Yield RC Scoping
**File:** `section-03-for-yield-rc.md` | **Status:** Complete

```
for-yield, lower_for_yield_iterator, prepare_iterator, for_yield.rs
__for_coll phantom, block param threading, mutable var scoping
AIMS extra RcDec, source list double-free, ArcVarId lifetime
bb10 spurious dec, post-loop scope leakage, Switch edge cleanup
RcDec/RcInc pairs, emit_defined_dead, collect_iter_element_defs
lower_for_yield vs lower_for_iterator parity
break, continue, continue value, LoopContext, loop_ctx
propagate_borrowed_closure, project_borrowed_defs, all_borrowed_defs
walk.rs BLOAT, realize/mod.rs BLOAT, transfer/mod.rs BLOAT
```

---

### Section 04: For-Do / For-Yield Parity Audit
**File:** `section-04-parity-audit.md` | **Status:** Complete

```
for-do, for-yield, parity, structural comparison
__for_coll phantom, mutable var threading, exit block cleanup
element types: str, [int], Option<str>, closures, structs, maps, Set<str>
RC trace, ORI_TRACE_RC, ORI_CHECK_LEAKS, Valgrind
iter_element_defs, project_borrowed_defs, all_borrowed_defs
ori_iter_from_map, ori_map_buffer_rc_dec, map iterator path
```

---

### Section 05: Comprehensive Test Matrix
**File:** `section-05-test-matrix.md` | **Status:** Complete

```
test matrix, combinatorial, regression guard, AOT test
element type x iteration pattern x loop variant
[str], [[int]], [Option<str>], [(int)->int], [{name:str}], {str:int}, Set<str>
for-do, for-yield, break, continue, guard, nested, two-call, unwind
iter_rc_matrix.rs, tests/spec/iterators/rc_matrix/
fat_ptr_iter.rs, debug, release, behavioral equivalence
ORI_CHECK_LEAKS, Valgrind, dual-exec-verify
```

---

### Section 06: Verification & Merge Gate
**File:** `section-06-verification.md` | **Status:** Complete

```
test-all.sh, clippy-all.sh, valgrind-aot.sh, dual-exec-verify.sh
release build, debug build, RC balance, code journey
merge gate, regression, behavioral equivalence
ORI_TRACE_RC, ORI_CHECK_LEAKS, ORI_AUDIT_CODEGEN
rc-stats.sh, codegen-audit.sh, release-lto
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Root Cause Analysis & Design | `section-01-root-cause.md` |
| 02 | Fix Iterator elem_dec_fn (List, Map, Set) | `section-02-elem-dec-fn.md` |
| 03 | Fix For-Yield RC Scoping | `section-03-for-yield-rc.md` |
| 04 | For-Do / For-Yield Parity Audit | `section-04-parity-audit.md` |
| 05 | Comprehensive Test Matrix | `section-05-test-matrix.md` |
| 06 | Verification & Merge Gate | `section-06-verification.md` |

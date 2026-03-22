---
reroute: true
name: "RC Elem Dec"
full_name: "RC Header elem_dec_fn — Proper Element Cleanup for Fat Pointer Collections"
status: active
order: 1
---

# RC Header elem_dec_fn Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

### Section 01: RC Header Extension
**File:** `section-01-rc-header.md` | **Status:** Complete

```
RC_HEADER_SIZE, ori_rc_alloc, ori_rc_free, ori_rc_dec, ori_rc_realloc
header layout, data_size, strong_count, elem_dec_fn, elem_count, drop_fn
compiler/ori_rt/src/rc/mod.rs, compiler/ori_rt/src/rc/allocate.rs
compiler/ori_rt/src/rc/elem_header.rs, compiler/ori_rt/src/rc/list_rc.rs
ori_buffer_rc_dec, ori_buffer_drop_unique, slice_buffer_rc_dec
store_elem_dec_fn, load_elem_dec_fn, store_elem_count, load_elem_count
element cleanup, RC header V5, 16 bytes → 32 bytes, ABI change, alignment
```

---

### Section 02: Codegen & Runtime Integration
**File:** `section-02-integration.md` | **Status:** Complete

```
emit_list_iter (already passes real elem_dec_fn), emit_buffer_rc_dec_list_or_set, emit_buffer_drop_unique
emit_map_iter (already passes real key/val dec fns), ori_map_buffer_rc_dec, key_dec_fn, val_dec_fn
ori_iter_from_list, ori_iter_from_map, IterState::List, IterState::Map, elem_dec_fn parameter
ori_buffer_store_elem_dec (created), ori_buffer_store_elem_count (created)
elem_count propagation, SSO correctness, COW slow path propagation
write_array_to_list, write_array_to_list_from_data, ori_list_push_new (JIT-only, resolved)
ori_map_keys_to_list, ori_map_values_to_list, ori_set_to_list, ori_str_split
alloc_set_hash_buffer, rehash_set, set COW propagation
emit_iter_collect (builtins/iterator_consumers.rs), emit_iter_collect_set
ori_args_from_argv (lib.rs, [str] buffer creation), ori_list_ensure_capacity (JIT-only)
ori_list_new (JIT-only), ori_list_push (codegen-called), ABI sync points
map double-free investigation, map_buffer_cleanup
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs
compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs
compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs
compiler/ori_llvm/src/codegen/arc_emitter/construction.rs
compiler/ori_rt/src/rc/elem_header.rs (new — extracted element header helpers)
compiler/ori_rt/src/iterator/sources.rs, compiler/ori_rt/src/iterator/state.rs
compiler/ori_rt/src/list/cow.rs, cow_structural.rs, cow_sort.rs (list COW slow paths)
compiler/ori_rt/src/list/query.rs, slice.rs, mod.rs (non-COW buffer creation)
compiler/ori_rt/src/set/cow/basic.rs, cow/algebra.rs, mod.rs (set COW slow paths)
compiler/ori_rt/src/map/mod.rs (map-to-list functions, codegen-based key/val dec)
compiler/ori_rt/src/string/ops.rs (ori_str_split, direct alloc, needs elem_dec_fn param)
compiler/ori_rt/src/iterator/consumers.rs (ori_iter_collect, ori_iter_collect_set)
```

---

### Section 03: Remove Workarounds & Simplify
**File:** `section-03-remove-workarounds.md` | **Status:** Complete

```
__for_coll_N, phantom binding, dummy reference, ordering hack
for_coll_counter, propagate_borrowed_closure, for_yield.rs
coll_param, coll_var, exit_coll_param, ForYieldContext, prepare_iterator
lower_for, lower_for_iterator, lower_for_yield_iterator, exit block ordering
lower_break coll_param, lower_continue coll_param
compiler/ori_arc/src/lower/control_flow/loops.rs
compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs
compiler/ori_arc/src/lower/control_flow/for_yield.rs
compiler/ori_arc/src/lower/control_flow/for_yield_option.rs (coll_param: None in ForYieldContext)
compiler/ori_arc/src/lower/control_flow/mod.rs (lower_break, lower_continue coll_param)
compiler/ori_arc/src/lower/expr/mod.rs (for_coll_counter field, ForYieldContext::coll_param)
compiler/ori_arc/src/lower/mod.rs (for_coll_counter initialization)
compiler/ori_arc/src/lower/calls/lambda.rs (for_coll_counter initialization)
compiler/ori_arc/src/aims/emit_rc/borrowed_defs.rs (propagate_borrowed_closure)
compiler/ori_arc/src/aims/realize/walk_dec.rs (__for_coll reference)
simplify, remove workaround, clean up
remove dead elem_dec_fn parameter, ori_iter_from_list, IterState::List
compiler/ori_rt/src/iterator/sources.rs, compiler/ori_rt/src/iterator/state.rs
compiler/ori_rt/src/iterator/tests.rs (30+ ori_iter_from_list calls, 5 args -> 4 args)
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs (emit_list_iter)
compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
compiler/ori_llvm/src/evaluator/runtime_mappings.rs
```

---

### Section 04: Combinatorial Test Matrix
**File:** `section-04-test-matrix.md` | **Status:** Not Started

```
fat_ptr_iter, combinatorial, cross-product, test matrix
[str], [[int]], [Option<str>], [{name: str}], Set<str>, closure capture
SSO/heap mixed strings, T1b
for-do, for-yield, for-break, for-continue, for-guard, slice iteration
function parameter, nested loop, multi-call, COW interaction
cow_push_str, collect_str, set_cow_insert, map_keys_str, str_split
COW mutation, collection conversion, write_array_to_list
valgrind, ORI_CHECK_LEAKS, dual-exec, behavioral equivalence, release build
compiler/ori_llvm/tests/aot/fat_ptr_iter.rs, tests/valgrind/fat_ptr_iter/
```

---

### Section 05: Verification & Cleanup
**File:** `section-05-verification.md` | **Status:** Not Started

```
test-all.sh, clippy-all.sh, valgrind, ORI_CHECK_LEAKS
code journey, J15, J16, J17, re-run, score
unignore, remove #[ignore], regression guard
fat-pointer-hardening Section 01.2 complete
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | RC Header Extension | `section-01-rc-header.md` |
| 02 | Codegen & Runtime Integration | `section-02-integration.md` |
| 03 | Remove Workarounds & Simplify | `section-03-remove-workarounds.md` |
| 04 | Combinatorial Test Matrix | `section-04-test-matrix.md` |
| 05 | Verification & Cleanup | `section-05-verification.md` |

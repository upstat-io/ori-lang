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
**File:** `section-01-rc-header.md` | **Status:** In Progress

```
RC_HEADER_SIZE, ori_rc_alloc, ori_rc_free, ori_rc_dec, ori_rc_realloc
header layout, data_size, strong_count, elem_dec_fn, drop_fn
compiler/ori_rt/src/rc/mod.rs, compiler/ori_rt/src/rc/allocate.rs
compiler/ori_rt/src/rc/list_rc.rs, ori_buffer_rc_dec, ori_buffer_drop_unique
slice_buffer_rc_dec, element cleanup, RC header V4
16 bytes → 24 bytes, ABI change, alignment
```

---

### Section 02: Codegen & Runtime Integration
**File:** `section-02-integration.md` | **Status:** Not Started

```
emit_list_iter, emit_buffer_rc_dec_list_or_set, emit_buffer_drop_unique
emit_map_iter, ori_map_buffer_rc_dec, key_dec_fn, val_dec_fn
ori_iter_from_list, ori_iter_from_map, IterState::List, IterState::Map, elem_dec_fn parameter
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/map_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs
compiler/ori_llvm/src/codegen/arc_emitter/element_fn_gen.rs
compiler/ori_llvm/src/codegen/arc_emitter/construction.rs
compiler/ori_rt/src/iterator/sources.rs, compiler/ori_rt/src/iterator/state.rs
ori_buffer_store_elem_dec (to be created)
```

---

### Section 03: Remove Workarounds & Simplify
**File:** `section-03-remove-workarounds.md` | **Status:** Not Started

```
__for_coll_N, phantom binding, dummy reference, ordering hack
lower_for, lower_for_iterator, exit block ordering
compiler/ori_arc/src/lower/control_flow/loops.rs
compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs
simplify, remove workaround, clean up
remove dead elem_dec_fn parameter, ori_iter_from_list, IterState::List
compiler/ori_rt/src/iterator/sources.rs, compiler/ori_rt/src/iterator/state.rs
compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
```

---

### Section 04: Combinatorial Test Matrix
**File:** `section-04-test-matrix.md` | **Status:** Not Started

```
fat_ptr_iter, combinatorial, cross-product, test matrix
[str], [[int]], [Option<str>], [{name: str}], Set<str>, closure capture
for-do, for-yield, for-break, for-continue, for-guard, slice iteration
function parameter, nested loop, multi-call, COW interaction
valgrind, ORI_CHECK_LEAKS, dual-exec, behavioral equivalence, release build
compiler/ori_llvm/tests/aot/fat_ptr_iter.rs, tests/valgrind/
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

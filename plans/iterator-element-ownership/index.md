---
reroute: true
name: "Iter Ownership"
full_name: "Iterator Element Ownership Protocol"
status: queued
order: 2
---

# Iterator Element Ownership Protocol Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Sources: Inc on Yield | `section-01-sources.md` | Not Started |
| 02 | Adapters: Dec on Discard | `section-02-adapters.md` | Not Started |
| 03 | Consumers: Dec After Use | `section-03-consumers.md` | Not Started |
| 04 | Codegen: Parameter Plumbing | `section-04-codegen.md` | Not Started |
| 05 | AIMS: Owned Element Contract | `section-05-aims.md` | Not Started |
| 06 | Verification Matrix | `section-06-verification.md` | Not Started |

## Keyword Clusters

### Section 01 — Sources: Inc on Yield
```
next_list, next_str, next_range, next_map, ori_iter_from_list,
ori_iter_from_str, ori_iter_from_map, ori_iter_from_option,
elem_inc_fn, IterState, sources.rs, next.rs, RC increment,
borrowed element, owned element, yield boundary
```

### Section 02 — Adapters: Dec on Discard
```
next_filtered, next_skip, filter, skip, cycle, rev, flatten,
elem_dec_fn, discard, rejected element, scratch buffer,
adapters.rs, next.rs, predicate_fn, RC decrement
```

### Section 03 — Consumers: Dec After Use
```
ori_iter_collect, ori_iter_join, ori_iter_count, ori_iter_any,
ori_iter_all, ori_iter_find, ori_iter_for_each, ori_iter_fold,
ori_iter_last, ori_iter_rfind, ori_iter_rfold, ori_iter_collect_set,
consumers.rs, elem_dec_fn, cleanup, transfer semantics
```

### Section 04 — Codegen: Parameter Plumbing
```
emit_iter_collect, emit_iter_join, emit_iter_count, emit_iter_any,
emit_iter_all, emit_iter_find, emit_iter_for_each, emit_iter_fold,
emit_iter_last, emit_iter_rfind, emit_iter_rfold,
get_or_generate_elem_dec_fn, get_or_generate_elem_inc_fn,
iterator_consumers.rs, iterator.rs, element_fn_gen.rs,
runtime_fn, emit_rt_call, ArcIrEmitter
```

### Section 05 — AIMS: Owned Element Contract
```
collect_iter_element_defs, borrowed_defs.rs, walk_dec.rs,
iter_element_defs, __iter_next, IterNext protocol builtin,
instr_dispatch.rs, AimsState, RcDec suppression, ownership
```

### Section 06 — Verification Matrix
```
ORI_CHECK_LEAKS, assert_aot_success, valgrind, test matrix,
consumer x adapter, type coverage, str, [int], closures,
semantic pin, negative pin, regression test
```

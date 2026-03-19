---
reroute: true
name: "Journey Codegen Polish"
full_name: "Journey Codegen Polish: All 17 Journeys to 10.0/10 Quality"
status: resolved
order: 1
---

# Journey Codegen Polish Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Nounwind Propagation
**File:** `section-01-nounwind.md` | **Status:** Complete

```
nounwind, unwind, exception handling, landing pad, personality
noreturn, panic, cold, main wrapper, entry point, abort
is_arc_function_nounwind, compute_nounwind_set, fixed-point
apply_posthoc_nounwind, post-hoc, invoke vs call
function_compiler/nounwind.rs, entry_point.rs, define_phase.rs
runtime_decl/runtime_functions.rs, ori_panic_cstr, cold-block exclusion
J15, J16, J17, attributes, EH tables, impl methods
```

---

### Section 02: Dead Aggregate Load Elimination
**File:** `section-02-dead-loads.md` | **Status:** Complete

```
aggregate load, param.load, borrowed parameter, dead code
load_struct_selective, emit_function, forwarded pointer
borrowed_param_ptrs, def_var_repr, scan_used_fields, field_scan
ir_builder/memory.rs, arc_emitter/emit_function.rs, field_scan/mod.rs
J16, J17, instruction purity, unjustified instructions
```

---

### Section 03: Sret Identity Copy Elimination
**File:** `section-03-sret-identity.md` | **Status:** Complete

```
sret, return, identity copy, load+store, no-op
emit_terminator, call_with_sret, sret pointer, sret forwarding
current_sret_ptr, sret_forwarded, ret void
arc_emitter/terminators.rs, arc_emitter/apply_helpers.rs, ir_builder/calls.rs
J16, make_string, ori_str_from_raw
```

---

### Section 04: Iterator Option Wrapping
**File:** `section-04-iterator-wrapping.md` | **Status:** Complete

```
iterator, option, wrapping, alloca, round-trip, scratch buffer
emit_iter_next, for loop, for-yield, for-guard, has_next, tag
EmittedValue, Project, build_struct, ori_iter_next, decomposed
borrowed_param_ptrs, str_to_ptr_forwarded, pointer forwarding
side-channel map, iter_next_decomposed, Approach A/B/C
arc_emitter/builtins/iterator.rs, lower/control_flow/for_loops/for_iterator.rs
lower/control_flow/for_yield.rs, instr_dispatch.rs, apply_helpers.rs
J15, count_chars, insertvalue, extractvalue
```

---

### Section 05: Range Unused Field Extraction
**File:** `section-05-range-fields.md` | **Status:** Complete

```
range, inclusive, field extraction, unused field, extractvalue
for_range.rs, emit_project, step, bounds check
get_literal_int, get_field_literal_int, get_construct_arg
lower/control_flow/for_loops/for_range.rs, lower/builder/mod.rs
J07, sum_for, proj.3
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Complete

```
code journey, re-run, score, 10.0, verification
test-all, clippy, regression, overview.md
J07, J15, J16, J17, PASS
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Nounwind Propagation | `section-01-nounwind.md` |
| 02 | Dead Aggregate Load Elimination | `section-02-dead-loads.md` |
| 03 | Sret Identity Copy Elimination | `section-03-sret-identity.md` |
| 04 | Iterator Option Wrapping | `section-04-iterator-wrapping.md` |
| 05 | Range Unused Field Extraction | `section-05-range-fields.md` |
| 06 | Verification | `section-06-verification.md` |

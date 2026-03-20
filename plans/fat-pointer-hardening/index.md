---
reroute: true
name: "Fat Ptr"
full_name: "Fat Pointer Hardening: All 17 Journeys to 10/10"
status: resolved
order: 1
---

# Fat Pointer Hardening Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

### Section 01: Iterator–Collection Ownership Contract
**File:** `section-01-iterator-ownership.md` | **Status:** Not Started

```
double-free, [str], [T] where T: Drop, ori_iter_drop, ori_buffer_rc_dec
_ori_elem_dec, element cleanup, iterator ownership, collection destructor
ori_rt/src/iterator/state.rs, ori_rt/src/rc/list_rc.rs, ori_rt/src/list/
ori_arc/src/lower/control_flow/for_loops/, ori_arc/src/aims/emit_rc/
IterState, ori_buffer_drop_unique, ori_list_rc_inc, elem_dec_fn
nested ARC, element-level RC, fat pointer in collection
landingpad, unwind path, double ori_buffer_rc_dec, EH cleanup
J15 double-free, C15-1, C15-2
for yield, break continue, partial consumption, COW interaction
map iteration, string iteration, IterState::Map, IterState::Str
```

---

### Section 02: Monomorphization of Captured Types
**File:** `section-02-monomorphization.md` | **Status:** Not Started

```
closure, capture, str, fat pointer, unresolved type variable, Idx leak
monomorphization, mono instance, type propagation, lambda parameter
ori_types/infer/expr/calls/monomorphization.rs
ori_llvm/src/monomorphize/mod.rs, ori_llvm/codegen/arc_emitter/closures.rs
ori_arc/src/lower/calls/lambda.rs, type_info/store.rs
codegen crash, LLVM IR verification, Call parameter type mismatch
ori_rc_dec i64 vs ptr, _ori_drop$N, extractvalue on wrong type
_ori_partial_N thunk, FunctionAbi, partial_apply
J17 C17, closure capturing non-scalar
```

---

### Section 03: Aggregate Value Emission
**File:** `section-03-aggregate-emission.md` | **Status:** Not Started

```
field-by-field copy, aggregate load, aggregate store, fat pointer copy
GEP+load+insertvalue, 10-instruction sequence, 2-instruction ideal
str passing, {i64, i64, ptr}, 24-byte aggregate, value_emission.rs
ori_llvm/codegen/arc_emitter/value_emission.rs, apply_helpers.rs
ori_llvm/codegen/arc_emitter/rc_buffer_ops.rs, cfg_simplify/mod.rs
ori_llvm/codegen/arc_emitter/dead_unwind.rs, terminators.rs
instruction bloat, 3-6x overhead, materialization
sret, return ABI, ParamPassing::Indirect, FatPointer
pointer forwarding, sret forwarding, JIT vs AOT gate
J16 HIGH-1, J16 LOW-2, J14 LOW-1, J14 LOW-2
nounwind, invoke vs call, dead landing pad, single-predecessor merge
nounwind_functions, ctx.nounwind_functions, HashMap to FxHashMap
```

---

### Section 04: Combinatorial Test Matrix
**File:** `section-04-test-matrix.md` | **Status:** Not Started

```
test matrix, combinatorial, fat pointer x feature, regression guard
[str], [T: Drop], closure capture, generics, pattern matching
Option<str>, Result<str, E>, struct with str field, tuple with str
Map<str, int>, Set<str>, (str, int) tuple
loops, recursion, derived traits, break/continue, higher-order
derived Clone, ? propagation, multiple values, loop accumulation
AOT test, spec test, valgrind, dual-exec, behavioral equivalence
ori_llvm/tests/aot/fat_matrix/, tests/spec/, tests/valgrind/fat_matrix/
type category x language feature cross-product
18 type categories (T1-T18), 20 feature dimensions (F1-F20)
```

---

### Section 05: Verification
**File:** `section-05-verification.md` | **Status:** Not Started

```
code journey, re-run, 10/10, score validation, overview
test-all.sh, clippy-all.sh, fmt-all.sh, valgrind-aot.sh
dual-exec-verify.sh, ORI_CHECK_LEAKS, extract-metrics.py
plans/code-journeys/, overview.md, *-results.md
merge gate, regression, behavioral equivalence
release build, debug build, C15-1, C15-2, C17, bug status
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Iterator–Collection Ownership Contract | `section-01-iterator-ownership.md` |
| 02 | Monomorphization of Captured Types | `section-02-monomorphization.md` |
| 03 | Aggregate Value Emission | `section-03-aggregate-emission.md` |
| 04 | Combinatorial Test Matrix | `section-04-test-matrix.md` |
| 05 | Verification | `section-05-verification.md` |

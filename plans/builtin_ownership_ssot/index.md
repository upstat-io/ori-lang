# Builtin Ownership Single Source of Truth — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Related:** `plans/aot_codegen_pipeline/section-05-builtin-architecture.md`

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 00: Overview
**File:** `section-00-overview.md` | **Status:** Not Started

```
ownership, receiver_borrows, single source of truth, SSoT
fragmented registries, 4-way drift, borrow inference bug
s.len() leak, str.len missing from borrow inference
ori_ir MethodDef, ori_arc borrow inference, ori_llvm BuiltinRegistration
receiver_borrowed, borrowing_builtin_names, FxHashSet<Name>
crate DAG, dependency inversion, bottom-of-DAG metadata
```

---

### Section 01: Extend MethodDef with Ownership
**File:** `section-01-methoddef-ownership.md` | **Status:** Complete

```
MethodDef, receiver_borrows, type_flow, TypeFlow, struct field, const fn
comparable(), eq_trait(), clone_trait(), hash_trait()
to_str_trait(), debug_trait(), standard(), convenience constructors
borrowing_method_names(), method_borrows_receiver()
query functions, compile-time enforcement, no default
BUILTIN_METHODS, 162 entries, all true, TypeFlow::Standard
ClosureOutputBecomesElement, ClosureOutputFlatElement, Accumulator
```

---

### Section 02: Expand IR Registry to All 20 Types
**File:** `section-02-ir-registry-expansion.md` | **Status:** Not Started

```
COLLECTION_TYPES gap, 11 missing types, 398 TYPECK entries
list, map, Set, Option, Result, Iterator, DoubleEndedIterator
range, tuple, error, Channel, ParamSpec, ReturnSpec
file split, primitives.rs, special_types.rs, collections.rs
wrappers.rs, 500 line limit, submodule arrays
TYPECK_METHODS_NOT_IN_IR, EVAL_METHODS_NOT_IN_IR
consistency.rs gap lists, COLLECTION_TYPES elimination
```

---

### Section 03: Wire ori_arc to ori_ir
**File:** `section-03-arc-wiring.md` | **Status:** Not Started

```
builtin_borrowing_names(), ori_arc::lib.rs, StringInterner
infer_borrows, borrowing_builtins, FxHashSet<Name>
compile_common.rs, evaluator.rs, function_compiler/mod.rs
call site replacement, 4 call sites, source of truth change
Iterator exclusion, .iter() exclusion, hidden dependencies
TypeFlow ignored by ARC, receiver_borrows only
```

---

### Section 03B: Wire ori_types to Read TypeFlow from Registry
**File:** `section-03B-typeck-wiring.md` | **Status:** Not Started

```
TypeFlow, type_flow, unify_higher_order_constraints
ClosureOutputBecomesElement, ClosureOutputFlatElement, Accumulator
tag_to_builtin_type, ReceiverDispatch, registry lookup
calls.rs hard-coded match replacement, find_method
Iterator.map, Iterator.flat_map, Iterator.fold, Iterator.rfold
closure return type, element type, accumulator type
```

---

### Section 04: Remove Ownership from ori_llvm
**File:** `section-04-llvm-cleanup.md` | **Status:** Not Started

```
BuiltinRegistration, receiver_borrowed field removal
declare_builtins! macro simplification, borrow: syntax removal
borrowing_builtin_names() deletion, 7 submodule files
primitives.rs, collections.rs, traits.rs, compound_traits.rs
iterator.rs, option_result.rs, trampolines.rs
179 entries, macro syntax update
```

---

### Section 05: Enforcement Tests
**File:** `section-05-enforcement.md` | **Status:** Not Started

```
every_codegen_builtin_has_ir_method_def, structural enforcement
BuiltinType::from_name, find_method, compile-time guarantee
no_phantom_builtin_entries update, builtin_coverage_above_threshold
consistency.rs updates, COLLECTION_TYPES reduction
TYPECK_METHODS_NOT_IN_IR reduction, ir_registry_covers_all_types
higher_order_methods_have_type_flow, TypeFlow enforcement
```

---

### Section 06: Legacy Removal & Verification
**File:** `section-06-legacy-removal.md` | **Status:** Not Started

```
grep verification, dead code, clippy warnings
borrowing_builtin_names grep, receiver_borrowed grep
borrow: syntax grep, fmt-all.sh, clippy-all.sh
test-all.sh, llvm-test.sh, full suite verification
exit criteria, zero traces, structural guarantee
hard-coded method names grep, unify_higher_order_constraints
```

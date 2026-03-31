# Full Project Implementation Hygiene Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Registry as Universal SSOT (Operator Dispatch)
**File:** `section-01-registry-operator-dispatch.md` | **Status:** Not Started

```
OpDefs, OpStrategy, BinaryOp, UnaryOp, operator dispatch, infer_binary
infer_unary, emit_binary_op, operators.rs, operator traits
ori_registry, ori_types, ori_eval, ori_llvm, scattered knowledge
Tag::Int, Tag::Float, Tag::Bool, Tag::Duration, Tag::Size
bitwise, arithmetic, comparison, negation, BitNot, Not
```

---

### Section 02: Registry as Universal SSOT (Methods & Traits)
**File:** `section-02-registry-methods-traits.md` | **Status:** Not Started

```
primitive_satisfies_trait, type_satisfies_trait, trait satisfaction
INT_TRAITS, FLOAT_TRAITS, BOOL_TRAITS, STR_TRAITS, hardcoded arrays
infer_ident, builtin identifiers, hash_combine, repeat
WellKnownNames, named type methods, DEI propagation
ori_types, ori_registry, SSOT, scattered knowledge
```

---

### Section 03: Cross-Backend Algorithmic DRY (eval / LLVM)
**File:** `section-03-cross-backend-dry.md` | **Status:** Not Started

```
eval, LLVM, parallel dispatch, algorithmic duplication
emit_option_method, emit_result_method, eval_option_binary, eval_result_binary
emit_equals, emit_compare, emit_hash, derived methods
Option routing, Result routing, Ordering predicates
derive processing, exhaustiveness guards, iterator method lists
```

---

### Section 04: Named Constants for Tag Values & Field Indices
**File:** `section-04-named-constants.md` | **Status:** Not Started

```
Some tag 0, None tag 1, Ok tag 0, Err tag 1, magic numbers
Option discriminant, Result discriminant, tag constants
FNV_OFFSET_BASIS, FNV_PRIME, hash constants
collection field indices, len index 0, cap index 1, data index 2
FatPointer, Closure, Range, struct sizes
```

---

### Section 05: Layout Computation Unification
**File:** `section-05-layout-unification.md` | **Status:** Not Started

```
enum_tag_bytes, type layout, layout computation
ori_repr, ori_arc, ori_llvm, layout duplication
min_tag_width, variant_count, struct_layout
MachineRepr, ReprPlan, TypeInfo
```

---

### Section 06: LLVM Internal Algorithmic DRY
**File:** `section-06-llvm-internal-dry.md` | **Status:** Not Started

```
emit_inline_enum_inc, emit_inline_enum_dec, enum RC
slice_aware_rc_inc, list trait loop scaffold
emit_option_equals, emit_option_compare, emit_option_hash
wrapper_cmp.rs, option_result.rs, compound_type_impls
pre-interned names, rc_helpers.rs, rc_ops.rs
```

---

### Section 07: Runtime RC Protocol DRY + Correctness
**File:** `section-07-runtime-rc-protocol.md` | **Status:** Not Started

```
ori_rc_dec, ori_buffer_rc_dec, ori_str_rc_dec
RC protocol, immortal sentinel, MAX_REFCOUNT
fetch_sub, Ordering::Release, Ordering::Acquire
rc_underflow_abort, call_drop_fn, ori_rt
```

---

### Section 08: Cross-Phase Invariant Contracts
**File:** `section-08-invariant-contracts.md` | **Status:** Not Started

```
debug_assert, type variable resolution, Tag::Var
RC balance, ARC pipeline, error node filtering
TypeId::FIRST_COMPOUND, Idx::FIRST_DYNAMIC, sync
ABI FIXME, codegen preconditions, phase contracts
```

---

### Section 09: Registration Sync & Enforcement
**File:** `section-09-registration-sync.md` | **Status:** Not Started

```
iterator methods, sync points, coverage threshold
builtin_coverage_above_threshold, min_pct 25
operator trait name, eq vs equals
eval operator dispatch, parallel mapping
ori_registry, ori_types, ori_eval, ori_llvm
```

---

### Section 10: Scattered Knowledge Cleanup
**File:** `section-10-scattered-knowledge.md` | **Status:** Not Started

```
TypeInfo::is_trivial, ReprPlan::is_trivial, triviality
is_primitive_value, semantic mismatch
BuiltinType::is_comparable, hardcoded
TypeId::name, BuiltinType::name, duplication
is_builtin_indexable, suggestion fields
ReprAttrKind, ReprAttribute, duplication
lex, lex_full, swallowed errors, warnings dropped
```

---

### Section 11: Stale Plan Annotations
**File:** `section-11-stale-annotations.md` | **Status:** Not Started

```
TPR-, CROSS-04, plan annotations, stale references
ori_arc, ori_llvm, ori_types, oric
section references, phase annotations, cleanup
```

---

### Section 12: Surface Hygiene
**File:** `section-12-surface-hygiene.md` | **Status:** Not Started

```
file size, 500 line limit, unsafe SAFETY comment
module docs, dead code, #[allow]
match arms, data-driven dispatch
Pool var_ids, FunctionSig, string allocation
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Registry as Universal SSOT (Operator Dispatch) | `section-01-registry-operator-dispatch.md` |
| 02 | Registry as Universal SSOT (Methods & Traits) | `section-02-registry-methods-traits.md` |
| 03 | Cross-Backend Algorithmic DRY (eval / LLVM) | `section-03-cross-backend-dry.md` |
| 04 | Named Constants for Tag Values & Field Indices | `section-04-named-constants.md` |
| 05 | Layout Computation Unification | `section-05-layout-unification.md` |
| 06 | LLVM Internal Algorithmic DRY | `section-06-llvm-internal-dry.md` |
| 07 | Runtime RC Protocol DRY + Correctness | `section-07-runtime-rc-protocol.md` |
| 08 | Cross-Phase Invariant Contracts | `section-08-invariant-contracts.md` |
| 09 | Registration Sync & Enforcement | `section-09-registration-sync.md` |
| 10 | Scattered Knowledge Cleanup | `section-10-scattered-knowledge.md` |
| 11 | Stale Plan Annotations | `section-11-stale-annotations.md` |
| 12 | Surface Hygiene | `section-12-surface-hygiene.md` |

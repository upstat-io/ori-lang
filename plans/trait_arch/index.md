# Trait Architecture Refactor Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

> **Motivation:** Analysis of the last 40 commits (Feb 17-18 2026) revealed that 35% of all work is reactive — fixing sync drift, filling codegen gaps, cleaning hygiene. The root cause is structural: the codebase extends along traits but couples along crates, requiring 12+ file touches per new trait with no compile-time enforcement of completeness. This plan eliminates that friction.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Trait Metadata Registry
**File:** `section-01-trait-metadata.md` | **Status:** Complete

```
DerivedTrait, define_derived_traits!, macro, registry
ALL, COUNT, trait_name, method_name, from_name
metadata, param_count, return_type_tag, takes_other
requires_supertrait, supports_sum_types, constraints
ori_ir, derives, single source of truth, canonical
```

---

### Section 02: Data-Driven Trait Satisfaction
**File:** `section-02-trait-satisfaction.md` | **Status:** Not Started

```
primitive_satisfies_trait, type_satisfies_trait, well_known
bitset, TraitSet, trait set, satisfaction matrix
N×M matrix, per-type per-trait, boolean chain
WellKnownNames, pre-interned, O(1) lookup
primitive types, compound types, container types
Tag, Idx, Pool, trait checking
```

---

### Section 03: Registration Module Split
**File:** `section-03-registration-split.md` | **Status:** Not Started

```
registration, mod.rs, 1761 lines, file split, submodule
check/registration, build_derived_methods, register_traits
operator traits, core traits, format traits
derived, builtin_impls, validation
god file, accretion, 500-line limit
```

---

### Section 04: LLVM Codegen Consolidation
**File:** `section-04-llvm-refactor.md` | **Status:** Not Started

```
lower_builtin_methods, 1497 lines, LLVM codegen, split
derive_codegen, scaffolding, factory, boilerplate
field_ops, emit_field_eq, emit_field_compare, coerce_to_i64
compile_derive, function creation, ABI, symbol
type group, numeric, text, containers, collections
```

---

### Section 05: Cross-Crate Sync Enforcement
**File:** `section-05-sync-enforcement.md` | **Status:** Complete

```
sync, drift, completeness test, cross-crate
DerivedTrait::ALL, iteration, coverage, enforcement
ori_ir, ori_types, ori_eval, ori_llvm, prelude
compile-time, test-time, exhaustive, validation
consistency test, method registry, trait registry
```

---

### Section 06: Error Code Generation
**File:** `section-06-error-codegen.md` | **Status:** Not Started

```
error code, E20XX, define_error_codes!, macro
error_code/mod.rs, from_u16, to_u16, ErrorCode
markdown, error docs, E20XX.md, template
oric, reporting, typeck, rendering
diagnostic, ori_diagnostic, boilerplate
```

---

### Section 07: Shared Derive Strategy
**File:** `section-07-derive-strategy.md` | **Status:** Not Started

```
derive strategy, DeriveStep, CombineOp
dual backend, eval, LLVM, duplication
ForEachField, AllEqual, LexicographicCmp, HashCombine
field iteration, field operation, field dispatch
eval_derived, compile_derive, shared logic
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | Trait Metadata Registry | `section-01-trait-metadata.md` | 1 |
| 02 | Data-Driven Trait Satisfaction | `section-02-trait-satisfaction.md` | 1 |
| 03 | Registration Module Split | `section-03-registration-split.md` | 1 |
| 04 | LLVM Codegen Consolidation | `section-04-llvm-refactor.md` | 2 |
| 05 | Cross-Crate Sync Enforcement | `section-05-sync-enforcement.md` | 1 |
| 06 | Error Code Generation | `section-06-error-codegen.md` | 3 |
| 07 | Shared Derive Strategy | `section-07-derive-strategy.md` | 3 |

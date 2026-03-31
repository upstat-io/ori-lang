---
section: "02"
title: "Registry as Universal SSOT (Methods & Traits)"
status: not-started
reviewed: false
goal: "Type checker queries ori_registry for trait satisfaction and builtin method signatures instead of maintaining parallel hardcoded arrays"
inspired_by:
  - "ori_registry defs/*.rs TypeDef pattern -- methods and operator defs per type"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Trait Satisfaction via Registry"
    status: not-started
  - id: "02.2"
    title: "Builtin Identifier Signatures"
    status: not-started
  - id: "02.3"
    title: "Named Type Method Dispatch"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Registry as Universal SSOT (Methods & Traits)

**Status:** Not Started
**Goal:** The type checker queries `ori_registry` for builtin trait satisfaction and method signatures instead of maintaining parallel hardcoded string arrays and inline type constructions. After this section, adding a trait impl for a builtin type requires modifying only the registry.

**Context:** The type checker maintains independent parallel data structures for trait satisfaction (`primitive_satisfies_trait` in `compiler/ori_types/src/infer/expr/calls/traits.rs`) and builtin identifier signatures (`infer_ident` in `compiler/ori_types/src/infer/expr/identifiers.rs`). These hardcoded arrays and inline constructions duplicate knowledge that the registry already holds, creating drift risk.

**Reference implementations:**
- **ori_registry** `compiler/ori_registry/src/defs/int.rs`: `INT` TypeDef with methods and operators
- **ori_registry** `compiler/ori_registry/src/lib.rs`: `find_type()`, `find_method()`, `has_method()` query API

**Depends on:** None (but benefits from Section 01 patterns).

**Feasibility note:** The registry's `OpDefs` can derive operator-trait satisfaction (e.g., `add != Unsupported` implies `Add` trait). However, non-operator traits like `Clone`, `Default`, `Printable`, `Debug`, `Len`, `IsEmpty`, `Iterable` are NOT directly represented in `OpDefs` -- they come from the `methods` array and trait_name fields in `MethodDef`. The bridge function must handle both categories: operator traits (from `OpDefs`) and method traits (from `MethodDef.trait_name`).

**Test strategy:** Pure refactoring -- no behavioral changes. The test matrix is the existing test suite plus:
- Semantic pin: Rust unit test that queries registry for each type's trait satisfaction and asserts equivalence with the old hardcoded arrays
- Regression: existing `#compile_fail` tests for trait bound violations (e.g., non-Hashable map keys) must pass unchanged

---

## 02.1 Trait Satisfaction via Registry

**File(s):** `compiler/ori_types/src/infer/expr/calls/traits.rs`

The `primitive_satisfies_trait()` function (lines 14-176) maintains 10 parallel `const` string arrays (`INT_TRAITS`, `FLOAT_TRAITS`, `BOOL_TRAITS`, `STR_TRAITS`, `CHAR_TRAITS`, `BYTE_TRAITS`, `UNIT_TRAITS`, `DURATION_TRAITS`, `SIZE_TRAITS`, `ORDERING_TRAITS`) listing which traits each primitive type satisfies. The `type_satisfies_trait()` function (lines 183-218) adds more hardcoded arrays for compound types (`COLLECTION_TRAITS`, `WRAPPER_TRAITS`, `RESULT_TRAITS`).

The registry's `TypeDef` already includes `operators: OpDefs` which encodes trait support (e.g., `operators.add != Unsupported` implies `Add` trait). The registry's `methods` array includes trait-associated methods. This data could drive trait satisfaction checks.

- [ ] **LEAK:scattered-knowledge** `traits.rs:16-141` -- 10 per-primitive `const` trait arrays (`INT_TRAITS`, `FLOAT_TRAITS`, etc.) duplicate knowledge derivable from registry `TypeDef.operators` and `TypeDef.methods`
- [ ] **LEAK:scattered-knowledge** `traits.rs:184-194` -- 3 compound-type trait arrays (`COLLECTION_TRAITS`, `WRAPPER_TRAITS`, `RESULT_TRAITS`) duplicate knowledge derivable from registry
- [ ] **LEAK:scattered-knowledge** `traits.rs:202-218` -- Per-tag trait satisfaction (`Tag::List`, `Tag::Map`, `Tag::Option`, etc.) hardcoded instead of registry-driven
- [ ] Create a bridge function (e.g., `registry_satisfies_trait(tag: TypeTag, trait_name: &str) -> bool`) that queries the registry and use it as the primary satisfaction check

---

## 02.2 Builtin Identifier Signatures

**File(s):** `compiler/ori_types/src/infer/expr/identifiers.rs`

The `infer_ident()` function constructs type signatures for builtin identifiers inline (e.g., `hash_combine` at line 74, `repeat` at line 80). These are free functions registered in the prelude whose signatures should be derivable from a canonical source rather than hardcoded in the type checker.

- [ ] **LEAK:scattered-knowledge** `identifiers.rs:74-80` -- Builtin identifier signatures (`hash_combine`, `repeat`) hardcoded in type checker instead of derived from a canonical prelude definition

---

## 02.3 Named Type Method Dispatch

**File(s):** `compiler/ori_types/src/infer/expr/methods/`

The type checker's method resolution for named types maintains its own dispatch logic for builtin methods. While the registry is queried for method existence (`has_method`), the type checker constructs return types and validates parameters independently rather than using the registry's `MethodDef.returns` and `MethodDef.params`.

- [ ] **LEAK:scattered-knowledge** -- Named type method return types and parameter validation duplicated between type checker inline construction and registry `MethodDef` definitions
- [ ] **LEAK:scattered-knowledge** -- DEI (DoubleEndedIterator) method propagation logic duplicated outside the registry's `TypeTag::base_type()` resolution

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `primitive_satisfies_trait()` queries registry instead of maintaining 10 parallel `const` arrays
- [ ] `type_satisfies_trait()` queries registry instead of maintaining 3 compound-type arrays
- [ ] Adding a trait impl for a builtin type in the registry is sufficient for the type checker to recognize it
- [ ] Builtin identifier signatures are derived from a canonical source
- [ ] `timeout 150 ./test-all.sh` passes with zero regressions
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** The 13 hardcoded trait arrays in `traits.rs` are replaced by registry queries. `./test-all.sh` green.

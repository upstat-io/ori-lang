---
reroute: true
name: "Empty-Container Typeck"
full_name: "Empty-Container Typeck Phase-Contract Enforcement"
status: active
reviewed: false
order: 1
---

# Empty-Container Typeck Phase-Contract Enforcement Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Supersedes:** `plans/bug-tracker/fix-BUG-04-074.md` (bug entry remains, escalation pointer added)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: AST-based Value Restriction
**File:** `section-01-value-restriction.md` | **Status:** Not Started

```
value restriction, AST-based detection, ExprKind::Lambda, let-polymorphism
should_generalize, generalization policy, monomorphic local binding
infer_block, infer_let, sequences.rs, body_captures_outer
compiler/ori_types/src/infer/expr/blocks.rs, compiler/ori_types/src/infer/expr/mod.rs, compiler/ori_types/src/infer/expr/sequences.rs
Rust rustc_hir_typeck (no let-polymorphism), Haskell monomorphism restriction
capture detection, non-capturing lambda, engine.generalize
```

---

### Section 02: Validator Module (`ori_types::check::validators`)
**File:** `section-02-validator-module.md` | **Status:** Not Started

```
validator module, validate_body_types, check::validators, pub(crate) mod validators
Pool::visit_children reuse, HAS_VAR flag-based walk, resolve_fully at each step
VarState::Generalized exemption (SC-1 divergence), Tag::BoundVar vs Tag::Var
HAS_ERROR cascade suppression, TypeFlags, FunctionSig signature validation
twelve-cell test matrix, positive/negative convention, dedup cell T12
E2005 emission, AmbiguousType, cannot infer type, span_of closure
pool/mod.rs compute_flags scheme-flag propagation fix (TF-3)
compiler/ori_types/src/check/validators/mod.rs, lib.rs narrow re-export
```

---

### Section 03: Bodies-Pass Integration
**File:** `section-03-bodies-pass-integration.md` | **Status:** Not Started

```
bodies-pass integration, 4 call sites, typeck.md CK-1
check_function_bodies, check_function, check_test, check_impl_method, check_def_impl_method
compiler/ori_types/src/check/bodies/mod.rs:39, compiler/ori_types/src/check/bodies/mod.rs:182
ModuleChecker::arena, InferEngine::expr_types, with_function_scope closure
post-body validation, producer-side enforcement, typeck.md PC-2
phase-contract enforcement, cross-phase invariant contract
```

---

### Section 04: Codegen Defense-in-Depth Assertions
**File:** `section-04-codegen-assertions.md` | **Status:** Not Started

```
debug_assert!, defense-in-depth, Cross-Phase Invariant Contract
impl-hygiene.md, consumer-side assertion, release ICE
internal compiler error, panic with ArcFunction name
prepare_mono_cached, process_arc_function, ArcFunction.var_types
compiler/ori_llvm/src/codegen/function_compiler/nounwind/prepare.rs:95
compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs:315
compiler/ori_llvm/src/evaluator/compile.rs:230 (JIT pre-mono hook)
compiler/oric/src/commands/codegen_pipeline.rs:112 (AOT pre-mono hook)
ArcVarId index u32, Vec<Idx> enumerate, collect_mono_functions
MonoInstance generic_args Vec<GenericArg>, FunctionSig, StringInterner
```

---

### Section 05: Test Matrix + Semantic Pins
**File:** `section-05-test-matrix.md` | **Status:** Not Started

```
test matrix, TDD-first, CLAUDE.md §TDD for Bugs
matrix dimensions: container type × element type × usage pattern × constraint availability
empty list, [int], [str], [bool], [struct], [closure]
push, insert, len, is_empty, iter, map, filter, fold, nested let, try block
semantic pin, test_let_polymorphism_for_lambda, regression pin
negative pin, positive/negative pairing, dual-execution parity
compiler/ori_types/src/check/validators/tests.rs, compiler/ori_llvm/tests/aot/
tests/spec/types/collections.ori, diagnostics/dual-exec-verify.sh
test_empty_list_let_binding_emits_e2005, test_empty_list_with_push_and_len_compiles_with_annotation
cargo test -p ori_types, cargo test -p ori_llvm, cargo st
```

---

### Section 06: Diagnostics + Spec-Test Audit
**File:** `section-06-diagnostics-audit.md` | **Status:** Not Started

```
E2005 message, diagnostic wording, suggestion format
add a type annotation like let x: [int] = []
spec 14-expressions.md:1224-1228, empty list compile-time error
test audit, [].iter(), [].len(), [].is_empty(), let $x = [] patterns
tests/spec/types/collections.ori, tests/spec/collections/cow/
double_ended.ori, double_ended_methods.ori (direct-receiver uncontextualized empty-list forms)
spec-compliance audit, annotation sweep, rg tests/ library/
TPR-04-005-codex audit recommendation
```

---

### Section 07: Close-out + Supersession
**File:** `section-07-closeout.md` | **Status:** Not Started

```
plan close-out, supersession, BUG-04-074 resolution
plan annotations removal, ephemeral scaffolding, impl-hygiene.md
/tpr-review final gate, /impl-hygiene-review, /improve-tooling sweep, /sync-claude
plans/bug-tracker/00-overview.md update, bug count decrement
plans/bug-tracker/section-04-codegen-llvm.md entry update
fix-BUG-04-074.md supersession, audit trail preservation
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | AST-based Value Restriction | `section-01-value-restriction.md` |
| 02 | Validator Module (`ori_types::check::validators`) | `section-02-validator-module.md` |
| 03 | Bodies-Pass Integration | `section-03-bodies-pass-integration.md` |
| 04 | Codegen Defense-in-Depth Assertions | `section-04-codegen-assertions.md` |
| 05 | Test Matrix + Semantic Pins | `section-05-test-matrix.md` |
| 06 | Diagnostics + Spec-Test Audit | `section-06-diagnostics-audit.md` |
| 07 | Close-out + Supersession | `section-07-closeout.md` |

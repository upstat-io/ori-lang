# AOT Codegen Pipeline Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Supersedes:** `plans/arc_optimization/`, `plans/arc_codegen_unification/`

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Emission Layer Typing
**File:** `section-01-emission-layer-typing.md` | **Status:** Not Started

```
ValueRepr, EmittedValue, OperandValue, RcStrategy, value representation
scalar, rc pointer, aggregate, fat value, pair, zero sized
HeapPointer, FatPointer, Closure, AggregateFields, InlineEnum
InlineEnum asymmetry, Inc no-op, Dec tag-switch, stack-allocated enum
ValueId::NONE, undefined variable guard, Option None payload
emit_inline_enum_inc (deleted), emit_inline_enum_dec
type info lookup, TypeInfo query, load confusion, store confusion
Pool query elimination, information contract chain
Rust OperandValue pattern, Lean type-indexed phases, Lean isPointer flag
var_map, emit_instr, emit_terminator, emit_apply, emit_rc_op
arc_emitter/mod.rs, ir/mod.rs, ArcClassifier, rc_insert/mod.rs
```

---

### Section 02: ARC Lowerer Gap Closure
**File:** `section-02-lowerer-gaps.md` | **Status:** Not Started

```
CanExpr, lower_function_can, UnsupportedExpr
FunctionExp, FunctionRef, HashLength, FormatWith, Await, WithCapability
Idx::ERROR, lower_try, result_err, TryOperator
err payload type, i64 truncation, pool.result_err()
builtin tag-check lowering, emit_call_or_invoke interception
canonical IR desugars r.is_err() to is_err(r), CanExpr::Ident not MethodCall
Project + PrimOp::Binary(Eq) tag check, builder.var_type()
lower_exp_panic, lower_exp_unreachable, lower_exp_todo
print, println, format, format_with
lower/expr/mod.rs, lower/constructs.rs
```

---

### Section 03: Closure Codegen Completion
**File:** `section-03-closure-codegen.md` | **Status:** Not Started

```
PartialApply, closure, lambda, capture, environment
closure struct, wrapper function, fn_ptr, env_ptr
closure environment drop, ClosureEnv, drop_fn_cache
heap allocation, RC-tracked allocation
arc_emitter/mod.rs, drop_gen.rs
```

---

### Section 04: Borrow Inference Hardening
**File:** `section-04-borrow-hardening.md` | **Status:** Not Started

```
borrow inference, AnnotatedSig, annotated_sigs
lookup miss, silent fallback, all-Owned
tracing::warn, debug_assert
method index, O(1) dispatch, linear scan
lookup_method_by_unqualified_name, method_functions
ArgOwnership, call-site ownership, borrowing vs consuming
external callee, ori_* runtime, is_external_callee
builtin method borrowing, is_err, is_ok, unwrap, is_some, is_none
try_emit_builtin_method, BUILTIN_BORROWING_METHODS
emit_call_or_invoke tag-check interception, canonical IR method desugaring
annotated_sigs synthetic entries, builtin receiver borrow
emit_call_or_invoke, is_nounwind_call, Invoke vs PrimOp inline
lower_call not lower_method_call, builder.var_type() receiver lookup
Project borrowing, is_borrowing_instr, scalar projection
Lean 4 proj borrows x, is_scalar(dst), cross-block liveness
lower_try tag extraction, malloc unaligned tcache chunk, heap corruption
compute_refined_liveness, cross-block Dec placement, per-path Dec at last use
insert_external_invoke_cleanup, is_borrowing_instr
Perceus ownership transfer, caller-side RcDec
function_compiler/mod.rs, arc_emitter/mod.rs, rc_insert/mod.rs
```

---

### Section 05: Builtin Method Architecture
**File:** `section-05-builtin-architecture.md` | **Status:** Not Started

```
builtin method, inline codegen, dispatch table
BuiltinTable, BuiltinEntry, try_emit_builtin_method
TYPECK_BUILTIN_METHODS, consistency, sync test
builtins/mod.rs, builtins/collections.rs, builtins/primitives.rs
builtins/iterator.rs, builtins/traits.rs, builtins/compound_traits.rs
builtins/option_result.rs, builtins/trampolines.rs
```

---

### Section 06: RC Identity Propagation
**File:** `section-06-rc-identity.md` | **Status:** Not Started

```
RC identity, RCIdentityFunctionInfo, root normalization
DerivedOwnership, BorrowedFrom, projection chain
RcInc, RcDec, canonical root
rc_identity.rs, rc_elim, eliminate_rc_ops
Swift ARC optimizer, identity map
```

---

### Section 07: Cross-Block RC Elimination
**File:** `section-07-cross-block-elim.md` | **Status:** Not Started

```
cross-block elimination, multi-block, single-predecessor
known safe, guarding pair, bracketed region
dataflow analysis, lattice, state machine
bottom-up, top-down, matching pass
rc_elim/mod.rs, Swift ARCSequenceOpts
```

---

### Section 08: Salsa-Integrated Borrow Inference
**File:** `section-08-salsa-integration.md` | **Status:** Not Started

```
Salsa, incremental, memoization, query
infer_borrows, AnnotatedSig, dependent query
invalidation, recompilation, callee signature
oric, database, query/mod.rs
```

---

### Section 09: FBIP Enforcement
**File:** `section-09-fbip-enforcement.md` | **Status:** Not Started

```
FBIP, functional but in-place, Koka Perceus
@fbip annotation, analyze_fbip, missed reuse
enforcement, diagnostic, compile error
fbip/mod.rs, CheckFBIP
```

---

### Section 10: Legacy Cleanup & Unification
**File:** `section-10-legacy-cleanup.md` | **Status:** Not Started

```
Tier 1, ExprLowerer, use_arc_codegen, feature flag
delete, remove, cleanup, legacy
JIT, evaluator, wire, ArcClassifier
11K lines, 25 files, monolithic codegen
```

---

### Section 11: Comprehensive Verification
**File:** `section-11-verification.md` | **Status:** Not Started

```
verification, testing, AOT test matrix
dual-execution, JIT vs AOT, RC matching
spec tests, conformance, coverage
arc.rs, aot tests, llvm-test.sh
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Emission Layer Typing | `section-01-emission-layer-typing.md` |
| 02 | ARC Lowerer Gap Closure | `section-02-lowerer-gaps.md` |
| 03 | Closure Codegen Completion | `section-03-closure-codegen.md` |
| 04 | Borrow Inference Hardening | `section-04-borrow-hardening.md` |
| 05 | Builtin Method Architecture | `section-05-builtin-architecture.md` |
| 06 | RC Identity Propagation | `section-06-rc-identity.md` |
| 07 | Cross-Block RC Elimination | `section-07-cross-block-elim.md` |
| 08 | Salsa-Integrated Borrow Inference | `section-08-salsa-integration.md` |
| 09 | FBIP Enforcement | `section-09-fbip-enforcement.md` |
| 10 | Legacy Cleanup & Unification | `section-10-legacy-cleanup.md` |
| 11 | Comprehensive Verification | `section-11-verification.md` |

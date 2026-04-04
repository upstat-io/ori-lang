---
reroute: true
name: "JIT EH"
full_name: "JIT Exception Handling"
status: active
order: 1
---

# JIT Exception Handling Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Runtime Panic Path
**File:** `section-01-runtime.md` | **Status:** Complete

```
ori_panic, ori_panic_cstr, longjmp, setjmp, jit_run_protected
aot_raise_exception, ori_raise_exception, _Unwind_RaiseException
ori_run_main, catch_unwind, ori_try_call, run_main_thunk
JIT_MODE, JIT_RECOVERY_BUF, enter_jit_mode, leave_jit_mode
io/mod.rs, io/jit_recovery.rs, lib.rs, eh_personality.c
stale comments, longjmp references, doc cleanup
```

---

### Section 02: ARC IR InvokeIndirect
**File:** `section-02-arc-ir.md` | **Status:** Complete

```
InvokeIndirect, ArcTerminator, terminate_invoke_indirect
emit_invoke_indirect, catch_unwind_target, ApplyIndirect
lower_call, lower_binary, lower_short_circuit_and, lower_short_circuit_or
short-circuit, &&, ||, BinaryOp::And, BinaryOp::Or
ir/mod.rs, lower/builder/emission.rs, lower/calls/mod.rs, lower/expr/mod.rs
used_vars, substitute_var, successor_block_ids, collect_invoke_defs
```

---

### Section 03: LLVM Emission & Wrappers
**File:** `section-03-llvm-emission.md` | **Status:** Complete

```
compile_tests, test_wrapper, invoke, landingpad, landingpad_catch_all
emit_invoke_indirect, emit_terminator, InvokeIndirect
emit_apply, void return, def_var, EmittedValue, BUG-04-024
ori_catch_cleanup, ori_eh_personality, personality
run_test, did_panic, reset_panic_state
impls.rs, terminators.rs, apply.rs, evaluator/mod.rs
dead_unwind.rs, field_scan/mod.rs, rpo.rs, nounwind/analyze.rs
```

---

### Section 04: Exposed Bug Fixes
**File:** `section-04-exposed-bugs.md` | **Status:** Complete

```
04.1: sdiv, srem, division by zero, checked_div, checked_rem, checked_ops.rs
04.2: COW, double-free, nested map, list mutation, ori_rc_dec, elem_inc_fn
      inc_copied_elements, propagate_elem_header, list_cow.rs
04.3: tuple layout, struct layout, type confusion, misaligned pointer, for-yield
      ori_rc_inc, string data pointer, aggregate_size_with_padding, emitter_utils.rs
04.4a: negative range, infinite range, i64::MAX sentinel, step, next_range
       lower_range, ori_iter_from_range, lower/collections/mod.rs
04.4b: coalesce, ??, ARC leak, lower_coalesce, AIMS, Option wrapper RC
04.4c: coalesce None path, side-effecting block, scope restoration
04.H: dead code, decorative banners, file bloat, strategy.rs
semantic pin, negative pin, TDD, matrix testing
tests/spec/types/integer_safety.ori, tests/spec/expressions/operators_bitwise.ori
tests/spec/collections/cow/nested.ori, tests/spec/collections/cow/sharing.ori
tests/spec/types/struct_layout.ori, tests/spec/test_coalesce_copy.ori
tests/spec/traits/iterator/infinite_range.ori
```

---

### Section 04B: Polymorphic Lambda Monomorphization
**File:** `section-04b-lambda-mono.md` | **Status:** In Progress (blocked by BUG-04-030)

```
polymorphic lambda, forall, Scheme, BoundVar, monomorphization
curried, nested closure, closure capture, type variable
lower_lambda, lambda.rs, scheme_body, scheme_vars
resolve_fully, resolve_body_type, type_subst, body_type_map
compile_lambda_arc, define_phase.rs, emit_arc_function
classify_triviality, PossibleRef, ArcClass, needs_rc
ori_rc_dec type mismatch, LCFail, unresolved type variable
Tag::Scheme, Tag::BoundVar, Tag::Var, VarState::Generalized
```

---

### Section 05: Verification
**File:** `section-05-verification.md` | **Status:** In Progress

```
test-all.sh, dual-exec parity, dual-exec-verify.sh, LLVM spec tests
assert_panics, catch(expr:), matrix testing
debug, release, FastISel, interpreter
tpr-review, impl-hygiene-review
stale comment cleanup, plan annotation cleanup, 01.R
pre-verification, baseline, regression
operators_logical.ori, operators_bitwise.ori
```

---

### Section 06: LCFail Resolution
**File:** `section-06-lcfail-resolution.md` | **Status:** Not Started

```
BUG-04-030, BUG-04-031, BUG-04-032, BUG-04-033, LCFail
Generalized, VarState::Generalized, resolve_fully, generalization.rs
u32::MAX, index out of bounds, 4294967295, ArcVarId, ArcBlockId
find_concrete_copy_of, find_concrete_copy_type, arity matching
ori_iter_join, ori_iter_flatten, jit_allowed, RT_FUNCTIONS
list concat, ori_list_concat_cow, sret, calling convention
PHINode predecessor, short-circuit, &&, ||, side-effect propagation
multi-clause, Ackermann, clause dispatch, build_struct
StructValue, IntValue, ABI, ParamPassing, Direct, Indirect
type_resolve.rs, emitter_utils.rs, short_circuit.rs, runtime_functions.rs
pool/accessors.rs, generalization.rs, abi/mod.rs
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Runtime Panic Path | `section-01-runtime.md` | Complete |
| 02 | ARC IR InvokeIndirect | `section-02-arc-ir.md` | Complete |
| 03 | LLVM Emission & Wrappers | `section-03-llvm-emission.md` | Complete |
| 04 | Exposed Bug Fixes (8 bugs) | `section-04-exposed-bugs.md` | Complete |
| 04B | Polymorphic Lambda Monomorphization | `section-04b-lambda-mono.md` | In Progress (blocked by BUG-04-030) |
| 05 | Verification | `section-05-verification.md` | In Progress |
| 06 | LCFail Resolution (BUG-04-030/031/032/033) | `section-06-lcfail-resolution.md` | Not Started |

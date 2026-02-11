# Eval Legacy Removal Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Audit Canonical Coverage
**File:** `section-01-audit.md` | **Status:** Not Started

```
audit, coverage, canonical root, root_for, has_canon
debug_assert, fallback, gap, missing canonical IR
eval_can, eval, ExprId, CanId, SharedCanonResult
entry point, function registration, module registration
```

---

### Section 02: Stub Remaining FunctionExp Patterns
**File:** `section-02-inline-patterns.md` | **Status:** Not Started

```
FunctionExp, FunctionExpKind, pattern, stub, placeholder
Cache, Parallel, Spawn, Timeout, With
tracing::warn, loud stub, honest stub, not implemented
eval_can_function_exp, can_eval.rs, legacy delegate
roadmap dependency, future implementation
```

---

### Section 03: Remove Entry-Point Fallbacks
**File:** `section-03-entry-points.md` | **Status:** Not Started

```
entry point, run.rs, runner.rs, harness.rs, query
root_for, if/else dispatch, eval_can unconditional
eval(ExprId) removal, public API, Interpreter
oric commands, test runner, test harness
```

---

### Section 04: Remove Function/Method Call Fallbacks
**File:** `section-04-call-dispatch.md` | **Status:** Not Started

```
function_call.rs, method_dispatch.rs, has_canon
call dispatch, dual dispatch, canonical branch
bind_parameters_with_defaults, bind_parameters_with_can_defaults
MemoizedFunction, UserMethod, associated function
default parameter evaluation, legacy default eval
```

---

### Section 05: Strip Dual Body from FunctionValue/UserMethod
**File:** `section-05-dual-body.md` | **Status:** Not Started

```
FunctionValue, UserMethod, dual body, ExprId field
can_body, canon, SharedCanonResult, SharedArena
body field removal, set_canon, has_canon, constructor
composite.rs, user_methods.rs, module_registration.rs
value type cleanup, arena reference
```

---

### Section 06: Delete Legacy Code
**File:** `section-06-delete-legacy.md` | **Status:** Not Started

```
delete, remove, dead code, legacy eval, eval_inner
function_seq.rs, ForIterator, eval_block, eval_loop
eval_cast, eval_unary, eval_binary, eval_call_args
eval_expr_list, eval_function_exp, eval_with_hash_length
exec dead functions, bind_pattern, try_match
PatternExecutor impl, mod.rs cleanup, lib.rs re-export
```

---

### Section 07: Hygiene Pass
**File:** `section-07-hygiene.md` | **Status:** Not Started

```
RAII, scope guard, with_env_scope_result, ScopedInterpreter
manual push/pop, eval_can_block, eval_can_for, eval_can_match
eager collect, lazy iteration, roundtrip elimination
parking_lot Mutex, BufferPrintHandler, std::sync::Mutex
pub(crate), visibility, ModeState, call_count
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Audit Canonical Coverage | `section-01-audit.md` |
| 02 | Stub Remaining FunctionExp Patterns | `section-02-inline-patterns.md` |
| 03 | Remove Entry-Point Fallbacks | `section-03-entry-points.md` |
| 04 | Remove Function/Method Call Fallbacks | `section-04-call-dispatch.md` |
| 05 | Strip Dual Body from FunctionValue/UserMethod | `section-05-dual-body.md` |
| 06 | Delete Legacy Code | `section-06-delete-legacy.md` |
| 07 | Hygiene Pass | `section-07-hygiene.md` |

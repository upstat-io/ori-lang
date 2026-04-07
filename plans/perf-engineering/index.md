---
reroute: true
name: "Interp Perf"
full_name: "Interpreter Performance Engineering"
status: queued
order: 6
---

# Interpreter Performance Engineering Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Benchmark Infrastructure
**File:** `section-01-benchmarks.md` | **Status:** Not Started

```
benchmark, criterion, ackermann, fibonacci, perf-baseline
function call overhead, microseconds, calls per second
interpreter throughput, eval_call, tree-walking
measurement, regression, baseline, flamegraph
```

---

### Section 02: Zero-Allocation Call Path (OPTIONAL — profile-gated)
**File:** `section-02-zero-alloc-calls.md` | **Status:** Not Started

```
CallStack, clone, frame pointer, frame index, push, pop_to
Environment, child, push_scope, pop_scope, scope stack, flat Vec
Rc, RefCell, LocalScope, FxHashMap, heap allocation
create_function_interpreter, prepare_call_env
malloc, free, system time, allocation pressure
arena, pool, pre-allocate, stack-allocate
ControlAction, Break, Continue, Propagate
```

---

### Section 03: Value Passing Optimization (OPTIONAL — profile-gated)
**File:** `section-03-value-passing.md` | **Status:** Not Started

```
Value, clone, Arc, Rc, refcount, increment
bind_parameters_with_defaults, define, FunctionValue
self-binding, capture, closure, bind_captures
ModeState, mode_state, child, budget
args, parameters, argument passing
```

---

### Section 04: Bytecode Compilation
**File:** `section-04-bytecode.md` | **Status:** Not Started

```
bytecode, opcode, instruction, register, VM
CanExpr, CanId, CanBindingPattern, DecisionTree, compile, emit, dispatch loop
direct threading, computed goto, switch dispatch
instruction set, operand, constant pool, Chunk
bytecode compiler, code generation, lowering
FunctionExp, FunctionExpKind, FormatWith
```

---

### Section 05: Register-Based VM
**File:** `section-05-register-vm.md` | **Status:** Not Started

```
register, register file, register allocation
call frame, activation record, stack frame
function prologue, epilogue, calling convention
VM loop, dispatch, fetch-decode-execute
Lua, LuaJIT, CPython, interpreter loop
```

---

### Section 07: Salsa Integration & Transition
**File:** `section-07-integration.md` | **Status:** Not Started

```
Salsa, evaluated, query, tracked, incremental
ConstEval, const_eval, budget, compile-time, type checker
TestRun, test harness, output capture, test mode
feature flag, ORI_USE_BYTECODE_VM, transition, switchover
tree-walker, preservation, retirement, fallback
EvalMode, ModeState, run_evaluation, ModuleEvalResult
canonicalize, SharedCanonResult, CanArena
Evaluator, InterpreterBuilder, eval_can
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Not Started

```
dual execution, parity, correctness, behavioral equivalence
test matrix, spec tests, regression, benchmark
interpreter vs bytecode, semantic equivalence
stress test, recursion depth, allocation count
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Benchmark Infrastructure | `section-01-benchmarks.md` |
| 02 | Zero-Allocation Call Path (OPTIONAL) | `section-02-zero-alloc-calls.md` |
| 03 | Value Passing Optimization (OPTIONAL) | `section-03-value-passing.md` |
| 04 | Bytecode Compilation | `section-04-bytecode.md` |
| 05 | Register-Based VM | `section-05-register-vm.md` |
| 07 | Salsa Integration & Transition | `section-07-integration.md` |
| 06 | Verification | `section-06-verification.md` |

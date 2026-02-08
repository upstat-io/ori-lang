# Eval V2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Value System V2
**File:** `section-01-value-system.md` | **Status:** Not Started

```
value, Value enum, ValuePool, interning, intern pool, ori_value crate
Heap, Arc, Cow, ScalarInt, immediate, scalar, checked arithmetic
representation, promotion, small value, constant
factory method, value creation, heap allocation
ori_patterns decomposition, crate extraction
Zig InternPool, Rust Immediate, Go representation promotion
```

---

### Section 02: Machine Abstraction
**File:** `section-02-machine-abstraction.md` | **Status:** Not Started

```
machine, EvalMode enum, evaluation mode, enum dispatch
interpreter, const eval, test runner, REPL
policy, parametric, match dispatch, Salsa-compatible
Rust CTFE Machine, Go operand mode, Zig comptime
compile-time, runtime, check mode, ModeState
```

---

### Section 03: Environment V2
**File:** `section-03-environment.md` | **Status:** Not Started

```
environment, scope, binding, variable, lookup
ScopeStack, FxHashMap, hash-based, scope stack
RAII, guard, drop, push scope, pop scope, ScopedInterpreter
capture, closure, lexical, CapturedEnv, SmallVec
define, update, mutability, immutable, mutable
```

---

### Section 04: Pattern Compilation
**File:** `section-04-pattern-compilation.md` | **Status:** Not Started

```
pattern, match, decision tree, exhaustiveness
compile, arm, guard, wildcard, constructor
Gleam exhaustiveness, Elm DecisionTree, Roc exhaustive
PatternCompiler, DecisionTree, Switch, Leaf, Chain
variant inference, narrowing, unreachable
```

---

### Section 05: Structured Control Flow
**File:** `section-05-control-flow.md` | **Status:** Not Started

```
control flow, break, continue, loop, for, while
join point, JoinPointId, structured, block
EvalError, ControlAction, error-based, signal
EvalFlow, FlowOrError, eval_flow, flow action
try operator, question mark, propagate, early exit
Roc join point, Go SSA, static scheduling
```

---

### Section 06: Method Resolution V2
**File:** `section-06-method-resolution.md` | **Status:** Not Started

```
method, dispatch, resolution, resolver, chain
builtin, user, trait, impl, associated
hash, lookup, MethodTable, MethodResolver
numeric, collection, variant, unit
type-directed, receiver, self
```

---

### Section 07: Constant Evaluation
**File:** `section-07-constant-evaluation.md` | **Status:** Not Started

```
constant, const, compile-time, folding, propagation
static, initialization, scheduling, eager, lazy
Zig comptime, Go constant, Rust CTFE
memoization, cache, deterministic
constexpr, pure, side-effect-free
```

---

### Section 08: Canonical Eval IR
**File:** `section-08-canonical-ir.md` | **Status:** Not Started

```
IR, intermediate representation, lowering, canonical
EvalIR, ExprArena, transformation, optimization
dead code, elimination, unreachable, unused
Roc mono IR, Elm optimized AST, Zig AIR
arena, node, instruction, statement
```

---

### Section 09: Reference Counting Integration
**File:** `section-09-reference-counting.md` | **Status:** Not Started

```
reference counting, ARC, RC, Inc, Dec, Free
Perceus, reuse, borrow, ownership, drop
insertion, analysis, optimization, elision
Roc inc_dec, Swift ARC, Koka FBIP, Lean RC
memory, allocation, deallocation, leak
```

---

### Section 10: Tracing & Diagnostics V2
**File:** `section-10-tracing-diagnostics.md` | **Status:** Not Started

```
tracing, diagnostic, error, backtrace, span
ORI_LOG, debug, trace, instrument
EvalError, structured, report, message
Rust InterpError, Elm error reporting, Gleam diagnostic
performance, profiling, instrumentation
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | Value System V2 | `section-01-value-system.md` | 0 |
| 02 | Machine Abstraction | `section-02-machine-abstraction.md` | 0 |
| 03 | Environment V2 | `section-03-environment.md` | 0 |
| 04 | Pattern Compilation | `section-04-pattern-compilation.md` | 1 |
| 05 | Structured Control Flow | `section-05-control-flow.md` | 1 |
| 06 | Method Resolution V2 | `section-06-method-resolution.md` | 1 |
| 07 | Constant Evaluation | `section-07-constant-evaluation.md` | 2 |
| 08 | Canonical Eval IR | `section-08-canonical-ir.md` | 2 |
| 09 | Reference Counting Integration | `section-09-reference-counting.md` | 3 |
| 10 | Tracing & Diagnostics V2 | `section-10-tracing-diagnostics.md` | 3 |

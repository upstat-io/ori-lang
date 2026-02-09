# Canonical IR & Evaluation Pipeline — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Canonical IR
**File:** `section-01-canonical-ir.md` | **Status:** Complete (2026-02-09)

```
CanExpr, CanArena, CanId, CanRange, CanNode
canonical, IR, type definition, arena, index
ConstantId, ConstantPool, DecisionTreeId, DecisionTreePool
CanonResult, ori_canon, ori_ir, crate
decision tree, relocation, ori_arc, shared types
```

---

### Section 02: AST Lowering
**File:** `section-02-lowering.md` | **Status:** Complete (2026-02-09)

```
lower, lowering, Lowerer, ExprArena, CanArena, transform
ExprKind, mapping, variant, desugar, desugaring, sugar
CallNamed, MethodCallNamed, named argument, positional, reorder
TemplateLiteral, TemplateFull, template, interpolation, to_str, concat
ListWithSpread, MapWithSpread, StructWithSpread, spread, merge
type attachment, Idx, expr_types, resolved type
```

---

### Section 03: Pattern Compilation
**File:** `section-03-pattern-compilation.md` | **Status:** Complete (2026-02-09)

```
pattern, match, decision tree, DecisionTree, compile, Maranget
PatternMatrix, PatternRow, CompilerPattern, column selection
Switch, Leaf, Guard, Fail, ScrutineePath, PathInstruction
TestKind, TestValue, EnumTag, IntEq, StrEq, ListLen
eval_decision_tree, resolve_path, interpreter
MatchPattern, Wildcard, Binding, Variant, Struct, Tuple, List, Or, As
```

---

### Section 04: Constant Folding
**File:** `section-04-constant-folding.md` | **Status:** Complete (2026-02-09)

```
constant, const, fold, folding, compile-time, Constness
ConstantPool, ConstantId, intern, dedup, sentinel
arithmetic, pure operator, dead branch, elimination
overflow, division-by-zero, defer, runtime
```

---

### Section 05: Evaluation Modes
**File:** `section-05-eval-modes.md` | **Status:** Complete (2026-02-09)

```
EvalMode, Interpret, ConstEval, TestRun, mode, policy
ModeState, budget, call_count, test_results, memo_cache
allows_io, allows_capability, collects_tests, max_recursion_depth
Salsa, query, builder, SharedPrintHandler, PrintHandlerImpl
ori run, ori check, ori test, machine
```

---

### Section 06: Structured Diagnostics
**File:** `section-06-diagnostics.md` | **Status:** In Progress (2026-02-09)

```
EvalErrorKind, EvalError, structured, typed, category
CallStack, CallFrame, EvalBacktrace, BacktraceFrame
backtrace, call stack, depth, overflow, frame
EvalNote, context, note, span, message
Diagnostic, conversion, E6xxx, error code, eval_error_to_diagnostic
EvalCounters, profile, instrumentation, statistics, ModeState
```

---

### Section 07: Backend Migration
**File:** `section-07-backend-migration.md` | **Status:** Not Started

```
migration, backend, rewrite, dispatch, eval_canon
ori_eval, ori_arc, ori_llvm, ExprKind, CanExpr
shadow, validate, cut-over, delete, dead code
sync, verification, cross-backend, divergence
invoke, landingpad, resume, unwind, panic, cleanup, exception handling
ArcTerminator, Invoke, Resume, __gxx_personality_v0, Itanium EH
cross-block, RC elimination, dataflow, rc_elim, edge pair
ASAN, leak, end-to-end, AOT panic, debug info
```

---

## Quick Reference

| ID | Title | File | Tier |
|----|-------|------|------|
| 01 | Canonical IR | `section-01-canonical-ir.md` | 0 |
| 02 | AST Lowering | `section-02-lowering.md` | 1 |
| 03 | Pattern Compilation | `section-03-pattern-compilation.md` | 1 |
| 04 | Constant Folding | `section-04-constant-folding.md` | 1 |
| 05 | Evaluation Modes | `section-05-eval-modes.md` | 1 |
| 06 | Structured Diagnostics | `section-06-diagnostics.md` | 1 |
| 07 | Backend Migration | `section-07-backend-migration.md` | 2 |

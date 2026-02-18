---
section: 10
title: Control Flow
status: in-progress
tier: 3
goal: Complete control flow constructs
spec:
  - spec/09-expressions.md
  - spec/17-blocks-and-scope.md
  - spec/19-control-flow.md
  - spec/20-errors-and-panics.md
sections:
  - id: "10.1"
    title: if Expression
    status: in-progress
  - id: "10.2"
    title: for Expressions
    status: in-progress
  - id: "10.3"
    title: loop Expression
    status: in-progress
  - id: "10.4"
    title: Error Propagation (?)
    status: not-started
  - id: "10.5"
    title: Let Bindings
    status: in-progress
  - id: "10.6"
    title: Scoping
    status: in-progress
  - id: "10.7"
    title: Panics
    status: in-progress
  - id: "10.8"
    title: Index Expressions
    status: in-progress
  - id: "10.9"
    title: Section Completion Checklist
    status: not-started
---

# Section 10: Control Flow

**Goal**: Complete control flow constructs

> **SPEC**: `spec/09-expressions.md`, `spec/17-blocks-and-scope.md`, `spec/19-control-flow.md`, `spec/20-errors-and-panics.md`
> **PROPOSALS**:
> - `proposals/approved/if-expression-proposal.md` — Conditional expression semantics
> - `proposals/approved/error-return-traces-proposal.md` — Automatic error trace collection
> - `proposals/approved/loop-expression-proposal.md` — Loop expression semantics

---

## 10.1 if Expression

**Proposal**: `proposals/approved/if-expression-proposal.md`

- [x] **Implement**: Parse `if cond then expr else expr` — spec/09-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — if expression
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori` — 19 tests
  - [ ] **LLVM Support**: LLVM codegen for if expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — if expression codegen

- [x] **Implement**: Else-if chains (grammar convenience) — spec/09-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — chained if parsing
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for chained conditions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — chained conditions codegen

- [x] **Implement**: Condition must be `bool` — spec/09-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — condition type checking
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [x] **Implement**: Branch type unification — spec/09-expressions.md § Conditional [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — branch type unification
  - [x] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for branch type unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — branch type unification codegen

- [ ] **Implement**: Without else: then-branch must be `void` or `Never` — spec/09-expressions.md § Conditional
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — void branch validation
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Never coercion in branches — spec/09-expressions.md § Conditional
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — Never coercion
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for diverging branches
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — Never coercion codegen

- [ ] **Implement**: Struct literal restriction in condition — spec/09-expressions.md § Conditional
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — struct literal exclusion
  - [ ] **Ori Tests**: `tests/compile-fail/if_struct_literal.ori`
  - [ ] **LLVM Support**: N/A (parse-time check)
  - [ ] **LLVM Rust Tests**: N/A

---

## 10.2 for Expressions

> **NOTE**: This is the `for x in items do/yield expr` **expression** syntax for iteration.
> The `for(over:, match:, default:)` **pattern** with named arguments is a separate construct in Section 8.

**Imperative form (do):**

- [x] **Implement**: Parse `for x in items do expr` — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-do parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — 29 tests
  - [ ] **LLVM Support**: LLVM codegen for for-do expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do codegen

- [x] **Implement**: Bind loop variable — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — loop variable binding
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop variable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — loop variable binding codegen

- [x] **Implement**: Execute body for side effects — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — body execution
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop body execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop body execution codegen

- [x] **Implement**: Result type `void` — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — for-do type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-do void type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do void type codegen

**Collection building (yield):**

- [x] **Implement**: Parse `for x in items yield expr` — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-yield parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield codegen

- [x] **Implement**: Collect results into list — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — yield collection
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for yield collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — yield collection codegen

- [x] **Implement**: Result type `[T]` — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — for-yield type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield list type codegen

**With guards:**

- [x] **Implement**: Parse `for x in items if guard yield expr` — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — for-guard parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — for_do_with_guard, for_yield_with_guard tests
  - [ ] **LLVM Support**: LLVM codegen for for-guard expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-guard codegen

- [x] **Implement**: Only yield when guard true — spec/09-expressions.md § For Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — guard filtering
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — guard_all_filtered, guard_transform tests
  - [ ] **LLVM Support**: LLVM codegen for guard filtering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — guard filtering codegen

**For-yield comprehensions** (proposals/approved/for-yield-comprehensions-proposal.md):

- [ ] **Implement**: Type inference for collection target — proposals/approved/for-yield-comprehensions-proposal.md § Type Inference
  - [ ] Infer from context (`let list: [int] = for ...`)
  - [ ] Default to list when no context
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — for-yield type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for type-directed collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — comprehension type inference

- [ ] **Implement**: Collect into any `Collect<T>` type — proposals/approved/for-yield-comprehensions-proposal.md § Collect Target
  - [ ] Support `Set<T>` collection
  - [ ] Support `{K: V}` collection via `Collect<(K, V)>`
  - [ ] Duplicate map keys overwrite earlier values
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — multi-target collection
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for multi-target collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — multi-target collection codegen

- [ ] **Implement**: Nested `for` clauses — proposals/approved/for-yield-comprehensions-proposal.md § Nested Comprehensions
  - [ ] Parse `for x in xs for y in ys yield expr`
  - [ ] Desugar to `flat_map`
  - [ ] Support filters on each clause
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — nested for parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested comprehensions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — nested comprehensions codegen

- [ ] **Implement**: Break/continue in yield context — proposals/approved/for-yield-comprehensions-proposal.md § Break and Continue
  - [ ] `continue` skips current element
  - [ ] `continue value` substitutes value for yield expression
  - [ ] `break` stops iteration, collects results so far
  - [ ] `break value` stops and adds final value
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — yield break/continue
  - [ ] **Ori Tests**: `tests/spec/expressions/comprehensions.ori`
  - [ ] **LLVM Support**: LLVM codegen for yield break/continue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — yield break/continue codegen

---

## 10.3 loop Expression

**Proposal**: `proposals/approved/loop-expression-proposal.md`

- [x] **Implement**: Parse `loop(body)` — spec/09-expressions.md § Loop Expressions [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — loop parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break, loop_break_value, loop_int tests
  - [ ] **LLVM Support**: LLVM codegen for loop expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop expression codegen

- [x] **Implement**: Loop until `break` — spec/19-control-flow.md § Break [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — break handling
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break test
  - [ ] **LLVM Support**: LLVM codegen for break handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break handling codegen

- [x] **Implement**: Body is single expression; use `run(...)` for sequences — proposals/approved/loop-expression-proposal.md § Body [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — loop body parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — all loop tests use `loop(run(...))`
  - [ ] **LLVM Support**: LLVM codegen for loop body
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop body codegen

- [x] **Implement**: Parse `break` with optional value — spec/19-control-flow.md § Break [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — break parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_break_value, loop_conditional_break tests
  - [ ] **LLVM Support**: LLVM codegen for break with value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break with value codegen

- [x] **Implement**: Parse `continue` — spec/19-control-flow.md § Continue [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — continue parsing
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_continue test
  - [ ] **LLVM Support**: LLVM codegen for continue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — continue codegen

- [ ] **Implement**: `continue value` error in loop — proposals/approved/loop-expression-proposal.md § Continue With Value
  - [ ] Error E0861 when continue has value in loop context
  - [ ] Helpful suggestion to use break or remove value
  - [ ] **Rust Tests**: `oric/src/typeck/checker/loops.rs` — continue value validation
  - [ ] **Ori Tests**: `tests/compile-fail/loop_continue_value.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [x] **Implement**: Result type from `break` value — proposals/approved/loop-expression-proposal.md § Loop Type [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — break type inference
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_break_value, loop_int tests
  - [ ] **LLVM Support**: LLVM codegen for break type inference
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break type inference codegen

- [x] **Implement**: Type `void` for break without value — proposals/approved/loop-expression-proposal.md § Break Without Value [done] (2026-02-10)
  - [x] **Rust Tests**: Type checker — void loop type
  - [x] **Ori Tests**: `tests/spec/expressions/loops.ori` — loop_with_break (void function)
  - [ ] **LLVM Support**: LLVM codegen for void loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — void loop codegen

- [ ] **Implement**: Type `Never` for infinite loops — proposals/approved/loop-expression-proposal.md § Infinite Loop Type
  - [ ] Loop with no break has type Never
  - [ ] Coerces to any type in value context
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — Never loop type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for Never loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — Never loop codegen

- [ ] **Implement**: Multiple break paths type unification — proposals/approved/loop-expression-proposal.md § Multiple Break Paths
  - [ ] All breaks must have compatible types
  - [ ] Error E0860 for type mismatch
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — break type unification
  - [ ] **Ori Tests**: `tests/compile-fail/loop_break_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for break unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break unification codegen

**Labeled loops:**

- [ ] **Implement**: Parse `loop:name(body)` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled loop parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for labeled loop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — labeled loop codegen

- [ ] **Implement**: Parse `for:name x in items` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled for parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for labeled for
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — labeled for codegen

- [ ] **Implement**: Parse `break:name` and `continue:name` — spec/19-control-flow.md § Label Reference
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — label reference parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for label references
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — label references codegen

**Labeled loop semantics** (proposals/approved/labeled-loops-proposal.md):

- [ ] **Implement**: Label scope rules — proposals/approved/labeled-loops-proposal.md § Label Scope
  - [ ] Labels visible only within their loop body
  - [ ] No language-imposed nesting depth limit
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — label scope validation
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for label scope
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — label scope codegen

- [ ] **Implement**: No label shadowing — proposals/approved/labeled-loops-proposal.md § No Shadowing
  - [ ] Error if label already in scope
  - [ ] Error E0871 with helpful suggestion
  - [ ] **Rust Tests**: `oric/src/typeck/checker/labels.rs` — shadowing detection
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_loop_shadow.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Type consistency for `break:label value` — proposals/approved/labeled-loops-proposal.md § Type Consistency
  - [ ] All break paths for a labeled loop must produce same type
  - [ ] Error E0872 for type mismatch
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — break type unification
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_break_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for typed break
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — typed break codegen

- [ ] **Implement**: `continue:label value` in for-yield — proposals/approved/labeled-loops-proposal.md § Continue With Value in For-Yield
  - [ ] Value type must match target loop's yield element type
  - [ ] Inner loop's partial collection discarded
  - [ ] Value contributed to outer loop's collection
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — continue value in yield
  - [ ] **Ori Tests**: `tests/spec/expressions/labeled_loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for continue value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — continue value codegen

- [ ] **Implement**: `continue:label value` error in for-do — proposals/approved/labeled-loops-proposal.md § Continue With Value in For-Do
  - [ ] Error E0873 when continue has value in for-do context
  - [ ] Helpful suggestion to use for-yield or remove value
  - [ ] **Rust Tests**: `oric/src/typeck/checker/loops.rs` — continue value validation
  - [ ] **Ori Tests**: `tests/compile-fail/labeled_continue_in_do.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

- [ ] **Implement**: Undefined label error — proposals/approved/labeled-loops-proposal.md § Error Messages
  - [ ] Error E0870 for undefined label
  - [ ] Suggest similar labels if available
  - [ ] **Rust Tests**: `oric/src/resolve/labels.rs` — undefined label detection
  - [ ] **Ori Tests**: `tests/compile-fail/undefined_label.ori`
  - [ ] **LLVM Support**: N/A (compile-time check)
  - [ ] **LLVM Rust Tests**: N/A

---

## 10.4 Error Propagation (?)

- [ ] **Implement**: Parse postfix `?` operator — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — ? operator parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for ? operator
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — ? operator codegen

- [ ] **Implement**: On `Result<T, E>`: unwrap `Ok` or return `Err` — spec/19-control-flow.md § On Result
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — Result propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Result propagation codegen

- [ ] **Implement**: On `Option<T>`: unwrap `Some` or return `None` — spec/19-control-flow.md § On Option
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — Option propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Option propagation codegen

- [ ] **Implement**: Only valid in functions returning `Result`/`Option` — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `oric/src/typeck/checker/propagation.rs` — context validation
  - [ ] **Ori Tests**: `tests/compile-fail/invalid_propagation.ori`
  - [ ] **LLVM Support**: LLVM codegen for propagation context validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — context validation codegen

**Error Return Traces** (proposals/approved/error-return-traces-proposal.md):

- [ ] **Implement**: Automatic trace collection at `?` propagation points
  - [ ] `?` operator records source location (file, line, column, function name)
  - [ ] Trace entries stored internally in Error type
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — trace collection
  - [ ] **Ori Tests**: `tests/spec/errors/trace_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for trace collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — trace collection codegen

- [ ] **Implement**: `TraceEntry` type — proposals/approved/error-return-traces-proposal.md § Error Type Enhancement
  - [ ] Fields: `function: str`, `file: str`, `line: int`, `column: int`
  - [ ] **Rust Tests**: `ori_ir/src/types/error.rs` — TraceEntry type
  - [ ] **Ori Tests**: `tests/spec/errors/trace_entry.ori`
  - [ ] **LLVM Support**: LLVM codegen for TraceEntry type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — TraceEntry type codegen

- [ ] **Implement**: Error trace methods — proposals/approved/error-return-traces-proposal.md § Accessing Traces
  - [ ] `Error.trace() -> str` — formatted trace string
  - [ ] `Error.trace_entries() -> [TraceEntry]` — programmatic access
  - [ ] `Error.has_trace() -> bool` — check if trace available
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error trace methods
  - [ ] **Ori Tests**: `tests/spec/errors/trace_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for Error trace methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Error trace methods codegen

- [ ] **Implement**: `Printable` for Error includes trace — proposals/approved/error-return-traces-proposal.md § Printing Errors
  - [ ] `str(error)` includes trace in output
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error printing
  - [ ] **Ori Tests**: `tests/spec/errors/trace_printing.ori`
  - [ ] **LLVM Support**: LLVM codegen for Error printing with trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Error printing codegen

- [ ] **Implement**: `Result.context()` method — proposals/approved/error-return-traces-proposal.md § Result Methods
  - [ ] `.context(msg: str)` adds context while preserving trace
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.context
  - [ ] **Ori Tests**: `tests/spec/errors/context_method.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.context method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Result.context codegen

- [x] **Implement**: `Traceable` trait for built-in Error type — implemented in §3.13 (2026-02-17)
  - [x] `@with_trace(self, entry: TraceEntry) -> Self` — push trace entry
  - [x] `@trace(self) -> str` — formatted trace string
  - [x] `@trace_entries(self) -> [TraceEntry]` — list of TraceEntry structs
  - [x] `@has_trace(self) -> bool` — check if trace exists
  - Note: Custom error types implementing Traceable is deferred (requires user-defined trait impls)
  - [ ] **LLVM Support**: LLVM codegen for Traceable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Traceable trait codegen

---

## 10.5 Let Bindings

- [x] **Implement**: Parse `let x = expr` — spec/09-expressions.md § Let Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — let binding
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 17 tests (let_inferred, let_string, etc.)
  - [ ] **LLVM Support**: LLVM codegen for let binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — let binding codegen

- [x] **Implement**: Parse `let mut x = expr` — spec/09-expressions.md § Mutable Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and evaluator — mutable binding
  - [x] **Ori Tests**: `tests/spec/expressions/mutation.ori` — 15 tests (mutable_basic, mutable_loop, etc.)
  - [ ] **LLVM Support**: LLVM codegen for mutable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mutable binding codegen

- [x] **Implement**: Parse `let x: Type = expr` — spec/09-expressions.md § Let Bindings [done] (2026-02-10)
  - [x] **Rust Tests**: Parser and type checker — typed binding
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — let_annotated_int, let_annotated_str, etc.
  - [ ] **LLVM Support**: LLVM codegen for typed binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — typed binding codegen

- [x] **Implement**: Parse struct destructuring `let { x, y } = val` — spec/09-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — struct destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — struct_destructure_shorthand, struct_destructure_rename
  - [ ] **LLVM Support**: LLVM codegen for struct destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — struct destructuring codegen

- [x] **Implement**: Parse tuple destructuring `let (a, b) = val` — spec/09-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — tuple destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — tuple_destructure test
  - [ ] **LLVM Support**: LLVM codegen for tuple destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — tuple destructuring codegen

- [x] **Implement**: Parse list destructuring `let [head, ..tail] = val` — spec/09-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser — list destructure parsing
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — list_destructure_basic, list_destructure_head, list_destructure_with_rest
  - [ ] **LLVM Support**: LLVM codegen for list destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — list destructuring codegen

---

## 10.6 Scoping

- [x] **Implement**: Lexical scoping — spec/17-blocks-and-scope.md § Lexical Scoping [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — lexical scope tests
  - [x] **Ori Tests**: `tests/spec/expressions/block_scope.ori` — 3 tests (let_bindings_in_run, nested_run_shadowing, run_returns_last_expression)
  - [ ] **LLVM Support**: LLVM codegen for lexical scoping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lexical scoping codegen

- [x] **Implement**: No hoisting — spec/17-blocks-and-scope.md § No Hoisting [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — no hoisting tests
  - [x] **Ori Tests**: `tests/spec/expressions/block_scope.ori` — sequential binding verified
  - [ ] **LLVM Support**: LLVM codegen for no hoisting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — no hoisting codegen

- [x] **Implement**: Shadowing — spec/17-blocks-and-scope.md § Shadowing [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — shadowing tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — let_shadow, let_shadow_different_type; `mutation.ori` — shadow_mutability
  - [ ] **LLVM Support**: LLVM codegen for shadowing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — shadowing codegen

- [x] **Implement**: Lambda capture by value — spec/17-blocks-and-scope.md § Lambda Capture [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — capture tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori` — 29 tests (closure_capture, closure_capture_multiple, closure_nested)
  - [ ] **LLVM Support**: LLVM codegen for lambda capture by value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lambda capture codegen

---

## 10.7 Panics

- [ ] **Implement**: Implicit panics (index out of bounds, division by zero) — spec/20-errors-and-panics.md § Implicit Panics
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — implicit panic tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`
  - [ ] **LLVM Support**: LLVM codegen for implicit panics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — implicit panics codegen

- [x] **Implement**: `panic(message)` function — spec/20-errors-and-panics.md § Explicit Panic [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator — panic function
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — panic in short-circuit tests; `operators_bitwise.ori` — assert_panics tests
  - [ ] **LLVM Support**: LLVM codegen for panic function
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — panic function codegen

- [ ] **Implement**: `catch(expr)` pattern — spec/20-errors-and-panics.md § Catching Panics
  - [ ] **Rust Tests**: `oric/src/patterns/catch.rs` — catch pattern tests
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`
  - [ ] **LLVM Support**: LLVM codegen for catch pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — catch pattern codegen

- [ ] **Implement**: `PanicInfo` type — spec/20-errors-and-panics.md § PanicInfo
  - [ ] **Rust Tests**: `ori_ir/src/types/panic.rs` — PanicInfo type tests
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`
  - [ ] **LLVM Support**: LLVM codegen for PanicInfo type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — PanicInfo type codegen

---

## 10.8 Index Expressions — [partial] Interpreter Complete

- [x] **Implement**: `#` length symbol in index brackets (`list[# - 1]`) — spec/09-expressions.md § Index Access [done] (2026-02-10)
  - [x] **Parser**: Parse `#` as `ExprKind::HashLength` inside `[...]` — `ori_parse/src/grammar/expr/postfix.rs`
  - [x] **Type Checker**: Resolve `HashLength` to receiver's length type (`int`) — `ori_typeck/src/infer/mod.rs`
  - [x] **Evaluator**: Evaluate `HashLength` as `len(receiver)` in index context — `ori_eval/src/interpreter/mod.rs`
  - [x] **Ori Tests**: `tests/spec/expressions/index_access.ori` — hash_last, hash_second_last, hash_first, hash_middle, hash_arithmetic (35 total tests)
  - [ ] **LLVM Support**: LLVM codegen for hash length in index (placeholder exists, needs real impl)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — hash length codegen

**Implementation Notes:**
- Added `IN_INDEX` context flag to `ParseContext`
- Parser recognizes `#` (TokenKind::Hash) as `ExprKind::HashLength` only inside index brackets
- Type checker and evaluator already had full support for `HashLength`
- 901 tests pass (up from 900)
- LLVM has placeholder (returns 0), needs proper implementation later

---

## 10.9 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]`
- [ ] Spec updated: `spec/09-expressions.md`, `spec/19-control-flow.md` reflect implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: All control flow constructs work including labeled loops, scoping, and panic handling

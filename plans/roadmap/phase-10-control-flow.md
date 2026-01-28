# Phase 10: Control Flow

**Goal**: Complete control flow constructs

> **SPEC**: `spec/09-expressions.md`, `spec/17-blocks-and-scope.md`, `spec/19-control-flow.md`, `spec/20-errors-and-panics.md`
> **PROPOSAL**: `proposals/approved/error-return-traces-proposal.md` — Automatic error trace collection

---

## 10.1 if Expression

- [ ] **Implement**: Parse `if cond then expr else expr` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — if expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for if expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — if expression codegen

- [ ] **Implement**: Chained conditions — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — chained if parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for chained conditions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — chained conditions codegen

- [ ] **Implement**: Condition must be `bool` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — condition type checking
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for condition type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — condition type checking codegen

- [ ] **Implement**: Both branches same type — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — branch type unification
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`
  - [ ] **LLVM Support**: LLVM codegen for branch type unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — branch type unification codegen

---

## 10.2 for Expressions

> **NOTE**: This is the `for x in items do/yield expr` **expression** syntax for iteration.
> The `for(over:, match:, default:)` **pattern** with named arguments is a separate construct in Phase 8.

**Imperative form (do):**

- [ ] **Implement**: Parse `for x in items do expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-do parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-do expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do codegen

- [ ] **Implement**: Bind loop variable — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — loop variable binding
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop variable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — loop variable binding codegen

- [ ] **Implement**: Execute body for side effects — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — body execution
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop body execution
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop body execution codegen

- [ ] **Implement**: Result type `void` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — for-do type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-do void type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-do void type codegen

**Collection building (yield):**

- [ ] **Implement**: Parse `for x in items yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-yield parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield codegen

- [ ] **Implement**: Collect results into list — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — yield collection
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for yield collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — yield collection codegen

- [ ] **Implement**: Result type `[T]` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — for-yield type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-yield list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-yield list type codegen

**With guards:**

- [ ] **Implement**: Parse `for x in items if guard yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-guard parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for for-guard expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — for-guard codegen

- [ ] **Implement**: Only yield when guard true — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — guard filtering
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard filtering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — guard filtering codegen

---

## 10.3 loop Expression

- [ ] **Implement**: Parse `loop(body)` — spec/09-expressions.md § Loop Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — loop parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — loop expression codegen

- [ ] **Implement**: Loop until `break` — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — break handling
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for break handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break handling codegen

- [ ] **Implement**: Parse `break` with optional value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — break parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for break with value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break with value codegen

- [ ] **Implement**: Parse `continue` — spec/19-control-flow.md § Continue
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — continue parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for continue
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — continue codegen

- [ ] **Implement**: Result type from `break` value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — break type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for break type inference
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/control_flow_tests.rs` — break type inference codegen

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

- [ ] **Implement**: `Traceable` trait for custom error types — proposals/approved/error-return-traces-proposal.md § Custom Error Types
  - [ ] `@with_trace(self, trace: [TraceEntry]) -> Self`
  - [ ] `@get_trace(self) -> [TraceEntry]`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/traits.rs` — Traceable trait
  - [ ] **Ori Tests**: `tests/spec/errors/traceable_trait.ori`
  - [ ] **LLVM Support**: LLVM codegen for Traceable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_propagation_tests.rs` — Traceable trait codegen

---

## 10.5 Let Bindings

- [ ] **Implement**: Parse `let x = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — let binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for let binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — let binding codegen

- [ ] **Implement**: Parse `let mut x = expr` — spec/09-expressions.md § Mutable Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — mutable binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for mutable binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mutable binding codegen

- [ ] **Implement**: Parse `let x: Type = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — typed binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for typed binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — typed binding codegen

- [ ] **Implement**: Parse struct destructuring `let { x, y } = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — struct destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — struct destructuring codegen

- [ ] **Implement**: Parse tuple destructuring `let (a, b) = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — tuple destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for tuple destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — tuple destructuring codegen

- [ ] **Implement**: Parse list destructuring `let [head, ..tail] = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for list destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — list destructuring codegen

---

## 10.6 Scoping

- [ ] **Implement**: Lexical scoping — spec/17-blocks-and-scope.md § Lexical Scoping
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — lexical scope tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`
  - [ ] **LLVM Support**: LLVM codegen for lexical scoping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lexical scoping codegen

- [ ] **Implement**: No hoisting — spec/17-blocks-and-scope.md § No Hoisting
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — no hoisting tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`
  - [ ] **LLVM Support**: LLVM codegen for no hoisting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — no hoisting codegen

- [ ] **Implement**: Shadowing — spec/17-blocks-and-scope.md § Shadowing
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — shadowing tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`
  - [ ] **LLVM Support**: LLVM codegen for shadowing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — shadowing codegen

- [ ] **Implement**: Lambda capture by value — spec/17-blocks-and-scope.md § Lambda Capture
  - [ ] **Rust Tests**: `oric/src/eval/exec/lambda.rs` — capture tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`
  - [ ] **LLVM Support**: LLVM codegen for lambda capture by value
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/scope_tests.rs` — lambda capture codegen

---

## 10.7 Panics

- [ ] **Implement**: Implicit panics (index out of bounds, division by zero) — spec/20-errors-and-panics.md § Implicit Panics
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — implicit panic tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`
  - [ ] **LLVM Support**: LLVM codegen for implicit panics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — implicit panics codegen

- [ ] **Implement**: `panic(message)` function — spec/20-errors-and-panics.md § Explicit Panic
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — panic function tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`
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

## 10.8 Index Expressions — ✅ COMPLETED 2026-01-28

- [x] **Implement**: `#` length symbol in index brackets (`list[# - 1]`) — spec/09-expressions.md § Index Access
  - [x] **Parser**: Parse `#` as `ExprKind::HashLength` inside `[...]` — `ori_parse/src/grammar/expr/postfix.rs`
  - [x] **Type Checker**: Resolve `HashLength` to receiver's length type (`int`) — `ori_typeck/src/infer/mod.rs` (already implemented)
  - [x] **Evaluator**: Evaluate `HashLength` as `len(receiver)` in index context — `ori_eval/src/interpreter/mod.rs` (already implemented)
  - [x] **Ori Tests**: `tests/spec/types/collections.ori` — `test_list_index_last`
  - [ ] **LLVM Support**: LLVM codegen for hash length in index
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — hash length codegen

**Implementation Notes (2026-01-28):**
- Added `IN_INDEX` context flag to `ParseContext`
- Parser recognizes `#` (TokenKind::Hash) as `ExprKind::HashLength` only inside index brackets
- Type checker and evaluator already had full support for `HashLength`
- 901 tests pass (up from 900)

---

## 10.9 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/09-expressions.md`, `spec/19-control-flow.md` reflect implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: All control flow constructs work including labeled loops, scoping, and panic handling

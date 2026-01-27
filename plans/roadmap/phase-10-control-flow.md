# Phase 10: Control Flow

**Goal**: Complete control flow constructs

> **SPEC**: `spec/09-expressions.md`, `spec/17-blocks-and-scope.md`, `spec/19-control-flow.md`, `spec/20-errors-and-panics.md`
> **PROPOSAL**: `proposals/approved/error-return-traces-proposal.md` — Automatic error trace collection

---

## 10.1 if Expression

- [ ] **Implement**: Parse `if cond then expr else expr` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — if expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`

- [ ] **Implement**: Chained conditions — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — chained if parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`

- [ ] **Implement**: Condition must be `bool` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — condition type checking
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`

- [ ] **Implement**: Both branches same type — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — branch type unification
  - [ ] **Ori Tests**: `tests/spec/expressions/conditionals.ori`

---

## 10.2 for Expressions

> **NOTE**: This is the `for x in items do/yield expr` **expression** syntax for iteration.
> The `for(over:, match:, default:)` **pattern** with named arguments is a separate construct in Phase 8.

**Imperative form (do):**

- [ ] **Implement**: Parse `for x in items do expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-do parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Bind loop variable — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — loop variable binding
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Execute body for side effects — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — body execution
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Result type `void` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — for-do type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

**Collection building (yield):**

- [ ] **Implement**: Parse `for x in items yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-yield parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Collect results into list — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — yield collection
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Result type `[T]` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — for-yield type
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

**With guards:**

- [ ] **Implement**: Parse `for x in items if guard yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — for-guard parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Only yield when guard true — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — guard filtering
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

---

## 10.3 loop Expression

- [ ] **Implement**: Parse `loop(body)` — spec/09-expressions.md § Loop Expressions
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — loop parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Loop until `break` — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `oric/src/eval/exec/loops.rs` — break handling
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Parse `break` with optional value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — break parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Parse `continue` — spec/19-control-flow.md § Continue
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — continue parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Result type from `break` value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — break type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

**Labeled loops:**

- [ ] **Implement**: Parse `loop:name(body)` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled loop parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Parse `for:name x in items` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — labeled for parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Parse `break:name` and `continue:name` — spec/19-control-flow.md § Label Reference
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — label reference parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

---

## 10.4 Error Propagation (?)

- [ ] **Implement**: Parse postfix `?` operator — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — ? operator parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`

- [ ] **Implement**: On `Result<T, E>`: unwrap `Ok` or return `Err` — spec/19-control-flow.md § On Result
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — Result propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`

- [ ] **Implement**: On `Option<T>`: unwrap `Some` or return `None` — spec/19-control-flow.md § On Option
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — Option propagation
  - [ ] **Ori Tests**: `tests/spec/expressions/postfix.ori`

- [ ] **Implement**: Only valid in functions returning `Result`/`Option` — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `oric/src/typeck/checker/propagation.rs` — context validation
  - [ ] **Ori Tests**: `tests/compile-fail/invalid_propagation.ori`

**Error Return Traces** (proposals/approved/error-return-traces-proposal.md):

- [ ] **Implement**: Automatic trace collection at `?` propagation points
  - [ ] `?` operator records source location (file, line, column, function name)
  - [ ] Trace entries stored internally in Error type
  - [ ] **Rust Tests**: `oric/src/eval/exec/postfix.rs` — trace collection
  - [ ] **Ori Tests**: `tests/spec/errors/trace_collection.ori`

- [ ] **Implement**: `TraceEntry` type — proposals/approved/error-return-traces-proposal.md § Error Type Enhancement
  - [ ] Fields: `function: str`, `file: str`, `line: int`, `column: int`
  - [ ] **Rust Tests**: `ori_ir/src/types/error.rs` — TraceEntry type
  - [ ] **Ori Tests**: `tests/spec/errors/trace_entry.ori`

- [ ] **Implement**: Error trace methods — proposals/approved/error-return-traces-proposal.md § Accessing Traces
  - [ ] `Error.trace() -> str` — formatted trace string
  - [ ] `Error.trace_entries() -> [TraceEntry]` — programmatic access
  - [ ] `Error.has_trace() -> bool` — check if trace available
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error trace methods
  - [ ] **Ori Tests**: `tests/spec/errors/trace_methods.ori`

- [ ] **Implement**: `Printable` for Error includes trace — proposals/approved/error-return-traces-proposal.md § Printing Errors
  - [ ] `str(error)` includes trace in output
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Error printing
  - [ ] **Ori Tests**: `tests/spec/errors/trace_printing.ori`

- [ ] **Implement**: `Result.context()` method — proposals/approved/error-return-traces-proposal.md § Result Methods
  - [ ] `.context(msg: str)` adds context while preserving trace
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.context
  - [ ] **Ori Tests**: `tests/spec/errors/context_method.ori`

- [ ] **Implement**: `Traceable` trait for custom error types — proposals/approved/error-return-traces-proposal.md § Custom Error Types
  - [ ] `@with_trace(self, trace: [TraceEntry]) -> Self`
  - [ ] `@get_trace(self) -> [TraceEntry]`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/traits.rs` — Traceable trait
  - [ ] **Ori Tests**: `tests/spec/errors/traceable_trait.ori`

---

## 10.5 Let Bindings

- [ ] **Implement**: Parse `let x = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — let binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Parse `let mut x = expr` — spec/09-expressions.md § Mutable Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — mutable binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Parse `let x: Type = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `ori_parse/src/grammar/stmt.rs` — typed binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Parse struct destructuring `let { x, y } = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — struct destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Parse tuple destructuring `let (a, b) = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — tuple destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Parse list destructuring `let [head, ..tail] = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — list destructure parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

---

## 10.6 Scoping

- [ ] **Implement**: Lexical scoping — spec/17-blocks-and-scope.md § Lexical Scoping
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — lexical scope tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`

- [ ] **Implement**: No hoisting — spec/17-blocks-and-scope.md § No Hoisting
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — no hoisting tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`

- [ ] **Implement**: Shadowing — spec/17-blocks-and-scope.md § Shadowing
  - [ ] **Rust Tests**: `oric/src/eval/environment.rs` — shadowing tests
  - [ ] **Ori Tests**: `tests/spec/expressions/scoping.ori`

- [ ] **Implement**: Lambda capture by value — spec/17-blocks-and-scope.md § Lambda Capture
  - [ ] **Rust Tests**: `oric/src/eval/exec/lambda.rs` — capture tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

---

## 10.7 Panics

- [ ] **Implement**: Implicit panics (index out of bounds, division by zero) — spec/20-errors-and-panics.md § Implicit Panics
  - [ ] **Rust Tests**: `oric/src/eval/exec/binary.rs` — implicit panic tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`

- [ ] **Implement**: `panic(message)` function — spec/20-errors-and-panics.md § Explicit Panic
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — panic function tests
  - [ ] **Ori Tests**: `tests/spec/expressions/panics.ori`

- [ ] **Implement**: `catch(expr)` pattern — spec/20-errors-and-panics.md § Catching Panics
  - [ ] **Rust Tests**: `oric/src/patterns/catch.rs` — catch pattern tests
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`

- [ ] **Implement**: `PanicInfo` type — spec/20-errors-and-panics.md § PanicInfo
  - [ ] **Rust Tests**: `ori_ir/src/types/panic.rs` — PanicInfo type tests
  - [ ] **Ori Tests**: `tests/spec/patterns/catch.ori`

---

## 10.8 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/09-expressions.md`, `spec/19-control-flow.md` reflect implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: All control flow constructs work including labeled loops, scoping, and panic handling

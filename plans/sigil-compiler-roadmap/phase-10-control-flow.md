# Phase 10: Control Flow

**Goal**: Complete control flow constructs

> **SPEC**: `spec/09-expressions.md`, `spec/17-blocks-and-scope.md`, `spec/19-control-flow.md`, `spec/20-errors-and-panics.md`
> **PROPOSAL**: `proposals/approved/error-return-traces-proposal.md` — Automatic error trace collection

---

## 10.1 if Expression

- [ ] **Implement**: Parse `if cond then expr else expr` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — if expression parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/conditionals.si`

- [ ] **Implement**: Chained conditions — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — chained if parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/conditionals.si`

- [ ] **Implement**: Condition must be `bool` — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — condition type checking
  - [ ] **Sigil Tests**: `tests/spec/expressions/conditionals.si`

- [ ] **Implement**: Both branches same type — spec/09-expressions.md § If Expressions
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — branch type unification
  - [ ] **Sigil Tests**: `tests/spec/expressions/conditionals.si`

---

## 10.2 for Expressions

> **NOTE**: This is the `for x in items do/yield expr` **expression** syntax for iteration.
> The `for(over:, match:, default:)` **pattern** with named arguments is a separate construct in Phase 8.

**Imperative form (do):**

- [ ] **Implement**: Parse `for x in items do expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — for-do parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Bind loop variable — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/loops.rs` — loop variable binding
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Execute body for side effects — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/loops.rs` — body execution
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Result type `void` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — for-do type
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

**Collection building (yield):**

- [ ] **Implement**: Parse `for x in items yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — for-yield parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Collect results into list — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/loops.rs` — yield collection
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Result type `[T]` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — for-yield type
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

**With guards:**

- [ ] **Implement**: Parse `for x in items if guard yield expr` — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — for-guard parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Only yield when guard true — spec/09-expressions.md § For Expressions
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/loops.rs` — guard filtering
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

---

## 10.3 loop Expression

- [ ] **Implement**: Parse `loop(body)` — spec/09-expressions.md § Loop Expressions
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — loop parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Loop until `break` — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/loops.rs` — break handling
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Parse `break` with optional value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — break parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Parse `continue` — spec/19-control-flow.md § Continue
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — continue parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Result type from `break` value — spec/19-control-flow.md § Break
  - [ ] **Rust Tests**: `sigilc/src/typeck/infer/expr.rs` — break type inference
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

**Labeled loops:**

- [ ] **Implement**: Parse `loop:name(body)` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — labeled loop parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Parse `for:name x in items` — spec/19-control-flow.md § Labeled Loops
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — labeled for parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

- [ ] **Implement**: Parse `break:name` and `continue:name` — spec/19-control-flow.md § Label Reference
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — label reference parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/loops.si`

---

## 10.4 Error Propagation (?)

- [ ] **Implement**: Parse postfix `?` operator — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/postfix.rs` — ? operator parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/postfix.si`

- [ ] **Implement**: On `Result<T, E>`: unwrap `Ok` or return `Err` — spec/19-control-flow.md § On Result
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/postfix.rs` — Result propagation
  - [ ] **Sigil Tests**: `tests/spec/expressions/postfix.si`

- [ ] **Implement**: On `Option<T>`: unwrap `Some` or return `None` — spec/19-control-flow.md § On Option
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/postfix.rs` — Option propagation
  - [ ] **Sigil Tests**: `tests/spec/expressions/postfix.si`

- [ ] **Implement**: Only valid in functions returning `Result`/`Option` — spec/19-control-flow.md § Error Propagation
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/propagation.rs` — context validation
  - [ ] **Sigil Tests**: `tests/compile-fail/invalid_propagation.si`

**Error Return Traces** (proposals/approved/error-return-traces-proposal.md):

- [ ] **Implement**: Automatic trace collection at `?` propagation points
  - [ ] `?` operator records source location (file, line, column, function name)
  - [ ] Trace entries stored internally in Error type
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/postfix.rs` — trace collection
  - [ ] **Sigil Tests**: `tests/spec/errors/trace_collection.si`

- [ ] **Implement**: `TraceEntry` type — proposals/approved/error-return-traces-proposal.md § Error Type Enhancement
  - [ ] Fields: `function: str`, `file: str`, `line: int`, `column: int`
  - [ ] **Rust Tests**: `sigil_ir/src/types/error.rs` — TraceEntry type
  - [ ] **Sigil Tests**: `tests/spec/errors/trace_entry.si`

- [ ] **Implement**: Error trace methods — proposals/approved/error-return-traces-proposal.md § Accessing Traces
  - [ ] `Error.trace() -> str` — formatted trace string
  - [ ] `Error.trace_entries() -> [TraceEntry]` — programmatic access
  - [ ] `Error.has_trace() -> bool` — check if trace available
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Error trace methods
  - [ ] **Sigil Tests**: `tests/spec/errors/trace_methods.si`

- [ ] **Implement**: `Printable` for Error includes trace — proposals/approved/error-return-traces-proposal.md § Printing Errors
  - [ ] `str(error)` includes trace in output
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Error printing
  - [ ] **Sigil Tests**: `tests/spec/errors/trace_printing.si`

- [ ] **Implement**: `Result.context()` method — proposals/approved/error-return-traces-proposal.md § Result Methods
  - [ ] `.context(msg: str)` adds context while preserving trace
  - [ ] **Rust Tests**: `sigil_eval/src/methods.rs` — Result.context
  - [ ] **Sigil Tests**: `tests/spec/errors/context_method.si`

- [ ] **Implement**: `Traceable` trait for custom error types — proposals/approved/error-return-traces-proposal.md § Custom Error Types
  - [ ] `@with_trace(self, trace: [TraceEntry]) -> Self`
  - [ ] `@get_trace(self) -> [TraceEntry]`
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/traits.rs` — Traceable trait
  - [ ] **Sigil Tests**: `tests/spec/errors/traceable_trait.si`

---

## 10.5 Let Bindings

- [ ] **Implement**: Parse `let x = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/stmt.rs` — let binding parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [ ] **Implement**: Parse `let mut x = expr` — spec/09-expressions.md § Mutable Bindings
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/stmt.rs` — mutable binding parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [ ] **Implement**: Parse `let x: Type = expr` — spec/09-expressions.md § Let Bindings
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/stmt.rs` — typed binding parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [ ] **Implement**: Parse struct destructuring `let { x, y } = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — struct destructure parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [ ] **Implement**: Parse tuple destructuring `let (a, b) = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — tuple destructure parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [ ] **Implement**: Parse list destructuring `let [head, ..tail] = val` — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — list destructure parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/bindings.si`

---

## 10.6 Scoping

- [ ] **Implement**: Lexical scoping — spec/17-blocks-and-scope.md § Lexical Scoping
  - [ ] **Rust Tests**: `sigilc/src/eval/environment.rs` — lexical scope tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/scoping.si`

- [ ] **Implement**: No hoisting — spec/17-blocks-and-scope.md § No Hoisting
  - [ ] **Rust Tests**: `sigilc/src/eval/environment.rs` — no hoisting tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/scoping.si`

- [ ] **Implement**: Shadowing — spec/17-blocks-and-scope.md § Shadowing
  - [ ] **Rust Tests**: `sigilc/src/eval/environment.rs` — shadowing tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/scoping.si`

- [ ] **Implement**: Lambda capture by value — spec/17-blocks-and-scope.md § Lambda Capture
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/lambda.rs` — capture tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/lambdas.si`

---

## 10.7 Panics

- [ ] **Implement**: Implicit panics (index out of bounds, division by zero) — spec/20-errors-and-panics.md § Implicit Panics
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/binary.rs` — implicit panic tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/panics.si`

- [ ] **Implement**: `panic(message)` function — spec/20-errors-and-panics.md § Explicit Panic
  - [ ] **Rust Tests**: `sigilc/src/eval/builtins.rs` — panic function tests
  - [ ] **Sigil Tests**: `tests/spec/expressions/panics.si`

- [ ] **Implement**: `catch(expr)` pattern — spec/20-errors-and-panics.md § Catching Panics
  - [ ] **Rust Tests**: `sigilc/src/patterns/catch.rs` — catch pattern tests
  - [ ] **Sigil Tests**: `tests/spec/patterns/catch.si`

- [ ] **Implement**: `PanicInfo` type — spec/20-errors-and-panics.md § PanicInfo
  - [ ] **Rust Tests**: `sigil_ir/src/types/panic.rs` — PanicInfo type tests
  - [ ] **Sigil Tests**: `tests/spec/patterns/catch.si`

---

## 10.8 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/09-expressions.md`, `spec/19-control-flow.md` reflect implementation
- [ ] CLAUDE.md updated if syntax/behavior changed
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: All control flow constructs work including labeled loops, scoping, and panic handling

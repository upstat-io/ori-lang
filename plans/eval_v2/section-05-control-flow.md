---
section: "05"
title: Structured Control Flow
status: not-started
goal: Replace error-based break/continue with structured control flow using join points
sections:
  - id: "05.1"
    title: ControlAction Enum
    status: not-started
  - id: "05.2"
    title: Join Points
    status: not-started
  - id: "05.3"
    title: Loop Evaluation Rewrite
    status: not-started
  - id: "05.4"
    title: Try Operator Rewrite
    status: not-started
  - id: "05.5"
    title: Panic / Todo / Unreachable
    status: not-started
---

# Section 05: Structured Control Flow

**Status:** 📋 Planned
**Goal:** Replace error-based `break`/`continue` signaling with structured control flow using a `ControlAction` enum and Roc-inspired join points — making control flow explicit and composable.

---

## Phase 0: Prerequisite — Labeled Break/Continue AST Extension

**Blocking prerequisite** for Sections 05.1 and 05.3. Currently `ExprKind::Break(ExprId)` and `ExprKind::Continue(ExprId)` carry no label. Labeled break/continue requires cross-crate changes:

1. **`ori_ir`**: Extend `ExprKind::Break(ExprId)` to `ExprKind::Break(ExprId, Option<Name>)` and `ExprKind::Continue(ExprId)` to `ExprKind::Continue(ExprId, Option<Name>)`. **Size impact**: The current `ExprKind` has a 24-byte size assertion — adding `Option<Name>` (4 bytes for interned `Name`) may push some variants over. Audit with `std::mem::size_of::<ExprKind>()` and adjust the assertion if needed.
2. **`ori_parse`**: Parse labeled syntax (`break:label value`, `continue:label`). Extend the break/continue parsing paths to recognize the `:label` suffix.
3. **`ori_types`**: Update type checking for Break/Continue to thread through the optional label. Validate that labels reference enclosing loops.
4. **`ori_eval`**: Consume the label in `ControlAction::Break`/`Continue` (covered in 05.1/05.3 below).

This prerequisite must be completed before implementing `ControlAction` with label support.

---

## Prior Art Analysis

### Current Ori: Error-Based Control Flow
The current evaluator uses `EvalError` variants to signal `break` and `continue`:
```rust
Break(v) => Err(EvalError::break_with(val))
Continue(v) => Err(EvalError::continue_with(val))
```
These "errors" propagate up the call stack until caught by the enclosing loop. This conflates control flow with error handling, making it hard to distinguish real errors from flow signals and complicating the `?` operator.

### Roc: Join Points
Roc's mono IR uses `Join { id, parameters, body, remainder }` and `Jump(JoinPointId, args)` for structured control flow. A join point is a labeled continuation that can be jumped to from multiple places. This maps directly to efficient code generation (no exception unwinding) and makes control flow explicit in the IR.

### Rust CTFE: Terminator-Based Flow
Rust's const evaluator processes MIR terminators (`Goto`, `Call`, `Return`, `SwitchInt`, `Drop`, `Assert`) that make control flow explicit at the IR level. Each step advances to a new basic block. No exceptions or error-based signaling.

### Go: SSA Control Flow
Go's SSA representation uses basic blocks with explicit edges. Flow is tracked via `FlowLabel` (merge points), `FlowAssignment` (variable updates), and `FlowCondition` (narrowing). All flow is structural, never exception-based.

---

## 05.1 ControlAction Enum

Replace `EvalError` control flow with a proper return type.

**Module placement:** All types in this section — `ControlAction`, `FlowOrError`, `EvalFlow`, `PropagateInfo` — belong in `ori_eval` (e.g., `ori_eval/src/flow.rs` or `ori_eval/src/control.rs`). They are **not** part of `ori_patterns` or `ori_ir`. The `From<EvalError> for FlowOrError` impl enables using `?` on `EvalResult` in `EvalFlow`-returning functions.

```rust
pub enum ControlAction {
    /// Break out of the enclosing loop, optionally with a value.
    /// The optional Name is the loop label (e.g., `break:outer value`).
    Break(Value, Option<Name>),
    /// Continue to the next iteration, optionally with a value.
    /// The optional Name is the loop label (e.g., `continue:outer`).
    Continue(Value, Option<Name>),
    /// Propagate an error value (from postfix `?` operator)
    Propagate(PropagateInfo),
}

pub struct PropagateInfo {
    /// The error/None value being propagated
    pub value: Value,
    /// Source span of the `?` operator
    pub span: Span,
    /// Optional diagnostic message for error reporting parity
    /// (e.g., the error message from Result::Err, or "None" for Option::None)
    pub message: Option<String>,
    // NOTE: No type info here — we trust the type checker to have validated
    // that `?` is applied to Option/Result types. Optionally add debug_assert
    // at function boundary for development defense-in-depth.
}

/// Already exists in ori_patterns — updated to be generic with default.
/// Follows the `std::io::Result<T>` pattern: `EvalResult` (no param) defaults to
/// `Result<Value, EvalError>`, while `EvalResult<()>` or `EvalResult<bool>` can be
/// used for non-Value-returning operations.
pub type EvalResult<T = Value> = Result<T, EvalError>;

/// New type alias: extended result that includes control flow.
/// Visibility: pub(crate) — internal to ori_eval. External callers use EvalResult.
pub(crate) type EvalFlow = Result<Value, FlowOrError>;

pub enum FlowOrError {
    Flow(ControlAction),
    Error(EvalError),
}

impl FlowOrError {
    /// Convert a FlowOrError into an EvalError.
    /// Flow actions become "uncaught control flow" errors; Error variants pass through.
    /// Used at loop/function boundaries to convert unhandled flow into errors.
    pub fn into_eval_error(self) -> EvalError {
        match self {
            FlowOrError::Error(e) => e,
            FlowOrError::Flow(action) => EvalError::uncaught_control_flow(action),
        }
    }
}
```

> **Migration note:** The current `eval(ExprId) -> EvalResult` is split into `eval_flow(ExprId) -> EvalFlow` (internal, handles ControlAction) and the public `eval(ExprId) -> EvalResult` which wraps `eval_flow` and converts uncaught flow actions to errors. Existing callers that only need values continue using `eval()`; only loop/try/function-boundary code uses `eval_flow()`.

**Design choice**: Use `Result<Value, FlowOrError>` rather than a tri-state enum. This leverages Rust's `?` operator for error propagation while keeping flow actions separate from errors.

**Why not a single `EvalOutcome`?**
- `Result<V, E>` integrates with Rust's `?` — can't use `?` with a custom tri-state
- Most expression evaluations return values, not actions — `Result` is the common case
- Flow actions only arise in specific contexts (loops, `?` operator)
- Cleaner: `eval_inner` returns `EvalFlow`, loop/try handlers convert to `EvalResult`

> **Rejected Alternative: `EvalOutcome` tri-state enum**
> ```rust
> // REJECTED — included for design context only
> pub enum EvalOutcome {
>     Value(Value),
>     Action(ControlAction),
>     Error(EvalError),
> }
> ```
> This was considered but rejected because it cannot integrate with Rust's `?` operator,
> would require manual propagation at every call site, and makes the common case (Value)
> heavier. The `Result<Value, FlowOrError>` approach was chosen instead.

- [ ] Define `ControlAction` enum
  - [ ] `Break(Value, Option<Name>)` — break from loop with optional value and optional label
  - [ ] `Continue(Value, Option<Name>)` — continue loop with optional value and optional label
  - [ ] `Propagate(PropagateInfo)` — postfix `?` operator propagation
- [ ] Define `FlowOrError` enum
  - [ ] `Flow(ControlAction)` — control flow action
  - [ ] `Error(EvalError)` — actual error
- [ ] Implement `From<EvalError> for FlowOrError` (for `?` operator)
- [ ] Implement `From<ControlAction> for FlowOrError` (for flow signals)
- [ ] Update `eval_inner` to return `EvalFlow` internally
  - [ ] All callers that handle loops/try convert to `EvalResult`
  - [ ] Public `eval()` method still returns `EvalResult`

---

## 05.2 Join Points

For the EvalIR (Section 08), introduce join points for structured control flow:

```rust
/// A join point is a labeled continuation within evaluation.
/// Inspired by Roc's mono IR join points.
///
/// Parameters are stored in the EvalIrArena extra array (Pool pattern).
/// Extra layout: [count, name0, name1, ...]
pub struct JoinPoint {
    pub id: JoinPointId,
    pub params_extra: u32,  // index into EvalIrArena.extra — layout: [count, name0, name1, ...]
    pub body: EvalIrId,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct JoinPointId(u32);

// Jump is an EvalIrNode variant (Section 08), not a separate struct:
//   EvalIrNode::Jump { target: JoinPointId, extra: u32 }
//   Extra layout: [count, arg0, arg1, ...]
// Arguments are accessed via ir_arena.get_children(extra).
```

**Usage in EvalIR**:
```rust
// A for loop compiles to:
// join continue_point(acc) =
//   if has_next(iter) then
//     let item = next(iter)
//     let new_acc = <body>(acc, item)
//     jump continue_point(new_acc)
//   else
//     acc
//
// join break_point(val) =
//   val
//
// jump continue_point(initial_acc)
```

Join points make break/continue targets **explicit** rather than implicit (error propagation). They're also the natural compilation target for LLVM basic blocks (Section 21 of roadmap).

- [ ] Define `JoinPoint` and `JoinPointId` types
  - [ ] `JoinPointId` is a simple newtype around u32
  - [ ] `JoinPoint` has id, `params_extra: u32` (index into `EvalIrArena.extra`), and body
  - [ ] Parameters stored in extra array: `[count, name0, name1, ...]`
  - [ ] Access via `ir_arena.get_children(params_extra)` (caller wraps as `Name`)
- [ ] Add join point support to EvalIR (Section 08)
  - [ ] `EvalIrNode::Join { point: JoinPoint, body: EvalIrId }`
  - [ ] `EvalIrNode::Jump { target: JoinPointId, extra: u32 }` — args in extra array: `[count, arg0, arg1, ...]`
- [ ] Document join point semantics
  - [ ] Join points are NOT functions — no closure creation, no recursion
  - [ ] They're continuations: jumped to, not called
  - [ ] Parameters are bound in the join point's scope
  - [ ] Each join point has exactly one use site (the enclosing Join node)

---

## 05.3 Loop Evaluation Rewrite

Rewrite loop evaluation to use `ControlAction` instead of `EvalError`:

```rust
impl<'a> Interpreter<'a> {
    pub fn eval_loop(&mut self, body: ExprId) -> EvalResult {
        loop {
            match self.eval_flow(body) {
                Ok(value) => {
                    // Body produced a value — loop continues
                    // (void for statement loops, value for expression loops)
                }
                Err(FlowOrError::Flow(ControlAction::Break(val, _label))) => {
                    return Ok(val);
                }
                Err(FlowOrError::Flow(ControlAction::Continue(_, _label))) => {
                    continue;
                }
                // Propagate passes through loops to the function boundary.
                // Errors also pass through. Both are Err variants that the
                // loop does not handle — they propagate out naturally.
                Err(e) => return Err(e.into_eval_error()),
            }
        }
    }

    pub fn eval_for(
        &mut self,
        binding: Name,
        iter: ExprId,
        guard: ExprId,
        body: ExprId,
        is_yield: bool,
    ) -> EvalResult {
        let iter_value = self.eval(iter)?;
        // Construct a ForIterator by pattern matching on the iterable value.
        // No `eval_to_iterator` method — iteration is done by matching the
        // value type directly:
        //   Value::List(items) → iterate over items
        //   Value::Range { start, end, inclusive } → iterate over range
        //   _ → EvalError::not_iterable(iter_value.type_name())
        //
        // NOTE: Str and Map iteration are NEW FEATURES not present in
        // current code. They are deferred to a follow-up task and should
        // not be implemented as part of this section's control flow rewrite.
        let mut for_iter = self.value_to_for_iterator(&iter_value)?;
        let mut results = if is_yield { Some(Vec::new()) } else { None };

        // Private helper for loop body evaluation control flow.
        // enum LoopAction { Continue, Break(Value) }

        while let Some(item) = for_iter.next() {
            self.with_binding(binding, item, Mutability::Immutable, |scoped| {
                // Check guard (uses ExprId convention — guard.is_present()
                // returns false for absent guards, matching current codebase)
                if guard.is_present() {
                    let guard_val = scoped.eval_flow(guard)?;
                    if !guard_val.is_truthy() {
                        return Ok(LoopAction::Continue);
                    }
                }

                match scoped.eval_flow(body) {
                    Ok(val) => {
                        if let Some(ref mut results) = results {
                            results.push(val);
                        }
                        Ok(LoopAction::Continue)
                    }
                    Err(FlowOrError::Flow(ControlAction::Break(val, _label))) => {
                        // TODO: if label doesn't match this loop, re-propagate
                        Ok(LoopAction::Break(val))
                    }
                    Err(FlowOrError::Flow(ControlAction::Continue(val, _label))) => {
                        // TODO: if label doesn't match this loop, re-propagate
                        if let Some(ref mut results) = results {
                            if val != Value::Void {
                                results.push(val);
                            }
                        }
                        Ok(LoopAction::Continue)
                    }
                    // Propagate and Error pass through to function boundary
                    Err(e) => Err(e.into_eval_error()),
                }
            })?;
        }

        match results {
            Some(items) => Ok(Value::list(items)),
            None => Ok(Value::Void),
        }
    }
}
```

- [ ] Add `eval_flow(expr) -> Result<Value, FlowOrError>` to Interpreter
  - [ ] Wraps `eval_inner` but catches ControlAction instead of EvalError control variants
  - [ ] Public `eval()` converts any uncaught ControlAction to EvalError
- [ ] Rewrite `eval_loop()` to use `ControlAction::Break`/`Continue`
  - [ ] No more `to_loop_action()` helper converting EvalError
  - [ ] Direct pattern matching on FlowOrError
- [ ] Rewrite `eval_for()` to use `ControlAction`
  - [ ] Same pattern — match on FlowOrError
  - [ ] Yield loops collect values from `Continue` actions
- [ ] Rewrite `Break` and `Continue` expression evaluation
  - [ ] `Expr::Break(v, label)` → `Err(FlowOrError::Flow(ControlAction::Break(eval(v)?, label)))`
  - [ ] `Expr::Continue(v, label)` → `Err(FlowOrError::Flow(ControlAction::Continue(eval(v)?, label)))`
- [ ] Implement `value_to_for_iterator(&self, value: &Value) -> EvalResult<ForIterator>` (lazy iterator, matching current code's `ForIterator` pattern)
  - [ ] Pattern match on Value::List, Value::Range (current iterable types)
  - [ ] Return EvalError::not_iterable for non-iterable types
  - [ ] **Deferred new features:** Value::Str (character iteration) and Value::Map (key-value tuple iteration) are new iterable types not present in current code. Add them as a separate follow-up task, not as part of the control flow rewrite.
- [ ] Support labeled loops (`loop:label`, `for:label`) — depends on Phase 0 prerequisite above
  - [ ] `break:label value` dispatches Break with `Some(label_name)`
  - [ ] `continue:label` dispatches Continue with `Some(label_name)`
  - [ ] Loop handlers compare labels: if label is `Some(name)` and doesn't match the current loop's label, re-propagate the Break/Continue to the enclosing loop
  - [ ] Unlabeled break/continue (`None` label) always targets the innermost loop
- [ ] Remove `ControlFlow` from `EvalError`
  - [ ] `EvalError::control_flow` field → removed
  - [ ] `EvalError::break_with()` → removed
  - [ ] `EvalError::continue_with()` → removed

---

## 05.4 Postfix `?` Operator Rewrite

This section covers only the **postfix `?` operator** (e.g., `result?`). The `try(...)` function_seq pattern (which returns `Ok(Value::Err(e))` instead of using `ControlAction::Propagate`) is a separate mechanism preserved in its current form — it does not use control flow actions and will be adapted as needed in Section 08 (EvalIR).

Rewrite the postfix `?` operator to use `ControlAction::Propagate`:

```rust
impl<'a> Interpreter<'a> {
    pub fn eval_try(&mut self, inner: ExprId) -> EvalFlow {
        let value = self.eval(inner)?;
        match &value {
            Value::Ok(v) => Ok((**v).clone()),
            Value::Some(v) => Ok((**v).clone()),
            Value::Err(e) => Err(FlowOrError::Flow(ControlAction::Propagate(
                PropagateInfo {
                    value: Value::Err(e.clone()),
                    span: self.current_span(),
                    message: Some(e.display_string()),
                }
            ))),
            Value::None => Err(FlowOrError::Flow(ControlAction::Propagate(
                PropagateInfo {
                    value: Value::None,
                    span: self.current_span(),
                    message: None,
                }
            ))),
            // Non-Option/Result: pass through unchanged
            other => Ok(other.clone()),
        }
    }
}
```

**Function-level propagation handling**:
```rust
pub fn eval_function_body(&mut self, body: ExprId) -> EvalResult {
    match self.eval_flow(body) {
        Ok(val) => Ok(val),
        Err(FlowOrError::Flow(ControlAction::Propagate(info))) => {
            // Propagate converts to the function's return type.
            // We trust the type checker validated this, but add a debug_assert
            // for development defense-in-depth.
            debug_assert!(
                matches!(&info.value, Value::Err(_) | Value::None),
                "Propagate should only carry Err/None values, got: {:?}",
                info.value
            );
            Ok(info.value)
        }
        Err(FlowOrError::Flow(action)) => {
            // Uncaught break/continue — error
            Err(EvalError::uncaught_control_flow(action))
        }
        Err(FlowOrError::Error(e)) => Err(e),
    }
}
```

- [ ] Rewrite `eval_try()` to use `ControlAction::Propagate`
  - [ ] Ok/Some → unwrap
  - [ ] Err/None → propagate with span information
  - [ ] Other → pass through
- [ ] Update function body evaluation to catch Propagate
  - [ ] At function boundaries, Propagate becomes a return value
  - [ ] Uncaught break/continue at function boundary → error
- [ ] Remove `propagated_value` from `EvalError`
  - [ ] `EvalError::propagated_value` → removed
  - [ ] `EvalError::propagate()` → removed

---

## 05.5 Panic / Todo / Unreachable

Handle the three early-termination constructs. Currently, `panic`/`todo`/`unreachable` are dispatched through the existing `PatternDefinition` / `FunctionExpKind` path in the evaluator. This section enhances that path with structured error kinds and backtrace support, rather than replacing it.

**Integration with existing dispatch:** The current evaluator handles `FunctionExpKind::Panic`, `FunctionExpKind::Todo`, and `FunctionExpKind::Unreachable` within the `PatternDefinition` dispatch in `eval_function_exp`. The change here is to make the error output more structured:
- Replace ad-hoc error construction with dedicated `EvalErrorKind` variants
- Add backtrace capture at the point of panic
- Keep the existing `FunctionExpKind` dispatch path — do NOT create a separate `PanicKind` enum for the interpreter. (Note: Section 08 introduces `PanicKind` as part of the EvalIR lowered representation, which is distinct from the interpreter dispatch path.)

```rust
impl<'a> Interpreter<'a> {
    /// Enhanced panic handling within the existing FunctionExpKind dispatch.
    /// Called from the existing eval_function_exp match arm for Panic/Todo/Unreachable.
    ///
    /// Note: `message` uses ExprId here (pre-Section 08). After Section 08
    /// migration, this will change to EvalIrId when evaluation moves to the
    /// EvalIR path.
    fn eval_panic_kind(&mut self, message: ExprId, kind: &FunctionExpKind) -> EvalFlow {
        let msg_value = self.eval(message)?;
        let msg_str = msg_value.display_string();
        let backtrace = self.capture_backtrace();
        match kind {
            FunctionExpKind::Panic => Err(FlowOrError::Error(
                EvalError::panic_called(msg_str).with_backtrace(backtrace)
            )),
            FunctionExpKind::Todo => Err(FlowOrError::Error(
                EvalError::todo(msg_str).with_backtrace(backtrace)
            )),
            FunctionExpKind::Unreachable => Err(FlowOrError::Error(
                EvalError::unreachable(msg_str).with_backtrace(backtrace)
            )),
            _ => unreachable!("eval_panic_kind called with non-panic FunctionExpKind"),
        }
    }
}
```

- [ ] Add `PanicCalled`, `TodoReached`, `UnreachableReached` to `EvalErrorKind` (Section 10)
  - **Note:** `EvalErrorKind` is a **new enum** to be introduced — it does not exist yet. The current `EvalError` uses a flat `message: String` field. The structured `EvalErrorKind` enum design is defined in Section 10 (Tracing & Diagnostics). This checklist item depends on Section 10 being implemented first.
  - **Transitional note (Phase 2 → Phase 4):** During Phase 2, the `panic_called`/`todo`/`unreachable` factory methods use the current flat `message: String` pattern (e.g., `EvalError::panic_called(msg: String) -> EvalError`). During Phase 4 (Section 10), these are refactored to use `EvalErrorKind` variants (`EvalErrorKind::PanicCalled { message }`, etc.). The factory method signatures remain stable; only the internal construction changes.
- [ ] **New features to implement** (not yet in codebase):
  - [ ] `EvalError::panic_called(msg: String) -> EvalError` — constructor for panic errors
  - [ ] `EvalError::todo(msg: String) -> EvalError` — constructor for todo errors
  - [ ] `EvalError::unreachable(msg: String) -> EvalError` — constructor for unreachable errors
  - [ ] `EvalError::with_backtrace(self, backtrace: EvalBacktrace) -> EvalError` — attach backtrace to error
  - [ ] `Interpreter::capture_backtrace(&self) -> EvalBacktrace` — capture current call stack
  - [ ] Backtrace infrastructure depends on Section 10 (Tracing & Diagnostics). Implement basic versions here; full backtrace formatting in Section 10.
- [ ] Integrate `eval_panic_kind` into existing `eval_function_exp` dispatch for `FunctionExpKind::Panic/Todo/Unreachable`
- [ ] `@panic` handler integration: if user defines `@panic(info)`, call it before aborting
- [ ] All three produce `EvalError` (not `ControlAction`) — they are errors, not flow

---

## 05.6 Completion Checklist

- [ ] `ControlAction` enum defined (Break, Continue with optional label, Propagate)
- [ ] `FlowOrError` enum separates control flow from errors
- [ ] `eval_flow()` method added to Interpreter
- [ ] Loop evaluation rewritten (no error-based break/continue)
- [ ] For evaluation rewritten (no error-based break/continue)
- [ ] Labeled loops supported (`break:label`, `continue:label`)
- [ ] Postfix `?` operator rewritten (uses Propagate action)
- [ ] `try(...)` function_seq pattern preserved separately
- [ ] Panic/todo/unreachable handled as errors with backtraces
- [ ] `ControlFlow` removed from `EvalError`
- [ ] `propagated_value` removed from `EvalError`
- [ ] `JoinPoint` types defined for future EvalIR integration
- [ ] All loop/match/try tests pass unchanged

**Exit Criteria:** Control flow is explicit via `ControlAction`, not smuggled through `EvalError`. Break, continue, and `?` propagation have clear, separate code paths. Panic/todo/unreachable produce structured errors with backtraces.

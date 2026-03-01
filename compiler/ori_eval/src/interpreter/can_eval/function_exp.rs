//! Lambda creation, collection literals, and function expression evaluation.

use ori_ir::canon::{CanId, CanMapEntryRange, CanParamRange};
use ori_ir::{FunctionExpKind, Name};
use ori_patterns::{ControlAction, EvalError, EvalResult, RangeValue, Value};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use super::super::Interpreter;
use crate::errors::{map_key_not_hashable, range_bound_not_int};
use crate::{FunctionValue, MemoizedFunctionValue, Mutability, StructValue};

impl Interpreter<'_> {
    /// Evaluate a canonical lambda: create a `FunctionValue` with canonical data.
    pub(super) fn eval_can_lambda(&mut self, params: CanParamRange, body: CanId) -> EvalResult {
        let canon = self.canon_ref();
        let can_params: SmallVec<[_; 8]> = SmallVec::from_slice(canon.arena.get_params(params));

        // Extract param names and defaults
        let names: Vec<Name> = can_params.iter().map(|p| p.name).collect();
        let defaults: Vec<Option<CanId>> = can_params
            .iter()
            .map(|p| {
                if p.default.is_valid() {
                    Some(p.default)
                } else {
                    Option::None
                }
            })
            .collect();

        let captures = self.env.capture();

        // Lambdas carry their SharedCanonResult for body evaluation.
        let Some(shared_canon) = self.canon.clone() else {
            return Err(
                EvalError::new("eval_can_lambda: canonical IR not available".to_string()).into(),
            );
        };

        // Carry the shared arena (O(1) Arc clone).
        let arena = self.imported_arena.clone();

        let mut func = FunctionValue::new(names, captures, arena);

        // Set canonical data so function calls dispatch via eval_can
        func.set_canon(body, shared_canon);

        // Set canonical defaults directly (no legacy ExprId conversion needed)
        if defaults.iter().any(Option::is_some) {
            func.set_can_defaults(defaults);
        }

        Ok(Value::Function(func))
    }

    /// Evaluate a canonical map literal: `{ k: v, ... }`.
    pub(super) fn eval_can_map(&mut self, can_id: CanId, entries: CanMapEntryRange) -> EvalResult {
        let span = self.can_span(can_id);
        let entry_list: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_map_entries(entries));
        let mut map = std::collections::BTreeMap::new();
        for entry in &entry_list {
            let key = self.eval_can(entry.key)?;
            let value = self.eval_can(entry.value)?;
            let key_str = key
                .to_map_key()
                .map_err(|_| Self::attach_span(map_key_not_hashable().into(), span))?;
            map.insert(key_str, value);
        }
        Ok(Value::map(map))
    }

    /// Evaluate a canonical struct literal: `Point { x: 0, y: 0 }`.
    pub(super) fn eval_can_struct(
        &mut self,
        name: Name,
        fields: ori_ir::canon::CanFieldRange,
    ) -> EvalResult {
        let field_list: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_fields(fields));
        let mut field_values: FxHashMap<Name, Value> = FxHashMap::default();
        field_values.reserve(field_list.len());
        for field in &field_list {
            let value = self.eval_can(field.value)?;
            field_values.insert(field.name, value);
        }
        Ok(Value::Struct(StructValue::new(name, field_values)))
    }

    /// Evaluate a canonical range: `start..end`, `start..=end`, `start..end by step`.
    ///
    /// Evaluates range bounds directly via `eval_can` — no `ExprId` roundtrip.
    pub(super) fn eval_can_range(
        &mut self,
        start: CanId,
        end: CanId,
        step: CanId,
        inclusive: bool,
    ) -> EvalResult {
        let start_val = if start.is_valid() {
            self.eval_can(start)?
                .as_int()
                .ok_or_else(|| ControlAction::from(range_bound_not_int("start")))?
        } else {
            0
        };
        let end_val = if end.is_valid() {
            Some(
                self.eval_can(end)?
                    .as_int()
                    .ok_or_else(|| ControlAction::from(range_bound_not_int("end")))?,
            )
        } else {
            None
        };
        let step_val = if step.is_valid() {
            self.eval_can(step)?
                .as_int()
                .ok_or_else(|| ControlAction::from(range_bound_not_int("step")))?
        } else {
            1
        };

        match end_val {
            Some(end) => {
                if inclusive {
                    Ok(Value::Range(RangeValue::inclusive_with_step(
                        start_val, end, step_val,
                    )))
                } else {
                    Ok(Value::Range(RangeValue::exclusive_with_step(
                        start_val, end, step_val,
                    )))
                }
            }
            None => Ok(Value::Range(RangeValue::unbounded_with_step(
                start_val, step_val,
            ))),
        }
    }

    /// Evaluate a canonical `FunctionExp` by pre-evaluating props and dispatching.
    ///
    /// In canonical IR, `FunctionExp` props are `CanNamedExpr` (name + `CanId`).
    /// We evaluate all props eagerly, then delegate to the existing pattern
    /// registry via the legacy `EvalContext` path by bridging the evaluated values.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive FunctionExpKind dispatch across all built-in function expression kinds"
    )]
    pub(super) fn eval_can_function_exp(
        &mut self,
        kind: FunctionExpKind,
        props: ori_ir::canon::CanNamedExprRange,
    ) -> EvalResult {
        // Catch and Recurse require lazy evaluation — their props must NOT
        // be pre-evaluated because evaluation order and error handling matter.
        match kind {
            FunctionExpKind::Catch => return self.eval_can_catch(props),
            FunctionExpKind::Recurse => return self.eval_can_recurse(props),
            _ => {}
        }

        // Pre-evaluate all props (safe for eager patterns like print, panic, etc.)
        let named: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_named_exprs(props));
        let mut values: Vec<(Name, Value)> = Vec::with_capacity(named.len());
        for ne in &named {
            let v = self.eval_can(ne.value)?;
            values.push((ne.name, v));
        }

        let pn = self.prop_names;

        // Dispatch by kind with pre-evaluated values
        match kind {
            FunctionExpKind::Print => {
                let msg = super::find_prop_value(&values, pn.msg, self.interner)?;
                self.print_handler.println(&msg.display_value());
                Ok(Value::Void)
            }
            FunctionExpKind::Panic => {
                let msg = super::find_prop_value(&values, pn.msg, self.interner)?;
                Err(EvalError::new(msg.display_value()).into())
            }
            FunctionExpKind::Todo => {
                let msg = values
                    .iter()
                    .find(|(n, _)| *n == pn.msg)
                    .map(|(_, v)| v.display_value());
                let text = match msg {
                    Some(m) => format!("not yet implemented: {m}"),
                    None => "not yet implemented".to_string(),
                };
                Err(EvalError::new(text).into())
            }
            FunctionExpKind::Unreachable => {
                Err(EvalError::new("reached unreachable code".to_string()).into())
            }
            // Catch and Recurse handled above via early return
            FunctionExpKind::Catch | FunctionExpKind::Recurse => unreachable!(),

            // Stub patterns — honest stubs that evaluate args via the canonical
            // path and emit tracing::warn! so they're impossible to miss in logs.
            // Real implementations are roadmap items.
            FunctionExpKind::Cache => {
                tracing::warn!(
                    "pattern 'cache' is a stub — operation is called without memoization"
                );
                let operation = super::find_prop_value(&values, pn.operation, self.interner)?;
                match operation {
                    Value::Function(_) | Value::FunctionVal(_, _) => {
                        self.eval_call(&operation, &[])
                    }
                    _ => Ok(operation),
                }
            }
            FunctionExpKind::Parallel => {
                tracing::warn!("pattern 'parallel' is a stub — tasks are executed sequentially");
                let tasks = super::find_prop_value(&values, pn.tasks, self.interner)?;
                let Value::List(task_list) = tasks else {
                    return Err(EvalError::new("parallel: tasks must be a list".to_string()).into());
                };
                let mut results = Vec::with_capacity(task_list.len());
                for task in task_list.iter() {
                    let result = match self.eval_call(task, &[]) {
                        Ok(v) => Value::ok(v),
                        Err(ControlAction::Error(e)) => {
                            Value::err(Value::string(e.message.clone()))
                        }
                        Err(e) => return Err(e),
                    };
                    results.push(result);
                }
                Ok(Value::list(results))
            }
            FunctionExpKind::Spawn => {
                tracing::warn!("pattern 'spawn' is a stub — tasks are executed synchronously");
                let tasks = super::find_prop_value(&values, pn.tasks, self.interner)?;
                let Value::List(task_list) = tasks else {
                    return Err(EvalError::new("spawn: tasks must be a list".to_string()).into());
                };
                for task in task_list.iter() {
                    let _ = self.eval_call(task, &[]);
                }
                Ok(Value::Void)
            }
            FunctionExpKind::Timeout => {
                tracing::warn!("pattern 'timeout' is a stub — no timeout enforcement");
                let operation = super::find_prop_value(&values, pn.operation, self.interner)?;
                Ok(Value::ok(operation))
            }
            FunctionExpKind::With => {
                tracing::warn!(
                    "pattern 'with' is a stub — resource management without type checker integration"
                );
                let resource = super::find_prop_value(&values, pn.acquire, self.interner)?;
                let action_fn = super::find_prop_value(&values, pn.action, self.interner)?;
                let result = self.eval_call(&action_fn, std::slice::from_ref(&resource));
                // Always call release if provided (RAII guarantee)
                if let Ok(release_fn) = super::find_prop_value(&values, pn.release, self.interner) {
                    let _ = self.eval_call(&release_fn, std::slice::from_ref(&resource));
                }
                result
            }
            FunctionExpKind::Channel
            | FunctionExpKind::ChannelIn
            | FunctionExpKind::ChannelOut
            | FunctionExpKind::ChannelAll => {
                tracing::warn!(
                    "pattern '{}' is a stub — channels are not yet implemented",
                    kind.name()
                );
                Ok(Value::Void)
            }
        }
    }

    /// Evaluate a `catch(expr: ...)` expression with lazy prop evaluation.
    fn eval_can_catch(&mut self, props: ori_ir::canon::CanNamedExprRange) -> EvalResult {
        let named: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_named_exprs(props));
        let expr_can_id = super::find_prop_can_id(&named, self.prop_names.expr, self.interner)?;

        match self.eval_can(expr_can_id) {
            Ok(v) => Ok(Value::ok(v)),
            Err(ControlAction::Error(e)) => Ok(Value::err(Value::string(e.message.clone()))),
            Err(e) => Err(e),
        }
    }

    /// Evaluate a `recurse(condition: ..., base: ..., step: ...)` expression.
    fn eval_can_recurse(&mut self, props: ori_ir::canon::CanNamedExprRange) -> EvalResult {
        let named: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_named_exprs(props));
        let pn = self.prop_names;

        let condition_id = super::find_prop_can_id(&named, pn.condition, self.interner)?;
        let base_id = super::find_prop_can_id(&named, pn.base, self.interner)?;
        let step_id = super::find_prop_can_id(&named, pn.step, self.interner)?;

        // Check optional memo prop
        let memo_id = named
            .iter()
            .find(|ne| ne.name == pn.memo)
            .map(|ne| ne.value);

        if let Some(mid) = memo_id {
            let memo_val = self.eval_can(mid)?;
            if memo_val.is_truthy() {
                // Wrap `self` in a memoized function for the step evaluation
                let self_name = self.self_name;
                if let Some(Value::Function(f)) = self.env.lookup(self_name) {
                    let memoized = Value::MemoizedFunction(MemoizedFunctionValue::new(f));
                    return self.with_binding(
                        self_name,
                        memoized,
                        Mutability::Immutable,
                        |scoped| scoped.eval_can_recurse_body(condition_id, base_id, step_id),
                    );
                }
            }
        }

        self.eval_can_recurse_body(condition_id, base_id, step_id)
    }

    /// Evaluate the condition/base/step of a recurse pattern.
    fn eval_can_recurse_body(
        &mut self,
        condition_id: CanId,
        base_id: CanId,
        step_id: CanId,
    ) -> EvalResult {
        let cond_val = self.eval_can(condition_id)?;
        if cond_val.is_truthy() {
            self.eval_can(base_id)
        } else {
            self.eval_can(step_id)
        }
    }
}

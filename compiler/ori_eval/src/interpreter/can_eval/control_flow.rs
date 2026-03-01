//! Control flow evaluation: blocks, bindings, loops, match, assignment.

use ori_ir::canon::{CanBindingPattern, CanBindingPatternId, CanExpr, CanId, CanRange};
use ori_patterns::{ControlAction, EvalError, EvalResult, IteratorValue, Value};
use smallvec::SmallVec;

use super::super::Interpreter;
use crate::Mutability;

impl Interpreter<'_> {
    /// Evaluate a canonical block: `{ stmts; result }`.
    pub(super) fn eval_can_block(&mut self, stmts: CanRange, result: CanId) -> EvalResult {
        let mut scoped = self.scoped();

        // Evaluate each statement. In canonical IR, block statements are just
        // expressions (Let bindings are expressions that return Void).
        let stmt_ids: SmallVec<[CanId; 8]> =
            SmallVec::from_slice(scoped.canon_ref().arena.get_expr_list(stmts));
        for stmt_id in stmt_ids {
            scoped.eval_can(stmt_id)?;
        }

        if result.is_valid() {
            scoped.eval_can(result)
        } else {
            Ok(Value::Void)
        }
    }

    /// Bind a canonical binding pattern to a value.
    pub(super) fn bind_can_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        value: Value,
    ) -> EvalResult {
        match pattern {
            CanBindingPattern::Name { name, mutable } => {
                // Per-binding mutability: use the flag from the pattern itself,
                // not the inherited top-level mutability. This enables `let ($x, y) = ...`
                // where `x` is immutable and `y` is mutable.
                self.env.define(*name, value, *mutable);
                Ok(Value::Void)
            }
            CanBindingPattern::Wildcard => Ok(Value::Void),
            CanBindingPattern::Tuple(range) => {
                if let Value::Tuple(values) = value {
                    let pat_ids: SmallVec<[_; 8]> = SmallVec::from_slice(
                        self.canon_ref().arena.get_binding_pattern_list(*range),
                    );
                    if pat_ids.len() != values.len() {
                        return Err(crate::errors::tuple_pattern_mismatch().into());
                    }
                    // Copy elision: when the tuple has refcount 1 (e.g., freshly
                    // created by iter.next()), move elements out instead of cloning.
                    match values.try_into_inner() {
                        Ok(owned) => {
                            for (pat_id, val) in pat_ids.into_iter().zip(owned) {
                                let sub_pat = *self.canon_ref().arena.get_binding_pattern(pat_id);
                                self.bind_can_pattern(&sub_pat, val)?;
                            }
                        }
                        Err(shared) => {
                            for (pat_id, val) in pat_ids.into_iter().zip(shared.iter()) {
                                let sub_pat = *self.canon_ref().arena.get_binding_pattern(pat_id);
                                self.bind_can_pattern(&sub_pat, val.clone())?;
                            }
                        }
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::errors::expected_tuple().into())
                }
            }
            CanBindingPattern::Struct { fields } => {
                if let Value::Struct(s) = value {
                    let field_bindings: SmallVec<[_; 8]> =
                        SmallVec::from_slice(self.canon_ref().arena.get_field_bindings(*fields));
                    for fb in &field_bindings {
                        if let Some(val) = s.get_field(fb.name) {
                            // Copy the sub-pattern out to avoid borrow conflict
                            let sub_pat = *self.canon_ref().arena.get_binding_pattern(fb.pattern);
                            self.bind_can_pattern(&sub_pat, val.clone())?;
                        } else {
                            return Err(crate::errors::missing_struct_field().into());
                        }
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::errors::expected_struct().into())
                }
            }
            CanBindingPattern::List { elements, rest } => {
                if let Value::List(values) = value {
                    let pat_ids: SmallVec<[_; 8]> = SmallVec::from_slice(
                        self.canon_ref().arena.get_binding_pattern_list(*elements),
                    );
                    if values.len() < pat_ids.len() {
                        return Err(crate::errors::list_pattern_too_long().into());
                    }
                    for (pat_id, val) in pat_ids.iter().zip(values.iter()) {
                        // Copy the sub-pattern out to avoid borrow conflict
                        let sub_pat = *self.canon_ref().arena.get_binding_pattern(*pat_id);
                        self.bind_can_pattern(&sub_pat, val.clone())?;
                    }
                    if let Some((rest_name, rest_mut)) = rest {
                        let rest_list = values.skip(pat_ids.len());
                        self.env
                            .define(*rest_name, Value::List(rest_list), *rest_mut);
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::errors::expected_list().into())
                }
            }
        }
    }

    /// Evaluate a canonical assignment: `target = value`.
    pub(super) fn eval_can_assign(&mut self, target: CanId, value: Value) -> EvalResult {
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(target);
        match kind {
            CanExpr::Ident(name) => {
                self.env.assign(name, value.clone()).map_err(|e| {
                    let name_str = self.interner.lookup(name);
                    ControlAction::from(match e {
                        crate::AssignError::Immutable => {
                            crate::errors::cannot_assign_immutable(name_str)
                        }
                        crate::AssignError::Undefined => {
                            crate::errors::undefined_variable(name_str)
                        }
                    })
                })?;
                Ok(value)
            }
            CanExpr::Index { .. } => Err(crate::errors::index_assignment_not_supported().into()),
            CanExpr::Field { .. } => Err(crate::errors::field_assignment_not_implemented().into()),
            _ => Err(crate::errors::invalid_assignment_target().into()),
        }
    }

    /// Evaluate a canonical match expression via decision tree.
    pub(super) fn eval_can_match(
        &mut self,
        value: &Value,
        decision_tree_id: ori_ir::canon::DecisionTreeId,
        arms: CanRange,
    ) -> EvalResult {
        self.mode_state.count_pattern_match();
        // Single borrow: extract both the decision tree (O(1) Arc clone) and arm IDs
        // before releasing the borrow on self.canon for the guard callback's &mut self.
        let (tree, arm_ids) = {
            let canon = self.canon_ref();
            let tree = canon.decision_trees.get_shared(decision_tree_id);
            let arm_ids: SmallVec<[CanId; 8]> =
                SmallVec::from_slice(canon.arena.get_expr_list(arms));
            (tree, arm_ids)
        };

        // Walk the decision tree with a guard callback that evaluates via eval_can.
        let result = crate::exec::decision_tree::eval_decision_tree(
            &tree,
            value,
            self.interner,
            &mut |guard_id, bindings| {
                // Bind guard variables in a RAII-guarded scope
                let guard_result = {
                    let mut scoped = self.scoped();
                    for (name, val) in bindings {
                        scoped.env.define(*name, val.clone(), Mutability::Immutable);
                    }
                    scoped.eval_can(guard_id)
                };

                match guard_result {
                    Ok(Value::Bool(b)) => Ok(b),
                    Ok(_) => Err(EvalError::new(
                        "guard expression must return bool".to_string(),
                    )),
                    Err(ControlAction::Error(e)) => Err(*e),
                    Err(_) => Err(EvalError::new(
                        "control flow in guard expression".to_string(),
                    )),
                }
            },
        );

        match result {
            Ok(match_result) => {
                // Bind matched variables and evaluate the arm body in a RAII-guarded scope
                let arm_id = arm_ids
                    .get(match_result.arm_index)
                    .copied()
                    .ok_or_else(crate::errors::non_exhaustive_match)?;

                self.with_match_bindings(match_result.bindings, |scoped| scoped.eval_can(arm_id))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Evaluate a canonical for loop via the functional iterator protocol.
    ///
    /// Converts the iterable value to an `IteratorValue` and advances it
    /// through `eval_iter_next()`, threading the immutable iterator state
    /// through each step.
    pub(super) fn eval_can_for(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: &Value,
        guard: CanId,
        body: CanId,
        is_yield: bool,
    ) -> EvalResult {
        use crate::exec::control::{to_loop_action, LoopAction};

        // Convert to functional iterator via Iterable trait
        let mut current_iter = IteratorValue::from_value(iter_val)
            .ok_or_else(|| ControlAction::from(crate::errors::for_requires_iterable()))?;

        // Load the pattern once (it's Copy-sized enum with arena indices)
        let pat = *self.canon_ref().arena.get_binding_pattern(pattern);

        if is_yield {
            // for...yield: collect results into list
            let (lower, _) = current_iter.size_hint();
            let mut results = Vec::with_capacity(lower);
            loop {
                let (item, new_iter) = self.eval_iter_next(current_iter)?;
                current_iter = new_iter;
                let Some(val) = item else { break };

                let mut scoped = self.scoped();
                scoped.bind_can_pattern(&pat, val)?;

                // Check guard
                if guard.is_valid() {
                    match scoped.eval_can(guard) {
                        Ok(v) if !v.is_truthy() => continue,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }

                match scoped.eval_can(body) {
                    Ok(v) => results.push(v),
                    Err(e) => match to_loop_action(e) {
                        LoopAction::Continue => {}
                        LoopAction::ContinueWith(v) => results.push(v),
                        LoopAction::Break(v) => {
                            if !matches!(v, Value::Void) {
                                results.push(v);
                            }
                            return Ok(Value::list(results));
                        }
                        LoopAction::Error(e) => return Err(e),
                    },
                }
            }
            Ok(Value::list(results))
        } else {
            // for...do: iterate for side effects
            loop {
                let (item, new_iter) = self.eval_iter_next(current_iter)?;
                current_iter = new_iter;
                let Some(val) = item else { break };

                let mut scoped = self.scoped();
                scoped.bind_can_pattern(&pat, val)?;

                // Check guard
                if guard.is_valid() {
                    match scoped.eval_can(guard) {
                        Ok(v) if !v.is_truthy() => continue,
                        Err(e) => return Err(e),
                        _ => {}
                    }
                }

                match scoped.eval_can(body) {
                    Ok(_) => {}
                    Err(e) => match to_loop_action(e) {
                        LoopAction::Continue | LoopAction::ContinueWith(_) => {}
                        LoopAction::Break(v) => return Ok(v),
                        LoopAction::Error(e) => return Err(e),
                    },
                }
            }
            Ok(Value::Void)
        }
    }

    /// Evaluate a canonical infinite loop.
    pub(super) fn eval_can_loop(&mut self, body: CanId) -> EvalResult {
        use crate::exec::control::{to_loop_action, LoopAction};

        loop {
            match self.eval_can(body) {
                Ok(_) => {}
                Err(e) => match to_loop_action(e) {
                    LoopAction::Continue | LoopAction::ContinueWith(_) => {}
                    LoopAction::Break(v) => return Ok(v),
                    LoopAction::Error(e) => return Err(e),
                },
            }
        }
    }
}

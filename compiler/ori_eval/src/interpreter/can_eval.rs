//! Canonical expression evaluation — `CanExpr` dispatch.
//!
//! This module provides `eval_can(CanId)` as a parallel evaluation path alongside
//! the legacy `eval(ExprId)`. Functions with canonical bodies (`has_canon()`) dispatch
//! here; functions without fall back to the legacy path.
//!
//! # Architecture
//!
//! `eval_can` reads from `self.canon` (`SharedCanonResult`) instead of `self.arena`
//! (`ExprArena`). Since `CanExpr` is sugar-free, there are no spread/template/named-call
//! variants to handle — those are desugared during canonicalization.
//!
//! # Borrow Pattern
//!
//! `CanExpr` is `Copy` (24 bytes), so we copy the kind out of the arena before
//! dispatching. This releases the immutable borrow on `self.canon`, allowing
//! recursive `self.eval_can()` calls in each arm.

use ori_ir::canon::{
    CanBindingPattern, CanExpr, CanId, CanMapEntryRange, CanParamRange, CanRange, CanonResult,
};
use ori_ir::{BinaryOp, FunctionExpKind, Name, Span, UnaryOp};
use ori_patterns::{ControlAction, EvalError, EvalResult, Value};
use ori_stack::ensure_sufficient_stack;
use rustc_hash::FxHashMap;

use super::Interpreter;
use crate::exec::expr;
use crate::{
    await_not_supported, evaluate_binary, evaluate_unary, hash_outside_index, map_key_not_hashable,
    non_exhaustive_match, parse_error, self_outside_method, undefined_const, undefined_function,
    FunctionValue, MemoizedFunctionValue, Mutability, StructValue,
};

impl Interpreter<'_> {
    /// Entry point for canonical expression evaluation with stack safety.
    ///
    /// Analogous to `eval(ExprId)` but dispatches on `CanExpr` variants from
    /// the canonical IR. Requires `self.canon` to be `Some`.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn eval_can(&mut self, can_id: CanId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_can_inner(can_id))
    }

    /// Get the canonical result reference.
    ///
    /// # Panics
    /// Panics if `self.canon` is `None`. Callers must ensure canonical IR is set
    /// before calling `eval_can`.
    #[inline]
    fn canon_ref(&self) -> &CanonResult {
        // SAFETY: eval_can is only called when canon is known to be Some.
        // This is enforced by the call sites (function_call checks has_canon()).
        #[expect(clippy::expect_used, reason = "Invariant: eval_can requires canon")]
        self.canon
            .as_ref()
            .expect("eval_can called without canonical IR")
    }

    /// Get the span for a canonical expression.
    #[inline]
    fn can_span(&self, can_id: CanId) -> Span {
        self.canon_ref().arena.span(can_id)
    }

    /// Evaluate a list of canonical expressions from a `CanRange`.
    fn eval_can_expr_list(&mut self, range: CanRange) -> Result<Vec<Value>, ControlAction> {
        let ids: Vec<CanId> = self.canon_ref().arena.get_expr_list(range).to_vec();
        ids.into_iter().map(|id| self.eval_can(id)).collect()
    }

    /// Inner canonical evaluation dispatch.
    ///
    /// Handles all 44 `CanExpr` variants exhaustively. No `_ =>` catch-all.
    fn eval_can_inner(&mut self, can_id: CanId) -> EvalResult {
        self.mode_state.count_expression();

        // Copy the kind out to release the borrow on self.canon.
        // CanExpr is Copy (24 bytes) — this is cheap.
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(can_id);

        match kind {
            // === Literals ===
            CanExpr::Int(n) => Ok(Value::int(n)),
            CanExpr::Float(bits) => Ok(Value::Float(f64::from_bits(bits))),
            CanExpr::Bool(b) => Ok(Value::Bool(b)),
            CanExpr::Str(name) => Ok(Value::string_static(self.interner.lookup_static(name))),
            CanExpr::Char(c) => Ok(Value::Char(c)),
            CanExpr::Duration { value, unit } => Ok(Value::Duration(unit.to_nanos(value))),
            CanExpr::Size { value, unit } => Ok(Value::Size(unit.to_bytes(value))),
            CanExpr::Unit => Ok(Value::Void),

            // === Compile-Time Constant ===
            CanExpr::Constant(id) => {
                let cv = self.canon_ref().constants.get(id);
                Ok(const_to_value(cv, self.interner))
            }

            // === References ===
            CanExpr::Ident(name) => expr::eval_ident(
                name,
                &self.env,
                self.interner,
                Some(&self.user_method_registry.read()),
            ),
            CanExpr::Const(name) => self
                .env
                .lookup(name)
                .ok_or_else(|| undefined_const(self.interner.lookup(name)).into()),
            CanExpr::SelfRef => self
                .env
                .lookup(self.self_name)
                .ok_or_else(|| self_outside_method().into()),
            CanExpr::FunctionRef(name) => self
                .env
                .lookup(name)
                .ok_or_else(|| undefined_function(self.interner.lookup(name)).into()),
            CanExpr::HashLength => Err(hash_outside_index().into()),

            // === Operators ===
            CanExpr::Binary { op, left, right } => self.eval_can_binary(can_id, left, op, right),
            CanExpr::Unary { op, operand } => self.eval_can_unary(can_id, op, operand),
            CanExpr::Cast {
                expr,
                target,
                fallible,
            } => {
                let value = self.eval_can(expr)?;
                self.eval_can_cast(value, target, fallible)
            }

            // === Calls ===
            CanExpr::Call { func, args } => {
                let func_val = self.eval_can(func)?;
                let arg_vals = self.eval_can_expr_list(args)?;
                self.eval_call(&func_val, &arg_vals)
            }
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.eval_can(receiver)?;
                let arg_vals = self.eval_can_expr_list(args)?;
                self.dispatch_method_call(recv, method, arg_vals)
            }

            // === Access ===
            CanExpr::Field { receiver, field } => {
                let value = self.eval_can(receiver)?;
                expr::eval_field_access(value, field, self.interner)
            }
            CanExpr::Index { receiver, index } => {
                let value = self.eval_can(receiver)?;
                let length = expr::get_collection_length(&value)?;
                let idx = self.eval_can_with_hash_length(index, length)?;
                expr::eval_index(value, idx)
            }

            // === Control Flow ===
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                if self.eval_can(cond)?.is_truthy() {
                    self.eval_can(then_branch)
                } else if else_branch.is_valid() {
                    self.eval_can(else_branch)
                } else {
                    Ok(Value::Void)
                }
            }
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => {
                let value = self.eval_can(scrutinee)?;
                self.eval_can_match(&value, decision_tree, arms)
            }
            CanExpr::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                let iter_val = self.eval_can(iter)?;
                self.eval_can_for(binding, &iter_val, guard, body, is_yield)
            }
            CanExpr::Loop { body } => self.eval_can_loop(body),
            CanExpr::Break(v) => {
                let val = if v.is_valid() {
                    self.eval_can(v)?
                } else {
                    Value::Void
                };
                Err(ControlAction::Break(val))
            }
            CanExpr::Continue(v) => {
                let val = if v.is_valid() {
                    self.eval_can(v)?
                } else {
                    Value::Void
                };
                Err(ControlAction::Continue(val))
            }

            // === Bindings ===
            CanExpr::Block { stmts, result } => self.eval_can_block(stmts, result),
            CanExpr::Let {
                pattern,
                init,
                mutable,
            } => {
                let value = self.eval_can(init)?;
                let mutability = if mutable {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                // Copy the pattern out to avoid borrow conflict
                let pat = *self.canon_ref().arena.get_binding_pattern(pattern);
                self.bind_can_pattern(&pat, value, mutability)
            }
            CanExpr::Assign { target, value } => {
                let val = self.eval_can(value)?;
                self.eval_can_assign(target, val)
            }

            // === Functions ===
            CanExpr::Lambda { params, body } => self.eval_can_lambda(params, body),

            // === Collections ===
            CanExpr::List(range) => Ok(Value::list(self.eval_can_expr_list(range)?)),
            CanExpr::Tuple(range) => Ok(Value::tuple(self.eval_can_expr_list(range)?)),
            CanExpr::Map(entries) => self.eval_can_map(entries),
            CanExpr::Struct { name, fields } => self.eval_can_struct(name, fields),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.eval_can_range(start, end, step, inclusive),

            // === Algebraic ===
            CanExpr::Ok(inner) => Ok(Value::ok(if inner.is_valid() {
                self.eval_can(inner)?
            } else {
                Value::Void
            })),
            CanExpr::Err(inner) => Ok(Value::err(if inner.is_valid() {
                self.eval_can(inner)?
            } else {
                Value::Void
            })),
            CanExpr::Some(inner) => Ok(Value::some(self.eval_can(inner)?)),
            CanExpr::None => Ok(Value::None),

            // === Error Handling ===
            CanExpr::Try(inner) => match self.eval_can(inner)? {
                Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
                Value::Err(e) => Err(ControlAction::Propagate(Value::Err(e))),
                Value::None => Err(ControlAction::Propagate(Value::None)),
                other => Ok(other),
            },
            CanExpr::Await(_) => Err(await_not_supported().into()),

            // === Capabilities ===
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider_val = self.eval_can(provider)?;
                self.env.push_scope();
                self.env
                    .define(capability, provider_val, Mutability::Immutable);
                let result = self.eval_can(body);
                self.env.pop_scope();
                result
            }

            // === Special Forms ===
            CanExpr::FunctionExp { kind, props } => self.eval_can_function_exp(kind, props),

            // === Error Recovery ===
            CanExpr::Error => Err(parse_error().into()),
        }
    }

    // ── Binary Operators ────────────────────────────────────────────

    /// Evaluate a canonical binary operation with short-circuit support.
    fn eval_can_binary(
        &mut self,
        binary_id: CanId,
        left: CanId,
        op: BinaryOp,
        right: CanId,
    ) -> EvalResult {
        let left_val = self.eval_can(left)?;
        let span = self.can_span(binary_id);

        // Short-circuit for &&, ||, ??
        match op {
            BinaryOp::And => {
                if !left_val.is_truthy() {
                    return Ok(Value::Bool(false));
                }
                let right_val = self.eval_can(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            BinaryOp::Or => {
                if left_val.is_truthy() {
                    return Ok(Value::Bool(true));
                }
                let right_val = self.eval_can(right)?;
                return Ok(Value::Bool(right_val.is_truthy()));
            }
            BinaryOp::Coalesce => {
                // In canonical mode, we compare TypeIds directly (always available).
                let canon = self.canon_ref();
                let is_chaining = canon.arena.ty(left) == canon.arena.ty(binary_id);

                match left_val {
                    Value::Some(inner) => {
                        if is_chaining {
                            return Ok(Value::Some(inner));
                        }
                        return Ok((*inner).clone());
                    }
                    Value::Ok(inner) => {
                        if is_chaining {
                            return Ok(Value::Ok(inner));
                        }
                        return Ok((*inner).clone());
                    }
                    Value::None | Value::Err(_) => {
                        return self.eval_can(right);
                    }
                    _ => {
                        let err: ControlAction = EvalError::new(format!(
                            "operator '??' requires Option or Result, got {}",
                            left_val.type_name()
                        ))
                        .into();
                        return Err(Self::attach_span(err, span));
                    }
                }
            }
            _ => {}
        }

        let right_val = self.eval_can(right)?;

        // Primitive types use direct evaluation
        if super::is_primitive_value(&left_val) && super::is_primitive_value(&right_val) {
            return evaluate_binary(left_val, right_val, op)
                .map_err(|e| Self::attach_span(e, span));
        }

        // User-defined types: dispatch through method system
        if let Some(method_name) = super::binary_op_to_method(op) {
            let method = self.interner.intern(method_name);
            return self.eval_method_call(left_val, method, vec![right_val]);
        }

        evaluate_binary(left_val, right_val, op).map_err(|e| Self::attach_span(e, span))
    }

    // ── Unary Operators ────────────────────────────────────────────

    /// Evaluate a canonical unary operation.
    fn eval_can_unary(&mut self, expr_id: CanId, op: UnaryOp, operand: CanId) -> EvalResult {
        let value = self.eval_can(operand)?;
        let span = self.can_span(expr_id);

        if super::is_primitive_value(&value) {
            return evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span));
        }

        if let Some(method_name) = super::unary_op_to_method(op) {
            let method = self.interner.intern(method_name);
            return self.eval_method_call(value, method, vec![]);
        }

        evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span))
    }

    // ── Type Cast ──────────────────────────────────────────────────

    /// Evaluate a canonical type cast using the target type name.
    ///
    /// In canonical IR, the target is stored as an interned `Name` instead of
    /// `ParsedTypeId`. We look up the string to dispatch.
    fn eval_can_cast(&self, value: Value, target: Name, fallible: bool) -> EvalResult {
        let target_name = self.interner.lookup(target);
        let result = match (target_name, &value) {
            // int conversions
            #[allow(clippy::cast_precision_loss)]
            ("float", Value::Int(n)) => Ok(Value::Float(n.raw() as f64)),
            ("byte", Value::Int(n)) => {
                let raw = n.raw();
                if !(0..=255).contains(&raw) {
                    if fallible {
                        return Ok(Value::None);
                    }
                    return Err(EvalError::new(format!(
                        "value {raw} out of range for byte (0-255)"
                    ))
                    .into());
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Ok(Value::Byte(raw as u8))
            }
            ("char", Value::Int(n)) => {
                let raw = n.raw();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                if let Some(c) = char::from_u32(raw as u32) {
                    Ok(Value::Char(c))
                } else if fallible {
                    return Ok(Value::None);
                } else {
                    return Err(EvalError::new(format!(
                        "value {raw} is not a valid Unicode codepoint"
                    ))
                    .into());
                }
            }
            ("int", Value::Byte(b)) => Ok(Value::int(i64::from(*b))),
            ("int", Value::Char(c)) => Ok(Value::int(i64::from(*c as u32))),
            #[allow(clippy::cast_possible_truncation)]
            ("int", Value::Float(f)) => Ok(Value::int(*f as i64)),
            ("int", Value::Str(s)) => match s.parse::<i64>() {
                Ok(n) => Ok(Value::int(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as int")).into());
                }
            },
            ("float", Value::Str(s)) => match s.parse::<f64>() {
                Ok(n) => Ok(Value::Float(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as float")).into());
                }
            },
            // Identity conversions
            ("int", Value::Int(_))
            | ("float", Value::Float(_))
            | ("str", Value::Str(_))
            | ("bool", Value::Bool(_))
            | ("byte", Value::Byte(_))
            | ("char", Value::Char(_)) => Ok(value),
            // str conversion - anything can become a string
            ("str", v) => Ok(Value::string(v.to_string())),
            _ => {
                if fallible {
                    return Ok(Value::None);
                }
                Err(EvalError::new(format!(
                    "cannot convert {} to {target_name}",
                    value.type_name()
                ))
                .into())
            }
        };
        if fallible {
            result.map(Value::some)
        } else {
            result
        }
    }

    // ── Block ──────────────────────────────────────────────────────

    /// Evaluate a canonical block: `{ stmts; result }`.
    fn eval_can_block(&mut self, stmts: CanRange, result: CanId) -> EvalResult {
        self.env.push_scope();

        // Evaluate each statement. In canonical IR, block statements are just
        // expressions (Let bindings are expressions that return Void).
        let stmt_ids: Vec<CanId> = self.canon_ref().arena.get_expr_list(stmts).to_vec();
        for stmt_id in stmt_ids {
            let res = self.eval_can(stmt_id);
            if let Err(e) = res {
                self.env.pop_scope();
                return Err(e);
            }
        }

        let r = if result.is_valid() {
            self.eval_can(result)
        } else {
            Ok(Value::Void)
        };
        self.env.pop_scope();
        r
    }

    // ── Binding Pattern ────────────────────────────────────────────

    /// Bind a canonical binding pattern to a value.
    fn bind_can_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        value: Value,
        mutability: Mutability,
    ) -> EvalResult {
        match pattern {
            CanBindingPattern::Name(name) => {
                self.env.define(*name, value, mutability);
                Ok(Value::Void)
            }
            CanBindingPattern::Wildcard => Ok(Value::Void),
            CanBindingPattern::Tuple(range) => {
                if let Value::Tuple(values) = value {
                    let pat_ids: Vec<_> = self
                        .canon_ref()
                        .arena
                        .get_binding_pattern_list(*range)
                        .to_vec();
                    if pat_ids.len() != values.len() {
                        return Err(crate::tuple_pattern_mismatch().into());
                    }
                    for (pat_id, val) in pat_ids.into_iter().zip(values.iter()) {
                        // Copy the sub-pattern out to avoid borrow conflict
                        let sub_pat = *self.canon_ref().arena.get_binding_pattern(pat_id);
                        self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::expected_tuple().into())
                }
            }
            CanBindingPattern::Struct { fields } => {
                if let Value::Struct(s) = value {
                    let field_bindings: Vec<_> =
                        self.canon_ref().arena.get_field_bindings(*fields).to_vec();
                    for fb in &field_bindings {
                        if let Some(val) = s.get_field(fb.name) {
                            // Copy the sub-pattern out to avoid borrow conflict
                            let sub_pat = *self.canon_ref().arena.get_binding_pattern(fb.pattern);
                            self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
                        } else {
                            return Err(crate::missing_struct_field().into());
                        }
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::expected_struct().into())
                }
            }
            CanBindingPattern::List { elements, rest } => {
                if let Value::List(values) = value {
                    let pat_ids: Vec<_> = self
                        .canon_ref()
                        .arena
                        .get_binding_pattern_list(*elements)
                        .to_vec();
                    if values.len() < pat_ids.len() {
                        return Err(crate::list_pattern_too_long().into());
                    }
                    for (pat_id, val) in pat_ids.iter().zip(values.iter()) {
                        // Copy the sub-pattern out to avoid borrow conflict
                        let sub_pat = *self.canon_ref().arena.get_binding_pattern(*pat_id);
                        self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
                    }
                    if let Some(rest_name) = rest {
                        let rest_values = values[pat_ids.len()..].to_vec();
                        self.env
                            .define(*rest_name, Value::list(rest_values), mutability);
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::expected_list().into())
                }
            }
        }
    }

    // ── Assignment ────────────────────────────────────────────────

    /// Evaluate a canonical assignment: `target = value`.
    fn eval_can_assign(&mut self, target: CanId, value: Value) -> EvalResult {
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(target);
        match kind {
            CanExpr::Ident(name) => {
                self.env.assign(name, value.clone()).map_err(|_| {
                    let name_str = self.interner.lookup(name);
                    ControlAction::from(crate::cannot_assign_immutable(name_str))
                })?;
                Ok(value)
            }
            CanExpr::Index { .. } => Err(crate::index_assignment_not_implemented().into()),
            CanExpr::Field { .. } => Err(crate::field_assignment_not_implemented().into()),
            _ => Err(crate::invalid_assignment_target().into()),
        }
    }

    // ── Lambda ─────────────────────────────────────────────────────

    /// Evaluate a canonical lambda: create a `FunctionValue` with canonical data.
    fn eval_can_lambda(&mut self, params: CanParamRange, body: CanId) -> EvalResult {
        let canon = self.canon_ref();
        let can_params: Vec<_> = canon.arena.get_params(params).to_vec();

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

        // Also carry the legacy arena (still needed for methods/patterns that use ExprId).
        let arena = match &self.imported_arena {
            Some(a) => a.clone(),
            None => ori_ir::SharedArena::new(self.arena.clone()),
        };

        let mut func = FunctionValue::new(names, body.to_expr_id(), captures, arena);

        // Set canonical data so function calls dispatch via eval_can
        func.set_canon(body, shared_canon);

        // Set canonical defaults directly (no legacy ExprId conversion needed)
        if defaults.iter().any(Option::is_some) {
            func.set_can_defaults(defaults);
        }

        Ok(Value::Function(func))
    }

    // ── Map ────────────────────────────────────────────────────────

    /// Evaluate a canonical map literal: `{ k: v, ... }`.
    fn eval_can_map(&mut self, entries: CanMapEntryRange) -> EvalResult {
        let entry_list: Vec<_> = self.canon_ref().arena.get_map_entries(entries).to_vec();
        let mut map = std::collections::BTreeMap::new();
        for entry in &entry_list {
            let key = self.eval_can(entry.key)?;
            let value = self.eval_can(entry.value)?;
            let key_str = key.to_map_key().map_err(|_| map_key_not_hashable())?;
            map.insert(key_str, value);
        }
        Ok(Value::map(map))
    }

    // ── Struct ─────────────────────────────────────────────────────

    /// Evaluate a canonical struct literal: `Point { x: 0, y: 0 }`.
    fn eval_can_struct(&mut self, name: Name, fields: ori_ir::canon::CanFieldRange) -> EvalResult {
        let field_list: Vec<_> = self.canon_ref().arena.get_fields(fields).to_vec();
        let mut field_values: FxHashMap<Name, Value> = FxHashMap::default();
        field_values.reserve(field_list.len());
        for field in &field_list {
            let value = self.eval_can(field.value)?;
            field_values.insert(field.name, value);
        }
        Ok(Value::Struct(StructValue::new(name, field_values)))
    }

    // ── Range ──────────────────────────────────────────────────────

    /// Evaluate a canonical range: `start..end`, `start..=end`, `start..end by step`.
    fn eval_can_range(
        &mut self,
        start: CanId,
        end: CanId,
        step: CanId,
        inclusive: bool,
    ) -> EvalResult {
        expr::eval_range(
            if start.is_valid() {
                start.to_expr_id()
            } else {
                ori_ir::ExprId::INVALID
            },
            if end.is_valid() {
                end.to_expr_id()
            } else {
                ori_ir::ExprId::INVALID
            },
            if step.is_valid() {
                step.to_expr_id()
            } else {
                ori_ir::ExprId::INVALID
            },
            inclusive,
            |eid| {
                // Bridge: convert ExprId back to CanId and evaluate canonically.
                // This works because eval_range uses INVALID checks on ExprId,
                // and we only pass valid ExprIds that map 1:1 to our CanIds.
                // The eval_range function just calls eval_fn(expr_id) on start/end/step.
                // We stored CanId.to_expr_id() above, so convert back here.
                let cid = ori_ir::canon::CanId::from_expr_id(eid);
                self.eval_can(cid)
            },
        )
    }

    // ── Match (Decision Tree) ─────────────────────────────────────

    /// Evaluate a canonical match expression via decision tree.
    fn eval_can_match(
        &mut self,
        value: &Value,
        decision_tree_id: ori_ir::canon::DecisionTreeId,
        arms: CanRange,
    ) -> EvalResult {
        let tree = self
            .canon_ref()
            .decision_trees
            .get(decision_tree_id)
            .clone();
        let arm_ids: Vec<CanId> = self.canon_ref().arena.get_expr_list(arms).to_vec();

        // Walk the decision tree with a guard callback that evaluates via eval_can.
        let result = crate::exec::decision_tree::eval_decision_tree(
            &tree,
            value,
            self.interner,
            &mut |guard_id, bindings| {
                // Bind guard variables in a temporary scope
                self.env.push_scope();
                for (name, val) in bindings {
                    self.env.define(*name, val.clone(), Mutability::Immutable);
                }
                let guard_result = self.eval_can(guard_id);
                self.env.pop_scope();

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
                // Bind matched variables and evaluate the arm body
                let arm_id = arm_ids
                    .get(match_result.arm_index)
                    .copied()
                    .ok_or_else(non_exhaustive_match)?;

                self.env.push_scope();
                for (name, val) in &match_result.bindings {
                    self.env.define(*name, val.clone(), Mutability::Immutable);
                }
                let body_result = self.eval_can(arm_id);
                self.env.pop_scope();
                body_result
            }
            Err(e) => Err(e.into()),
        }
    }

    // ── For Loop ──────────────────────────────────────────────────

    /// Evaluate a canonical for loop.
    fn eval_can_for(
        &mut self,
        binding: Name,
        iter_val: &Value,
        guard: CanId,
        body: CanId,
        is_yield: bool,
    ) -> EvalResult {
        use crate::exec::control::{to_loop_action, LoopAction};

        // Build an iterator from the value
        let items: Vec<Value> = match iter_val {
            Value::List(items) => items.to_vec(),
            Value::Map(map) => map
                .iter()
                .map(|(k, v)| Value::tuple(vec![Value::string(k.clone()), v.clone()]))
                .collect(),
            Value::Range(range) => range.iter().map(Value::int).collect(),
            Value::Str(s) => s.chars().map(Value::Char).collect(),
            _ => {
                return Err(crate::for_requires_iterable().into());
            }
        };

        if is_yield {
            // for...yield collects results into a list
            let mut results = Vec::with_capacity(items.len());
            for item in items {
                self.env.push_scope();
                self.env.define(binding, item, Mutability::Immutable);

                // Check guard
                if guard.is_valid() {
                    let guard_val = self.eval_can(guard);
                    match guard_val {
                        Ok(v) if !v.is_truthy() => {
                            self.env.pop_scope();
                            continue;
                        }
                        Err(e) => {
                            self.env.pop_scope();
                            return Err(e);
                        }
                        _ => {}
                    }
                }

                match self.eval_can(body) {
                    Ok(v) => results.push(v),
                    Err(e) => {
                        self.env.pop_scope();
                        match to_loop_action(e) {
                            LoopAction::Continue => continue,
                            LoopAction::ContinueWith(v) => {
                                results.push(v);
                                continue;
                            }
                            LoopAction::Break(v) => {
                                if !matches!(v, Value::Void) {
                                    results.push(v);
                                }
                                return Ok(Value::list(results));
                            }
                            LoopAction::Error(e) => return Err(e),
                        }
                    }
                }
                self.env.pop_scope();
            }
            Ok(Value::list(results))
        } else {
            // Regular for loop returns Void
            for item in items {
                self.env.push_scope();
                self.env.define(binding, item, Mutability::Immutable);

                // Check guard
                if guard.is_valid() {
                    let guard_val = self.eval_can(guard);
                    match guard_val {
                        Ok(v) if !v.is_truthy() => {
                            self.env.pop_scope();
                            continue;
                        }
                        Err(e) => {
                            self.env.pop_scope();
                            return Err(e);
                        }
                        _ => {}
                    }
                }

                match self.eval_can(body) {
                    Ok(_) => {}
                    Err(e) => {
                        self.env.pop_scope();
                        match to_loop_action(e) {
                            LoopAction::Continue | LoopAction::ContinueWith(_) => {
                                continue;
                            }
                            LoopAction::Break(v) => return Ok(v),
                            LoopAction::Error(e) => return Err(e),
                        }
                    }
                }
                self.env.pop_scope();
            }
            Ok(Value::Void)
        }
    }

    // ── Loop ──────────────────────────────────────────────────────

    /// Evaluate a canonical infinite loop.
    fn eval_can_loop(&mut self, body: CanId) -> EvalResult {
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

    // ── Hash Length ────────────────────────────────────────────────

    /// Evaluate a canonical expression with `#` resolved to a collection length.
    fn eval_can_with_hash_length(&mut self, can_id: CanId, length: i64) -> EvalResult {
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(can_id);
        match kind {
            CanExpr::HashLength => Ok(Value::int(length)),
            CanExpr::Binary { op, left, right } => {
                let l = self.eval_can_with_hash_length(left, length)?;
                let r = self.eval_can_with_hash_length(right, length)?;
                evaluate_binary(l, r, op).map_err(|e| Self::attach_span(e, self.can_span(can_id)))
            }
            CanExpr::Unary {
                op: UnaryOp::Neg,
                operand,
            } => {
                let v = self.eval_can_with_hash_length(operand, length)?;
                evaluate_unary(v, UnaryOp::Neg)
                    .map_err(|e| Self::attach_span(e, self.can_span(can_id)))
            }
            _ => self.eval_can(can_id),
        }
    }

    // ── FunctionExp ────────────────────────────────────────────────

    /// Evaluate a canonical `FunctionExp` by pre-evaluating props and dispatching.
    ///
    /// In canonical IR, `FunctionExp` props are `CanNamedExpr` (name + `CanId`).
    /// We evaluate all props eagerly, then delegate to the existing pattern
    /// registry via the legacy `EvalContext` path by bridging the evaluated values.
    fn eval_can_function_exp(
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
        let named: Vec<_> = self.canon_ref().arena.get_named_exprs(props).to_vec();
        let mut values: Vec<(Name, Value)> = Vec::with_capacity(named.len());
        for ne in &named {
            let v = self.eval_can(ne.value)?;
            values.push((ne.name, v));
        }

        // Dispatch by kind with pre-evaluated values
        match kind {
            FunctionExpKind::Print => {
                let msg = find_prop_value(&values, "msg", self.interner)?;
                self.print_handler.println(&msg.display_value());
                Ok(Value::Void)
            }
            FunctionExpKind::Panic => {
                let msg = find_prop_value(&values, "msg", self.interner)?;
                Err(EvalError::new(msg.display_value()).into())
            }
            FunctionExpKind::Todo => {
                let msg = values
                    .iter()
                    .find(|(n, _)| self.interner.lookup(*n) == "msg")
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
            // For patterns that need the full pattern registry (cache, recurse,
            // parallel, spawn, timeout, with), fall back to the legacy path.
            // This is safe because the interpreter still has self.arena.
            _ => Err(EvalError::new(format!(
                "pattern '{}' not yet supported in canonical evaluation mode",
                kind.name()
            ))
            .into()),
        }
    }

    /// Evaluate a `catch(expr: ...)` expression with lazy prop evaluation.
    ///
    /// Unlike other patterns, `catch` must evaluate its `expr` prop *inside*
    /// the catch context so that panics during evaluation are captured as
    /// `Err` values rather than propagated.
    fn eval_can_catch(&mut self, props: ori_ir::canon::CanNamedExprRange) -> EvalResult {
        let named = self.canon_ref().arena.get_named_exprs(props).to_vec();
        let expr_can_id = find_prop_can_id(&named, "expr", self.interner)?;

        match self.eval_can(expr_can_id) {
            Ok(v) => Ok(Value::ok(v)),
            Err(ControlAction::Error(e)) => Ok(Value::err(Value::string(e.message.clone()))),
            Err(e) => Err(e),
        }
    }

    /// Evaluate a `recurse(condition: ..., base: ..., step: ...)` expression.
    ///
    /// All three props must be evaluated lazily:
    /// - `condition` is evaluated first
    /// - If truthy, only `base` is evaluated (short-circuit)
    /// - If falsy, only `step` is evaluated (which may call `self()`)
    ///
    /// This prevents eager evaluation of `step` from causing index-out-of-bounds
    /// or division-by-zero when the base case should have returned.
    fn eval_can_recurse(&mut self, props: ori_ir::canon::CanNamedExprRange) -> EvalResult {
        let named = self.canon_ref().arena.get_named_exprs(props).to_vec();

        let condition_id = find_prop_can_id(&named, "condition", self.interner)?;
        let base_id = find_prop_can_id(&named, "base", self.interner)?;
        let step_id = find_prop_can_id(&named, "step", self.interner)?;

        // Check optional memo prop
        let memo_id = named
            .iter()
            .find(|ne| self.interner.lookup(ne.name) == "memo")
            .map(|ne| ne.value);

        if let Some(mid) = memo_id {
            let memo_val = self.eval_can(mid)?;
            if memo_val.is_truthy() {
                // Wrap `self` in a memoized function for the step evaluation
                if let Some(Value::Function(f)) = self.env.lookup(self.self_name) {
                    let memoized = Value::MemoizedFunction(MemoizedFunctionValue::new(f));
                    self.env.push_scope();
                    self.env
                        .define(self.self_name, memoized, Mutability::Immutable);
                    let result = self.eval_can_recurse_body(condition_id, base_id, step_id);
                    self.env.pop_scope();
                    return result;
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

// ── Helpers ────────────────────────────────────────────────────────

/// Convert a `ConstValue` from the constant pool to a runtime `Value`.
fn const_to_value(cv: &ori_ir::canon::ConstValue, interner: &ori_ir::StringInterner) -> Value {
    match *cv {
        ori_ir::canon::ConstValue::Int(n) => Value::int(n),
        ori_ir::canon::ConstValue::Float(bits) => Value::Float(f64::from_bits(bits)),
        ori_ir::canon::ConstValue::Bool(b) => Value::Bool(b),
        ori_ir::canon::ConstValue::Str(name) => Value::string_static(interner.lookup_static(name)),
        ori_ir::canon::ConstValue::Char(c) => Value::Char(c),
        ori_ir::canon::ConstValue::Unit => Value::Void,
        ori_ir::canon::ConstValue::Duration { value, unit } => {
            Value::Duration(unit.to_nanos(value))
        }
        ori_ir::canon::ConstValue::Size { value, unit } => Value::Size(unit.to_bytes(value)),
    }
}

/// Look up a pre-evaluated prop by name.
fn find_prop_value(
    values: &[(Name, Value)],
    name: &str,
    interner: &ori_ir::StringInterner,
) -> Result<Value, ControlAction> {
    values
        .iter()
        .find(|(n, _)| interner.lookup(*n) == name)
        .map(|(_, v)| v.clone())
        .ok_or_else(|| EvalError::new(format!("missing required property: {name}")).into())
}

/// Look up an unevaluated prop's `CanId` by name (for lazy evaluation).
fn find_prop_can_id(
    named: &[ori_ir::canon::CanNamedExpr],
    name: &str,
    interner: &ori_ir::StringInterner,
) -> Result<CanId, ControlAction> {
    named
        .iter()
        .find(|ne| interner.lookup(ne.name) == name)
        .map(|ne| ne.value)
        .ok_or_else(|| EvalError::new(format!("missing required property: {name}")).into())
}

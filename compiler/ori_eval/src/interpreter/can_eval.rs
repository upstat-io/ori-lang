//! Canonical expression evaluation — `CanExpr` dispatch.
//!
//! This module provides `eval_can(CanId)` as the sole evaluation path.
//! All function and method calls dispatch through `eval_can`.
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
use ori_patterns::{ControlAction, EvalError, EvalResult, RangeValue, Value};
use ori_stack::ensure_sufficient_stack;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use super::Interpreter;
use crate::errors::{
    await_not_supported, hash_outside_index, map_key_not_hashable, non_exhaustive_match,
    parse_error, range_bound_not_int, self_outside_method, unbounded_range_end, undefined_const,
    undefined_function,
};
use crate::exec::expr;
use crate::{
    evaluate_binary, evaluate_unary, FunctionValue, MemoizedFunctionValue, Mutability, StructValue,
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
        // Invariant: eval_can is only called when canon is known to be Some.
        // This is enforced by the call sites (function_call sets canon before calling).
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
        let ids: SmallVec<[CanId; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_expr_list(range));
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
            // Literals
            CanExpr::Int(n) => Ok(Value::int(n)),
            CanExpr::Float(bits) => Ok(Value::Float(f64::from_bits(bits))),
            CanExpr::Bool(b) => Ok(Value::Bool(b)),
            CanExpr::Str(name) => Ok(Value::string_static(self.interner.lookup_static(name))),
            CanExpr::Char(c) => Ok(Value::Char(c)),
            CanExpr::Duration { value, unit } => Ok(Value::Duration(unit.to_nanos(value))),
            CanExpr::Size { value, unit } => Ok(Value::Size(unit.to_bytes(value))),
            CanExpr::Unit => Ok(Value::Void),

            // Compile-Time Constant
            CanExpr::Constant(id) => {
                let cv = self.canon_ref().constants.get(id);
                Ok(const_to_value(cv, self.interner))
            }

            // References
            CanExpr::Ident(name) => {
                let span = self.can_span(can_id);
                expr::eval_ident(name, &self.env, self.interner)
                    .or_else(|e| {
                        // FIXME(canonicalization): Type reference resolution should
                        // happen in ori_canon, not here. This fallback exists because
                        // cross-module type refs aren't yet resolved during
                        // canonicalization. Remove once ori_canon handles imported
                        // type resolution.
                        if self
                            .user_method_registry
                            .read()
                            .has_any_methods_for_type(name)
                        {
                            Ok(Value::TypeRef { type_name: name })
                        } else {
                            Err(e)
                        }
                    })
                    .map_err(|e| Self::attach_span(e, span))
            }
            CanExpr::TypeRef(name) => {
                // Type reference resolved at canonicalization time.
                // Check environment first for variable shadowing.
                if let Some(val) = self.env.lookup(name) {
                    Ok(val)
                } else {
                    Ok(Value::TypeRef { type_name: name })
                }
            }
            CanExpr::Const(name) => {
                let span = self.can_span(can_id);
                self.env.lookup(name).ok_or_else(|| {
                    Self::attach_span(undefined_const(self.interner.lookup(name)).into(), span)
                })
            }
            CanExpr::SelfRef => {
                let span = self.can_span(can_id);
                self.env
                    .lookup(self.self_name)
                    .ok_or_else(|| Self::attach_span(self_outside_method().into(), span))
            }
            CanExpr::FunctionRef(name) => {
                let span = self.can_span(can_id);
                self.env.lookup(name).ok_or_else(|| {
                    Self::attach_span(undefined_function(self.interner.lookup(name)).into(), span)
                })
            }
            CanExpr::HashLength => {
                let span = self.can_span(can_id);
                Err(Self::attach_span(hash_outside_index().into(), span))
            }

            // Operators
            CanExpr::Binary { op, left, right } => self.eval_can_binary(can_id, left, op, right),
            CanExpr::Unary { op, operand } => self.eval_can_unary(can_id, op, operand),
            CanExpr::Cast {
                expr,
                target,
                fallible,
            } => {
                let value = self.eval_can(expr)?;
                let span = self.can_span(can_id);
                self.eval_can_cast(value, target, fallible)
                    .map_err(|e| Self::attach_span(e, span))
            }

            // Calls
            CanExpr::Call { func, args } => {
                let func_val = self.eval_can(func)?;
                let arg_vals = self.eval_can_expr_list(args)?;
                let span = self.can_span(can_id);
                self.eval_call(&func_val, &arg_vals)
                    .map_err(|e| Self::attach_span(e, span))
            }
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let recv = self.eval_can(receiver)?;
                let arg_vals = self.eval_can_expr_list(args)?;
                let span = self.can_span(can_id);
                self.dispatch_method_call(recv, method, arg_vals)
                    .map_err(|e| Self::attach_span(e, span))
            }

            // Access
            CanExpr::Field { receiver, field } => {
                let span = self.can_span(can_id);
                let value = self.eval_can(receiver)?;
                expr::eval_field_access(value, field, self.interner)
                    .map_err(|e| Self::attach_span(e, span))
            }
            CanExpr::Index { receiver, index } => {
                let span = self.can_span(can_id);
                let value = self.eval_can(receiver)?;
                let length = expr::get_collection_length(&value)
                    .map_err(|e| Self::attach_span(e.into(), span))?;
                let idx = self.eval_can_with_hash_length(index, length)?;
                expr::eval_index(value, idx).map_err(|e| Self::attach_span(e, span))
            }

            // Control Flow
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
                let span = self.can_span(can_id);
                self.eval_can_match(&value, decision_tree, arms)
                    .map_err(|e| Self::attach_span(e, span))
            }
            CanExpr::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
                ..
            } => {
                let iter_val = self.eval_can(iter)?;
                let span = self.can_span(can_id);
                self.eval_can_for(binding, &iter_val, guard, body, is_yield)
                    .map_err(|e| Self::attach_span(e, span))
            }
            CanExpr::Loop { body, .. } => self.eval_can_loop(body),
            CanExpr::Break { value: v, .. } => {
                let val = if v.is_valid() {
                    self.eval_can(v)?
                } else {
                    Value::Void
                };
                Err(ControlAction::Break(val))
            }
            CanExpr::Continue { value: v, .. } => {
                let val = if v.is_valid() {
                    self.eval_can(v)?
                } else {
                    Value::Void
                };
                Err(ControlAction::Continue(val))
            }

            // Bindings
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

            // Functions
            CanExpr::Lambda { params, body } => {
                let span = self.can_span(can_id);
                self.eval_can_lambda(params, body)
                    .map_err(|e| Self::attach_span(e, span))
            }

            // Collections
            CanExpr::List(range) => Ok(Value::list(self.eval_can_expr_list(range)?)),
            CanExpr::Tuple(range) => Ok(Value::tuple(self.eval_can_expr_list(range)?)),
            CanExpr::Map(entries) => self.eval_can_map(can_id, entries),
            CanExpr::Struct { name, fields } => self.eval_can_struct(name, fields),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.eval_can_range(start, end, step, inclusive),

            // Algebraic
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

            // Error Handling
            CanExpr::Try(inner) => match self.eval_can(inner)? {
                Value::Ok(v) | Value::Some(v) => Ok((*v).clone()),
                Value::Err(e) => Err(ControlAction::Propagate(Value::Err(e))),
                Value::None => Err(ControlAction::Propagate(Value::None)),
                other => Ok(other),
            },
            CanExpr::Await(_) => {
                let span = self.can_span(can_id);
                Err(Self::attach_span(await_not_supported().into(), span))
            }

            // Capabilities
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider_val = self.eval_can(provider)?;
                self.with_binding(capability, provider_val, Mutability::Immutable, |scoped| {
                    scoped.eval_can(body)
                })
            }

            // Special Forms
            CanExpr::FunctionExp { kind, props } => {
                let span = self.can_span(can_id);
                self.eval_can_function_exp(kind, props)
                    .map_err(|e| Self::attach_span(e, span))
            }

            // Error Recovery
            CanExpr::Error => {
                let span = self.can_span(can_id);
                Err(Self::attach_span(parse_error().into(), span))
            }
        }
    }

    // Binary Operators

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
        if let Some(method) = super::binary_op_to_method(op, self.op_names) {
            return self.eval_method_call(left_val, method, vec![right_val]);
        }

        evaluate_binary(left_val, right_val, op).map_err(|e| Self::attach_span(e, span))
    }

    // Unary Operators

    /// Evaluate a canonical unary operation.
    fn eval_can_unary(&mut self, expr_id: CanId, op: UnaryOp, operand: CanId) -> EvalResult {
        let value = self.eval_can(operand)?;
        let span = self.can_span(expr_id);

        if super::is_primitive_value(&value) {
            return evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span));
        }

        if let Some(method) = super::unary_op_to_method(op, self.op_names) {
            return self.eval_method_call(value, method, vec![]);
        }

        evaluate_unary(value, op).map_err(|e| Self::attach_span(e, span))
    }

    // Type Cast

    /// Evaluate a canonical type cast using the target type name.
    ///
    /// Uses pre-interned `TypeNames` for O(1) `Name` comparison instead of
    /// deinterning to `&str`. Only falls back to `interner.lookup()` on the
    /// cold error path for diagnostic messages.
    fn eval_can_cast(&self, value: Value, target: Name, fallible: bool) -> EvalResult {
        let tn = &self.type_names;
        let result = match &value {
            // int conversions
            #[expect(
                clippy::cast_precision_loss,
                reason = "intentional int-to-float conversion"
            )]
            Value::Int(n) if target == tn.float => Ok(Value::Float(n.raw() as f64)),
            Value::Int(n) if target == tn.byte => {
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
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "range checked on line above"
                )]
                Ok(Value::Byte(raw as u8))
            }
            Value::Int(n) if target == tn.char_ => {
                let raw = n.raw();
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "char::from_u32 validates the value"
                )]
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
            Value::Byte(b) if target == tn.int => Ok(Value::int(i64::from(*b))),
            Value::Char(c) if target == tn.int => Ok(Value::int(i64::from(*c as u32))),
            #[expect(
                clippy::cast_possible_truncation,
                reason = "intentional float-to-int truncation"
            )]
            Value::Float(f) if target == tn.int => Ok(Value::int(*f as i64)),
            Value::Str(s) if target == tn.int => match s.parse::<i64>() {
                Ok(n) => Ok(Value::int(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as int")).into());
                }
            },
            Value::Str(s) if target == tn.float => match s.parse::<f64>() {
                Ok(n) => Ok(Value::Float(n)),
                Err(_) if fallible => return Ok(Value::None),
                Err(_) => {
                    return Err(EvalError::new(format!("cannot parse '{s}' as float")).into());
                }
            },
            // Identity conversions
            Value::Int(_) if target == tn.int => Ok(value),
            Value::Float(_) if target == tn.float => Ok(value),
            Value::Str(_) if target == tn.str_ => Ok(value),
            Value::Bool(_) if target == tn.bool_ => Ok(value),
            Value::Byte(_) if target == tn.byte => Ok(value),
            Value::Char(_) if target == tn.char_ => Ok(value),
            // str conversion - anything can become a string
            _ if target == tn.str_ => Ok(Value::string(value.to_string())),
            _ => {
                if fallible {
                    return Ok(Value::None);
                }
                let target_name = self.interner.lookup(target);
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

    // Block

    /// Evaluate a canonical block: `{ stmts; result }`.
    fn eval_can_block(&mut self, stmts: CanRange, result: CanId) -> EvalResult {
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

    // Binding Pattern

    /// Bind a canonical binding pattern to a value.
    fn bind_can_pattern(
        &mut self,
        pattern: &CanBindingPattern,
        value: Value,
        mutability: Mutability,
    ) -> EvalResult {
        match pattern {
            CanBindingPattern::Name { name, mutable } => {
                // Per-binding mutability: use the flag from the pattern itself,
                // not the inherited top-level mutability. This enables `let ($x, y) = ...`
                // where `x` is immutable and `y` is mutable.
                let binding_mutability = if *mutable {
                    Mutability::Mutable
                } else {
                    Mutability::Immutable
                };
                self.env.define(*name, value, binding_mutability);
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
                    for (pat_id, val) in pat_ids.into_iter().zip(values.iter()) {
                        // Copy the sub-pattern out to avoid borrow conflict
                        let sub_pat = *self.canon_ref().arena.get_binding_pattern(pat_id);
                        self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
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
                            self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
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
                        self.bind_can_pattern(&sub_pat, val.clone(), mutability)?;
                    }
                    if let Some(rest_name) = rest {
                        let rest_values = values[pat_ids.len()..].to_vec();
                        self.env
                            .define(*rest_name, Value::list(rest_values), mutability);
                    }
                    Ok(Value::Void)
                } else {
                    Err(crate::errors::expected_list().into())
                }
            }
        }
    }

    // Assignment

    /// Evaluate a canonical assignment: `target = value`.
    fn eval_can_assign(&mut self, target: CanId, value: Value) -> EvalResult {
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
            CanExpr::Index { .. } => Err(crate::errors::index_assignment_not_implemented().into()),
            CanExpr::Field { .. } => Err(crate::errors::field_assignment_not_implemented().into()),
            _ => Err(crate::errors::invalid_assignment_target().into()),
        }
    }

    // Lambda

    /// Evaluate a canonical lambda: create a `FunctionValue` with canonical data.
    fn eval_can_lambda(&mut self, params: CanParamRange, body: CanId) -> EvalResult {
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

    // Map

    /// Evaluate a canonical map literal: `{ k: v, ... }`.
    fn eval_can_map(&mut self, can_id: CanId, entries: CanMapEntryRange) -> EvalResult {
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

    // Struct

    /// Evaluate a canonical struct literal: `Point { x: 0, y: 0 }`.
    fn eval_can_struct(&mut self, name: Name, fields: ori_ir::canon::CanFieldRange) -> EvalResult {
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

    // Range

    /// Evaluate a canonical range: `start..end`, `start..=end`, `start..end by step`.
    ///
    /// Evaluates range bounds directly via `eval_can` — no `ExprId` roundtrip.
    fn eval_can_range(
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
            self.eval_can(end)?
                .as_int()
                .ok_or_else(|| ControlAction::from(range_bound_not_int("end")))?
        } else {
            return Err(unbounded_range_end().into());
        };
        let step_val = if step.is_valid() {
            self.eval_can(step)?
                .as_int()
                .ok_or_else(|| ControlAction::from(range_bound_not_int("step")))?
        } else {
            1
        };

        if inclusive {
            Ok(Value::Range(RangeValue::inclusive_with_step(
                start_val, end_val, step_val,
            )))
        } else {
            Ok(Value::Range(RangeValue::exclusive_with_step(
                start_val, end_val, step_val,
            )))
        }
    }

    // Match (Decision Tree)

    /// Evaluate a canonical match expression via decision tree.
    fn eval_can_match(
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
                    .ok_or_else(non_exhaustive_match)?;

                self.with_match_bindings(match_result.bindings, |scoped| scoped.eval_can(arm_id))
            }
            Err(e) => Err(e.into()),
        }
    }

    // For Loop

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

        // Build a stack-allocated iterator via enum dispatch (no heap allocation).
        let iter = ForIterator::from_value(iter_val)?;

        if is_yield {
            // for...yield collects results into a list
            let capacity = match iter_val {
                Value::List(items) => items.len(),
                Value::Map(map) => map.len(),
                _ => 0,
            };
            let mut results = Vec::with_capacity(capacity);
            for item in iter {
                let mut scoped = self.scoped();
                scoped.env.define(binding, item, Mutability::Immutable);

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
            // Regular for loop returns Void
            for item in iter {
                let mut scoped = self.scoped();
                scoped.env.define(binding, item, Mutability::Immutable);

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

    // Loop

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

    // Hash Length

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

    // FunctionExp

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
                let msg = find_prop_value(&values, pn.msg, self.interner)?;
                self.print_handler.println(&msg.display_value());
                Ok(Value::Void)
            }
            FunctionExpKind::Panic => {
                let msg = find_prop_value(&values, pn.msg, self.interner)?;
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
                let operation = find_prop_value(&values, pn.operation, self.interner)?;
                match operation {
                    Value::Function(_) | Value::FunctionVal(_, _) => {
                        self.eval_call(&operation, &[])
                    }
                    _ => Ok(operation),
                }
            }
            FunctionExpKind::Parallel => {
                tracing::warn!("pattern 'parallel' is a stub — tasks are executed sequentially");
                let tasks = find_prop_value(&values, pn.tasks, self.interner)?;
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
                let tasks = find_prop_value(&values, pn.tasks, self.interner)?;
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
                let operation = find_prop_value(&values, pn.operation, self.interner)?;
                Ok(Value::ok(operation))
            }
            FunctionExpKind::With => {
                tracing::warn!(
                    "pattern 'with' is a stub — resource management without type checker integration"
                );
                let resource = find_prop_value(&values, pn.acquire, self.interner)?;
                let action_fn = find_prop_value(&values, pn.action, self.interner)?;
                let result = self.eval_call(&action_fn, std::slice::from_ref(&resource));
                // Always call release if provided (RAII guarantee)
                if let Ok(release_fn) = find_prop_value(&values, pn.release, self.interner) {
                    let _ = self.eval_call(&release_fn, std::slice::from_ref(&resource));
                }
                result
            }
        }
    }

    /// Evaluate a `catch(expr: ...)` expression with lazy prop evaluation.
    ///
    /// Unlike other patterns, `catch` must evaluate its `expr` prop *inside*
    /// the catch context so that panics during evaluation are captured as
    /// `Err` values rather than propagated.
    fn eval_can_catch(&mut self, props: ori_ir::canon::CanNamedExprRange) -> EvalResult {
        let named: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_named_exprs(props));
        let expr_can_id = find_prop_can_id(&named, self.prop_names.expr, self.interner)?;

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
        let named: SmallVec<[_; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_named_exprs(props));
        let pn = self.prop_names;

        let condition_id = find_prop_can_id(&named, pn.condition, self.interner)?;
        let base_id = find_prop_can_id(&named, pn.base, self.interner)?;
        let step_id = find_prop_can_id(&named, pn.step, self.interner)?;

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

// For Loop Iterator

/// Stack-allocated iterator for for-loop dispatch.
///
/// Replaces `Box<dyn Iterator<Item = Value>>` with enum dispatch to avoid
/// a heap allocation + vtable indirection per for-loop. The iterator type
/// set is closed (List, Map, Range, Str), so enum dispatch is correct.
///
/// Each variant stores the borrowed data and a position/cursor for
/// stateful iteration. List, Map, and Str borrow from the `Value`
/// being iterated (which outlives the loop body). Range stores its
/// bounds directly for zero-allocation stepping.
enum ForIterator<'a> {
    /// Iterates over list elements by index.
    List { items: &'a [Value], pos: usize },
    /// Iterates over map entries as `(key, value)` tuples.
    Map {
        iter: std::collections::btree_map::Iter<'a, String, Value>,
    },
    /// Iterates over a range of integers (fully owned, no borrowing).
    Range {
        current: Option<i64>,
        end: i64,
        step: i64,
        inclusive: bool,
    },
    /// Iterates over string characters.
    Str { chars: std::str::Chars<'a> },
}

impl<'a> ForIterator<'a> {
    /// Create a `ForIterator` from a `Value`, or return an error if not iterable.
    fn from_value(value: &'a Value) -> Result<Self, ControlAction> {
        match value {
            Value::List(items) => Ok(ForIterator::List { items, pos: 0 }),
            Value::Map(map) => Ok(ForIterator::Map { iter: map.iter() }),
            Value::Range(range) => Ok(ForIterator::Range {
                current: range_initial(range),
                end: range.end,
                step: range.step,
                inclusive: range.inclusive,
            }),
            Value::Str(s) => Ok(ForIterator::Str { chars: s.chars() }),
            _ => Err(crate::errors::for_requires_iterable().into()),
        }
    }
}

#[expect(
    clippy::arithmetic_side_effects,
    reason = "range bound arithmetic on user-provided i64 values"
)]
impl Iterator for ForIterator<'_> {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        match self {
            ForIterator::List { items, pos } => {
                let item = items.get(*pos)?;
                *pos += 1;
                Some(item.clone())
            }
            ForIterator::Map { iter } => {
                let (k, v) = iter.next()?;
                Some(Value::tuple(vec![Value::string(k.clone()), v.clone()]))
            }
            ForIterator::Range {
                current,
                end,
                step,
                inclusive,
            } => {
                let val = (*current)?;
                let s = *step;
                let e = *end;
                let incl = *inclusive;
                // Compute next value
                let next = val + s;
                *current = match s.cmp(&0) {
                    std::cmp::Ordering::Greater => {
                        if incl {
                            (next <= e).then_some(next)
                        } else {
                            (next < e).then_some(next)
                        }
                    }
                    std::cmp::Ordering::Less => {
                        if incl {
                            (next >= e).then_some(next)
                        } else {
                            (next > e).then_some(next)
                        }
                    }
                    std::cmp::Ordering::Equal => None,
                };
                Some(Value::int(val))
            }
            ForIterator::Str { chars } => chars.next().map(Value::Char),
        }
    }
}

/// Compute the initial value for a range iterator.
///
/// Returns `None` if the range is empty (e.g., `5..0` with positive step).
fn range_initial(range: &RangeValue) -> Option<i64> {
    match range.step.cmp(&0) {
        std::cmp::Ordering::Greater => {
            if range.inclusive {
                (range.start <= range.end).then_some(range.start)
            } else {
                (range.start < range.end).then_some(range.start)
            }
        }
        std::cmp::Ordering::Less => {
            if range.inclusive {
                (range.start >= range.end).then_some(range.start)
            } else {
                (range.start > range.end).then_some(range.start)
            }
        }
        std::cmp::Ordering::Equal => None,
    }
}

// Helpers

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

/// Look up a pre-evaluated prop by interned `Name`.
///
/// Uses direct `Name` comparison (single `u32 == u32`) instead of
/// string lookup per prop. Callers pass pre-interned names from `PropNames`.
fn find_prop_value(
    values: &[(Name, Value)],
    name: Name,
    interner: &ori_ir::StringInterner,
) -> Result<Value, ControlAction> {
    values
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| v.clone())
        .ok_or_else(|| {
            EvalError::new(format!(
                "missing required property: {}",
                interner.lookup(name)
            ))
            .into()
        })
}

/// Look up an unevaluated prop's `CanId` by interned `Name` (for lazy evaluation).
fn find_prop_can_id(
    named: &[ori_ir::canon::CanNamedExpr],
    name: Name,
    interner: &ori_ir::StringInterner,
) -> Result<CanId, ControlAction> {
    named
        .iter()
        .find(|ne| ne.name == name)
        .map(|ne| ne.value)
        .ok_or_else(|| {
            EvalError::new(format!(
                "missing required property: {}",
                interner.lookup(name)
            ))
            .into()
        })
}

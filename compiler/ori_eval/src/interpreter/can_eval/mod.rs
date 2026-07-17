//! Stack-safe evaluation over sugar-free [`CanExpr`] nodes, copying node kinds
//! so arena borrows do not span recursive dispatch.

mod control_flow;
mod function_exp;
mod operators;
mod trace;

use ori_ir::canon::{
    CanBindingPatternId, CanExpr, CanId, CanNamedExprRange, CanParamRange, CanRange, CanonResult,
    ConstantId, DecisionTreeId, MonoConstBinding, MonoInstanceId,
};
use ori_ir::{FunctionExpKind, Name, Span};
use ori_patterns::{ControlAction, EvalError, EvalResult, Value};
use ori_stack::ensure_sufficient_stack;
use smallvec::SmallVec;

use super::Interpreter;
use crate::errors::{
    await_not_supported, hash_outside_index, integer_overflow, parse_error, self_outside_method,
    undefined_const, undefined_function,
};
use crate::exec::expr;
use crate::Mutability;

impl Interpreter<'_> {
    /// Evaluates one canonical expression with stack safety.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn eval_can(&mut self, can_id: CanId) -> EvalResult {
        ensure_sufficient_stack(|| self.eval_can_inner(can_id))
    }

    #[inline]
    fn canon_ref(&self) -> &CanonResult {
        match &self.canon {
            Some(canon) => canon,
            None => unreachable!("canonical evaluation requires installed canonical IR"),
        }
    }

    #[inline]
    pub(super) fn can_span(&self, can_id: CanId) -> Span {
        self.canon_ref().arena.span(can_id)
    }

    /// Returns the monomorphization identity recorded for a generic call site.
    #[inline]
    pub(super) fn mono_instance_id_for(&self, can_id: CanId) -> Option<MonoInstanceId> {
        let map = &self.canon_ref().mono_dispatch_map_can;
        map.binary_search_by_key(&can_id.raw(), |&(k, _)| k.raw())
            .ok()
            .map(|idx| map[idx].1)
    }

    fn eval_can_expr_list(&mut self, range: CanRange) -> Result<Vec<Value>, ControlAction> {
        let ids: SmallVec<[CanId; 8]> =
            SmallVec::from_slice(self.canon_ref().arena.get_expr_list(range));
        ids.into_iter().map(|id| self.eval_can(id)).collect()
    }

    /// Evaluates one canonical node through an exhaustive dispatch.
    fn eval_can_inner(&mut self, can_id: CanId) -> EvalResult {
        self.mode_state.count_expression();

        let canon = self.canon_ref();
        let kind = *canon.arena.kind(can_id);
        tracing::trace!(?can_id, ?kind, "eval_can_inner");

        match kind {
            CanExpr::Int(n) => Ok(Value::int(n)),
            CanExpr::Float(bits) => Ok(Value::Float(f64::from_bits(bits))),
            CanExpr::Bool(b) => Ok(Value::Bool(b)),
            CanExpr::Str(name) => Ok(Value::string_static(self.interner.lookup_static(name))),
            CanExpr::Char(c) => Ok(Value::Char(c)),
            CanExpr::Duration { value, unit } => Ok(Value::Duration(
                unit.to_nanos(value)
                    .ok_or_else(|| integer_overflow("duration literal"))?,
            )),
            CanExpr::Size { value, unit } => Ok(Value::Size(
                unit.to_bytes(value)
                    .ok_or_else(|| integer_overflow("size literal"))?,
            )),
            CanExpr::Unit => Ok(Value::Void),
            CanExpr::Constant(id) => Ok(self.eval_can_constant(id)),
            CanExpr::Ident(name) => self.eval_can_ident(can_id, name),
            CanExpr::TypeRef(name) => Ok(self.eval_can_type_ref(name)),
            CanExpr::Const(name) => self.eval_can_const(can_id, name),
            CanExpr::SelfRef => self.eval_can_self_ref(can_id),
            CanExpr::FunctionRef(name) => self.eval_can_function_ref(can_id, name),
            CanExpr::HashLength => self.eval_can_hash_length(can_id),
            CanExpr::Binary { op, left, right } => self.eval_can_binary(can_id, left, op, right),
            CanExpr::Unary { op, operand } => self.eval_can_unary(can_id, op, operand),
            CanExpr::Cast {
                expr,
                target,
                fallible,
            } => self.eval_can_cast_expr(can_id, expr, target, fallible),
            CanExpr::Call { func, args } => self.eval_can_call(can_id, func, args),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.eval_can_method_call(can_id, receiver, method, args),
            CanExpr::Field { receiver, field } => self.eval_can_field(can_id, receiver, field),
            CanExpr::Index { receiver, index } => self.eval_can_index(can_id, receiver, index),
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => self.eval_can_if_expr(cond, then_branch, else_branch),
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => self.eval_can_match_expr(can_id, scrutinee, decision_tree, arms),
            CanExpr::For {
                pattern,
                iter,
                guard,
                body,
                is_yield,
                label,
                ..
            } => self.eval_can_for_expr(can_id, pattern, iter, guard, body, (is_yield, label)),
            CanExpr::Loop { body, label, .. } => self.eval_can_loop(body, label),
            CanExpr::Break { value, label, .. } => self.eval_can_break(value, label),
            CanExpr::Continue { value, label, .. } => self.eval_can_continue(value, label),
            CanExpr::Block { stmts, result } => self.eval_can_block(stmts, result),
            CanExpr::Let { pattern, init, .. } => self.eval_can_let(pattern, init),
            CanExpr::Assign { target, value } => {
                let val = self.eval_can(value)?;
                self.eval_can_assign(target, val)
            }
            CanExpr::Lambda { params, body } => self.eval_can_lambda_expr(can_id, params, body),
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
            CanExpr::Ok(inner) => Ok(Value::ok(self.eval_can_or_void(inner)?)),
            CanExpr::Err(inner) => Ok(Value::err(self.eval_can_or_void(inner)?)),
            CanExpr::Some(inner) => Ok(Value::some(self.eval_can(inner)?)),
            CanExpr::None => Ok(Value::None),
            CanExpr::Try(inner) => self.eval_can_try(can_id, inner),
            CanExpr::Unsafe(inner) => self.eval_can(inner),
            CanExpr::Await(_) => self.eval_can_await(can_id),
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => self.eval_can_with_capability(capability, provider, body),
            CanExpr::FunctionExp { kind, props } => {
                self.eval_can_function_expr(can_id, kind, props)
            }
            CanExpr::FormatWith { expr, spec } => self.eval_format_with(can_id, expr, spec),
            CanExpr::Error => self.eval_can_error(can_id),
        }
    }

    fn eval_can_constant(&self, id: ConstantId) -> Value {
        const_to_value(self.canon_ref().constants.get(id), self.interner)
    }

    fn eval_can_ident(&self, can_id: CanId, name: Name) -> EvalResult {
        let span = self.can_span(can_id);
        expr::eval_ident(name, &self.env, self.interner)
            .or_else(|error| {
                if self
                    .user_method_registry
                    .read()
                    .has_any_methods_for_type(name)
                {
                    Ok(Value::TypeRef { type_name: name })
                } else {
                    Err(error)
                }
            })
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_type_ref(&self, name: Name) -> Value {
        self.env
            .lookup(name)
            .unwrap_or(Value::TypeRef { type_name: name })
    }

    fn eval_can_const(&self, can_id: CanId, name: Name) -> EvalResult {
        let span = self.can_span(can_id);
        self.env.lookup(name).ok_or_else(|| {
            Self::attach_span(undefined_const(self.interner.lookup(name)).into(), span)
        })
    }

    fn eval_can_self_ref(&self, can_id: CanId) -> EvalResult {
        let span = self.can_span(can_id);
        self.env
            .lookup(self.self_name)
            .ok_or_else(|| Self::attach_span(self_outside_method().into(), span))
    }

    fn eval_can_function_ref(&self, can_id: CanId, name: Name) -> EvalResult {
        let span = self.can_span(can_id);
        self.env.lookup(name).ok_or_else(|| {
            Self::attach_span(undefined_function(self.interner.lookup(name)).into(), span)
        })
    }

    fn eval_can_hash_length(&self, can_id: CanId) -> EvalResult {
        Err(Self::attach_span(
            hash_outside_index().into(),
            self.can_span(can_id),
        ))
    }

    fn eval_can_cast_expr(
        &mut self,
        can_id: CanId,
        expr: CanId,
        target: Name,
        fallible: bool,
    ) -> EvalResult {
        let value = self.eval_can(expr)?;
        let span = self.can_span(can_id);
        self.eval_can_cast(value, target, fallible)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_call(&mut self, can_id: CanId, func: CanId, args: CanRange) -> EvalResult {
        let (mono_instance_id, const_bindings) = self.mono_const_bindings(can_id);
        if let Some(id) = mono_instance_id {
            tracing::trace!(
                ?can_id,
                mono_instance_id = id.raw(),
                "eval Call mono dispatch"
            );
        }
        let func_value = self.eval_can(func)?;
        let arg_values = self.eval_can_expr_list(args)?;
        let span = self.can_span(can_id);
        self.eval_call_with_const_bindings(&func_value, &arg_values, &const_bindings)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_method_call(
        &mut self,
        can_id: CanId,
        receiver: CanId,
        method: Name,
        args: CanRange,
    ) -> EvalResult {
        let (mono_instance_id, const_bindings) = self.mono_const_bindings(can_id);
        if let Some(id) = mono_instance_id {
            tracing::trace!(
                ?can_id,
                mono_instance_id = id.raw(),
                "eval MethodCall mono dispatch"
            );
        }
        let receiver_value = self.eval_can(receiver)?;
        let arg_values = self.eval_can_expr_list(args)?;
        let span = self.can_span(can_id);
        self.dispatch_method_call_with_const_bindings(
            receiver_value,
            method,
            arg_values,
            &const_bindings,
        )
        .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_field(&mut self, can_id: CanId, receiver: CanId, field: Name) -> EvalResult {
        let span = self.can_span(can_id);
        let value = self.eval_can(receiver)?;
        expr::eval_field_access(value, field, self.interner)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn mono_const_bindings(
        &self,
        can_id: CanId,
    ) -> (Option<MonoInstanceId>, Vec<MonoConstBinding>) {
        let id = self.mono_instance_id_for(can_id);
        let bindings = id
            .and_then(|id| self.canon_ref().mono_const_bindings(id))
            .unwrap_or_default()
            .to_vec();
        (id, bindings)
    }

    fn eval_can_index(&mut self, can_id: CanId, receiver: CanId, index: CanId) -> EvalResult {
        let span = self.can_span(can_id);
        let value = self.eval_can(receiver)?;
        if super::operator_dispatch::is_builtin_indexable(&value) {
            let length = expr::get_collection_length(&value)
                .map_err(|error| Self::attach_span(error.into(), span))?;
            let index = self.eval_can_with_hash_length(index, length)?;
            expr::eval_index(value, index).map_err(|error| Self::attach_span(error, span))
        } else {
            let index = self.eval_can(index)?;
            self.eval_index_user_type(value, index)
        }
    }

    fn eval_can_if_expr(
        &mut self,
        cond: CanId,
        then_branch: CanId,
        else_branch: CanId,
    ) -> EvalResult {
        if self.eval_can(cond)?.is_truthy() {
            self.eval_can(then_branch)
        } else if else_branch.is_valid() {
            self.eval_can(else_branch)
        } else {
            Ok(Value::Void)
        }
    }

    fn eval_can_match_expr(
        &mut self,
        can_id: CanId,
        scrutinee: CanId,
        decision_tree: DecisionTreeId,
        arms: CanRange,
    ) -> EvalResult {
        let value = self.eval_can(scrutinee)?;
        let span = self.can_span(can_id);
        self.eval_can_match(&value, decision_tree, arms)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_for_expr(
        &mut self,
        can_id: CanId,
        pattern: CanBindingPatternId,
        iter: CanId,
        guard: CanId,
        body: CanId,
        mode: (bool, Name),
    ) -> EvalResult {
        let (is_yield, label) = mode;
        let iter_value = self.eval_can(iter)?;
        let span = self.can_span(can_id);
        self.eval_can_for(pattern, &iter_value, guard, body, is_yield, label)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_break(&mut self, value: CanId, label: Name) -> EvalResult {
        Err(ControlAction::Break(self.eval_can_or_void(value)?, label))
    }

    fn eval_can_continue(&mut self, value: CanId, label: Name) -> EvalResult {
        Err(ControlAction::Continue(
            self.eval_can_or_void(value)?,
            label,
        ))
    }

    fn eval_can_let(&mut self, pattern: CanBindingPatternId, init: CanId) -> EvalResult {
        let value = self.eval_can(init)?;
        let pattern = *self.canon_ref().arena.get_binding_pattern(pattern);
        self.bind_can_pattern(&pattern, value)
    }

    fn eval_can_lambda_expr(
        &mut self,
        can_id: CanId,
        params: CanParamRange,
        body: CanId,
    ) -> EvalResult {
        let span = self.can_span(can_id);
        self.eval_can_lambda(params, body)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_or_void(&mut self, can_id: CanId) -> EvalResult {
        if can_id.is_valid() {
            self.eval_can(can_id)
        } else {
            Ok(Value::Void)
        }
    }

    fn eval_can_try(&mut self, can_id: CanId, inner: CanId) -> EvalResult {
        match self.eval_can(inner)? {
            Value::Ok(value) | Value::Some(value) => Ok((*value).clone()),
            error @ Value::Err(_) => Err(ControlAction::Propagate(
                self.inject_trace_entry(error, can_id),
            )),
            Value::None => Err(ControlAction::Propagate(Value::None)),
            other => Ok(other),
        }
    }

    fn eval_can_await(&self, can_id: CanId) -> EvalResult {
        Err(Self::attach_span(
            await_not_supported().into(),
            self.can_span(can_id),
        ))
    }

    fn eval_can_with_capability(
        &mut self,
        capability: Name,
        provider: CanId,
        body: CanId,
    ) -> EvalResult {
        let provider = self.eval_can(provider)?;
        self.with_binding(capability, provider, Mutability::Immutable, |scoped| {
            scoped.eval_can(body)
        })
    }

    fn eval_can_function_expr(
        &mut self,
        can_id: CanId,
        kind: FunctionExpKind,
        props: CanNamedExprRange,
    ) -> EvalResult {
        let span = self.can_span(can_id);
        self.eval_can_function_exp(kind, props)
            .map_err(|error| Self::attach_span(error, span))
    }

    fn eval_can_error(&self, can_id: CanId) -> EvalResult {
        Err(Self::attach_span(
            parse_error().into(),
            self.can_span(can_id),
        ))
    }
}

/// Converts a constant-pool value to its runtime representation.
#[expect(
    clippy::expect_used,
    reason = "Constants come from cooker-validated literals (overflow-checked) or \
              const-fold results (Nanoseconds/Bytes unit, i64-bounded arithmetic). \
              Both paths guarantee to_nanos/to_bytes succeed."
)]
fn const_to_value(cv: &ori_ir::canon::ConstValue, interner: &ori_ir::StringInterner) -> Value {
    match *cv {
        ori_ir::canon::ConstValue::Int(n) => Value::int(n),
        ori_ir::canon::ConstValue::Float(bits) => Value::Float(f64::from_bits(bits)),
        ori_ir::canon::ConstValue::Bool(b) => Value::Bool(b),
        ori_ir::canon::ConstValue::Str(name) => Value::string_static(interner.lookup_static(name)),
        ori_ir::canon::ConstValue::Char(c) => Value::Char(c),
        ori_ir::canon::ConstValue::Unit => Value::Void,
        ori_ir::canon::ConstValue::Duration { value, unit } => Value::Duration(
            unit.to_nanos(value)
                .expect("duration overflow: constant should have been validated"),
        ),
        ori_ir::canon::ConstValue::Size { value, unit } => Value::Size(
            unit.to_bytes(value)
                .expect("size overflow: constant should have been validated"),
        ),
    }
}

/// Returns a pre-evaluated property selected by interned `Name`.
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

/// Returns an unevaluated property's `CanId` for lazy evaluation.
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

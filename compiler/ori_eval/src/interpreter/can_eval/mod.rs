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

mod control_flow;
mod function_exp;
mod operators;
mod trace;

use ori_ir::canon::{CanExpr, CanId, CanRange, CanonResult};
use ori_ir::{Name, Span};
use ori_patterns::{ControlAction, EvalError, EvalResult, Value};
use ori_stack::ensure_sufficient_stack;
use smallvec::SmallVec;

use super::Interpreter;
use crate::errors::{
    await_not_supported, hash_outside_index, parse_error, self_outside_method, undefined_const,
    undefined_function,
};
use crate::exec::expr;
use crate::Mutability;

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
    pub(super) fn can_span(&self, can_id: CanId) -> Span {
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
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive CanExpr variant dispatch — no catch-all"
    )]
    fn eval_can_inner(&mut self, can_id: CanId) -> EvalResult {
        self.mode_state.count_expression();

        // Copy the kind out to release the borrow on self.canon.
        // CanExpr is Copy (24 bytes) — this is cheap.
        let canon = self.canon_ref();
        let kind = *canon.arena.kind(can_id);
        tracing::trace!(?can_id, ?kind, "eval_can_inner");

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

                // Built-in types: fast path with # (hash length) support
                if super::operator_dispatch::is_builtin_indexable(&value) {
                    let length = expr::get_collection_length(&value)
                        .map_err(|e| Self::attach_span(e.into(), span))?;
                    let idx = self.eval_can_with_hash_length(index, length)?;
                    expr::eval_index(value, idx).map_err(|e| Self::attach_span(e, span))
                } else {
                    // User-defined types: dispatch via Index trait method
                    let idx_val = self.eval_can(index)?;
                    self.eval_index_user_type(value, idx_val)
                }
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
                pattern,
                iter,
                guard,
                body,
                is_yield,
                ..
            } => {
                let iter_val = self.eval_can(iter)?;
                let span = self.can_span(can_id);
                self.eval_can_for(pattern, &iter_val, guard, body, is_yield)
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
            CanExpr::Let { pattern, init, .. } => {
                let value = self.eval_can(init)?;
                // Copy the pattern out to avoid borrow conflict
                let pat = *self.canon_ref().arena.get_binding_pattern(pattern);
                self.bind_can_pattern(&pat, value)
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
                err @ Value::Err(_) => {
                    let traced = self.inject_trace_entry(err, can_id);
                    Err(ControlAction::Propagate(traced))
                }
                Value::None => Err(ControlAction::Propagate(Value::None)),
                other => Ok(other),
            },
            CanExpr::Unsafe(inner) => self.eval_can(inner),
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

            // Formatting
            CanExpr::FormatWith { expr, spec } => self.eval_format_with(can_id, expr, spec),

            // Error Recovery
            CanExpr::Error => {
                let span = self.can_span(can_id);
                Err(Self::attach_span(parse_error().into(), span))
            }
        }
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

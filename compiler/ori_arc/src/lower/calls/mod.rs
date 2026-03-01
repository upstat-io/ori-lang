//! Call and lambda lowering.
//!
//! Lowers function calls (direct, method) and lambda expressions.
//! Lambda bodies become separate [`ArcFunction`]s with captures as
//! leading parameters, and the call site emits `PartialApply` to
//! pack the captured variables into a closure.
//!
//! Named-argument call variants (`CallNamed`, `MethodCallNamed`) are
//! eliminated during canonicalization — all calls here use positional args.

mod lambda;

use ori_ir::canon::{CanExpr, CanId, CanRange};
use ori_ir::{Name, Span};
use ori_types::{Idx, Tag};

use crate::ir::{ArcVarId, CtorKind};

use super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    // Nounwind classification

    /// Check if a function name refers to a nounwind call.
    ///
    /// Runtime functions (`ori_*`) and compiler-internal helpers (`__*`)
    /// are known to never unwind. User-defined functions may panic, so
    /// they require `Invoke` terminators for cleanup.
    fn is_nounwind_call(&self, name: Name) -> bool {
        let s = self.interner.lookup(name);
        s.starts_with("ori_") || s.starts_with("__")
    }

    /// Emit either Apply (nounwind) or Invoke (may-unwind) for a direct call.
    fn emit_call_or_invoke(
        &mut self,
        ty: Idx,
        name: Name,
        args: Vec<ArcVarId>,
        span: Span,
    ) -> ArcVarId {
        if self.is_nounwind_call(name) {
            self.builder.emit_apply(ty, name, args, Some(span))
        } else {
            self.builder.emit_invoke(ty, name, args, Some(span))
        }
    }

    // Call (positional -- named args already desugared)

    /// Lower a function call expression to ARC IR.
    pub(crate) fn lower_call(
        &mut self,
        func: CanId,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let func_kind = *self.arena.kind(func);

        // Lower all arguments first.
        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();
        let arg_vars: Vec<_> = arg_ids.iter().map(|&id| self.lower_expr(id)).collect();

        match func_kind {
            CanExpr::FunctionRef(name) => {
                // Variant constructor: emit Construct instead of function call
                if let Some(&(enum_name, variant_idx, _)) = self.variant_ctors.get(&name) {
                    tracing::trace!(
                        variant = self.name_str(name),
                        enum_name = self.name_str(enum_name),
                        "call: enum variant constructor (FunctionRef)"
                    );
                    return self.builder.emit_construct(
                        ty,
                        CtorKind::EnumVariant {
                            enum_name,
                            variant: variant_idx,
                        },
                        arg_vars,
                        Some(span),
                    );
                }
                tracing::trace!(
                    func = self.name_str(name),
                    args = arg_vars.len(),
                    "call: direct (FunctionRef)"
                );
                self.emit_call_or_invoke(ty, name, arg_vars, span)
            }
            CanExpr::Ident(name) if self.scope.lookup(name).is_some() => {
                // Local variable holding a closure — indirect call through
                // the closure fat pointer {fn_ptr, env_ptr}.
                let closure_var = self.lower_expr(func);
                tracing::trace!(
                    func = self.name_str(name),
                    closure = closure_var.raw(),
                    args = arg_vars.len(),
                    "call: indirect (local closure)"
                );
                self.builder
                    .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
            }
            CanExpr::Ident(name) => {
                // Variant constructor: emit Construct instead of function call
                if let Some(&(enum_name, variant_idx, _)) = self.variant_ctors.get(&name) {
                    tracing::trace!(
                        variant = self.name_str(name),
                        enum_name = self.name_str(enum_name),
                        "call: enum variant constructor (Ident)"
                    );
                    return self.builder.emit_construct(
                        ty,
                        CtorKind::EnumVariant {
                            enum_name,
                            variant: variant_idx,
                        },
                        arg_vars,
                        Some(span),
                    );
                }
                // Not in local scope — top-level function reference.
                tracing::trace!(
                    func = self.name_str(name),
                    args = arg_vars.len(),
                    "call: direct (Ident)"
                );
                // Inline tag-check builtins: canonicalization desugars
                // `r.is_err()` to `is_err(r)` — a direct Call. These are
                // compiled inline (tag extraction + compare) and don't
                // participate in Perceus ownership.
                if arg_vars.len() == 1 {
                    let arg_ty = self.expr_type(arg_ids[0]);
                    if let Some(var) = self.emit_tag_check(name, arg_vars[0], arg_ty, span) {
                        return var;
                    }
                }
                self.emit_call_or_invoke(ty, name, arg_vars, span)
            }
            _ => {
                // Other expressions (field access, method result, etc.) —
                // evaluate to get closure value, then indirect call.
                let closure_var = self.lower_expr(func);
                tracing::trace!(
                    closure = closure_var.raw(),
                    args = arg_vars.len(),
                    "call: indirect"
                );
                self.builder
                    .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
            }
        }
    }

    // Method call (positional -- named args already desugared)

    /// Lower a method call expression to ARC IR.
    ///
    /// For type-qualified calls (receiver is `TypeRef`, e.g. `Point.default()`),
    /// the receiver is NOT passed as an argument — these are static/associated
    /// methods with no `self` parameter.
    pub(crate) fn lower_method_call(
        &mut self,
        receiver: CanId,
        method: Name,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let receiver_kind = *self.arena.kind(receiver);
        let is_type_qualified = matches!(receiver_kind, CanExpr::TypeRef(_));

        // Inline lowering for tag-check builtins (is_err, is_ok, is_some,
        // is_none). These are compiled inline by LLVM codegen and don't go
        // through the ARC pipeline, so emitting them as Invoke would cause
        // the Perceus algorithm to transfer ownership to a callee that never
        // Dec's the receiver. Lower as Project + PrimOp instead.
        if !is_type_qualified {
            if let Some(var) = self.try_lower_tag_check(receiver, method, span) {
                return var;
            }
        }

        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();

        let mut all_args = if is_type_qualified {
            // Type-qualified call (e.g., `Point.default()`) — no self argument.
            Vec::with_capacity(arg_ids.len())
        } else {
            let recv_var = self.lower_expr(receiver);
            let mut v = Vec::with_capacity(arg_ids.len() + 1);
            v.push(recv_var);
            v
        };

        for &id in &arg_ids {
            all_args.push(self.lower_expr(id));
        }
        tracing::trace!(
            method = self.name_str(method),
            type_qualified = is_type_qualified,
            args = all_args.len(),
            "method_call: dispatch"
        );
        self.emit_call_or_invoke(ty, method, all_args, span)
    }

    /// Emit an inline tag comparison for a Result/Option type.
    ///
    /// Returns `Some(bool_var)` if `method` is a recognized tag-check
    /// builtin (`is_err`, `is_ok`, `is_some`, `is_none`) on a matching type.
    /// The receiver must already be lowered to an `ArcVarId`.
    ///
    /// These builtins are lowered as `Project(tag) == constant` instead
    /// of an Invoke, because they're compiled inline by LLVM codegen and
    /// don't participate in Perceus ownership.
    fn emit_tag_check(
        &mut self,
        method: Name,
        recv_var: ArcVarId,
        recv_ty: Idx,
        span: Span,
    ) -> Option<ArcVarId> {
        let method_str = self.name_str(method);
        let resolved = self.pool.resolve_fully(recv_ty);
        let tag = self.pool.tag(resolved);

        // Result: Ok=0, Err=1. Option: Some=0, None=1.
        let target_tag = match (method_str, tag) {
            ("is_ok", Tag::Result) | ("is_some", Tag::Option) => 0,
            ("is_err", Tag::Result) | ("is_none", Tag::Option) => 1,
            _ => return None,
        };

        let tag_var = self.builder.emit_project(Idx::INT, recv_var, 0, Some(span));
        let tag_const = self.builder.emit_let(
            Idx::INT,
            crate::ir::ArcValue::Literal(crate::ir::LitValue::Int(target_tag)),
            None,
        );
        Some(self.builder.emit_let(
            Idx::BOOL,
            crate::ir::ArcValue::PrimOp {
                op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag_var, tag_const],
            },
            Some(span),
        ))
    }

    /// Try to lower a tag-check method call inline.
    ///
    /// Handles the method call path where the receiver hasn't been
    /// lowered yet. Delegates to [`emit_tag_check`] after lowering.
    fn try_lower_tag_check(
        &mut self,
        receiver: CanId,
        method: Name,
        span: Span,
    ) -> Option<ArcVarId> {
        // Quick pre-check: bail early for non-tag-check methods.
        let s = self.name_str(method);
        if !matches!(s, "is_ok" | "is_err" | "is_some" | "is_none") {
            return None;
        }
        let recv_var = self.lower_expr(receiver);
        let recv_ty = self.expr_type(receiver);
        self.emit_tag_check(method, recv_var, recv_ty, span)
    }
}

// Tests

#[cfg(test)]
mod tests;

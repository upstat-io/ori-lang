//! Call and lambda lowering.
//!
//! Lowers function calls (direct, method) and lambda expressions.
//! Lambda bodies become separate [`ArcFunction`]s.
//!
//! Named-argument call variants (`CallNamed`, `MethodCallNamed`) are
//! eliminated during canonicalization — all calls here use positional args.

use ori_ir::canon::{CanExpr, CanId, CanParamRange, CanRange};
use ori_ir::{Name, Span};
use ori_types::Idx;

use crate::ir::{ArcParam, ArcVarId, CtorKind};
use crate::Ownership;

use super::expr::ArcLowerer;
use super::scope::ArcScope;
use super::ArcIrBuilder;

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
            CanExpr::Ident(name) | CanExpr::FunctionRef(name) => {
                self.emit_call_or_invoke(ty, name, arg_vars, span)
            }
            _ => {
                let closure_var = self.lower_expr(func);
                self.builder
                    .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
            }
        }
    }

    // Method call (positional -- named args already desugared)

    /// Lower a method call expression to ARC IR.
    pub(crate) fn lower_method_call(
        &mut self,
        receiver: CanId,
        method: Name,
        args: CanRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv_var = self.lower_expr(receiver);
        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();
        let mut all_args = Vec::with_capacity(arg_ids.len() + 1);
        all_args.push(recv_var);
        for &id in &arg_ids {
            all_args.push(self.lower_expr(id));
        }
        self.emit_call_or_invoke(ty, method, all_args, span)
    }

    // Lambda

    /// Lower a lambda expression.
    ///
    /// The lambda body becomes a separate `ArcFunction`. The lambda expression
    /// itself produces a `Construct(Closure { func }, captures)`.
    pub(crate) fn lower_lambda(
        &mut self,
        params: CanParamRange,
        body: CanId,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let param_slice: Vec<_> = self.arena.get_params(params).to_vec();
        let lambda_name = self.interner.intern("__lambda");

        // Build the lambda function body.
        let mut lambda_builder = ArcIrBuilder::new();
        let mut lambda_scope = ArcScope::new();
        let mut lambda_params = Vec::with_capacity(param_slice.len());

        for param in &param_slice {
            let param_ty = Idx::UNIT;
            let var = lambda_builder.fresh_var(param_ty);
            lambda_scope.bind(param.name, var);
            lambda_params.push(ArcParam {
                var,
                ty: param_ty,
                ownership: Ownership::Owned,
            });
        }

        let body_ty = self.expr_type(body);
        let entry = lambda_builder.entry_block();

        let mut lambda_problems = Vec::new();
        {
            let mut lambda_lowerer = ArcLowerer {
                builder: &mut lambda_builder,
                arena: self.arena,
                canon: self.canon,
                interner: self.interner,
                pool: self.pool,
                scope: lambda_scope,
                loop_ctx: None,
                problems: &mut lambda_problems,
                lambdas: self.lambdas,
            };
            let result = lambda_lowerer.lower_expr(body);
            if !lambda_lowerer.builder.is_terminated() {
                lambda_lowerer.builder.terminate_return(result);
            }
        }

        self.problems.append(&mut lambda_problems);
        let lambda_func = lambda_builder.finish(lambda_name, lambda_params, body_ty, entry);
        self.lambdas.push(lambda_func);

        self.builder.emit_construct(
            ty,
            CtorKind::Closure { func: lambda_name },
            vec![],
            Some(span),
        )
    }
}

// Tests

#[cfg(test)]
mod tests;

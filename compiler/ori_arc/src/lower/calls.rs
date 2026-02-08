//! Call and lambda lowering.
//!
//! Lowers function calls (direct, named, method) and lambda expressions.
//! Lambda bodies become separate [`ArcFunction`]s.

use ori_ir::ast::ExprKind;
use ori_ir::{CallArgRange, ExprId, ExprRange, Name, ParamRange, Span};
use ori_types::Idx;

use crate::ir::{ArcParam, ArcVarId, CtorKind};
use crate::Ownership;

use super::expr::ArcLowerer;
use super::scope::ArcScope;
use super::ArcIrBuilder;

impl ArcLowerer<'_> {
    // ── Call (positional) ──────────────────────────────────────

    pub(crate) fn lower_call(
        &mut self,
        func: ExprId,
        args: ExprRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let func_expr = self.arena.get_expr(func);

        // Check if the callee is a direct function reference.
        let arg_ids: Vec<_> = self.arena.get_expr_list(args).to_vec();
        let arg_vars: Vec<_> = arg_ids.iter().map(|&id| self.lower_expr(id)).collect();

        match &func_expr.kind {
            ExprKind::Ident(name) | ExprKind::FunctionRef(name) => {
                // Direct call.
                self.builder.emit_apply(ty, *name, arg_vars, Some(span))
            }
            _ => {
                // Indirect call through a closure value.
                let closure_var = self.lower_expr(func);
                self.builder
                    .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
            }
        }
    }

    // ── Call (named arguments) ─────────────────────────────────

    pub(crate) fn lower_call_named(
        &mut self,
        func: ExprId,
        args: CallArgRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let func_expr = self.arena.get_expr(func);
        let call_args: Vec<_> = self.arena.get_call_args(args).to_vec();
        let arg_vars: Vec<_> = call_args
            .iter()
            .map(|arg| self.lower_expr(arg.value))
            .collect();

        match &func_expr.kind {
            ExprKind::Ident(name) | ExprKind::FunctionRef(name) => {
                self.builder.emit_apply(ty, *name, arg_vars, Some(span))
            }
            _ => {
                let closure_var = self.lower_expr(func);
                self.builder
                    .emit_apply_indirect(ty, closure_var, arg_vars, Some(span))
            }
        }
    }

    // ── Method call (positional) ───────────────────────────────

    pub(crate) fn lower_method_call(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
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
        self.builder.emit_apply(ty, method, all_args, Some(span))
    }

    // ── Method call (named arguments) ──────────────────────────

    pub(crate) fn lower_method_call_named(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
        ty: Idx,
        span: Span,
    ) -> ArcVarId {
        let recv_var = self.lower_expr(receiver);
        let call_args: Vec<_> = self.arena.get_call_args(args).to_vec();
        let mut all_args = Vec::with_capacity(call_args.len() + 1);
        all_args.push(recv_var);
        for arg in &call_args {
            all_args.push(self.lower_expr(arg.value));
        }
        self.builder.emit_apply(ty, method, all_args, Some(span))
    }

    // ── Lambda ─────────────────────────────────────────────────

    /// Lower a lambda expression.
    ///
    /// The lambda body becomes a separate `ArcFunction`. The lambda expression
    /// itself produces a `Construct(Closure { func }, captures)`.
    pub(crate) fn lower_lambda(
        &mut self,
        params: ParamRange,
        body: ExprId,
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
            // Parameter types come from the function type, not individual type annotations.
            // For lambda lowering, we default to UNIT here — the actual types are
            // refined by the type checker and available through the function type.
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
                expr_types: self.expr_types,
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

        // The lambda expression produces a Closure construct.
        // For now, captures are empty — a capture analysis pass will fill them.
        self.builder.emit_construct(
            ty,
            CtorKind::Closure { func: lambda_name },
            vec![],
            Some(span),
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::ast::{Expr, ExprKind};
    use ori_ir::{ExprArena, Name, Span, StringInterner};
    use ori_types::Idx;
    use ori_types::Pool;

    use crate::ir::ArcInstr;

    #[test]
    fn lower_direct_call() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let func_name = Name::from_raw(200);
        let func_ref = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 1)));
        let arg = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(2, 4)));
        let args = arena.alloc_expr_list_inline(&[arg]);
        let call = arena.alloc_expr(Expr::new(
            ExprKind::Call {
                func: func_ref,
                args,
            },
            Span::new(0, 5),
        ));

        let max_id = call.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[func_ref.index()] = Idx::INT; // Function type (simplified).
        expr_types[arg.index()] = Idx::INT;
        expr_types[call.index()] = Idx::INT;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::INT,
            call,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        // Should have: let arg=42, Apply(func_name, [arg])
        let has_apply = func.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { .. }));
        assert!(has_apply, "expected Apply instruction");
    }

    #[test]
    fn lower_method_call() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = ExprArena::new();

        let receiver = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let method_name = Name::from_raw(300);
        let args = arena.alloc_expr_list_inline(&[]);
        let method_call = arena.alloc_expr(Expr::new(
            ExprKind::MethodCall {
                receiver,
                method: method_name,
                args,
            },
            Span::new(0, 10),
        ));

        let max_id = method_call.index() + 1;
        let mut expr_types = vec![Idx::ERROR; max_id];
        expr_types[receiver.index()] = Idx::INT;
        expr_types[method_call.index()] = Idx::STR;

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function(
            Name::from_raw(1),
            &[],
            Idx::STR,
            method_call,
            &arena,
            &expr_types,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        // Should have: let recv=1, Apply(method, [recv])
        let has_apply = func.blocks[0].body.iter().any(|i| {
            matches!(i, ArcInstr::Apply { func, args, .. } if args.len() == 1 && *func == method_name)
        });
        assert!(has_apply, "expected method Apply");
    }
}

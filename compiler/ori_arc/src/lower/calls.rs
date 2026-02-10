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
    // ── Nounwind classification ──────────────────────────────

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

    // ── Call (positional — named args already desugared) ──────

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

    // ── Method call (positional — named args already desugared)

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

    // ── Lambda ─────────────────────────────────────────────────

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

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult};
    use ori_ir::{Name, Span, StringInterner, TypeId};
    use ori_types::Idx;
    use ori_types::Pool;

    use crate::ir::{ArcInstr, ArcTerminator};

    /// Helper: lower a Call expression and return the resulting function.
    fn lower_call_expr(
        interner: &StringInterner,
        func_name: Name,
        arg_val: i64,
    ) -> crate::ir::ArcFunction {
        let pool = Pool::new();
        let mut arena = CanArena::with_capacity(100);

        let func_ref = arena.push(CanNode::new(
            CanExpr::Ident(func_name),
            Span::new(0, 1),
            TypeId::from_raw(Idx::INT.raw()),
        ));
        let arg = arena.push(CanNode::new(
            CanExpr::Int(arg_val),
            Span::new(2, 4),
            TypeId::from_raw(Idx::INT.raw()),
        ));
        let args = arena.push_expr_list(&[arg]);
        let call = arena.push(CanNode::new(
            CanExpr::Call {
                func: func_ref,
                args,
            },
            Span::new(0, 5),
            TypeId::from_raw(Idx::INT.raw()),
        ));

        let canon = CanonResult {
            arena,
            constants: ori_ir::canon::ConstantPool::new(),
            decision_trees: ori_ir::canon::DecisionTreePool::default(),
            root: call,
            roots: vec![],
            method_roots: vec![],
        };

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function_can(
            Name::from_raw(1),
            &[],
            Idx::INT,
            call,
            &canon,
            interner,
            &pool,
            &mut problems,
        );
        assert!(problems.is_empty());
        func
    }

    #[test]
    fn user_call_emits_invoke() {
        let interner = StringInterner::new();
        let func_name = interner.intern("my_function");

        let func = lower_call_expr(&interner, func_name, 42);

        let has_invoke = func.blocks.iter().any(|b| {
            matches!(
                &b.terminator,
                ArcTerminator::Invoke { func, .. } if *func == func_name
            )
        });
        assert!(has_invoke, "expected Invoke terminator for user call");

        let has_apply = func.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
        assert!(!has_apply, "user call should not emit Apply");
    }

    #[test]
    fn runtime_call_emits_apply() {
        let interner = StringInterner::new();
        let func_name = interner.intern("ori_print_int");

        let func = lower_call_expr(&interner, func_name, 42);

        let has_apply = func.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
        assert!(has_apply, "expected Apply for runtime call");

        let has_invoke = func.blocks.iter().any(|b| {
            matches!(
                &b.terminator,
                ArcTerminator::Invoke { func, .. } if *func == func_name
            )
        });
        assert!(!has_invoke, "runtime call should not emit Invoke");
    }

    #[test]
    fn compiler_intrinsic_call_emits_apply() {
        let interner = StringInterner::new();
        let func_name = interner.intern("__index");

        let func = lower_call_expr(&interner, func_name, 0);

        let has_apply = func.blocks[0]
            .body
            .iter()
            .any(|i| matches!(i, ArcInstr::Apply { func, .. } if *func == func_name));
        assert!(has_apply, "expected Apply for compiler intrinsic");
    }

    #[test]
    fn invoke_creates_normal_and_unwind_blocks() {
        let interner = StringInterner::new();
        let func_name = interner.intern("my_function");

        let func = lower_call_expr(&interner, func_name, 42);

        assert!(
            func.blocks.len() >= 3,
            "expected at least 3 blocks (entry + normal + unwind), got {}",
            func.blocks.len()
        );

        let invoke_block = func
            .blocks
            .iter()
            .find(|b| matches!(&b.terminator, ArcTerminator::Invoke { .. }));
        assert!(invoke_block.is_some(), "expected an Invoke terminator");

        let has_resume = func
            .blocks
            .iter()
            .any(|b| matches!(&b.terminator, ArcTerminator::Resume));
        assert!(has_resume, "expected Resume terminator in unwind block");
    }

    #[test]
    fn invoke_dst_is_valid_variable() {
        let interner = StringInterner::new();
        let func_name = interner.intern("my_function");

        let func = lower_call_expr(&interner, func_name, 42);

        if let Some(block) = func
            .blocks
            .iter()
            .find(|b| matches!(&b.terminator, ArcTerminator::Invoke { .. }))
        {
            if let ArcTerminator::Invoke { dst, normal, .. } = &block.terminator {
                let normal_block = &func.blocks[normal.index()];
                assert!(
                    matches!(&normal_block.terminator, ArcTerminator::Return { value } if *value == *dst),
                    "expected normal block to return the invoke dst"
                );
            }
        }
    }

    #[test]
    fn lower_method_call_user_defined() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let mut arena = CanArena::with_capacity(100);

        let method_name = interner.intern("to_string");
        let receiver = arena.push(CanNode::new(
            CanExpr::Int(1),
            Span::new(0, 1),
            TypeId::from_raw(Idx::INT.raw()),
        ));
        let args = arena.push_expr_list(&[]);
        let method_call = arena.push(CanNode::new(
            CanExpr::MethodCall {
                receiver,
                method: method_name,
                args,
            },
            Span::new(0, 10),
            TypeId::from_raw(Idx::STR.raw()),
        ));

        let canon = CanonResult {
            arena,
            constants: ori_ir::canon::ConstantPool::new(),
            decision_trees: ori_ir::canon::DecisionTreePool::default(),
            root: method_call,
            roots: vec![],
            method_roots: vec![],
        };

        let mut problems = Vec::new();
        let (func, _) = super::super::super::lower_function_can(
            Name::from_raw(1),
            &[],
            Idx::STR,
            method_call,
            &canon,
            &interner,
            &pool,
            &mut problems,
        );

        assert!(problems.is_empty());
        let has_invoke = func.blocks.iter().any(|b| {
            matches!(
                &b.terminator,
                ArcTerminator::Invoke { func, .. } if *func == method_name
            )
        });
        assert!(has_invoke, "expected Invoke for user-defined method call");
    }
}

//! Call desugaring: `CallNamed` and `MethodCallNamed`.
//!
//! Reorders named arguments to match function/method parameter order,
//! filling omitted parameters with their default expressions.

use ori_ir::canon::CanExpr;
use ori_ir::{CallArgRange, ExprId, Name, Span, TypeId};

use crate::lower::Lowerer;

impl Lowerer<'_> {
    // CallNamed → Call

    /// Desugar `CallNamed { func, args: CallArgRange }` to `Call { func, args: CanRange }`.
    ///
    /// Named arguments are reordered to match the function signature's parameter
    /// order. If the function signature is unavailable (error recovery, lambdas),
    /// arguments are kept in source order.
    pub(crate) fn desugar_call_named(
        &mut self,
        func: ExprId,
        args: CallArgRange,
        span: Span,
        ty: TypeId,
    ) -> ori_ir::canon::CanId {
        let func_kind = *self.src.expr_kind(func);
        let lowered_func = self.lower_expr(func);

        // Get source call arguments (copy out to avoid borrow conflict).
        let src_args = self.src.get_call_args(args);
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_args.iter().map(|a| (a.name, a.value)).collect();

        // Try to resolve the function signature for reordering and default filling.
        let params = self.resolve_func_params(func_kind);

        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args_range = self.arena.push_expr_list(&lowered_args);

        self.push(
            CanExpr::Call {
                func: lowered_func,
                args: args_range,
            },
            span,
            ty,
        )
    }

    // MethodCallNamed → MethodCall

    /// Desugar `MethodCallNamed { receiver, method, args }` to `MethodCall`.
    ///
    /// Same reordering logic as `CallNamed` but looks up the method signature
    /// from `impl_sigs`.
    pub(crate) fn desugar_method_call_named(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
        span: Span,
        ty: TypeId,
    ) -> ori_ir::canon::CanId {
        let lowered_receiver = self.lower_expr(receiver);

        // Get source call arguments.
        let src_args = self.src.get_call_args(args);
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_args.iter().map(|a| (a.name, a.value)).collect();

        // Try to resolve the method signature for reordering and default filling.
        let params = self.resolve_method_params(method);

        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args_range = self.arena.push_expr_list(&lowered_args);

        self.push(
            CanExpr::MethodCall {
                receiver: lowered_receiver,
                method,
                args: args_range,
            },
            span,
            ty,
        )
    }

    /// Reorder named arguments to match parameter order, filling omitted
    /// parameters with their default expressions.
    ///
    /// If `params` is available, arguments with names are placed in the
    /// corresponding parameter position. Unnamed/positional arguments fill
    /// remaining slots left-to-right. Empty slots are filled by lowering the
    /// parameter's default expression. If `params` is `None`, arguments stay
    /// in source order (fallback for lambdas and error recovery).
    fn reorder_and_lower_args(
        &mut self,
        src_args: &[(Option<Name>, ExprId)],
        params: Option<&[(Name, Option<ExprId>)]>,
    ) -> Vec<ori_ir::canon::CanId> {
        match params {
            Some(params) if !params.is_empty() => {
                // Build positional slots matching parameter count.
                let mut slots: Vec<Option<ori_ir::canon::CanId>> = vec![None; params.len()];
                let mut unnamed = Vec::new();

                for &(name, value) in src_args {
                    let lowered = self.lower_expr(value);
                    if let Some(arg_name) = name {
                        // Find the parameter position by name.
                        if let Some(pos) = params.iter().position(|(p, _)| *p == arg_name) {
                            slots[pos] = Some(lowered);
                        } else {
                            // Unknown param name — append as-is (error recovery).
                            unnamed.push(lowered);
                        }
                    } else {
                        unnamed.push(lowered);
                    }
                }

                // Fill empty slots: first try unnamed positional args, then defaults.
                let mut unnamed_iter = unnamed.into_iter();
                for (i, slot) in slots.iter_mut().enumerate() {
                    if slot.is_none() {
                        if let Some(val) = unnamed_iter.next() {
                            *slot = Some(val);
                        } else if let Some(default_expr) = params[i].1 {
                            // Lower the default expression from the function signature.
                            *slot = Some(self.lower_expr(default_expr));
                        }
                    }
                }

                // Collect: all slots (filled by named args, positional args, or defaults),
                // then any remaining unnamed args (error recovery — more args than params).
                let mut result: Vec<ori_ir::canon::CanId> = slots.into_iter().flatten().collect();
                result.extend(unnamed_iter);
                result
            }
            _ => {
                // No signature available — keep source order.
                src_args
                    .iter()
                    .map(|&(_, value)| self.lower_expr(value))
                    .collect()
            }
        }
    }

    /// Try to resolve parameter info (names + defaults) from a function expression.
    fn resolve_func_params(
        &self,
        func_kind: ori_ir::ExprKind,
    ) -> Option<Vec<(Name, Option<ExprId>)>> {
        let (ori_ir::ExprKind::Ident(name) | ori_ir::ExprKind::FunctionRef(name)) = func_kind
        else {
            return None;
        };
        self.typed.function(name).map(|sig| {
            sig.param_names
                .iter()
                .zip(
                    sig.param_defaults
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(None)),
                )
                .map(|(&name, default)| (name, default))
                .collect()
        })
    }

    /// Try to resolve parameter info (names + defaults) from a method signature.
    fn resolve_method_params(&self, method: Name) -> Option<Vec<(Name, Option<ExprId>)>> {
        self.typed
            .impl_sigs
            .iter()
            .find(|(name, _)| *name == method)
            .map(|(_, sig)| {
                sig.param_names
                    .iter()
                    .zip(
                        sig.param_defaults
                            .iter()
                            .copied()
                            .chain(std::iter::repeat(None)),
                    )
                    .map(|(&name, default)| (name, default))
                    .collect()
            })
    }
}

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
        call_expr_id: ExprId,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
        span: Span,
        ty: TypeId,
    ) -> ori_ir::canon::CanId {
        // Get source call arguments (named).
        let src_args: Vec<(Option<Name>, ExprId)> = self
            .src
            .get_call_args(args)
            .iter()
            .map(|a| (a.name, a.value))
            .collect();

        // Module-alias qualified named call (`alias.func(a: v, ...)`): the type
        // checker recorded this call's rewrite target. Lower it as a free `Call`
        // to the qualified imported function — the namespace receiver is dropped
        // (not lowered, not threaded as `self`). This is the path ALL named
        // alias-qualified spec cases take. Mirrors `lower_method_call`.
        if let Some(qualified) = self.typed.resolve_module_alias_call(call_expr_id) {
            return self.lower_module_alias_call(call_expr_id, qualified, &src_args, span, ty);
        }

        // A routed call threads `recv.iter()` as the receiver (typed as the
        // iterator) so the materialized iterator is a real IR node.
        let (lowered_receiver, receiver_ty, adapter_ty) =
            self.lower_method_receiver(call_expr_id, receiver, span);

        // Try to resolve the method signature for reordering and default filling.
        // Pass the (possibly iterator) receiver's type so same-named methods on
        // different impls resolve to their own signature.
        let params = self.resolve_method_params(method, receiver_ty);

        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args_range = self.arena.push_expr_list(&lowered_args);

        let method_ty = adapter_ty.map_or(ty, |idx| TypeId::from_raw(idx.raw()));
        let adapter_call = self.push(
            CanExpr::MethodCall {
                receiver: lowered_receiver,
                method,
                args: args_range,
            },
            span,
            method_ty,
        );
        let can_id = self.finish_eager_iter_adapter(adapter_call, adapter_ty, span, ty);
        // Named-arg method calls desugar to the same positional `MethodCall` as
        // `lower_method_call`; publish the typeck mono-dispatch entry here too so
        // a monomorphized method invoked with named args (e.g. `h.map(f: ...)`)
        // carries its `mono_instance_id` to codegen. Without it the apply falls
        // to the arg-type fallback, which mis-handles closure-typed params.
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// Reorder named arguments to match parameter order, filling omitted
    /// parameters with their default expressions.
    ///
    /// If `params` is available, arguments with names are placed in the
    /// corresponding parameter position. Unnamed/positional arguments fill
    /// remaining slots left-to-right. Empty slots are filled by lowering the
    /// parameter's default expression. If `params` is `None`, arguments stay
    /// in source order (fallback for lambdas and error recovery).
    pub(crate) fn reorder_and_lower_args(
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
    pub(crate) fn resolve_func_params(
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
    ///
    /// `receiver_ty` disambiguates same-named methods across impls: among the impls
    /// defining `method`, the one whose `self` (param 0) type equals the receiver is
    /// preferred, so each receiver fills its OWN defaults. Falls back to the first
    /// name-match when the receiver type is unknown or no `self` type matches
    /// (generic impls, where `self` is a type variable not equal to the concrete
    /// receiver type).
    pub(crate) fn resolve_method_params(
        &self,
        method: Name,
        receiver_ty: Option<ori_types::Idx>,
    ) -> Option<Vec<(Name, Option<ExprId>)>> {
        let self_name = self.interner.intern("self");
        let by_receiver = receiver_ty.and_then(|recv| {
            self.typed.impl_sigs.iter().find(|entry| {
                entry.name == method
                    && entry.sig.param_names.first().copied() == Some(self_name)
                    && entry.sig.param_types.first().copied() == Some(recv)
            })
        });
        by_receiver
            .or_else(|| {
                self.typed
                    .impl_sigs
                    .iter()
                    .find(|entry| entry.name == method)
            })
            .map(|entry| {
                let sig = &entry.sig;
                // A method's `self` receiver is param[0] in the signature, but the
                // call's positional args exclude it (it is the separate receiver).
                // Drop the leading `self` so positional alignment + omitted-default
                // fill in reorder_and_lower_args match the non-self params only —
                // else a provided positional arg lands in `self`'s slot and the
                // first real param is wrongly default-filled.
                let skip = usize::from(sig.param_names.first().copied() == Some(self_name));
                sig.param_names
                    .iter()
                    .zip(
                        sig.param_defaults
                            .iter()
                            .copied()
                            .chain(std::iter::repeat(None)),
                    )
                    .skip(skip)
                    .map(|(&name, default)| (name, default))
                    .collect()
            })
    }
}

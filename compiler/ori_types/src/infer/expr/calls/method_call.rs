//! Method call type inference: `receiver.method(args)`.

mod support;

use ori_ir::{ExprArena, ExprId, Name, Span};

use crate::{Expected, Idx};

use super::super::super::InferEngine;
use super::super::infer_expr;
use super::closure_unify::unify_higher_order_constraints;
use super::constraints::{check_method_inline_bounds, check_method_where_clauses};
use super::impl_lookup::{lookup_impl_method, LookupOutcome};
use super::impl_signature::resolve_impl_signature;
use super::method_diagnostics::{emit_into_not_implemented, emit_unknown_method};
use super::method_receiver::{resolve_receiver_and_builtin, ReceiverDispatch};
use super::module_alias_call;
use super::monomorphization::maybe_record_method_mono_instance;
use support::{
    apply_receiver_type_args, callable_field_fn_ty, check_callable_field_positional,
    check_named_args, check_positional_args,
};

pub(crate) use support::suggest_iterator_fix;

#[derive(Clone, Copy)]
pub(in crate::infer::expr) struct MethodCallSite<'a> {
    call_expr_id: ExprId,
    receiver: ExprId,
    method: Name,
    span: Span,
    expected: Option<&'a Expected>,
}

impl<'a> MethodCallSite<'a> {
    pub(in crate::infer::expr) const fn new(
        call_expr_id: ExprId,
        receiver: ExprId,
        method: Name,
        span: Span,
        expected: Option<&'a Expected>,
    ) -> Self {
        Self {
            call_expr_id,
            receiver,
            method,
            span,
            expected,
        }
    }
}

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Type: Trait { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
pub(in crate::infer::expr) fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    site: MethodCallSite<'_>,
    args: ori_ir::ExprRange,
) -> Idx {
    let MethodCallSite {
        call_expr_id,
        receiver,
        method,
        span,
        expected,
    } = site;

    // Module-alias qualified call `alias.func(args)` (Spec: Clause 18.3.4). Resolved
    // against the aliased module's signature BEFORE ordinary method dispatch so
    // the namespace receiver does not poison to `Idx::ERROR`.
    {
        let arg_ids = arena.get_expr_list(args).to_vec();
        if let Some(ret) = module_alias_call::try_infer_module_alias_call(
            engine,
            arena,
            call_expr_id,
            receiver,
            method,
            &arg_ids,
            span,
        ) {
            if let Some(exp) = expected {
                let _ = engine.check_type(ret, exp, span);
            }
            return ret;
        }
    }

    let resolved =
        match resolve_receiver_and_builtin(engine, arena, call_expr_id, receiver, method, span) {
            ReceiverDispatch::Return {
                ret_ty,
                receiver_ty,
            } => {
                let arg_ids = arena.get_expr_list(args);
                let arg_types: Vec<Idx> = arg_ids
                    .iter()
                    .map(|&arg_id| infer_expr(engine, arena, arg_id))
                    .collect();
                let arg_spans: Vec<Span> = arg_ids
                    .iter()
                    .map(|&arg_id| arena.get_expr(arg_id).span)
                    .collect();
                unify_higher_order_constraints(
                    engine,
                    method,
                    ret_ty,
                    receiver_ty,
                    &arg_types,
                    &arg_spans,
                    span,
                );
                // BD-2: propagate outer expected into builtin ret_ty so a
                // generic-return builtin (e.g. collect's default [T]) unifies
                // with the LHS annotation at the call site.
                if let Some(exp) = expected {
                    let _ = engine.check_type(ret_ty, exp, span);
                }
                return ret_ty;
            }
            ReceiverDispatch::Continue { resolved } => {
                apply_receiver_type_args(engine, arena, resolved, receiver, span)
            }
        };

    let arg_ids = arena.get_expr_list(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    // A genuine `NotFound` (NOT `Ambiguous`, which already pushed E2023)
    // must surface a diagnostic, not silently poison via `Idx::ERROR`.
    let was_not_found = matches!(outcome, LookupOutcome::NotFound);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, arg_ids.len(), span) {
        // BD-2: propagate outer expected into sig.ret BEFORE arg-checking
        // so the generic return slot is constrained by the LHS annotation.
        // Closes the `let e: Error = msg.into()` gap: without this, the
        // generic `T` in `into<T>(self) -> T` stays an unresolved fresh var.
        if let Some(exp) = expected {
            let _ = engine.check_type(sig.ret, exp, span);
        }
        let ret_ty = check_positional_args(engine, arena, arg_ids, &sig, span);
        check_method_where_clauses(
            engine,
            &sig.where_clause_metadata,
            &sig.instantiation_subst,
            span,
        );
        check_method_inline_bounds(
            engine,
            &sig.generic_param_metadata,
            &sig.scheme_var_ids,
            &sig.instantiation_subst,
            span,
        );
        // Record a receiver-bearing MonoInstance for an inherent generic-impl
        // method (`Box<int>.unwrap()`). Runs AFTER arg-checking so the method's
        // instantiation vars are unified; inert for every other dispatch kind.
        maybe_record_method_mono_instance(
            engine,
            Some(call_expr_id),
            method,
            resolved,
            &sig,
            expected,
        );
        return ret_ty;
    }

    // Callable struct field: `s.transform(21)` where `transform: (int) -> int`
    // is a field, not a method. `callable_field_fn_ty` handles this receiver form.
    if let Some(fn_ty) = callable_field_fn_ty(engine, resolved, method) {
        return check_callable_field_positional(engine, arena, fn_ty, arg_ids, span, expected);
    }

    // Error or not found -- infer all args for side effects
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);
    // Surface a method-not-found diagnostic for a genuine NotFound on a
    // diagnosable receiver (concrete / RigidVar) — closes the silent-poison
    // class (concrete-receiver NotFound + rigid-receiver negative case). Skipped for
    // Ambiguous (already emitted) + unresolved Var (deferred) + into.
    if was_not_found {
        emit_unknown_method(engine, resolved, method, span);
    }

    Idx::ERROR
}

/// Infer the type of a named-argument method call: `receiver.method(name: value)`.
pub(in crate::infer::expr) fn infer_method_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    site: MethodCallSite<'_>,
    args: ori_ir::CallArgRange,
) -> Idx {
    let MethodCallSite {
        call_expr_id,
        receiver,
        method,
        span,
        expected,
    } = site;

    // Module-alias qualified call `alias.func(name: value, ...)` (Spec: Clause 18.3.4).
    {
        let call_args = arena.get_call_args(args).to_vec();
        if let Some(ret) = module_alias_call::try_infer_module_alias_call_named(
            engine,
            arena,
            call_expr_id,
            receiver,
            method,
            &call_args,
            span,
        ) {
            if let Some(exp) = expected {
                let _ = engine.check_type(ret, exp, span);
            }
            return ret;
        }
    }

    let resolved =
        match resolve_receiver_and_builtin(engine, arena, call_expr_id, receiver, method, span) {
            ReceiverDispatch::Return {
                ret_ty,
                receiver_ty,
            } => {
                let call_args = arena.get_call_args(args);
                let arg_types: Vec<Idx> = call_args
                    .iter()
                    .map(|arg| infer_expr(engine, arena, arg.value))
                    .collect();
                // Use arena.get_expr(arg.value).span (value-only span) instead of
                // arg.span (whole `name: value` range) so diagnostics anchor at the
                // closure body, not the keyword.
                let arg_spans: Vec<Span> = call_args
                    .iter()
                    .map(|arg| arena.get_expr(arg.value).span)
                    .collect();
                unify_higher_order_constraints(
                    engine,
                    method,
                    ret_ty,
                    receiver_ty,
                    &arg_types,
                    &arg_spans,
                    span,
                );
                // BD-2: propagate outer expected into builtin ret_ty
                // (mirrors infer_method_call positional path).
                if let Some(exp) = expected {
                    let _ = engine.check_type(ret_ty, exp, span);
                }
                return ret_ty;
            }
            ReceiverDispatch::Continue { resolved } => {
                apply_receiver_type_args(engine, arena, resolved, receiver, span)
            }
        };

    let call_args = arena.get_call_args(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    let was_not_found = matches!(outcome, LookupOutcome::NotFound);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, call_args.len(), span) {
        // BD-2: propagate outer expected into sig.ret BEFORE arg-checking.
        if let Some(exp) = expected {
            let _ = engine.check_type(sig.ret, exp, span);
        }
        check_named_args(engine, arena, args, &sig, span);
        check_method_where_clauses(
            engine,
            &sig.where_clause_metadata,
            &sig.instantiation_subst,
            span,
        );
        check_method_inline_bounds(
            engine,
            &sig.generic_param_metadata,
            &sig.scheme_var_ids,
            &sig.instantiation_subst,
            span,
        );
        // Mirror the positional path: record a receiver-bearing MonoInstance for
        // an inherent generic-impl method after named-arg checking.
        maybe_record_method_mono_instance(
            engine,
            Some(call_expr_id),
            method,
            resolved,
            &sig,
            expected,
        );
        return sig.ret;
    }

    // Callable struct field: a closure-typed field invoked through the receiver.
    // `callable_field_fn_ty` handles this receiver form. The closure type carries no parameter names, so
    // check each named arg's value positionally against the closure params.
    if let Some(fn_ty) = callable_field_fn_ty(engine, resolved, method) {
        let value_ids: Vec<ExprId> = call_args.iter().map(|arg| arg.value).collect();
        return check_callable_field_positional(engine, arena, fn_ty, &value_ids, span, expected);
    }

    // Error or not found -- infer all args for side effects
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);
    // Surface a method-not-found diagnostic for a genuine NotFound on a
    // diagnosable receiver (concrete / RigidVar) — closes the silent-poison
    // class (concrete-receiver NotFound + rigid-receiver negative case). Skipped for
    // Ambiguous (already emitted) + unresolved Var (deferred) + into.
    if was_not_found {
        emit_unknown_method(engine, resolved, method, span);
    }

    Idx::ERROR
}

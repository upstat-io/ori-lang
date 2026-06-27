//! Method call type inference: `receiver.method(args)`.

use ori_diagnostic::Suggestion;
use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, lookup_struct_field_types};
use super::closure_unify::unify_higher_order_constraints;
use super::constraints::{check_method_inline_bounds, check_method_where_clauses};
use super::impl_lookup::{lookup_impl_method, ImplMethodSig, LookupOutcome};
use super::impl_signature::resolve_impl_signature;
use super::method_diagnostics::{emit_into_not_implemented, emit_unknown_method};
use super::method_receiver::{resolve_receiver_and_builtin, ReceiverDispatch};
use super::module_alias_call;
use super::monomorphization::maybe_record_method_mono_instance;
use crate::infer::expr::type_resolution::resolve_parsed_type_list;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag};

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Type: Trait { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors ExprKind::MethodCall fields"
)]
pub(crate) fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    receiver: ExprId,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
    // Module-alias qualified call `alias.func(args)` (Spec: Clause 12). Resolved
    // against the aliased module's signature BEFORE ordinary method dispatch so
    // the namespace receiver does not poison to `Idx::ERROR`.
    {
        let arg_ids = arena.get_expr_list(args).to_vec();
        if let Some(ret) = module_alias_call::try_infer_module_alias_call(
            engine, arena, receiver, method, &arg_ids, span,
        ) {
            if let Some(exp) = expected {
                let _ = engine.check_type(ret, exp, span);
            }
            return ret;
        }
    }

    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
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
        // Closes the `let e: Error = msg.into()` gap where the generic T in
        // `into<T>(self) -> T` previously stayed an unresolved fresh var.
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
        maybe_record_method_mono_instance(engine, call_expr_id, method, resolved, &sig);
        return ret_ty;
    }

    // Callable struct field: `s.transform(21)` where `transform: (int) -> int`
    // is a FIELD, not a method (typeck's concrete-receiver dispatch is otherwise
    // incomplete here — see `callable_field_fn_ty`).
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
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors ExprKind::MethodCallNamed fields"
)]
pub(crate) fn infer_method_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
    receiver: ExprId,
    method: Name,
    args: ori_ir::CallArgRange,
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
    // Module-alias qualified call `alias.func(name: value, ...)` (Spec: Clause 12).
    {
        let call_args = arena.get_call_args(args).to_vec();
        if let Some(ret) = module_alias_call::try_infer_module_alias_call_named(
            engine, arena, receiver, method, &call_args, span,
        ) {
            if let Some(exp) = expected {
                let _ = engine.check_type(ret, exp, span);
            }
            return ret;
        }
    }

    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
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
        for (i, (arg, &param_ty)) in call_args.iter().zip(sig.params.iter()).enumerate() {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: None,
                        arg_index: i,
                        param_name: arg.name,
                    },
                },
            };
            let arg_ty = infer_expr(engine, arena, arg.value);
            let _ = engine.check_type(arg_ty, &expected, arg.span);
        }
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
        maybe_record_method_mono_instance(engine, call_expr_id, method, resolved, &sig);
        return sig.ret;
    }

    // Callable struct field: a closure-typed field invoked through the receiver
    // (typeck's concrete-receiver dispatch is otherwise incomplete here — see
    // `callable_field_fn_ty`). The closure type carries no parameter names, so
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

/// If `receiver_ty` is a struct carrying a field named `method` whose type is a
/// closure (`Tag::Function`), return that field's resolved function type.
///
/// typeck's concrete-receiver method dispatch is otherwise incomplete for
/// callable struct fields — `s.transform(21)` where `transform: (int) -> int`
/// is a FIELD, not a method. Without resolving it here, the method call silently
/// poisons to `Idx::ERROR` (no diagnostic), and only the evaluator's dynamic
/// dispatch produces the right value. The LLVM backend needs the real return
/// type in the typed IR (the ARC lowerer projects the field and emits an
/// indirect call). Mirrors `ori_eval` callable-struct-field dispatch.
fn callable_field_fn_ty(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> Option<Idx> {
    let (type_name, type_args) = match engine.pool().tag(receiver_ty) {
        Tag::Named => (engine.pool().named_name(receiver_ty), None),
        Tag::Applied => (
            engine.pool().applied_name(receiver_ty),
            Some(engine.pool().applied_args(receiver_ty)),
        ),
        _ => return None,
    };
    let fields = lookup_struct_field_types(engine, type_name, type_args.as_deref())?;
    let field_ty = engine.resolve(*fields.get(&method)?);
    (engine.pool().tag(field_ty) == Tag::Function).then_some(field_ty)
}

/// Check positional call args against a closure field's parameter types and
/// return the closure's return type. Shared by the positional method-call path.
fn check_callable_field_positional(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    fn_ty: Idx,
    arg_ids: &[ExprId],
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
    let params = engine.pool().function_params(fn_ty);
    let ret = engine.pool().function_return(fn_ty);
    for (i, &arg_id) in arg_ids.iter().enumerate() {
        let arg_ty = infer_expr(engine, arena, arg_id);
        if let Some(&param_ty) = params.get(i) {
            let arg_expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: None,
                        arg_index: i,
                        param_name: None,
                    },
                },
            };
            let _ = engine.check_type(arg_ty, &arg_expected, arena.get_expr(arg_id).span);
        }
    }
    if let Some(exp) = expected {
        let _ = engine.check_type(ret, exp, span);
    }
    ret
}

/// Instantiate a primary-position type-path turbofish receiver (`Box<int>.new(...)`)
/// with its parsed receiver type-arguments (recorded in the arena side-table keyed by
/// the receiver `ExprId`), so the receiver type is concrete BEFORE associated-function
/// resolution. Mirrors the `Box<int>` annotation path in `type_resolution.rs`
/// (well-known generics use their dedicated pool constructors). Returns `resolved`
/// unchanged when there are no receiver type-args or the receiver is not a bare
/// nominal type-name (`Tag::Named`).
fn apply_receiver_type_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    resolved: Idx,
    receiver: ExprId,
    _span: Span,
) -> Idx {
    let type_args = arena.receiver_type_args(receiver);
    if type_args.is_empty() {
        return resolved;
    }
    let arg_idxs = resolve_parsed_type_list(engine, arena, type_args);
    match engine.pool().tag(resolved) {
        // The receiver type-name already instantiated to `Applied(Box, [fresh_var])`
        // (the generic struct's params became fresh inference vars). Bind each fresh
        // arg-var to the explicit turbofish arg so the receiver is concrete BEFORE
        // associated-function resolution; the value-arg check then sees the bound type.
        Tag::Applied => {
            let existing = engine.pool().applied_args(resolved);
            for (&recv_arg, &explicit) in existing.iter().zip(arg_idxs.iter()) {
                let _ = engine.unify_types(recv_arg, explicit);
            }
            resolved
        }
        // A bare nominal type-name with no instantiated args: build `Applied(base, args)`
        // matching the `Box<int>` annotation path (well-known generics use their
        // dedicated pool constructors so the entry matches an annotation's).
        Tag::Named => {
            let base_name = engine.pool().named_name(resolved);
            let wk = engine.well_known();
            let resolved_wk = if let Some(wk) = wk {
                wk.resolve_generic(engine.pool_mut(), base_name, &arg_idxs)
            } else {
                None
            };
            resolved_wk.unwrap_or_else(|| engine.pool_mut().applied(base_name, &arg_idxs))
        }
        _ => resolved,
    }
}

/// Type-check positional method call arguments against resolved param types.
fn check_positional_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    arg_ids: &[ExprId],
    sig: &ImplMethodSig,
    span: Span,
) -> Idx {
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(sig.params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }
    sig.ret
}

/// Tag-specialized fix suggestion for the `flat_map` closure-return diagnostic.
///
/// Queries `ori_registry` to determine whether the actual closure-return
/// tag has a callable `.iter()` method that yields `Iterator<U>`. When yes,
/// the suggestion points users straight at that fix. When no (or when the
/// tag has no registry mapping — type variables, named types, projections,
/// etc.), the suggestion falls back to the generic "wrap in iterator" message.
pub(crate) fn suggest_iterator_fix(inner_tag: Tag) -> Suggestion {
    use super::super::registry_bridge::tag_to_type_tag;
    let has_iter = tag_to_type_tag(inner_tag)
        .and_then(|tt| ori_registry::find_method(tt, "iter"))
        .is_some();
    let text = if has_iter {
        "this type is not an Iterator; call `.iter()` on it (e.g., `[x, x * 10].iter()`)"
    } else {
        "this type is not an Iterator; `flat_map` requires the closure to return an iterator type"
    };
    Suggestion::text(text, 1)
}

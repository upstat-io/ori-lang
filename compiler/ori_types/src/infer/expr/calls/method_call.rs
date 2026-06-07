//! Method call type inference: `receiver.method(args)`.

use ori_diagnostic::Suggestion;
use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, range_method_requires_iteration, resolve_builtin_method};
use super::closure_unify::unify_higher_order_constraints;
use super::constraints::{check_method_inline_bounds, check_method_where_clauses};
use super::impl_lookup::{
    emit_into_not_implemented, lookup_impl_method, resolve_impl_signature, ImplMethodSig,
};
use super::infinite_iterator::check_infinite_iterator_consumed;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError};

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Type: Trait { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
pub(crate) fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
        ReceiverDispatch::Return {
            ret_ty,
            receiver_ty,
        } => {
            let arg_ids: Vec<ExprId> = arena.get_expr_list(args).to_vec();
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
            // §09.5 BD-2: propagate outer expected into builtin ret_ty so a
            // generic-return builtin (e.g. collect's default [T]) unifies
            // with the LHS annotation at the call site.
            if let Some(exp) = expected {
                let _ = engine.check_type(ret_ty, exp, span);
            }
            return ret_ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let arg_ids = arena.get_expr_list(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, arg_ids.len(), span) {
        // §09.5 BD-2: propagate outer expected into sig.ret BEFORE arg-checking
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
        return ret_ty;
    }

    // Error or not found -- infer all args for side effects
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);

    Idx::ERROR
}

/// Infer the type of a named-argument method call: `receiver.method(name: value)`.
pub(crate) fn infer_method_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::CallArgRange,
    span: Span,
    expected: Option<&Expected>,
) -> Idx {
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
            // Per Plan TPR finding A4: use arena.get_expr(arg.value).span
            // (value-only span) instead of arg.span (whole `name: value` range)
            // so diagnostics anchor at the closure body, not the keyword.
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
            // §09.5 BD-2: propagate outer expected into builtin ret_ty
            // (mirrors infer_method_call positional path).
            if let Some(exp) = expected {
                let _ = engine.check_type(ret_ty, exp, span);
            }
            return ret_ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let call_args = arena.get_call_args(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, call_args.len(), span) {
        // §09.5 BD-2: propagate outer expected into sig.ret BEFORE arg-checking.
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
        return sig.ret;
    }

    // Error or not found -- infer all args for side effects
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);

    Idx::ERROR
}

/// Result of resolving a method receiver and checking builtin dispatch.
enum ReceiverDispatch {
    /// Return this type. Caller must infer all args first.
    /// `receiver_ty` is the resolved receiver, needed for higher-order constraint propagation.
    Return { ret_ty: Idx, receiver_ty: Idx },
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
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

/// Resolve the receiver type and try builtin method dispatch.
///
/// Handles: receiver inference, error propagation, scheme instantiation,
/// type-variable deferral, builtin method lookup, `DoubleEndedIterator`
/// gating, and `Range<float>` iteration rejection.
///
/// Returns `Return(ty)` for early results (caller should infer all args
/// and return the type). Returns `Continue { resolved }` to proceed
/// with impl method lookup.
fn resolve_receiver_and_builtin(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    span: Span,
) -> ReceiverDispatch {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    // Propagate errors silently
    if resolved == Idx::ERROR {
        return ReceiverDispatch::Return {
            ret_ty: Idx::ERROR,
            receiver_ty: Idx::ERROR,
        };
    }

    // If receiver is a scheme, instantiate it to get the concrete type
    let resolved = if engine.pool().tag(resolved) == Tag::Scheme {
        engine.instantiate(resolved)
    } else {
        resolved
    };

    // For unresolved type variables, defer resolution UNLESS the var has
    // registered trait bounds (§10.1 bound-chain dispatch on top-level
    // function type-params, which use `pool.fresh_named_var` and surface
    // as `Tag::Var` rather than `Tag::RigidVar`). Bounded vars must
    // continue through `lookup_impl_method` so the bound chain runs;
    // otherwise the early-return masks the dispatch.
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        let has_bounds = engine.rigid_var_bounds(resolved).is_some();
        if !has_bounds {
            return ReceiverDispatch::Return {
                ret_ty: engine.pool_mut().fresh_var(),
                receiver_ty: resolved,
            };
        }
    }

    let method_str = engine.lookup_name(method);

    // 1. Try built-in method resolution
    if let Some(name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            // 1a. Before returning, check for infinite iterator consumption
            if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
                check_infinite_iterator_consumed(engine, arena, receiver, name_str, span);
            }
            return ReceiverDispatch::Return {
                ret_ty: ret,
                receiver_ty: resolved,
            };
        }
    }

    // 1b. Reject DoubleEndedIterator methods on plain Iterator receivers
    if tag == Tag::Iterator {
        if let Some(name_str) = method_str {
            if ori_registry::is_dei_only(name_str) {
                engine.push_error(TypeCheckError::unsatisfied_bound(
                    span,
                    format!(
                        "`{name_str}` requires a DoubleEndedIterator, \
                         but this is an Iterator (use .iter() on a list, range, \
                         or string to get a DoubleEndedIterator)"
                    ),
                ));
                return ReceiverDispatch::Return {
                    ret_ty: Idx::ERROR,
                    receiver_ty: Idx::ERROR,
                };
            }
        }
    }

    // 1c. Reject ALL methods on Range<float> — ranges are int-only per spec.
    // Float range construction is now rejected in infer_range(), but this guard
    // is defense-in-depth in case Range<float> is constructed through other means.
    // Iteration methods get a specific diagnostic; other methods get a generic
    // "Range<float> not supported" error with a diagnostic (not silent).
    if tag == Tag::Range && engine.pool().range_elem(resolved) == Idx::FLOAT {
        if let Some(err) = check_range_float_iteration(engine, resolved, tag, method_str, span) {
            return ReceiverDispatch::Return {
                ret_ty: err,
                receiver_ty: resolved,
            };
        }
        // Non-iteration methods: emit diagnostic, then return error
        engine.push_error(TypeCheckError::range_float_not_constructible(span));
        return ReceiverDispatch::Return {
            ret_ty: Idx::ERROR,
            receiver_ty: resolved,
        };
    }

    ReceiverDispatch::Continue { resolved }
}

/// Check if a method call on a `Range<float>` is attempting iteration.
///
/// Returns `Some(Idx::ERROR)` with a diagnostic pushed if the method
/// is an iteration method and the range element type is `float`.
/// Returns `None` if the check doesn't apply.
fn check_range_float_iteration(
    engine: &mut InferEngine<'_>,
    resolved: Idx,
    tag: Tag,
    method_str: Option<&str>,
    span: Span,
) -> Option<Idx> {
    if tag != Tag::Range {
        return None;
    }
    let name_str = method_str?;
    if !range_method_requires_iteration(name_str) {
        return None;
    }
    let elem = engine.pool().range_elem(resolved);
    if elem != Idx::FLOAT {
        return None;
    }
    engine.push_error(TypeCheckError::range_float_not_iterable(
        span,
        "(0..10).iter().map((i) -> i.to_float() / 10.0)",
    ));
    Some(Idx::ERROR)
}

/// Tag-specialized fix suggestion for the `flat_map` closure-return diagnostic.
///
/// Queries `ori_registry` to determine whether the actual closure-return
/// tag has a callable `.iter()` method that yields `Iterator<U>`. When yes,
/// the suggestion points users straight at that fix. When no (or when the
/// tag has no registry mapping — type variables, named types, projections,
/// etc.), the suggestion falls back to the generic "wrap in iterator" message.
///
/// Replaces the hardcoded tag set (`List | Set | Map | Str | Range | Option`)
/// flagged by Phase 5 Code TPR Round 0 (codex F4 + gemini F2 + opencode F3 —
/// LEAK:scattered-knowledge violation duplicating registry method knowledge).
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

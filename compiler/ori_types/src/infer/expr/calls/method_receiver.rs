//! Method-call receiver resolution + builtin dispatch.
//!
//! Resolves the receiver type, runs builtin method dispatch, and gates the
//! `DoubleEndedIterator` / `Range<float>` cases — returning a `ReceiverDispatch`
//! that tells `infer_method_call` whether to return early or proceed to impl
//! lookup.

use ori_ir::{ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, range_method_requires_iteration, resolve_builtin_method};
use super::infinite_iterator::check_infinite_iterator_consumed;
use super::method_diagnostics::is_named_generic_var;
use crate::{Idx, IterMethodRoute, Tag, TypeCheckError};

/// Result of resolving a method receiver and checking builtin dispatch.
pub(super) enum ReceiverDispatch {
    /// Return this type. Caller must infer all args first.
    /// `receiver_ty` is the resolved receiver, needed for higher-order constraint propagation.
    Return { ret_ty: Idx, receiver_ty: Idx },
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
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
pub(super) fn resolve_receiver_and_builtin(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    call_expr_id: ExprId,
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
    // registered trait bounds (bound-chain dispatch on top-level
    // function type-params, which use `pool.fresh_named_var` and surface
    // as `Tag::Var` rather than `Tag::RigidVar`). Bounded vars must
    // continue through `lookup_impl_method` so the bound chain runs;
    // otherwise the early-return masks the dispatch.
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        let has_bounds = engine.rigid_var_bounds(resolved).is_some();
        if !has_bounds {
            // A NAMED unbound var that is a generic type parameter (`@f<T>(x: T)`,
            // surfaced via `fresh_named_var`, name not a registered trait) has no
            // methods statically, so `x.m()` MUST surface a diagnostic via the
            // NotFound path, NOT defer. An ANONYMOUS unbound var (genuine
            // inference var) and a capability/trait-namespace named var (`Http`,
            // name IS a registered trait) defer — see `is_named_generic_var`.
            if !is_named_generic_var(engine, resolved) {
                return ReceiverDispatch::Return {
                    ret_ty: engine.pool_mut().fresh_var(),
                    receiver_ty: resolved,
                };
            }
            // Named generic with no bound: fall through to `lookup_impl_method`,
            // which returns NotFound → `emit_unknown_method` reports the error.
        }
    }

    let method_str = engine.lookup_name(method);

    // 1. Try built-in method resolution
    if let Some(name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            // Annex C gives direct Range higher-order methods eager result
            // shapes. Keep that public type while recording the iterator
            // implementation path that canonicalization must materialize.
            let mut dispatch_receiver_ty = resolved;
            if tag == Tag::Range {
                if let Some(route) = range_eager_iter_route(engine, resolved, name_str, ret) {
                    dispatch_receiver_ty = route.iter_ty;
                    engine.record_iter_route(call_expr_id, route);
                }
            }
            // 1a. Before returning, check for infinite iterator consumption
            if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
                check_infinite_iterator_consumed(engine, arena, receiver, name_str, span);
            }
            return ReceiverDispatch::Return {
                ret_ty: ret,
                receiver_ty: dispatch_receiver_ty,
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

    // Reject all methods on `Range<float>` because ranges are integer-only.
    // This guard also covers synthetic `Range<float>` values.
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

    // 1d. Iterable fallthrough: a method miss on a type whose `iter`
    // yields an Iterator routes through the Iterator surface. `find_method` is
    // DEI-aware, so DEI-only methods (rev/last/rfind) stay rejected.
    if let Some(name_str) = method_str {
        if let Some(iter_ty) = resolve_builtin_method(engine, resolved, tag, "iter") {
            let iter_tag = engine.pool().tag(iter_ty);
            if matches!(iter_tag, Tag::Iterator | Tag::DoubleEndedIterator) {
                if let Some(ret) = resolve_builtin_method(engine, iter_ty, iter_tag, name_str) {
                    // Record the route so `ori_canon` lowers this to
                    // `recv.iter().method(args)`; the exact call `ExprId`
                    // prevents nested routes from aliasing one another.
                    engine.record_iter_route(
                        call_expr_id,
                        IterMethodRoute {
                            iter_ty,
                            adapter_ty: None,
                        },
                    );
                    return ReceiverDispatch::Return {
                        ret_ty: ret,
                        receiver_ty: iter_ty,
                    };
                }
            }
        }
    }

    ReceiverDispatch::Continue { resolved }
}

/// Build the canonical iterator route for Annex C's direct eager Range methods.
///
/// The type checker owns this projection because it knows both the public result
/// shape and the intermediate adapter element type. Canon and LLVM
/// consume the frozen route without rediscovering Range semantics.
fn range_eager_iter_route(
    engine: &mut InferEngine<'_>,
    range_ty: Idx,
    method: &str,
    ret_ty: Idx,
) -> Option<IterMethodRoute> {
    if !matches!(method, "map" | "filter" | "fold") {
        return None;
    }

    let range_elem = engine.pool().range_elem(range_ty);
    let iter_ty = engine.pool_mut().double_ended_iterator(range_elem);
    let adapter_ty = match method {
        "map" => {
            let resolved_ret = engine.resolve(ret_ty);
            if engine.pool().tag(resolved_ret) != Tag::List {
                return None;
            }
            let mapped_elem = engine.pool().list_elem(resolved_ret);
            Some(engine.pool_mut().double_ended_iterator(mapped_elem))
        }
        "filter" => Some(iter_ty),
        "fold" => None,
        _ => unreachable!("method guard admits only Range map/filter/fold"),
    };

    Some(IterMethodRoute {
        iter_ty,
        adapter_ty,
    })
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

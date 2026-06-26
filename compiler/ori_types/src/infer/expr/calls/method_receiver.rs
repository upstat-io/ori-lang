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
use crate::{Idx, Tag, TypeCheckError};

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

//! Closure-argument unification helpers for higher-order iterator adapters.
//!
//! `unify_higher_order_constraints` dispatches per-method to the right
//! closure-unification routine for `map`, `flat_map`, `filter`, `any`,
//! `all`, `find`, `for_each`, `fold`, `rfold`. The two helpers below it
//! (`unify_closure_param_with_iterator_elem`, `unify_flat_map_constraints`)
//! handle the structural unification of the closure's params/return
//! against the iterator's element / result types.
//!
//! Extracted from `method_call.rs` to keep modules under the 500-line cap so
//! lambda-param propagation extensions to `unify_closure_param_with_iterator_elem`
//! can land without growing `method_call.rs` past the limit.

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use super::method_call::suggest_iterator_fix;
use crate::type_error::ErrorContext;
use crate::{ContextKind, Idx, Tag, TypeCheckError};

/// Unify fresh type variables in builtin method return types with constraints
/// from closure arguments.
pub(super) fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    method: Name,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
) {
    debug_assert_eq!(
        arg_types.len(),
        arg_spans.len(),
        "arg_types and arg_spans must be parallel (one entry per arg)"
    );

    let Some(method_str) = engine.lookup_name(method) else {
        return;
    };

    match method_str {
        "map" => {
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            let resolved_ret = engine.resolve(ret_ty);
            if !engine.pool().tag(resolved_ret).is_iterator() {
                return;
            }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let _ = engine.unify().unify(elem_var, closure_ret);
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        "flat_map" => {
            unify_flat_map_constraints(
                engine,
                ret_ty,
                receiver_ty,
                arg_types,
                arg_spans,
                call_span,
            );
        }
        "filter" | "any" | "all" | "find" | "for_each" => {
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        "fold" | "rfold" => {
            if let Some(&init_ty) = arg_types.first() {
                let _ = engine.unify().unify(ret_ty, init_ty);
            }
            if let Some(&closure_ty) = arg_types.get(1) {
                let resolved_closure = engine.resolve(closure_ty);
                if engine.pool().tag(resolved_closure) == Tag::Function {
                    let closure_ret = engine.pool().function_return(resolved_closure);
                    let _ = engine.unify().unify(ret_ty, closure_ret);
                    let params = engine.pool().function_params(resolved_closure);
                    if let Some(&first_param) = params.first() {
                        let _ = engine.unify().unify(first_param, ret_ty);
                    }
                    // Bind the closure's second param (the element) to the
                    // receiver's element type via the shared SSOT — covers
                    // `Tag::Map` ((K, V) tuple) and `Tag::Str` (char) at parity
                    // with `unify_closure_param_with_iterator_elem`.
                    if let Some(elem) = receiver_element_type(engine, receiver_ty) {
                        if let Some(&second_param) = params.get(1) {
                            let _ = engine.unify().unify(second_param, elem);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Compute the per-element type a higher-order closure parameter binds to
/// for a given receiver.
///
/// SSOT for both the first-param bind in
/// `unify_closure_param_with_iterator_elem` and the second-param (element)
/// bind in the `fold`/`rfold` arm of `unify_higher_order_constraints`.
/// Covers the full adapter-receiver surface: iterator / `Tag::List` /
/// `Tag::Set` / `Tag::Map` (synthetic `(K, V)` tuple — Map iteration shape) /
/// `Tag::Str` (`Idx::CHAR`). Returns `None` for receivers with no element
/// shape (the closure param stays an unconstrained `Tag::Var`).
fn receiver_element_type(engine: &mut InferEngine<'_>, receiver_ty: Idx) -> Option<Idx> {
    let resolved_recv = engine.resolve(receiver_ty);
    let recv_tag = engine.pool().tag(resolved_recv);
    if recv_tag.is_iterator() {
        Some(engine.pool().iterator_elem(resolved_recv))
    } else if recv_tag == Tag::List {
        Some(engine.pool().list_elem(resolved_recv))
    } else if recv_tag == Tag::Set {
        Some(engine.pool().set_elem(resolved_recv))
    } else if recv_tag == Tag::Map {
        // INVARIANT: Map iteration shape is `(K, V)` tuples.
        let k = engine.pool().map_key(resolved_recv);
        let v = engine.pool().map_value(resolved_recv);
        Some(engine.pool_mut().tuple(&[k, v]))
    } else if recv_tag == Tag::Str {
        Some(Idx::CHAR)
    } else {
        None
    }
}

/// Unify a closure's first parameter with the source receiver's element type.
///
/// For adapters like `.map(r -> r.score)`, ensures that `r` is constrained to
/// the receiver's element type rather than remaining as an unresolved type variable.
///
/// Applies to `Tag::List`, `Tag::Set`, `Tag::Map`, `Tag::Str` receivers in
/// addition to the original `Tag::Iterator` / `Tag::DoubleEndedIterator`
/// gate (via `receiver_element_type`). Closes the lambda-parameter inference
/// gap that previously left `list.map(x -> x + 1)` with an unresolved
/// `Tag::Var` for `x`. Map receivers project the `(K, V)` iteration shape as a
/// synthetic tuple so `kvs.map(kv -> kv.0)` resolves `kv: (K, V)`.
pub(super) fn unify_closure_param_with_iterator_elem(
    engine: &mut InferEngine<'_>,
    resolved_closure: Idx,
    receiver_ty: Idx,
) {
    let Some(source_elem) = receiver_element_type(engine, receiver_ty) else {
        return;
    };
    let params = engine.pool().function_params(resolved_closure);
    if let Some(&first_param) = params.first() {
        let _ = engine.unify().unify(first_param, source_elem);
    }
}

/// Unify the result-iterator's element variable for `flat_map` against the
/// closure's inner-iterator return type, with full diagnostic + absorb
/// handling for non-iterator returns and divergent (`Never`) bodies.
pub(super) fn unify_flat_map_constraints(
    engine: &mut InferEngine<'_>,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
) {
    let Some(&closure_ty) = arg_types.first() else {
        return;
    };
    let closure_span = arg_spans.first().copied().unwrap_or(call_span);
    let resolved_ret = engine.resolve(ret_ty);
    if !engine.pool().tag(resolved_ret).is_iterator() {
        return;
    }
    let elem_var = engine.pool().iterator_elem(resolved_ret);
    let resolved_closure = engine.resolve(closure_ty);
    if engine.pool().tag(resolved_closure) != Tag::Function {
        return;
    }

    unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);

    let closure_ret = engine.pool().function_return(resolved_closure);
    let resolved_inner = engine.resolve(closure_ret);
    let inner_tag = engine.pool().tag(resolved_inner);

    if engine.pool().flags(resolved_inner).has_errors() {
        return;
    }

    if inner_tag.is_iterator() {
        let inner_elem = engine.pool().iterator_elem(resolved_inner);
        let _ = engine.unify().unify(elem_var, inner_elem);
    } else if inner_tag == Tag::Never {
        let _ = engine.unify().unify(elem_var, Idx::NEVER);
    } else if matches!(inner_tag, Tag::Var)
        || inner_tag == Tag::Projection
        || inner_tag == Tag::Error
        || inner_tag == Tag::Alias
    {
        // Silent defer / absorb (Var/Projection/Error/Alias): no elem_var bind.
    } else {
        let err = TypeCheckError::unsatisfied_bound(
            closure_span,
            "`flat_map` closure must return an iterator type (`Iterator<U>` or `DoubleEndedIterator<U>`)",
        )
        .with_context(ErrorContext::new(ContextKind::HigherOrderClosureReturn {
            adapter_name: "flat_map",
        }))
        .with_suggestion(suggest_iterator_fix(inner_tag));
        engine.push_error(err);

        let _ = engine.unify().unify(elem_var, Idx::ERROR);
    }
}

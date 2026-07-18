//! Closure-argument unification helpers for higher-order iterator adapters.
//!
//! Per-method dispatch constrains closure parameters and returns against the
//! iterator element and result types.

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
            // Result.map dispatches through the Result-family handler (closure
            // operates on the Ok value); List/Iterator map keep the iterator-shaped
            // logic used by the remaining receiver families.
            let resolved_recv = engine.resolve(receiver_ty);
            if engine.pool().tag(resolved_recv) == Tag::Result {
                unify_result_closure_constraints(engine, "map", ret_ty, resolved_recv, arg_types);
                return;
            }
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            let resolved_ret = engine.resolve(ret_ty);
            let ret_tag = engine.pool().tag(resolved_ret);
            // map's result element comes from either an iterator-shaped return (Iterator/DEI
            // receivers) or a List-shaped return (List receivers, shaped by computed_list_return).
            let elem_var = if ret_tag.is_iterator() {
                engine.pool().iterator_elem(resolved_ret)
            } else if ret_tag == Tag::List {
                engine.pool().list_elem(resolved_ret)
            } else {
                return;
            };
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
                    // Bind the closure's second param (element) to the receiver's
                    // element type via `receiver_element_type` (the shared SSOT).
                    if let Some(elem) = receiver_element_type(engine, receiver_ty) {
                        if let Some(&second_param) = params.get(1) {
                            let _ = engine.unify().unify(second_param, elem);
                        }
                    }
                }
            }
        }
        // Result closure-methods (Result-only method names). The Result-family
        // helper guards on Tag::Result; a Tag::Error poison receiver adds no
        // constraint (the error type stays poison, never destructured).
        "map_err" | "and_then" | "or_else" => {
            let resolved_recv = engine.resolve(receiver_ty);
            if engine.pool().tag(resolved_recv) == Tag::Result {
                unify_result_closure_constraints(
                    engine,
                    method_str,
                    ret_ty,
                    resolved_recv,
                    arg_types,
                );
            }
        }
        _ => {}
    }
}

/// Unify a Result closure-method's closure against the receiver Ok/Err slots and
/// the computed `Result<_,_>` return (BD-2 propagation). Precondition:
/// `receiver_ty` resolves to `Tag::Result` (`result_ok`/`result_err` assert it).
///
/// - `map`/`map_err`: closure param binds receiver Ok/Err; closure return binds
///   the result's Ok (`map`) / Err (`map_err`) slot.
/// - `and_then`/`or_else`: closure param binds receiver Ok/Err; closure return
///   binds the whole computed `Result`.
fn unify_result_closure_constraints(
    engine: &mut InferEngine<'_>,
    method: &str,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
) {
    let Some(&closure_ty) = arg_types.first() else {
        return;
    };
    let resolved_closure = engine.resolve(closure_ty);
    if engine.pool().tag(resolved_closure) != Tag::Function {
        return;
    }
    let resolved_ret = engine.resolve(ret_ty);
    if engine.pool().tag(resolved_ret) != Tag::Result {
        return;
    }
    let closure_ret = engine.pool().function_return(resolved_closure);
    let first_param = engine
        .pool()
        .function_params(resolved_closure)
        .first()
        .copied();

    match method {
        // closure param = receiver Ok; closure return = result Ok slot
        "map" => {
            if let Some(param) = first_param {
                let recv_ok = engine.pool().result_ok(receiver_ty);
                let _ = engine.unify().unify(param, recv_ok);
            }
            let ret_ok = engine.pool().result_ok(resolved_ret);
            let _ = engine.unify().unify(closure_ret, ret_ok);
        }
        // closure param = receiver Err; closure return = result Err slot
        "map_err" => {
            if let Some(param) = first_param {
                let recv_err = engine.pool().result_err(receiver_ty);
                let _ = engine.unify().unify(param, recv_err);
            }
            let ret_err = engine.pool().result_err(resolved_ret);
            let _ = engine.unify().unify(closure_ret, ret_err);
        }
        // closure param = receiver Ok; closure return = whole computed Result
        "and_then" => {
            if let Some(param) = first_param {
                let recv_ok = engine.pool().result_ok(receiver_ty);
                let _ = engine.unify().unify(param, recv_ok);
            }
            let _ = engine.unify().unify(closure_ret, resolved_ret);
        }
        // closure param = receiver Err; closure return = whole computed Result
        "or_else" => {
            if let Some(param) = first_param {
                let recv_err = engine.pool().result_err(receiver_ty);
                let _ = engine.unify().unify(param, recv_err);
            }
            let _ = engine.unify().unify(closure_ret, resolved_ret);
        }
        _ => {}
    }
}

/// Compute the per-element type a higher-order closure parameter binds to for a
/// given receiver. SSOT for the first-param bind in
/// `unify_closure_param_with_iterator_elem` and the `fold`/`rfold` second-param
/// (element) bind. Covers iterator / `Tag::Range` / `Tag::List` / `Tag::Set` / `Tag::Map`
/// (synthetic `(K, V)` tuple) / `Tag::Str` (`Idx::CHAR`); returns `None` for a
/// receiver with no element shape (the closure param stays an unconstrained `Tag::Var`).
fn receiver_element_type(engine: &mut InferEngine<'_>, receiver_ty: Idx) -> Option<Idx> {
    let resolved_recv = engine.resolve(receiver_ty);
    let recv_tag = engine.pool().tag(resolved_recv);
    if recv_tag.is_iterator() {
        Some(engine.pool().iterator_elem(resolved_recv))
    } else if recv_tag == Tag::Range {
        Some(engine.pool().range_elem(resolved_recv))
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

/// Constrain a closure's first parameter to the receiver's element type, so an
/// adapter like `.map(r -> r.score)` resolves `r` instead of leaving it an
/// unresolved `Tag::Var`. Delegates the per-receiver element type to
/// `receiver_element_type`, covering iterator / `Tag::Range` / `Tag::List` / `Tag::Set` /
/// `Tag::Map` (projecting the `(K, V)` iteration shape as a synthetic tuple so
/// `kvs.map(kv -> kv.0)` resolves `kv: (K, V)`) / `Tag::Str`.
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

//! Computed return types for builtin methods with `ReturnTag::Fresh`.
//!
//! Most methods with `ReturnTag::Fresh` in the registry genuinely return a fresh
//! type variable (higher-order methods where the return depends on a closure
//! argument). A handful need specific type construction for better inference:
//!
//! - `list.zip` — returns `[(_elem_, U)]`, not bare `fresh`
//! - `iter.map` — DEI-aware: returns `DEI<U>` or `Iterator<U>`
//! - `iter.zip` — returns `Iterator<(_elem_, U)>`
//! - `iter.flatten`/`flat_map` — returns `Iterator<U>`
//! - `result.trace_entries` / `error.trace_entries` — returns `[TraceEntry]`
//!
//! Called only when `find_method()` returns a `MethodDef` with
//! `returns == ReturnTag::Fresh`. All other return tags are handled
//! by [`super::super::registry_bridge::return_tag_to_idx`].

use crate::infer::InferEngine;
use crate::{Idx, Tag};

/// Compute the return type for a method whose registry `ReturnTag` is `Fresh`.
///
/// Dispatches to per-type helpers for the few methods that need specific
/// type construction. All other `Fresh` methods get `engine.fresh_var()`.
pub(super) fn resolve_computed_return(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    tag: Tag,
    method_name: &str,
) -> Idx {
    match tag {
        Tag::List => computed_list_return(engine, receiver_ty, method_name),
        Tag::Iterator | Tag::DoubleEndedIterator => {
            computed_iterator_return(engine, receiver_ty, method_name)
        }
        // Spec: Clause 14.3 — Traceable trait: trace_entries() -> [TraceEntry]
        Tag::Result | Tag::Error => match method_name {
            "trace_entries" => computed_trace_entries(engine),
            _ => engine.fresh_var(),
        },
        _ => engine.fresh_var(),
    }
}

/// Construct `[TraceEntry]` for `trace_entries()` on Result and Error.
///
/// Falls back to `fresh_var()` if the interner is not available (isolated tests
/// without full module setup).
fn computed_trace_entries(engine: &mut InferEngine<'_>) -> Idx {
    if let Some(name) = engine.intern_name("TraceEntry") {
        let trace_entry = engine.pool_mut().named(name);
        engine.pool_mut().list(trace_entry)
    } else {
        engine.fresh_var()
    }
}

/// List methods needing constructed return types.
fn computed_list_return(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Idx {
    match method {
        // zip returns List<(T, U)> where U is a fresh var for the other list's element type
        "zip" => {
            let elem = engine.pool().list_elem(receiver_ty);
            let other_elem = engine.fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            engine.pool_mut().list(pair)
        }
        _ => engine.fresh_var(),
    }
}

/// Iterator/DEI methods needing constructed return types.
fn computed_iterator_return(engine: &mut InferEngine<'_>, receiver_ty: Idx, method: &str) -> Idx {
    let is_dei = engine.pool().tag(receiver_ty) == Tag::DoubleEndedIterator;

    match method {
        // map propagates DEI-ness: DEI<T>.map(f) -> DEI<U>, Iterator<T>.map(f) -> Iterator<U>
        "map" => {
            let new_elem = engine.fresh_var();
            if is_dei {
                engine.pool_mut().double_ended_iterator(new_elem)
            } else {
                engine.pool_mut().iterator(new_elem)
            }
        }
        // zip returns Iterator<(T, U)> where U is fresh (other iterator's element type)
        "zip" => {
            let elem = engine.pool().iterator_elem(receiver_ty);
            let other_elem = engine.fresh_var();
            let pair = engine.pool_mut().tuple(&[elem, other_elem]);
            engine.pool_mut().iterator(pair)
        }
        // flatten/flat_map return Iterator<U> where U is fresh (flattened inner element)
        "flatten" | "flat_map" => {
            let new_elem = engine.fresh_var();
            engine.pool_mut().iterator(new_elem)
        }
        _ => engine.fresh_var(),
    }
}

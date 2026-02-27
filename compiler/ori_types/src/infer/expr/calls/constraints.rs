//! Capability and where-clause constraint checking for function calls.

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use super::traits::type_satisfies_trait;
use crate::{Idx, TypeCheckError};

/// Validate that required capabilities are available at a call site.
///
/// Looks up the callee's signature to find its `uses` capabilities,
/// then checks each one against the caller's declared + provided capabilities.
/// Emits `E2014 MissingCapability` for each missing capability.
pub(super) fn check_call_capabilities(
    engine: &mut InferEngine<'_>,
    func_name: Option<Name>,
    span: Span,
) {
    let Some(name) = func_name else { return };
    let Some(sig) = engine.get_signature(name) else {
        return;
    };

    // Collect missing capabilities during immutable borrow
    let missing: Vec<Name> = sig
        .capabilities
        .iter()
        .copied()
        .filter(|&cap| !engine.has_capability(cap))
        .collect();

    if missing.is_empty() {
        return;
    }

    // Push errors in a separate mutable pass
    let available = engine.available_capabilities();
    for cap in missing {
        tracing::debug!(?cap, "missing capability at call site");
        engine.push_error(TypeCheckError::missing_capability(span, cap, &available));
    }
}

/// Validate where-clause constraints for a generic function call.
///
/// After argument type-checking has unified generic type variables with concrete
/// types, this checks constraints like `where C.Item: Eq` by:
/// 1. Resolving the concrete type for the generic param
/// 2. Finding the trait impl that defines the associated type
/// 3. Looking up the projected type
/// 4. Checking the projected type satisfies the required trait bound
///
/// Uses a three-phase approach to satisfy the borrow checker:
/// 1. Mutable phase: resolve types and create pool entries
/// 2. Immutable phase: check trait registry and collect violations
/// 3. Mutable phase: push collected errors
#[expect(
    clippy::too_many_lines,
    reason = "three-phase where clause checking: resolve, collect violations, push errors"
)]
pub(super) fn check_where_clauses(
    engine: &mut InferEngine<'_>,
    func_name: Name,
    params: &[Idx],
    call_span: Span,
) {
    struct PreparedCheck {
        concrete_type: Idx,
        projection: Option<Name>,
        bound_entries: Vec<(Name, Idx)>,
        trait_bound_entries: Vec<Idx>,
    }

    let Some(sig) = engine.get_signature(func_name) else {
        return;
    };

    if sig.where_clauses.is_empty() {
        return;
    }

    // Extract only the fields we need, avoiding a full FunctionSig clone
    let where_clauses = sig.where_clauses.clone();
    let type_params = sig.type_params.clone();
    let type_param_bounds = sig.type_param_bounds.clone();
    let generic_param_mapping = sig.generic_param_mapping.clone();

    // Phase 1 (mutable): Resolve concrete types and create named Idx entries

    let mut prepared = Vec::new();

    for wc in &where_clauses {
        let Some(tp_idx) = type_params.iter().position(|&n| n == wc.param) else {
            continue;
        };
        let Some(Some(param_idx)) = generic_param_mapping.get(tp_idx) else {
            continue;
        };
        let Some(&instantiated_param) = params.get(*param_idx) else {
            continue;
        };
        let concrete_type = engine.resolve(instantiated_param);
        if concrete_type == Idx::ERROR {
            continue;
        }

        // Pre-create named Idx for each bound (needs &mut pool)
        let bound_entries: Vec<(Name, Idx)> = wc
            .bounds
            .iter()
            .map(|&name| (name, engine.pool_mut().named(name)))
            .collect();

        // Pre-create named Idx for type param bounds (for projection lookup)
        let tp_bounds = type_param_bounds.get(tp_idx).cloned().unwrap_or_default();
        let trait_bound_entries: Vec<Idx> = tp_bounds
            .iter()
            .map(|&name| engine.pool_mut().named(name))
            .collect();

        prepared.push(PreparedCheck {
            concrete_type,
            projection: wc.projection,
            bound_entries,
            trait_bound_entries,
        });
    }

    // Phase 2 (immutable): Check trait registry and collect error messages
    let errors = {
        let Some(trait_registry) = engine.trait_registry() else {
            return;
        };
        let pool = engine.pool();
        let well_known = engine.well_known();

        let mut errors: Vec<String> = Vec::new();

        for check in &prepared {
            if let Some(projection) = check.projection {
                // Where-clause with projection: `where C.Item: Eq`
                for &trait_idx in &check.trait_bound_entries {
                    let Some((_, impl_entry)) =
                        trait_registry.find_impl(trait_idx, check.concrete_type)
                    else {
                        continue;
                    };
                    let Some(&projected_type) = impl_entry.assoc_types.get(&projection) else {
                        continue;
                    };
                    for &(bound_name, bound_idx) in &check.bound_entries {
                        if !trait_registry.has_impl(bound_idx, projected_type) {
                            let satisfies = if let Some(wk) = well_known {
                                wk.type_satisfies_trait(projected_type, bound_name, pool)
                            } else {
                                let s = engine.lookup_name(bound_name).unwrap_or("");
                                type_satisfies_trait(projected_type, s, pool)
                            };
                            if !satisfies {
                                let bound_str = engine.lookup_name(bound_name).unwrap_or("?");
                                errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                            }
                        }
                    }
                }
            } else {
                // Direct bound: `where T: Clone`
                for &(bound_name, bound_idx) in &check.bound_entries {
                    if !trait_registry.has_impl(bound_idx, check.concrete_type) {
                        let satisfies = if let Some(wk) = well_known {
                            wk.type_satisfies_trait(check.concrete_type, bound_name, pool)
                        } else {
                            let s = engine.lookup_name(bound_name).unwrap_or("");
                            type_satisfies_trait(check.concrete_type, s, pool)
                        };
                        if !satisfies {
                            let bound_str = engine.lookup_name(bound_name).unwrap_or("?");
                            errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                        }
                    }
                }
            }
        }

        errors
    };

    // Phase 3 (mutable): Push collected errors
    for msg in errors {
        engine.push_error(TypeCheckError::unsatisfied_bound(call_span, msg));
    }
}

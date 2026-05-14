//! Capability and where-clause constraint checking for function calls.

use rustc_hash::FxHashMap;

use ori_ir::{Name, Span};

use super::super::super::InferEngine;
use super::traits::type_satisfies_trait;
use crate::pool::substitute::substitute_in_pool;
use crate::{GenericParamMeta, Idx, Tag, TypeCheckError, TypeFlags, WhereConstraint};

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

/// Validate method-level where-clause constraints at a method call site.
///
/// Mirrors the three-phase borrow-dance shape of `check_where_clauses` for
/// `FunctionSig` top-level generics, but operates on the method's deep-copied
/// `where_clause_metadata: Vec<WhereConstraint>` (registered by
/// `build_method_generic_metadata` in `check/registration/type_resolution.rs`).
///
/// # Substitution model
///
/// Method-level binders are stored as `Tag::Var` (per `fresh_named_var` in
/// registration). Argument unification at the call site (via
/// `check_positional_args`) binds those Vars to concrete arg types, so
/// `engine.resolve(wc.ty)` follows the union-find chain and returns the
/// post-unification concrete type.
///
/// When the resolved type still carries unresolved binders
/// (`HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR`), the constraint is conservatively
/// skipped — best-effort enforcement, mirrors the `TF-5` fast-path gate. A
/// follow-up commit (§05 B.2 inline-bound enforcement) will extend coverage
/// to `generic_param_metadata.bounds` (the `<T: Eq>` inline form).
///
/// # Three phases
/// 1. Mutable resolve: walk each `WhereConstraint`, resolve `ty`, prepare
///    `(concrete_ty, bound_idx)` checks; skip when `concrete_ty` is unresolved.
/// 2. Immutable: query `trait_registry.get_trait_by_idx(bound)` for the trait
///    Name, run `type_satisfies_trait` for primitives or `has_impl` for
///    user-defined types, collect failure messages.
/// 3. Mutable push: emit `E2001` (unsatisfied bound) per failure.
pub(super) fn check_method_where_clauses(
    engine: &mut InferEngine<'_>,
    where_clause_metadata: &[WhereConstraint],
    subst: &FxHashMap<u32, Idx>,
    call_span: Span,
) {
    struct PreparedMethodCheck {
        concrete_type: Idx,
        bound_entries: Vec<(Name, Idx)>,
    }

    if where_clause_metadata.is_empty() {
        return;
    }

    // Phase 1 (mutable): resolve each WhereConstraint's `ty`, skip unresolved.
    // `wc.ty` is the fresh-Var Idx allocated by
    // `build_method_generic_metadata`'s `scheme_overlay` at registration.
    // Call-site `instantiate_with_subst` mapped that scheme Idx to a fresh
    // Var via `subst`; walk wc.ty so resolution sees the post-instantiation
    // chain that arg unification has bound to a concrete type.
    let mut prepared: Vec<PreparedMethodCheck> = Vec::new();
    for wc in where_clause_metadata {
        let instantiated_ty = if subst.is_empty() {
            wc.ty
        } else {
            substitute_in_pool(engine.pool_mut(), wc.ty, subst)
        };
        let concrete_type = engine.resolve(instantiated_ty);
        let flags = engine.pool().flags(concrete_type);
        let unresolved = TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR;
        if flags.intersects(unresolved) {
            // Conservatively skip: substitution incomplete (TF-5 fast-path)
            continue;
        }
        if concrete_type == Idx::ERROR {
            continue;
        }

        // Extract trait Name from each bound Idx (Tag::Named, deep-copied at registration)
        let mut bound_entries: Vec<(Name, Idx)> = Vec::new();
        for &bound_idx in &wc.bounds {
            if engine.pool().tag(bound_idx) != Tag::Named {
                continue;
            }
            let bound_name = engine.pool().named_name(bound_idx);
            bound_entries.push((bound_name, bound_idx));
        }
        if bound_entries.is_empty() {
            continue;
        }

        prepared.push(PreparedMethodCheck {
            concrete_type,
            bound_entries,
        });
    }

    // Phase 2 (immutable): check each (concrete_type, bound) pair
    let errors = {
        let Some(trait_registry) = engine.trait_registry() else {
            return;
        };
        let pool = engine.pool();
        let well_known = engine.well_known();

        let mut errors: Vec<String> = Vec::new();
        for check in &prepared {
            for &(bound_name, bound_idx) in &check.bound_entries {
                if trait_registry.has_impl(bound_idx, check.concrete_type) {
                    continue;
                }
                let satisfies = if let Some(wk) = well_known {
                    wk.type_satisfies_trait(check.concrete_type, bound_name, pool)
                } else {
                    let s = engine.lookup_name(bound_name).unwrap_or("");
                    type_satisfies_trait(check.concrete_type, s, pool)
                };
                if !satisfies {
                    let bound_str = engine.lookup_name(bound_name).unwrap_or("?");
                    errors.push(format!("does not satisfy trait bound `{bound_str}`"));
                }
            }
        }
        errors
    };

    // Phase 3 (mutable): push errors
    for msg in errors {
        engine.push_error(TypeCheckError::unsatisfied_bound(call_span, msg));
    }
}

/// Validate inline trait bounds on method-level type-generic parameters.
///
/// Mirrors the three-phase borrow-dance shape of `check_method_where_clauses`,
/// but operates on the inline `<T: Bound + Bound2>` form via
/// `generic_param_metadata` rather than the trailing `where T: Bound` form via
/// `where_clause_metadata`. Together these two helpers cover both bound surfaces
/// for method-level binders.
///
/// # Substitution model
///
/// `scheme_var_ids` is parallel (in declaration order) to non-const entries of
/// `generic_param_metadata` — it carries each method-level binder's
/// registration-time pool `var_id`. After call-site `instantiate_with_subst`
/// runs, `subst[var_id]` is the fresh `Tag::Var` Idx allocated for that binder.
/// Argument unification (via `check_positional_args`) binds those Vars to
/// concrete types, so `engine.resolve(subst[var_id])` follows the union-find
/// chain and returns the post-unification concrete type.
///
/// When the resolved type still carries unresolved binders
/// (`HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR`), the bound is conservatively
/// skipped — best-effort enforcement, mirrors the `TF-5` fast-path gate
/// precedent set by `check_method_where_clauses`.
///
/// # Three phases
/// 1. Mutable resolve: zip non-const params with `scheme_var_ids`, look up each
///    var's fresh Idx in `subst`, resolve via union-find. Skip when the
///    resolved type is unresolved or `Idx::ERROR`.
/// 2. Immutable: query `trait_registry.has_impl` and `type_satisfies_trait` for
///    each bound; collect failure messages.
/// 3. Mutable push: emit `E2001` (unsatisfied bound) per failure.
pub(super) fn check_method_inline_bounds(
    engine: &mut InferEngine<'_>,
    generic_param_metadata: &[GenericParamMeta],
    scheme_var_ids: &[u32],
    subst: &FxHashMap<u32, Idx>,
    call_span: Span,
) {
    struct PreparedInlineCheck {
        concrete_type: Idx,
        bound_entries: Vec<(Name, Idx)>,
    }

    if generic_param_metadata.is_empty() || subst.is_empty() {
        return;
    }

    // Phase 1 (mutable): zip non-const params with scheme_var_ids in declaration
    // order, look up each binder's fresh Var Idx in subst, resolve to concrete.
    let mut prepared: Vec<PreparedInlineCheck> = Vec::new();
    let mut var_id_iter = scheme_var_ids.iter();
    for param in generic_param_metadata {
        if param.is_const {
            continue;
        }
        let Some(&var_id) = var_id_iter.next() else {
            // scheme_var_ids exhausted before generic_param_metadata — registration drift
            break;
        };
        if param.bounds.is_empty() {
            continue;
        }
        let Some(&fresh_idx) = subst.get(&var_id) else {
            continue;
        };

        let concrete_type = engine.resolve(fresh_idx);
        let flags = engine.pool().flags(concrete_type);
        let unresolved = TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR | TypeFlags::HAS_RIGID_VAR;
        if flags.intersects(unresolved) {
            continue;
        }
        if concrete_type == Idx::ERROR {
            continue;
        }

        let mut bound_entries: Vec<(Name, Idx)> = Vec::new();
        for &bound_idx in &param.bounds {
            if engine.pool().tag(bound_idx) != Tag::Named {
                continue;
            }
            let bound_name = engine.pool().named_name(bound_idx);
            bound_entries.push((bound_name, bound_idx));
        }
        if bound_entries.is_empty() {
            continue;
        }

        prepared.push(PreparedInlineCheck {
            concrete_type,
            bound_entries,
        });
    }

    // Phase 2 (immutable): check each (concrete_type, bound) pair
    let errors = {
        let Some(trait_registry) = engine.trait_registry() else {
            return;
        };
        let pool = engine.pool();
        let well_known = engine.well_known();

        let mut errors: Vec<String> = Vec::new();
        for check in &prepared {
            for &(bound_name, bound_idx) in &check.bound_entries {
                if trait_registry.has_impl(bound_idx, check.concrete_type) {
                    continue;
                }
                let satisfies = if let Some(wk) = well_known {
                    wk.type_satisfies_trait(check.concrete_type, bound_name, pool)
                } else {
                    let s = engine.lookup_name(bound_name).unwrap_or("");
                    type_satisfies_trait(check.concrete_type, s, pool)
                };
                if !satisfies {
                    let bound_str = engine.lookup_name(bound_name).unwrap_or("?");
                    errors.push(format!("does not satisfy trait bound `{bound_str}`"));
                }
            }
        }
        errors
    };

    // Phase 3 (mutable): push errors
    for msg in errors {
        engine.push_error(TypeCheckError::unsatisfied_bound(call_span, msg));
    }
}

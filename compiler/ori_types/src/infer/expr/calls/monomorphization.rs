//! Monomorphization instance recording for generic function calls.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use super::super::super::InferEngine;
use super::super::structs::substitute_named_types;
use crate::pool::substitute::{extract_var_from_types, substitute_in_pool};
use crate::{GenericArg, Idx, MonoInstance, Pool, Tag};

/// Record a monomorphization instance if the callee is a generic function.
///
/// Called after argument type-checking, when all type variables have been unified
/// with concrete types. Extracts concrete type args via `generic_param_mapping`,
/// builds a substitution map from `scheme_var_ids`, and computes the `body_type_map`
/// for the ARC lowerer.
pub(super) fn maybe_record_mono_instance(
    engine: &mut InferEngine<'_>,
    func_name: Option<Name>,
    params: &[Idx],
) {
    let Some(fn_name) = func_name else {
        return;
    };

    // Extract sig data in an immutable borrow scope.
    let (scheme_var_ids, generic_param_mapping, param_types, return_type) = {
        let Some(sig) = engine.get_signature(fn_name) else {
            return;
        };
        if !sig.is_generic() || sig.scheme_var_ids.is_empty() {
            return;
        }
        (
            sig.scheme_var_ids.clone(),
            sig.generic_param_mapping.clone(),
            sig.param_types.clone(),
            sig.return_type,
        )
    };

    // Build the var_id -> resolved_type substitution map.
    let (mut var_subst, generic_args, has_unresolved_vars) = build_mono_var_subst(
        engine,
        &scheme_var_ids,
        &generic_param_mapping,
        &param_types,
        params,
    );

    // All type params must be mapped (even if some are still variables).
    if var_subst.len() != scheme_var_ids.len() {
        return;
    }

    // Deferred case: some type params are still variables (generic calling generic).
    if has_unresolved_vars {
        record_deferred_mono_call(
            engine,
            fn_name,
            &scheme_var_ids,
            &var_subst,
            param_types,
            return_type,
        );
        return;
    }

    // Extend var_subst with root var_ids of equivalence classes so
    // substitute_in_pool can handle root vars from inner instantiations.
    // Threads the declared scheme_var_ids (the cloned Vec from the
    // sig-lookup block above) explicitly, rather than recovering the
    // list from var_subst's keys — the helper's contract is "extend
    // for THESE declared scheme vars" (SSOT per impl-hygiene.md
    // §Algorithmic DRY; shared with the deferred-resolve site in
    // check::exports::resolve_deferred_mono_calls and the JIT imported-mono
    // site in oric::test::runner::imported_mono).
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &scheme_var_ids,
        &mut var_subst,
    );

    // Collect struct type params before taking pool_mut(), so
    // register_concrete_applied_resolutions can build Named->Idx
    // substitutions for struct fields (which use Named tags, not Var tags).
    let struct_type_params: FxHashMap<Name, Vec<Name>> = engine
        .type_registry()
        .map(|tr| {
            tr.iter()
                .filter(|entry| !entry.type_params.is_empty())
                .map(|entry| (entry.name, entry.type_params.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Concrete case: all type params resolved -- build full MonoInstance.
    let pool = engine.pool_mut();
    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, &var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, &var_subst);

    // Build body_type_map via the canonical SSOT helper; sort+dedup for
    // deterministic Eq/Hash (Salsa early cutoff) is call-site-local
    // post-processing per `pool::substitute::build_mono_body_type_map`.
    let mut body_type_map: Vec<(crate::Idx, crate::Idx)> = Vec::new();
    crate::pool::substitute::build_mono_body_type_map(pool, &var_subst, &mut body_type_map);
    body_type_map.sort_by_key(|(k, _)| k.raw());
    body_type_map.dedup_by_key(|(k, _)| k.raw());

    // Register pool resolutions for concrete Applied types so the LLVM
    // TypeInfoStore can resolve them to struct layouts during codegen.
    register_concrete_applied_resolutions(pool, &body_type_map, &struct_type_params);

    let instance = MonoInstance {
        fn_name,
        generic_args,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    };

    tracing::debug!(
        fn_name = ?fn_name,
        args = ?instance.generic_args,
        "recorded mono instance"
    );

    engine.record_mono_instance(instance);
}

/// Build the `var_id` -> `resolved_type` substitution map for monomorphization.
///
/// For each scheme variable, resolves it either directly from function params
/// (when type param maps to a parameter position) or indirectly by structural
/// extraction from generic param types.
///
/// Returns `(var_subst, generic_args, has_unresolved_vars)`.
fn build_mono_var_subst(
    engine: &mut InferEngine<'_>,
    scheme_var_ids: &[u32],
    generic_param_mapping: &[Option<usize>],
    param_types: &[Idx],
    params: &[Idx],
) -> (FxHashMap<u32, Idx>, Vec<GenericArg>, bool) {
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    let mut generic_args = Vec::with_capacity(scheme_var_ids.len());
    let mut has_unresolved_vars = false;

    for (i, &var_id) in scheme_var_ids.iter().enumerate() {
        let Some(concrete) = resolve_scheme_var(
            engine,
            i,
            var_id,
            generic_param_mapping,
            param_types,
            params,
        ) else {
            continue;
        };

        if engine.pool().tag(concrete) == Tag::Var {
            has_unresolved_vars = true;
        }

        var_subst.insert(var_id, concrete);
        generic_args.push(GenericArg::Type(concrete));
    }

    (var_subst, generic_args, has_unresolved_vars)
}

/// Resolve a single scheme variable at position `i` to a concrete type, either
/// directly from a function parameter (when `generic_param_mapping[i]` points
/// at a parameter) or indirectly by structural extraction from the generic
/// parameter types. Returns `None` when no concrete type can be resolved yet
/// — the outer worklist skips the var and revisits on a later iteration.
fn resolve_scheme_var(
    engine: &mut InferEngine<'_>,
    i: usize,
    var_id: u32,
    generic_param_mapping: &[Option<usize>],
    param_types: &[Idx],
    params: &[Idx],
) -> Option<Idx> {
    if let Some(Some(param_idx)) = generic_param_mapping.get(i) {
        // Type param appears directly as a function parameter -- resolve it.
        let &param_ty = params.get(*param_idx)?;
        return Some(engine.resolve(param_ty));
    }

    // Indirect type param (e.g., T in Pair<T, int>) -- extract concrete
    // type by walking generic and concrete param types in parallel.
    extract_indirect_scheme_var(engine, var_id, param_types, params)
}

/// Walk generic and concrete parameter types in parallel looking for a
/// concrete substitution for `var_id`. Resolves the extracted type through
/// link chains (the extracted type may itself be a fresh var unified with a
/// concrete type).
fn extract_indirect_scheme_var(
    engine: &mut InferEngine<'_>,
    var_id: u32,
    param_types: &[Idx],
    params: &[Idx],
) -> Option<Idx> {
    for (j, &param_type) in param_types.iter().enumerate() {
        let Some(&actual) = params.get(j) else {
            continue;
        };
        let Some(c) = extract_var_from_types(engine.pool(), param_type, actual, var_id) else {
            continue;
        };
        // Resolve through link chains (extracted type may be a
        // fresh var linked to a concrete type via unification).
        return Some(engine.resolve(c));
    }
    None
}

/// Record a deferred monomorphization call when a generic function calls another
/// generic with type variables still unresolved.
///
/// Maps each callee scheme var to either a caller scheme var position (for vars
/// that depend on the caller's type params) or a concrete type (for vars that
/// are already resolved at the call site).
fn record_deferred_mono_call(
    engine: &mut InferEngine<'_>,
    callee: Name,
    callee_scheme_var_ids: &[u32],
    var_subst: &FxHashMap<u32, Idx>,
    callee_param_types: Vec<Idx>,
    callee_return_type: Idx,
) {
    let Some(caller) = engine.current_function() else {
        return;
    };

    // Get caller's signature data (borrow dance: clone to release immutable borrow).
    let caller_sig_data = engine.get_signature(caller).map(|sig| {
        (
            sig.scheme_var_ids.clone(),
            sig.param_types.clone(),
            sig.generic_param_mapping.clone(),
        )
    });
    let Some((caller_svids, caller_ptypes, caller_gpm)) = caller_sig_data else {
        return;
    };

    // Resolve each caller scheme var through the engine to find its root.
    let caller_roots: Vec<Idx> = caller_svids
        .iter()
        .enumerate()
        .map(|(pos, _)| {
            if let Some(Some(param_idx)) = caller_gpm.get(pos) {
                engine.resolve(caller_ptypes[*param_idx])
            } else {
                let sv_id = caller_svids[pos];
                let pool = engine.pool();
                let var_idx = pool.var_idx_for_id(sv_id);
                var_idx.map_or(crate::Idx::ERROR, |idx| engine.resolve(idx))
            }
        })
        .collect();

    // Map each callee var to a caller scheme var position or concrete type.
    let mut semantic_subst: Vec<(u32, crate::DeferredVarBinding)> = Vec::new();
    let mut all_mapped = true;
    for (&callee_var_id, &concrete_idx) in var_subst {
        if engine.pool().tag(concrete_idx) != Tag::Var {
            semantic_subst.push((
                callee_var_id,
                crate::DeferredVarBinding::Concrete(concrete_idx),
            ));
        } else if let Some(pos) = caller_roots.iter().position(|&r| r == concrete_idx) {
            semantic_subst.push((
                callee_var_id,
                crate::DeferredVarBinding::CallerSchemeVar(pos),
            ));
        } else {
            tracing::warn!(
                callee_var_id,
                ?concrete_idx,
                "could not map callee var to caller scheme var"
            );
            all_mapped = false;
        }
    }

    if all_mapped && semantic_subst.len() == callee_scheme_var_ids.len() {
        let deferred = crate::DeferredMonoCall {
            caller,
            callee,
            callee_scheme_var_ids: callee_scheme_var_ids.to_vec(),
            var_subst: semantic_subst,
            callee_param_types,
            callee_return_type,
        };
        tracing::debug!(
            caller = ?caller,
            callee = ?callee,
            subst = ?deferred.var_subst,
            "recorded deferred mono call (generic calling generic)"
        );
        engine.record_deferred_mono_call(deferred);
    }
}

/// Register pool resolutions for concrete Applied types produced by monomorphization.
///
/// When a generic struct like `Pair<A, B>` is instantiated as `Pair<int, int>`,
/// the `body_type_map` contains `Applied(Pair, [Var(A), Var(B)]) -> Applied(Pair, [int, int])`.
/// The LLVM `TypeInfoStore` needs to resolve that concrete Applied to a concrete Struct
/// to compute field layout. This function creates those resolutions.
///
/// Handles nested generics: if `Wrapper<T>` is instantiated with `T = Pair<int, bool>`,
/// the concrete struct field `inner: Applied(Pair, [int, bool])` is also registered.
fn register_concrete_applied_resolutions(
    pool: &mut Pool,
    body_type_map: &[(Idx, Idx)],
    struct_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    for &(_generic_idx, concrete_idx) in body_type_map {
        if pool.tag(concrete_idx) == Tag::Applied {
            resolve_applied_type(pool, concrete_idx, struct_type_params);
        }
    }
}

/// Resolve a single concrete Applied type to a concrete Struct in the pool.
///
/// Recursively resolves any Applied types that appear as field types after
/// substitution (e.g., `Wrapper<Pair<int, bool>>` -> field `inner: Pair<int, bool>`
/// -> also needs `Pair<int, bool>` registered).
fn resolve_applied_type(
    pool: &mut Pool,
    applied_idx: Idx,
    struct_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    use crate::TypeFlags;

    // Skip non-Applied, already-resolved, or types with unresolved vars.
    if pool.tag(applied_idx) != Tag::Applied {
        return;
    }
    if pool.resolve(applied_idx).is_some() {
        return;
    }
    if pool.flags(applied_idx).contains(TypeFlags::HAS_VAR) {
        return;
    }

    // Use resolve_fully's Applied->Named fallback to find the generic struct.
    let resolved = pool.resolve_fully(applied_idx);
    if resolved == applied_idx || pool.tag(resolved) != Tag::Struct {
        return;
    }

    // Build Named->Idx substitution from Applied args and struct type params.
    // The struct fields use Named("A"), Named("B") etc. -- not Var tags --
    // so substitute_named_types is required instead of substitute_in_pool.
    let name = pool.applied_name(applied_idx);
    let args = pool.applied_args(applied_idx);
    let Some(type_params) = struct_type_params.get(&name) else {
        return;
    };
    if type_params.len() != args.len() {
        return;
    }

    let named_subst: FxHashMap<Name, Idx> = type_params
        .iter()
        .zip(args.iter())
        .map(|(&param_name, &arg)| (param_name, arg))
        .collect();

    let fields = pool.struct_fields(resolved);
    let concrete_fields: Vec<(Name, Idx)> = fields
        .iter()
        .map(|&(field_name, field_ty)| {
            let concrete_field = substitute_named_types(pool, field_ty, &named_subst);
            (field_name, concrete_field)
        })
        .collect();

    let concrete_struct = pool.struct_type(name, &concrete_fields);
    pool.set_resolution(applied_idx, concrete_struct);

    tracing::debug!(
        ?name,
        ?applied_idx,
        ?concrete_struct,
        "registered Applied -> Struct resolution for monomorphized type"
    );

    // Recursively resolve Applied types in field types (handles nested generics
    // like Wrapper<Pair<int, bool>> where field inner: Pair<int, bool> also
    // needs registration).
    for &(_, field_ty) in &concrete_fields {
        if pool.tag(field_ty) == Tag::Applied {
            resolve_applied_type(pool, field_ty, struct_type_params);
        }
    }
}

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
    extend_var_subst_with_roots(engine, &mut var_subst);

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

    // Build body_type_map: for every pool entry containing vars, compute the substituted version.
    // Sorted by key for deterministic Eq/Hash (Salsa early cutoff).
    let mut body_type_map = Vec::new();
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    for raw in crate::Idx::FIRST_DYNAMIC..pool_len {
        let idx = crate::Idx::from_raw(raw);
        if pool.flags(idx).contains(crate::TypeFlags::HAS_VAR) {
            let substituted = substitute_in_pool(pool, idx, &var_subst);
            if substituted != idx {
                body_type_map.push((idx, substituted));
            }
        }
    }
    body_type_map.sort_by_key(|(k, _)| k.raw());

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
        let concrete = if let Some(Some(param_idx)) = generic_param_mapping.get(i) {
            // Type param appears directly as a function parameter -- resolve it.
            if let Some(&param_ty) = params.get(*param_idx) {
                engine.resolve(param_ty)
            } else {
                continue;
            }
        } else {
            // Indirect type param (e.g., T in Pair<T, int>) -- extract concrete
            // type by walking generic and concrete param types in parallel.
            let mut found_concrete = None;
            for (j, &param_type) in param_types.iter().enumerate() {
                if let Some(&actual) = params.get(j) {
                    if let Some(c) =
                        extract_var_from_types(engine.pool(), param_type, actual, var_id)
                    {
                        // Resolve through link chains (extracted type may be a
                        // fresh var linked to a concrete type via unification).
                        found_concrete = Some(engine.resolve(c));
                        break;
                    }
                }
            }
            if let Some(c) = found_concrete {
                c
            } else {
                continue;
            }
        };

        if engine.pool().tag(concrete) == Tag::Var {
            has_unresolved_vars = true;
        }

        var_subst.insert(var_id, concrete);
        generic_args.push(GenericArg::Type(concrete));
    }

    (var_subst, generic_args, has_unresolved_vars)
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
                let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
                let var_idx = (crate::Idx::FIRST_DYNAMIC..pool_len)
                    .map(crate::Idx::from_raw)
                    .find(|&idx| pool.tag(idx) == Tag::Var && pool.data(idx) == sv_id);
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

/// Extend `var_subst` with root `var_ids` of each scheme var's equivalence class.
///
/// When a generic function's body calls another generic, unification creates a
/// union-find chain: `scheme_var` -> `fresh_body_var` -> `instantiation_root`.
/// `substitute_in_pool` follows links child->parent but can't substitute root
/// vars whose `var_id` isn't in `var_subst`. Adding root `var_ids` ensures all
/// vars in the equivalence class are substitutable.
fn extend_var_subst_with_roots(engine: &mut InferEngine<'_>, var_subst: &mut FxHashMap<u32, Idx>) {
    let mut root_extensions: Vec<(u32, crate::Idx)> = Vec::new();
    for (&sv_id, &concrete) in var_subst.iter() {
        // Borrow dance: find the scheme var Idx in a scoped borrow,
        // then resolve with &mut engine after the borrow drops.
        let sv_idx = {
            let pool = engine.pool();
            let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
            (crate::Idx::FIRST_DYNAMIC..pool_len)
                .map(crate::Idx::from_raw)
                .find(|&idx| pool.tag(idx) == Tag::Var && pool.data(idx) == sv_id)
        };
        if let Some(sv_idx) = sv_idx {
            let root = engine.resolve(sv_idx);
            let pool = engine.pool();
            if pool.tag(root) == Tag::Var {
                let root_vid = pool.data(root);
                if root_vid != sv_id {
                    root_extensions.push((root_vid, concrete));
                }
            }
        }
    }
    for (vid, concrete) in root_extensions {
        var_subst.insert(vid, concrete);
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

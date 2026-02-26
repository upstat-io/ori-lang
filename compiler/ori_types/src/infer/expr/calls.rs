//! Function call and method call inference.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};
use rustc_hash::FxHashMap;

use super::super::InferEngine;
use super::methods::DEI_ONLY_METHODS;
use super::structs::substitute_named_types;
use super::{infer_expr, resolve_builtin_method};
use crate::pool::substitute::{extract_var_from_types, substitute_in_pool};
use crate::{
    ContextKind, Expected, ExpectedOrigin, GenericArg, Idx, MethodLookupResult, MonoInstance, Pool,
    Tag, TypeCheckError, TypeCheckWarning,
};

/// Infer the type of a function call expression.
pub(crate) fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let arg_ids = arena.get_expr_list(args);

    // Extract function name for signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if arg_ids.len() < required_params || arg_ids.len() > params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each provided argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
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

    // Record monomorphization instance for generic function calls.
    // At this point type variables have been unified with concrete types.
    maybe_record_mono_instance(engine, func_name_id, &params);

    ret
}

/// Infer the type of a named-argument function call.
pub(crate) fn infer_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let call_args = arena.get_call_args(args);

    // Extract function name for error messages and signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if call_args.len() < required_params || call_args.len() > params.len() {
        // Allocate func name string only on the error path
        let func_name = func_name_id.and_then(|n| engine.lookup_name(n).map(String::from));
        if let Some(name) = func_name {
            engine.push_error(TypeCheckError::arity_mismatch_named(
                span,
                name,
                params.len(),
                call_args.len(),
            ));
        } else {
            engine.push_error(TypeCheckError::arity_mismatch(
                span,
                params.len(),
                call_args.len(),
                crate::ArityMismatchKind::Function,
            ));
        }
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each argument type by position
    for (i, (arg, &param_ty)) in call_args.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: func_name_id,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arg.span);
    }

    // Record monomorphization instance for generic function calls.
    maybe_record_mono_instance(engine, func_name_id, &params);

    // Validate where-clause constraints after argument type-checking.
    // At this point, generic type variables have been unified with concrete types.
    if let Some(func_name) = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(n) | ExprKind::Ident(n) => Some(*n),
        _ => None,
    } {
        check_where_clauses(engine, func_name, &params, span);
    }

    ret
}

/// Record a monomorphization instance if the callee is a generic function.
///
/// Called after argument type-checking, when all type variables have been unified
/// with concrete types. Extracts concrete type args via `generic_param_mapping`,
/// builds a substitution map from `scheme_var_ids`, and computes the `body_type_map`
/// for the ARC lowerer.
fn maybe_record_mono_instance(
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

    // Build the var_id → resolved_type substitution map.
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
    // register_concrete_applied_resolutions can build Named→Idx
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

    // Concrete case: all type params resolved — build full MonoInstance.
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

/// Build the `var_id` → `resolved_type` substitution map for monomorphization.
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
            // Type param appears directly as a function parameter — resolve it.
            if let Some(&param_ty) = params.get(*param_idx) {
                engine.resolve(param_ty)
            } else {
                continue;
            }
        } else {
            // Indirect type param (e.g., T in Pair<T, int>) — extract concrete
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

/// Validate that required capabilities are available at a call site.
///
/// Looks up the callee's signature to find its `uses` capabilities,
/// then checks each one against the caller's declared + provided capabilities.
/// Emits `E2014 MissingCapability` for each missing capability.
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
/// union-find chain: `scheme_var` → `fresh_body_var` → `instantiation_root`.
/// `substitute_in_pool` follows links child→parent but can't substitute root
/// vars whose `var_id` isn't in `var_subst`. Adding root `var_ids` ensures all
/// vars in the equivalence class are substitutable.
/// Register pool resolutions for concrete Applied types produced by monomorphization.
///
/// When a generic struct like `Pair<A, B>` is instantiated as `Pair<int, int>`,
/// the `body_type_map` contains `Applied(Pair, [Var(A), Var(B)]) → Applied(Pair, [int, int])`.
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
/// substitution (e.g., `Wrapper<Pair<int, bool>>` → field `inner: Pair<int, bool>`
/// → also needs `Pair<int, bool>` registered).
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

    // Use resolve_fully's Applied→Named fallback to find the generic struct.
    let resolved = pool.resolve_fully(applied_idx);
    if resolved == applied_idx || pool.tag(resolved) != Tag::Struct {
        return;
    }

    // Build Named→Idx substitution from Applied args and struct type params.
    // The struct fields use Named("A"), Named("B") etc. — not Var tags —
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
        "registered Applied → Struct resolution for monomorphized type"
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

pub(crate) fn check_call_capabilities(
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
pub(crate) fn check_where_clauses(
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

/// Check if a type inherently satisfies a trait without needing an explicit impl.
///
/// Mirrors V1's `primitive_implements_trait()` from `bound_checking.rs`.
/// Primitive and built-in types have known trait implementations that don't
/// require explicit `impl` blocks in the trait registry.
#[expect(
    clippy::too_many_lines,
    reason = "per-primitive trait set lookup table"
)]
pub(crate) fn primitive_satisfies_trait(ty: Idx, trait_name: &str) -> bool {
    // Trait sets for each primitive type, matching V1's const arrays.
    const INT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const FLOAT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Neg",
    ];
    const BOOL_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Not",
    ];
    const STR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Len",
        "IsEmpty",
        "Add",
    ];
    const CHAR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
    ];
    const BYTE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const UNIT_TRAITS: &[&str] = &["Eq", "Clone", "Default", "Debug"];
    const DURATION_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "Neg",
    ];
    const SIZE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Debug",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
    ];
    const ORDERING_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Debug",
    ];

    // Check primitive types by Idx constant
    if ty == Idx::INT {
        return INT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::FLOAT {
        return FLOAT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BOOL {
        return BOOL_TRAITS.contains(&trait_name);
    }
    if ty == Idx::STR {
        return STR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::CHAR {
        return CHAR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BYTE {
        return BYTE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::UNIT {
        return UNIT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::DURATION {
        return DURATION_TRAITS.contains(&trait_name);
    }
    if ty == Idx::SIZE {
        return SIZE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::ORDERING {
        return ORDERING_TRAITS.contains(&trait_name);
    }

    false
}

/// Extended trait satisfaction check that also handles compound types via Pool tags.
///
/// This extends `primitive_satisfies_trait` to handle List, Map, Option, Result,
/// Tuple, Set, and Range — types that aren't simple Idx constants but can be
/// identified by their Pool tag.
pub(crate) fn type_satisfies_trait(ty: Idx, trait_name: &str, pool: &Pool) -> bool {
    const COLLECTION_TRAITS: &[&str] = &["Eq", "Clone", "Hashable", "Printable", "Len", "IsEmpty"];
    const WRAPPER_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Default",
    ];
    const RESULT_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Printable"];

    // First check primitives (no pool access needed)
    if primitive_satisfies_trait(ty, trait_name) {
        return true;
    }

    // Then check compound types by tag

    match pool.tag(ty) {
        Tag::List => {
            COLLECTION_TRAITS.contains(&trait_name)
                || trait_name == "Comparable"
                || trait_name == "Iterable"
        }
        Tag::Map | Tag::Set => COLLECTION_TRAITS.contains(&trait_name) || trait_name == "Iterable",
        Tag::Option => WRAPPER_TRAITS.contains(&trait_name),
        Tag::Result => RESULT_TRAITS.contains(&trait_name),
        Tag::Tuple => RESULT_TRAITS.contains(&trait_name) || trait_name == "Len",
        Tag::Range => matches!(trait_name, "Printable" | "Len" | "Iterable"),
        Tag::Str => trait_name == "Iterable",
        Tag::DoubleEndedIterator => trait_name == "Iterator" || trait_name == "DoubleEndedIterator",
        Tag::Iterator => trait_name == "Iterator",
        _ => false,
    }
}

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Trait for Type { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
pub(crate) fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
        ReceiverDispatch::Return {
            ret_ty,
            receiver_ty,
        } => {
            let arg_types: Vec<Idx> = arena
                .get_expr_list(args)
                .iter()
                .map(|&arg_id| infer_expr(engine, arena, arg_id))
                .collect();
            unify_higher_order_constraints(engine, method, ret_ty, receiver_ty, &arg_types);
            return ret_ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let arg_ids = arena.get_expr_list(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, arg_ids.len(), span) {
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
        return sig.ret;
    }

    // Error or not found — infer all args for side effects
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
) -> Idx {
    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
        ReceiverDispatch::Return {
            ret_ty,
            receiver_ty,
        } => {
            let arg_types: Vec<Idx> = arena
                .get_call_args(args)
                .iter()
                .map(|arg| infer_expr(engine, arena, arg.value))
                .collect();
            unify_higher_order_constraints(engine, method, ret_ty, receiver_ty, &arg_types);
            return ret_ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let call_args = arena.get_call_args(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, call_args.len(), span) {
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
        return sig.ret;
    }

    // Error or not found — infer all args for side effects
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);

    Idx::ERROR
}

// ── Shared method dispatch helpers ───────────────────────────────────

/// Result of resolving a method receiver and checking builtin dispatch.
enum ReceiverDispatch {
    /// Return this type. Caller must infer all args first.
    /// `receiver_ty` is the resolved receiver, needed for higher-order constraint propagation.
    Return { ret_ty: Idx, receiver_ty: Idx },
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
}

/// Unify fresh type variables in builtin method return types with constraints
/// from closure arguments.
///
/// Higher-order iterator adapters (`map`, `flat_map`, `fold`) create fresh
/// type variables in their return types. After the closure arguments are
/// inferred, this function unifies those variables with the closure's return
/// type so they resolve to concrete types before codegen.
///
/// Also unifies closure **parameter** types with the source iterator's element
/// type, ensuring unannotated lambda params (e.g., `r` in `.map(r -> r.score)`)
/// resolve to the correct type rather than remaining as unresolved type variables.
fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    method: Name,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
) {
    let Some(method_str) = engine.lookup_name(method) else {
        return;
    };

    match method_str {
        "map" => {
            // ret_ty is Iterator<var> or DEI<var>.
            // arg_types[0] is the closure (T) -> U. Unify var with U.
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
                // Unify closure param with source iterator element
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        "flat_map" => {
            // ret_ty is Iterator<var>.
            // arg_types[0] is closure (T) -> Iterator<U>. Unify var with U.
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
                let resolved_inner = engine.resolve(closure_ret);
                if engine.pool().tag(resolved_inner).is_iterator() {
                    let inner_elem = engine.pool().iterator_elem(resolved_inner);
                    let _ = engine.unify().unify(elem_var, inner_elem);
                }
                // Unify closure param with source iterator element
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        // filter, any, all, find, for_each: closure (T) -> bool/void
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
            // ret_ty is a fresh var. Unify with initial value and closure return.
            if let Some(&init_ty) = arg_types.first() {
                let _ = engine.unify().unify(ret_ty, init_ty);
            }
            if let Some(&closure_ty) = arg_types.get(1) {
                let resolved_closure = engine.resolve(closure_ty);
                if engine.pool().tag(resolved_closure) == Tag::Function {
                    let closure_ret = engine.pool().function_return(resolved_closure);
                    let _ = engine.unify().unify(ret_ty, closure_ret);
                    // fold/rfold closure is (Acc, T) -> Acc: second param is iterator elem
                    let resolved_recv = engine.resolve(receiver_ty);
                    if engine.pool().tag(resolved_recv).is_iterator() {
                        let source_elem = engine.pool().iterator_elem(resolved_recv);
                        let params = engine.pool().function_params(resolved_closure);
                        if let Some(&second_param) = params.get(1) {
                            let _ = engine.unify().unify(second_param, source_elem);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Unify a closure's first parameter with the source iterator's element type.
///
/// For adapters like `.map(r -> r.score)`, ensures that `r` is constrained to
/// the iterator's element type rather than remaining as an unresolved type variable.
fn unify_closure_param_with_iterator_elem(
    engine: &mut InferEngine<'_>,
    resolved_closure: Idx,
    receiver_ty: Idx,
) {
    let resolved_recv = engine.resolve(receiver_ty);
    if !engine.pool().tag(resolved_recv).is_iterator() {
        return;
    }
    let source_elem = engine.pool().iterator_elem(resolved_recv);
    let params = engine.pool().function_params(resolved_closure);
    if let Some(&first_param) = params.first() {
        let _ = engine.unify().unify(first_param, source_elem);
    }
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

    // For unresolved type variables, defer resolution
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        return ReceiverDispatch::Return {
            ret_ty: engine.pool_mut().fresh_var(),
            receiver_ty: resolved,
        };
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
            if DEI_ONLY_METHODS.contains(&name_str) {
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

    // 1c. Reject iteration methods on Range<float>
    if let Some(err) = check_range_float_iteration(engine, resolved, tag, method_str, span) {
        return ReceiverDispatch::Return {
            ret_ty: err,
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
    if !matches!(name_str, "iter" | "collect" | "to_list") {
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

/// Methods that consume an entire iterator and will never terminate on infinite sources.
const INFINITE_CONSUMING_METHODS: &[&str] = &["collect", "count", "fold", "for_each", "to_list"];

/// Methods that are transparent — they wrap the source but don't bound it.
const TRANSPARENT_ADAPTERS: &[&str] = &[
    "map",
    "filter",
    "enumerate",
    "skip",
    "zip",
    "chain",
    "flatten",
    "flat_map",
    "rev",
    "iter",
];

/// Methods that bound an infinite iterator, making consumption safe.
const BOUNDING_METHODS: &[&str] = &["take"];

/// Check if a consuming method is called on an infinite iterator source.
///
/// Walks the receiver's AST chain backward looking for infinite sources
/// (`repeat()`, unbounded ranges `start..`, `.cycle()`) without an
/// intervening `.take()` that would bound the iteration.
///
/// Emits a warning (W2001) if an infinite pattern is detected.
fn check_infinite_iterator_consumed(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: &str,
    span: Span,
) {
    if !INFINITE_CONSUMING_METHODS.contains(&method) {
        return;
    }

    if let Some(source_desc) = find_infinite_source(engine, arena, receiver) {
        engine.push_warning(TypeCheckWarning::infinite_iterator_consumed(
            span,
            method,
            source_desc,
        ));
    }
}

/// Walk the AST chain from a receiver expression looking for an infinite source.
///
/// Returns `Some(description)` if an unbounded infinite source is found,
/// `None` if the chain is bounded or not infinite.
pub(crate) fn find_infinite_source(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
) -> Option<String> {
    let node = arena.get_expr(expr);
    match &node.kind {
        // Method call chain: check the method name, then walk the receiver
        ExprKind::MethodCall {
            receiver, method, ..
        }
        | ExprKind::MethodCallNamed {
            receiver, method, ..
        } => {
            let name = engine.lookup_name(*method).unwrap_or("");
            // .take() bounds the chain — safe
            if BOUNDING_METHODS.contains(&name) {
                return None;
            }
            // .cycle() is an infinite source
            if name == "cycle" {
                return Some("cycle()".into());
            }
            // Transparent adapters — keep walking
            if TRANSPARENT_ADAPTERS.contains(&name) {
                return find_infinite_source(engine, arena, *receiver);
            }
            // Unknown method — stop (conservative: don't warn)
            None
        }

        // Function call: check if it's `repeat(...)`
        ExprKind::Call { func, .. } | ExprKind::CallNamed { func, .. } => {
            let func_node = arena.get_expr(*func);
            if let ExprKind::Ident(name) = &func_node.kind {
                let name_str = engine.lookup_name(*name).unwrap_or("");
                if name_str == "repeat" {
                    return Some("repeat()".into());
                }
            }
            None
        }

        // Range expression: check if end is unbounded
        ExprKind::Range { end, .. } => {
            if !end.is_valid() {
                return Some("unbounded range (start..)".into());
            }
            None
        }

        // Anything else — stop (conservative: don't warn on unknowns)
        _ => None,
    }
}

// ── Impl method resolution (TraitRegistry) ───────────────────────────

/// Result of looking up a method in the `TraitRegistry`.
enum LookupOutcome {
    Found { sig: Idx, has_self: bool },
    Ambiguous(Vec<ori_ir::Name>),
    NotFound,
}

/// Successfully resolved impl method signature.
struct ImplMethodSig {
    /// Method parameters (excluding `self`).
    params: Vec<Idx>,
    /// Return type.
    ret: Idx,
}

/// Perform the borrow-dance lookup for impl methods via `TraitRegistry`.
///
/// Scopes the immutable `trait_registry` borrow to extract data, so the
/// caller can use `engine` mutably afterwards.
fn lookup_impl_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let trait_registry = engine.trait_registry();
    match trait_registry {
        None => LookupOutcome::NotFound,
        Some(reg) => match reg.lookup_method_checked(receiver_ty, method) {
            MethodLookupResult::Found(lookup) => LookupOutcome::Found {
                sig: lookup.method().signature,
                has_self: lookup.method().has_self,
            },
            MethodLookupResult::Ambiguous { candidates } => {
                LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect())
            }
            MethodLookupResult::NotFound => LookupOutcome::NotFound,
        },
    }
}

/// After an impl method lookup, resolve the signature and validate arity.
///
/// Returns `Some(Ok(sig))` on success with params (excluding `self`) and
/// return type. Returns `Some(Err(()))` for errors (ambiguous, bad
/// signature, arity mismatch — diagnostic already pushed). Returns `None`
/// if the method was not found.
fn resolve_impl_signature(
    engine: &mut InferEngine<'_>,
    outcome: LookupOutcome,
    method: Name,
    arg_count: usize,
    span: Span,
) -> Option<Result<ImplMethodSig, ()>> {
    let (sig_ty, has_self) = match outcome {
        LookupOutcome::Found { sig, has_self } => (sig, has_self),
        LookupOutcome::Ambiguous(trait_names) => {
            engine.push_error(TypeCheckError::ambiguous_method(span, method, trait_names));
            return Some(Err(()));
        }
        LookupOutcome::NotFound => return None,
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Err(()));
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // For instance methods (has_self), skip the first `self` param
    let skip = usize::from(has_self);
    let method_params = params[skip..].to_vec();

    if arg_count != method_params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            method_params.len(),
            arg_count,
            crate::ArityMismatchKind::Function,
        ));
        return Some(Err(()));
    }

    Some(Ok(ImplMethodSig {
        params: method_params,
        ret,
    }))
}

/// Emit E2036 when `.into()` is called on a type with no Into implementation.
///
/// Only fires when the method name matches the well-known `into` name.
/// Non-into methods fall through silently (handled by other error paths).
fn emit_into_not_implemented(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
    span: Span,
) {
    let is_into = engine
        .well_known()
        .is_some_and(|wk| method == wk.into_method);
    if is_into {
        engine.push_error(TypeCheckError::into_not_implemented(
            span,
            receiver_ty,
            None,
        ));
    }
}

//! Monomorphization instance recording for generic function calls.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use crate::pool::substitute::{extract_var_from_types, substitute_in_pool};
use crate::{GenericArg, Idx, MonoInstance, Pool};

use super::super::super::InferEngine;

mod applied;
mod burden;
mod deferred;
mod method;

pub(crate) use applied::register_concrete_applied_resolutions;
pub(crate) use burden::{compose_builtin_burdens_for_resolved_types, compose_for_idx};
pub(in crate::infer::expr::calls) use method::maybe_record_method_mono_instance;

use applied::resolve_applied_type;
use deferred::record_deferred_mono_call;

/// Record a monomorphization instance if the callee is a generic function.
///
/// Called after argument type-checking, when all type variables have been unified
/// with concrete types. Extracts concrete type args via `generic_param_mapping`,
/// builds a substitution map from `scheme_var_ids`, and computes the `body_type_map`
/// for the ARC lowerer.
///
/// `call_expr_id` is the AST `ExprId` of the call expression itself. Both the
/// eager (concrete) path and the deferred (still-unresolved-vars) path thread
/// it forward so a dispatch entry lands in `TypedModule.mono_dispatch_map`:
/// the eager path calls `engine.record_mono_with_dispatch` directly; the
/// deferred path stores the id on `DeferredMonoCall.call_expr_id`, and
/// `check::exports::resolve_deferred_mono_calls` publishes the entry once
/// the deferred call resolves to a concrete `MonoInstance`. Both paths therefore land in the same pre-dedup buffer and
/// flow through the same dedup-remap pipeline downstream.
pub(super) fn maybe_record_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    func_name: Option<Name>,
    params: &[Idx],
    inst_return_type: Idx,
) {
    let Some(fn_name) = func_name else {
        return;
    };

    // Extract sig data in an immutable borrow scope.
    let (scheme_var_ids, generic_param_mapping, param_types, sig_return_type) = {
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

    // Hoist the return-projection BEFORE recording: a generic free function
    // returning a bound type-param projection (`-> C.Item`) carries
    // `return_type == Idx::ERROR` (symbolic poison) in its signature. Resolving
    // the projection to the concrete impl binding here — via the same call-site
    // logic `infer_call` uses — keeps the recorded `MonoInstance` return type
    // concrete instead of the poison `Idx::ERROR`.
    let return_type = super::call_inference::resolve_return_projection(
        engine,
        Some(fn_name),
        params,
        sig_return_type,
    );

    {
        let resolved_args: Vec<Idx> = params.iter().map(|&p| engine.resolve(p)).collect();
        let name_str = engine.lookup_name(fn_name).map(str::to_string);
        let pool = engine.pool();
        tracing::debug!(
            target: "ori_types::mono",
            name = ?name_str,
            sig_params = ?param_types.iter().map(|&p| (pool.tag(p), pool.flags(p))).collect::<Vec<_>>(),
            actual_args = ?resolved_args.iter().map(|&p| (pool.tag(p), pool.flags(p))).collect::<Vec<_>>(),
            scheme_vars = ?scheme_var_ids,
            gpm = ?generic_param_mapping,
            "maybe_record_mono_instance: extraction inputs"
        );
    }

    // Build the var_id -> resolved_type substitution map.
    let (mut var_subst, generic_args, has_unresolved_vars) = build_mono_var_subst(
        engine,
        &scheme_var_ids,
        &generic_param_mapping,
        &param_types,
        params,
        return_type,
        inst_return_type,
    );

    tracing::debug!(
        target: "ori_types::mono",
        ?fn_name,
        var_subst_len = var_subst.len(),
        scheme_len = scheme_var_ids.len(),
        has_unresolved_vars,
        subst = ?var_subst.iter().map(|(k, v)| (*k, engine.pool().tag(*v), engine.pool().flags(*v))).collect::<Vec<_>>(),
        "maybe_record_mono_instance: routing decision"
    );

    // All type params must be mapped (even if some are still variables).
    if var_subst.len() != scheme_var_ids.len() {
        return;
    }

    // Deferred case: some type params are still variables (generic calling generic).
    if has_unresolved_vars {
        record_deferred_mono_call(
            engine,
            call_expr_id,
            fn_name,
            &scheme_var_ids,
            &var_subst,
            param_types,
            return_type,
        );
        return;
    }

    // Poison gate: a scheme var resolved to Idx::ERROR (type-error poison) clears
    // the var/infer gate above (has_any_var_or_infer excludes HAS_ERROR) yet must
    // never be monomorphized — minting it produces a phantom instance whose body
    // codegens method invokes on the poison receiver (AOT missing-mono).
    if !var_subst
        .values()
        .all(|&idx| engine.pool().flags(idx).is_recordable())
    {
        return;
    }

    // Extend var_subst with root var_ids of equivalence classes so
    // substitute_in_pool can handle root vars from inner instantiations.
    // Threads the declared scheme_var_ids (the cloned Vec from the
    // sig-lookup block above) explicitly, rather than recovering the
    // list from var_subst's keys. The helper's contract is "extend for
    // THESE declared scheme vars" — canonical scheme-var scoping shared
    // with the deferred-resolve site in
    // check::exports::resolve_deferred_mono_calls and the JIT imported-mono
    // site in oric::test::runner::imported_mono.
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &scheme_var_ids,
        &mut var_subst,
    );

    // Concrete case: all type params resolved -- build + register the instance.
    record_top_level_mono_instance(
        engine,
        call_expr_id,
        fn_name,
        generic_args,
        &var_subst,
        &param_types,
        return_type,
    );

    // Burden composition for generic-builtin instances runs once per body at
    // `InferEngine::take_composed_burdens` (a single full-pool sweep), NOT
    // per-monomorphization here — the per-call sweep was a quadratic full-pool
    // walk on every generic call AND missed collection instances minted by
    // literals that never flow through a generic free-function call. Spec:
    // Annex E §AIMS — composition at type-instantiation time prevents Phase 5
    // from emitting indirect dispatch on each burden walk.
}

/// Build the concrete top-level `MonoInstance` and publish its dispatch entry,
/// once `maybe_record_mono_instance`'s routing gates have proven every scheme
/// var resolved + recordable and `var_subst` extended with equivalence-class
/// roots. `generic_args` is consumed into the minted instance; `var_subst` is
/// read-only here (the root extension already ran in the caller).
fn record_top_level_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    fn_name: Name,
    generic_args: Vec<GenericArg>,
    var_subst: &FxHashMap<u32, Idx>,
    param_types: &[Idx],
    return_type: Idx,
) {
    // Collect generic-composite type params before taking pool_mut(), so
    // register_concrete_applied_resolutions can build Named->Idx substitutions
    // for struct fields and enum payloads (which use Named tags, not Var tags).
    let generic_type_params = collect_generic_type_params(engine);

    let pool = engine.pool_mut();
    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, var_subst);

    // Top-level free functions carry no impl-binder (`Tag::Named`) entries, so
    // `extra_named` is empty here — the shared helper reduces to the canonical
    // build + sort + dedup + Applied-resolution registration.
    let body_type_map =
        build_and_register_body_type_map(pool, var_subst, &[], &generic_type_params);

    // Register the concrete struct resolution for each concrete param / return
    // type (e.g. `Box<[int]>`) — the free-function analogue of the method path's
    // receiver `resolve_applied_type`. This sets the `Applied -> Struct{concrete
    // fields}` resolution so `compose_for_idx`'s user-struct branch composes the
    // per-instantiation burden from the substituted field types instead of the
    // generic `Box<T>` (empty `owned_fields`) fallback.
    for &pt in &concrete_param_types {
        resolve_applied_type(pool, pt, &generic_type_params);
    }
    resolve_applied_type(pool, concrete_return_type, &generic_type_params);

    let instance = MonoInstance::new_top_level(
        fn_name,
        generic_args,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    );

    tracing::debug!(
        fn_name = ?fn_name,
        args = ?instance.generic_args,
        "recorded mono instance"
    );

    // Eager path publishes a dispatch entry keyed on the call expression's
    // ExprId so canon → ARC → ori_llvm/ori_eval can resolve the abstract
    // `MonoInstanceId` without re-derivation. The deferred path carries the
    // same `call_expr_id` on `DeferredMonoCall` and publishes its entry once
    // the deferred call resolves to a concrete `MonoInstance`.
    engine.record_mono_with_dispatch(call_expr_id, instance);
}

/// Collect every user-defined generic type's `name → type_params` so
/// [`register_concrete_applied_resolutions`] can build `Named → Idx`
/// substitutions for struct fields and enum payloads (which use `Tag::Named`,
/// not `Tag::Var`). Read-only on the registry; call BEFORE taking `pool_mut()`.
fn collect_generic_type_params(engine: &InferEngine<'_>) -> FxHashMap<Name, Vec<Name>> {
    engine
        .type_registry()
        .map(|tr| {
            tr.iter()
                .filter(|entry| !entry.type_params.is_empty())
                .map(|entry| (entry.name, entry.type_params.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Build the `body_type_map` from the method/function var substitution plus any
/// impl-level `Tag::Named(binder) → concrete` entries, then register the
/// concrete Applied-type resolutions the LLVM `TypeInfoStore` consumes.
///
/// `extra_named` is empty for top-level free functions (no impl binders) and
/// carries the impl binders for method instances. Sort + dedup by key gives the
/// deterministic `Eq`/`Hash` shape Salsa early cutoff requires.
fn build_and_register_body_type_map(
    pool: &mut Pool,
    var_subst: &FxHashMap<u32, Idx>,
    extra_named: &[(Name, Idx)],
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) -> Vec<(Idx, Idx)> {
    let mut body_type_map: Vec<(Idx, Idx)> = Vec::new();
    // Merge impl-level `Tag::RigidVar(var_id) → concrete` entries into the
    // var-subst so `build_mono_body_type_map` records BOTH leaf rigids
    // (`self.value: T`) AND rigid-containing COMPOSITES (a `Pair<B, A>` ctor
    // inside `swap`). The rigid var_ids are recovered by name-matching the impl
    // binders in `extra_named` via the `build_impl_rigid_var_subst` SSOT scan;
    // the widened `build_mono_body_type_map` mask (HAS_RIGID_VAR) then walks
    // every rigid-containing pool type. Without the composite entries, a
    // generic-struct ctor in the body resolves to the generic layout (`{i64,
    // i64}`) and fails LLVM IR verification (`Invalid InsertValueInst`).
    let combined_subst: FxHashMap<u32, Idx> = if extra_named.is_empty() {
        var_subst.clone()
    } else {
        let name_to_concrete: FxHashMap<Name, Idx> = extra_named.iter().copied().collect();
        let rigid_subst =
            crate::pool::substitute::build_impl_rigid_var_subst(pool, &name_to_concrete);
        let mut combined = var_subst.clone();
        combined.extend(rigid_subst);
        combined
    };
    crate::pool::substitute::build_mono_body_type_map(pool, &combined_subst, &mut body_type_map);
    for &(name, concrete) in extra_named {
        let generic_idx = pool.named(name);
        if generic_idx != concrete {
            body_type_map.push((generic_idx, concrete));
        }
    }
    crate::pool::substitute::finalize_body_type_map(&mut body_type_map);
    register_concrete_applied_resolutions(pool, &body_type_map, generic_type_params);
    body_type_map
}

/// Build the `var_id` -> `resolved_type` substitution map for monomorphization.
///
/// For each scheme variable, resolves it either directly from function params
/// (when type param maps to a parameter position), indirectly by structural
/// extraction from generic param types, or — when neither binds the param (a
/// zero-arg / return-type-determined generic) — by structural extraction from
/// the generic-vs-instantiated return type.
///
/// Returns `(var_subst, generic_args, has_unresolved_vars)`.
fn build_mono_var_subst(
    engine: &mut InferEngine<'_>,
    scheme_var_ids: &[u32],
    generic_param_mapping: &[Option<usize>],
    param_types: &[Idx],
    params: &[Idx],
    sig_return_type: Idx,
    inst_return_type: Idx,
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
            sig_return_type,
            inst_return_type,
        ) else {
            continue;
        };

        // A resolved type that still carries an unbound inference var — a bare
        // `Tag::Var` OR a composite (`[Var]`, `Option<Var>`) with the `HAS_VAR`
        // flag propagated up — is TRANSIENT, not poison (`HAS_ERROR` is the
        // poison signal, gated separately below). It resolves to its concrete
        // root later in the same body pass, so route it to the deferred path
        // rather than dropping it at the poison gate (AOT missing-mono).
        let flags = engine.pool().flags(concrete);
        if flags.has_vars() && !flags.has_errors() {
            has_unresolved_vars = true;
        }

        var_subst.insert(var_id, concrete);
        generic_args.push(GenericArg::Type(concrete));
    }

    (var_subst, generic_args, has_unresolved_vars)
}

/// Resolve a single scheme variable at position `i` to a concrete type, either
/// directly from a function parameter (when `generic_param_mapping[i]` points
/// at a parameter), indirectly by structural extraction from the generic
/// parameter types, or — when no parameter binds it — by structural extraction
/// from the generic-vs-instantiated return type. Returns `None` when no
/// concrete type can be resolved yet — the outer worklist skips the var and
/// revisits on a later iteration.
#[expect(
    clippy::too_many_arguments,
    reason = "threads the full scheme-var resolution context (param mapping + generic/concrete param + return types) — distinct inputs, not a cohesive struct"
)]
fn resolve_scheme_var(
    engine: &mut InferEngine<'_>,
    i: usize,
    var_id: u32,
    generic_param_mapping: &[Option<usize>],
    param_types: &[Idx],
    params: &[Idx],
    sig_return_type: Idx,
    inst_return_type: Idx,
) -> Option<Idx> {
    if let Some(Some(param_idx)) = generic_param_mapping.get(i) {
        // Type param appears directly as a function parameter -- resolve it.
        let &param_ty = params.get(*param_idx)?;
        return Some(engine.resolve(param_ty));
    }

    // Indirect type param (e.g., T in Pair<T, int>) -- extract concrete
    // type by walking generic and concrete param types in parallel.
    if let Some(c) = extract_indirect_scheme_var(engine, var_id, param_types, params) {
        return Some(c);
    }

    // Return-type-determined type param: a zero-arg (or otherwise not-arg-bound)
    // generic whose `T` is fixed only by the call's expected/return binding
    // (`let b: Box<int> = empty_box()` -> `T = int`). At recording time the
    // call's expected-type unification has not yet run, so the extracted type is
    // the fresh instantiation `Tag::Var`; it carries `HAS_VAR` and routes to the
    // deferred path, where the body-final seed pass resolves it once the var
    // links to its concrete root.
    let c = extract_var_from_types(engine.pool(), sig_return_type, inst_return_type, var_id)?;
    Some(engine.resolve(c))
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

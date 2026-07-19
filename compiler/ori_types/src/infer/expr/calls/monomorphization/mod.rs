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
pub(crate) use burden::compose_for_idx;
pub use burden::{compose_burden_for_idx, register_resolved_collection_burdens};
pub(in crate::infer::expr::calls) use method::{
    maybe_record_method_mono_instance, resolve_method_call_generic_args,
};

use applied::resolve_applied_type;
use deferred::{record_deferred_mono_call, DeferredCallSite};

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
/// The callee's generic shape plus its resolved capability-provider
/// substitution, extracted under an immutable signature borrow by
/// [`resolve_sig_and_capabilities`].
struct SigAndCapabilities {
    is_generic: bool,
    scheme_var_ids: Vec<u32>,
    generic_param_mapping: Vec<Option<usize>>,
    param_types: Vec<Idx>,
    sig_return_type: Idx,
    capability_args: Vec<Idx>,
    capability_var_ids: Vec<u32>,
    capability_subst: FxHashMap<u32, Idx>,
    has_unresolved_capability_vars: bool,
}

/// Extract the callee's generic-shape signature data plus its capability
/// providers' concrete substitution, under an immutable signature borrow.
///
/// Returns `None` when there is nothing to record: no registered signature,
/// a non-generic signature with no capabilities, a provider-count mismatch
/// against the declared capability params, or a provider naming the wrong
/// capability at its declared position.
fn resolve_sig_and_capabilities(
    engine: &mut InferEngine<'_>,
    fn_name: Name,
    capability_providers: &[crate::CapabilityProvider],
) -> Option<SigAndCapabilities> {
    // Extract sig data in an immutable borrow scope.
    let (
        is_generic,
        scheme_var_ids,
        generic_param_mapping,
        param_types,
        sig_return_type,
        capability_params,
    ) = {
        let sig = engine.get_signature(fn_name)?;
        let capability_params: Vec<_> = sig
            .capability_params
            .iter()
            .copied()
            .filter_map(|param| match param {
                crate::CapabilityParam::Value {
                    capability,
                    provider_type,
                    provider_var_id,
                } => Some((capability, provider_type, provider_var_id)),
                crate::CapabilityParam::Marker { .. } => None,
            })
            .collect();
        if !sig.is_generic() && capability_params.is_empty() {
            return None;
        }
        (
            sig.is_generic(),
            sig.scheme_var_ids.clone(),
            sig.generic_param_mapping.clone(),
            sig.param_types.clone(),
            sig.return_type,
            capability_params,
        )
    };

    if capability_params.len() != capability_providers.len() {
        return None;
    }
    let mut capability_args = Vec::with_capacity(capability_params.len());
    let mut capability_var_ids = Vec::with_capacity(capability_params.len());
    let mut capability_subst = FxHashMap::default();
    let mut has_unresolved_capability_vars = false;
    for ((capability, _schema, provider_var_id), provider) in
        capability_params.iter().zip(capability_providers)
    {
        if provider.capability != *capability {
            return None;
        }
        let concrete = engine.resolve(provider.provider_type);
        if !engine.pool().flags(concrete).is_recordable() {
            // A provider forwarded through an unspecialized source template is
            // resolved against the concrete caller instance by the deferred
            // path. Capability-only templates have no ordinary generic caller
            // instance and remain discoverable by the fixed-point ARC census.
            has_unresolved_capability_vars = true;
        }
        capability_args.push(concrete);
        capability_var_ids.push(*provider_var_id);
        capability_subst.insert(*provider_var_id, concrete);
    }

    Some(SigAndCapabilities {
        is_generic,
        scheme_var_ids,
        generic_param_mapping,
        param_types,
        sig_return_type,
        capability_args,
        capability_var_ids,
        capability_subst,
        has_unresolved_capability_vars,
    })
}

/// Trace the resolved call-site arg types alongside the callee's declared
/// signature shape, at the point `maybe_record_mono_instance` decides how to
/// route the call (top-level vs. deferred vs. concrete generic).
fn trace_extraction_inputs(
    engine: &mut InferEngine<'_>,
    fn_name: Name,
    params: &[Idx],
    param_types: &[Idx],
    scheme_var_ids: &[u32],
    generic_param_mapping: &[Option<usize>],
) {
    let resolved_args: Vec<Idx> = params.iter().map(|&p| engine.resolve(p)).collect();
    let name_str = engine.lookup_name(fn_name).map(str::to_string);
    let pool = engine.pool();
    tracing::debug!(
        name = ?name_str,
        sig_params = ?param_types.iter().map(|&p| (pool.tag(p), pool.flags(p))).collect::<Vec<_>>(),
        actual_args = ?resolved_args.iter().map(|&p| (pool.tag(p), pool.flags(p))).collect::<Vec<_>>(),
        scheme_vars = ?scheme_var_ids,
        gpm = ?generic_param_mapping,
        "maybe_record_mono_instance: extraction inputs"
    );
}

/// Trace the resolved generic var substitution map at the point
/// `maybe_record_mono_instance` decides between the deferred (still-unresolved)
/// and concrete-generic recording paths.
fn trace_routing_decision(
    engine: &mut InferEngine<'_>,
    fn_name: Name,
    var_subst: &FxHashMap<u32, Idx>,
    scheme_len: usize,
    has_unresolved_vars: bool,
) {
    tracing::debug!(
        ?fn_name,
        var_subst_len = var_subst.len(),
        scheme_len,
        has_unresolved_vars,
        subst = ?var_subst.iter().map(|(k, v)| (*k, engine.pool().tag(*v), engine.pool().flags(*v))).collect::<Vec<_>>(),
        "maybe_record_mono_instance: routing decision"
    );
}

/// The non-generic top-level call site fed to
/// [`record_non_generic_top_level`] — a plain function's (possibly
/// capability-bearing) signature shape plus its resolved capability-provider
/// substitution, bundled to keep the recording call under the workspace
/// argument-count lint.
struct NonGenericTopLevelSite<'a> {
    fn_name: Name,
    capability_args: Vec<Idx>,
    capability_var_ids: &'a [u32],
    capability_subst: FxHashMap<u32, Idx>,
    param_types: &'a [Idx],
    sig_return_type: Idx,
    has_unresolved_capability_vars: bool,
}

/// Record the non-generic top-level path (a plain function whose only
/// recordable shape comes from capability providers, not generic type
/// params). Inert when a capability provider is still unresolved — the
/// deferred path picks it up once the caller's own instance concretizes it.
fn record_non_generic_top_level(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    site: NonGenericTopLevelSite<'_>,
) {
    let NonGenericTopLevelSite {
        fn_name,
        capability_args,
        capability_var_ids,
        mut capability_subst,
        param_types,
        sig_return_type,
        has_unresolved_capability_vars,
    } = site;
    if has_unresolved_capability_vars {
        return;
    }
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        capability_var_ids,
        &mut capability_subst,
    );
    record_top_level_mono_instance(
        engine,
        call_expr_id,
        &capability_subst,
        TopLevelMonoSite {
            fn_name,
            generic_args: Vec::new(),
            capability_args,
            param_types,
            return_type: sig_return_type,
        },
    );
}

/// Poison gate: a scheme var resolved to `Idx::ERROR` (type-error poison)
/// clears the var/infer gate upstream (`has_any_var_or_infer` excludes
/// `HAS_ERROR`) yet must never be monomorphized — minting it produces a
/// phantom instance whose body codegens method invokes on the poison
/// receiver (AOT missing-mono).
fn all_scheme_vars_recordable(
    engine: &mut InferEngine<'_>,
    var_subst: &FxHashMap<u32, Idx>,
) -> bool {
    var_subst
        .values()
        .all(|&idx| engine.pool().flags(idx).is_recordable())
}

pub(super) fn maybe_record_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    func_name: Option<Name>,
    params: &[Idx],
    inst_return_type: Idx,
    capability_providers: &[crate::CapabilityProvider],
) {
    let Some(fn_name) = func_name else {
        return;
    };
    let Some(SigAndCapabilities {
        is_generic,
        scheme_var_ids,
        generic_param_mapping,
        param_types,
        sig_return_type,
        capability_args,
        capability_var_ids,
        capability_subst,
        has_unresolved_capability_vars,
    }) = resolve_sig_and_capabilities(engine, fn_name, capability_providers)
    else {
        return;
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

    trace_extraction_inputs(
        engine,
        fn_name,
        params,
        &param_types,
        &scheme_var_ids,
        &generic_param_mapping,
    );

    if !is_generic {
        record_non_generic_top_level(
            engine,
            call_expr_id,
            NonGenericTopLevelSite {
                fn_name,
                capability_args,
                capability_var_ids: &capability_var_ids,
                capability_subst,
                param_types: &param_types,
                sig_return_type,
                has_unresolved_capability_vars,
            },
        );
        return;
    }

    let Some(ResolvedGenericCall {
        var_subst,
        generic_args,
    }) = resolve_generic_var_subst(
        engine,
        call_expr_id,
        GenericCallSite {
            fn_name,
            scheme_var_ids: &scheme_var_ids,
            generic_param_mapping: &generic_param_mapping,
            param_types: &param_types,
            params,
            return_type,
            inst_return_type,
            capability_var_ids: &capability_var_ids,
            capability_subst,
            has_unresolved_capability_vars,
        },
    )
    else {
        return;
    };

    // Concrete case: all type params resolved -- build + register the instance.
    record_top_level_mono_instance(
        engine,
        call_expr_id,
        &var_subst,
        TopLevelMonoSite {
            fn_name,
            generic_args,
            capability_args,
            param_types: &param_types,
            return_type,
        },
    );

    // Burden composition for generic-builtin instances runs once per body at
    // `InferEngine::take_composed_burdens` (a single full-pool sweep), NOT
    // per-monomorphization here — the per-call sweep was a quadratic full-pool
    // walk on every generic call AND missed collection instances minted by
    // literals that never flow through a generic free-function call. Spec:
    // Annex E §AIMS — composition at type-instantiation time prevents Phase 5
    // from emitting indirect dispatch on each burden walk.
}

/// The generic-calling-generic routing inputs bundled for
/// [`resolve_generic_var_subst`] — everything needed to build the var
/// substitution, decide defer-vs-record, and extend with equivalence-class
/// roots, kept under the workspace argument-count lint.
struct GenericCallSite<'a> {
    fn_name: Name,
    scheme_var_ids: &'a [u32],
    generic_param_mapping: &'a [Option<usize>],
    param_types: &'a [Idx],
    params: &'a [Idx],
    return_type: Idx,
    inst_return_type: Idx,
    capability_var_ids: &'a [u32],
    capability_subst: FxHashMap<u32, Idx>,
    has_unresolved_capability_vars: bool,
}

/// The resolved generic-call substitution and its concrete generic arguments,
/// ready for [`record_top_level_mono_instance`].
struct ResolvedGenericCall {
    var_subst: FxHashMap<u32, Idx>,
    generic_args: Vec<GenericArg>,
}

/// Build the generic `var_id` -> `resolved_type` substitution for a concrete
/// generic call, routing thru the deferred (generic-calling-generic) path
/// and the not-yet-recordable path inline. Returns `None` when the call has
/// already been fully handled by one of those two paths — the caller
/// returns immediately in that case.
fn resolve_generic_var_subst(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    site: GenericCallSite<'_>,
) -> Option<ResolvedGenericCall> {
    let GenericCallSite {
        fn_name,
        scheme_var_ids,
        generic_param_mapping,
        param_types,
        params,
        return_type,
        inst_return_type,
        capability_var_ids,
        capability_subst,
        has_unresolved_capability_vars,
    } = site;

    // Build the ordinary generic var_id -> resolved_type substitution map.
    let (mut var_subst, generic_args, has_unresolved_vars) = build_mono_var_subst(
        engine,
        scheme_var_ids,
        generic_param_mapping,
        param_types,
        params,
        return_type,
        inst_return_type,
    );

    trace_routing_decision(
        engine,
        fn_name,
        &var_subst,
        scheme_var_ids.len(),
        has_unresolved_vars,
    );

    // All type params must be mapped (even if some are still variables).
    if var_subst.len() != scheme_var_ids.len() {
        return None;
    }

    // Deferred case: some type params are still variables (generic calling generic).
    if has_unresolved_vars || has_unresolved_capability_vars {
        var_subst.extend(capability_subst);
        record_deferred_mono_call(
            engine,
            call_expr_id,
            &var_subst,
            DeferredCallSite {
                callee: fn_name,
                callee_scheme_var_ids: scheme_var_ids,
                capability_var_ids,
                callee_param_types: param_types.to_vec(),
                callee_return_type: return_type,
            },
        );
        return None;
    }

    var_subst.extend(capability_subst);

    if !all_scheme_vars_recordable(engine, &var_subst) {
        return None;
    }

    // Extend var_subst with root var_ids of equivalence classes so
    // substitute_in_pool can handle root vars from inner instantiations.
    // Threads the declared scheme_var_ids explicitly, rather than
    // recovering the list from var_subst's keys. The helper's contract is
    // "extend for THESE declared scheme vars" — canonical scheme-var
    // scoping shared with the deferred-resolve site in
    // check::exports::resolve_deferred_mono_calls and the JIT imported-mono
    // site in oric::test::runner::imported_mono.
    let mut retained_var_ids = scheme_var_ids.to_vec();
    retained_var_ids.extend(capability_var_ids);
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &retained_var_ids,
        &mut var_subst,
    );

    Some(ResolvedGenericCall {
        var_subst,
        generic_args,
    })
}

/// The concrete top-level call site fed to [`record_top_level_mono_instance`] —
/// callee identity plus its concrete generic/capability arguments and
/// signature shape, bundled to keep the recording call under the workspace
/// argument-count lint.
struct TopLevelMonoSite<'a> {
    fn_name: Name,
    generic_args: Vec<GenericArg>,
    capability_args: Vec<Idx>,
    param_types: &'a [Idx],
    return_type: Idx,
}

/// Build the concrete top-level `MonoInstance` and publish its dispatch entry,
/// once `maybe_record_mono_instance`'s routing gates have proven every scheme
/// var resolved + recordable and `var_subst` extended with equivalence-class
/// roots. `generic_args` is consumed into the minted instance; `var_subst` is
/// read-only here (the root extension already ran in the caller).
fn record_top_level_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    var_subst: &FxHashMap<u32, Idx>,
    site: TopLevelMonoSite<'_>,
) {
    let TopLevelMonoSite {
        fn_name,
        generic_args,
        capability_args,
        param_types,
        return_type,
    } = site;
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

    let instance = MonoInstance::new_top_level_with_capabilities(
        fn_name,
        generic_args,
        capability_args,
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
    // Pre-resolve the impl-binder `Tag::Named` entries (generic-idx, concrete),
    // identity-skipping generic_idx == concrete, as the bookend's already-resolved
    // `extra_named` slice; the register tail stays SITE-LOCAL.
    let named_entries: Vec<(Idx, Idx)> = extra_named
        .iter()
        .filter_map(|&(name, concrete)| {
            let generic_idx = pool.named(name);
            (generic_idx != concrete).then_some((generic_idx, concrete))
        })
        .collect();
    let body_type_map = crate::pool::substitute::build_finalized_body_type_map(
        pool,
        &combined_subst,
        &named_entries,
    );
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

        // A resolved type that still carries any variable or inference form —
        // including an impl body's rigid variable — is transient, not poison.
        // The producer-qualified caller instance resolves it later, so route
        // it to the deferred path rather than dropping it at this gate.
        let flags = engine.pool().flags(concrete);
        if flags.has_any_var_or_infer() && !flags.has_errors() {
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

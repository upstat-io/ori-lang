//! Monomorphization instance recording for generic function calls.

use ori_ir::{ExprId, Name};
use ori_registry::burden::table::{
    BurdenRegistry, TYPE_ID_LIST, TYPE_ID_MAP, TYPE_ID_OPTION, TYPE_ID_RANGE, TYPE_ID_RESULT,
    TYPE_ID_SET,
};
use rustc_hash::FxHashMap;

use super::super::super::InferEngine;
use crate::pool::substitute::{extract_var_from_types, substitute_in_pool};
use crate::registry::burden_compose::compose_user_burden;
use crate::{GenericArg, Idx, MonoInstance, Pool, Tag, TypeFlags};

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
    // concrete instead of the poison `Idx::ERROR` (BUG-02-067).
    let return_type = super::call_inference::resolve_return_projection(
        engine,
        Some(fn_name),
        params,
        sig_return_type,
    );

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
            call_expr_id,
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

    // Collect generic-composite type params before taking pool_mut(), so
    // register_concrete_applied_resolutions can build Named->Idx substitutions
    // for struct fields and enum payloads (which use Named tags, not Var tags).
    let generic_type_params = collect_generic_type_params(engine);

    // Concrete case: all type params resolved -- build full MonoInstance.
    let pool = engine.pool_mut();
    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, &var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, &var_subst);

    // Top-level free functions carry no impl-binder (`Tag::Named`) entries, so
    // `extra_named` is empty here — the shared helper reduces to the canonical
    // build + sort + dedup + Applied-resolution registration.
    let body_type_map =
        build_and_register_body_type_map(pool, &var_subst, &[], &generic_type_params);

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

    // Burden composition for generic-builtin instances runs once per body at
    // `InferEngine::take_composed_burdens` (a single full-pool sweep), NOT
    // per-monomorphization here — the per-call sweep was a quadratic full-pool
    // walk on every generic call AND missed collection instances minted by
    // literals that never flow through a generic free-function call. Spec:
    // Annex E §AIMS — composition at type-instantiation time prevents Phase 5
    // from emitting indirect dispatch on each burden walk.
}

/// `(method_args, method_named, var_subst)` triple from `resolve_method_binder_args`.
type MethodBinderArgs = (Vec<GenericArg>, Vec<(Name, Idx)>, FxHashMap<u32, Idx>);

/// Resolve the method's own `<T>`-style binders to concrete args.
///
/// In the SIGNATURE the binders are fresh unification vars (in
/// `instantiation_subst`, link-resolved); in the BODY they are
/// `VarState::Rigid { name }` rigid vars whose `var_id` is NOT the scheme `var_id`.
/// The signature substitution keys on `scheme_var_ids` (var-id); the body
/// substitution keys on the binder NAME via `build_impl_rigid_var_subst` — so
/// the binder name (from `generic_param_metadata`, parallel to `scheme_var_ids`
/// in declaration order, non-const entries only) is captured alongside each
/// resolved arg for `extra_named` threading (the impl-binder name-scan path).
///
/// Returns `(method_args, method_named, var_subst)`, or `None` when a binder arg
/// is not fully concrete (caller skips the recording this pass).
fn resolve_method_binder_args(
    engine: &mut InferEngine<'_>,
    sig: &super::impl_lookup::ImplMethodSig,
) -> Option<MethodBinderArgs> {
    let method_binder_names: Vec<Name> = sig
        .generic_param_metadata
        .iter()
        .filter(|m| !m.is_const)
        .map(|m| m.name)
        .collect();
    let mut method_args = Vec::with_capacity(sig.scheme_var_ids.len());
    let mut method_named: Vec<(Name, Idx)> = Vec::with_capacity(sig.scheme_var_ids.len());
    let mut var_subst: FxHashMap<u32, Idx> = FxHashMap::default();
    for (pos, &sv_id) in sig.scheme_var_ids.iter().enumerate() {
        let Some(&fresh) = sig.instantiation_subst.get(&sv_id) else {
            continue;
        };
        let resolved = engine.pool().resolve_fully(fresh);
        if !is_fully_concrete(engine, resolved) {
            return None;
        }
        var_subst.insert(sv_id, resolved);
        method_args.push(GenericArg::Type(resolved));
        if let Some(&name) = method_binder_names.get(pos) {
            method_named.push((name, resolved));
        }
    }
    Some((method_args, method_named, var_subst))
}

/// Record a `MonoInstance` for a generic method call — either an IMPL-level
/// generic (`b.unwrap()` where `b: Box<int>` and the impl is
/// `impl<T> Box<T> { @unwrap (self) -> T }`) OR a METHOD-level generic
/// (`b.pick(item: 5)` where the impl is `impl Boxer { @pick<T> (self, item: T) -> T }`).
///
/// Fires when EITHER binder axis is present: [`ImplMethodSig::method_mono`] is
/// `Some` (impl generic over the receiver's type params) OR
/// [`ImplMethodSig::scheme_var_ids`] is non-empty (the method's own `<U>` binders,
/// present even on a concrete-receiver impl). A non-generic method on a
/// non-generic impl leaves both empty and is conservatively skipped. Emission is
/// additionally gated on the receiver (and every substituted param / return type)
/// being fully concrete; a receiver that still carries type variables — a generic
/// method resolving through another generic — is conservatively skipped this pass.
///
/// MUST be called AFTER argument type-checking has unified the method's
/// instantiation vars, so `engine.resolve` yields concrete types.
pub(super) fn maybe_record_method_mono_instance(
    engine: &mut InferEngine<'_>,
    call_expr_id: ExprId,
    method_name: Name,
    receiver_ty: Idx,
    sig: &super::impl_lookup::ImplMethodSig,
) {
    // Entry gate keys on EITHER binder axis: `method_mono` (impl-level, set when
    // the impl is generic over the receiver — `impl<T> Box<T>`) OR
    // `scheme_var_ids` (method-level, the method's own `<U>` binders, present
    // even on a concrete-receiver impl — `impl Boxer { @pick<T> }`). Keying on
    // `method_mono` alone conflates "impl is generic over the receiver" with
    // "method needs monomorphization"; a method-level-only generic then records
    // no MonoInstance and its `Tag::RigidVar` survives to codegen (PC-2 break).
    let mono = sig.method_mono.as_ref();
    if mono.is_none() && sig.scheme_var_ids.is_empty() {
        return;
    }

    // Receiver carrier: the FULL concrete receiver Idx (e.g. `Box<int>`, NOT
    // the generic `Box<T>` shell). A receiver that still has type vars is not a
    // concrete instantiation — skip per the deferred-receiver carve-out.
    // Deep link-following resolution that PRESERVES the `Applied` shape: the
    // receiver is `Applied(Box, [Var])` whose element Var is linked to the
    // concrete type at the call site, but the cached `HAS_VAR` flag on the
    // Applied survives shallow `engine.resolve`. `substitute_in_pool` with an
    // empty map follows each child Var's `VarState::Link` to `int`, re-interning
    // `Applied(Box, [int])`. `resolve_fully` is WRONG here — its matching-args
    // fallback collapses `Box<int>` to the concrete struct, whose `generic_shell`
    // no longer matches the impl method's `Applied(Box, [RigidVar])` self-param
    // shell at LLVM mono lookup (`collect_mono_functions`).
    let receiver = substitute_in_pool(engine.pool_mut(), receiver_ty, &FxHashMap::default());
    let ret_resolved = engine.resolve(sig.ret);
    tracing::debug!(
        target: "ori_types::mono",
        method = ?method_name,
        receiver_concrete = is_fully_concrete(engine, receiver),
        receiver_tag = ?engine.pool().tag(receiver),
        receiver_flags = ?engine.pool().flags(receiver),
        impl_args = ?mono.map(|m| &m.impl_type_args),
        ret_tag = ?engine.pool().tag(ret_resolved),
        ret_concrete = is_fully_concrete(engine, ret_resolved),
        "maybe_record_method entry gate"
    );
    if !is_fully_concrete(engine, receiver) {
        return;
    }

    // Impl-level + method-level concrete arguments. `impl_type_args` is the
    // receiver-side substitution in declaration order (`[(T, int)]`); method-
    // level args come from the call-site instantiation of `<U>`-style binders.
    // A method-level-only generic on a concrete-receiver impl carries no impl
    // binders (`mono` is `None`) — `impl_args` is empty, and `MonoInstance::
    // new_method` accepts empty `impl_args` with populated `method_args`.
    let mut impl_args = Vec::with_capacity(mono.map_or(0, |m| m.impl_type_args.len()));
    if let Some(mono) = mono {
        for &(_, concrete) in &mono.impl_type_args {
            let resolved = engine.pool().resolve_fully(concrete);
            if !is_fully_concrete(engine, resolved) {
                return;
            }
            impl_args.push(GenericArg::Type(resolved));
        }
    }

    // The method's `<T>`-style binders, resolved to concrete args; `None` when
    // any binder arg still carries type vars (skip this pass).
    let Some((method_args, method_named, mut var_subst)) = resolve_method_binder_args(engine, sig)
    else {
        return;
    };

    // Concrete param / return types via `substitute_in_pool` (empty map), which
    // follows each child Var's `VarState::Link` while PRESERVING the `Applied`
    // shape — `Applied(Pair, [Var->str, Var->int])` deep-resolves to the
    // concrete `Applied(Pair, [str, int])`. `resolve_fully` is WRONG here: its
    // matching-args fallback collapses `Pair<str, int>` to the generic `Pair`
    // struct, so codegen would emit the wrong layout (`{i64, i64}`) and fail IR
    // verification on a permuted-generic return like `swap (self) -> Pair<B, A>`.
    let empty: FxHashMap<u32, Idx> = FxHashMap::default();
    let concrete_param_types: Vec<Idx> = sig
        .params
        .iter()
        .map(|&p| substitute_in_pool(engine.pool_mut(), p, &empty))
        .collect();
    let concrete_return_type = substitute_in_pool(engine.pool_mut(), sig.ret, &empty);
    if concrete_param_types
        .iter()
        .any(|&p| !is_fully_concrete(engine, p))
        || !is_fully_concrete(engine, concrete_return_type)
    {
        return;
    }

    let (body_type_map, extra_named) =
        build_method_body_type_map(engine, sig, mono, receiver, method_named, &mut var_subst);

    let instance = MonoInstance::new_method(
        method_name,
        impl_args,
        method_args,
        receiver,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    );

    tracing::debug!(
        target: "ori_types::mono",
        fn_name = ?method_name,
        receiver = ?receiver,
        impl_args = ?instance.impl_args,
        method_args = ?instance.method_args,
        extra_named = ?extra_named,
        body_type_map = ?instance.body_type_map,
        "recorded mono instance"
    );

    engine.record_mono_with_dispatch(call_expr_id, instance);
}

/// `(body_type_map, extra_named)` returned by [`build_method_body_type_map`]:
/// the generic-body-type → concrete substitutions and the name-keyed binder
/// list threaded into the recorded-instance trace.
type MethodBodyTypeMap = (Vec<(Idx, Idx)>, Vec<(Name, Idx)>);

/// Build the body type map for a method instance and register the receiver's
/// concrete applied resolution. Returns `(body_type_map, extra_named)`; the
/// caller threads `extra_named` into the recorded-instance trace.
fn build_method_body_type_map(
    engine: &mut InferEngine<'_>,
    sig: &super::impl_lookup::ImplMethodSig,
    mono: Option<&super::impl_lookup::MethodMonoData>,
    receiver: Idx,
    method_named: Vec<(Name, Idx)>,
    var_subst: &mut FxHashMap<u32, Idx>,
) -> MethodBodyTypeMap {
    // `body_type_map` maps each generic body type to its concrete form: the
    // method-level scheme vars via the canonical SSOT helper, plus the impl-
    // level `Tag::Named(binder)` entries the var-keyed helper cannot reach.
    crate::pool::substitute::extend_var_subst_with_roots(
        engine.pool(),
        &sig.scheme_var_ids,
        var_subst,
    );
    let generic_type_params = collect_generic_type_params(engine);
    // Name-keyed binder entries the var-keyed helper cannot reach — both the
    // impl-level `Tag::Named(binder)` binders (when `mono` is `Some`) AND the
    // method-level `<T>` binders (the body's `VarState::Rigid { name }` rigid
    // vars). `build_and_register_body_type_map` runs `build_impl_rigid_var_subst`
    // over `extra_named` (a pool scan mapping each binder NAME to its body rigid
    // var_id) so a `[T]`-returning method body re-interns to `[int]`. Without the
    // method binders here the signature monomorphizes but the body's rigid leaf
    // survives to codegen (the `Tag::rigid_var` symptom).
    let mut extra_named: Vec<(Name, Idx)> = mono.map_or_else(Vec::new, |mono| {
        mono.impl_type_args
            .iter()
            .map(|&(name, concrete)| (name, engine.resolve(concrete)))
            .collect()
    });
    extra_named.extend(method_named);
    let pool = engine.pool_mut();
    let body_type_map =
        build_and_register_body_type_map(pool, var_subst, &extra_named, &generic_type_params);

    // The receiver's own concrete Applied type (e.g. `Box<str>`) is the `self`
    // type — never a value in `body_type_map`, which carries only binder
    // substitutions (`Named("T") -> str`). Register its concrete struct
    // resolution directly so the LLVM `TypeInfoStore` resolves the receiver
    // type AND its structurally-interned construction-site `Idx` with the
    // substituted field layout instead of the generic `Box<T>` fallback. The
    // helper recurses into nested generic fields (e.g. `Wrapper<Box<int>>`).
    // The mono-dispatch shell key derives from the receiver's `Tag::Applied`
    // structure directly (`Pool::generic_shell`), so this resolution does not
    // collapse the `(method, Box<_>)` dispatch key.
    resolve_applied_type(pool, receiver, &generic_type_params);

    (body_type_map, extra_named)
}

/// True when `ty` carries no remaining type variables / inference holes —
/// i.e. it is a fully concrete monomorphic type safe to record in a
/// `MonoInstance`.
fn is_fully_concrete(engine: &InferEngine<'_>, ty: Idx) -> bool {
    !engine.pool().flags(ty).has_any_var_or_infer()
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
    body_type_map.sort_by_key(|(k, _)| k.raw());
    body_type_map.dedup_by_key(|(k, _)| k.raw());
    register_concrete_applied_resolutions(pool, &body_type_map, generic_type_params);
    body_type_map
}

/// Walk every fully-resolved generic-builtin `Idx` in the engine's pool and
/// compose its `UserBurdenSpec`, pushing each into the engine's
/// `composed_burdens` accumulator. Runs as a STRUCTURAL pass over the pool;
/// the composition function is pure (no pool/registry mutation).
///
/// Two callers cover the two ways a collection instance enters the pool:
/// (1) after a generic free-function monomorphization (the original
/// `maybe_record_mono_instance` site), and (2) the body-final sweep
/// (`InferEngine::compose_resolved_builtin_burdens`) which catches collection
/// instances minted by literals (`["a", "b"]`, `{k: v}`, `Set` builders) that
/// never flow through a generic call. The `compose_for_idx` accumulator dedups
/// repeat hits, so running both is idempotent in effect.
pub(crate) fn compose_builtin_burdens_for_resolved_types(engine: &mut InferEngine<'_>) {
    // Materialize each concrete generic-composite `Applied` body BEFORE burden
    // composition so the codegen-direct derived-method path and the struct/enum
    // burden arms below read concrete field/payload types, not the generic param.
    materialize_concrete_applied_composites(engine);

    // The concrete types produced by this monomorphization sit in the
    // engine's pool. Walking the entire pool every call is quadratic;
    // `collect_candidate_indices` restricts to generic-builtin tags + the
    // just-materialized concrete user-composite `Applied`s whose flags show no
    // remaining type variables. Composed specs accumulate in the engine's
    // `composed_burdens` Vec, drained at body-pass end via
    // `take_composed_burdens` and flushed into the TypeRegistry burden
    // surface by `ModuleChecker::flush_composed_burdens`.
    let snapshot_indices: Vec<Idx> = {
        let pool = engine.pool();
        collect_candidate_indices(pool)
    };
    for idx in snapshot_indices {
        compose_for_idx(engine, idx);
    }
}

/// Materialize the concrete pool body for every fully-resolved generic-composite
/// `Applied` in the engine's pool that lacks a resolution. Reads the
/// `name → declared param names` map from the registry, then drives the shared
/// `materialize_applied_body` helper (which interns the concrete `Struct`/`Enum`
/// and records `set_resolution(applied -> concrete)`). Read-only registry scan
/// first, then a single mutable pool walk; idempotent (already-resolved
/// `Applied`s short-circuit inside the helper).
fn materialize_concrete_applied_composites(engine: &mut InferEngine<'_>) {
    let generic_type_params = collect_generic_type_params(engine);
    if generic_type_params.is_empty() {
        return;
    }
    let candidates: Vec<Idx> = {
        let pool = engine.pool();
        pool.iter_indices()
            // Offer every unresolved `Applied` to the helper, which
            // resolves each arg through its var-links and skips only genuinely
            // generic instantiations. A raw `HAS_VAR` filter here would drop an
            // inferred construct's type whose arg is a concrete-linked Var.
            .filter(|&idx| pool.tag(idx) == Tag::Applied && pool.resolve(idx).is_none())
            .collect()
    };
    if candidates.is_empty() {
        return;
    }
    let pool = engine.pool_mut();
    let mut in_progress = rustc_hash::FxHashSet::default();
    for applied in candidates {
        crate::pool::substitute::materialize_applied_body(
            pool,
            applied,
            &generic_type_params,
            &mut in_progress,
        );
    }
}

/// Collect every pool Idx whose tag matches a builtin generic template AND
/// whose flags show no remaining type variables (fully-resolved monomorph).
/// Walking the full pool here is a placeholder — once the body-pass
/// integration lands, this becomes a directed enumeration over the
/// `body_type_map` of the just-recorded `MonoInstance`.
fn collect_candidate_indices(pool: &Pool) -> Vec<Idx> {
    let mut out = Vec::new();
    for idx in pool.iter_indices() {
        let tag = pool.tag(idx);
        // Builtin generic templates always candidate; a user-composite
        // `Applied` candidates ONLY once materialized (a concrete resolution is
        // recorded) so `compose_for_idx`'s struct/enum arm reads the concrete
        // body. A generic (unmaterialized) `Applied` is skipped.
        let is_candidate = matches!(
            tag,
            Tag::Option | Tag::Result | Tag::List | Tag::Map | Tag::Set | Tag::Range
        ) || (tag == Tag::Applied && pool.resolve(idx).is_some());
        if !is_candidate {
            continue;
        }
        let flags = pool.flags(idx);
        if flags.contains(TypeFlags::HAS_VAR)
            || flags.contains(TypeFlags::HAS_INFER)
            || flags.contains(TypeFlags::HAS_PROJECTION)
        {
            continue;
        }
        out.push(idx);
    }
    out
}

/// Compose the burden spec for a single fully-resolved generic-builtin Idx,
/// pushing the result into the engine's accumulator. No-op when the Idx's
/// tag does not match a builtin template OR when its args cannot be
/// extracted from the pool.
pub(crate) fn compose_for_idx(engine: &mut InferEngine<'_>, idx: Idx) {
    // Generic-user-struct instantiation (e.g. `Box<[int]>`): the builtin
    // templates below cover Option/Result/[T]/{K:V}/Set/Range only. A user
    // struct's per-instantiation burden is composed from its concrete
    // (substituted) field types — read from the concrete struct resolution set
    // by `resolve_applied_type` at monomorphization. Without this the generic
    // `Box<T>` declaration burden (empty `owned_fields`) is used, so the
    // instantiated aggregate never carries RC and its heap field's drop is
    // mis-attributed to borrowed field projections (`Spec: Annex E §AIMS`).
    {
        let resolved = engine.pool().resolve_fully(idx);
        if engine.pool().tag(resolved) == Tag::Struct {
            let field_types: Vec<Idx> = engine
                .pool()
                .struct_fields(resolved)
                .iter()
                .map(|&(_, ty)| ty)
                .collect();
            if let Some(composed) =
                crate::check::registration::burden_compute::compute_struct_burden_from_field_types(
                    &field_types,
                    engine.pool(),
                )
            {
                engine.record_composed_burden(idx, composed);
            }
            return;
        }
        // Enum twin of the struct arm: a concrete user-defined
        // generic enum (`Either<[int], int>`) composes its per-instantiation
        // burden from the materialized concrete variant payloads, read via
        // `Pool::enum_variants`. Without it the builtin-template match below
        // falls through (`Tag::Enum` is not Option/Result/[T]/{K:V}/Set/Range)
        // and the enum heap payload's drop is mis-attributed.
        if engine.pool().tag(resolved) == Tag::Enum {
            let variants = engine.pool().enum_variants(resolved);
            if let Some(composed) =
                crate::check::registration::burden_compute::compute_enum_burden_from_variant_payloads(
                    &variants,
                    engine.pool(),
                )
            {
                engine.record_composed_burden(idx, composed);
            }
            return;
        }
    }
    let (template_id, type_args) = {
        let pool = engine.pool();
        let template_id = match pool.tag(idx) {
            Tag::Option => TYPE_ID_OPTION,
            Tag::Result => TYPE_ID_RESULT,
            Tag::List => TYPE_ID_LIST,
            Tag::Map => TYPE_ID_MAP,
            Tag::Set => TYPE_ID_SET,
            Tag::Range => TYPE_ID_RANGE,
            _ => return,
        };
        let type_args = extract_type_args(pool, idx);
        (template_id, type_args)
    };

    let Some(template) = BurdenRegistry::lookup_builtin(template_id) else {
        return;
    };

    // Pool borrow released; compose under fresh immutable borrows.
    // The composition function accepts `_pool` and `_registry` for forward-
    // compatible signature but does not consult them today; an absent
    // registry yields the same composed spec as a present one.
    let dummy_registry = crate::TypeRegistry::new();
    let composed = {
        let pool = engine.pool();
        let registry = engine.type_registry().unwrap_or(&dummy_registry);
        compose_user_burden(template, &type_args, pool, registry)
    };

    engine.record_composed_burden(idx, composed);
}

/// Extract the concrete type arguments for a generic-builtin Idx by reading
/// the pool's extra payload via the per-tag accessor surface. Each tag
/// (Option/Result/List/Map/Set/Range) encodes its element types via the
/// `children` accessor on Pool.
fn extract_type_args(pool: &Pool, idx: Idx) -> Vec<Idx> {
    match pool.tag(idx) {
        Tag::Option => vec![pool.option_inner(idx)],
        Tag::Result => vec![pool.result_ok(idx), pool.result_err(idx)],
        Tag::List => vec![pool.list_elem(idx)],
        Tag::Map => vec![pool.map_key(idx), pool.map_value(idx)],
        Tag::Set => vec![pool.set_elem(idx)],
        Tag::Range => vec![pool.range_elem(idx)],
        _ => Vec::new(),
    }
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
    call_expr_id: ExprId,
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
            call_expr_id,
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
pub(crate) fn register_concrete_applied_resolutions(
    pool: &mut Pool,
    body_type_map: &[(Idx, Idx)],
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    for &(_generic_idx, concrete_idx) in body_type_map {
        if pool.tag(concrete_idx) == Tag::Applied {
            resolve_applied_type(pool, concrete_idx, generic_type_params);
        }
    }
}

/// Resolve a single concrete Applied type to its concrete composite body in the
/// pool, covering BOTH `Tag::Struct` and `Tag::Enum`. Delegates to
/// the SSOT `materialize_applied_body` helper in `pool::substitute` (which
/// substitutes the generic field/payload types via the canonical name-keyed
/// walker `substitute_named_in_pool`, interns the concrete `Struct`/`Enum`,
/// records `set_resolution`, and recurses into nested generic fields under an
/// `in_progress` guard). Threads the registry's `name → param names` map and
/// pre-resolves the field/param list at the call site rather than plumbing a
/// `&TypeRegistry` through the generic substitution path.
fn resolve_applied_type(
    pool: &mut Pool,
    applied_idx: Idx,
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    let mut in_progress = rustc_hash::FxHashSet::default();
    crate::pool::substitute::materialize_applied_body(
        pool,
        applied_idx,
        generic_type_params,
        &mut in_progress,
    );
}

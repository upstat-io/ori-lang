//! Transitive deferred-monomorphization resolution helpers.
//!
//! The deferred-call resolution cluster consumed by
//! [`super::resolve_deferred_mono_calls`]: the per-call resolver, its
//! caller-context carrier, the `var_subst` resolution, and the `MonoInstance`
//! builder.

use ori_ir::canon::MonoInstanceId;
use ori_ir::{ExprId, Name};

use crate::Pool;

/// Reconstruct a `var_id -> concrete` substitution from a caller mono
/// instance's `body_type_map`. Mirrors the leaf-extraction in
/// `refresh_method_mono_body_type_maps`: `Pool::data` on a variable item IS
/// its `var_id`, so each `(Var | RigidVar | BoundVar, concrete)` entry yields
/// the per-instantiation binding the ARC lowerer applies to the caller body.
pub(super) fn var_subst_from_body_type_map(
    pool: &Pool,
    body_type_map: &[(crate::Idx, crate::Idx)],
) -> rustc_hash::FxHashMap<u32, crate::Idx> {
    use crate::Tag;
    let mut subst: rustc_hash::FxHashMap<u32, crate::Idx> = rustc_hash::FxHashMap::default();
    for &(key, val) in body_type_map {
        if matches!(pool.tag(key), Tag::Var | Tag::RigidVar | Tag::BoundVar) {
            subst.insert(pool.data(key), val);
        }
    }
    subst
}

/// The realized caller instance's context a deferred call resolves against:
/// its identity (trace), its realized `generic_args` (the `CallerSchemeVar`
/// binding lookup), and its `body_type_map` (the return-type-determined
/// rigid->concrete bindings). Read together from one `MonoInstance` at every
/// call site, so they travel as one domain value rather than three params.
pub(super) struct CallerContext<'a> {
    pub(super) id: crate::DeferredMonoCaller,
    pub(super) generic_args: &'a [crate::GenericArg],
    pub(super) body_type_map: &'a [(crate::Idx, crate::Idx)],
}

/// Attempt to resolve a single deferred call against the caller's
/// already-realized `generic_args`. Pushes a fresh `MonoInstance` onto
/// `mono_instances` iff the call's `var_subst` is fully concrete, the
/// generic-args vector is complete, and the resulting `(callee, args)`
/// key has not been seen before. Silently skips the call on any failure
/// — the outer worklist will retry once a later iteration realizes the
/// missing pieces.
pub(super) fn try_resolve_deferred_call(
    pool: &mut Pool,
    mono_instances: &mut Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: &mut Vec<(ExprId, MonoInstanceId)>,
    seen: &mut rustc_hash::FxHashSet<(Name, Vec<crate::GenericArg>, Vec<crate::Idx>)>,
    caller: &CallerContext<'_>,
    deferred: &crate::DeferredMonoCall,
) {
    tracing::trace!(
        caller = ?caller.id,
        callee = ?deferred.callee,
        caller_generic_args = ?caller.generic_args,
        var_subst = ?deferred.var_subst,
        "processing deferred call"
    );

    let Some(mut resolved_var_subst) = resolve_deferred_var_subst(pool, caller, deferred) else {
        return;
    };

    // Extend resolved_var_subst with union-find root var_ids so
    // build_mono_body_type_map can substitute raw Tag::Var leaves
    // whose var_id is the root rather than the declared callee
    // scheme var. Without this extension, a 3-hop generic chain
    // where the middle layer takes the deferred path leaks
    // Tag::Var leaves into the realized ARC IR and fires the
    // PC-2 seam assertion.
    // SSOT — shared with the
    // eager-path site at infer::expr::calls::monomorphization and
    // the JIT imported-mono site at oric::test::runner::imported_mono.
    let retained_var_ids: Vec<_> = deferred
        .callee_scheme_var_ids
        .iter()
        .copied()
        .chain(deferred.capability_var_ids.iter().copied())
        .collect();
    crate::pool::substitute::extend_var_subst_with_roots(
        pool,
        &retained_var_ids,
        &mut resolved_var_subst,
    );

    let generic_args: Vec<crate::GenericArg> = deferred
        .callee_scheme_var_ids
        .iter()
        .filter_map(|var_id| {
            resolved_var_subst
                .get(var_id)
                .map(|&idx| crate::GenericArg::Type(idx))
        })
        .collect();

    if generic_args.len() != deferred.callee_scheme_var_ids.len() {
        return;
    }
    let capability_args: Vec<_> = deferred
        .capability_var_ids
        .iter()
        .filter_map(|var_id| resolved_var_subst.get(var_id).copied())
        .collect();
    if capability_args.len() != deferred.capability_var_ids.len() {
        return;
    }

    let key = (
        deferred.callee,
        generic_args.clone(),
        capability_args.clone(),
    );
    if !seen.insert(key) {
        // The `(callee, generic_args)` instance already exists — created
        // eagerly (seeded into `seen` from the pre-existing instances) or by an
        // earlier deferred call site sharing this key. The eager path publishes
        // a dispatch entry PER call site and relies on the downstream
        // final dedup-remap to dedup
        // instances. This path mirrors it: publish THIS call site's dispatch
        // entry against the existing instance, else `call_expr_id` receives no
        // `MonoInstanceId` and AOT mono-dispatch drops the call (interp↔LLVM
        // parity break).
        if let Some(existing_idx) = mono_instances.iter().position(|inst| {
            inst.fn_name == deferred.callee
                && inst.generic_args == generic_args
                && inst.capability_args == capability_args
        }) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
            )]
            let existing_id = MonoInstanceId::new(existing_idx as u32);
            mono_dispatch_pre_dedup.push((deferred.call_expr_id, existing_id));
        }
        return;
    }

    let instance = build_mono_instance(
        pool,
        deferred.callee,
        generic_args,
        capability_args,
        &deferred.callee_param_types,
        deferred.callee_return_type,
        &resolved_var_subst,
    );

    tracing::debug!(
        callee = ?deferred.callee,
        args = ?instance.generic_args,
        "resolved transitive mono instance"
    );

    // Publish a dispatch entry for the deferred call so downstream phases
    // (canon → ARC → ori_llvm/ori_eval) see the same `MonoInstanceId` they
    // see for eager-path calls. The new instance lands at the current
    // `mono_instances.len()` slot; this entry flows through the same
    // dedup-remap pipeline as eager entries (see `check/mod.rs` export
    // pipeline).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
    )]
    let new_id = MonoInstanceId::new(mono_instances.len() as u32);
    mono_instances.push(instance);
    mono_dispatch_pre_dedup.push((deferred.call_expr_id, new_id));
}

/// Resolve the callee's `var_subst` into a concrete `(callee_var_id →
/// Idx)` map by looking up each binding against the caller's realized
/// `generic_args`. Returns `None` if any binding refers to a
/// still-unrealized caller generic (missing position) or if the
/// resolved concrete type is itself still a `Tag::Var` — in both cases
/// the caller retries on the next worklist iteration once more
/// instances land.
fn resolve_deferred_var_subst(
    pool: &mut Pool,
    caller: &CallerContext<'_>,
    deferred: &crate::DeferredMonoCall,
) -> Option<rustc_hash::FxHashMap<u32, crate::Idx>> {
    use rustc_hash::FxHashMap;

    let empty: FxHashMap<u32, crate::Idx> = FxHashMap::default();
    let mut resolved: FxHashMap<u32, crate::Idx> =
        FxHashMap::with_capacity_and_hasher(deferred.var_subst.len(), rustc_hash::FxBuildHasher);

    for (callee_var_id, binding) in &deferred.var_subst {
        let concrete = match binding {
            crate::DeferredVarBinding::CallerSchemeVar(pos) => {
                let Some(crate::GenericArg::Type(idx)) = caller.generic_args.get(*pos) else {
                    return None;
                };
                *idx
            }
            crate::DeferredVarBinding::Concrete(idx) => *idx,
            crate::DeferredVarBinding::DeferredType { idx } => {
                // Re-resolve the transient type against the now-fully-linked
                // pool. `substitute_in_pool` with an empty map follows every
                // interior `Tag::Var` link to its concrete leaf and re-interns
                // the composite (`[Var]` → `[str]`). The `is_recordable` gate
                // below rejects the publish (retry) if any interior var is
                // still unbound.
                let direct = crate::pool::substitute::substitute_in_pool(pool, *idx, &empty);
                // A return-type-determined nested generic call (e.g.
                // `make_empty<U>() -> Queue<U> = empty_queue()`) captures the
                // callee scheme var bound to the CALLER's own rigid type param
                // (`empty_queue`'s `T` -> `make_empty`'s `U`), recorded as a
                // `DeferredType` whose `idx` is that rigid var. The empty-map
                // substitute above leaves it rigid (the rigid->concrete binding
                // lives in the caller INSTANCE's `body_type_map`, not in the
                // pool's union-find), so `is_recordable` would reject it and the
                // nested instance is never minted (AOT missing-mono). When the
                // direct result is still non-recordable, re-resolve `idx`
                // through the caller instance's `body_type_map` — the proven
                // per-instantiation substitution (`U -> int`) the ARC lowerer
                // already uses for the caller body — minting the nested instance
                // per concrete instantiation.
                if pool.flags(direct).is_recordable() || caller.body_type_map.is_empty() {
                    direct
                } else {
                    let caller_subst = var_subst_from_body_type_map(pool, caller.body_type_map);
                    crate::pool::substitute::substitute_in_pool(pool, *idx, &caller_subst)
                }
            }
        };

        tracing::trace!(callee_var_id, ?binding, ?concrete, "resolved deferred var");

        // A non-recordable resolution — any unresolved var/infer form (Var, Infer,
        // BoundVar, RigidVar, or a propagated child flag) OR a type-error poison
        // (Idx::ERROR, reached via the record_deferred_mono_call `map_or(Idx::ERROR, ...)`
        // fallback) — must abort the publish: minting it produces a phantom whose body
        // codegens method invokes on a poison/var receiver (AOT missing-mono). Use the
        // canonical TypeFlags::is_recordable predicate, not a hand-rolled narrower gate.
        if !pool.flags(concrete).is_recordable() {
            return None;
        }
        resolved.insert(*callee_var_id, concrete);
    }

    Some(resolved)
}

/// Build a `MonoInstance` by substituting type variables with concrete types.
///
/// Computes concrete param/return types and scans the pool for entries that
/// need substitution (the `body_type_map` for ARC lowering).
fn build_mono_instance(
    pool: &mut Pool,
    fn_name: Name,
    generic_args: Vec<crate::GenericArg>,
    capability_args: Vec<crate::Idx>,
    param_types: &[crate::Idx],
    return_type: crate::Idx,
    var_subst: &rustc_hash::FxHashMap<u32, crate::Idx>,
) -> crate::MonoInstance {
    use crate::pool::substitute::{build_finalized_body_type_map, substitute_in_pool};
    use crate::Idx;

    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, var_subst);

    // Build body_type_map via the canonical build+finalize bookend; this path
    // has no named entries and NO register tail (must NOT register).
    let body_type_map = build_finalized_body_type_map(pool, var_subst, &[]);

    crate::MonoInstance::new_top_level_with_capabilities(
        fn_name,
        generic_args,
        capability_args,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    )
}

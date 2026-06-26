//! Export generation for type checking results.
//!
//! Free functions that produce portable type descriptors, type metadata,
//! collection surface hashes, and resolved monomorphization instances for
//! cross-module transport.

use ori_ir::canon::MonoInstanceId;
use ori_ir::{ExprId, Name};
use rustc_hash::FxHashSet;

use crate::pool::TypeDescriptor;
use crate::FunctionSig;
use crate::Pool;

/// Generate portable type descriptors for all types in public function signatures.
///
/// Iterates public functions, collecting descriptors for every param type and
/// return type. Deduplicates by Merkle hash via `describe_recursive`'s visited set.
/// The result is topologically sorted: leaves (primitives) first.
pub(super) fn generate_export_descriptors(
    pool: &Pool,
    functions: &[FunctionSig],
) -> Vec<(u64, TypeDescriptor)> {
    let mut descriptors = Vec::new();
    let mut visited = FxHashSet::default();

    for sig in functions {
        if !sig.is_public {
            continue;
        }
        for &idx in &sig.param_types {
            pool.describe_recursive(idx, &mut descriptors, &mut visited);
        }
        pool.describe_recursive(sig.return_type, &mut descriptors, &mut visited);
    }

    descriptors
}

/// Generate exported type metadata for cross-module repr plan construction.
///
/// For each user-defined type that has a `#repr` attribute or public visibility,
/// emits an [`ExportedTypeMetadata`] entry carrying the Merkle hash, repr, and
/// visibility. Merges in `imported` metadata from dependency modules so that
/// transitive chains (A→B→C) propagate correctly — B's exports include C's
/// forwarded metadata, so A receives everything transitively.
///
/// Deduplication: local entries take priority (by Merkle hash).
pub(super) fn generate_exported_type_metadata(
    types: &[crate::registry::TypeEntry],
    imported: &[crate::output::ExportedTypeMetadata],
) -> Vec<crate::output::ExportedTypeMetadata> {
    let local: Vec<crate::output::ExportedTypeMetadata> = types
        .iter()
        .filter(|te| te.repr.is_some() || te.visibility == crate::Visibility::Public)
        .map(|te| crate::output::ExportedTypeMetadata {
            merkle_hash: te.merkle_hash,
            repr: te.repr,
            is_public: te.visibility == crate::Visibility::Public,
        })
        .collect();

    if imported.is_empty() {
        return local;
    }

    // Merge: local entries first (take priority), then imported (skip duplicates).
    let mut seen = FxHashSet::default();
    let mut result = Vec::with_capacity(local.len() + imported.len());

    for entry in local {
        if seen.insert(entry.merkle_hash) {
            result.push(entry);
        }
    }
    for entry in imported {
        if seen.insert(entry.merkle_hash) {
            result.push(entry.clone());
        }
    }

    result
}

/// Generate merkle hashes of collection types in public function signatures.
///
/// Walks public function parameter and return types to discover List/Set types
/// (including nested inside Option, Result, Tuple, Map, Struct, Enum). Returns
/// the merkle hashes of discovered collection types, merged with imported
/// collection surfaces for transitive forwarding (A→B→C propagation).
///
/// This parallels `collect_public_collection_types()` in `repr_setup.rs` but
/// outputs merkle hashes (for cross-module transport) instead of Pool Idx values
/// (for same-module use). Both use the shared `walk_collection_types()` walker.
pub(super) fn generate_exported_collection_surfaces(
    pool: &Pool,
    functions: &[crate::output::FunctionSig],
    imported: &[u64],
) -> Vec<u64> {
    let mut hashes = FxHashSet::default();

    for sig in functions {
        if !sig.is_public {
            continue;
        }
        for &param_ty in &sig.param_types {
            crate::pool::walk_collection_types(pool, param_ty, &mut |idx| {
                hashes.insert(pool.hash(idx));
            });
        }
        crate::pool::walk_collection_types(pool, sig.return_type, &mut |idx| {
            hashes.insert(pool.hash(idx));
        });
    }

    // Merge imported collection surfaces for transitive forwarding.
    for &hash in imported {
        hashes.insert(hash);
    }

    hashes.into_iter().collect()
}

/// Resolve deferred mono calls transitively.
///
/// When a generic function calls another generic, the type checker records a
/// [`DeferredMonoCall`] instead of a direct [`MonoInstance`] (because the type
/// arguments are still variables at checking time). This function resolves those
/// deferred calls using the body type maps from concrete `MonoInstance`s.
///
/// Uses a fixed-point worklist: each newly discovered `MonoInstance` may trigger
/// further deferred calls (e.g., `double_wrap<int>` → `wrap<int>` → `id<int>`).
///
/// # Algorithm
///
/// For each `MonoInstance` (e.g., `apply_identity<int>`):
/// 1. Find deferred calls from that function (e.g., `identity` from `apply_identity`)
/// 2. Apply the caller's body type map to resolve deferred variable mappings
///    (e.g., `identity`'s `T` → `apply_identity`'s `T` → `int`)
/// 3. Use `substitute_in_pool` to build concrete types and body type map
/// 4. Create new `MonoInstance` for the callee
/// 5. Repeat until no new instances are discovered
pub(super) fn resolve_deferred_mono_calls(
    pool: &mut Pool,
    mono_instances: &mut Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: &mut Vec<(ExprId, MonoInstanceId)>,
    deferred_calls: &[crate::DeferredMonoCall],
) {
    use rustc_hash::FxHashSet;

    use crate::GenericArg;

    let mut seen: FxHashSet<(Name, Vec<GenericArg>)> = mono_instances
        .iter()
        .map(|m| (m.fn_name, m.generic_args.clone()))
        .collect();

    tracing::debug!(
        instances = mono_instances.len(),
        deferred = deferred_calls.len(),
        "resolve_deferred_mono_calls: starting"
    );

    // Seed pass: a deferred call whose bindings are all caller-independent
    // (`Concrete` / `DeferredType`, no `CallerSchemeVar`) was recorded from a
    // NON-generic caller, which carries no `MonoInstance` for the
    // instance-driven worklist below to key on. Resolve it directly against
    // the fully-linked pool — `DeferredType` follows the transient var's
    // union-find link to its concrete root. The caller's `generic_args` are
    // irrelevant here (no `CallerSchemeVar` binding reads them), so an empty
    // slice is the correct caller context.
    for deferred in deferred_calls.iter().filter(|d| {
        d.var_subst
            .iter()
            .all(|(_, b)| !matches!(b, crate::DeferredVarBinding::CallerSchemeVar(_)))
    }) {
        try_resolve_deferred_call(
            pool,
            mono_instances,
            mono_dispatch_pre_dedup,
            &mut seen,
            deferred.caller,
            &[],
            deferred,
        );
    }

    // Fixed-point worklist: process all instances including newly discovered ones.
    let mut i = 0;
    while i < mono_instances.len() {
        let caller_name = mono_instances[i].fn_name;
        let caller_generic_args = mono_instances[i].generic_args.clone();

        for deferred in deferred_calls.iter().filter(|d| d.caller == caller_name) {
            try_resolve_deferred_call(
                pool,
                mono_instances,
                mono_dispatch_pre_dedup,
                &mut seen,
                caller_name,
                &caller_generic_args,
                deferred,
            );
        }

        i += 1;
    }
}

/// Complete each method instance's `body_type_map` against the now-fully-interned
/// pool, after every body pass has run.
///
/// The eager method-mono path (`maybe_record_method_mono_instance`) builds the
/// `body_type_map` at the CALL site (Pass 3), but a generic-impl method body
/// interns its own composite types during method-body inference (Pass 4) — e.g.
/// a `Pair<B, A>` constructor inside `swap (self) -> Pair<B, A>`. Those body
/// composites cannot exist when the call-site map is built, so the flat map
/// misses them and the composite reaches codegen with its impl `RigidVar`s
/// unsubstituted (LLVM "Invalid `InsertValueInst`" on the un-substituted struct).
///
/// This pass reconstructs each instance's `var_id → concrete` substitution from
/// its eager `body_type_map` leaf entries and re-walks the complete pool via the
/// canonical `build_mono_body_type_map`. `substitute_in_pool` follows the body
/// inference vars' links to the impl `RigidVar`s, which the leaf substitution
/// maps to concrete — so a Pass-4 `Pair<Var, Var>` ctor composite resolves to
/// the concrete `Pair<str, int>` and its `Applied → Struct` resolution registers.
pub(super) fn refresh_method_mono_body_type_maps(
    pool: &mut Pool,
    mono_instances: &mut [crate::MonoInstance],
    generic_type_params: &rustc_hash::FxHashMap<Name, Vec<Name>>,
) {
    use crate::{Idx, Tag};

    for inst in mono_instances.iter_mut() {
        // Method instances can construct generic composites in their body on
        // EITHER binder axis: impl-level (`impl<T> Box<T>` → a `Pair<B, A>` ctor
        // in `swap`) OR method-level (`impl Boxer { @wrap<T> }` → a `[T]` ctor in
        // `wrap`, whose `[Rigid(T)]` is interned at Pass 4, AFTER the Pass-3 call-
        // site recording). Top-level free functions carry no impl/method rigid
        // leaves (a method-level-only generic has empty `impl_args`
        // but populated `method_args` — skipping it left its body composite
        // unrefreshed and the rigid leaf survived to codegen).
        if inst.impl_args.is_empty() && inst.method_args.is_empty() {
            continue;
        }

        // Reconstruct the `var_id → concrete` substitution from the eager map's
        // leaf (Var / RigidVar / BoundVar) entries. `Pool::data` on a variable
        // item IS its `var_id`.
        let mut var_subst: rustc_hash::FxHashMap<u32, Idx> = rustc_hash::FxHashMap::default();
        for &(key, val) in &inst.body_type_map {
            if matches!(pool.tag(key), Tag::Var | Tag::RigidVar | Tag::BoundVar) {
                var_subst.insert(pool.data(key), val);
            }
        }
        if var_subst.is_empty() {
            continue;
        }

        // Re-walk the now-complete pool so body composites interned AFTER eager
        // recording are captured. Preserve the eager `Tag::Named` binder entries
        // (`build_mono_body_type_map` does not produce them).
        let mut refreshed: Vec<(Idx, Idx)> = Vec::new();
        crate::pool::substitute::build_mono_body_type_map(pool, &var_subst, &mut refreshed);
        for &(key, val) in &inst.body_type_map {
            if pool.tag(key) == Tag::Named {
                refreshed.push((key, val));
            }
        }
        crate::pool::substitute::finalize_body_type_map(&mut refreshed);

        crate::infer::register_concrete_applied_resolutions(pool, &refreshed, generic_type_params);
        inst.body_type_map = refreshed;
    }
}

/// Attempt to resolve a single deferred call against the caller's
/// already-realized `generic_args`. Pushes a fresh `MonoInstance` onto
/// `mono_instances` iff the call's `var_subst` is fully concrete, the
/// generic-args vector is complete, and the resulting `(callee, args)`
/// key has not been seen before. Silently skips the call on any failure
/// — the outer worklist will retry once a later iteration realizes the
/// missing pieces.
fn try_resolve_deferred_call(
    pool: &mut Pool,
    mono_instances: &mut Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: &mut Vec<(ExprId, MonoInstanceId)>,
    seen: &mut rustc_hash::FxHashSet<(Name, Vec<crate::GenericArg>)>,
    caller_name: Name,
    caller_generic_args: &[crate::GenericArg],
    deferred: &crate::DeferredMonoCall,
) {
    tracing::trace!(
        caller = ?caller_name,
        callee = ?deferred.callee,
        ?caller_generic_args,
        var_subst = ?deferred.var_subst,
        "processing deferred call"
    );

    let Some(mut resolved_var_subst) =
        resolve_deferred_var_subst(pool, caller_generic_args, deferred)
    else {
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
    crate::pool::substitute::extend_var_subst_with_roots(
        pool,
        &deferred.callee_scheme_var_ids,
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

    let key = (deferred.callee, generic_args.clone());
    if !seen.insert(key) {
        // The `(callee, generic_args)` instance already exists — created
        // eagerly (seeded into `seen` from the pre-existing instances) or by an
        // earlier deferred call site sharing this key. The eager path publishes
        // a dispatch entry PER call site and relies on the downstream
        // dedup-remap (`check/mod.rs::dedup_and_remap_mono_instances`) to dedup
        // instances. This path mirrors it: publish THIS call site's dispatch
        // entry against the existing instance, else `call_expr_id` receives no
        // `MonoInstanceId` and AOT mono-dispatch drops the call (interp↔LLVM
        // parity break).
        if let Some(existing_idx) = mono_instances
            .iter()
            .position(|inst| inst.fn_name == deferred.callee && inst.generic_args == generic_args)
        {
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
    caller_generic_args: &[crate::GenericArg],
    deferred: &crate::DeferredMonoCall,
) -> Option<rustc_hash::FxHashMap<u32, crate::Idx>> {
    use rustc_hash::FxHashMap;

    let empty: FxHashMap<u32, crate::Idx> = FxHashMap::default();
    let mut resolved: FxHashMap<u32, crate::Idx> =
        FxHashMap::with_capacity_and_hasher(deferred.var_subst.len(), rustc_hash::FxBuildHasher);

    for (callee_var_id, binding) in &deferred.var_subst {
        let concrete = match binding {
            crate::DeferredVarBinding::CallerSchemeVar(pos) => {
                let Some(crate::GenericArg::Type(idx)) = caller_generic_args.get(*pos) else {
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
                crate::pool::substitute::substitute_in_pool(pool, *idx, &empty)
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
    param_types: &[crate::Idx],
    return_type: crate::Idx,
    var_subst: &rustc_hash::FxHashMap<u32, crate::Idx>,
) -> crate::MonoInstance {
    use crate::pool::substitute::{
        build_mono_body_type_map, finalize_body_type_map, substitute_in_pool,
    };
    use crate::Idx;

    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, var_subst);

    // Build body_type_map via the canonical SSOT helper; sort+dedup for
    // Salsa-deterministic shape is call-site-local post-processing.
    let mut body_type_map: Vec<(Idx, Idx)> = Vec::new();
    build_mono_body_type_map(pool, var_subst, &mut body_type_map);
    finalize_body_type_map(&mut body_type_map);

    crate::MonoInstance::new_top_level(
        fn_name,
        generic_args,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    )
}

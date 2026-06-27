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

mod deferred_mono;

use deferred_mono::{try_resolve_deferred_call, var_subst_from_body_type_map, CallerContext};

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
            &CallerContext {
                name: deferred.caller,
                generic_args: &[],
                body_type_map: &[],
            },
            deferred,
        );
    }

    // Fixed-point worklist: process all instances including newly discovered ones.
    let mut i = 0;
    while i < mono_instances.len() {
        let caller_name = mono_instances[i].fn_name;
        let caller_generic_args = mono_instances[i].generic_args.clone();
        // The caller instance's `body_type_map` carries the per-instantiation
        // rigid->concrete bindings (`U -> int`) the deferred resolver needs to
        // mint a return-type-determined nested generic call whose `DeferredType`
        // binding holds the caller's own rigid type param.
        let caller_body_type_map = mono_instances[i].body_type_map.clone();

        for deferred in deferred_calls.iter().filter(|d| d.caller == caller_name) {
            try_resolve_deferred_call(
                pool,
                mono_instances,
                mono_dispatch_pre_dedup,
                &mut seen,
                &CallerContext {
                    name: caller_name,
                    generic_args: &caller_generic_args,
                    body_type_map: &caller_body_type_map,
                },
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
        // leaf entries via the canonical extractor.
        let var_subst = var_subst_from_body_type_map(pool, &inst.body_type_map);
        if var_subst.is_empty() {
            continue;
        }

        // Re-walk the now-complete pool so body composites interned AFTER eager
        // recording are captured. Preserve the eager `Tag::Named` binder entries
        // (`build_mono_body_type_map` does not produce them) by pushing them as
        // the bookend's already-resolved `extra_named` slice; the register tail
        // stays SITE-LOCAL.
        let named_entries: Vec<(Idx, Idx)> = inst
            .body_type_map
            .iter()
            .copied()
            .filter(|&(key, _)| pool.tag(key) == Tag::Named)
            .collect();
        let refreshed = crate::pool::substitute::build_finalized_body_type_map(
            pool,
            &var_subst,
            &named_entries,
        );

        crate::infer::register_concrete_applied_resolutions(pool, &refreshed, generic_type_params);
        inst.body_type_map = refreshed;
    }
}

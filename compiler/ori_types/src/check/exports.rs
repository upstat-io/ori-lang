//! Export generation for type checking results.
//!
//! Free functions that produce portable type descriptors, type metadata,
//! collection surface hashes, and resolved monomorphization instances for
//! cross-module transport.

use ori_ir::Name;
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
    deferred_calls: &[crate::DeferredMonoCall],
) {
    use rustc_hash::{FxHashMap, FxHashSet};

    use crate::{GenericArg, Idx};

    // Track already-seen (fn_name, generic_args) to avoid duplicates.
    let mut seen: FxHashSet<(Name, Vec<GenericArg>)> = mono_instances
        .iter()
        .map(|m| (m.fn_name, m.generic_args.clone()))
        .collect();

    tracing::debug!(
        instances = mono_instances.len(),
        deferred = deferred_calls.len(),
        "resolve_deferred_mono_calls: starting"
    );

    // Fixed-point worklist: process all instances including newly discovered ones.
    let mut i = 0;
    while i < mono_instances.len() {
        let caller_name = mono_instances[i].fn_name;
        let caller_generic_args = mono_instances[i].generic_args.clone();

        for deferred in deferred_calls.iter().filter(|d| d.caller == caller_name) {
            tracing::trace!(
                caller = ?caller_name,
                callee = ?deferred.callee,
                caller_generic_args = ?caller_generic_args,
                var_subst = ?deferred.var_subst,
                "processing deferred call"
            );

            // Resolve the callee's var_subst: map each callee var_id to a concrete
            // type by looking up the caller's generic_args at the stored position.
            let mut resolved_var_subst: FxHashMap<u32, Idx> = FxHashMap::with_capacity_and_hasher(
                deferred.var_subst.len(),
                rustc_hash::FxBuildHasher,
            );
            let mut all_concrete = true;

            for (callee_var_id, binding) in &deferred.var_subst {
                let concrete = match binding {
                    crate::DeferredVarBinding::CallerSchemeVar(pos) => {
                        let Some(GenericArg::Type(idx)) = caller_generic_args.get(*pos) else {
                            all_concrete = false;
                            break;
                        };
                        *idx
                    }
                    crate::DeferredVarBinding::Concrete(idx) => *idx,
                };

                tracing::trace!(callee_var_id, ?binding, ?concrete, "resolved deferred var");

                if pool.tag(concrete) == crate::Tag::Var {
                    all_concrete = false;
                    break;
                }
                resolved_var_subst.insert(*callee_var_id, concrete);
            }

            if !all_concrete {
                continue;
            }

            // Build generic_args in scheme_var_ids order.
            let generic_args: Vec<GenericArg> = deferred
                .callee_scheme_var_ids
                .iter()
                .filter_map(|var_id| {
                    resolved_var_subst
                        .get(var_id)
                        .map(|&idx| GenericArg::Type(idx))
                })
                .collect();

            if generic_args.len() != deferred.callee_scheme_var_ids.len() {
                continue;
            }

            let key = (deferred.callee, generic_args.clone());
            if !seen.insert(key) {
                continue;
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

            mono_instances.push(instance);
        }

        i += 1;
    }
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
    use crate::pool::substitute::substitute_in_pool;
    use crate::{Idx, TypeFlags};

    let concrete_param_types: Vec<Idx> = param_types
        .iter()
        .map(|&pt| substitute_in_pool(pool, pt, var_subst))
        .collect();
    let concrete_return_type = substitute_in_pool(pool, return_type, var_subst);

    let mut body_type_map = Vec::new();
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        if pool.flags(idx).contains(TypeFlags::HAS_VAR) {
            let substituted = substitute_in_pool(pool, idx, var_subst);
            if substituted != idx {
                body_type_map.push((idx, substituted));
            }
        }
    }
    body_type_map.sort_by_key(|(k, _)| k.raw());

    crate::MonoInstance {
        fn_name,
        generic_args,
        concrete_param_types,
        concrete_return_type,
        body_type_map,
    }
}

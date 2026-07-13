//! Builtin-burden composition for fully-resolved generic-builtin instances.

use ori_registry::burden::table::{
    BurdenRegistry, TYPE_ID_LIST, TYPE_ID_MAP, TYPE_ID_OPTION, TYPE_ID_RANGE, TYPE_ID_RESULT,
    TYPE_ID_SET,
};

use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_compose::compose_user_burden;
use crate::{Idx, Pool, Tag, TypeFlags};

use crate::infer::InferEngine;

use super::collect_generic_type_params;

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
    let dummy_registry = crate::TypeRegistry::new();
    let composed = {
        let pool = engine.pool();
        let registry = engine.type_registry().unwrap_or(&dummy_registry);
        compose_burden_for_idx(pool, registry, idx)
    };
    if let Some(spec) = composed {
        engine.record_composed_burden(idx, spec);
    }
}

/// Compose the `UserBurdenSpec` for a fully-resolved generic-builtin or
/// concrete-user-composite `Idx` from `pool` + `registry` alone (no
/// `InferEngine`). Returns `None` for a non-composable tag (scalar, function,
/// unresolved). SSOT for per-Idx burden composition consumed by `compose_for_idx`
/// (the engine-bound monomorphization sweep) AND by the codegen pipeline's
/// imported-body collection-burden pass (an imported function's internal
/// collection types resolve into the importer's pool but never flow through the
/// engine sweep). Spec: Annex E §AIMS.
pub fn compose_burden_for_idx(
    pool: &Pool,
    registry: &crate::TypeRegistry,
    idx: Idx,
) -> Option<UserBurdenSpec> {
    // Generic-user-struct / -enum instantiation: compose from the concrete
    // (substituted) field / variant-payload types (the builtin templates below
    // cover Option/Result/[T]/{K:V}/Set/Range only).
    let resolved = pool.resolve_fully(idx);
    match pool.tag(resolved) {
        Tag::Struct => {
            let field_types: Vec<Idx> = pool
                .struct_fields(resolved)
                .iter()
                .map(|&(_, ty)| ty)
                .collect();
            return crate::check::registration::burden_compute::compute_struct_burden_from_field_types(
                &field_types,
                pool,
            );
        }
        Tag::Enum => {
            let variants = pool.enum_variants(resolved);
            return crate::check::registration::burden_compute::compute_enum_burden_from_variant_payloads(
                &variants,
                pool,
            );
        }
        // A tuple is the anonymous struct: compose from its element types
        // (positional fields) so a standalone tuple Idx resolves the same
        // owned-field burden a named struct with those fields would.
        Tag::Tuple => {
            let elem_types = pool.tuple_elems(resolved);
            return crate::check::registration::burden_compute::compute_struct_burden_from_field_types(
                &elem_types,
                pool,
            );
        }
        _ => {}
    }
    let template_id = match pool.tag(idx) {
        Tag::Option => TYPE_ID_OPTION,
        Tag::Result => TYPE_ID_RESULT,
        Tag::List => TYPE_ID_LIST,
        Tag::Map => TYPE_ID_MAP,
        Tag::Set => TYPE_ID_SET,
        Tag::Range => TYPE_ID_RANGE,
        _ => return None,
    };
    let template = BurdenRegistry::lookup_builtin(template_id)?;
    let type_args = extract_type_args(pool, idx);
    Some(compose_user_burden(template, &type_args, pool, registry))
}

/// Compose + register the burden for every fully-resolved collection / composite
/// `Idx` in `pool` whose burden `registry` does not already carry.
///
/// An imported function's body resolves its internal collection types into the
/// importer's merged pool, but `register_imported_function` composes burdens for
/// signature types only — so an imported body's internal collection (e.g. a dead
/// `for…yield` `[bool]` local) can reach class-ledger Step-4b emission without the
/// burden metadata needed to resolve its RC-bearing shape. Both codegen registries
/// (AOT + JIT) rebuild from typed-module exports via
/// `TypeRegistry::from_typed_exports`, which carry signature-reachable burdens
/// only; this pass walks the merged pool and fills the gaps using the SSOT
/// `compose_burden_for_idx`. Pool-walking (not arc-IR-walking) keeps registration
/// independent of when each function's `ArcFunction` is lowered. Spec: Annex E
/// §AIMS.
pub fn register_resolved_collection_burdens(pool: &Pool, registry: &mut crate::TypeRegistry) {
    let mut composed: Vec<(Idx, UserBurdenSpec)> = Vec::new();
    for idx in collect_candidate_indices(pool) {
        if registry.burden(idx).is_some() {
            continue;
        }
        if let Some(spec) = compose_burden_for_idx(pool, registry, idx) {
            composed.push((idx, spec));
        }
    }
    for (idx, spec) in composed {
        let _ = registry.register_user_burden(idx, spec);
    }
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

#[cfg(test)]
mod tests;

//! Phase 0 metadata propagation: imported types and monomorphized generics.
//!
//! These functions run early in the pipeline to seed the `ReprPlan` with
//! `#repr` attributes and public-type markers from imported modules, then
//! propagate that metadata through generic type resolutions.

use ori_ir::ReprAttrKind;
use ori_types::{Idx, Pool, Tag};

use crate::plan::{ReprAttribute, ReprPlan};

use super::convert_repr_attr_kind;

/// Phase 0a-import: Seed imported metadata from cross-module transport channels.
///
/// Resolves imported `ExportedTypeMetadata` (repr/pub) and collection surface
/// hashes to local pool indices, storing repr attributes in the plan and
/// collecting public type + collection indices for Phase 0b.
///
/// Returns `(imported_repr_attrs, imported_pub_indices, imported_collection_indices)`.
pub(crate) fn seed_imported_metadata(
    plan: &mut ReprPlan,
    pool: &Pool,
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
) -> (Vec<(Idx, ReprAttrKind)>, Vec<Idx>, Vec<Idx>) {
    let mut imported_repr_attrs: Vec<(Idx, ReprAttrKind)> = Vec::new();
    let mut imported_pub_indices: Vec<Idx> = Vec::new();

    for meta in imported_type_metadata {
        if let Some(idx) = pool.lookup_by_hash(meta.merkle_hash) {
            if let Some(repr) = meta.repr {
                let converted = convert_repr_attr_kind(&repr);
                plan.set_repr_attr(idx, converted);
                let resolved = pool.resolve_fully(idx);
                if resolved != idx {
                    plan.set_repr_attr(resolved, converted);
                }
                imported_repr_attrs.push((idx, repr));
                tracing::trace!(
                    ?idx,
                    hash = meta.merkle_hash,
                    ?repr,
                    "imported repr attr seeded"
                );
            }
            if meta.is_public {
                let resolved = pool.resolve_fully(idx);
                if resolved == idx {
                    imported_pub_indices.push(idx);
                } else {
                    imported_pub_indices.push(idx);
                    imported_pub_indices.push(resolved);
                }
                tracing::trace!(?idx, hash = meta.merkle_hash, "imported pub type seeded");
            }
        }
    }

    // Resolve imported collection surface hashes to local Idx.
    // These collection types (List, Set) appeared in imported public function
    // signatures. Resolved for diagnostics and future cross-module ABI use.
    // NOT added to pub_type_indices — see note in Phase 0b.
    let mut imported_collection_indices: Vec<Idx> = Vec::new();
    for &hash in imported_collection_surfaces {
        if let Some(idx) = pool.lookup_by_hash(hash) {
            let resolved = pool.resolve_fully(idx);
            if resolved == idx {
                imported_collection_indices.push(idx);
            } else {
                imported_collection_indices.push(idx);
                imported_collection_indices.push(resolved);
            }
            tracing::trace!(?idx, hash, "imported collection surface seeded");
        }
    }

    (
        imported_repr_attrs,
        imported_pub_indices,
        imported_collection_indices,
    )
}

/// Phase 0c: Propagate repr/pub metadata to monomorphized generic type resolutions.
///
/// Monomorphization creates `Applied(Name, [ConcreteArgs]) → Struct` resolutions
/// that are distinct from the `Named → Struct` chain handled by Phases 0/0b.
/// This function collects the set of protected type `Name`s, scans the pool for
/// `Applied` entries whose name matches, resolves each through the pool, and
/// propagates `repr_attr` / `pub_type` metadata to the resolved concrete `Struct` idx.
pub(crate) fn propagate_metadata_to_applied_resolutions(
    plan: &mut ReprPlan,
    pool: &Pool,
    repr_attrs: &[(Idx, ReprAttrKind)],
    pub_type_indices: &[Idx],
) {
    use rustc_hash::FxHashMap;

    // Collect protected type names with their metadata.
    let mut protected: FxHashMap<ori_ir::Name, (Option<ReprAttribute>, bool)> =
        FxHashMap::default();

    for &(idx, ref attr) in repr_attrs {
        let tag = pool.tag(idx);
        let name = match tag {
            Tag::Named => pool.named_name(idx),
            Tag::Applied => pool.applied_name(idx),
            _ => continue,
        };
        let entry = protected.entry(name).or_insert((None, false));
        entry.0 = Some(convert_repr_attr_kind(attr));
    }

    for &idx in pub_type_indices {
        let tag = pool.tag(idx);
        let name = match tag {
            Tag::Named => pool.named_name(idx),
            Tag::Applied => pool.applied_name(idx),
            _ => continue,
        };
        let entry = protected.entry(name).or_insert((None, false));
        entry.1 = true;
    }

    if protected.is_empty() {
        return;
    }

    // Scan all dynamic pool entries for Applied types matching protected names.
    let pool_len = u32::try_from(pool.len()).unwrap_or(u32::MAX);
    for raw in Idx::FIRST_DYNAMIC..pool_len {
        let idx = Idx::from_raw(raw);
        if pool.tag(idx) != Tag::Applied {
            continue;
        }

        let name = pool.applied_name(idx);
        let Some(&(ref repr_attr, is_pub)) = protected.get(&name) else {
            continue;
        };

        // Resolve through the pool to find the concrete Struct/Tuple.
        let resolved = pool.resolve_fully(idx);
        if resolved == idx {
            // No resolution registered for this Applied — skip.
            continue;
        }

        if let Some(attr) = repr_attr {
            if plan.repr_attr(resolved).is_none() {
                plan.set_repr_attr(resolved, *attr);
                tracing::trace!(
                    ?idx,
                    ?resolved,
                    ?attr,
                    "propagated repr attr to monomorphized concrete type"
                );
            }
        }

        if is_pub && !plan.is_public_type(resolved) {
            plan.set_pub_type_indices(std::iter::once(resolved));
            tracing::trace!(
                ?idx,
                ?resolved,
                "propagated pub status to monomorphized concrete type"
            );
        }
    }
}

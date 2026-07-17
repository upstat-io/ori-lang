//! Skip-authority derivation for the field-decomposition cure: HOW a
//! container's `DecPartial` skip set is keyed (variant ordinal vs positional
//! field indices), from construct uniformity or the type's burden table.

use crate::aims::intraprocedural::birth_site_partition::{BirthSitePartition, NodeIdx};
use crate::ir::ArcFunction;

use super::FieldViewHazard;

/// Whether the container's type carries a USER enum burden with variant
/// entries — a logical sum whose cleanup operation is variant-sensitive.
/// Builtin `Option`/`Result` share the payload's ownership identity, so
/// skipping their payload drops the class's only logical release.
fn container_is_user_variant_enum(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> bool {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    let Some((var, _)) = partition.node_key(hazard.container) else {
        return false;
    };
    let Some(&ty) = func.var_types.get(var.index()) else {
        return false;
    };
    match lookup_burden(idx_to_type_ref(ty, type_registry), type_registry) {
        Some(BurdenRef::User(user)) => !user.variant_burdens.is_empty(),
        _ => false,
    }
}

/// Type-derived variant skip for a CONSTRUCTLESS sum container (call- or
/// merge-produced — no `Construct` in the container class to inspect):
/// the container type's burden table names EXACTLY ONE payload-bearing
/// variant carrying ONE payload slot, so any extracted payload belongs to
/// that variant — the moved mark is variant-unique by type structure
/// (`FD_skipset_sound`; the construct-uniform premise met by the type
/// instead of the construct sites). `self_owned_identity` required: a
/// wrapper with no distinct ownership identity is never decomposable — its
/// whole-var decrement IS the payload's release.
fn derive_constructless_enum_variant(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> Option<Vec<u32>> {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    if !hazard.construct_sites.is_empty() || hazard.skip_fields != [1] {
        return None;
    }
    let (var, _) = partition.node_key(hazard.container)?;
    let &ty = func.var_types.get(var.index())?;
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(ty, type_registry), type_registry)
    else {
        return None;
    };
    if !user.self_owned_identity {
        return None;
    }
    let mut payload_bearing = user
        .variant_burdens
        .iter()
        .filter(|v| !v.retained_owned.is_empty() || !v.transfers_on_match.is_empty());
    let unique = payload_bearing.next()?;
    if payload_bearing.next().is_some() || unique.retained_owned.len() != 1 {
        return None;
    }
    // `VariantId` is 1-indexed; the skip names the 0-based ordinal selected
    // by the variant-sensitive cleanup operation.
    Some(vec![unique.variant_id.get().get() - 1])
}

/// The container's skip authority: HOW its `DecPartial` skip set is keyed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SkipAuthority {
    /// A SUM container: the skip names the moved-out VARIANT ordinal
    /// (variant-sensitive logical cleanup).
    Variant(Vec<u32>),
    /// A STRUCT/TUPLE container: the skip names top-level FIELD ordinals
    /// (positional logical cleanup).
    Positional(Vec<u32>),
}

impl SkipAuthority {
    pub(super) fn skip_fields(&self) -> &[u32] {
        match self {
            Self::Variant(skip) | Self::Positional(skip) => skip,
        }
    }

    /// The variant ordinal for tag-exclusion; `None` for positional skips.
    pub(super) fn variant_ordinal(&self) -> Option<u32> {
        match self {
            Self::Variant(skip) => skip.first().copied(),
            Self::Positional(_) => None,
        }
    }
}

/// Type-derived POSITIONAL skip for a CONSTRUCTLESS struct/tuple container
/// (call-produced — no `Construct` to inspect): the container type's burden
/// is struct-shaped (no variants; positional logical cleanup), and
/// every consume-marked view path names one of its owned-field ordinals, so
/// the skip IS the hazard's own field set (`FD_skipset_sound` — the moved
/// mark is field-unique by position; `FD_per_site_skipset_sound` gates each
/// release site by extraction domination downstream).
fn derive_constructless_positional_skip(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    hazard: &FieldViewHazard,
) -> Option<Vec<u32>> {
    use crate::lower::burden::BurdenRef;
    use crate::lower::burden_lookup::{idx_to_type_ref, lookup_burden};

    if !hazard.construct_sites.is_empty()
        || hazard.is_nested_path()
        || hazard.skip_fields.is_empty()
    {
        return None;
    }
    let (var, _) = partition.node_key(hazard.container)?;
    let &ty = func.var_types.get(var.index())?;
    let Some(BurdenRef::User(user)) =
        lookup_burden(idx_to_type_ref(ty, type_registry), type_registry)
    else {
        return None;
    };
    if !user.variant_burdens.is_empty() {
        return None;
    }
    let owned: Vec<u32> = user
        .owned_fields
        .iter()
        .filter_map(|field| field.field_path.first().copied())
        .collect();
    if !hazard.skip_fields.iter().all(|field| owned.contains(field)) {
        return None;
    }
    if !view_projections_all_move_out(func, partition, hazard.view) {
        return None;
    }
    let mut skip = hazard.skip_fields.clone();
    skip.sort_unstable();
    Some(skip)
}

/// Every member-defining Project of the view class MOVES its projection out:
/// the dst — through its function-wide `Let`-alias closure — reaches a
/// transfer position (an owned call arg, a Construct/Reuse arg, a `Set`
/// value, a `PartialApply` capture, a `Return`, or a `Jump` arg, per the
/// committed RL-2 transfer table). A borrow-read projection (a `len()`
/// receiver, a condition read) DECLINES the positional authority —
/// crediting it as an extraction would mint a reference no runtime
/// acquisition matches, and the view's re-booked plan would over-release
/// (the mixed read-and-move shape).
fn view_projections_all_move_out(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    view: NodeIdx,
) -> bool {
    use crate::aims::intraprocedural::birth_site_partition::FieldPath;
    use crate::ir::{ArcInstr, ArcTerminator, ArgOwnership};

    let mut projection_dsts: Vec<crate::ir::ArcVarId> = Vec::new();
    for arc_block in &func.blocks {
        for instr in &arc_block.body {
            let ArcInstr::Project { dst, .. } = instr else {
                continue;
            };
            let node = partition.register_node(*dst, FieldPath::whole_var());
            if partition.rep_of(node) == view {
                projection_dsts.push(*dst);
            }
        }
    }
    projection_dsts.iter().all(|&dst| {
        let closure = super::emit::close_over_let_aliases(func, std::iter::once(dst).collect());
        let transferred_in_body = func.blocks.iter().any(|arc_block| {
            arc_block.body.iter().any(|instr| match instr {
                ArcInstr::Apply {
                    args,
                    arg_ownership,
                    ..
                } => args.iter().enumerate().any(|(position, arg)| {
                    closure.contains(arg)
                        && arg_ownership.get(position) == Some(&ArgOwnership::Owned)
                }),
                ArcInstr::Construct { args, .. }
                | ArcInstr::Reuse { args, .. }
                | ArcInstr::CollectionReuse { args, .. }
                | ArcInstr::PartialApply { args, .. } => {
                    args.iter().any(|arg| closure.contains(arg))
                }
                ArcInstr::Set { value, .. } => closure.contains(value),
                _ => false,
            })
        });
        let transferred_at_terminator =
            func.blocks
                .iter()
                .any(|arc_block| match &arc_block.terminator {
                    ArcTerminator::Return { value } => closure.contains(value),
                    ArcTerminator::Jump { args, .. } => {
                        args.iter().any(|arg| closure.contains(arg))
                    }
                    ArcTerminator::Invoke {
                        args,
                        arg_ownership,
                        ..
                    }
                    | ArcTerminator::InvokeIndirect {
                        args,
                        arg_ownership,
                        ..
                    } => args.iter().enumerate().any(|(position, arg)| {
                        closure.contains(arg)
                            && arg_ownership.get(position) == Some(&ArgOwnership::Owned)
                    }),
                    _ => false,
                });
        transferred_in_body || transferred_at_terminator
    })
}

/// The `DecPartial` skip set for a SUM container — the moved-out variant's
/// ordinal — or `None` for a struct/tuple container. `Err(())` declines:
/// a sum container's skip is discriminant- and arm-conditional, so sums
/// decline (fail-closed) EXCEPT the uniform single-payload-variant shape
/// (every construct site builds the SAME one-payload variant and the view
/// is its sole payload slot; slot 0 is the tag), whose skip names the
/// variant ordinal required by variant-sensitive logical cleanup. A CONSTRUCTLESS
/// container derives the variant from the type's burden table instead
/// (`derive_constructless_enum_variant`).
pub(super) fn derive_sum_skip(
    func: &ArcFunction,
    partition: &mut BirthSitePartition,
    type_registry: &ori_types::TypeRegistry,
    interner: &ori_ir::StringInterner,
    hazard: &FieldViewHazard,
) -> Result<Option<SkipAuthority>, ()> {
    // Niche-family wrappers share the payload's ownership identity, so their
    // whole-var decrement is the payload's release — never decomposable.
    let niche_family = hazard
        .sum_enum_name
        .is_some_and(|name| name == interner.intern("Option") || name == interner.intern("Result"));
    match (hazard.is_sum_container(), hazard.sum_variant) {
        (false, _) => Ok(
            derive_constructless_enum_variant(func, partition, type_registry, hazard)
                .map(SkipAuthority::Variant)
                .or_else(|| {
                    derive_constructless_positional_skip(func, partition, type_registry, hazard)
                        .map(SkipAuthority::Positional)
                }),
        ),
        (true, Some(variant))
            if hazard.skip_fields == [1]
                && !niche_family
                && container_is_user_variant_enum(func, partition, type_registry, hazard) =>
        {
            Ok(Some(SkipAuthority::Variant(vec![variant])))
        }
        (true, _) => {
            tracing::trace!(
                target: "ori_arc::aims::class_ledger",
                view = ?partition.node_key(hazard.view),
                sum_variant = ?hazard.sum_variant,
                skip_fields = ?hazard.skip_fields,
                "field-decomposition cure declined: sum skip not arm-safe"
            );
            Err(())
        }
    }
}

//! Reuse opportunity detection from pre-collected events.
//!
//! Matches death events to allocation events in two phases:
//!
//! 1. **Same-block** — nearest subsequent allocation of same type in the same
//!    block, with no intervening uses of the dying variable.
//! 2. **Cross-block** — via [`ReusePlanner`] with dominator/post-dominator
//!    validation. v1: static-unique only.
//!
//! Events are collected inline during the unified forward walk in `realize/`.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::Idx;

use crate::aims::lattice::{Cardinality, ShapeClass, Uniqueness};
use crate::ir::{ArcBlockId, ArcFunction};

use super::planner::ReusePlanner;
use super::{AllocEvent, DeathEvent, ReuseOpportunity};

/// Match death events to allocation events within the same block.
///
/// For each death event, find the nearest subsequent allocation of the same
/// type in the same block. Ensure no use of the dying variable occurs between
/// the death and allocation points.
///
/// Consumed event set: tracks `(block_id, instr_idx)` pairs.
type ConsumedSet = FxHashSet<(ArcBlockId, usize)>;

/// Returns `(opportunities, consumed_deaths, consumed_allocs)`.
fn match_same_block(
    deaths: &[DeathEvent],
    allocs: &[AllocEvent],
    func: &ArcFunction,
) -> (Vec<ReuseOpportunity>, ConsumedSet, ConsumedSet) {
    // Group allocations by (block, type) for efficient lookup.
    let mut allocs_by_block_type: FxHashMap<(ArcBlockId, Idx), Vec<&AllocEvent>> =
        FxHashMap::default();
    for alloc in allocs {
        allocs_by_block_type
            .entry((alloc.block, alloc.ty))
            .or_default()
            .push(alloc);
    }

    let mut opportunities = Vec::new();
    let mut consumed_allocs: FxHashSet<(ArcBlockId, usize)> = FxHashSet::default();
    let mut consumed_deaths: FxHashSet<(ArcBlockId, usize)> = FxHashSet::default();

    for death in deaths {
        let key = (death.block, death.ty);
        let Some(candidates) = allocs_by_block_type.get(&key) else {
            continue;
        };

        // Find the nearest allocation AFTER the death in the same block.
        let best = candidates
            .iter()
            .filter(|a| {
                a.instr_idx > death.instr_idx && !consumed_allocs.contains(&(a.block, a.instr_idx))
            })
            .min_by_key(|a| a.instr_idx);

        let Some(alloc) = best else { continue };

        // Verify no use of the dying variable between death and allocation.
        let block = &func.blocks[death.block.index()];
        let intervening_use = block.body[death.instr_idx + 1..alloc.instr_idx]
            .iter()
            .any(|instr| instr.uses_var(death.var));

        if intervening_use {
            continue;
        }

        consumed_allocs.insert((alloc.block, alloc.instr_idx));
        consumed_deaths.insert((death.block, death.instr_idx));

        // Cross-dimensional uniqueness proof (Section 09.2 Shape Activation):
        // Once+ReusableCtor → static reuse without IsShared check.
        // Fresh construction (ReusableCtor shape) gives refcount=1.
        // Single use (Once cardinality) means no duplication occurred.
        // Therefore, even if uniqueness says MaybeShared (conservative),
        // the value is provably unique at its death point.
        let is_static = death.uniqueness == Uniqueness::Unique
            || (death.uniqueness == Uniqueness::MaybeShared
                && death.cardinality == Cardinality::Once
                && matches!(death.shape, ShapeClass::ReusableCtor(_)));
        opportunities.push(ReuseOpportunity {
            source_var: death.var,
            source_block: death.block,
            source_instr: death.instr_idx,
            target_instr: (alloc.block, alloc.instr_idx),
            is_static_unique: is_static,
        });
    }

    (opportunities, consumed_deaths, consumed_allocs)
}

/// Find reuse opportunities from pre-collected death and alloc events.
///
/// Same matching logic as [`find_reuse_opportunities`] but skips the
/// `collect_death_events()` and `collect_alloc_events()` scans. Used by
/// `realize/` when events are collected during the unified forward walk.
pub(crate) fn find_reuse_opportunities_from_events(
    func: &ArcFunction,
    death_events: &[DeathEvent],
    alloc_events: &[AllocEvent],
) -> (Vec<ReuseOpportunity>, usize) {
    let total_deaths = death_events.len();
    if death_events.is_empty() || alloc_events.is_empty() {
        return (Vec::new(), total_deaths);
    }

    // Phase 1: same-block matching.
    let (same_block_opps, consumed_deaths, consumed_allocs) =
        match_same_block(death_events, alloc_events, func);

    // Phase 2: cross-block matching for unmatched events.
    let remaining_deaths: Vec<_> = death_events
        .iter()
        .filter(|d| !consumed_deaths.contains(&(d.block, d.instr_idx)))
        .collect();
    let remaining_allocs: Vec<_> = alloc_events
        .iter()
        .filter(|a| !consumed_allocs.contains(&(a.block, a.instr_idx)))
        .collect();

    let cross_block_opps = if !remaining_deaths.is_empty() && !remaining_allocs.is_empty() {
        let mut planner = ReusePlanner::new(func);
        planner.find_opportunities(&remaining_deaths, &remaining_allocs)
    } else {
        Vec::new()
    };

    let mut all = same_block_opps;
    all.extend(cross_block_opps);
    (all, total_deaths)
}

/// Whether a constructor kind produces a reusable allocation.
pub(crate) fn is_reusable_ctor(ctor: &crate::ir::CtorKind) -> bool {
    matches!(
        ctor,
        crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::EnumVariant { .. }
    )
}

/// Map a constructor kind to its shape classification.
pub(crate) fn ctor_to_shape(ctor: &crate::ir::CtorKind) -> ShapeClass {
    match ctor {
        crate::ir::CtorKind::Struct(_) => {
            ShapeClass::ReusableCtor(crate::aims::lattice::ReuseCtorKind::Struct)
        }
        crate::ir::CtorKind::EnumVariant { .. } => {
            ShapeClass::ReusableCtor(crate::aims::lattice::ReuseCtorKind::EnumVariant)
        }
        _ => ShapeClass::NonReusable,
    }
}

//! Reuse opportunity detection from AIMS state map.
//!
//! Scans the function for death events (`RcDec` on owned, unique/maybe-shared,
//! reusable-shape variables) and allocation events (`Construct`), then matches
//! pairs in two phases:
//!
//! 1. **Same-block** — nearest subsequent allocation of same type in the same
//!    block, with no intervening uses of the dying variable.
//! 2. **Cross-block** — via [`ReusePlanner`] with dominator/post-dominator
//!    validation. Stage 1: static-unique only.

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::{Idx, Pool};

use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::{Cardinality, ShapeClass, SizeClass, Uniqueness};
use crate::ir::{ArcBlockId, ArcFunction, ArcInstr};

use super::planner::ReusePlanner;
use super::{AllocEvent, DeathEvent, ReuseOpportunity};

/// Find reuse opportunities by matching death events to allocation events.
///
/// Phase 1: same-block, same-type matching.
/// Phase 2: cross-block matching via [`ReusePlanner`] for unmatched events.
///
/// Returns `(opportunities, total_death_events)` — the death count is used
/// by `emit_reuse` for missed-reuse tracking and FBIP enrichment.
pub(super) fn find_reuse_opportunities(
    func: &ArcFunction,
    state_map: &AimsStateMap,
    _pool: &Pool,
) -> (Vec<ReuseOpportunity>, usize) {
    let death_events = collect_death_events(func, state_map);
    let total_deaths = death_events.len();
    let alloc_events = collect_alloc_events(func, state_map);

    if death_events.is_empty() || alloc_events.is_empty() {
        return (Vec::new(), total_deaths);
    }

    // Phase 1: same-block matching.
    let (same_block_opps, consumed_deaths, consumed_allocs) =
        match_same_block(&death_events, &alloc_events, func);

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

/// Collect death events: `RcDec` instructions where the variable has a
/// reusable shape and is unique or maybe-shared.
///
/// Shape is read from the per-variable shape map (populated post-convergence
/// from definition instructions), NOT from block-level exit states. This is
/// necessary because backward analysis doesn't propagate shape through use
/// sites — the block exit state for a dying variable typically has BOTTOM
/// shape (`NonReusable`).
///
/// Section 09.2 Shape Activation: per-variable shape enables death event
/// detection that was previously blocked by missing shape propagation.
pub(super) fn collect_death_events(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> Vec<DeathEvent> {
    let mut events = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = ArcBlockId::new(
            u32::try_from(block_idx)
                .unwrap_or_else(|_| panic!("block index {block_idx} exceeds u32::MAX")),
        );

        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::RcDec { var, .. } = instr {
                if state_map.is_excluded(*var) {
                    continue;
                }

                // Shape from definition instruction (per-variable, not per-block).
                // The backward analysis doesn't propagate shape through use
                // sites, so the block exit state's shape is typically BOTTOM
                // (NonReusable). The per-variable shape map has the correct
                // shape from the definition instruction.
                let shape = state_map.var_shape(*var);
                if matches!(shape, ShapeClass::NonReusable) {
                    continue;
                }

                // Uniqueness from block exit state (demand from successors).
                // For dying variables with no successor demand, this is BOTTOM
                // (Unique) — correct, as no other references exist downstream.
                // For variables also demanded by successors (branch/loop),
                // uniqueness reflects the join of successor demands.
                let state = state_map.var_state_at_block_exit(blk, *var);

                // Must be unique or maybe-shared for reuse.
                if state.uniqueness == Uniqueness::Shared {
                    continue;
                }

                let ty = func.var_types[var.index()];

                events.push(DeathEvent {
                    var: *var,
                    block: blk,
                    instr_idx,
                    uniqueness: state.uniqueness,
                    cardinality: state.cardinality,
                    ty,
                    shape,
                    size_class: SizeClass::UNKNOWN, // Stage 1: same-type matching implies same size.
                });
            }
        }
    }

    events
}

/// Collect allocation events: `Construct` instructions that produce reusable
/// values (structs, enum variants — not collections, which use `CollectionReuse`).
pub(super) fn collect_alloc_events(
    func: &ArcFunction,
    state_map: &AimsStateMap,
) -> Vec<AllocEvent> {
    let mut events = Vec::new();

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let blk = ArcBlockId::new(
            u32::try_from(block_idx)
                .unwrap_or_else(|_| panic!("block index {block_idx} exceeds u32::MAX")),
        );

        for (instr_idx, instr) in block.body.iter().enumerate() {
            if let ArcInstr::Construct { dst, ty, ctor, .. } = instr {
                // Only struct and enum constructors are reusable.
                // Collections use CollectionReuse (self-contained).
                if !is_reusable_ctor(ctor) {
                    continue;
                }

                if state_map.is_excluded(*dst) {
                    continue;
                }

                let shape = ctor_to_shape(ctor);

                events.push(AllocEvent {
                    block: blk,
                    instr_idx,
                    dst: *dst,
                    ty: *ty,
                    shape,
                    size_class: SizeClass::UNKNOWN, // Stage 1: same-type matching implies same size.
                });
            }
        }
    }

    events
}

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

/// Whether a constructor kind produces a reusable allocation.
fn is_reusable_ctor(ctor: &crate::ir::CtorKind) -> bool {
    matches!(
        ctor,
        crate::ir::CtorKind::Struct(_) | crate::ir::CtorKind::EnumVariant { .. }
    )
}

/// Map a constructor kind to its shape classification.
fn ctor_to_shape(ctor: &crate::ir::CtorKind) -> ShapeClass {
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

//! Project alias analysis for backward demand propagation.
//!
//! Computes a function-wide map from Project destinations (and their transitive
//! aliases through Let instructions and Jump-arg → block-param edges) to the
//! original Project source variable. This enables [`propagate_project_source_demand`]
//! to keep parent aggregates alive when borrowed children are used cross-block.
//!
//! # Example
//!
//! ```text
//! %3 = Project %2.0
//! %4 = Let Var(%3)
//! Jump block1, args=[%4]
//! // block1 params: [%5]
//! ```
//!
//! Maps: `%3 → %2`, `%4 → %2`, `%5 → %2`

use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

use super::super::lattice::{AccessClass, AimsState, Cardinality};
use super::state_map::AimsStateMap;

/// Compute a function-wide map from (Project destination + transitive Let
/// aliases + block param aliases) to Project source variable.
///
/// For `%3 = Project %2.0` followed by `%4 = Let Var(%3)`, maps both
/// `%3 → %2` and `%4 → %2`. For `Jump block1, args=[%3]` where block1
/// has param `%5`, maps `%5 → %2`. This enables `propagate_project_source_demand`
/// to detect demand on any alias of a projected value — including block params
/// that receive projected values via Jump arguments (CFG merges, loop headers).
///
/// Precomputed once before the worklist loop (the alias structure is static).
pub(crate) fn compute_project_alias_sources(func: &ArcFunction) -> FxHashMap<ArcVarId, ArcVarId> {
    // Step 1: Direct Project destinations → sources.
    let mut alias_sources: FxHashMap<ArcVarId, ArcVarId> = FxHashMap::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Project { dst, value, .. } = instr {
                alias_sources.insert(*dst, *value);
            }
        }
    }

    // Step 2: Extend transitively through Let aliases AND Jump arg → block
    // param edges. Both are simple variable renaming (phi-like), so the
    // same fixed-point closure applies.
    //
    // `Let { dst, Var(src) }` where `src` has a Project source means `dst`
    // inherits the same source. Similarly, `Jump { target, args }` where
    // arg N has a Project source means the target's param N inherits it.
    //
    // Without Jump propagation, a projected value routed through a CFG
    // merge or loop header block param loses its alias mapping, causing
    // `propagate_project_source_demand` to miss the parent aggregate
    // demand — premature RcDec, use-after-free.
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            // Let aliases within the block body.
            for instr in &block.body {
                if let ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } = instr
                {
                    if let Some(&source) = alias_sources.get(src) {
                        if let Entry::Vacant(e) = alias_sources.entry(*dst) {
                            e.insert(source);
                            changed = true;
                        }
                    }
                }
            }

            // Jump arg → target block param edges (phi-like renaming).
            if let ArcTerminator::Jump { target, args } = &block.terminator {
                let target_idx = target.index();
                if target_idx < func.blocks.len() {
                    for (i, &arg) in args.iter().enumerate() {
                        if let Some(&source) = alias_sources.get(&arg) {
                            if let Some(&(param_var, _)) = func.blocks[target_idx].params.get(i) {
                                if let Entry::Vacant(e) = alias_sources.entry(param_var) {
                                    e.insert(source);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    alias_sources
}

/// Propagate Project source demand: when a Project result (or its alias)
/// has demand, the Project source must also have demand to prevent premature
/// `RcDec`.
///
/// Handles four cases:
/// 1. Direct: `%3 = Project %2.0` in block A, `%3` used in block B
/// 2. Aliased: `%3 = Project %2.0`, `%4 = Let %3`, `%4` used in block B
/// 3. Transitive: chains of `Let` aliases from a Project destination
/// 4. Block params: `Jump block1, args=[%3]`, param `%5` used in block1
///
/// Without this, the parent aggregate (`%2`) would have no demand at block
/// B's entry, causing edge cleanup to emit a premature `RcDec` for `%2` —
/// use-after-free when the borrowed child is still alive.
pub(super) fn propagate_project_source_demand(
    current: &mut FxHashMap<ArcVarId, AimsState>,
    state_map: &AimsStateMap,
    project_alias_sources: &FxHashMap<ArcVarId, ArcVarId>,
) {
    let mut extra_demand: Vec<(ArcVarId, AimsState)> = Vec::new();
    for (&var, &state) in current.iter() {
        if state.cardinality == Cardinality::Absent {
            continue;
        }
        if let Some(&source) = project_alias_sources.get(&var) {
            if !state_map.is_excluded(source) {
                let mut src_demand = AimsState {
                    access: AccessClass::Borrowed,
                    cardinality: state.cardinality,
                    ..state
                };
                // Canonicalize: Rule 8 enforces Borrowed → locality <=
                // FunctionLocal. Without this, a returned child's
                // HeapEscaping locality would create an infeasible state.
                src_demand.canonicalize();
                extra_demand.push((source, src_demand));
            }
        }
    }
    for (var, demand) in extra_demand {
        let joined = current
            .get(&var)
            .map_or(demand, |existing| existing.join(&demand));
        current.insert(var, joined);
    }
}

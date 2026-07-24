//! Forward lattice propagation across aliasing instructions.

use crate::aims::lattice::{Locality, ShapeClass, Uniqueness};
use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId};

use super::super::state_map::AimsStateMap;

enum AliasKind {
    Full,
    View,
    Join,
}

/// Propagate TF-2, TF-4, TF-8, and TF-11 alias facts to a fixpoint.
pub(crate) fn propagate_alias_forward_state(state_map: &mut AimsStateMap, func: &ArcFunction) {
    let edges = collect_alias_forward_edges(func);
    loop {
        let mut changed = false;
        for (dst, kind, sources) in &edges {
            let dst = *dst;
            if state_map.is_excluded(dst) {
                continue;
            }
            changed |= match kind {
                AliasKind::Full => step_full_alias(state_map, dst, sources[0]),
                AliasKind::View => step_view_alias(state_map, dst, sources[0]),
                AliasKind::Join => step_join_alias(state_map, dst, sources),
            };
        }
        if !changed {
            break;
        }
    }
}

fn collect_alias_forward_edges(func: &ArcFunction) -> Vec<(ArcVarId, AliasKind, Vec<ArcVarId>)> {
    let mut edges = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => edges.push((*dst, AliasKind::Full, vec![*src])),
                ArcInstr::Project { dst, value, .. } => {
                    edges.push((*dst, AliasKind::View, vec![*value]));
                }
                ArcInstr::Select {
                    dst,
                    true_val,
                    false_val,
                    ..
                } => edges.push((*dst, AliasKind::Join, vec![*true_val, *false_val])),
                _ => {}
            }
        }
    }
    for (param, edge_args) in super::super::project_aliases::compute_param_edge_args(func) {
        let sources: Vec<ArcVarId> = edge_args.iter().map(|edge| edge.arg).collect();
        if !sources.is_empty() {
            edges.push((param, AliasKind::Join, sources));
        }
    }
    edges
}

fn step_full_alias(state_map: &mut AimsStateMap, dst: ArcVarId, src: ArcVarId) -> bool {
    let mut changed = false;
    if state_map.contract_uniqueness(dst).is_none() {
        if let Some(uniqueness) = state_map.contract_uniqueness(src) {
            state_map.set_var_uniqueness(dst, uniqueness);
            changed = true;
        }
    }
    if state_map.contract_locality(dst).is_none() {
        if let Some(locality) = state_map.contract_locality(src) {
            state_map.set_var_locality(dst, locality);
            changed = true;
        }
    }
    if matches!(state_map.var_shape(dst), ShapeClass::NonReusable) {
        let src_shape = state_map.var_shape(src);
        if !matches!(src_shape, ShapeClass::NonReusable) {
            state_map.set_var_shape(dst, src_shape);
            changed = true;
        }
    }
    changed
}

fn step_view_alias(state_map: &mut AimsStateMap, dst: ArcVarId, src: ArcVarId) -> bool {
    let mut changed = false;
    if state_map.contract_uniqueness(dst).is_none() {
        if let Some(mut uniqueness) = state_map.contract_uniqueness(src) {
            if uniqueness == Uniqueness::Unique
                && matches!(
                    state_map.contract_locality(src),
                    Some(Locality::HeapEscaping | Locality::Unknown)
                )
            {
                uniqueness = Uniqueness::MaybeShared;
            }
            state_map.set_var_uniqueness(dst, uniqueness);
            changed = true;
        }
    }
    if state_map.contract_locality(dst).is_none() {
        if let Some(locality) = state_map.contract_locality(src) {
            state_map.set_var_locality(dst, locality);
            changed = true;
        }
    }
    changed
}

fn step_join_alias(state_map: &mut AimsStateMap, dst: ArcVarId, sources: &[ArcVarId]) -> bool {
    let mut changed = false;
    if let Some(joined) = sources
        .iter()
        .filter_map(|source| state_map.contract_uniqueness(*source))
        .reduce(Uniqueness::join)
    {
        if state_map
            .contract_uniqueness(dst)
            .is_none_or(|current| joined > current)
        {
            state_map.set_var_uniqueness(dst, joined);
            changed = true;
        }
    }
    if let Some(joined) = sources
        .iter()
        .filter_map(|source| state_map.contract_locality(*source))
        .reduce(Locality::join)
    {
        if state_map
            .contract_locality(dst)
            .is_none_or(|current| joined > current)
        {
            state_map.set_var_locality(dst, joined);
            changed = true;
        }
    }
    changed
}

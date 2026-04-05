//! SSA-based closure variable resolution for indirect calls.
//!
//! Traces `ApplyIndirect`/`InvokeIndirect` closure variables through the
//! SSA def map to find the originating `PartialApply` instruction. This
//! provides the target function name and capture arg list, enabling the
//! ownership annotation in [`super::annotate`] to reuse the direct-call
//! ownership computation path.
//!
//! Handles alias chains (`Let { value: Var(src) }`), block parameters
//! (with multi-predecessor merging), and loop-carried back-edges (cycle
//! detection). Unresolvable closures (opaque parameters, conflicting
//! merges) return `None` → all-Borrowed fallback.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, ArcVarId};

/// SSA definition classification for closure resolution.
///
/// Captures the information needed to trace an `ApplyIndirect` closure
/// variable back to its originating `PartialApply`. Stores only the
/// minimal data (not `&ArcInstr` references) to avoid borrow conflicts
/// with the mutable annotation loop.
pub(super) enum ResolvedDef {
    /// The variable was defined by a `PartialApply` instruction.
    PartialApply {
        func: ori_ir::Name,
        capture_args: Vec<ArcVarId>,
    },
    /// The variable is a `Let { value: Var(src) }` alias.
    Alias(ArcVarId),
    /// The variable is a block parameter with multiple predecessors.
    BlockParam { predecessors: Vec<ArcVarId> },
    /// Any other definition (Project, Construct, Apply result, etc.).
    Other,
}

/// Build a mapping from variable → definition classification.
///
/// Two-pass construction:
/// 1. Walk all block body instructions to record `PartialApply`, `Alias`,
///    and `Other` definitions.
/// 2. Walk all `Jump` terminators to record block-param → predecessor
///    variable mappings.
pub(super) fn build_closure_def_map(blocks: &[ArcBlock]) -> FxHashMap<ArcVarId, ResolvedDef> {
    let mut map = FxHashMap::default();

    // Pass 1: body instructions.
    for block in blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::PartialApply {
                    dst, func, args, ..
                } => {
                    map.insert(
                        *dst,
                        ResolvedDef::PartialApply {
                            func: *func,
                            capture_args: args.clone(),
                        },
                    );
                }
                ArcInstr::Let {
                    dst,
                    value: ArcValue::Var(src),
                    ..
                } => {
                    map.insert(*dst, ResolvedDef::Alias(*src));
                }
                _ => {
                    if let Some(dst) = instr.defined_var() {
                        map.insert(dst, ResolvedDef::Other);
                    }
                }
            }
        }
    }

    // Pass 2: Jump terminators → block-param predecessors.
    for block in blocks {
        if let ArcTerminator::Jump { target, args } = &block.terminator {
            let target_idx = target.index();
            if target_idx < blocks.len() {
                let target_block = &blocks[target_idx];
                for (i, &(param_var, _)) in target_block.params.iter().enumerate() {
                    if let Some(&pred_var) = args.get(i) {
                        match map.get_mut(&param_var) {
                            Some(ResolvedDef::BlockParam { predecessors }) => {
                                predecessors.push(pred_var);
                            }
                            _ => {
                                map.insert(
                                    param_var,
                                    ResolvedDef::BlockParam {
                                        predecessors: vec![pred_var],
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    map
}

/// Trace a closure variable through SSA defs to find a `PartialApply`.
///
/// Returns `Some((target_func, capture_args))` if a unique `PartialApply`
/// is found. Returns `None` for unresolvable origins (opaque params,
/// conflicting merges, cycles).
pub(super) fn resolve_to_partial_apply(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, ResolvedDef>,
) -> Option<(ori_ir::Name, Vec<ArcVarId>)> {
    let mut visited = FxHashSet::default();
    resolve_inner(var, def_map, &mut visited)
}

fn resolve_inner(
    var: ArcVarId,
    def_map: &FxHashMap<ArcVarId, ResolvedDef>,
    visited: &mut FxHashSet<ArcVarId>,
) -> Option<(ori_ir::Name, Vec<ArcVarId>)> {
    if !visited.insert(var) {
        return None; // Cycle detected.
    }

    match def_map.get(&var) {
        Some(ResolvedDef::PartialApply { func, capture_args }) => {
            Some((*func, capture_args.clone()))
        }
        Some(ResolvedDef::Alias(src)) => resolve_inner(*src, def_map, visited),
        Some(ResolvedDef::BlockParam { predecessors }) => {
            // Non-cycled predecessors must all agree on the same target and
            // capture count. Back-edge predecessors (already in `visited`)
            // are skipped — they carry the same closure value.
            let mut resolved_target: Option<(ori_ir::Name, Vec<ArcVarId>)> = None;
            let mut has_non_cycled = false;

            for &pred_var in predecessors {
                if visited.contains(&pred_var) {
                    continue; // Back-edge cycle.
                }
                has_non_cycled = true;
                let pred_result = resolve_inner(pred_var, def_map, visited);
                match (&resolved_target, pred_result) {
                    (None, Some(result)) => {
                        resolved_target = Some(result);
                    }
                    (Some((existing_func, existing_captures)), Some((new_func, new_captures))) => {
                        if *existing_func != new_func
                            || existing_captures.len() != new_captures.len()
                        {
                            return None; // Conflicting closures.
                        }
                    }
                    (_, None) => {
                        return None; // One non-cycled predecessor is unresolvable.
                    }
                }
            }

            if has_non_cycled {
                resolved_target
            } else {
                None // All predecessors are cycles.
            }
        }
        Some(ResolvedDef::Other) | None => None,
    }
}
